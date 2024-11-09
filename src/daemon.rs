use crate::config::Config;
use crate::logs::*;
use crate::utils::*;
use hyprland::data::Client;
use hyprland::data::Clients;
use hyprland::dispatch::*;
use hyprland::event_listener::EventListener;
use hyprland::prelude::*;
use hyprland::Result;
use std::fs::{create_dir, remove_file};
use std::io::prelude::*;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

fn handle_scratchpad(
    stream: &mut UnixStream,
    request: String,
    config: &mut Config,
    prev_titles: &mut [String; 2],
) -> Result<()> {
    if request.len() > 1 {
        config.dirty_titles.retain(|x| *x != request[2..]);
        if request[2..] != prev_titles[0] {
            prev_titles[1] = prev_titles[0].clone();
            prev_titles[0] = request[2..].to_string();
        }
    }

    let non_persist_titles = config
        .normal_titles
        .clone()
        .into_iter()
        .zip(config.options.clone())
        .filter(|(_, option)| !option.contains("persist"))
        .map(|(title, _)| title)
        .collect::<Vec<_>>()
        .join(" ");

    stream.write_all(non_persist_titles.as_bytes())?;
    Ok(())
}

fn handle_return(request: String, config: &mut Config) -> Result<()> {
    config.dirty_titles.push(request[2..].to_string());
    Ok(())
}

fn handle_reload(config: &mut Config, config_path: Option<String>) -> Result<()> {
    config.reload(config_path)?;
    autospawn(config)?;
    Ok(())
}

fn handle_killall(config: &Config) -> Result<()> {
    Clients::get()?
        .into_iter()
        .filter(|x| config.titles.contains(&x.initial_title))
        .for_each(|x| {
            hyprland::dispatch!(CloseWindow, WindowIdentifier::Address(x.address))
                .unwrap_log(file!(), line!())
        });
    Ok(())
}

fn handle_cycle(
    stream: &mut UnixStream,
    request: String,
    config: &Config,
    cycle_index: &mut usize,
    prev_titles: &mut [String; 2],
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
            stream.write_all(b"empty")?;
            return Ok(());
        }

        while m != config.options[current_index].contains("special") {
            current_index = (current_index + 1) % config.titles.len();
        }
    }

    if config.titles[current_index] != prev_titles[0] {
        prev_titles[1] = prev_titles[0].clone();
        prev_titles[0] = config.titles[current_index].clone();
    }

    let next_scratchpad = format!(
        "{}:{}:{}",
        config.titles[current_index], config.commands[current_index], config.options[current_index]
    );
    stream.write_all(next_scratchpad.as_bytes())?;
    *cycle_index = current_index + 1;
    Ok(())
}

fn handle_previous(
    stream: &mut UnixStream,
    request: String,
    config: &Config,
    prev_titles: &mut [String; 2],
) -> Result<()> {
    let previous_active = (request[2..] == prev_titles[0]) as usize;
    if prev_titles[previous_active].is_empty() {
        stream.write_all(b"empty")?;
        return Ok(());
    }

    let index = config
        .titles
        .clone()
        .into_iter()
        .position(|x| x == prev_titles[previous_active]);

    if let Some(prev_index) = index {
        let prev_scratchpad = format!(
            "{}:{}:{}",
            config.titles[prev_index], config.commands[prev_index], config.options[prev_index]
        );
        stream.write_all(prev_scratchpad.as_bytes())?;
    } else {
        stream.write_all(b"empty")?;
    }

    Ok(())
}

fn clean(spotless: bool, config: Arc<Mutex<Config>>) -> Result<()> {
    let mut ev = EventListener::new();

    let config1 = Arc::clone(&config);
    ev.add_workspace_changed_handler(move |_| {
        move_floating(
            config1
                .lock()
                .unwrap_log(file!(), line!())
                .slick_titles
                .clone(),
        )
        .unwrap_log(file!(), line!());

        if let Some(cl) = Client::get_active().unwrap_log(file!(), line!()) {
            if cl.workspace.id < 0 {
                hyprland::dispatch!(ToggleSpecialWorkspace, Some(cl.initial_title))
                    .unwrap_log(file!(), line!());
            }
        }
    });

    let config2 = Arc::clone(&config);
    if spotless {
        ev.add_active_window_changed_handler(move |_| {
            if let Some(cl) = Client::get_active().unwrap_log(file!(), line!()) {
                if !cl.floating {
                    move_floating(
                        config2
                            .lock()
                            .unwrap_log(file!(), line!())
                            .dirty_titles
                            .clone(),
                    )
                    .unwrap_log(file!(), line!());
                }
            }
        });
    }

    ev.start_listener()?;
    Ok(())
}

