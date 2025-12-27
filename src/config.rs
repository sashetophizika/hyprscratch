use crate::logs::*;
use crate::scratchpad::{Scratchpad, ScratchpadOptions};
use crate::utils::{dequote, get_flag_arg, prepend_rules};
use crate::DEFAULT_CONFIG_FILES;
use crate::KNOWN_COMMANDS;
use hyprland::Result;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

type Scratchpads = HashMap<String, Scratchpad>;
type Groups = HashMap<String, Vec<Scratchpad>>;

struct ConfigData {
    daemon_options: String,
    scratchpads: Scratchpads,
    groups: Groups,
    names: Vec<String>,
}

impl ConfigData {
    fn new() -> ConfigData {
        ConfigData {
            daemon_options: String::new(),
            scratchpads: HashMap::new(),
            groups: HashMap::new(),
            names: Vec::new(),
        }
    }

    fn append(&mut self, new_data: &mut ConfigData) {
        self.daemon_options.push_str(&new_data.daemon_options);
        self.scratchpads.extend(new_data.scratchpads.drain());
        self.groups.extend(new_data.groups.drain());
        self.names.append(&mut new_data.names);
    }

    fn add_scratchpad(&mut self, name: &str, scratchpad: &Scratchpad) {
        self.names.push(name.into());
        self.scratchpads.insert(name.into(), scratchpad.clone());
    }

    fn add_to_group(&mut self, name: &str, scratchpads: &mut Vec<Scratchpad>) {
        if let Some(group) = self.groups.get_mut(name) {
            group.append(scratchpads);
        } else {
            self.groups.insert(name.into(), scratchpads.to_vec());
        }
    }

    fn add_to_config(&mut self, args: &[String]) {
        if let Some([title, command, opts]) = parse_args(args) {
            let scratchpad = Scratchpad::new(&title, &command, &opts);
            self.add_scratchpad(&title, &scratchpad);

            if let Some(g) = get_flag_arg(args, "group") {
                self.add_to_group(&g, &mut vec![scratchpad]);
            }
        }
    }

    fn add_globals(&mut self, state: &ParserState) {
        self.daemon_options
            .push_str(&state.scratchpad_data["daemon_options"]);
        self.scratchpads.values_mut().for_each(|sc| {
            sc.add_opts(&state.scratchpad_data["global_options"]);
            sc.add_rules(&state.scratchpad_data["global_rules"]);
        });
    }
}

struct ParserState {
    active_scratchpad: Option<String>,
    scratchpad_data: HashMap<String, String>,
    active_group: Option<String>,
    group_data: Vec<Scratchpad>,
    in_scope: bool,
}

