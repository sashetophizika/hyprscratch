use crate::config::Config;
use crate::event::start_event_listeners;
use crate::logs::*;
use crate::scratchpad::Scratchpad;
use crate::utils::*;
use crate::DEFAULT_SOCKET;
use crate::HYPRSCRATCH_DIR;
use hyprland::data::{Client, Clients};
use hyprland::dispatch::*;
use hyprland::error::HyprError;
use hyprland::keyword::Keyword;
use hyprland::prelude::*;
use hyprland::Result;
use std::collections::HashMap;
use std::fs::{create_dir, remove_file};
use std::io::prelude::*;
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::spawn;

type ConfigMutex = Arc<Mutex<Config>>;

#[derive(Clone, Copy)]
pub struct DaemonOptions {
    pub eager: bool,
    pub clean: bool,
    pub spotless: bool,
    pub auto_reload: bool,
}

impl DaemonOptions {
    pub fn new(opts: &str, config: &Config) -> DaemonOptions {
        let options = format!("{opts} {}", config.daemon_options);
        DaemonOptions {
            eager: options.contains("eager"),
            clean: options.contains("clean"),
            spotless: options.contains("spotless"),
            auto_reload: !options.contains("no-auto-reload"),
        }
    }
}

#[derive(Clone)]
pub struct DaemonState {
    pub cycle_index: usize,
    pub prev_titles: [String; 2],
    pub options: Arc<DaemonOptions>,
}

impl DaemonState {
    pub fn new(args: &str, config: &Config) -> DaemonState {
        DaemonState {
            cycle_index: 0,
            prev_titles: [String::new(), String::new()],
            options: Arc::new(DaemonOptions::new(args, config)),
        }
    }

    fn update_prev_titles(&mut self, new_title: &str) {
        if new_title != self.prev_titles[0] {
            self.prev_titles[1] = self.prev_titles[0].clone();
            self.prev_titles[0] = new_title.to_string();
        }
    }
}

fn handle_scratchpad(
    name: &str,
    mode: &str,
    config: &mut Config,
    state: &mut DaemonState,
) -> Result<()> {
    if let Some(sc) = config.scratchpads.get_mut(name) {
        state.update_prev_titles(&sc.title);
        sc.options.toggle(mode);
        sc.trigger(&config.cache.fickle_map, name)?;
        sc.options.toggle(mode);
        Ok(())
    } else {
        let _ = log(format!("Scratchpad or group '{name}' not found"), Warn);
        Ok(())
    }
}

fn handle_group(name: &str, mode: &str, config: &Config, state: &mut DaemonState) -> Result<()> {
    let mut group = config.groups[name].clone();

    if group.is_empty() {
        return Ok(());
    }
    state.update_prev_titles(&group.last().unwrap().title);

    for sc in &mut group {
        let cover = sc.options.cover;
        if !cover {
            sc.options.cover = true;
        }

        sc.options.toggle(mode);
        sc.trigger(&config.cache.fickle_map, name)?;
        sc.options.toggle(mode);

        if cover != sc.options.cover {
            sc.options.cover = false;
        }
    }
    Ok(())
}

fn get_cycle_name(msg: &str, config: &Config, state: &mut DaemonState) -> Option<String> {
    let len = config.scratchpads.len();

    let warn_empty = |titles: &[_]| {
        if titles.is_empty() {
            let _ = log(format!("No {msg} scratchpads found"), Warn);
            return true;
        }
        false
    };

    let find_next = |mode| -> usize {
        let mut index = (state.cycle_index + 1) % len;
        while mode == config.scratchpads[&config.names[index]].options.special {
            index = (index + 1) % len;
        }
        index
    };

    if msg.contains("special") {
        if warn_empty(&config.cache.special_titles) {
            return None;
        }
        state.cycle_index = find_next(false);
    } else if msg.contains("normal") {
        if warn_empty(&config.cache.normal_titles) {
            return None;
        }
        state.cycle_index = find_next(true);
    } else {
        if warn_empty(&config.names) {
            return None;
        }
        state.cycle_index = (state.cycle_index + 1) % len;
    }

    let name = &config.names[state.cycle_index];
    state.update_prev_titles(&config.scratchpads[name].title);

    Some(name.into())
}

