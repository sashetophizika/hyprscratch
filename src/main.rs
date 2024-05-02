use hyprland::data::{Client, Clients, Workspace};
use hyprland::dispatch::*;
use hyprland::event_listener::EventListenerMutable;
use hyprland::prelude::*;
use hyprland::Result;
use regex::Regex;
use std::fs::remove_file;
use std::io::prelude::*;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

fn summon_special(args: &[String]) -> Result<()> {
    let title = args[0].clone();
    let special_with_title = &Clients::get()?
        .filter(|x| x.initial_title == title && x.workspace.id < 0)
        .collect::<Vec<_>>();

    if special_with_title.is_empty() {
        let cmd = args[1].replacen("[", &format!("[workspace special:{title}; "), 1);
        hyprland::dispatch!(Exec, &cmd)?;
    } else {
        hyprland::dispatch!(ToggleSpecialWorkspace, Some(title))?;
    }
    Ok(())
}

fn summon_normal(args: &[String]) -> Result<()> {
    let clients_with_title = &Clients::get()?
        .filter(|x| x.initial_title == args[0])
        .collect::<Vec<_>>();

    if clients_with_title.is_empty() {
        hyprland::dispatch!(Exec, &args[1])?;
    } else {
        let pid = clients_with_title[0].pid as u32;
        if clients_with_title[0].workspace.id == Workspace::get_active()?.id {
            hyprland::dispatch!(FocusWindow, WindowIdentifier::ProcessId(pid))?;
        } else {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Relative(0),
                Some(WindowIdentifier::ProcessId(pid))
            )?;
            hyprland::dispatch!(FocusWindow, WindowIdentifier::ProcessId(pid))?;
        }
    }
    Ok(())
}

fn scratchpad(args: &[String]) -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch.sock")?;
    stream.write_all(b"\0")?;

    let mut titles = String::new();
    stream.read_to_string(&mut titles)?;
    if args[2..].contains(&"special".to_string()) {
        summon_special(&args)?;
        return Ok(());
    }

    let active_client = Client::get_active()?;
    match active_client {
        Some(active_client) => {
            let mut clients_with_title = Clients::get()?
                .filter(|x| {
                    x.initial_title == args[0]
                        && x.workspace.id == Workspace::get_active().unwrap().id
                })
                .peekable();

            if active_client.initial_title == args[0]
                || (!active_client.floating && clients_with_title.peek().is_some())
            {
                clients_with_title.for_each(|x| {
                    hyprland::dispatch!(
                        MoveToWorkspaceSilent,
                        WorkspaceIdentifierWithSpecial::Id(42),
                        Some(WindowIdentifier::ProcessId(x.pid as u32))
                    )
                    .unwrap()
                });
            } else {
                summon_normal(&args)?;

                if !args[2..].contains(&"stack".to_string())
                    && active_client.floating
                    && titles.contains(&active_client.initial_title)
                {
                    hyprland::dispatch!(
                        MoveToWorkspaceSilent,
                        WorkspaceIdentifierWithSpecial::Id(42),
                        Some(WindowIdentifier::ProcessId(active_client.pid as u32))
                    )?;
                }
            }
        }
        None => summon_normal(&args)?,
    }

    Dispatch::call(DispatchType::BringActiveToTop)?;
    Ok(())
}

fn cycle() -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch.sock")?;
    stream.write_all(b"c")?;

    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;
    stream.flush()?;

    let args: Vec<String> = buf.split(':').map(|x| x.to_owned()).collect();
    scratchpad(&args)?;
    Ok(())
}

fn reload() -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch.sock")?;
    stream.write_all(b"r")?;
    Ok(())
}

