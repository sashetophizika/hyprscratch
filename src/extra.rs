use crate::logs::log;
use crate::utils::move_floating;
use hyprland::data::Client;
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use std::io::prelude::*;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;

fn connect_to_sock(socket: Option<&str>, request: &str) -> Result<UnixStream> {
    let mut stream = UnixStream::connect(socket.unwrap_or("/tmp/hyprscratch/hyprscratch.sock"))?;
    stream.write_all(request.as_bytes())?;
    stream.shutdown(Shutdown::Write)?;
    Ok(stream)
}

pub fn hide_all(socket: Option<&str>) -> Result<()> {
    let mut titles = String::new();
    let mut stream = connect_to_sock(socket, "scratchpad?")?;
    stream.read_to_string(&mut titles)?;

    move_floating(titles.split(" ").map(|x| x.to_string()).collect())?;
    let active_client = Client::get_active()?.unwrap();
    if active_client.workspace.id <= 0 {
        hyprland::dispatch!(ToggleSpecialWorkspace, Some(active_client.initial_title))?;
    }
    Ok(())
}

pub fn cycle(socket: Option<&str>, args: String) -> Result<()> {
    let request = if args.contains("special") {
        "cycle?1"
    } else if args.contains("normal") {
        "cycle?0"
    } else {
        "cycle?"
    };

    connect_to_sock(socket, request)?;
    Ok(())
}

pub fn call(socket: Option<&str>, args: &[String], mode: &str) -> Result<()> {
    if args.len() <= 1 {
        log(format!("No scratchpad title given to '{mode}'"), "WARN")?
    }

    let title = args[1].clone();
    connect_to_sock(socket, format!("{mode}?{title}").as_str())?;
    Ok(())
}

pub fn previous(socket: Option<&str>) -> Result<()> {
    let active_title = Client::get_active()?.unwrap().initial_title;
    connect_to_sock(socket, format!("previous?{active_title}").as_str())?;
    Ok(())
}

pub fn reload(socket: Option<&str>, config_file: Option<String>) -> Result<()> {
    connect_to_sock(
        socket,
        &format!("reload?{}", config_file.unwrap_or("".to_string())),
    )?;
    Ok(())
}

pub fn kill(socket: Option<&str>) -> Result<()> {
    connect_to_sock(socket, "kill?")?;
    Ok(())
}

pub fn kill_all(socket: Option<&str>) -> Result<()> {
    connect_to_sock(socket, "killall?")?;
    Ok(())
}

pub fn get_config(socket: Option<&str>) -> Result<()> {
    let mut socket = connect_to_sock(socket, "get-config?")?;
    let mut buf = String::new();
    socket.read_to_string(&mut buf)?;

    let [titles, commands, options]: [Vec<&str>; 3] = buf
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
            .replace("[", "[\x1b[0;34m")
            .replace("]", "\x1b[0;0m]")
            .replace("?", "\x1b[0;0m;\x1b[0;34m")
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

    print_border("┌", "┬", "┐");
    println!(
        "│ \x1b[0;31m{}\x1b[0;0m │ \x1b[0;31m{}\x1b[0;0m │ \x1b[0;31m{}\x1b[0;0m │",
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
  config </path/to/config>      Specify a path to the configuration file             

SCRATCHPAD OPTIONS
  persist                     Prevent the scratchpad from being replaced when a new one is summoned
  cover                       Prevent the scratchpad from replacing another one if one is already present
  sticky                      Prevent the scratchpad from being hidden by 'clean'
  shiny                       Prevent the scratchpad from being hidden by 'spotless'
  lazy                        Prevent the scratchpad from being spawned by 'eager'
  summon                      Only creates or brings up the scratchpad
  hide                        Only hides the scratchpad
  poly                        Toggles all scratchpads matching the title simultaneously
  tiled                       Makes a tiled scratchpad instead of a floating one
  special                     Use Hyprland's special workspace, ignores most other options

EXTRA COMMANDS
  cycle [normal|special]      Cycle between [only normal | only special] scratchpads
  toggle <name>               Toggles the scratchpad with the given name
  summon <name>               Summons the scratchpad with the given name
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

#[cfg(test)]
mod tests {
    use std::{thread::sleep, time::Duration};

    use super::*;
    use crate::initialize_daemon;

    #[test]
    fn test_extra_commands() {
        std::thread::spawn(|| {
            initialize_daemon(
                "".to_string(),
                Some("test_configs/test_config3.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            )
        });
        sleep(Duration::from_millis(1000));

        let socket = Some("/tmp/hyprscratch_test.sock");
        cycle(socket, "".to_string()).unwrap();
        cycle(socket, "special".to_string()).unwrap();
        cycle(socket, "normal".to_string()).unwrap();
        previous(socket).unwrap();
        sleep(Duration::from_millis(1000));

        hide_all(socket).unwrap();
        reload(socket, None).unwrap();
        get_config(socket).unwrap();
        sleep(Duration::from_millis(1000));

        kill_all(socket).unwrap();
        kill(socket).unwrap();
        sleep(Duration::from_millis(1000));
    }
}
