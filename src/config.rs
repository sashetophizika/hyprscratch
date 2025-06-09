use crate::logs::*;
use crate::scratchpad::{Scratchpad, ScratchpadOptions};
use crate::utils::dequote;
use crate::DEFAULT_CONFIG_FILES;
use crate::KNOWN_COMMANDS;
use hyprland::Result;
use std::collections::HashMap;
use std::env::var;
use std::ffi::OsStr;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use toml::{Table, Value};

#[derive(Debug)]
pub struct Config {
    pub scratchpads: Vec<Scratchpad>,
    pub ephemeral_titles: Vec<String>,
    pub special_titles: Vec<String>,
    pub normal_titles: Vec<String>,
    pub pinned_titles: Vec<String>,
    pub fickle_titles: Vec<String>,
    pub slick_titles: Vec<String>,
    pub dirty_titles: Vec<String>,
    pub config_file: String,
}

impl Config {
    fn find_config_files() -> Vec<String> {
        let home = var("HOME").unwrap_log(file!(), line!());
        let prepend_home = |str| format!("{home}/.config/{str}");

        DEFAULT_CONFIG_FILES
            .iter()
            .map(prepend_home)
            .filter(|x| Path::new(&x).exists())
            .collect()
    }

    fn get_config_files(config_path: Option<String>) -> Result<Vec<String>> {
        let default_configs = Self::find_config_files();
        if default_configs.is_empty() {
            log("No configuration files found".into(), Error)?;
        }

        let config_files = if let Some(conf) = config_path {
            if !default_configs.contains(&conf) {
                if !Path::new(&conf).exists() {
                    log(format!("Config file not found: {conf}"), Error)?;
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
            let ext = Path::new(&config).extension().unwrap_or(OsStr::new("conf"));
            let mut config_str = String::new();
            File::open(config)?.read_to_string(&mut config_str)?;

            let mut config_data = if config.contains("hyprland.conf") || ext == "txt" {
                parse_config(&config_str)?
            } else if ext == "toml" {
                parse_toml(&config_str)?
            } else {
                parse_hyprlang(&config_str)?
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
            Info,
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
            ephemeral_titles: filter_titles(&|opts| opts.ephemeral),
            special_titles: filter_titles(&|opts| opts.special),
            normal_titles: filter_titles(&|opts| !opts.special),
            fickle_titles: filter_titles(&|opts| !opts.persist && !opts.special),
            pinned_titles: filter_titles(&|opts| !opts.special && opts.pin),
            slick_titles: filter_titles(&|opts| !opts.sticky && !opts.pin),
            dirty_titles: filter_titles(&|opts| !opts.sticky && !opts.shiny && !opts.pin),
            config_file: config_files[0].clone(),
            scratchpads,
        })
    }

    fn find_daemon_options(config: &str) -> String {
        for line in config.lines() {
            if line.starts_with('#') {
                continue;
            } else if let Some((k, v)) = line.split_once('=') {
                if k.trim() == "daemon_options" {
                    return v.into();
                }
            }
        }

        "".into()
    }

    pub fn get_daemon_options(config_path: Option<String>) -> Result<String> {
        let config_files = Self::get_config_files(config_path)?;
        for config in config_files {
            let mut config_str = String::new();
            File::open(&config)?.read_to_string(&mut config_str)?;

            let ext = Path::new(&config).extension().unwrap_or(OsStr::new("conf"));
            if !config.contains("hyprland.conf") && ext == "conf" {
                return Ok(Self::find_daemon_options(&config_str));
            };
        }
        Ok("".into())
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

fn get_hyprscratch_lines(config: &str) -> Vec<String> {
    let mut lines = vec![];
    for line in config.lines() {
        if line.trim().starts_with("#") {
            continue;
        }

        if let Some(l) = line.find("hyprscratch") {
            lines.push(line.split_at(l).1.to_string());
        }
    }
    lines
}

fn warn_unknown_options(opts: &str) {
    let known_arg_options = ["monitor"];
    let known_options = [
        "", "pin", "cover", "persist", "sticky", "shiny", "lazy", "show", "hide", "poly", "tiled",
        "special",
    ];

    let warn_unknown = |opt: &str, is_arg: bool| -> bool {
        if is_arg {
            return false;
        }
        if !known_options.contains(&opt) {
            if known_arg_options.contains(&opt) {
                return true;
            } else {
                let _ = log(format!("Unknown scratchpad option: {opt}"), Warn);
            }
        }
        false
    };

    opts.split_whitespace()
        .fold(false, |acc, x| warn_unknown(x, acc));
}

fn parse_args(args: &[String]) -> Option<[String; 3]> {
    if KNOWN_COMMANDS.contains(&args.get(1).map_or("", |s| s.as_str())) {
        return None;
    }

    match args.len() {
        4.. => {
            let opts = args[3..].join(" ");
            warn_unknown_options(&opts);
            Some([dequote(&args[1]), dequote(&args[2]), opts])
        }
        3 => Some([dequote(&args[1]), dequote(&args[2]), "".into()]),
        2 => {
            let _ = log(
                format!("Unknown command or no command after title: {}", args[1]),
                Warn,
            );
            None
        }
        _ => {
            let _ = log("Use without arguments is not supported".into(), Warn);
            None
        }
    }
}

fn parse_config(config: &str) -> Result<Vec<Scratchpad>> {
    let lines: Vec<String> = get_hyprscratch_lines(config);

    let mut scratchpads: Vec<Scratchpad> = vec![];
    for line in lines {
        let args = split_args(line);
        if let Some([title, command, opts]) = parse_args(&args) {
            scratchpads.push(Scratchpad::new(&title, &title, &command, &opts));
        }
    }

    Ok(scratchpads)
}

use SyntaxErr::*;
enum SyntaxErr<'a> {
    MissingField(&'a str, &'a str),
    UnknownField(&'a str),
    GlobalInScope,
    NotInScope,
    Unopened,
    Unclosed,
    Nameless,
}

fn warn_syntax_err(err: SyntaxErr) {
    let msg = match err {
        MissingField(f, n) => &format!("Field '{f}' not defined for scratchpad '{n}'"),
        UnknownField(f) => &format!("Unknown scratchpad field '{f}'"),
        GlobalInScope => "Global variable defined inside scratchpad",
        NotInScope => "Field set outside of scratchpad",
        Unclosed => "Unclosed '{'",
        Unopened => "Unopened '}'",
        Nameless => "Scratchpad with no name",
    };
    let _ = log(format!("Syntax error in configuration: {msg}"), Warn);
}

fn open_scope(scratchpad_data: &mut HashMap<&str, String>, in_scope: &mut bool, line: &str) {
    if *in_scope {
        warn_syntax_err(Unclosed);
        return;
    }

    if let Some(n) = line.split_whitespace().next() {
        if n == "{" {
            warn_syntax_err(Nameless);
        } else {
            *in_scope = true;
            scratchpad_data.insert("name", n.into());
            for f in ["title", "command", "rules", "options"] {
                scratchpad_data.insert(f, String::new());
            }
        }
    }
}

fn validate_data(scratchpad_data: &HashMap<&str, String>) -> bool {
    let warn_empty = |field: &str, name: &str| -> bool {
        if field.is_empty() {
            warn_syntax_err(MissingField(field, name));
            return true;
        }
        false
    };

    if warn_empty(&scratchpad_data["title"], &scratchpad_data["name"]) {
        return false;
    }

    if warn_empty(&scratchpad_data["command"], &scratchpad_data["name"]) {
        return false;
    }

    warn_unknown_options(&scratchpad_data["options"]);
    warn_unknown_options(&scratchpad_data["global_options"]);
    true
}

fn close_scope(
    scratchpad_data: &HashMap<&str, String>,
    in_scope: &mut bool,
    scratchpads: &mut Vec<Scratchpad>,
) {
    if !*in_scope {
        warn_syntax_err(Unopened);
        return;
    }

    *in_scope = false;
    if !validate_data(scratchpad_data) {
        return;
    }

    let command = &if scratchpad_data["rules"].is_empty() {
        scratchpad_data["command"].clone()
    } else {
        format!(
            "[{}] {}",
            scratchpad_data["rules"].replace(",", ";"),
            scratchpad_data["command"]
        )
    };

    let [name, title, options] = [
        &scratchpad_data["name"],
        &scratchpad_data["title"],
        &scratchpad_data["options"],
    ];

    scratchpads.push(Scratchpad::new(name, title, command, options));
}

fn escape(s: &str) -> String {
    dequote(&s.replace("\\\\", "^").replace("\\", "").replace("^", "\\"))
}

fn set_global<'a>(
    scratchpad_data: &mut HashMap<&'a str, String>,
    in_scope: bool,
    (k, v): (&'a str, &'a str),
) {
    if in_scope {
        warn_syntax_err(GlobalInScope);
        return;
    }

    scratchpad_data.insert(k, escape(v));
}

fn set_field<'a>(
    scratchpad_data: &mut HashMap<&'a str, String>,
    in_scope: bool,
    (k, v): (&'a str, &'a str),
) {
    if !in_scope {
        warn_syntax_err(NotInScope);
        return;
    }

    if scratchpad_data.contains_key(k) {
        scratchpad_data.insert(k, escape(v));
    } else {
        warn_syntax_err(UnknownField(k));
    }
}

fn set_var<'a>(
    scratchpad_data: &mut HashMap<&'a str, String>,
    in_scope: bool,
    split: (&'a str, &'a str),
) {
    let k = split.0.trim();
    if k.contains("global") {
        set_global(scratchpad_data, in_scope, (k, split.1));
    } else {
        set_field(scratchpad_data, in_scope, (k, split.1));
    }
}

fn initialize_globals(scratchpad_data: &mut HashMap<&str, String>) {
    scratchpad_data.insert("global_options", "".into());
    scratchpad_data.insert("global_rules", "".into());
}

fn parse_hyprlang(config: &str) -> Result<Vec<Scratchpad>> {
    let mut scratchpad_data: HashMap<&str, String> = HashMap::new();
    initialize_globals(&mut scratchpad_data);

    let mut scratchpads = vec![];
    let mut in_scope = false;

    for line in config.lines() {
        if line.starts_with("#") {
            continue;
        } else if line.split_whitespace().any(|s| s == "{") {
            open_scope(&mut scratchpad_data, &mut in_scope, line)
        } else if let Some(split) = line.split_once("=") {
            set_var(&mut scratchpad_data, in_scope, split);
        } else if line.trim() == "}" {
            close_scope(&scratchpad_data, &mut in_scope, &mut scratchpads);
        }
    }

    scratchpads.iter_mut().for_each(|sc| {
        sc.append_opts(&scratchpad_data["global_options"]);
        sc.append_rules(&scratchpad_data["global_rules"]);
    });
    Ok(scratchpads)
}

fn parse_toml(config: &str) -> Result<Vec<Scratchpad>> {
    log(
        "Toml configuration is deprecated. Convert to hyprlang.".into(),
        Warn,
    )?;

    let toml = config.parse::<Table>().unwrap();
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
    use pretty_assertions::assert_eq;
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

    fn open_conf(config_file: &str) -> String {
        let mut conf = String::new();
        File::open(config_file)
            .unwrap()
            .read_to_string(&mut conf)
            .unwrap();
        conf
    }
    #[test]
    fn test_parse_hyprlang() {
        println!("{}", &open_conf("./test_configs/test_hyprlang.conf"));
        let scratchpads = parse_hyprlang(&open_conf("./test_configs/test_hyprlang.conf")).unwrap();
        assert_eq!(scratchpads, expected_scratchpads());
    }

    #[test]
    fn test_parse_toml() {
        let scratchpads = parse_toml(&open_conf("./test_configs/test_toml.toml")).unwrap();
        assert_eq!(scratchpads, expected_scratchpads());
    }

    #[test]
    fn test_parse_config() {
        let scratchpads = parse_config(&open_conf("./test_configs/test_config1.txt")).unwrap();
        let mut expected_scratchpads = expected_scratchpads();
        for sc in expected_scratchpads.iter_mut() {
            sc.name = sc.title.clone();
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
            dirty_titles: vec![
                "firefox".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
            fickle_titles: vec![
                "firefox".to_string(),
                "btop".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
            pinned_titles: vec![],
            ephemeral_titles: vec![],
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
            dirty_titles: vec!["btop".to_string(), "cmat".to_string()],
            fickle_titles: vec![
                "firefox".to_string(),
                "btop".to_string(),
                "htop".to_string(),
                "cmat".to_string(),
            ],
            pinned_titles: vec![],
            ephemeral_titles: vec![],
        };

        assert_eq!(config.scratchpads, expected_config.scratchpads);
        assert_eq!(config.normal_titles, expected_config.normal_titles);
        assert_eq!(config.special_titles, expected_config.special_titles);
        assert_eq!(config.slick_titles, expected_config.slick_titles);
        assert_eq!(config.dirty_titles, expected_config.dirty_titles);
    }
}
