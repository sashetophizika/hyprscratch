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

pub fn hideall() -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(b"s")?;
    stream.shutdown(Shutdown::Write)?;

    let mut titles = String::new();
    stream.read_to_string(&mut titles)?;

    move_floating(titles.split(" ").map(|x| x.to_string()).collect())?;
    let active_client = Client::get_active()?.unwrap();
    if active_client.workspace.id < 0 {
        hyprland::dispatch!(ToggleSpecialWorkspace, Some(active_client.initial_title))?;
    }
    Ok(())
}

pub fn cycle(args: String) -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    if args.contains("special") {
        stream.write_all(b"c?1")?;
    } else if args.contains("normal") {
        stream.write_all(b"c?0")?;
    } else {
        stream.write_all(b"c")?;
    }
    stream.shutdown(Shutdown::Write)?;

    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;
    if buf == "empty" {
        return Ok(());
    }

    let args: Vec<String> = buf.split(':').map(|x| x.to_owned()).collect();
    scratchpad(&args[0], &args[1], &args[2])?;
    Ok(())
}

pub fn reload() -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(b"reload")?;
    stream.shutdown(Shutdown::Write)?;
    Ok(())
}

pub fn kill() -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(b"kill")?;
    stream.shutdown(Shutdown::Write)?;
    Ok(())
}

pub fn get_config(config_file: Option<String>) -> Result<()> {
    let conf = Config::new(config_file)?;
    let max_len = |xs: &Vec<String>| xs.iter().map(|x| x.chars().count()).max().unwrap();
    let padding = |x: usize, y: &str| " ".repeat(x - y.chars().count());

    let max_titles = max_len(&conf.titles);
    let max_commands = max_len(&conf.commands);
    let max_options = max_len(&conf.options);

    for i in 0..conf.titles.len() {
        println!(
            "\x1b[0;34mTitle:\x1b[0;0m {}{}  \x1b[0;34mCommand:\x1b[0;0m {}{}  \x1b[0;34mOptions:\x1b[0;0m {}{}",
            conf.titles[i],
            padding(max_titles, &conf.titles[i]),
            conf.commands[i],
            padding(max_commands, &conf.commands[i]),
            conf.options[i],
            padding(max_options, &conf.options[i])
        )
    }

    Ok(())
}

pub fn logs() -> Result<()> {
    let path = Path::new("/tmp/hyprscratch/hyprscratch.log");
    if path.exists() {
        let mut file = std::fs::File::open(path)?;
        let mut buf = String::new();

        file.read_to_string(&mut buf)?;
        let b = buf
            .replace("ERROR", "\x1b[0;31mERROR\x1b[0;0m")
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
  stack                       Prevent the scratchpad from hiding the one that is already present
  shiny                       Prevent the scratchpad from being affected by 'clean spotless'
  on-start                    Spawn the scratchpads at the start of a Hyprland session
  summon                      Only creates or brings up the scratchpad
  hide                        Only hides the scratchpad
  special                     Use Hyprland's special workspace, ignores most other options

EXTRA COMMANDS
  cycle [normal|special]      Cycle between [only normal | only special] scratchpads
  hide-all                    Hide all scratchpads
  reload                      Reparse config file
  get-config                  Print parsed config file
  kill                        Kill the hyprscratch daemon
  logs                        Print log file contents
  help                        Print this help message
  version                     Print current version"
    )
}
