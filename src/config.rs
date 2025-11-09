use crate::logs::*;
use crate::scratchpad::{Scratchpad, ScratchpadOptions};
use crate::utils::{dequote, prepend_rules};
use crate::DEFAULT_CONFIG_FILES;
use crate::KNOWN_COMMANDS;
use hyprland::Result;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

struct ConfigData {
    daemon_options: String,
    scratchpads: Vec<Scratchpad>,
    groups: HashMap<String, Vec<Scratchpad>>,
}

impl ConfigData {
    fn new() -> ConfigData {
        ConfigData {
            daemon_options: String::new(),
            scratchpads: Vec::new(),
            groups: HashMap::new(),
        }
    }

    fn from_scratchpads(scratchpads: Vec<Scratchpad>) -> ConfigData {
        ConfigData {
            daemon_options: String::new(),
            scratchpads,
            groups: HashMap::new(),
        }
    }

    fn append(&mut self, new_data: &mut ConfigData) {
        self.daemon_options.push_str(&new_data.daemon_options);
        self.scratchpads.append(&mut new_data.scratchpads);
        self.groups.extend(new_data.groups.clone());
    }

    fn add_scratchpad(&mut self, scratchpad: Scratchpad) {
        self.scratchpads.push(scratchpad);
    }

    fn add_group(&mut self, name: String, scratchpads: Vec<Scratchpad>) {
        self.groups.insert(name, scratchpads);
    }

    fn add_globals(&mut self, state: &ParserState) {
        self.daemon_options
            .push_str(&state.scratchpad_data["daemon_options"]);
        self.scratchpads.iter_mut().for_each(|sc| {
            sc.add_opts(&state.scratchpad_data["global_options"]);
            sc.add_rules(&state.scratchpad_data["global_rules"]);
        });
    }
}

struct ParserState {
    scratchpad_data: HashMap<String, String>,
    group_data: Vec<Scratchpad>,
    active_group: Option<String>,
    in_scope: bool,
}

impl ParserState {
    fn new() -> ParserState {
        let mut state = ParserState {
            scratchpad_data: HashMap::new(),
            group_data: vec![],
            active_group: None,
            in_scope: false,
        };

        let global_keys = ["daemon_options", "global_options", "global_rules"];
        for key in global_keys {
            state.scratchpad_data.insert(key.into(), "".into());
        }
        state
    }

    fn open_scratchpad(&mut self, line: &str) {
        if self.in_scope {
            warn_syntax_err(Unclosed);
            return;
        }

        if let Some(n) = line.split_whitespace().next() {
            if n == "{" {
                warn_syntax_err(Nameless);
            } else {
                self.in_scope = true;
                self.scratchpad_data.insert("name".into(), n.into());
                for f in ["title", "class", "command", "rules", "options"] {
                    self.scratchpad_data.insert(f.into(), String::new());
                }
            }
        }
    }

    fn open_group(&mut self, line: &str) {
        let s = line.find(":");
        let e = line.find(" ");
        if let (Some(s), Some(e)) = (s, e) {
            self.active_group = Some(line[s + 1..e].into());
        } else {
            warn_syntax_err(Nameless);
        }
    }

    fn validate_data(&mut self) -> bool {
        let warn_empty = |fields: &[&str]| -> bool {
            if fields.iter().all(|&f| self.scratchpad_data[f].is_empty()) {
                warn_syntax_err(MissingField(fields, &self.scratchpad_data["name"]));
                return true;
            }
            false
        };

        if warn_empty(&["title", "class"]) {
            return false;
        }

        warn_unknown_options(&self.scratchpad_data["options"]);
        true
    }

    fn create_scratchpad(&mut self) -> Scratchpad {
        let command = &prepend_rules(
            &self.scratchpad_data["command"],
            &self.scratchpad_data["rules"].replace(",", ";"),
        )
        .join("?");

        let title = if self.scratchpad_data["title"].is_empty() {
            &self.scratchpad_data["class"]
        } else {
            &self.scratchpad_data["title"]
        };

        let [name, options] = [
            &self.scratchpad_data["name"],
            &self.scratchpad_data["options"],
        ];
        Scratchpad::new(name, title, command, options)
    }

    fn append_to_field(&mut self, k: &str, v: &str) {
        let field = self.scratchpad_data.get_mut(k).unwrap();

        if field.is_empty() {
            field.push_str(v);
            return;
        }

        let sep = match k {
            "command" => "?",
            "rules" => ";",
            "options" => "",
            _ => return,
        };

        field.push_str(&format!("{sep} {v}"));
    }

