use crate::logs::{log, LogErr};
use hyprland::Result;
use std::env::var;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::vec;
use toml::{Table, Value};

#[derive(Debug)]
pub struct Config {
    pub config_file: String,
    pub names: Vec<String>,
    pub titles: Vec<String>,
    pub normal_titles: Vec<String>,
    pub special_titles: Vec<String>,
    pub slick_titles: Vec<String>,
    pub dirty_titles: Vec<String>,
    pub non_persist_titles: Vec<String>,
    pub commands: Vec<String>,
    pub options: Vec<String>,
}

impl Config {
    pub fn new(config_path: Option<String>) -> Result<Config> {
        let config_file = config_path.unwrap_or(find_config_file());

        let [names, titles, commands, options] = if Path::new(&config_file)
            .extension()
            .unwrap_log(file!(), line!())
            == "toml"
        {
            parse_toml(&config_file)?
        } else {
            parse_config(&config_file)?
        };

        let filter_titles = |cond: &dyn Fn(&String) -> bool| {
            titles
                .clone()
                .into_iter()
                .zip(options.clone())
                .filter(|(_, opts)| cond(opts))
                .map(|(title, _)| title)
                .collect::<Vec<_>>()
        };
        let contains_any =
            |opts: &String, s: &[&str]| -> bool { opts.split(" ").any(|o| s.contains(&o)) };

        let normal_titles = filter_titles(&|opts: &String| !opts.contains("special"));
        let special_titles = filter_titles(&|opts: &String| opts.contains("special"));
        let slick_titles = filter_titles(&|opts: &String| !opts.contains("sticky"));

        let non_persist_titles =
            filter_titles(&|opts: &String| !contains_any(opts, &["persist", "special"]));
        let dirty_titles =
            filter_titles(&|opts: &String| !contains_any(opts, &["sticky", "shiny", "special"]));

        Ok(Config {
            config_file,
            names,
            titles,
            normal_titles,
            special_titles,
            slick_titles,
            dirty_titles,
            non_persist_titles,
            commands,
            options,
        })
    }

    pub fn reload(self: &mut Config, config_path: Option<String>) -> Result<()> {
        *self = match config_path {
            Some(_) => Config::new(config_path)?,
            None => Config::new(Some(self.config_file.clone()))?,
        };
        Ok(())
    }
}

fn find_config_file() -> String {
    let home = var("HOME").unwrap_log(file!(), line!());
    let paths = vec![
        format!("{home}/.config/hyprscratch/config.toml"),
        format!("{home}/.config/hyprscratch/hyprscratch.toml"),
        format!("{home}/.config/hypr/hyprscratch.toml"),
    ];

    paths
        .into_iter()
        .find(|p| Path::new(p.as_str()).exists())
        .unwrap_or(format!("{home}/.config/hypr/hyprland.conf"))
}

fn split_args(line: String) -> Vec<String> {
    let quote_types = [b'\"', b'\''];

    let mut args = vec![];
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
        if let Some(l) = line.find("hyprscratch") {
            lines.push(line.split_at(l).1.to_string());
        }
    }
    lines
}

fn parse_config(config_file: &String) -> Result<[Vec<String>; 4]> {
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

        let known_options = [
            "cover", "persist", "sticky", "shiny", "eager", "summon", "hide", "poly", "special",
        ];
        let known_commands = [
            "clean",
            "init",
            "spotless",
            "no-auto-reload",
            "hideall",
            "hide-all",
            "reload",
            "previous",
            "cycle",
            "call",
            "get-config",
            "kill",
            "logs",
            "version",
            "help",
        ];

        let dequote = |s: &String| -> String {
            match &s[..1] {
                "\"" | "'" => s[1..s.len() - 1].to_string(),
                _ => s.to_string(),
            }
        };

        match parsed_args[1].as_str() {
            cmd if known_commands.contains(&cmd) => (),
            _ => {
                if parsed_args.len() > 2 {
                    titles.push(dequote(&parsed_args[1]));
                    commands.push(dequote(&parsed_args[2]));
                } else {
                    log(
                        "Unknown command or no command after title: ".to_string() + &parsed_args[1],
                        "WARN",
                    )?;
                }

                if parsed_args.len() > 3 {
                    parsed_args[3..]
                        .iter()
                        .filter(|x| !known_options.contains(&x.as_str()))
                        .for_each(|x| {
                            log("Unknown scratchpad option: ".to_string() + x, "WARN").unwrap();
                        });
                    options.push(parsed_args[3..].join(" "));
                } else {
                    options.push(String::from(""));
                }
            }
        };
    }

    Ok([titles.clone(), titles, commands, options])
}

