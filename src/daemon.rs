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

struct DaemonState {
    cycle_index: usize,
    prev_titles: [String; 2],
}

impl DaemonState {
    fn new() -> DaemonState {
        DaemonState {
            cycle_index: 0,
            prev_titles: [String::new(), String::new()],
        }
    }

    fn update_prev_titles(&mut self, new_title: &str) {
        if new_title != self.prev_titles[0] {
            self.prev_titles[1] = self.prev_titles[0].clone();
            self.prev_titles[0] = new_title.to_string();
        }
    }
}

struct DaemonOptions {
    clean: bool,
    spotless: bool,
    auto_reload: bool,
}

impl DaemonOptions {
    fn new(opts: &str) -> DaemonOptions {
        DaemonOptions {
            clean: opts.contains("clean"),
            spotless: opts.contains("spotless"),
            auto_reload: !opts.contains("no-auto-reload"),
        }
    }
}

fn handle_scratchpad(config: &mut Config, state: &mut DaemonState, index: usize) -> Result<()> {
    if config.scratchpads.len() <= index {
        return Ok(());
    }

    let title = config.scratchpads[index].title.clone();
    config.dirty_titles.retain(|x| *x != title);
    state.update_prev_titles(&title);

    config.scratchpads[index].run(&config.non_persist_titles.join(" "))?;

    if !config.scratchpads[index].options.shiny {
        config.dirty_titles.push(title.to_string());
    }
    Ok(())
}

fn handle_cycle(msg: &str, config: &mut Config, state: &mut DaemonState) -> Result<()> {
    if config.scratchpads.is_empty() {
        return Ok(());
    }

    let mut current_index = state.cycle_index % config.scratchpads.len();
    let mode = if msg.is_empty() {
        None
    } else {
        Some(msg.as_bytes()[0] != 48)
    };

    if let Some(m) = mode {
        if (m && config.special_titles.is_empty()) || (!m && config.normal_titles.is_empty()) {
            return Ok(());
        }

        while m != config.scratchpads[current_index].options.special {
            current_index = (current_index + 1) % config.scratchpads.len();
        }
    }

    state.update_prev_titles(&config.scratchpads[current_index].title);
    state.cycle_index = current_index + 1;

    handle_scratchpad(config, state, current_index)?;
    Ok(())
}

fn handle_call(msg: &str, req: &str, config: &mut Config, state: &mut DaemonState) -> Result<()> {
    if msg.is_empty() {
        return Ok(());
    }

    let i = config
        .scratchpads
        .clone()
        .into_iter()
        .position(|x| x.name == msg);
    if let Some(i) = i {
        config.scratchpads[i].options.toggle(req);
        handle_scratchpad(config, state, i)?;
        config.scratchpads[i].options.toggle(req);
    }
    Ok(())
}

fn handle_previous(msg: &str, config: &mut Config, state: &mut DaemonState) -> Result<()> {
    if msg.is_empty() {
        return Ok(());
    }

    let prev_active = (msg == state.prev_titles[0]) as usize;
    if state.prev_titles[prev_active].is_empty() {
        return Ok(());
    }

    let index = config
        .scratchpads
        .clone()
        .into_iter()
        .position(|x| x.title == state.prev_titles[prev_active]);

    if let Some(i) = index {
        handle_scratchpad(config, state, i)?;
    }
    Ok(())
}

fn handle_reload(msg: &str, config: &mut Config, eager: bool) -> Result<()> {
    let config_path = if !msg.is_empty() && Path::new(msg).exists() {
        Some(msg.to_string())
    } else {
        None
    };
    config.reload(config_path)?;
    autospawn(config, eager)?;

    log("Configuration reloaded".to_string(), "INFO")?;
    Ok(())
}

fn handle_get_config(stream: &mut UnixStream, conf: &Config) -> Result<()> {
    let config = format!(
        "{}?{}?{}",
        conf.scratchpads
            .iter()
            .map(|x| x.title.clone())
            .collect::<Vec<_>>()
            .join("^"),
        conf.scratchpads
            .iter()
            .map(|x| x.command.clone())
            .collect::<Vec<_>>()
            .join("^"),
        conf.scratchpads
            .iter()
            .map(|x| x.options.clone().get_string())
            .collect::<Vec<_>>()
            .join("^"),
    );

    stream.write_all(config.as_bytes())?;
    Ok(())
}

fn handle_killall(config: &Config) -> Result<()> {
    Clients::get()?
        .into_iter()
        .filter(|x| {
            config
                .scratchpads
                .iter()
                .any(|scratchpad| scratchpad.title == x.initial_title)
        })
        .for_each(|x| {
            let res = hyprland::dispatch!(CloseWindow, WindowIdentifier::Address(x.address));
            if let Err(e) = res {
                let _ = log(format!("{e} in {} at {}", file!(), line!()), "WARN");
            }
        });
    Ok(())
}

