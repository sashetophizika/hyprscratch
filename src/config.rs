use crate::logs::{log, LogErr};
use crate::scratchpad::{Scratchpad, ScratchpadOptions};
use crate::utils::dequote;
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
    pub scratchpads: Vec<Scratchpad>,
    pub special_titles: Vec<String>,
    pub normal_titles: Vec<String>,
    pub fickle_titles: Vec<String>,
    pub slick_titles: Vec<String>,
    pub dirty_titles: Vec<String>,
}

impl Config {
    fn find_config_files() -> Vec<String> {
        let home = var("HOME").unwrap_log(file!(), line!());
        let prepend_home = |str| format!("{home}/.config/{str}");

        [
            "hypr/hyprscratch.conf",
            "hypr/hyprscratch.toml",
            "hyprscratch/config.conf",
            "hyprscratch/config.toml",
            "hyprscratch/hyprscratch.conf",
            "hyprscratch/hyprscratch.toml",
            "hypr/hyprland.conf",
        ]
        .iter()
        .map(prepend_home)
        .filter(|x| Path::new(&x).exists())
        .collect()
    }

    fn get_config_files(config_path: Option<String>) -> Result<Vec<String>> {
        let default_configs = Self::find_config_files();
        if default_configs.is_empty() {
            log("No configuration files found".into(), "ERROR")?;
        }

        let config_files = if let Some(conf) = config_path {
            if !default_configs.contains(&conf) {
                if !Path::new(&conf).exists() {
                    log(format!("Config file not found: {conf}"), "ERROR")?;
                }
                vec![conf]
            } else {
                default_configs
            }
        } else {
            default_configs
        };
        Ok(config_files)
    }

    fn get_scratchpads(config_files: &[String]) -> Result<Vec<Scratchpad>> {
        let mut scratchpads: Vec<Scratchpad> = vec![];
        for config in config_files {
            let ext = Path::new(&config).extension().unwrap_log(file!(), line!());
            let mut config_data = if config.contains("hyprland.conf") || ext == "txt" {
                parse_config(config)?
            } else if ext == "toml" {
                parse_toml(config)?
            } else {
                parse_hyprlang(config)?
            };
            scratchpads.append(&mut config_data);
        }

        Ok(scratchpads)
    }

    pub fn new(config_path: Option<String>) -> Result<Config> {
        let config_files = Self::get_config_files(config_path)?;
        let scratchpads = Self::get_scratchpads(&config_files)?;

        log(
            format!(
                "Configuration parsed successfully, config is {:?}",
                config_files[0]
            ),
            "INFO",
        )?;

        let filter_titles = |cond: &dyn Fn(&ScratchpadOptions) -> bool| {
            scratchpads
                .clone()
                .into_iter()
                .filter(|scratchpad| cond(&scratchpad.options))
                .map(|scratchpad| scratchpad.title)
                .collect()
        };

        Ok(Config {
            special_titles: filter_titles(&|opts| opts.special),
            normal_titles: filter_titles(&|opts| !opts.special),
            fickle_titles: filter_titles(&|opts| !opts.persist && !opts.special),
            slick_titles: filter_titles(&|opts| !opts.sticky && !opts.tiled),
            dirty_titles: filter_titles(&|opts| !opts.sticky && !opts.shiny && !opts.special),
            config_file: config_files[0].clone(),
            scratchpads,
        })
    }

    pub fn reload(&mut self, config_path: Option<String>) -> Result<()> {
        *self = match config_path {
            Some(_) => Config::new(config_path)?,
            None => Config::new(Some(self.config_file.clone()))?,
        };
        Ok(())
    }
}

