use hyprland::Result;
use std::io::prelude::*;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;

use crate::logs::log;

pub fn get_config(socket: Option<&str>) -> Result<()> {
    let mut stream = UnixStream::connect(socket.unwrap_or("/tmp/hyprscratch/hyprscratch.sock"))?;
    stream.write_all("get-config?".as_bytes())?;
    stream.shutdown(Shutdown::Write)?;

    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;

    let Some((conf, data)) = buf.split_once('#') else {
        log("Could not get configuration data".into(), "ERROR")?;
        return Ok(());
    };

    let [titles, commands, options]: [Vec<&str>; 3] = data
        .splitn(3, '?')
        .map(|x| x.split('^').collect::<Vec<_>>())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    let max_len = |xs: &Vec<&str>, min: usize, max: usize| {
        xs.iter()
            .map(|x| x.chars().count())
            .max()
            .unwrap_or(0)
            .max(min)
            .min(max)
    };

    let color_pad = |x: usize, y: &str| {
        y.to_string()
            .replace(";", "?")
            .replace("[", "[\x1b[0;36m")
            .replace("]", "\x1b[0;0m]")
            .replace("?", "\x1b[0;0m;\x1b[0;36m")
            + &" ".repeat(x - y.chars().count())
    };

    let max_chars = 100;
    let max_titles = max_len(&titles, 6, max_chars);
    let max_commands = max_len(&commands, 8, max_chars);
    let max_options = max_len(&options, 7, max_chars);

    let print_border = |sep_l: &str, sep_c: &str, sep_r: &str| {
        println!(
            "{}{}{}{}",
            sep_l,
            "─".repeat(max_titles + 2) + sep_c,
            "─".repeat(max_commands + 2) + sep_c,
            "─".repeat(max_options + 2) + sep_r,
        );
    };

    let truncate = |str: &str| -> String {
        if str.len() < max_chars {
            str.into()
        } else {
            str[..max_chars - 3].to_string() + "..."
        }
    };

    let table_width = max_titles + max_commands + max_options + 6;
    let center_conf = format!(
        "{}\x1b[0;35m{}\x1b[0;0m{}",
        " ".repeat(table_width / 2 - conf.len() / 2),
        conf,
        " ".repeat(table_width / 2 - (conf.len() + 1) / 2)
    );

    print_border("┌", "─", "┐");
    println!("│ {} │", center_conf);

    print_border("├", "┬", "┤");
    println!(
        "│ \x1b[0;33m{}\x1b[0;0m │ \x1b[0;33m{}\x1b[0;0m │ \x1b[0;33m{}\x1b[0;0m │",
        color_pad(max_titles, "Titles"),
        color_pad(max_commands, "Commands"),
        color_pad(max_options, "Options")
    );

    print_border("├", "┼", "┤");
    for i in 0..titles.len() {
        println!(
            "│ {} │ {} │ {} │",
            color_pad(max_titles, titles[i]),
            color_pad(max_commands, &truncate(commands[i])),
            color_pad(max_options, options[i])
        )
    }

    print_border("└", "┴", "┘");
    Ok(())
}

pub fn print_logs() -> Result<()> {
    let path = Path::new("/tmp/hyprscratch/hyprscratch.log");
    if path.exists() {
        let mut file = std::fs::File::open(path)?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;

        let log_str = buf
            .replacen("[", "[\x1b[0;34m", 1)
            .replace("\n[", "\n[\x1b[0;34m")
            .replace("] [", "\x1b[0;0m] [")
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
  monitor <id>                Restrict the scratchpad to a specified monitor

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