fn clean(ev: &mut EventListener, config: Arc<Mutex<Config>>) {
    ev.add_workspace_changed_handler(move |_| {
        move_floating(
            config
                .lock()
                .unwrap_log(file!(), line!())
                .slick_titles
                .clone(),
        )
        .log_err(file!(), line!());

        if let Ok(Some(cl)) = Client::get_active() {
            if cl.workspace.id < 0 && cl.workspace.id > -1000 {
                hyprland::dispatch!(ToggleSpecialWorkspace, Some(cl.initial_title))
                    .log_err(file!(), line!())
            }
        }
    });
}

fn spotless(ev: &mut EventListener, config: Arc<Mutex<Config>>) {
    ev.add_active_window_changed_handler(move |_| {
        if let Ok(Some(cl)) = Client::get_active() {
            if !cl.floating {
                move_floating(
                    config
                        .lock()
                        .unwrap_log(file!(), line!())
                        .dirty_titles
                        .clone(),
                )
                .log_err(file!(), line!());
            }
        }
    });
}

fn auto_reload(ev: &mut EventListener, config: Arc<Mutex<Config>>) {
    ev.add_config_reloaded_handler(move || {
        config
            .lock()
            .unwrap_log(file!(), line!())
            .reload(None)
            .log_err(file!(), line!())
    });
}

fn start_event_listeners(options: DaemonOptions, config: Arc<Mutex<Config>>) -> Result<()> {
    let mut ev = EventListener::new();

    if options.auto_reload {
        let config_clone = config.clone();
        auto_reload(&mut ev, config_clone);
    }

    if options.clean {
        let config_clone = config.clone();
        clean(&mut ev, config_clone);
    }

    if options.spotless {
        let config_clone = config.clone();
        spotless(&mut ev, config_clone);
    }

    ev.start_listener()
}

fn start_unix_listener(
    socket_path: Option<&str>,
    state: &mut DaemonState,
    config: Arc<Mutex<Config>>,
    eager: bool,
) -> Result<()> {
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
            "Daemon started successfully, listening on {:?}",
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
                let (req, msg) = buf.split_once("?").unwrap_log(file!(), line!());

                let res = match req {
                    "toggle" | "summon" | "hide" => handle_call(msg, req, conf, state),
                    "get-config" => handle_get_config(&mut stream, conf),
                    "previous" => handle_previous(msg, conf, state),
                    "killall" => handle_killall(conf),
                    "reload" => handle_reload(msg, conf, eager),
                    "cycle" => handle_cycle(msg, conf, state),
                    "kill" => {
                        log(
                            "Recieved 'kill' request, terminating listener".into(),
                            "INFO",
                        )?;
                        break;
                    }
                    _ => log(format!("Unknown request: {buf}"), "WARN"),
                };

                if let Err(e) = res {
                    log(format!("{e} in {req}:{msg}"), "WARN")?;
                }
            }
            Err(_) => {
                continue;
            }
        };
    }

    Ok(())
}

pub fn initialize_daemon(args: String, config_path: Option<String>, socket_path: Option<&str>) {
    let config = Arc::new(Mutex::new(
        Config::new(config_path.clone()).unwrap_log(file!(), line!()),
    ));

    let eager = args.contains("eager");
    autospawn(&mut config.lock().unwrap_log(file!(), line!()), eager).log_err(file!(), line!());

    let options = DaemonOptions::new(&args);
    let config_clone = Arc::clone(&config);
    thread::spawn(move || start_event_listeners(options, config_clone).log_err(file!(), line!()));

    let mut state = DaemonState::new();
    start_unix_listener(socket_path, &mut state, config, eager).unwrap_log(file!(), line!());
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprland::data::{Clients, Workspace};
    use std::{thread::sleep, time::Duration};

    fn test_handle(request: &str) {
        let mut stream = UnixStream::connect("/tmp/hyprscratch_test.sock").unwrap();

        stream.write_all(request.as_bytes()).unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();

        let mut buf = String::new();
        stream.read_to_string(&mut buf).unwrap();
    }

    #[test]
    fn test_handlers() {
        std::thread::spawn(|| {
            let args = "".to_string();
            initialize_daemon(
                args,
                Some("./test_configs/test_config2.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            )
        });
        std::thread::sleep(std::time::Duration::from_millis(100));

        //test_handle("cycle?");
        //test_handle("cycle?0");
        //test_handle("cycle?1");
        //
        //test_handle("toggle?");
        //test_handle("toggle?unknown");
        //test_handle("toggle?btop");
        //
        //test_handle("previous?cmat");
        //test_handle("previous?htop");
        //
        //test_handle("summon?btop");
        //test_handle("hide?btop");

        test_handle("reload?");
        test_handle("killall?");
        test_handle("?unknown");
        test_handle("kill?");
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
            let args = "clean".to_string();
            initialize_daemon(
                args,
                Some("./test_configs/test_config3.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            )
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
        test_handle("kill?");
        sleep(Duration::from_millis(1000));
    }

    #[test]
    fn test_spotless() {
        std::thread::spawn(|| {
            let args = "spotless".to_string();
            initialize_daemon(
                args,
                Some("./test_configs/test_config3.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            )
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
        sleep(Duration::from_millis(500));

        verify_test(&resources);
        test_handle("kill?");
        sleep(Duration::from_millis(1000));
    }
}