fn parse_toml(config_file: &String) -> Result<[Vec<String>; 4]> {
    let mut buf = String::new();
    File::open(config_file)?.read_to_string(&mut buf)?;
    let toml = buf.parse::<Table>().unwrap();

    let get_field = |key| {
        toml.values()
            .map(|val| {
                val.get(key)
                    .unwrap_or(&Value::String("".to_string()))
                    .as_str()
                    .unwrap_or("")
                    .to_string()
            })
            .collect::<Vec<_>>()
    };

    let names = toml.keys().map(|k| k.to_string()).collect::<Vec<String>>();
    let titles = get_field("title");
    let options = get_field("options");
    let commands = get_field("command")
        .into_iter()
        .zip(get_field("rules"))
        .map(|(c, r)| {
            if r.is_empty() {
                c
            } else {
                format!("[{r}] {c}")
            }
        })
        .collect();

    Ok([names, titles, commands, options])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn test_parse_toml() {
        let expected_names = vec!["btop", "nautilus", "noname", "wierd"];
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
            "cover persist sticky shiny eager summon hide poly special",
            "",
            "cover eager special",
            "hide summon",
        ];

        let [names, titles, commands, options] =
            parse_toml(&"./test_configs/test_toml.toml".to_owned()).unwrap();

        assert_eq!(names, expected_names);
        assert_eq!(titles, expected_titles);
        assert_eq!(commands, expected_commands);
        assert_eq!(options, expected_options);
    }

    #[test]
    fn test_parse_config() {
        let expected_names = vec![
            "btop",
            "Loading…",
            "\\\"",
            " a program with ' a wierd ' name",
        ];
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
            "cover persist sticky shiny eager summon hide poly special",
            "",
            "cover eager special",
            "hide summon",
        ];

        let [names, titles, commands, options] =
            parse_config(&"./test_configs/test_config1.txt".to_owned()).unwrap();

        assert_eq!(names, expected_names);
        assert_eq!(titles, expected_titles);
        assert_eq!(commands, expected_commands);
        assert_eq!(options, expected_options);
    }

    #[test]
    fn test_reload() {
        let mut config_file = File::create("./test_configs/test_config2.txt").unwrap();
        config_file.write(b"bind = $mainMod, a, exec, hyprscratch firefox 'firefox' cover
bind = $mainMod, b, exec, hyprscratch btop 'kitty --title btop -e btop' cover shiny eager summon hide special sticky
bind = $mainMod, c, exec, hyprscratch htop 'kitty --title htop -e htop' special
bind = $mainMod, d, exec, hyprscratch cmat 'kitty --title cmat -e cmat' eager").unwrap();

        let config_file = "./test_configs/test_config2.txt".to_string();
        let mut config = Config::new(Some(config_file.clone())).unwrap();
        let expected_config = Config {
            config_file,
            names: vec![
                "firefox".to_string(),
                "btop".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
            titles: vec![
                "firefox".to_string(),
                "btop".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
            normal_titles: vec!["firefox".to_string(), "cmat".to_string()],
            special_titles: vec!["btop".to_string(), "htop".to_string()],
            commands: vec![
                "firefox".to_string(),
                "kitty --title btop -e btop".to_string(),
                "kitty --title htop -e htop".to_string(),
                "kitty --title cmat -e cmat".to_string(),
            ],
            options: vec![
                "cover".to_string(),
                "cover shiny eager summon hide special sticky".to_string(),
                "special".to_string(),
                "eager".to_string(),
            ],
            slick_titles: vec![
                "firefox".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
            dirty_titles: vec!["firefox".to_string(), "cmat".to_string()],
            non_persist_titles: vec![
                "firefox".to_string(),
                "btop".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
        };

        assert_eq!(config.titles, expected_config.titles);
        assert_eq!(config.normal_titles, expected_config.normal_titles);
        assert_eq!(config.special_titles, expected_config.special_titles);
        assert_eq!(config.slick_titles, expected_config.slick_titles);
        assert_eq!(config.dirty_titles, expected_config.dirty_titles);
        assert_eq!(config.commands, expected_config.commands);
        assert_eq!(config.options, expected_config.options);

        let mut config_path = File::create("./test_configs/test_config2.txt").unwrap();
        config_path
            .write(
                b"bind = $mainMod, a, exec, hyprscratch firefox 'firefox --private-window' special sticky
bind = $mainMod, b, exec, hyprscratch btop 'kitty --title btop -e btop'
bind = $mainMod, c, exec, hyprscratch htop 'kitty --title htop -e htop' cover shiny
bind = $mainMod, d, exec, hyprscratch cmat 'kitty --title cmat -e cmat' special",
            )
            .unwrap();

        let config_file = "./test_configs/test_config2.txt".to_string();
        config.reload(Some(config_file.clone())).unwrap();
        let expected_config = Config {
            config_file,
            names: vec![
                "firefox".to_string(),
                "btop".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
            titles: vec![
                "firefox".to_string(),
                "btop".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
            normal_titles: vec!["btop".to_string(), "htop".to_string()],
            special_titles: vec!["firefox".to_string(), "cmat".to_string()],
            commands: vec![
                "firefox --private-window".to_string(),
                "kitty --title btop -e btop".to_string(),
                "kitty --title htop -e htop".to_string(),
                "kitty --title cmat -e cmat".to_string(),
            ],
            options: vec![
                "special sticky".to_string(),
                "".to_string(),
                "cover shiny".to_string(),
                "special".to_string(),
            ],
            slick_titles: vec!["btop".to_string(), "htop".to_string(), "cmat".to_string()],
            dirty_titles: vec!["btop".to_string()],
            non_persist_titles: vec![
                "firefox".to_string(),
                "btop".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
        };

        assert_eq!(config.titles, expected_config.titles);
        assert_eq!(config.normal_titles, expected_config.normal_titles);
        assert_eq!(config.special_titles, expected_config.special_titles);
        assert_eq!(config.slick_titles, expected_config.slick_titles);
        assert_eq!(config.dirty_titles, expected_config.dirty_titles);
        assert_eq!(config.commands, expected_config.commands);
        assert_eq!(config.options, expected_config.options);
    }
}
