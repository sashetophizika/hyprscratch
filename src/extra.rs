use crate::logs::*;
use crate::{DEFAULT_LOGFILE, DEFAULT_SOCKET};
use hyprland::Result;
use std::cmp::max;
use std::fs::File;
use std::io::prelude::*;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::vec;

type ParsedConfig<'a> = (&'a str, Vec<Vec<&'a str>>, Vec<Vec<&'a str>>);

fn print_table_outline(symbols: (char, char, char), widths: &[usize]) {
    let mut outline_str = format!("{}", symbols.0);
    let length = widths.len();

    for (i, width) in widths.iter().enumerate() {
        outline_str.push_str(&"─".repeat(width + 2));
        if i < length - 1 {
            outline_str.push(symbols.1);
        } else {
            outline_str.push(symbols.2);
        }
    }
    println!("{outline_str}");
}

fn color(str: String) -> String {
    let col_titles = ["Title/Class", "Command", "Options", "Group", "Scratchpads"];
    let mut colored_str = str
        .replace(";", "?")
        .replace("[", "[\x1b[0;36m")
        .replace("]", "\x1b[0;0m]")
        .replace("?", "\x1b[0;0m;\x1b[0;36m");

    if str.contains(".conf") {
        colored_str = colored_str.replace(&str, &format!("\x1b[0;35m{str}\x1b[0;0m"));
    }

    for title in col_titles {
        colored_str = colored_str.replace(title, &format!("\x1b[0;33m{title}\x1b[0;0m"));
    }
    colored_str
}

fn fancify(x: usize, str: &str) -> String {
    let str = if str.len() <= x {
        str.to_string() + &" ".repeat(max(x - str.chars().count(), 0))
    } else {
        str[..x - 3].to_string() + "..."
    };
    color(str)
}

fn print_table_row(data: &Vec<&str>, widths: &[usize]) {
    if data.len() != widths.len() {
        return;
    }

    let mut row_str = "│".to_string();
    for (width, field) in widths.iter().zip(data) {
        row_str.push(' ');
        row_str.push_str(&fancify(*width, field));
        row_str.push(' ');
        row_str.push('│');
    }
    println!("{row_str}");
}

fn max_len(xs: &Vec<&str>, min: usize, max: usize) -> usize {
    xs.iter()
        .map(|x| x.chars().count())
        .max()
        .unwrap_or_default()
        .max(min)
        .min(max)
}

fn get_config_data(socket: Option<&str>) -> Result<String> {
    let mut stream = UnixStream::connect(socket.unwrap_or(DEFAULT_SOCKET))?;
    stream.write_all("get-config?".as_bytes())?;
    stream.shutdown(Shutdown::Write)?;

    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;
    Ok(buf)
}

fn print_group_table(group_data: &[Vec<&str>]) {
    let [names, scratchpadss] = &group_data[0..2] else {
        return;
    };

    let max_chars = 80;
    let field_widths = vec![
        max_len(names, 5, max_chars),
        max_len(scratchpadss, 11, max_chars),
    ];

    print_table_outline(('┌', '┬', '┐'), &field_widths);
    print_table_row(&vec!["Group", "Scratchpads"], &field_widths);

    print_table_outline(('├', '┼', '┤'), &field_widths);
    for (name, scratchpads) in names.iter().zip(scratchpadss) {
        print_table_row(&vec![name, &scratchpads], &field_widths);
    }
    print_table_outline(('└', '┴', '┘'), &field_widths);
}

fn get_centered_conf(conf: &str, width: usize) -> String {
    let c = if width <= conf.len() {
        conf.split("/").last().unwrap_log(file!(), line!())
    } else {
        conf
    };

    let center_fix = if width % 2 == 0 {
        c.len() + c.len() % 2
    } else {
        c.len() - 1
    };

    format!(
        "{}{}{}",
        " ".repeat(max(width / 2 - c.len() / 2, 0)),
        c,
        " ".repeat(max((width - center_fix) / 2, 0))
    )
}

fn print_scratchpad_table(scratchpad_data: &[Vec<&str>], conf: &str) {
    let [titles, commands, options] = &scratchpad_data[0..3] else {
        return;
    };

    let max_chars = 80;
    let field_widths = vec![
        max_len(titles, 11, max_chars),
        max_len(commands, 7, max_chars),
        max_len(options, 7, max_chars),
    ];
    let config_str = get_centered_conf(conf, field_widths.iter().sum::<usize>() + 6);

    print_table_outline(('┌', '─', '┐'), &[config_str.len()]);
    print_table_row(&vec![&config_str], &[config_str.len()]);

    print_table_outline(('├', '┬', '┤'), &field_widths);
    print_table_row(&vec!["Title/Class", "Command", "Options"], &field_widths);

    print_table_outline(('├', '┼', '┤'), &field_widths);
    for ((title, command), option) in titles.iter().zip(commands).zip(options) {
        print_table_row(&vec![title, command, option], &field_widths);
    }

    print_table_outline(('└', '┴', '┘'), &field_widths);
}

