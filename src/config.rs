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
    pub fn new(config_path: Option<String>) -> Result<Config> {
        let config_file = match config_path {
            Some(config) => config,
            None => format!(
                "{}/.config/hypr/hyprland.conf",
                std::env::var("HOME").unwrap()
            ),
        };

        let [titles, commands, options] = parse_config(config_file)?;
        let normal_titles = titles
            .clone()
            .into_iter()
            .zip(options.clone())
            .filter_map(|(title, option)| {
                if !option.contains("special") {
                    Some(title)
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();

        let special_titles = titles
            .clone()
            .into_iter()
            .zip(options.clone())
            .filter_map(|(title, option)| {
                if option.contains("special") {
                    Some(title)
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();

        let shiny_titles: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(normal_titles.clone()));

        let unshiny_titles: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(
            titles
                .clone()
                .into_iter()
                .zip(options.clone())
                .filter_map(|(title, option)| {
                    if !option.contains("shiny") && !option.contains("special") {
                        Some(title)
                    } else {
                        None
                    }
                })
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

    pub fn reload(self: &mut Config, config_path: Option<String>) -> Result<()> {
        let config_file = match config_path {
            Some(config) => config,
            None => format!(
                "{}/.config/hypr/hyprland.conf",
                std::env::var("HOME").unwrap()
            ),
        };

        [self.titles, self.commands, self.options] = parse_config(config_file)?;
        self.normal_titles = self
            .titles
            .clone()
            .into_iter()
            .zip(self.options.clone())
            .filter_map(|(title, option)| {
                if !option.contains("special") {
                    Some(title)
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();

        self.special_titles = self
            .titles
            .clone()
            .into_iter()
            .zip(self.options.clone())
            .filter_map(|(title, option)| {
                if option.contains("special") {
                    Some(title)
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();

        let mut current_shiny_titles = self.shiny_titles.lock().unwrap();
        current_shiny_titles.clone_from(&self.normal_titles);

        let mut current_unshiny_titles = self.unshiny_titles.lock().unwrap();
        *current_unshiny_titles = self
            .titles
            .clone()
            .into_iter()
            .zip(self.options.clone())
            .filter_map(|(title, option)| {
                if !option.contains("shiny") && !option.contains("special") {
                    Some(title)
                } else {
                    None
                }
            })
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
        } else if !inquote_word.is_empty() {
            args.push(inquote_word);
            inquote_word = String::new();
        } else {
            args.push(word.to_string());
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

fn parse_config(config_file: String) -> Result<[Vec<String>; 3]> {
    let mut buf: String = String::new();

    let mut titles: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let mut options: Vec<String> = Vec::new();

    std::fs::File::open(config_file)?.read_to_string(&mut buf)?;

    let lines: Vec<String> = get_hyprscratch_lines(buf);
    for line in lines {
        let parsed_args = split_args(line);

        if parsed_args.len() <= 1 {
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
    use std::fs::File;

    #[test]
    fn test_parsing() {
        let expected_titles = vec![
            "btop",
            "Loading…",
            "\\\"",
            " a program with ' a wierd ' name",
        ];
        let expected_commands = vec![
            "[float;size 85% 85%;center] kitty --title btop -e btop",
            "[float;size 70% 80%;center] nautilus",
            "\\'",
            " a \"command with\" \\'a wierd\\' format",
        ];
        let expected_options = vec![
            "stack shiny onstart summon hide special",
            "",
            "stack onstart special",
            "hide summon",
        ];

        let [titles, commands, options] = parse_config("./test_configs/test_config1.txt".to_owned()).unwrap();

        assert_eq!(titles, expected_titles);
        assert_eq!(commands, expected_commands);
        assert_eq!(options, expected_options);
    }

    #[test]
    fn test_handle_reload() {
        let mut config_file = File::create("./test_configs/test_config2.txt").unwrap();
        config_file.write(b"bind = $mainMod, a, exec, hyprscratch firefox 'firefox' stack
bind = $mainMod, b, exec, hyprscratch btop 'kitty --title btop -e btop' stack shiny onstart summon hide special 
bind = $mainMod, c, exec, hyprscratch htop 'kitty --title htop -e htop' special
bind = $mainMod, d, exec, hyprscratch cmatrix 'kitty --title cmatrix -e cmatrix' onstart").unwrap();

        let mut config = Config::new(Some("./test_configs/test_config2.txt".to_string())).unwrap();
        let expected_config = Config {
            titles: vec![
                "firefox".to_string(),
                "btop".to_string(),
                "htop".to_string(),
                "cmatrix".to_string(),
            ],
            normal_titles: vec!["firefox".to_string(), "cmatrix".to_string()],
            special_titles: vec!["btop".to_string(), "htop".to_string()],
            commands: vec![
                "firefox".to_string(),
                "kitty --title btop -e btop".to_string(),
                "kitty --title htop -e htop".to_string(),
                "kitty --title cmatrix -e cmatrix".to_string(),
            ],
            options: vec![
                "stack".to_string(),
                "stack shiny onstart summon hide special".to_string(),
                "special".to_string(),
                "onstart".to_string(),
            ],
            shiny_titles: Arc::new(Mutex::new(vec![
                "firefox".to_string(),
                "cmatrix".to_string(),
            ])),
            unshiny_titles: Arc::new(Mutex::new(vec![
                "firefox".to_string(),
                "cmatrix".to_string(),
            ])),
        };

        assert_eq!(config.titles, expected_config.titles);
        assert_eq!(config.normal_titles, expected_config.normal_titles);
        assert_eq!(config.special_titles, expected_config.special_titles);
        assert_eq!(config.commands, expected_config.commands);
        assert_eq!(config.options, expected_config.options);
        assert_eq!(
            *config.shiny_titles.lock().unwrap(),
            *expected_config.shiny_titles.lock().unwrap()
        );
        assert_eq!(
            *config.unshiny_titles.lock().unwrap(),
            *expected_config.unshiny_titles.lock().unwrap()
        );

        let mut config_file = File::create("./test_configs/test_config2.txt").unwrap();
        config_file
            .write(
                b"bind = $mainMod, a, exec, hyprscratch firefox 'firefox --private-window' special
bind = $mainMod, b, exec, hyprscratch ytop 'kitty --title btop -e ytop'
bind = $mainMod, c, exec, hyprscratch htop 'kitty --title htop -e htop' stack shiny
bind = $mainMod, d, exec, hyprscratch cmatrix 'kitty --title cmatrix -e cmatrix' special",
            )
            .unwrap();

        config
            .reload(Some("./test_configs/test_config2.txt".to_string()))
            .unwrap();
        let expected_config = Config {
            titles: vec![
                "firefox".to_string(),
                "ytop".to_string(),
                "htop".to_string(),
                "cmatrix".to_string(),
            ],
            normal_titles: vec!["ytop".to_string(), "htop".to_string()],
            special_titles: vec!["firefox".to_string(), "cmatrix".to_string()],
            commands: vec![
                "firefox --private-window".to_string(),
                "kitty --title btop -e ytop".to_string(),
                "kitty --title htop -e htop".to_string(),
                "kitty --title cmatrix -e cmatrix".to_string(),
            ],
            options: vec![
                "special".to_string(),
                "".to_string(),
                "stack shiny".to_string(),
                "special".to_string(),
            ],
            shiny_titles: Arc::new(Mutex::new(vec!["ytop".to_string(), "htop".to_string()])),
            unshiny_titles: Arc::new(Mutex::new(vec!["ytop".to_string()])),
        };

        assert_eq!(config.titles, expected_config.titles);
        assert_eq!(config.normal_titles, expected_config.normal_titles);
        assert_eq!(config.special_titles, expected_config.special_titles);
        assert_eq!(config.commands, expected_config.commands);
        assert_eq!(config.options, expected_config.options);
        assert_eq!(
            *config.shiny_titles.lock().unwrap(),
            *expected_config.shiny_titles.lock().unwrap()
        );
        assert_eq!(
            *config.unshiny_titles.lock().unwrap(),
            *expected_config.unshiny_titles.lock().unwrap()
        );
    }
}
