use crate::config::Config;
use crate::logs::*;
use crate::scratchpad::Scratchpad;
use crate::scratchpad::ScratchpadOptions;
use crate::utils::*;
use hyprland::data::Client;
use hyprland::data::Clients;
use hyprland::data::Monitor;
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
    eager: bool,
}

impl DaemonState {
    fn new(args: &str) -> DaemonState {
        DaemonState {
            cycle_index: 0,
            prev_titles: [String::new(), String::new()],
            eager: args.contains("eager"),
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

    config.scratchpads[index].trigger(&config.fickle_titles)?;

    let opts = &config.scratchpads[index].options;
    if !opts.shiny && !opts.pin {
        config.dirty_titles.push(title.to_string());
    }
    Ok(())
}

fn get_mode(msg: &str) -> Option<bool> {
    if msg.contains("special") {
        Some(true)
    } else if msg.contains("normal") {
        Some(false)
    } else {
        None
    }
}

fn get_cycle_index(msg: &str, config: &Config, state: &mut DaemonState) -> Option<usize> {
    let mut current_index = state.cycle_index % config.scratchpads.len();
    if let Some(m) = get_mode(msg) {
        if (m && config.special_titles.is_empty()) || (!m && config.normal_titles.is_empty()) {
            let _ = log(format!("No {msg} scratchpads found"), "WARN");
            return None;
        }

        while m != config.scratchpads[current_index].options.special {
            current_index = (current_index + 1) % config.scratchpads.len();
        }
    }

    state.update_prev_titles(&config.scratchpads[current_index].title);
    state.cycle_index = current_index + 1;
    Some(current_index)
}

fn handle_cycle(msg: &str, config: &mut Config, state: &mut DaemonState) -> Result<()> {
    if config.scratchpads.is_empty() {
        return log("No scratchpads configured for 'cycle'".into(), "WARN");
    }

    if let Some(i) = get_cycle_index(msg, config, state) {
        handle_scratchpad(config, state, i)?;
    }

    Ok(())
}

fn get_previous_index(title: String, config: &Config, state: &mut DaemonState) -> Option<usize> {
    let prev_active = (title == state.prev_titles[0]) as usize;
    if state.prev_titles[prev_active].is_empty() {
        let _ = log("No previous scratchpad found".into(), "WARN");
        return None;
    }

    config
        .scratchpads
        .clone()
        .into_iter()
        .position(|x| x.title == state.prev_titles[prev_active])
}

fn handle_previous(config: &mut Config, state: &mut DaemonState) -> Result<()> {
    if state.prev_titles[0].is_empty() {
        return log("No previous scratchpads exist".into(), "WARN");
    }

    let active_title = if let Ok(Some(ac)) = Client::get_active() {
        ac.initial_title
    } else {
        "something that will never be a real title".into()
    };

    if let Some(i) = get_previous_index(active_title, config, state) {
        handle_scratchpad(config, state, i)?;
    }
    Ok(())
}

fn handle_call(msg: &str, req: &str, config: &mut Config, state: &mut DaemonState) -> Result<()> {
    if msg.is_empty() {
        return log(format!("No scratchpad title given to '{req}'"), "WARN");
    }

    let index = config
        .scratchpads
        .clone()
        .into_iter()
        .position(|x| x.name == msg);

    if let Some(i) = index {
        config.scratchpads[i].options.toggle(req);
        handle_scratchpad(config, state, i)?;
        config.scratchpads[i].options.toggle(req);
    } else {
        log(format!("Scratchpad '{msg}' not found"), "WARN")?;
    }

    Ok(())
}

fn handle_manual(msg: &str, config: &Config, state: &mut DaemonState) -> Result<()> {
    let args: Vec<&str> = msg.splitn(3, "^").collect();
    state.update_prev_titles(args[0]);
    Scratchpad::new(args[0], args[0], args[1], &args[2..].join(" ")).trigger(&config.fickle_titles)
}

fn get_config_path(msg: &str) -> Option<String> {
    if !msg.is_empty() && Path::new(msg).exists() {
        Some(msg.to_string())
    } else {
        None
    }
}

fn handle_reload(msg: &str, config: &mut Config, state: &mut DaemonState) -> Result<()> {
    config.reload(get_config_path(msg))?;
    if state.eager {
        autospawn(config)?;
    }

    log("Configuration reloaded".to_string(), "INFO")?;
    Ok(())
}

fn handle_get_config(stream: &mut UnixStream, conf: &Config) -> Result<()> {
    let map_format = |field: &dyn Fn(&Scratchpad) -> String| {
        conf.scratchpads
            .iter()
            .map(field)
            .collect::<Vec<_>>()
            .join("^")
    };

    let config = format!(
        "{}?{}?{}",
        map_format(&|x| x.title.clone()),
        map_format(&|x| x.command.clone()),
        map_format(&|x| x.options.clone().get_string()),
    );

    stream.write_all(config.as_bytes())?;
    Ok(())
}

fn handle_killall(config: &Config) -> Result<()> {
    let is_scratchpad = |cl: &Client| {
        config
            .scratchpads
            .iter()
            .any(|scratchpad| scratchpad.title == cl.initial_title)
    };

    let kill = |cl: Client| {
        let res = hyprland::dispatch!(CloseWindow, WindowIdentifier::Address(cl.address));
        if let Err(e) = res {
            let _ = log(format!("{e} in {} at {}", file!(), line!()), "WARN");
        }
    };

    Clients::get()?
        .into_iter()
        .filter(is_scratchpad)
        .for_each(kill);
    Ok(())
}

fn handle_hideall(config: &Config) -> Result<()> {
    move_floating(&config.normal_titles)?;
    if let Ok(Some(ac)) = Client::get_active() {
        hide_special(&ac);
    }
    Ok(())
}

fn pin(ev: &mut EventListener, config: Arc<Mutex<Config>>) {
    let should_move = |opts: &ScratchpadOptions| -> bool {
        if opts.special {
            return false;
        }

        if let (Some(monitor), Ok(active)) = (&opts.monitor, Monitor::get_active()) {
            if active.name != *monitor && active.id.to_string() != *monitor {
                return false;
            }
        }
        true
    };

    let follow = move || {
        let (f, l) = (file!(), line!());
        let conf = &config.lock().unwrap_log(f, l);
        let move_to_current = |cl: Client| {
            let idx = conf
                .scratchpads
                .iter()
                .position(|x| x.title == cl.initial_title)
                .unwrap_log(f, l);

            if !should_move(&conf.scratchpads[idx].options) {
                return;
            }

            hyprland::dispatch!(
                MoveToWorkspace,
                WorkspaceIdentifierWithSpecial::Relative(0),
                Some(WindowIdentifier::Address(cl.address))
            )
            .log_err(f, l)
        };

        if let Ok(clients) = Clients::get() {
            clients
                .into_iter()
                .filter(|cl| conf.pinned_titles.contains(&cl.initial_title) && cl.workspace.id > 0)
                .for_each(move_to_current);
        }
    };

    let follow_clone = follow.clone();
    ev.add_workspace_changed_handler(move |_| follow_clone());
    ev.add_active_monitor_changed_handler(move |_| follow());
}

fn clean(ev: &mut EventListener, config: Arc<Mutex<Config>>) {
    ev.add_workspace_changed_handler(move |_| {
        let (f, l) = (file!(), line!());
        let slick_titles = &config.lock().unwrap_log(f, l).slick_titles;
        move_floating(slick_titles).log_err(f, l);

        if let Ok(Some(ac)) = Client::get_active() {
            if slick_titles.contains(&ac.initial_title) {
                hide_special(&ac);
            }
        }
    });
}

fn spotless(ev: &mut EventListener, config: Arc<Mutex<Config>>) {
    ev.add_active_window_changed_handler(move |_| {
        if let Ok(Some(cl)) = Client::get_active() {
            if !cl.floating {
                let (f, l) = (file!(), line!());
                let dirty_titles = &config.lock().unwrap_log(f, l).dirty_titles;
                move_floating(dirty_titles).log_err(f, l);
            }
        }
    });
}

fn auto_reload(ev: &mut EventListener, config: Arc<Mutex<Config>>) {
    ev.add_config_reloaded_handler(move || {
        let (f, l) = (file!(), line!());
        config.lock().unwrap_log(f, l).reload(None).log_err(f, l);
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

    let config_clone = config.clone();
    pin(&mut ev, config_clone);

    ev.start_listener()
}

fn get_path_to_sock(socket_path: Option<&str>) -> &Path {
    match socket_path {
        Some(sp) => Path::new(sp),
        None => {
            let temp_dir = Path::new("/tmp/hyprscratch/");
            if !temp_dir.exists() {
                create_dir(temp_dir).log_err(file!(), line!());
            }
            Path::new("/tmp/hyprscratch/hyprscratch.sock")
        }
    }
}

fn start_unix_listener(
    socket_path: Option<&str>,
    state: &mut DaemonState,
    config: Arc<Mutex<Config>>,
) -> Result<()> {
    let path_to_sock = get_path_to_sock(socket_path);
    if path_to_sock.exists() {
        remove_file(path_to_sock)?;
    }

    let listener = UnixListener::bind(path_to_sock)?;
    log(
        format!("Daemon started successfully, listening on {path_to_sock:?}",),
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
                    "toggle" | "summon" | "show" | "hide" => handle_call(msg, req, conf, state),
                    "get-config" => handle_get_config(&mut stream, conf),
                    "previous" => handle_previous(conf, state),
                    "kill-all" => handle_killall(conf),
                    "hide-all" => handle_hideall(conf),
                    "reload" => handle_reload(msg, conf, state),
                    "manual" => handle_manual(msg, conf, state),
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
    let _ = connect_to_sock(socket_path, "kill", "");

    let config = Arc::new(Mutex::new(
        Config::new(config_path.clone()).unwrap_log(file!(), line!()),
    ));

    let options = DaemonOptions::new(&args);
    let config_clone = Arc::clone(&config);
    thread::spawn(move || start_event_listeners(options, config_clone).log_err(file!(), line!()));

    let mut state = DaemonState::new(&args);
    if state.eager {
        let (f, l) = (file!(), line!());
        autospawn(&mut config.lock().unwrap_log(f, l)).log_err(f, l);
    }

    start_unix_listener(socket_path, &mut state, config).unwrap_log(file!(), line!());
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

        test_handle("reload?");
        test_handle("kill-all?");
        test_handle("hide-all?");
        test_handle("?unknown");
        test_handle("kill?");
    }

    #[test]
    fn test_state() {
        let config = Config::new(Some("test_configs/test_config3.txt".into())).unwrap();
        let mut state = DaemonState::new("".into());

        assert_eq!(get_cycle_index("special", &config, &mut state), Some(2));
        assert_eq!(get_cycle_index("normal", &config, &mut state), Some(3));
        assert_eq!(get_cycle_index("", &config, &mut state), Some(4));
        assert_eq!(get_cycle_index("", &config, &mut state), Some(5));
        assert_eq!(get_cycle_index("", &config, &mut state), Some(0));
        assert_eq!(get_cycle_index("unknown", &config, &mut state), Some(1));

        assert_eq!(
            get_previous_index("test_nonexistant".into(), &config, &mut state),
            Some(1)
        );
        assert_eq!(
            get_previous_index("test_nonfloating".into(), &config, &mut state),
            Some(0)
        );
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
            sleep(Duration::from_millis(500));
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
                "test_sticky".to_string(),
                "test_normal".to_string(),
                "test_special".to_string(),
                "test_nonfloating".to_string(),
            ],
            commands: [
                "[float; size 30% 30%; move 60% 0] kitty --title test_sticky".to_string(),
                "[float; workspace special:test_special; size 30% 30%; move 30% 0] kitty --title test_special".to_string(),
                "[float; size 30% 30%; move 0 0] kitty --title test_normal".to_string(),
                "kitty --title test_nonfloating".to_string(),
            ],
            expected_workspace: [
                active_workspace.name.clone(),
                "special:test_normal".to_string(),
                "special:test_special".to_string(),
                active_workspace.name,
            ],
        };

        setup_test(&resources);
        hyprland::dispatch!(Workspace, WorkspaceIdentifierWithSpecial::Relative(1)).unwrap();
        sleep(Duration::from_millis(500));
        hyprland::dispatch!(Workspace, WorkspaceIdentifierWithSpecial::Relative(-1)).unwrap();

        verify_test(&resources);
        sleep(Duration::from_millis(500));
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
                "test_nonfloating".to_string(),
                "test_sticky".to_string(),
                "test_shiny".to_string(),
                "test_normal".to_string(),
            ],
            commands: [
                "kitty --title test_nonfloating".to_string(),
                "[float; size 30% 30%; move 60% 0] kitty --title test_sticky".to_string(),
                "[float; size 30% 30%; move 30% 0] kitty --title test_shiny".to_string(),
                "[float; size 30% 30%; move 0 0] kitty --title test_normal".to_string(),
            ],
            expected_workspace: [
                active_workspace.name.clone(),
                active_workspace.name.clone(),
                active_workspace.name,
                "special:test_normal".to_string(),
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
        sleep(Duration::from_millis(500));
    }

    #[test]
    fn test_pin() {
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
                "test_sticky".to_string(),
                "test_pin".to_string(),
                "test_normal".to_string(),
                "test_nonfloating".to_string(),
            ],
            commands: [
                "[float; size 30% 30%; move 60% 0] kitty --title test_sticky".to_string(),
                "[float; size 30% 30%; move 30% 0] kitty --title test_pin".to_string(),
                "[float; size 30% 30%; move 0 0] kitty --title test_normal".to_string(),
                "kitty --title test_nonfloating".to_string(),
            ],
            expected_workspace: [
                active_workspace.name.clone(),
                (active_workspace.id + 1).to_string(),
                "special:test_normal".to_string(),
                active_workspace.name,
            ],
        };

        setup_test(&resources);
        hyprland::dispatch!(Workspace, WorkspaceIdentifierWithSpecial::Relative(1)).unwrap();
        sleep(Duration::from_millis(500));

        verify_test(&resources);
        hyprland::dispatch!(Workspace, WorkspaceIdentifierWithSpecial::Relative(-1)).unwrap();
        sleep(Duration::from_millis(500));
    }
}
