use hyprland::Result;
use std::sync::{Arc, Mutex};
use regex::Regex;
use std::io::prelude::*;

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

    pub fn reload(self: &mut Self) -> Result<()> {
        [self.titles, self.commands, self.options] = parse_config()?;
        self.normal_titles = self.titles
            .iter()
            .enumerate()
            .filter(|&(i, _)| !self.options[i].contains("special"))
            .map(|(_, x)| x.to_owned())
            .collect::<Vec<String>>();

        self.special_titles = self.titles
            .iter()
            .enumerate()
            .filter(|&(i, _)| self.options[i].contains("special"))
            .map(|(_, x)| x.to_owned())
            .collect::<Vec<String>>();
        
        Ok(())
    }
}

fn dequote(string: &String) -> String {
    let dequoted = match &string[0..1] {
        "\"" | "'" => &string[1..string.len() - 1],
        _ => string,
    };
    dequoted.to_string()
}

fn parse_config() -> Result<[Vec<String>; 3]> {
    let hyprscratch_lines_regex = Regex::new("hyprscratch.+").unwrap();
    let hyprscratch_args_regex = Regex::new("\".+?\"|'.+?'|[\\w.-]+").unwrap();
    let mut buf: String = String::new();

    let mut titles: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let mut options: Vec<String> = Vec::new();

    std::fs::File::open(format!(
        "{}/.config/hypr/hyprland.conf",
        std::env::var("HOME").unwrap()
    ))?
    .read_to_string(&mut buf)?;

    let lines: Vec<&str> = hyprscratch_lines_regex
        .find_iter(&buf)
        .map(|x| x.as_str())
        .collect();

    for line in lines {
        let parsed_line = &hyprscratch_args_regex
            .find_iter(line)
            .map(|x| x.as_str().to_string())
            .collect::<Vec<_>>()[..];

        if parsed_line.len() == 1 {
            continue;
        }

        match parsed_line[1].as_str() {
            "clean" | "hideall" | "reload" | "cycle" => (),
            _ => {
                titles.push(dequote(&parsed_line[1]));
                commands.push(dequote(&parsed_line[2]));

                if parsed_line.len() > 3 {
                    options.push(parsed_line[3..].join(" "));
                } else {
                    options.push(String::from(""));
                }
            }
        };
    }

    Ok([titles, commands, options])
}