fn print_raw_data(data: &Vec<Vec<&str>>) {
    for row in (0..data[0].len())
        .map(|i| data.iter().map(|inner| inner[i]).collect::<Vec<&str>>())
        .collect::<Vec<Vec<&str>>>()
    {
        println!("{}", row.join("    "));
    }
}

fn print_raw((conf, scratchpad_data, group_data): ParsedConfig) {
    println!("{conf}\n");
    println!("## SCRATCHPADS ##\n");
    print_raw_data(&scratchpad_data);

    if !group_data.is_empty() {
        println!("\n## GROUPS ##\n");
        print_raw_data(&group_data);
    }
}

fn print_tables((conf, scratchpad_data, group_data): ParsedConfig) {
    print_scratchpad_table(&scratchpad_data, conf);
    if !group_data.is_empty() {
        print_group_table(&group_data);
    }
}

fn parse_data(data: &str, field_num: usize) -> Vec<Vec<&str>> {
    let parsed_data = &data
        .splitn(field_num, '\u{2C01}')
        .map(|x| x.split('\u{2C02}').collect::<Vec<_>>())
        .collect::<Vec<_>>()[0..field_num];

    if parsed_data.len() < field_num {
        let _ = log("Config data could not be parsed".into(), Error);
        return vec![];
    }

    parsed_data.to_vec()
}

fn parse_config_data(data: &str) -> ParsedConfig {
    match data.splitn(3, '\u{2C00}').collect::<Vec<_>>()[..] {
        [c, scd, gd] => (c, parse_data(scd, 3), parse_data(gd, 2)),
        [c, scd] => (c, parse_data(scd, 3), vec![]),
        _ => {
            let _ = log("Could not get configuration data".into(), Error);
            ("", vec![], vec![])
        }
    }
}

pub fn get_config(socket: Option<&str>, raw: bool) -> Result<()> {
    let data = get_config_data(socket)?;
    let parsed_data = parse_config_data(&data);

    if raw {
        print_raw(parsed_data);
    } else {
        print_tables(parsed_data);
    }

    Ok(())
}

fn get_log_data() -> Result<String> {
    let mut file = File::open(DEFAULT_LOGFILE)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    Ok(buf)
}

pub fn print_logs(raw: bool) -> Result<()> {
    if let Ok(data) = get_log_data() {
        if raw {
            println!("{}", data.trim());
            return Ok(());
        }

        let log_str = data
            .replace("ERROR", "\x1b[0;31mERROR\x1b[0;0m")
            .replace("DEBUG", "\x1b[0;32mDEBUG\x1b[0;0m")
            .replace("WARN", "\x1b[0;33mWARN\x1b[0;0m")
            .replace("INFO", "\x1b[0;36mINFO\x1b[0;0m");
        println!("{}", log_str.trim());
    } else {
        println!("Logs are empty");
    }
    Ok(())
}

pub fn print_full_raw(socket: Option<&str>) -> Result<()> {
    println!("Hyprscratch v{}", env!("CARGO_PKG_VERSION"));
    println!("### LOGS ###\n");
    print_logs(true)?;
    println!("\n### CONFIGURATION ###");
    get_config(socket, true)?;
    Ok(())
}

pub fn print_help() {
    println!(
        "Usage:
  Daemon:
    hypscratch init [options...]
  Scratchpads:
    hyprscratch title command [options...]

DAEMON OPTIONS
  clean                       Hide scratchpads on workspace change
  spotless                    Hide scratchpads on focus change
  eager                       Spawn scratchpads hidden on start
  no-auto-reload              Don't reload the configuration when the configuration file is updated
  config </path/to/config>    Specify a path to the configuration file

SCRATCHPAD OPTIONS
  ephemeral                   Close the scratchpad when it is hidden
  persist                     Prevent the scratchpad from being replaced when a new one is summoned
  cover                       Prevent the scratchpad from replacing another one if one is already present
  sticky                      Prevent the scratchpad from being hidden by 'clean'
  shiny                       Prevent the scratchpad from being hidden by 'spotless'
  lazy                        Prevent the scratchpad from being spawned by 'eager'
  show                        Only creates or brings up the scratchpad
  hide                        Only hides the scratchpad
  poly                        Toggle all scratchpads matching the title simultaneously
  pin                         Keep the scratchpad active through workspace changes
  tiled                       Makes a tiled scratchpad instead of a floating one
  special                     Use Hyprland's special workspace, ignores most other options
  monitor <id|name>           Restrict the scratchpad to a specified monitor

EXTRA COMMANDS
  cycle [normal|special]      Cycle between [only normal | only special] scratchpads
  toggle <name>               Toggles the scratchpad with the given name
  show <name>                 Shows the scratchpad with the given name
  hide <name>                 Hides the scratchpad with the given name
  previous                    Summon the previous non-active scratchpad
  hide-all                    Hide all scratchpads
  kill-all                    Close all scratchpads
  reload                      Reparse config file
  get-config                  Print parsed config file
  kill                        Kill the hyprscratch daemon
  logs                        Print log file contents
  version                     Print current version
  help                        Print this help message

FLAG ALIASES
  -c, --config
  -r, --reload
  -g, --get-config
  -k, --kill
  -l, --logs
  -v, --version
  -h, --help")
}
