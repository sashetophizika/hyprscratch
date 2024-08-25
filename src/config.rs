use hyprland::Result;
use std::io::prelude::*;
use std::sync::{Arc, Mutex};
use std::vec;

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
        let config_file = format!(
            "{}/.config/hypr/hyprland.conf",
            std::env::var("HOME").unwrap()
        );

        let [titles, commands, options] = parse_config(config_file)?;
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
        let config_file = format!(
            "{}/.config/hypr/hyprland.conf",
            std::env::var("HOME").unwrap()
        );

        [self.titles, self.commands, self.options] = parse_config(config_file)?;
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
    let quote_types = [b'\"', b'\''];

    let mut args: Vec<String> = vec![];
    let mut quotes = vec![];
    let mut inquote_word = String::new();

    for word in line.split(' ') {
        if word.is_empty() {
            continue;
        }

        let word_bytes = word.as_bytes();
        if word_bytes.len() == 1 && quote_types.contains(&word_bytes[0]) {
            if !quotes.is_empty() && quotes[quotes.len() - 1] == word_bytes[0] {
                quotes.pop();
            } else {
                quotes.push(word_bytes[0]);
            }
        } else {
            if quote_types.contains(&word_bytes[0]) {
                quotes.push(word_bytes[0]);
            } else if word_bytes[0] == b'\\' && quote_types.contains(&word_bytes[1]) {
                quotes.push(word_bytes[1]);
            }

            if quote_types.contains(&word_bytes[word_bytes.len() - 1]) {
                quotes.pop();
                if quotes.is_empty() {
                    inquote_word += word;
                }
            }
        }

        if !quotes.is_empty() {
            inquote_word += word;
            inquote_word += " ";
        } else if inquote_word.is_empty() {
            args.push(word.to_string());
        } else {
            args.push(inquote_word);
            inquote_word = String::new();
        }
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

fn parse_config(config_path: String) -> Result<[Vec<String>; 3]> {
    let mut buf: String = String::new();

    let mut titles: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let mut options: Vec<String> = Vec::new();

    std::fs::File::open(config_path)?.read_to_string(&mut buf)?;

    let lines: Vec<String> = get_hyprscratch_lines(buf);
    for line in lines {
        let parsed_args = split_args(line);

        if parsed_args.len() == 1 {
            continue;
        }

        match parsed_args[1].as_str() {
            "clean" | "hideall" | "reload" | "cycle" | "get-config" | "spotless" => (),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing() {
        let et = vec![
            "btop",
            "Loading…",
            "\\\"",
            " a program with ' a wierd ' name",
        ];
        let ec = vec![
            "[float;size 85% 85%;center] kitty --title btop -e btop",
            "[float;size 70% 80%;center] nautilus",
            "\\'",
            " a \"command with\" \\'a wierd\\' format",
        ];
        let eo = vec![
            "stack shiny onstart summon hide special",
            "",
            "hide summon",
            "stack onstart special",
        ];

        let [t, c, o] = parse_config("./test_config.txt".to_owned()).unwrap();

        assert!(t.iter().all(|x| et.contains(&x.as_str())));
        assert!(et.iter().all(|x| t.contains(&x.to_string())));

        assert!(c.iter().all(|x| ec.contains(&x.as_str())));
        assert!(ec.iter().all(|x| c.contains(&x.to_string())));

        assert!(o.iter().all(|x| eo.contains(&x.as_str())));
        assert!(eo.iter().all(|x| o.contains(&x.to_string())));
    }
}