impl ParserState {
    fn new() -> ParserState {
        let mut state = ParserState {
            active_scratchpad: None,
            scratchpad_data: HashMap::new(),
            active_group: None,
            group_data: vec![],
            in_scope: false,
        };

        let global_keys = ["daemon_options", "global_options", "global_rules"];
        for key in global_keys {
            state.scratchpad_data.insert(key.into(), String::new());
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
                self.active_scratchpad = Some(n.into());
                for f in ["title", "class", "command", "rules", "options"] {
                    self.scratchpad_data.insert(f.into(), String::new());
                }
            }
        }
    }

    fn open_group(&mut self, line: &str) {
        let s = line.find(':');
        let e = line.find(' ');
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
            &self.scratchpad_data["rules"].replace(',', ";"),
        )
        .join("?");

        let title = if self.scratchpad_data["title"].is_empty() {
            &self.scratchpad_data["class"]
        } else {
            &self.scratchpad_data["title"]
        };

        let options = &self.scratchpad_data["options"];
        Scratchpad::new(title, command, options)
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
        if let Some(name) = &self.active_group {
            config_data.add_to_group(name, &mut self.group_data);
            self.active_group = None;
            self.group_data = vec![];
        } else {
            warn_syntax_err(Unopened);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigCache {
    pub ephemeral_titles: Vec<String>,
    pub special_titles: Vec<String>,
    pub normal_titles: Vec<String>,
    pub spotless_map: HashMap<String, String>,
    pub replace_map: HashMap<String, String>,
    pub normal_map: HashMap<String, String>,
    pub clean_map: HashMap<String, String>,
}

impl ConfigCache {
    pub fn new(scratchpads: &Scratchpads) -> ConfigCache {
        let filter_titles = |cond: &dyn Fn(&ScratchpadOptions) -> bool| {
            scratchpads
                .values()
                .filter(|sc| cond(&sc.options))
                .map(|sc| sc.title.clone())
                .collect::<Vec<_>>()
        };

        let filter_maps = |cond: &dyn Fn(&ScratchpadOptions) -> bool| {
            scratchpads
                .iter()
                .filter(|(_, sc)| cond(&sc.options))
                .map(|(name, sc)| (sc.title.clone(), name.clone()))
                .collect::<HashMap<_, _>>()
        };

        ConfigCache {
            ephemeral_titles: filter_titles(&|opts| opts.ephemeral),
            special_titles: filter_titles(&|opts| opts.special),
            normal_titles: filter_titles(&|opts| !opts.special),
            normal_map: filter_maps(&|opts| !opts.special),
            replace_map: filter_maps(&|opts| !opts.persist && !opts.special),
            clean_map: filter_maps(&|opts| !opts.sticky && !opts.pin),
            spotless_map: filter_maps(&|opts| !opts.sticky && !opts.shiny && !opts.pin),
        }
    }

    fn update_titles(&mut self, options: &ScratchpadOptions, name: &str, title: &str) {
        if options.ephemeral {
            self.ephemeral_titles.push(title.into());
        }
        if options.special {
            self.special_titles.push(title.into());
        }
        if !options.special {
            self.normal_titles.push(title.into());
        }
        if !options.special && !options.persist {
            self.replace_map.insert(name.into(), title.into());
        }
        if !options.pin && !options.sticky {
            self.clean_map.insert(name.into(), title.into());
        }
        if !options.shiny && !options.sticky && !options.pin {
            self.spotless_map.insert(name.into(), title.into());
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub daemon_options: String,
    pub config_file: String,
    pub scratchpads: Scratchpads,
    pub groups: Groups,
    pub names: Vec<String>,
    pub cache: ConfigCache,
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

        Ok(Config {
            cache: ConfigCache::new(&config_data.scratchpads),
            daemon_options: config_data.daemon_options,
            config_file: config_files[0].clone(),
            scratchpads: config_data.scratchpads,
            groups: config_data.groups,
            names: config_data.names,
        })
    }

    fn update_cache(&mut self, options: &ScratchpadOptions, name: &str, title: &str) {
        self.cache.update_titles(options, name, title);
    }

    pub fn add_scratchpad(&mut self, name: &str, scratchpad: &Scratchpad) {
        if self.scratchpads.contains_key(name) {
            return;
        }

        self.update_cache(&scratchpad.options, name, &scratchpad.title);
        self.scratchpads.insert(name.into(), scratchpad.clone());
    }

    pub fn reload(&mut self, config_path: Option<String>) -> Result<()> {
        *self = match config_path {
            Some(_) => Config::new(config_path)?,
            None => Config::new(Some(self.config_file.clone()))?,
        };
        Ok(())
    }

    fn split_commands(scratchpads: &[&Scratchpad]) -> Vec<[String; 3]> {
        let split = |sc: &Scratchpad| -> Vec<[String; 3]> {
            sc.command
                .split('?')
                .map(|cmd| [sc.title.clone(), cmd.trim().into(), sc.options.as_string()])
                .collect()
        };

        scratchpads.iter().flat_map(|sc| split(&sc)).collect()
    }

    fn format_scratchpads(&mut self) -> String {
        let ordered_scratchpads: Vec<&Scratchpad> =
            self.names.iter().map(|k| &self.scratchpads[k]).collect();
        let scratchpads = Self::split_commands(&ordered_scratchpads);

        let format_field = |field: &dyn Fn(&[String; 3]) -> &str| {
            scratchpads
                .iter()
                .map(field)
                .collect::<Vec<_>>()
                .join("\u{2C02}")
        };

        let conf = &self.config_file;
        let names = self.names.join("\u{2C02}");
        let (titles, commands, options) = (
            format_field(&|x| &x[0]),
            format_field(&|x| &x[1]),
            format_field(&|x| &x[2]),
        );

        format!("{conf}\u{2C00}{names}\u{2C01}{titles}\u{2C01}{commands}\u{2C01}{options}")
    }

    fn format_groups(&self) -> String {
        type GroupField<'a> = &'a dyn Fn((&String, &Vec<Scratchpad>)) -> String;
        let format_group = |field: GroupField| {
            self.groups
                .iter()
                .map(field)
                .collect::<Vec<_>>()
                .join("\u{2C02}")
        };

        let get_titles = |scratchpads: &Vec<Scratchpad>| {
            scratchpads
                .iter()
                .map(|sc| sc.title.clone())
                .collect::<Vec<String>>()
                .join(",")
        };

        format!(
            "\u{2C00}{}\u{2C01}{}",
            format_group(&|(x, _)| x.clone()),
            format_group(&|(_, x)| get_titles(x))
        )
    }

    pub fn get_config_str(&mut self) -> String {
        let mut config_str = self.format_scratchpads();
        if !self.groups.is_empty() {
            config_str.push_str(&self.format_groups());
        }
        config_str
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
        if !Path::new(&conf).exists() {
            log(format!("Config file not found: {conf}"), Error)?;
        }

        if default_configs.contains(&conf) {
            return Ok(default_configs);
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
            parse_config(&content, parent)?
        } else {
            parse_hyprlang(&content)?
        };

        let _ = log(format!("Config file {config} parsed successfully"), Warn);
        config_data.append(&mut new_data);
    }

    Ok(config_data)
}

fn split_args(line: &str) -> Vec<String> {
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
        if line.trim().starts_with('#') {
            continue;
        }

        if let Some(l) = line.find(pat) {
            lines.push(line.split_at(l).1.to_string());
        }
    }
    lines
}

fn warn_unknown_options(opts: &str) {
    let known_arg_options = ["monitor", "group"];
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
            }
            let _ = log(format!("Unknown scratchpad option: {opt}"), Warn);
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
        3 => Some([dequote(&args[1]), dequote(&args[2]), String::new()]),
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

fn parse_source_config(source: &str, parent: &Path) -> Result<ConfigData> {
    let source_path = if let Some((_, s)) = source.split_once('=') {
        s.trim()
    } else {
        let _ = log(format!("No filename given to source in {source}"), Warn);
        return Ok(ConfigData::new());
    };

    let path = parent.join(source_path);

    if let Ok(mut conf_file) = File::open(&path) {
        let mut config = String::new();
        conf_file.read_to_string(&mut config)?;
        let parent = path.parent().unwrap_log(file!(), line!());

        let data = parse_config(&config, parent)?;
        let _ = log(format!("Source file {source_path} parsed"), Info);
        Ok(data)
    } else {
        let _ = log(format!("Source file not found: {source_path}"), Warn);
        Ok(ConfigData::new())
    }
}

fn parse_config(config: &str, parent: &Path) -> Result<ConfigData> {
    let mut config_data = ConfigData::new();

    let lines = get_lines_with("hyprscratch ", config);
    for line in lines {
        config_data.add_to_config(&split_args(&line));
    }

    for source in &get_lines_with("source ", config) {
        let mut data = parse_source_config(source, parent)?;
        config_data.append(&mut data);
    }

    Ok(config_data)
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
    if let Some(name) = &state.active_scratchpad {
        config_data.add_scratchpad(name, &scratchpad);
        state.active_scratchpad = None;
    }

    if state.active_group.is_some() {
        state.group_data.push(scratchpad);
    }
}

fn add_copy_to_group(name: &str, config_data: &mut ConfigData, state: &mut ParserState) {
    if state.in_scope || state.active_group.is_none() {
        warn_syntax_err(NameOutsideGroup);
        return;
    }

    if let Some(sc) = config_data.scratchpads.get(name) {
        state.group_data.push(sc.clone());
    }
}

fn escape(s: &str) -> String {
    dequote(&s.replace("\\\\", "^").replace('\\', "").replace('^', "\\"))
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
        "name" => add_copy_to_group(&v, config_data, state),
        _ => set_field(state, (k, &v)),
    }
}