    fn close_group(&mut self, config_data: &mut ConfigData) {
        if let Some(name) = self.active_group.clone() {
            config_data.add_group(name, self.group_data.clone());
            self.active_group = None;
            self.group_data = vec![];
        } else {
            warn_syntax_err(Unopened);
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Config {
    pub scratchpads: Vec<Scratchpad>,
    pub groups: HashMap<String, Vec<Scratchpad>>,
    pub ephemeral_titles: Vec<String>,
    pub special_titles: Vec<String>,
    pub normal_titles: Vec<String>,
    pub fickle_titles: Vec<String>,
    pub slick_titles: Vec<String>,
    pub dirty_titles: Vec<String>,
    pub daemon_options: String,
    pub config_file: String,
}

impl Config {
    pub fn new(config_path: Option<String>) -> Result<Config> {
        let config_files = get_config_files(config_path)?;
        let config_data = get_config_data(&config_files)?;

        log(
            format!(
                "Configuration parsed successfully, config is {:?}",
                config_files[0]
            ),
            Info,
        )?;

        let filter_titles = |cond: &dyn Fn(&ScratchpadOptions) -> bool| {
            config_data
                .scratchpads
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
            slick_titles: filter_titles(&|opts| !opts.sticky && !opts.pin),
            dirty_titles: filter_titles(&|opts| !opts.sticky && !opts.shiny && !opts.pin),
            config_file: config_files[0].clone(),
            daemon_options: config_data.daemon_options,
            scratchpads: config_data.scratchpads,
            groups: config_data.groups,
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

fn find_config_files() -> Vec<String> {
    let home = env::var("HOME").unwrap_log(file!(), line!());

    DEFAULT_CONFIG_FILES
        .iter()
        .map(|path| format!("{home}/.config/{path}"))
        .filter(|path| Path::new(&path).exists())
        .collect()
}

fn get_config_files(config_path: Option<String>) -> Result<Vec<String>> {
    let default_configs = find_config_files();
    if default_configs.is_empty() && config_path.is_none() {
        log("No configuration files found".into(), Error)?;
    }

    if let Some(conf) = config_path {
        if !default_configs.contains(&conf) && !Path::new(&conf).exists() {
            log(format!("Config file not found: {conf}"), Error)?;
        }
        return Ok(vec![conf]);
    }

    Ok(default_configs)
}

fn get_config_data(config_files: &[String]) -> Result<ConfigData> {
    let mut config_data = ConfigData::new();

    for config in config_files {
        let ext = Path::new(&config).extension().unwrap_or(OsStr::new("conf"));
        let parent = Path::new(config).parent().unwrap_log(file!(), line!());
        let mut content = String::new();
        File::open(config)?.read_to_string(&mut content)?;

        let mut new_data = if config.contains("hyprland.conf") || ext == "txt" {
            ConfigData::from_scratchpads(parse_config(&content, parent)?)
        } else {
            parse_hyprlang(&content)?
        };

        config_data.append(&mut new_data);
    }

    Ok(config_data)
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

fn get_lines_with(pat: &str, config: &str) -> Vec<String> {
    let mut lines = vec![];
    for line in config.lines() {
        if line.trim().starts_with("#") {
            continue;
        }

        if let Some(l) = line.find(pat) {
            lines.push(line.split_at(l).1.to_string());
        }
    }
    lines
}

fn warn_unknown_options(opts: &str) {
    let known_arg_options = ["monitor"];
    let known_options = [
        "",
        "pin",
        "cover",
        "persist",
        "sticky",
        "shiny",
        "lazy",
        "show",
        "hide",
        "poly",
        "tiled",
        "special",
        "ephemeral",
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

fn parse_source_config(source: &str, parent: &Path) -> Result<Vec<Scratchpad>> {
    let source_path = match source.split_once("=") {
        Some((_, s)) => s.trim(),
        None => {
            let _ = log(format!("No filename given to source in {source}"), Warn);
            return Ok(vec![]);
        }
    };

    let path = parent.join(source_path);

    if let Ok(mut conf_file) = File::open(&path) {
        let mut config = String::new();
        conf_file.read_to_string(&mut config)?;
        let parent = path.parent().unwrap_log(file!(), line!());

        let scratchpads = parse_config(&config, parent)?;
        Ok(scratchpads)
    } else {
        let _ = log(format!("Source file not found: {source_path}"), Warn);
        Ok(vec![])
    }
}

fn parse_config(config: &str, parent: &Path) -> Result<Vec<Scratchpad>> {
    let mut scratchpads: Vec<Scratchpad> = vec![];

    let lines = get_lines_with("hyprscratch", config);
    for line in lines {
        let args = split_args(line);
        if let Some([title, command, opts]) = parse_args(&args) {
            scratchpads.push(Scratchpad::new(&title, &title, &command, &opts));
        }
    }

    for source in get_lines_with("source", config).iter() {
        let mut scr = parse_source_config(source, parent)?;
        scratchpads.append(&mut scr);
    }

    Ok(scratchpads)
}

use SyntaxErr::*;
enum SyntaxErr<'a> {
    MissingField(&'a [&'a str], &'a str),
    UnknownField(&'a str),
    NameOutsideGroup,
    GlobalInScope,
    NotInScope,
    Unopened,
    Unclosed,
    Nameless,
}

fn warn_syntax_err(err: SyntaxErr) {
    let msg = match err {
        MissingField(f, n) => &format!("Field '{}' not found for scratchpad '{n}'", f.join(" or ")),
        UnknownField(f) => &format!("Unknown scratchpad field '{f}'"),
        NameOutsideGroup => "Name defined outside of a group scope",
        GlobalInScope => "Global variable defined inside scratchpad",
        NotInScope => "Field set outside of scratchpad",
        Nameless => "Scratchpad or group with no name",
        Unclosed => "Unclosed '{'",
        Unopened => "Unopened '}'",
    };
    let _ = log(format!("Syntax error in configuration: {msg}"), Warn);
}

fn open_scope(line: &str, state: &mut ParserState) {
    if line.starts_with("group:") {
        state.open_group(line);
    } else {
        state.open_scratchpad(line);
    }
}

fn close_scope(state: &mut ParserState, config_data: &mut ConfigData) {
    if !state.in_scope {
        state.close_group(config_data);
        return;
    }

    state.in_scope = false;
    if !state.validate_data() {
        return;
    }

    let scratchpad = state.create_scratchpad();
    config_data.add_scratchpad(scratchpad.clone());

    if state.active_group.is_some() {
        state.group_data.push(scratchpad);
    }
}

fn add_copy_to_group(name: String, config_data: &mut ConfigData, state: &mut ParserState) {
    if state.in_scope || state.active_group.is_none() {
        warn_syntax_err(NameOutsideGroup);
        return;
    }

    if let Some(sc) = config_data.scratchpads.iter().find(|sc| sc.name == name) {
        state.group_data.push(sc.clone());
    }
}

fn escape(s: &str) -> String {
    dequote(&s.replace("\\\\", "^").replace("\\", "").replace("^", "\\"))
}

fn set_global(state: &mut ParserState, (k, v): (&str, String)) {
    if state.in_scope {
        warn_syntax_err(GlobalInScope);
        return;
    }

    state.scratchpad_data.insert(k.into(), v);
}

fn set_field<'a>(state: &mut ParserState, (k, v): (&'a str, &'a str)) {
    if !state.in_scope {
        warn_syntax_err(NotInScope);
        return;
    }

    if state.scratchpad_data.contains_key(k) {
        state.append_to_field(k, v);
    } else {
        warn_syntax_err(UnknownField(k));
    }
}

fn set_var<'a>(split: (&'a str, &'a str), config_data: &mut ConfigData, state: &mut ParserState) {
    let (k, v) = (split.0.trim(), escape(split.1));
    match k {
        "global_options" | "global_rules" | "daemon_options" => set_global(state, (k, v)),
        "name" => add_copy_to_group(v, config_data, state),
        _ => set_field(state, (k, &v)),
    }
}

fn parse_hyprlang(config: &str) -> Result<ConfigData> {
    let mut state = ParserState::new();
    let mut config_data = ConfigData::new();

    for line in config.lines() {
        let line = line.trim();
        if line.starts_with("#") {
            continue;
        } else if let Some("{") = line.split_whitespace().last() {
            open_scope(line, &mut state)
        } else if let Some(split) = line.split_once("=") {
            set_var(split, &mut config_data, &mut state);
        } else if line == "}" {
            close_scope(&mut state, &mut config_data);
        }
    }

    warn_unknown_options(&state.scratchpad_data["global_options"]);
    config_data.add_globals(&state);

    Ok(config_data)
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
        let config_data = parse_hyprlang(&open_conf("./test_configs/test_hyprlang.conf")).unwrap();
        assert_eq!(config_data.scratchpads, expected_scratchpads());
    }

    #[test]
    fn test_parse_config() {
        let scratchpads = parse_config(
            &open_conf("./test_configs/test_config1.txt"),
            Path::new("./test_configs"),
        )
        .unwrap();

        let mut expected_scratchpads = expected_scratchpads();
        for sc in expected_scratchpads.iter_mut() {
            sc.name = sc.title.clone();
        }

        assert_eq!(scratchpads, expected_scratchpads);
    }

    #[test]
    fn test_recursive_config() {
        let scratchpads = parse_config(
            &open_conf("./test_configs/test_nested/config_main.txt"),
            Path::new("./test_configs/test_nested"),
        )
        .unwrap();

        let mut expected_scratchpads = vec![];
        for i in 1..=11 {
            let title = format!("scratch{i}");
            expected_scratchpads.push(Scratchpad::new(&title, &title, "noop", ""))
        }

        assert_eq!(scratchpads, expected_scratchpads);
    }

    struct ReloadResources {
        config_contents_a: &'static [u8],
        config_contents_b: &'static [u8],
        expected_config_a: Config,
        expected_config_b: Config,
    }

    fn create_reosources(config_file: &str) -> ReloadResources {
        ReloadResources {
            config_contents_a: b"bind = $mainMod, a, exec, hyprscratch firefox 'firefox' cover
bind = $mainMod, b, exec, hyprscratch btop 'kitty --title btop -e btop' cover shiny eager show hide special sticky
bind = $mainMod, c, exec, hyprscratch htop 'kitty --title htop -e htop' special
bind = $mainMod, d, exec, hyprscratch cmat 'kitty --title cmat -e cmat' eager\n",
            config_contents_b: b"bind = $mainMod, a, exec, hyprscratch firefox 'firefox --private-window' special sticky
bind = $mainMod, b, exec, hyprscratch btop 'kitty --title btop -e btop'
bind = $mainMod, c, exec, hyprscratch htop 'kitty --title htop -e htop' cover shiny
bind = $mainMod, d, exec, hyprscratch cmat 'kitty --title cmat -e cmat' special\n",
            expected_config_a: Config {
                config_file: config_file.to_string(),
                daemon_options: "".into(),
                groups: HashMap::new(),
                scratchpads: vec![
                Scratchpad::new("firefox", "firefox", "firefox", "cover"),
                Scratchpad::new(
                    "btop",
                    "btop",
                    "kitty --title btop -e btop",
                    "cover shiny eager show hide special sticky",
                ),
                Scratchpad::new("htop", "htop", "kitty --title htop -e htop", "special"),
                Scratchpad::new("cmat", "cmat", "kitty --title cmat -e cmat", "eager"),
            ],
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
                    "cmat".to_string(),
                ],
                ephemeral_titles: vec![],
            },
            expected_config_b: Config {
                config_file: config_file.to_string(),
                daemon_options: "".into(),
                groups: HashMap::new(),
                scratchpads: vec![
                Scratchpad::new(
                    "firefox",
                    "firefox",
                    "firefox --private-window",
                    "special sticky",
                ),
                Scratchpad::new("btop", "btop", "kitty --title btop -e btop", ""),
                Scratchpad::new("htop", "htop", "kitty --title htop -e htop", "cover shiny"),
                Scratchpad::new("cmat", "cmat", "kitty --title cmat -e cmat", "special"),
            ],
                normal_titles: vec!["btop".to_string(), "htop".to_string()],
                special_titles: vec!["firefox".to_string(), "cmat".to_string()],
                slick_titles: vec!["btop".to_string(), "htop".to_string(), "cmat".to_string()],
                dirty_titles: vec!["btop".to_string(), "cmat".to_string()],
                fickle_titles: vec![
                    "btop".to_string(),
                    "htop".to_string(),
                ],
                ephemeral_titles: vec![],
            }
        }
    }

    #[test]
    fn test_reload() {
        let config_path = "./test_configs/test_config2.txt";
        let mut config_file = File::create(config_path).unwrap();
        let resources = create_reosources(config_path);
        config_file.write(resources.config_contents_a).unwrap();

        let mut config = Config::new(Some(config_path.to_string())).unwrap();
        assert_eq!(config, resources.expected_config_a);

        let mut config_file = File::create(config_path).unwrap();
        config_file.write(resources.config_contents_b).unwrap();

        config.reload(Some(config_path.to_string())).unwrap();

        assert_eq!(config, resources.expected_config_b);
    }
}
