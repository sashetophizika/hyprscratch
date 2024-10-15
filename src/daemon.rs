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

fn handle_scratchpad(stream: &mut UnixStream, request: String, config: &Config) -> Result<()> {
    if request.len() > 1 {
        let mut unshiny_titles = config.unshiny_titles.lock().unwrap();
        unshiny_titles.retain(|x| *x != request[2..]);
    }

    let titles_string = format!("{:?}", config.normal_titles);
    stream.write_all(titles_string.as_bytes())?;
    Ok(())
}

fn handle_return(title: String, config: &Config) -> Result<()> {
    let mut unshiny_titles = config.unshiny_titles.lock().unwrap();
    unshiny_titles.push(title[2..].to_string());
    Ok(())
}

fn handle_reload(config: &mut Config) -> Result<()> {
    config.reload(None)?;
    shuffle_normal_special(&config.normal_titles, &config.special_titles)?;
    autospawn(config)?;
    Ok(())
}

fn handle_cycle(
    stream: &mut UnixStream,
    cycle_index: &mut usize,
    config: &Config,
    request: String,
) -> Result<()> {
    if config.titles.is_empty() {
        return Ok(());
    }

    let mut current_index = *cycle_index % config.titles.len();
    let mode = if request.len() == 1 {
        None
    } else {
        Some((request.as_bytes()[2] - 48) != 0)
    };

    if let Some(m) = mode {
        if (m && config.special_titles.is_empty()) || (!m && config.normal_titles.is_empty()) {
            return Ok(());
        }

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

pub fn initialize(
    args: &[String],
    config_path: Option<String>,
    socket_path: Option<&str>,
) -> Result<()> {
    let mut config = Config::new(config_path)?;
    let mut cycle_index: usize = 0;
    autospawn(&mut config)?;

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

    let path_to_sock = match socket_path {
        Some(sp) => Path::new(sp),
        None => {
            let temp_dir = Path::new("/tmp/hyprscratch/");
            if !temp_dir.exists() {
                create_dir(temp_dir)?;
            }
            Path::new("/tmp/hyprscratch/hyprscratch.sock")
        }
    };

    if path_to_sock.exists() {
        remove_file(path_to_sock)?;
    }

    let listener = UnixListener::bind(path_to_sock)?;
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let mut buf = String::new();
                stream.try_clone()?.read_to_string(&mut buf)?;

                match buf.as_str() {
                    "reload" => handle_reload(&mut config)?,
                    b if b.starts_with("c") => {
                        handle_cycle(&mut stream, &mut cycle_index, &config, buf)?
                    }
                    b if b.starts_with("s") => handle_scratchpad(&mut stream, buf, &config)?,
                    b if b.starts_with("r") => handle_return(buf, &config)?,
                    e => {
                        let error_message = format!("Unknown request: {e}");
                        stream.write_all(error_message.as_bytes()).unwrap();
                        println!("{error_message}");
                    }
                }
            }
            Err(_) => {
                continue;
            }
        };
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    fn test_handle(request: &str, expectation: &str) {
        let mut stream = UnixStream::connect("/tmp/hyprscratch_test.sock").unwrap();

        stream.write_all(request.as_bytes()).unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();

        let mut buf = String::new();
        stream.read_to_string(&mut buf).unwrap();
        assert_eq!(expectation, buf);
    }

    #[test]
    fn test_handlers() {
        std::thread::spawn(|| {
            let args = vec!["".to_string()];
            initialize(
                &args,
                Some("./test_config2.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            )
            .unwrap();
        });
        std::thread::sleep(std::time::Duration::from_millis(100));

        test_handle("c", "firefox:firefox --private-window:special");
        test_handle("c?0", "ytop:kitty --title btop -e ytop:");
        test_handle("c?1", "cmatrix:kitty --title cmatrix -e cmatrix:special");

        test_handle("s?btop", "[\"ytop\", \"htop\"]");
        test_handle("r?btop", "");

        test_handle("reload", "");
        test_handle("unknown", "Unknown request: unknown");
    }
}