fn parse_hyprlang(config: &str) -> Result<ConfigData> {
    let mut state = ParserState::new();
    let mut config_data = ConfigData::new();

    for line in config.lines().map(|l| l.trim()) {
        if line.starts_with('#') {
            continue;
        } else if let Some("{") = line.split_whitespace().last() {
            open_scope(line, &mut state);
        } else if let Some(split) = line.split_once('=') {
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
    use std::{fs::File, vec};

    fn expected_scratchpads(mode: bool) -> Scratchpads {
        let names = if mode {
            vec![
                "btop",
                "Loading…",
                "\\\"",
                " a program with ' a wierd ' name",
            ]
        } else {
            vec!["btop", "nautilus", "noname", "wierd"]
        };

        let scratchpads = vec![
            Scratchpad::new(
                "btop",
                "[size 85% 85%] kitty --title btop -e btop",
                "cover persist sticky shiny lazy show hide poly special tiled",
            ),
            Scratchpad::new("Loading…", "[size 70% 80%] nautilus", ""),
            Scratchpad::new("\\\"", "\\'", "cover lazy special"),
            Scratchpad::new(
                " a program with ' a wierd ' name",
                " a \"command with\" \\'a wierd\\' format",
                "hide show",
            ),
        ];

        names
            .into_iter()
            .zip(scratchpads)
            .map(|(n, s)| (n.into(), s))
            .collect()
    }

    fn expected_groups() -> Groups {
        let scs = expected_scratchpads(false);
        let mut groups = HashMap::new();
        groups.insert(
            "one".into(),
            vec![scs["nautilus"].clone(), scs["noname"].clone()],
        );
        groups.insert(
            "two".into(),
            vec![scs["btop"].clone(), scs["wierd"].clone()],
        );
        groups.insert(
            "three".into(),
            vec![scs["btop"].clone(), scs["noname"].clone()],
        );
        groups
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
        assert_eq!(config_data.scratchpads, expected_scratchpads(false));
    }

    #[test]
    fn test_groups() {
        let config_data = parse_hyprlang(&open_conf("./test_configs/test_hyprlang.conf")).unwrap();
        assert_eq!(config_data.groups, expected_groups());
    }

    #[test]
    fn test_parse_config() {
        let config_data = parse_config(
            &open_conf("./test_configs/test_config1.txt"),
            Path::new("./test_configs"),
        )
        .unwrap();

        let expected_scratchpads = expected_scratchpads(true);
        assert_eq!(config_data.scratchpads, expected_scratchpads);
    }

    #[test]
    fn test_recursive_config() {
        let config_data = parse_config(
            &open_conf("./test_configs/test_nested/config_main.txt"),
            Path::new("./test_configs/test_nested"),
        )
        .unwrap();

        let mut expected_scratchpads = HashMap::new();
        for i in 1..=11 {
            let title = format!("scratch{i}");
            expected_scratchpads.insert(title.clone(), Scratchpad::new(&title, "noop", ""));
        }

        assert_eq!(config_data.scratchpads, expected_scratchpads);
    }

    struct ReloadResources {
        config_contents_a: &'static [u8],
        config_contents_b: &'static [u8],
        expected_config_a: Config,
        expected_config_b: Config,
    }

    fn create_scratchpads(scratchpads: Vec<Scratchpad>) -> Scratchpads {
        scratchpads
            .into_iter()
            .map(|sc| (sc.title.clone(), sc))
            .collect()
    }

    fn create_reosources(config_file: &str) -> ReloadResources {
        ReloadResources {
            config_contents_a: b"bind = $mainMod, a, exec, hyprscratch firefox 'firefox' cover
bind = $mainMod, b, exec, hyprscratch btop 'kitty --title btop -e btop' cover shiny lazy show hide special sticky
bind = $mainMod, c, exec, hyprscratch htop 'kitty --title htop -e htop' special
bind = $mainMod, d, exec, hyprscratch cmat 'kitty --title cmat -e cmat' lazy\n",
            config_contents_b: b"bind = $mainMod, a, exec, hyprscratch firefox 'firefox --private-window' special sticky
bind = $mainMod, b, exec, hyprscratch btop 'kitty --title btop -e btop'
bind = $mainMod, c, exec, hyprscratch htop 'kitty --title htop -e htop' cover shiny
bind = $mainMod, d, exec, hyprscratch cmat 'kitty --title cmat -e cmat' special\n",
            expected_config_a: Config {
                config_file: config_file.to_string(),
                daemon_options: String::new(),
                groups: HashMap::new(),
                names: vec!["firefox".into(), "btop".into(), "htop".into(), "cmat".into()],
                scratchpads: create_scratchpads(vec![
                Scratchpad::new("firefox", "firefox", "cover"),
                Scratchpad::new(
                    "btop",
                    "kitty --title btop -e btop",
                    "cover shiny lazy show hide special sticky",
                ),
                Scratchpad::new("htop", "kitty --title htop -e htop", "special"),
                Scratchpad::new("cmat", "kitty --title cmat -e cmat", "lazy"),
            ]),
                cache: ConfigCache {
                    normal_titles: vec!["firefox".to_string(), "cmat".to_string()],
                    special_titles: vec!["btop".to_string(), "htop".to_string()],
                    ephemeral_titles: vec![],
                    normal_map: HashMap::from([
                        ("firefox".to_string(), "firefox".to_string()),
                        ("cmat".to_string(), "cmat".to_string()),
                    ]),
                    replace_map: HashMap::from([
                        ("firefox".to_string(), "firefox".to_string()),
                        ("cmat".to_string(), "cmat".to_string()),
                    ]),
                    clean_map: HashMap::from([
                        ("firefox".to_string(), "firefox".to_string()),
                        ("htop".to_string(), "htop".to_string()),
                        ("cmat".to_string(), "cmat".to_string()),
                    ]),
                    spotless_map: HashMap::from([
                        ("firefox".to_string(), "firefox".to_string()),
                        ("htop".to_string(), "htop".to_string()),
                        ("cmat".to_string(), "cmat".to_string()),
                    ]),
                },
            },
            expected_config_b: Config {
                config_file: config_file.to_string(),
                daemon_options: String::new(),
                groups: HashMap::new(),
                names: vec!["firefox".into(), "btop".into(), "htop".into(), "cmat".into()],
                scratchpads: create_scratchpads(vec![
                Scratchpad::new(
                    "firefox",
                    "firefox --private-window",
                    "special sticky",
                ),
                Scratchpad::new( "btop", "kitty --title btop -e btop", ""),
                Scratchpad::new( "htop", "kitty --title htop -e htop", "cover shiny"),
                Scratchpad::new( "cmat", "kitty --title cmat -e cmat", "special"),
            ]),
                cache: ConfigCache {
                    normal_titles: vec!["btop".to_string(), "htop".to_string()],
                    special_titles: vec!["firefox".to_string(), "cmat".to_string()],
                    ephemeral_titles: vec![],
                    normal_map: HashMap::from([
                        ("btop".to_string(), "btop".to_string()),
                        ("htop".to_string(), "htop".to_string()),
                    ]),
                    replace_map: HashMap::from([
                        ("btop".to_string(), "btop".to_string()),
                        ("htop".to_string(), "htop".to_string()),
                    ]),
                    clean_map: HashMap::from([
                        ("btop".to_string(), "btop".to_string()),
                        ("htop".to_string(), "htop".to_string()),
                        ("cmat".to_string(), "cmat".to_string()),
                    ]),
                    spotless_map: HashMap::from([
                        ("btop".to_string(), "btop".to_string()),
                        ("cmat".to_string(), "cmat".to_string()),
                    ]),
                },
            }
        }
    }

    fn compare_unsorted(v1: &mut Vec<String>, v2: &mut Vec<String>) {
        v1.sort();
        v2.sort();
        assert_eq!(v1, v2);
    }

    fn compare_configs(c1: &mut Config, c2: &mut Config) {
        assert_eq!(c1.scratchpads, c2.scratchpads);
        assert_eq!(c1.cache.clean_map, c2.cache.clean_map);
        assert_eq!(c1.cache.normal_map, c2.cache.normal_map);
        assert_eq!(c1.cache.replace_map, c2.cache.replace_map);
        assert_eq!(c1.cache.spotless_map, c2.cache.spotless_map);
        compare_unsorted(&mut c1.cache.normal_titles, &mut c2.cache.normal_titles);
        compare_unsorted(&mut c1.cache.special_titles, &mut c2.cache.special_titles);
        compare_unsorted(
            &mut c1.cache.ephemeral_titles,
            &mut c2.cache.ephemeral_titles,
        );
    }

    #[test]
    fn test_reload() {
        let config_path = "./test_configs/test_config2.txt";
        let mut config_file = File::create(config_path).unwrap();
        let mut resources = create_reosources(config_path);
        config_file.write_all(resources.config_contents_a).unwrap();

        let mut config_b = Config::new(Some(config_path.to_string())).unwrap();
        let mut config_a = config_b.clone();

        let mut config_file = File::create(config_path).unwrap();
        config_file.write_all(resources.config_contents_b).unwrap();

        config_b.reload(Some(config_path.to_string())).unwrap();

        compare_configs(&mut config_a, &mut resources.expected_config_a);
        compare_configs(&mut config_b, &mut resources.expected_config_b);
    }
}