fn hideall() -> Result<()> {
    Clients::get()?
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

fn parse_config() -> [Vec<String>; 3] {
    let hyprscratch_lines_regex = Regex::new("hyprscratch.+").unwrap();
    let hyprscratch_args_regex = Regex::new("\".+?\"|'.+?'|[\\w.-]+").unwrap();
    let mut buf: String = String::new();

    let mut titles: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let mut options: Vec<String> = Vec::new();

    std::fs::File::open(format!(
        "{}/.config/hypr/hyprland.conf",
        std::env::var("HOME").unwrap()
    ))
    .unwrap()
    .read_to_string(&mut buf)
    .unwrap();

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
    [titles, commands, options]
}

fn handle_stream(
    stream: &mut UnixStream,
    current_titles: &mut Vec<String>,
    current_normal_titles: &mut Vec<String>,
    current_commands: &mut Vec<String>,
    current_options: &mut Vec<String>,
    cycle_current: &mut usize,
    args: &[String],
) -> Result<()> {
    let mut buf = String::new();
    stream.try_clone()?.take(1).read_to_string(&mut buf)?;

    match buf.as_str() {
        "r" => {
            let [titles, commands, options] = parse_config();
            *current_titles = titles.clone();
            *current_commands = commands.clone();
            *current_options = options.clone();
            *current_normal_titles = current_titles
                .iter()
                .enumerate()
                .filter(|&(i, _)| !options[i].contains("special"))
                .map(|(_, x)| x.to_owned())
                .collect::<Vec<String>>();

            autospawn(&titles, &commands, &options);
            clean(args, &titles, &options)?;
        }
        "\0" => {
            let titles_string = format!("{current_normal_titles:?}",);
            stream.write_all(titles_string.as_bytes())?;
        }
        "c" => {
            let mut current_index = *cycle_current % current_titles.len();
            while current_options[current_index].contains("special") {
                *cycle_current += 1;
                current_index = *cycle_current % current_titles.len();
            }

            let next_scratchpad = format!(
                "{}:{}:{}",
                current_titles[current_index],
                current_commands[current_index],
                current_options[current_index]
            );
            stream.write_all(next_scratchpad.as_bytes())?;
            *cycle_current += 1;
        }

        e => println!("Unknown request {e}"),
    }
    Ok(())
}

fn move_floating(titles: Vec<String>) {
    if let Ok(clients) = Clients::get() {
        clients
            .filter(|x| x.floating && x.workspace.id != 42 && titles.contains(&x.initial_title))
            .for_each(|x| {
                hyprland::dispatch!(
                    MoveToWorkspaceSilent,
                    WorkspaceIdentifierWithSpecial::Id(42),
                    Some(WindowIdentifier::ProcessId(x.pid as u32))
                )
                .unwrap()
            })
    }
}

fn clean(cli_options: &[String], titles: &[String], options: &[String]) -> Result<()> {
    let mut ev = EventListenerMutable::new();

    let shiny_titles: Vec<String> = titles
        .iter()
        .cloned()
        .enumerate()
        .filter(|&(i, _)| !options[i].contains("special"))
        .map(|(_, x)| x)
        .collect();
    let unshiny_titles: Vec<String> = titles
        .iter()
        .cloned()
        .enumerate()
        .filter(|&(i, _)| !(options[i].contains("shiny") || options[i].contains("special")))
        .map(|(_, x)| x)
        .collect();

    ev.add_workspace_change_handler(move |_, _| {
        move_floating(shiny_titles.clone());
    });

    if cli_options.contains(&"spotless".to_string()) {
        ev.add_active_window_change_handler(move |_, _| {
            if let Some(cl) = Client::get_active().unwrap() {
                if !cl.floating {
                    move_floating(unshiny_titles.clone());
                }
            }
        });
    }
    std::thread::spawn(|| ev.start_listener().unwrap());
    Ok(())
}

fn autospawn(titles: &[String], commands: &[String], options: &[String]) {
    let client_titles = Clients::get()
        .unwrap()
        .map(|x| x.initial_title)
        .collect::<Vec<_>>();

    commands
        .iter()
        .enumerate()
        .filter(|&(i, _)| options[i].contains("onstart") && !client_titles.contains(&titles[i]))
        .for_each(|(i, x)| {
            if options[i].contains("special") {
                hyprland::dispatch!(
                    Exec,
                    &x.replacen('[', &format!("[workspace special:{} silent;", titles[i]), 1)
                )
                .unwrap()
            } else {
                hyprland::dispatch!(Exec, &x.replacen('[', "[workspace 42 silent;", 1)).unwrap()
            }
        });
}

fn initialize(args: &[String]) -> Result<()> {
    let [mut titles, mut commands, mut options] = parse_config();
    let mut normal_titles = titles
        .iter()
        .enumerate()
        .filter(|&(i, _)| !options[i].contains("special"))
        .map(|(_, x)| x.to_owned())
        .collect::<Vec<String>>();

    autospawn(&titles, &commands, &options);

    let mut cycle_current: usize = 0;

    if args.contains(&"clean".to_string()) {
        clean(&args[1..], &titles, &options)?;
    }

    let path_to_sock = Path::new("/tmp/hyprscratch.sock");
    if path_to_sock.exists() {
        remove_file(path_to_sock)?;
    }

    let listener = UnixListener::bind(path_to_sock)?;
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                handle_stream(
                    &mut stream,
                    &mut titles,
                    &mut normal_titles,
                    &mut commands,
                    &mut options,
                    &mut cycle_current,
                    &args[1..],
                )?;
            }
            Err(_) => {
                continue;
            }
        };
    }
    Ok(())
}

fn get_config() {
    let [titles, commands, options] = parse_config();
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
}

fn help() {
    println!("Usage:
  hyprscratch title command [options...]

EXTRA COMMANDS
  clean [spotless]    Start the deamon and hide scratchpads on workspace change and focus change with spotless
  cycle               Cycle between non-special scratchpads
  hideall             Hidall all scratchpads simultaneously
  reload              Reparse file without restarting daemon
  get-config          Print parsed config file

SCRATCHPAD OPTIONS
  stack               Prevent the scratchpad from hiding the one that is already present
  shiny               Prevent the scratchpad from being affected by 'clean spotless'
  onstart             Spawn the scratchpads at the start of a hyprland session
  special             Use hyprlands special workspace, ignores most other options")
}

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let title = match args.len() {
        0 | 1 => String::from(""),
        2.. => args[1].clone(),
    };

    match title.as_str() {
        "clean" | "" => initialize(&args)?,
        "get-config" => get_config(),
        "hideall" => hideall()?,
        "reload" => reload()?,
        "cycle" => cycle()?,
        "help" => help(),
        _ => {
            if args[2..].is_empty() {
                println!("Unknown command or not enough arguments given for scratchpad.\nTry 'hyprscratch help'.");
            } else {
                scratchpad(&args[1..])?;
            }
        }
    }
    Ok(())
}
