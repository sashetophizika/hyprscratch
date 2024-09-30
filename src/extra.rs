use crate::config::Config;
use crate::scratchpad::scratchpad;
use hyprland::data::{Client, Clients, Workspace};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use std::io::prelude::*;
use std::os::unix::net::UnixStream;

pub fn hideall() -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(b"s")?;

    let mut titles = String::new();
    stream.read_to_string(&mut titles)?;
    let active_workspace = Workspace::get_active()?;

    Clients::get()?
        .iter()
        .filter(|x| {
            x.floating && x.workspace.id == active_workspace.id && titles.contains(&x.title)
        })
        .for_each(|x| {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Id(42),
                Some(WindowIdentifier::Address(x.address.clone()))
            )
            .unwrap()
        });

    let active_client = Client::get_active()?.unwrap();
    if active_client.workspace.id < 0 {
        hyprland::dispatch!(ToggleSpecialWorkspace, Some(active_client.initial_title))?;
    }
    Ok(())
}

pub fn cycle() -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(b"c")?;

    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;
    stream.flush()?;

    let args: Vec<String> = buf.split(':').map(|x| x.to_owned()).collect();
    scratchpad(&args[0], &args[1], &args[2])?;
    Ok(())
}

pub fn reload() -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(b"r")?;
    Ok(())
}

pub fn get_config() -> Result<()> {
    let conf = Config::new()?;
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

pub fn help() {
    println!(
        "Usage:
  Daemon:
    hypscratch [options...]
  Scratchpads:
    hyprscratch title command [options...]

DAEMON OPTIONS
  clean [spotless]    Hide scratchpads on workspace change and focus change with spotless

SCRATCHPAD OPTIONS
  stack               Prevent the scratchpad from hiding the one that is already present
  shiny               Prevent the scratchpad from being affected by 'clean spotless'
  onstart             Spawn the scratchpads at the start of a hyprland session
  summon              Only creates or brings up the scratchpad
  hide                Only hides the scratchpad
  special             Use Hyprland's special workspace, ignores most other options

EXTRA COMMANDS
  cycle               Cycle between scratchpads
  hideall             Hidall all scratchpads simultaneously
  reload              Reparse file without restarting daemon
  get-config          Print parsed config file
  help                Print this help message
  version             Print current version"
    )
}
