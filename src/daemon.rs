use hyprland::data::{Client, Clients};
use hyprland::dispatch::*;
use hyprland::event_listener::EventListener;
use hyprland::prelude::*;
use hyprland::Result;
use std::fs::{create_dir, remove_file};
use std::io::prelude::*;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use crate::utils::parse_config;

struct Config {
    titles: Vec<String>,
    normal_titles: Vec<String>,
    commands: Vec<String>,
    options: Vec<String>,
    shiny_titles: Arc<Mutex<Vec<String>>>,
    unshiny_titles: Arc<Mutex<Vec<String>>>,
}

impl Config {
    fn new() -> Result<Config> {
        let [titles, commands, options] = parse_config()?;
        let normal_titles = titles
            .iter()
            .enumerate()
            .filter(|&(i, _)| !options[i].contains("special"))
            .map(|(_, x)| x.to_owned())
            .collect::<Vec<String>>();

        let shiny_titles: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(normal_titles.clone()));

        let unshiny_titles: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(
            titles
                .iter()
                .cloned()
                .enumerate()
                .filter(|&(i, _)| !(options[i].contains("shiny") || options[i].contains("special")))
                .map(|(_, x)| x)
                .collect(),
        ));

        Ok(Config {
            titles,
            normal_titles,
            commands,
            options,
            shiny_titles,
            unshiny_titles,
        })
    }
}

fn shuffle_normal_special(
    normal_titles: &[String],
    old_normal_titles: &[String],
    special_titles: &[String],
    old_special_titles: &[String],
) -> Result<()> {
    let clients = Clients::get()?;
    for title in old_normal_titles.iter() {
        if special_titles.contains(title) {
            clients.iter().filter(|x| &x.title == title).for_each(|x| {
                hyprland::dispatch!(
                    MoveToWorkspaceSilent,
                    WorkspaceIdentifierWithSpecial::Special(Some(title)),
                    Some(WindowIdentifier::ProcessId(x.pid as u32))
                )
                .unwrap()
            });
        }
    }

    for title in old_special_titles.iter() {
        if normal_titles.contains(title) {
            clients.iter().filter(|x| &x.title == title).for_each(|x| {
                hyprland::dispatch!(
                    MoveToWorkspaceSilent,
                    WorkspaceIdentifierWithSpecial::Id(42),
                    Some(WindowIdentifier::ProcessId(x.pid as u32))
                )
                .unwrap()
            });
        }
    }

    Ok(())
}

fn handle_reload(config: &mut Config) -> Result<()> {
    let [titles, commands, options] = parse_config()?;

    let normal_titles = titles
        .iter()
        .enumerate()
        .filter(|&(i, _)| !options[i].contains("special"))
        .map(|(_, x)| x.to_owned())
        .collect::<Vec<String>>();

    let old_normal_titles = config
        .titles
        .iter()
        .enumerate()
        .filter(|&(i, _)| !config.options[i].contains("special"))
        .map(|(_, x)| x.to_owned())
        .collect::<Vec<String>>();

    let special_titles = titles
        .iter()
        .enumerate()
        .filter(|&(i, _)| options[i].contains("special"))
        .map(|(_, x)| x.to_owned())
        .collect::<Vec<String>>();

    let old_special_titles = config
        .titles
        .iter()
        .enumerate()
        .filter(|&(i, _)| config.options[i].contains("special"))
        .map(|(_, x)| x.to_owned())
        .collect::<Vec<String>>();

    shuffle_normal_special(
        &normal_titles,
        &old_normal_titles,
        &special_titles,
        &old_special_titles,
    )?;

    autospawn(config)?;

    config.titles.clone_from(&titles);
    config.commands.clone_from(&commands);
    config.options.clone_from(&options);
    config.normal_titles.clone_from(&normal_titles);

    let mut current_shiny_titles = config.shiny_titles.lock().unwrap();
    current_shiny_titles.clone_from(&normal_titles);

    let mut current_unshiny_titles = config.unshiny_titles.lock().unwrap();
    *current_unshiny_titles = titles
        .iter()
        .cloned()
        .enumerate()
        .filter(|&(i, _)| !options[i].contains("shiny") && !options[i].contains("special"))
        .map(|(_, x)| x)
        .collect();

    Ok(())
}

