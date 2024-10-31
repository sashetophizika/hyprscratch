use crate::config::Config;
use crate::log;
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
use std::thread;

fn handle_scratchpad(stream: &mut UnixStream, request: String, config: &mut Config) -> Result<()> {
    if request.len() > 1 {
        config.dirty_titles.retain(|x| *x != request[2..]);
    }

    let titles_string = config.normal_titles.join(" ");
    stream.write_all(titles_string.as_bytes())?;
    Ok(())
}

fn handle_return(title: String, config: &mut Config) -> Result<()> {
    config.dirty_titles.push(title[2..].to_string());
    Ok(())
}

fn handle_reload(config: &mut Config) -> Result<()> {
    config.reload(None)?;
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
            stream.write_all(b"empty")?;
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

fn clean(spotless: &str, config: Arc<Mutex<Config>>) -> Result<()> {
    let mut ev = EventListener::new();

    let config1 = Arc::clone(&config);
    ev.add_workspace_changed_handler(move |_| {
        move_floating(config1.lock().unwrap().slick_titles.clone()).unwrap();
        if let Some(cl) = Client::get_active().unwrap() {
            if cl.workspace.id < 0 {
                hyprland::dispatch!(ToggleSpecialWorkspace, Some(cl.title)).unwrap();
            }
        }
    });

    let config2 = Arc::clone(&config);
    if spotless == "spotless" {
        ev.add_active_window_changed_handler(move |_| {
            if let Some(cl) = Client::get_active().unwrap() {
                if !cl.floating {
                    move_floating(config2.lock().unwrap().dirty_titles.clone()).unwrap();
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
        config.lock().unwrap().reload(None).unwrap();
    });

    ev.start_listener()?;
    Ok(())
}

pub fn initialize_daemon(
    args: &[String],
    config_path: Option<String>,
    socket_path: Option<&str>,
) -> Result<()> {
    let config = Arc::new(Mutex::new(Config::new(config_path)?));
    let mut cycle_index: usize = 0;
    autospawn(&mut config.lock().unwrap())?;

    if !args.contains(&"no-auto-reload".to_string()) {
        let config_clone = Arc::clone(&config);
        thread::spawn(move || auto_reload(config_clone));
    }

    if args.contains(&"clean".to_string()) {
        let config_clone = Arc::clone(&config);
        if args[1..].contains(&"spotless".to_string()) {
            thread::spawn(move || clean("spotless", config_clone));
        } else {
            thread::spawn(move || clean(" ", config_clone));
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
                stream.try_clone()?.read_to_string(&mut buf)?;

                match buf.as_str() {
                    "kill" => break,
                    "reload" => handle_reload(&mut config.lock().unwrap())?,
                    b if b.starts_with("c") => {
                        handle_cycle(&mut stream, &mut cycle_index, &config.lock().unwrap(), buf)?
                    }
                    b if b.starts_with("s") => {
                        handle_scratchpad(&mut stream, buf, &mut config.lock().unwrap())?
                    }
                    b if b.starts_with("r") => handle_return(buf, &mut config.lock().unwrap())?,
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

        test_handle("c", "firefox:firefox --private-window:special");
        test_handle("c?0", "ytop:kitty --title btop -e ytop:");
        test_handle("c?1", "cmatrix:kitty --title cmatrix -e cmatrix:special");

        test_handle("s?btop", "ytop htop");
        test_handle("r?btop", "");

        test_handle("reload", "");
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

    #[test]
    fn test_daemon() {
        test_handlers();
        test_clean();
        test_clean_spotless();
    }
}