fn auto_reload(config: Arc<Mutex<Config>>) -> Result<()> {
    let mut ev = EventListener::new();
    ev.add_config_reloaded_handler(move || {
        config
            .lock()
            .unwrap_log(file!(), line!())
            .reload(None)
            .unwrap_log(file!(), line!())
    });

    ev.start_listener()?;
    Ok(())
}

pub fn initialize_daemon(
    args: &[String],
    config_path: Option<String>,
    socket_path: Option<&str>,
) -> Result<()> {
    let config = Arc::new(Mutex::new(Config::new(config_path.clone())?));
    let mut cycle_index: usize = 0;
    let mut prev_titles: [String; 2] = [String::new(), String::new()];
    autospawn(&mut config.lock().unwrap_log(file!(), line!()))?;

    if !args.contains(&"no-auto-reload".to_string()) {
        let config_clone = Arc::clone(&config);
        thread::spawn(move || auto_reload(config_clone));
    }

    if args.contains(&"clean".to_string()) {
        let config_clone = Arc::clone(&config);
        if args[1..].contains(&"spotless".to_string()) {
            thread::spawn(move || clean(true, config_clone));
        } else {
            thread::spawn(move || clean(false, config_clone));
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
    log(
        format!(
            "Daemon successfully started, listening on {:?}",
            path_to_sock
        ),
        "INFO",
    )?;
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let mut buf = String::new();
                stream.read_to_string(&mut buf)?;

                let conf = &mut config.lock().unwrap_log(file!(), line!());
                match buf.as_str() {
                    "kill" => break,
                    "killall" => handle_killall(conf)?,
                    "reload" => handle_reload(conf, config_path.clone())?,
                    b if b.starts_with("p") => {
                        handle_previous(&mut stream, buf, conf, &mut prev_titles)?
                    }
                    b if b.starts_with("r") => handle_return(buf, conf)?,
                    b if b.starts_with("c") => {
                        handle_cycle(&mut stream, buf, conf, &mut cycle_index, &mut prev_titles)?
                    }
                    b if b.starts_with("s") => {
                        handle_scratchpad(&mut stream, buf, conf, &mut prev_titles)?
                    }
                    e => {
                        let error_message = format!("Daemon: unknown request - {e}");
                        stream.write_all(error_message.as_bytes()).unwrap();
                        log(error_message, "ERROR")?;
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
    use hyprland::data::{Clients, Workspace};
    use std::{thread::sleep, time::Duration};

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
            initialize_daemon(
                &args,
                Some("./test_configs/test_config2.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            )
            .unwrap();
        });
        std::thread::sleep(std::time::Duration::from_millis(100));

        test_handle("c", "firefox:firefox --private-window:special sticky");
        test_handle("c?0", "btop:kitty --title btop -e btop:");
        test_handle("c?1", "cmatrix:kitty --title cmatrix -e cmatrix:special");

        test_handle("p?cmatrix", "btop:kitty --title btop -e btop:");
        test_handle("p?htop", "cmatrix:kitty --title cmatrix -e cmatrix:special");

        test_handle("s?btop", "btop htop");
        test_handle("r?btop", "");

        test_handle("reload", "");
        test_handle("killall", "");

        test_handle("unknown", "Daemon: unknown request - unknown");
        test_handle("kill", "");
    }

    struct TestResources {
        titles: [String; 4],
        commands: [String; 4],
        expected_workspace: [String; 4],
    }

    impl Drop for TestResources {
        fn drop(&mut self) {
            self.titles.clone().into_iter().for_each(|title| {
                hyprland::dispatch!(CloseWindow, WindowIdentifier::Title(&title)).unwrap()
            });
            sleep(Duration::from_millis(1000));
        }
    }

    fn setup_test(resources: &TestResources) {
        let clients = Clients::get().unwrap().into_iter();
        resources
            .titles
            .clone()
            .map(|title| assert_eq!(clients.clone().any(|x| x.initial_title == title), false));

        resources
            .commands
            .clone()
            .map(|command| hyprland::dispatch!(Exec, &command).unwrap());
        sleep(Duration::from_millis(2000));
    }

    fn verify_test(resources: &TestResources) {
        let clients = Clients::get().unwrap().into_iter();
        resources
            .titles
            .clone()
            .into_iter()
            .zip(&resources.expected_workspace)
            .for_each(|(title, workspace)| {
                let clients_with_title: Vec<Client> = clients
                    .clone()
                    .filter(|x| x.initial_title == title)
                    .collect();

                assert_eq!(clients_with_title.len(), 1);
                assert_eq!(&clients_with_title[0].workspace.name, workspace);
            });
    }

    #[test]
    fn test_clean() {
        std::thread::spawn(|| {
            let args = vec!["clean".to_string()];
            initialize_daemon(
                &args,
                Some("./test_configs/test_config3.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            )
            .unwrap();
        });
        std::thread::sleep(std::time::Duration::from_millis(100));

        let active_workspace = Workspace::get_active().unwrap();
        let resources = TestResources {
            titles: [
                "test_sticky_clean".to_string(),
                "test_normal_clean".to_string(),
                "test_special_clean".to_string(),
                "test_nonfloating_clean".to_string(),
            ],
            commands: [
                "[float; size 30% 30%; move 60% 0] kitty --title test_sticky_clean".to_string(),
                "[float; workspace special:test_special_clean; size 30% 30%; move 30% 0] kitty --title test_special_clean".to_string(),
                "[float; size 30% 30%; move 0 0] kitty --title test_normal_clean".to_string(),
                "kitty --title test_nonfloating_clean".to_string(),
            ],
            expected_workspace: [
                active_workspace.name.clone(),
                "special:test_normal_clean".to_string(),
                "special:test_special_clean".to_string(),
                active_workspace.name,
            ],
        };

        setup_test(&resources);
        hyprland::dispatch!(Workspace, WorkspaceIdentifierWithSpecial::Relative(1)).unwrap();
        sleep(Duration::from_millis(1000));
        hyprland::dispatch!(Workspace, WorkspaceIdentifierWithSpecial::Relative(-1)).unwrap();

        verify_test(&resources);
        test_handle("kill", "");
        sleep(Duration::from_millis(1000));
    }

    #[test]
    fn test_clean_spotless() {
        std::thread::spawn(|| {
            let args = vec!["clean".to_string(), "spotless".to_string()];
            initialize_daemon(
                &args,
                Some("./test_configs/test_config3.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            )
            .unwrap();
        });
        std::thread::sleep(std::time::Duration::from_millis(100));

        let active_workspace = Workspace::get_active().unwrap();
        let resources = TestResources {
            titles: [
                "test_nonfloating_clean".to_string(),
                "test_sticky_clean".to_string(),
                "test_shiny_clean".to_string(),
                "test_normal_clean".to_string(),
            ],
            commands: [
                "kitty --title test_nonfloating_clean".to_string(),
                "[float; size 30% 30%; move 60% 0] kitty --title test_sticky_clean".to_string(),
                "[float; size 30% 30%; move 30% 0] kitty --title test_shiny_clean".to_string(),
                "[float; size 30% 30%; move 0 0] kitty --title test_normal_clean".to_string(),
            ],
            expected_workspace: [
                active_workspace.name.clone(),
                active_workspace.name.clone(),
                active_workspace.name,
                "special:test_normal_clean".to_string(),
            ],
        };

        let active_client = Client::get_active().unwrap().unwrap();
        setup_test(&resources);

        hyprland::dispatch!(
            FocusWindow,
            WindowIdentifier::Address(active_client.address)
        )
        .unwrap();
        sleep(Duration::from_millis(200));

        verify_test(&resources);
        test_handle("kill", "");
        sleep(Duration::from_millis(1000));
    }
}
