use hyprland::data::{Client, Clients, Workspace, Workspaces};
use hyprland::dispatch::*;
use hyprland::event_listener::EventListenerMutable;
use hyprland::prelude::*;
use hyprland::Result;
use regex::Regex;
use std::fs::remove_file;
use std::io::prelude::*;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

fn scratchpad(title: &str, cmd: &str) -> Result<()> {
    let clients_with_title = &Clients::get()?
        .filter(|x| x.initial_title == title)
        .collect::<Vec<_>>();

    if clients_with_title.is_empty() {
        hyprland::dispatch!(Exec, cmd)?;
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
        Dispatch::call(hyprland::dispatch::DispatchType::BringActiveToTop)?;
    }

    Ok(())
}

fn dequote(string: &String) -> String {
    let dequoted = match &string[0..1] {
        "\"" => &string[1..string.len() - 1],
        _ => &string,
    };
    dequoted.to_string()
}

fn parse_config() -> [Vec<String>; 3] {
    let hyprscrath_regex = Regex::new("hyprscratch.+").unwrap();
    let line_regex = Regex::new("\".+?\"|\\w+").unwrap();
    static mut BUF: String = String::new();

    let mut titles: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let mut options: Vec<String> = Vec::new();

    //It is unsafe because I need a mutable reference to a static variable
    unsafe {
        std::fs::File::open(format!(
            "{}/.config/hypr/hyprland.conf",
            std::env::var("HOME").unwrap()
        ))
        .unwrap()
        .read_to_string(&mut BUF)
        .unwrap();

        let lines: Vec<&str> = hyprscrath_regex
            .find_iter(&BUF)
            .map(|x| x.as_str())
            .collect();

        for line in lines {
            let parsed_line = &line_regex
                .find_iter(&line)
                .map(|x| x.as_str().to_string())
                .collect::<Vec<_>>()[..];

            if parsed_line.len() == 1 {
                continue;
            }

            match parsed_line[1].as_str() {
                "clean" | "hideall" => (),
                _ => {
                    titles.push(dequote(&parsed_line[1]));
                    commands.push(dequote(&parsed_line[2]));

                    if parsed_line.len() > 3 {
                        options.push(parsed_line[3..].join(""));
                    } else {
                        options.push(String::from(""));
                    }
                }
            };
        }
        [titles, commands, options]
    }
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

fn clean(cli_options: &[String], titles: &Vec<String>, options: &Vec<String>) -> Result<()> {
    let mut ev = EventListenerMutable::new();

    let titles_clone = titles.clone();
    let unshiny_titles: Vec<String> = titles
        .clone()
        .into_iter()
        .enumerate()
        .filter(|&(i, _)| !options[i].contains("shiny"))
        .map(|(_, x)| x)
        .collect();

    ev.add_workspace_change_handler(move |_, _| {
        move_floating(titles_clone.clone());
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

    std::thread::spawn(|| ev.start_listener());
    Ok(())
}

fn autospawn(titles: &Vec<String>, commands: &Vec<String>, options: &Vec<String>) {
    commands
        .iter()
        .enumerate()
        .filter(|&(i, _)| {
            options[i].contains("onstart")
                && !Clients::get()
                    .unwrap()
                    .map(|x| x.initial_title)
                    .collect::<Vec<_>>()
                    .contains(&titles[i])
        })
        .for_each(|(_, x)| {
            hyprland::dispatch!(Exec, &x.replacen("[", "[workspace 42 silent;", 1)).unwrap()
        });
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

fn initialize(title: &String, cmd: &[String]) -> Result<()> {
    let mut cli_args = cmd.join(" ");
    cli_args.push_str(title);

    let [titles, commands, options] = parse_config();
    autospawn(&titles, &commands, &options);

    if cli_args.contains("clean") {
        clean(cmd, &titles, &options)?;
    }

    let path_to_sock = Path::new("/tmp/hyprscratch.sock");
    if path_to_sock.exists() {
        remove_file(path_to_sock)?;
    }

    let listener = UnixListener::bind(path_to_sock)?;
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let titles_string = format!("{titles:?}");
                stream.write(titles_string.as_bytes())?;
            }
            Err(_) => {
                break;
            }
        };
    }

    Ok(())
}

fn main() -> Result<()> {
    let args = &std::env::args().collect::<Vec<String>>()[..];
    let title = match args.len() {
        0 | 1 => String::from(""),
        2.. => args[1].clone(),
    };

    let empty_sting_array = [String::new()];

    let cli_options = match args.len() {
        0..=2 => &empty_sting_array,
        3.. => &args[2..],
    };
    match title.as_str() {
        "clean" | "" => initialize(&title, cli_options)?,
        "hideall" => hideall()?,
        _ => {
            let cl = Client::get_active()?;
            let mut stream = UnixStream::connect("/tmp/hyprscratch.sock")?;
            let mut titles = String::new();
            stream.read_to_string(&mut titles)?;

            match cl {
                Some(cl) => {
                    if (!cli_options.contains(&"stack".to_string())
                        && (cl.floating && titles.contains(&title)))
                        || cl.initial_title == title
                    {
                        hyprland::dispatch!(
                            MoveToWorkspaceSilent,
                            WorkspaceIdentifierWithSpecial::Id(42),
                            None
                        )?;
                    }

                    if cl.initial_title != title {
                        scratchpad(&title, &cli_options[0])?;
                    }
                }
                None => scratchpad(&title, &cli_options[0])?
            }
        }
    }

    Ok(())
}