fn handle_cycle(msg: &str, config: &mut Config, state: &mut DaemonState) -> Result<()> {
    if config.scratchpads.is_empty() {
        return log("No scratchpads configured for 'cycle'".into(), Warn);
    }

    if let Some(name) = get_cycle_name(msg, config, state) {
        handle_scratchpad(&name, "", config, state)?;
    }

    Ok(())
}

fn handle_previous(msg: &str, config: &mut Config, state: &mut DaemonState) -> Result<()> {
    if state.prev_titles[0].is_empty() {
        return log("No previous scratchpads exist".into(), Warn);
    }

    let active_client = Client::get_active().unwrap_log(file!(), line!());
    let name = match active_client {
        Some(ac)
            if ac.initial_class == state.prev_titles[0]
                || ac.initial_title == state.prev_titles[0] =>
        {
            &state.prev_titles[1]
        }
        Some(_) => &state.prev_titles[0],
        None => &state.prev_titles[0],
    };

    handle_scratchpad(&name.clone(), msg, config, state)?;
    Ok(())
}

fn handle_call(msg: &str, req: &str, config: &mut Config, state: &mut DaemonState) -> Result<()> {
    if msg.is_empty() {
        return log(
            format!("No scratchpad or group title given to '{req}'"),
            Warn,
        );
    }

    if config.groups.contains_key(msg) {
        return handle_group(msg, req, config, state);
    }

    handle_scratchpad(msg, req, config, state)
}

fn handle_manual(msg: &str, config: &mut Config, state: &mut DaemonState) -> Result<()> {
    let args: Vec<&str> = msg.splitn(3, '^').collect();
    state.update_prev_titles(args[0]);

    let scratchpad = Scratchpad::new(args[0], args[1], &args[2..].join(" "));
    config.add_scratchpad(args[0], &scratchpad);
    scratchpad.trigger(&config.cache.fickle_map, args[0])
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
    if state.options.eager {
        autospawn(config)?;
    }

    log("Configuration reloaded".to_string(), Info)?;
    Ok(())
}

fn split_commands(scratchpads: &[Scratchpad]) -> Vec<[String; 3]> {
    let split = |sc: &Scratchpad| -> Vec<[String; 3]> {
        sc.command
            .split('?')
            .map(|cmd| [sc.title.clone(), cmd.trim().into(), sc.options.as_string()])
            .collect()
    };

    scratchpads.iter().flat_map(split).collect()
}

fn format_scratchpads(scratchpads: &[Scratchpad], config_file: &String) -> String {
    let scratchpads = split_commands(scratchpads);
    let format_field = |field: &dyn Fn(&[String; 3]) -> &str| {
        scratchpads
            .iter()
            .map(field)
            .collect::<Vec<_>>()
            .join("\u{2C02}")
    };

    format!(
        "{}\u{2C00}{}\u{2C01}{}\u{2C01}{}",
        config_file,
        format_field(&|x| &x[0]),
        format_field(&|x| &x[1]),
        format_field(&|x| &x[2]),
    )
}

fn format_groups(groups: &HashMap<String, Vec<Scratchpad>>) -> String {
    type GroupField<'a> = &'a dyn Fn((&String, &Vec<Scratchpad>)) -> String;
    let format_group = |field: GroupField| {
        groups
            .iter()
            .map(field)
            .collect::<Vec<_>>()
            .join("\u{2C02}")
    };

    let get_titles = |scratchpads: &Vec<Scratchpad>| {
        scratchpads
            .iter()
            .map(|sc| sc.title.clone())
            .collect::<Vec<String>>()
            .join(",")
    };

    format!(
        "\u{2C00}{}\u{2C01}{}",
        format_group(&|(x, _)| x.clone()),
        format_group(&|(_, x)| get_titles(x))
    )
}