fn split_args(line: String) -> Vec<String> {
    let is_quote = |b: u8| b == b'\"' || b == b'\'';

    let mut args = vec![];
    let mut quotes = vec![];
    let mut inquote_word = String::new();

    for word in line.split(' ') {
        if word.is_empty() {
            continue;
        }

        let word_bytes = word.as_bytes();
        if word_bytes.len() == 1 && is_quote(word_bytes[0]) {
            if !quotes.is_empty() && quotes[quotes.len() - 1] == word_bytes[0] {
                quotes.pop();
                if quotes.is_empty() {
                    inquote_word += word;
                }
            } else {
                quotes.push(word_bytes[0]);
            }
        } else {
            if is_quote(word_bytes[0]) {
                quotes.push(word_bytes[0]);
            } else if word_bytes[0] == b'\\' && is_quote(word_bytes[1]) {
                quotes.push(word_bytes[1]);
            }

            if is_quote(word_bytes[word_bytes.len() - 1]) {
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
        if line.trim().starts_with("#") {
            continue;
        }

        if let Some(l) = line.find("hyprscratch") {
            lines.push(line.split_at(l).1.to_string());
        }
    }
    lines
}

fn warn_unknown_option(opt: &str) {
    let known_options = [
        "", "cover", "persist", "sticky", "shiny", "lazy", "show", "hide", "poly", "tiled",
        "special", "summon",
    ];
    if !known_options.contains(&opt) {
        log("Unknown scratchpad option: ".to_string() + opt, "WARN").unwrap();
    }
}

fn parse_config(config_file: &String) -> Result<Vec<Scratchpad>> {
    let known_commands = [
        "no-auto-reload",
        "get-config",
        "spotless",
        "hide-all",
        "kill-all",
        "previous",
        "version",
        "reload",
        "toggle",
        "clean",
        "eager",
        "cycle",
        "init",
        "show",
        "hide",
        "kill",
        "logs",
        "help",
    ];

    let mut buf: String = String::new();
    std::fs::File::open(config_file)?.read_to_string(&mut buf)?;
    let lines: Vec<String> = get_hyprscratch_lines(buf);

    let mut scratchpads: Vec<Scratchpad> = vec![];
    for line in lines {
        let parsed_args = split_args(line);

        if parsed_args.len() <= 1 {
            continue;
        }

        match parsed_args[1].as_str() {
            cmd if known_commands.contains(&cmd) => continue,
            _ => {
                let [title, command, opts];
                if parsed_args.len() > 2 {
                    title = dequote(&parsed_args[1]);
                    command = dequote(&parsed_args[2]);
                } else {
                    log(
                        "Unknown command or no command after title: ".to_string() + &parsed_args[1],
                        "WARN",
                    )?;
                    continue;
                }

                if parsed_args.len() > 3 {
                    parsed_args[3..].iter().for_each(|x| warn_unknown_option(x));
                    opts = parsed_args[3..].join(" ");
                } else {
                    opts = "".into();
                }

                scratchpads.push(Scratchpad::new(&title, &title, &command, &opts));
            }
        };
    }

    Ok(scratchpads)
}

enum SyntaxErr<'a> {
    MissingField(&'a str, &'a str),
    UnknownField(&'a str),
    NotInScope,
    Unopened,
    Unclosed,
    Nameless,
}

fn warn_syntax_err(err: SyntaxErr) -> Result<()> {
    let msg = match err {
        SyntaxErr::MissingField(f, n) => &format!("Field '{f}' not defined for scratchpad '{n}'"),
        SyntaxErr::UnknownField(f) => &format!("Unknown scratchpad field '{f}'"),
        SyntaxErr::NotInScope => "Not in scope",
        SyntaxErr::Unclosed => "Unclosed '{'",
        SyntaxErr::Unopened => "Unopened '}'",
        SyntaxErr::Nameless => "Scratchpad with no name",
    };
    log(format!("Syntax error in configuration: {msg}"), "WARN")
}

fn parse_hyprlang(config_file: &String) -> Result<Vec<Scratchpad>> {
    let mut buf = String::new();
    File::open(config_file)?.read_to_string(&mut buf)?;

    let mut name: String = String::new();
    let mut title: String = String::new();
    let mut command: String = String::new();
    let mut rules: String = String::new();
    let mut options: String = String::new();

    let mut scratchpads = vec![];
    let mut in_scope = false;

    let escape = |s: &str| -> String {
        dequote(&s.replace("\\\\", "^").replace("\\", "").replace("^", "\\"))
    };

    let warn_empty = |field: &str, name: &str| -> bool {
        if field.is_empty() {
            warn_syntax_err(SyntaxErr::MissingField(field, name)).unwrap_log(file!(), line!());
            return true;
        }
        false
    };

    for line in buf.lines() {
        if line.split_whitespace().any(|x| x == "{") {
            if in_scope {
                warn_syntax_err(SyntaxErr::Unclosed)?;
                continue;
            }

            if let Some(n) = line.split_whitespace().next() {
                if n == "{" {
                    warn_syntax_err(SyntaxErr::Nameless)?;
                } else {
                    name = n.into();
                }
            }

            in_scope = true;
            title = String::new();
            command = String::new();
            rules = String::new();
            options = String::new();
        } else if let Some(split) = line.split_once("=") {
            if !in_scope {
                warn_syntax_err(SyntaxErr::NotInScope)?;
                continue;
            }

            match split.0.trim() {
                "title" => title = escape(split.1),
                "command" => command = escape(split.1),
                "rules" => rules = escape(split.1),
                "options" => options = escape(split.1),
                f => warn_syntax_err(SyntaxErr::UnknownField(f))?,
            }
        } else if line.trim() == "}" {
            if !in_scope {
                warn_syntax_err(SyntaxErr::Unopened)?;
                continue;
            }
            in_scope = false;

            if warn_empty(&title, &name) || warn_empty(&command, &name) {
                continue;
            }

            let cmd = if rules.is_empty() {
                command.clone()
            } else {
                format!("[{rules}] {command}")
            };

            scratchpads.push(Scratchpad::new(&name, &title, &cmd, &options))
        }
    }

    Ok(scratchpads)
}

fn parse_toml(config_file: &String) -> Result<Vec<Scratchpad>> {
    log(
        "Toml configuration is deprecated. Convert to hyprlang.".into(),
        "WARN",
    )?;

    let mut buf = String::new();
    File::open(config_file)?.read_to_string(&mut buf)?;
    let toml = buf.parse::<Table>().unwrap();

    let get_field = |key| {
        toml.values()
            .map(|val| {
                val.get(key)
                    .unwrap_or(&Value::String("".into()))
                    .as_str()
                    .unwrap_or("")
                    .to_string()
            })
            .collect::<Vec<_>>()
    };

    let names = toml.keys().map(|k| k.into()).collect::<Vec<String>>();
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
        .collect::<Vec<_>>();

    let scratchpads: Vec<Scratchpad> = names
        .into_iter()
        .zip(titles)
        .zip(commands)
        .zip(options)
        .map(|(((n, t), c), o)| Scratchpad::new(&n, &t, &c, &o))
        .collect();

    Ok(scratchpads)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn expected_scratchpads() -> Vec<Scratchpad> {
        vec![
            Scratchpad::new(
                "btop",
                "btop",
                "[size 85% 85%] kitty --title btop -e btop",
                "cover persist sticky shiny lazy show hide poly special tiled",
            ),
            Scratchpad::new("nautilus", "Loading…", "[size 70% 80%] nautilus", ""),
            Scratchpad::new("noname", "\\\"", "\\'", "cover eager special"),
            Scratchpad::new(
                "wierd",
                " a program with ' a wierd ' name",
                " a \"command with\" \\'a wierd\\' format",
                "hide show",
            ),
        ]
    }

    #[test]
    fn test_parse_hyprlang() {
        let scratchpads = parse_hyprlang(&"./test_configs/test_hyprlang.conf".to_owned()).unwrap();
        assert_eq!(scratchpads, expected_scratchpads());
    }

    #[test]
    fn test_parse_toml() {
        let scratchpads = parse_toml(&"./test_configs/test_toml.toml".to_owned()).unwrap();
        assert_eq!(scratchpads, expected_scratchpads());
    }

    #[test]
    fn test_parse_config() {
        let scratchpads = parse_config(&"./test_configs/test_config1.txt".to_owned()).unwrap();
        let mut expected_scratchpads = expected_scratchpads();
        for i in 0..4 {
            expected_scratchpads[i].name = expected_scratchpads[i].title.clone();
        }
        assert_eq!(scratchpads, expected_scratchpads);
    }

    #[test]
    fn test_reload() {
        let mut config_file = File::create("./test_configs/test_config2.txt").unwrap();
        config_file.write(b"bind = $mainMod, a, exec, hyprscratch firefox 'firefox' cover
bind = $mainMod, b, exec, hyprscratch btop 'kitty --title btop -e btop' cover shiny eager show hide special sticky
bind = $mainMod, c, exec, hyprscratch htop 'kitty --title htop -e htop' special
bind = $mainMod, d, exec, hyprscratch cmat 'kitty --title cmat -e cmat' eager\n").unwrap();

        let config_file = "./test_configs/test_config2.txt".to_string();
        let mut config = Config::new(Some(config_file.clone())).unwrap();
        let scratchpads = vec![
            Scratchpad::new("firefox", "firefox", "firefox", "cover"),
            Scratchpad::new(
                "btop",
                "btop",
                "kitty --title btop -e btop",
                "cover shiny eager show hide special sticky",
            ),
            Scratchpad::new("htop", "htop", "kitty --title htop -e htop", "special"),
            Scratchpad::new("cmat", "cmat", "kitty --title cmat -e cmat", "eager"),
        ];
        let expected_config = Config {
            config_file,
            scratchpads,
            normal_titles: vec!["firefox".to_string(), "cmat".to_string()],
            special_titles: vec!["btop".to_string(), "htop".to_string()],
            slick_titles: vec![
                "firefox".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
            dirty_titles: vec!["firefox".to_string(), "cmat".to_string()],
            fickle_titles: vec![
                "firefox".to_string(),
                "btop".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
        };

        assert_eq!(config.scratchpads, expected_config.scratchpads);
        assert_eq!(config.normal_titles, expected_config.normal_titles);
        assert_eq!(config.special_titles, expected_config.special_titles);
        assert_eq!(config.slick_titles, expected_config.slick_titles);
        assert_eq!(config.dirty_titles, expected_config.dirty_titles);

        let mut config_path = File::create("./test_configs/test_config2.txt").unwrap();
        config_path
            .write(
                b"bind = $mainMod, a, exec, hyprscratch firefox 'firefox --private-window' special sticky
bind = $mainMod, b, exec, hyprscratch btop 'kitty --title btop -e btop'
bind = $mainMod, c, exec, hyprscratch htop 'kitty --title htop -e htop' cover shiny
bind = $mainMod, d, exec, hyprscratch cmat 'kitty --title cmat -e cmat' special\n",
            )
            .unwrap();

        let config_file = "./test_configs/test_config2.txt".to_string();
        config.reload(Some(config_file.clone())).unwrap();
        let scratchpads = vec![
            Scratchpad::new(
                "firefox",
                "firefox",
                "firefox --private-window",
                "special sticky",
            ),
            Scratchpad::new("btop", "btop", "kitty --title btop -e btop", ""),
            Scratchpad::new("htop", "htop", "kitty --title htop -e htop", "cover shiny"),
            Scratchpad::new("cmat", "cmat", "kitty --title cmat -e cmat", "special"),
        ];
        let expected_config = Config {
            config_file,
            scratchpads,
            normal_titles: vec!["btop".to_string(), "htop".to_string()],
            special_titles: vec!["firefox".to_string(), "cmat".to_string()],
            slick_titles: vec!["btop".to_string(), "htop".to_string(), "cmat".to_string()],
            dirty_titles: vec!["btop".to_string()],
            fickle_titles: vec![
                "firefox".to_string(),
                "btop".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
        };

        assert_eq!(config.scratchpads, expected_config.scratchpads);
        assert_eq!(config.normal_titles, expected_config.normal_titles);
        assert_eq!(config.special_titles, expected_config.special_titles);
        assert_eq!(config.slick_titles, expected_config.slick_titles);
        assert_eq!(config.dirty_titles, expected_config.dirty_titles);
    }
}
