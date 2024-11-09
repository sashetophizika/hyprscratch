use crate::config::Config;
use crate::scratchpad::scratchpad;
use crate::utils::move_floating;
use hyprland::data::Client;
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use std::io::prelude::*;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;

fn connect_to_sock(request: &str) -> Result<UnixStream> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(request.as_bytes())?;
    stream.shutdown(Shutdown::Write)?;
    Ok(stream)
}

fn pass_to_scratchpad(stream: &mut UnixStream) -> Result<()> {
    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;
    if buf == "empty" {
        return Ok(());
    }

    let args: Vec<String> = buf.split(':').map(|x| x.to_owned()).collect();
    scratchpad(&args[0], &args[1], &args[2])?;
    Ok(())
}

pub fn hide_all() -> Result<()> {
    let mut titles = String::new();
    let mut stream = connect_to_sock("s")?;
    stream.read_to_string(&mut titles)?;

    move_floating(titles.split(" ").map(|x| x.to_string()).collect())?;
    let active_client = Client::get_active()?.unwrap();
    if active_client.workspace.id <= 0 {
        hyprland::dispatch!(ToggleSpecialWorkspace, Some(active_client.initial_title))?;
    }
    Ok(())
}

pub fn cycle(args: String) -> Result<()> {
    let request = if args.contains("special") {
        "c?1"
    } else if args.contains("normal") {
        "c?0"
    } else {
        "c"
    };
    let mut stream = connect_to_sock(request)?;
    pass_to_scratchpad(&mut stream)
}

pub fn previous() -> Result<()> {
    let active_title = Client::get_active()?.unwrap().initial_title;
    let mut stream = connect_to_sock(format!("p?{active_title}").as_str())?;
    pass_to_scratchpad(&mut stream)
}

pub fn reload() -> Result<()> {
    connect_to_sock("reload")?;
    Ok(())
}

pub fn kill() -> Result<()> {
    connect_to_sock("kill")?;
    Ok(())
}

pub fn kill_all() -> Result<()> {
    connect_to_sock("killall")?;
    Ok(())
}

pub fn get_config(config_file: Option<String>) -> Result<()> {
    let conf = Config::new(config_file)?;
    let max_len = |xs: &Vec<String>, def: usize| {
        xs.iter()
            .map(|x| x.chars().count())
            .max()
            .unwrap_or(0)
            .max(def)
    };
    let color_pad = |x: usize, y: &str| {
        y.to_string()
            .replace("[", "[\x1b[0;34m")
            .replace("]", "\x1b[0;0m]")
            + &" ".repeat(x - y.chars().count())
    };

    let max_titles = max_len(&conf.titles, 6);
    let max_commands = max_len(&conf.commands, 8);
    let max_options = max_len(&conf.options, 7);

    let print_border = |sep_l: &str, sep_c: &str, sep_r: &str| {
        println!(
            "{}{}{}{}",
            sep_l,
            "─".repeat(max_titles + 2) + sep_c,
            "─".repeat(max_commands + 2) + sep_c,
            "─".repeat(max_options + 2) + sep_r,
        );
    };

    print_border("┌", "┬", "┐");
    println!(
        "│ \x1b[0;31m{}\x1b[0;0m │ \x1b[0;31m{}\x1b[0;0m │ \x1b[0;31m{}\x1b[0;0m │",
        color_pad(max_titles, "Titles"),
        color_pad(max_commands, "Commands"),
        color_pad(max_options, "Options")
    );

    print_border("├", "┼", "┤");
    for i in 0..conf.titles.len() {
        println!(
            "│ {} │ {} │ {} │",
            color_pad(max_titles, &conf.titles[i]),
            color_pad(max_commands, &conf.commands[i]),
            color_pad(max_options, &conf.options[i])
        )
    }

    print_border("└", "┴", "┘");
    Ok(())
}

pub fn logs() -> Result<()> {
    let path = Path::new("/tmp/hyprscratch/hyprscratch.log");
    if path.exists() {
        let mut file = std::fs::File::open(path)?;
        let mut buf = String::new();

        file.read_to_string(&mut buf)?;
        let b = buf
            .replacen("[", "[\x1b[0;34m", 1)
            .replace("\n[", "\n[\x1b[0;34m")
            .replace("] [", "\x1b[0;0m] [")
            .replace("ERROR", "\x1b[0;31mERROR\x1b[0;0m")
            .replace("DEBUG", "\x1b[0;32mDEBUG\x1b[0;0m")
            .replace("WARN", "\x1b[0;33mWARN\x1b[0;0m")
            .replace("INFO", "\x1b[0;36mINFO\x1b[0;0m");
        println!("{}", b.trim());
    } else {
        println!("Logs are empty");
    }
    Ok(())
}

pub fn help() {
    println!(
        "Usage:
  Daemon:
    hypscratch [options...]
  Scratchpads:
    hyprscratch title command [options...]

DAEMON OPTIONS
  clean [spotless]            Hide scratchpads on workspace change [and focus change]
  no-auto-reload              Don't reload the configuration when the configuration file is updated

SCRATCHPAD OPTIONS
  persist                     Prevent the scratchpad from beign replaced when a new one is summoned
  cover                       Prevent the scratchpad from replacing another one if one is already present
  sticky                      Prevent the scratchpad from being affected by 'clean'
  shiny                       Prevent the scratchpad from being affected by 'clean spotless'
  eager                       Spawn the scratchpads at the start of a Hyprland session
  summon                      Only creates or brings up the scratchpad
  hide                        Only hides the scratchpad
  poly                        Toggles all scratchpads matching the title
  special                     Use Hyprland's special workspace, ignores most other options

EXTRA COMMANDS
  cycle [normal|special]      Cycle between [only normal | only special] scratchpads
  previous                    Summon the previous non-active scratchpad
  hide-all                    Hide all scratchpads
  reload                      Reparse config file
  kill-all                    Close all scratchpads
  get-config                  Print parsed config file
  kill                        Kill the hyprscratch daemon
  logs                        Print log file contents
  version                     Print current version
  help                        Print this help message"
    )
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
                &["".to_string()],
                Some("test_configs/test_config3.txt".to_string()),
                None,
            )
        });
        sleep(Duration::from_millis(500));

        cycle("".to_string()).unwrap();
        cycle("special".to_string()).unwrap();
        cycle("normal".to_string()).unwrap();
        previous().unwrap();
        sleep(Duration::from_millis(1000));

        hide_all().unwrap();
        reload().unwrap();
        sleep(Duration::from_millis(1000));

        kill_all().unwrap();
        kill().unwrap();
        sleep(Duration::from_millis(1000));
    }
}