fn handle_get_config(stream: &mut UnixStream, config: &mut Config) -> Result<()> {
    let scratchpads: Vec<Scratchpad> = config
        .names
        .iter()
        .map(|k| config.scratchpads[k].clone())
        .collect();

    let mut config_str = format_scratchpads(&scratchpads, &config.config_file);
    if !config.groups.is_empty() {
        config_str.push_str(&format_groups(&config.groups));
    }

    stream.write_all(config_str.as_bytes())?;
    Ok(())
}

fn handle_killall(config: &Config) -> Result<()> {
    let is_scratchpad = |cl: &Client| config.scratchpads.values().any(|sc| sc.matches_client(cl));

    let kill = |cl: Client| {
        let res = hyprland::dispatch!(CloseWindow, WindowIdentifier::Address(cl.address));
        if let Err(e) = res {
            let _ = log(format!("{e} in {} at {}", file!(), line!()), Warn);
        }
    };

    Clients::get()?
        .into_iter()
        .filter(is_scratchpad)
        .for_each(kill);
    Ok(())
}

fn handle_hideall(config: &Config) -> Result<()> {
    move_floating(&config.cache.normal_map)?;
    if let Ok(Some(ac)) = Client::get_active() {
        hide_special(&ac);
    }
    Ok(())
}

fn run_rofi(mode: &str, config: &mut Config, state: &mut DaemonState) -> Result<()> {
    let mut rofi = Command::new("rofi")
        .args(["-dmenu", "-i", "-p", "Scratchpads"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let rofi_str = config.names.join("\n")
        + "\n"
        + &config
            .groups
            .keys()
            .cloned()
            .fold("".into(), |acc: String, k| acc + "group:" + &k + "\n");

    if let Some(ref mut stdin) = rofi.stdin {
        stdin.write_all(rofi_str.as_bytes())?;
    }

    let output = rofi.wait_with_output()?;
    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            if let Some(("group", name)) = name.split_once(":") {
                handle_group(&name, mode, config, state)?;
            } else {
                handle_scratchpad(&name, mode, config, state)?;
            }
        } else {
            let _ = log("No scratchpad selected for rofi".into(), Warn);
        }
    }
    Ok(())
}

fn handle_rofi(mode: &str, config: &mut Config, state: &mut DaemonState) -> Result<()> {
    let mode = mode.to_string();
    let mut config = config.clone();
    let mut state = state.clone();
    spawn(move || run_rofi(&mode, &mut config, &mut state).log_err(file!(), line!()));
    Ok(())
}

fn handle_request(
    (req, msg): (&str, &str),
    stream: &mut UnixStream,
    state: &mut DaemonState,
    config: &mut Config,
) -> Result<()> {
    match req {
        "toggle" | "summon" | "show" | "hide" => handle_call(msg, req, config, state),
        "get-config" => handle_get_config(stream, config),
        "previous" => handle_previous(msg, config, state),
        "kill-all" => handle_killall(config),
        "hide-all" => handle_hideall(config),
        "reload" => handle_reload(msg, config, state),
        "manual" => handle_manual(msg, config, state),
        "cycle" => handle_cycle(msg, config, state),
        "rofi" => handle_rofi(msg, config, state),
        "kill" => {
            let _ = log("Recieved 'kill' request, terminating listener".into(), Info);
            Err(HyprError::Other("kill".into()))
        }
        _ => log(format!("Unknown request: {req}?{msg}"), Warn),
    }
}

fn get_sock(socket_path: Option<&str>) -> &Path {
    if let Some(sp) = socket_path {
        Path::new(sp)
    } else {
        let temp_dir = Path::new(HYPRSCRATCH_DIR);
        if !temp_dir.exists() {
            create_dir(temp_dir).log_err(file!(), line!());
        }
        Path::new(DEFAULT_SOCKET)
    }
}

