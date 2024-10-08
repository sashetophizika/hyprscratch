use crate::config::Config;
use crate::utils::*;
use hyprland::data::Client;
use hyprland::dispatch::*;
use hyprland::event_listener::EventListener;
use hyprland::prelude::*;
use hyprland::Result;
use std::fs::{create_dir, remove_file};
use std::io::prelude::*;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};

fn handle_scratchpad(stream: &mut UnixStream, config: &Config) -> Result<()> {
    let titles_string = format!("{:?}", config.normal_titles);
    stream.write_all(titles_string.as_bytes())?;
    Ok(())
}

fn handle_reload(config: &mut Config) -> Result<()> {
    config.reload()?;
    shuffle_normal_special(&config.normal_titles, &config.special_titles)?;
    autospawn(config)?;
    Ok(())
}

fn handle_cycle(
    stream: &mut UnixStream,
    cycle_index: &mut usize,
    config: &Config,
    mode: Option<bool>,
) -> Result<()> {
    let mut current_index = *cycle_index % config.titles.len();
    if let Some(m) = mode {
        while m != config.options[current_index].contains("special") {
            current_index = (current_index + 1) % config.titles.len();
        }
    }
    let next_scratchpad = format!(
        "{}:{}:{}",
        config.titles[current_index], config.commands[current_index], config.options[current_index]
    );
    stream.write_all(next_scratchpad.as_bytes())?;
    *cycle_index = current_index + 1;
    Ok(())
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

pub fn initialize(args: &[String]) -> Result<()> {
    let mut config = Config::new()?;

    autospawn(&mut config)?;

    let mut cycle_index: usize = 0;

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
                let mut buf = String::new();
                stream.try_clone()?.take(1).read_to_string(&mut buf)?;

                match buf.as_str() {
                    "s" => handle_scratchpad(&mut stream, &config)?,
                    "r" => handle_reload(&mut config)?,
                    "c" => handle_cycle(&mut stream, &mut cycle_index, &config, None)?,
                    "n" => handle_cycle(&mut stream, &mut cycle_index, &config, Some(false))?,
                    "l" => handle_cycle(&mut stream, &mut cycle_index, &config, Some(true))?,
                    e => println!("Unknown request {e}"),
                }
            }
            Err(_) => {
                continue;
            }
        };
    }
    Ok(())
}
