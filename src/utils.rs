use hyprland::data::{Clients, Workspace};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use regex::Regex;
use std::io::prelude::*;

pub fn hideall() -> Result<()> {
    Clients::get()?
        .iter()
        .filter(|x| x.floating && x.workspace.id == Workspace::get_active().unwrap().id)
        .for_each(|x| {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Id(42),
                Some(WindowIdentifier::ProcessId(x.pid as u32))
            )
            .unwrap()
        });
    Ok(())
}

fn dequote(string: &String) -> String {
    let dequoted = match &string[0..1] {
        "\"" | "'" => &string[1..string.len() - 1],
        _ => string,
    };
    dequoted.to_string()
}

pub fn parse_config() -> Result<[Vec<String>; 3]> {
    let hyprscratch_lines_regex = Regex::new("hyprscratch.+").unwrap();
    let hyprscratch_args_regex = Regex::new("\".+?\"|'.+?'|[\\w.-]+").unwrap();
    let mut buf: String = String::new();

    let mut titles: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let mut options: Vec<String> = Vec::new();

    std::fs::File::open(format!(
        "{}/.config/hypr/hyprland.conf",
        std::env::var("HOME").unwrap()
    ))?
    .read_to_string(&mut buf)?;

    let lines: Vec<&str> = hyprscratch_lines_regex
        .find_iter(&buf)
        .map(|x| x.as_str())
        .collect();

    for line in lines {
        let parsed_line = &hyprscratch_args_regex
            .find_iter(line)
            .map(|x| x.as_str().to_string())
            .collect::<Vec<_>>()[..];

        if parsed_line.len() == 1 {
            continue;
        }

        match parsed_line[1].as_str() {
            "clean" | "hideall" | "reload" | "cycle" => (),
            _ => {
                titles.push(dequote(&parsed_line[1]));
                commands.push(dequote(&parsed_line[2]));

                if parsed_line.len() > 3 {
                    options.push(parsed_line[3..].join(" "));
                } else {
                    options.push(String::from(""));
                }
            }
        };
    }

    Ok([titles, commands, options])
}

pub fn get_config() -> Result<()> {
    let [titles, commands, options] = parse_config()?;
    let max_len = |xs: &Vec<String>| xs.iter().map(|x| x.chars().count()).max().unwrap();
    let padding = |x: usize, y: &str| " ".repeat(x - y.chars().count());

    let max_titles = max_len(&titles);
    let max_commands = max_len(&commands);
    let max_options = max_len(&options);

    for i in 0..titles.len() {
        println!(
            "\x1b[0;34mTitle:\x1b[0;0m {}{}  \x1b[0;34mCommand:\x1b[0;1m {}{}  \x1b[0;34mOptions:\x1b[0;0m {}{}",
            titles[i],
            padding(max_titles, &titles[i]),
            commands[i],
            padding(max_commands, &commands[i]),
            options[i],
            padding(max_options, &options[i])
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
  special             Use Hyprland's special workspace, ignores most other options

EXTRA COMMANDS
  cycle               Cycle between non-special scratchpads
  hideall             Hidall all scratchpads simultaneously
  reload              Reparse file without restarting daemon
  get-config          Print parsed config file"
    )
}