fn get_listener(socket_path: Option<&str>) -> Result<UnixListener> {
    let sock = get_sock(socket_path);
    if sock.exists() {
        remove_file(sock)?;
    }

    let listener = UnixListener::bind(sock)?;
    let msg = format!("Daemon started successfully, listening on {sock:?}");
    log(msg, Info)?;
    Ok(listener)
}

fn start_unix_listener(
    socket_path: Option<&str>,
    state: &mut DaemonState,
    config: ConfigMutex,
) -> Result<()> {
    let listener = get_listener(socket_path)?;
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let mut buf = String::new();
                stream.read_to_string(&mut buf)?;

                let (req, msg) = buf.split_once('?').unwrap_log(file!(), line!());
                let conf = &mut config.lock().unwrap_log(file!(), line!());

                match handle_request((req, msg), &mut stream, state, conf) {
                    Ok(()) => (),
                    Err(HyprError::Other(_)) => break,
                    Err(e) => log(format!("{e} in {buf}"), Warn)?,
                }
            }
            Err(_) => {
                continue;
            }
        }
    }

    Ok(())
}

fn make_workspaces_persistent(config: &Config) -> Result<()> {
    let make_persistent = |name| -> Result<()> {
        let rule = format!("special:{}, persistent:true", name);
        Keyword::set("workspace", rule)
    };

    for name in config.scratchpads.keys() {
        make_persistent(name)?;
    }

    for name in config.groups.keys() {
        make_persistent(name)?;
    }

    Ok(())
}

