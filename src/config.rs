use hyprland::Result;
use std::io::prelude::*;
use std::sync::{Arc, Mutex};

pub struct Config {
    pub titles: Vec<String>,
    pub normal_titles: Vec<String>,
    pub special_titles: Vec<String>,
    pub commands: Vec<String>,
    pub options: Vec<String>,
    pub shiny_titles: Arc<Mutex<Vec<String>>>,
    pub unshiny_titles: Arc<Mutex<Vec<String>>>,
}

impl Config {
    pub fn new() -> Result<Config> {
        let [titles, commands, options] = parse_config()?;
        let normal_titles = titles
            .iter()
            .enumerate()
            .filter(|&(i, _)| !options[i].contains("special"))
            .map(|(_, x)| x.to_owned())
            .collect::<Vec<String>>();

        let special_titles = titles
            .iter()
            .enumerate()
            .filter(|&(i, _)| options[i].contains("special"))
            .map(|(_, x)| x.to_owned())
            .collect::<Vec<String>>();

        let shiny_titles: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(normal_titles.clone()));

        let unshiny_titles: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(
            titles
                .iter()
                .cloned()
                .enumerate()
                .filter(|&(i, _)| !(options[i].contains("shiny") || options[i].contains("special")))
                .map(|(_, x)| x)
                .collect(),
        ));

        Ok(Config {
            titles,
            normal_titles,
            special_titles,
            commands,
            options,
            shiny_titles,
            unshiny_titles,
        })
    }

    pub fn reload(self: &mut Config) -> Result<()> {
        [self.titles, self.commands, self.options] = parse_config()?;
        self.normal_titles = self
            .titles
            .iter()
            .enumerate()
            .filter(|&(i, _)| !self.options[i].contains("special"))
            .map(|(_, x)| x.to_owned())
            .collect::<Vec<String>>();

        self.special_titles = self
            .titles
            .iter()
            .enumerate()
            .filter(|&(i, _)| self.options[i].contains("special"))
            .map(|(_, x)| x.to_owned())
            .collect::<Vec<String>>();

        let mut current_shiny_titles = self.shiny_titles.lock().unwrap();
        current_shiny_titles.clone_from(&self.normal_titles);

        let mut current_unshiny_titles = self.unshiny_titles.lock().unwrap();
        *current_unshiny_titles = self
            .titles
            .iter()
            .cloned()
            .enumerate()
            .filter(|&(i, _)| {
                !self.options[i].contains("shiny") && !self.options[i].contains("special")
            })
            .map(|(_, x)| x)
            .collect();

        Ok(())
    }
}

fn split_args(line: String) -> Vec<String> {
    let mut args: Vec<String> = vec![];
    let mut word = String::new();
    let mut open_quote = '\0';
    let mut previous_char = '\0';

    for char in line.chars() {
        if let ' ' | '\n' = char {
            if open_quote != '\0' {
                word.push(char);
                continue;
            }

            if !word.is_empty() {
                args.push(word);
                word = String::new();
            }
        } else if let '\"' | '\'' = char {
            if (open_quote != '\0' && open_quote != char) || previous_char == '\\' {
                word.push(char);
                continue;
            }

            if open_quote == '\0' {
                open_quote = char;
            } else {
                open_quote = '\0';
            }

            if !word.is_empty() {
                args.push(word);
                word = String::new();
            }
        } else {
            word.push(char);
        }

        previous_char = char;
    }
    args
}

fn get_hyprscratch_lines(config_file: String) -> Vec<String> {
    let mut lines = vec![];
    for line in config_file.lines() {
        if let Some(i) = line.find("hyprscratch") {
            lines.push(line.split_at(i).1.to_string());
        }
    }
    lines
}

fn dequote(string: &String) -> String {
    let dequoted = match &string[0..1] {
        "\"" | "'" => &string[1..string.len() - 1],
        _ => string,
    };
    dequoted.to_string()
}

fn parse_config() -> Result<[Vec<String>; 3]> {
    let mut buf: String = String::new();

    let mut titles: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let mut options: Vec<String> = Vec::new();

    std::fs::File::open(format!(
        "{}/.config/hypr/hyprland.conf",
        std::env::var("HOME").unwrap()
    ))?
    .read_to_string(&mut buf)?;

    let lines: Vec<String> = get_hyprscratch_lines(buf);
    for line in lines {
        let parsed_args = split_args(line);

        if parsed_args.len() == 1 {
            continue;
        }

        match parsed_args[1].as_str() {
            "clean" | "hideall" | "reload" | "cycle" => (),
            _ => {
                titles.push(dequote(&parsed_args[1]));
                commands.push(dequote(&parsed_args[2]));

                if parsed_args.len() > 3 {
                    options.push(parsed_args[3..].join(" "));
                } else {
                    options.push(String::from(""));
                }
            }
        };
    }

    Ok([titles, commands, options])
}