fn handle_cycle(
    stream: &mut UnixStream,
    cycle_current: &mut usize,
    config: &mut Config,
) -> Result<()> {
    let mut current_index = *cycle_current % config.titles.len();
    while config.options[current_index].contains("special") {
        *cycle_current += 1;
        current_index = *cycle_current % config.titles.len();
    }

    let next_scratchpad = format!(
        "{}:{}:{}",
        config.titles[current_index], config.commands[current_index], config.options[current_index]
    );
    stream.write_all(next_scratchpad.as_bytes())?;
    *cycle_current += 1;
    Ok(())
}

fn handle_stream(
    stream: &mut UnixStream,
    cycle_current: &mut usize,
    config: &mut Config,
) -> Result<()> {
    let mut buf = String::new();
    stream.try_clone()?.take(1).read_to_string(&mut buf)?;

    match buf.as_str() {
        "\0" => {
            let titles_string = format!("{:?}", config.normal_titles);
            stream.write_all(titles_string.as_bytes())?;
        }
        "r" => handle_reload(config)?,
        "c" => handle_cycle(stream, cycle_current, config)?,
        e => println!("Unknown request {e}"),
    }
    Ok(())
}

fn move_floating(titles: Vec<String>) {
    if let Ok(clients) = Clients::get() {
        clients
            .iter()
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

fn clean(
    spotless: &str,
    shiny_titles: Arc<Mutex<Vec<String>>>,
    unshiny_titles: Arc<Mutex<Vec<String>>>,
) -> Result<()> {
    let mut ev = EventListener::new();

    ev.add_workspace_change_handler(move |_| {
        move_floating(shiny_titles.lock().unwrap().to_vec());
        if let Some(cl) = Client::get_active().unwrap() {
            if cl.workspace.id < 0 {
                hyprland::dispatch!(ToggleSpecialWorkspace, Some(cl.title)).unwrap();
            }
        }
    });

    if spotless == "spotless" {
        ev.add_active_window_change_handler(move |_| {
            if let Some(cl) = Client::get_active().unwrap() {
                if !cl.floating {
                    move_floating(unshiny_titles.lock().unwrap().to_vec());
                }
            }
        });
    }

    ev.start_listener()?;
    Ok(())
}

fn autospawn(config: &mut Config) -> Result<()> {
    let client_titles = Clients::get()?
        .into_iter()
        .map(|x| x.initial_title)
        .collect::<Vec<_>>();

    config
        .commands
        .iter()
        .enumerate()
        .filter(|&(i, _)| {
            config.options[i].contains("onstart") && !client_titles.contains(&config.titles[i])
        })
        .for_each(|(i, x)| {
            if config.options[i].contains("special") {
                hyprland::dispatch!(
                    Exec,
                    &x.replacen(
                        '[',
                        &format!("[workspace special:{} silent;", config.titles[i]),
                        1
                    )
                )
                .unwrap()
            } else {
                hyprland::dispatch!(Exec, &x.replacen('[', "[workspace 42 silent;", 1)).unwrap()
            }
        });

    Ok(())
}

pub fn initialize(args: &[String]) -> Result<()> {
    let mut config = Config::new()?;

    autospawn(&mut config)?;

    let mut cycle_current: usize = 0;

    if args.contains(&"clean".to_string()) {
        let shiny_titles = Arc::clone(&config.shiny_titles);
        let unshiny_titles = Arc::clone(&config.unshiny_titles);
        if args[1..].contains(&"spotless".to_string()) {
            std::thread::spawn(move || {
                clean("spotless", shiny_titles.clone(), unshiny_titles.clone())
            });
        } else {
            std::thread::spawn(move || clean(" ", shiny_titles.clone(), unshiny_titles.clone()));
        }
    }

    let temp_dir = Path::new("/tmp/hyprscratch/");
    if !temp_dir.exists() {
        create_dir(temp_dir)?;
    }

    let path_to_sock = Path::new("/tmp/hyprscratch/hyprscratch.sock");
    if path_to_sock.exists() {
        remove_file(path_to_sock)?;
    }

    let listener = UnixListener::bind(path_to_sock)?;
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                handle_stream(&mut stream, &mut cycle_current, &mut config)?;
            }
            Err(_) => {
                continue;
            }
        };
    }
    Ok(())
}