pub fn initialize_daemon(args: String, config_path: Option<String>, socket_path: Option<&str>) {
    let _ = send_request(socket_path, "kill", "");

    let (f, l) = (file!(), line!());
    let config = Config::new(config_path).unwrap_log(f, l);
    let mut state = DaemonState::new(&args, &config);
    make_workspaces_persistent(&config).unwrap_log(f, l);

    let config = Arc::new(Mutex::new(config));
    start_event_listeners(&config, &mut state);

    if state.options.eager {
        autospawn(&mut config.lock().unwrap_log(f, l)).log_err(f, l);
    }

    start_unix_listener(socket_path, &mut state, config).unwrap_log(file!(), line!());
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprland::data::{Clients, Workspace};
    use std::{env, fs::File, thread::sleep, time::Duration};

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
            let args = String::new();
            initialize_daemon(
                args,
                Some("./test_configs/test_config2.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            );
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
        let mut state = DaemonState::new("", &config);

        assert_eq!(
            get_cycle_name("special", &config, &mut state),
            Some("test_special".into())
        );
        assert_eq!(
            get_cycle_name("normal", &config, &mut state),
            Some("test_sticky".into())
        );
        assert_eq!(
            get_cycle_name("", &config, &mut state),
            Some("test_shiny".into())
        );
        assert_eq!(
            get_cycle_name("", &config, &mut state),
            Some("test_pin".into())
        );
        assert_eq!(
            get_cycle_name("", &config, &mut state),
            Some("test_normal".into())
        );
        assert_eq!(
            get_cycle_name("unknown", &config, &mut state),
            Some("test_nonfloating".into())
        );
    }

    struct TestResources {
        titles: [String; 4],
        commands: [String; 4],
        expected_workspace: [String; 4],
    }

    impl Drop for TestResources {
        fn drop(&mut self) {
            self.titles
                .iter()
                .zip(&self.expected_workspace)
                .for_each(|(title, ws)| {
                    if ws != "none" {
                        hyprland::dispatch!(CloseWindow, WindowIdentifier::Title(title)).unwrap();
                    }
                });

            sleep(Duration::from_millis(500));
        }
    }

    fn setup_test(resources: &TestResources) {
        let clients = Clients::get().unwrap().into_iter();
        resources
            .titles
            .clone()
            .map(|title| assert!(!clients.clone().any(|x| x.initial_title == title)));

        resources
            .commands
            .clone()
            .map(|command| hyprland::dispatch!(Exec, &command).unwrap());
        sleep(Duration::from_millis(1000));
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

                if workspace == "none" {
                    assert_eq!(clients_with_title.len(), 0);
                } else {
                    assert_eq!(clients_with_title.len(), 1);
                    assert_eq!(&clients_with_title[0].workspace.name, workspace);
                }
            });
    }

    #[test]
    fn test_clean() {
        std::thread::spawn(|| {
            initialize_daemon(
                "clean".to_string(),
                Some("./test_configs/test_config3.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            );
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
                "[float; size 30% 30%; move 0 0] kitty --title test_normal".to_string(),
                "[float; workspace special:test_special; size 30% 30%; move 30% 0] kitty --title test_special".to_string(),
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
        hyprland::dispatch!(ToggleSpecialWorkspace, Some("special:test_special".into())).unwrap();
        sleep(Duration::from_millis(500));
    }

    #[test]
    fn test_spotless() {
        std::thread::spawn(|| {
            initialize_daemon(
                "spotless".to_string(),
                Some("./test_configs/test_config3.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            );
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
    fn test_vanish() {
        std::thread::spawn(|| {
            initialize_daemon(
                "clean".into(),
                Some("./test_configs/test_config3.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            );
        });
        std::thread::sleep(std::time::Duration::from_millis(100));

        let active_workspace = Workspace::get_active().unwrap();
        let resources = TestResources {
            titles: [
                "test_sticky".to_string(),
                "test_pin".to_string(),
                "test_normal".to_string(),
                "test_ephemeral".to_string(),
            ],
            commands: [
                "[float; size 30% 30%; move 60% 0] kitty --title test_sticky".to_string(),
                "[float; pin; size 30% 30%; move 30% 0] kitty --title test_pin".to_string(),
                "[float; size 30% 30%; move 0 0] kitty --title test_normal".to_string(),
                "[float; size 30% 30%; move 0 30%] kitty --title test_ephemeral".to_string(),
            ],
            expected_workspace: [
                active_workspace.name.clone(),
                (active_workspace.id + 1).to_string(),
                "special:test_normal".to_string(),
                "none".into(),
            ],
        };

        setup_test(&resources);
        hyprland::dispatch!(Workspace, WorkspaceIdentifierWithSpecial::Relative(1)).unwrap();
        sleep(Duration::from_millis(500));

        verify_test(&resources);
        hyprland::dispatch!(Workspace, WorkspaceIdentifierWithSpecial::Relative(-1)).unwrap();
        sleep(Duration::from_millis(500));
    }

    #[test]
    fn test_auto_reload() {
        let config_path = "./test_configs/test_hyprlang.conf".replacen(
            '.',
            env::current_dir().unwrap().as_os_str().to_str().unwrap(),
            1,
        );

        let mut config_file = File::options()
            .read(true)
            .append(true)
            .open(&config_path)
            .unwrap();

        let mut content = String::new();
        config_file.read_to_string(&mut content).unwrap();

        let config = Config::new(Some(config_path.to_string())).unwrap();
        let mut state = DaemonState::new("", &config);

        let config = Arc::new(Mutex::new(config));
        start_event_listeners(&config, &mut state);
        std::thread::sleep(std::time::Duration::from_millis(500));

        config_file
            .write_all(b"test_reload {\ntitle=test_reload\ncommand=test_reload\n}\n")
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1000));

        assert!(config
            .lock()
            .unwrap()
            .names
            .iter()
            .any(|n| n == "test_reload"));

        let mut config_file = File::create(config_path).unwrap();
        config_file.write_all(content.as_bytes()).unwrap();
    }
}
