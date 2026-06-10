use crate::config::Config;
use crate::dispatchers::dispatchers;
use crate::event::start_event_listeners;
use crate::logs::*;
use crate::scratchpad::Scratchpad;
use crate::utils::*;
use crate::DEFAULT_SOCKET;
use crate::HYPRSCRATCH_DIR;
use hyprland::data::{Client, Clients};
use hyprland::dispatch::WindowIdentifier;
use hyprland::error::HyprError;
use hyprland::keyword::Keyword;
use hyprland::prelude::*;
use hyprland::Result;
use std::fs::{create_dir, remove_file};
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, RwLock};

type ConfigMutex = Arc<RwLock<Config>>;

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

struct RequestData<'a> {
    state: &'a mut DaemonState,
    config: &'a mut Config,
    req: String,
    msg: String,
}

impl<'a> RequestData<'a> {
    fn new(state: &'a mut DaemonState, config: &'a mut Config, req: &'a str, msg: &'a str) -> Self {
        RequestData {
            state,
            config,
            req: req.to_string(),
            msg: msg.to_string(),
        }
    }

    fn get_new_index(&mut self) {
        let warn_empty = |titles: &[_]| {
            if titles.is_empty() {
                let _ = log(format!("No {} scratchpads found", self.msg), Warn);
                return true;
            }
            false
        };

        let len = self.config.scratchpads.len();
        let find_next = |mode| -> usize {
            let mut index = (self.state.cycle_index + 1) % len;
            while mode
                == self.config.scratchpads[&self.config.names[index]]
                    .options
                    .special
            {
                index = (index + 1) % len;
            }
            index
        };

        let index = if self.msg.contains("special") {
            if warn_empty(&self.config.cache.special_titles) {
                return;
            }
            find_next(false)
        } else if self.msg.contains("normal") {
            if warn_empty(&self.config.cache.normal_titles) {
                return;
            }
            find_next(true)
        } else {
            if warn_empty(&self.config.names) {
                return;
            }
            (self.state.cycle_index + 1) % len
        };

        self.state.cycle_index = index;
    }

    fn get_next_name(&mut self) -> Option<String> {
        self.get_new_index();
        self.state.update_prev_titles(
            &self.config.scratchpads[&self.config.names[self.state.cycle_index]].title,
        );
        Some((&self.config.names[self.state.cycle_index]).into())
    }

    fn get_config_path(&self) -> Option<String> {
        if !self.msg.is_empty() && Path::new(&self.msg).exists() {
            Some(self.msg.clone())
        } else {
            None
        }
    }
}

fn trigger_action(sc: &mut Scratchpad, data: &mut RequestData) -> Result<()> {
    sc.options.toggle(&data.req);
    sc.trigger(&data.config.cache.replace_map, &data.msg)?;
    Ok(())
}

fn handle_scratchpad(data: &mut RequestData) -> Result<()> {
    let mut sc = match data.config.scratchpads.get_mut(data.msg.as_str()) {
        Some(sc) => sc.clone(),
        None => {
            let _ = log(format!("Scratchpad '{}' not found", data.msg), Warn);
            return Ok(());
        }
    };

    data.state.update_prev_titles(&sc.title);
    trigger_action(&mut sc, data)
}

fn handle_group(data: &mut RequestData) -> Result<()> {
    let group = match data.config.groups.get_mut(data.msg.as_str()) {
        Some(group) => group.clone(),
        None => {
            let _ = log(format!("Group '{}' not found", data.msg), Warn);
            return Ok(());
        }
    };

    if group.is_empty() {
        return Ok(());
    }

    data.state.update_prev_titles(&group.last().unwrap().title);

    for mut sc in group {
        sc.options.cover = true;
        trigger_action(&mut sc, data)?;
    }
    Ok(())
}

fn handle_cycle(mut data: RequestData) -> Result<()> {
    if data.config.scratchpads.is_empty() {
        return log("No scratchpads configured for 'cycle'".into(), Warn);
    }

    if let Some(name) = data.get_next_name() {
        data.msg = name;
        handle_scratchpad(&mut data)?;
        return Ok(());
    }

    Ok(())
}

fn handle_previous(mut data: RequestData) -> Result<()> {
    if data.state.prev_titles[0].is_empty() {
        return log("No previous scratchpads exist".into(), Warn);
    }

    let is_prev = |ac: &Client| {
        ac.initial_class == data.state.prev_titles[0]
            || ac.initial_title == data.state.prev_titles[0]
    };

    data.msg = match Client::get_active() {
        Ok(Some(ac)) if is_prev(&ac) => data.state.prev_titles[1].clone(),
        _ => data.state.prev_titles[0].clone(),
    };

    handle_scratchpad(&mut data)?;
    Ok(())
}

fn handle_call(mut data: RequestData) -> Result<()> {
    if data.msg.is_empty() {
        return log(
            format!("No scratchpad or group title given to '{}'", data.req),
            Warn,
        );
    }

    if let Some(("group", name)) = data.msg.split_once(":") {
        if data.config.groups.contains_key(name) {
            data.msg = name.to_string();
            return handle_group(&mut data);
        }
    }

    handle_scratchpad(&mut data)
}

fn handle_attach(data: RequestData) -> Result<()> {
    if let Some(client) = Client::get_active()? {
        let class = client.initial_class;
        let scratchpad = Scratchpad::new(&class, "", "", &data.msg);
        data.config.add_scratchpad(&class, &scratchpad);
    }

    Ok(())
}

fn handle_manual(mut data: RequestData) -> Result<()> {
    let args: Vec<&str> = data.msg.splitn(3, '^').collect();
    data.state.update_prev_titles(args[0]);

    let mut scratchpad = Scratchpad::new(args[0], args[1], "", &args[2..].join(" "));
    data.config.add_scratchpad(args[0], &scratchpad);

    data.msg = args[0].to_string();
    data.req = String::new();
    let _ = trigger_action(&mut scratchpad, &mut data);
    Ok(())
}

fn handle_reload(data: RequestData) -> Result<()> {
    data.config.reload(data.get_config_path())?;
    if data.state.options.eager {
        autospawn(data.config)?;
    }

    log("Configuration reloaded".to_string(), Info)?;
    Ok(())
}

fn handle_get_config(stream: &mut UnixStream, data: RequestData) -> Result<()> {
    stream.write_all(data.config.get_config_str().as_bytes())?;
    Ok(())
}

fn handle_killall(data: RequestData) -> Result<()> {
    let is_scratchpad = |cl: &Client| {
        data.config
            .scratchpads
            .values()
            .any(|sc| sc.matches_client(cl))
    };

    let kill = |cl: Client| {
        dispatchers()
            .close_window(WindowIdentifier::Address(cl.address))
            .log_err(file!(), line!());
    };

    Clients::get()?
        .into_iter()
        .filter(is_scratchpad)
        .for_each(kill);
    Ok(())
}

fn handle_hideall(data: RequestData) -> Result<()> {
    move_floating(&data.config.cache.normal_map)?;
    if let Ok(Some(ac)) = Client::get_active() {
        hide_special(&ac);
    }
    Ok(())
}

fn handle_menu(stream: &mut UnixStream, data: RequestData) -> Result<()> {
    let config = data.config;
    let list = config.names.join("\n")
        + "\n"
        + &config
            .groups
            .keys()
            .cloned()
            .fold("".into(), |acc: String, k| acc + "group:" + &k + "\n");
    stream.write_all(list.as_bytes())?;
    Ok(())
}

fn handle_request(data: RequestData, stream: &mut UnixStream) -> Result<()> {
    match data.req.as_str() {
        "toggle" | "summon" | "show" | "hide" => handle_call(data),
        "get-config" => handle_get_config(stream, data),
        "previous" => handle_previous(data),
        "kill-all" => handle_killall(data),
        "hide-all" => handle_hideall(data),
        "reload" => handle_reload(data),
        "manual" => handle_manual(data),
        "attach" => handle_attach(data),
        "cycle" => handle_cycle(data),
        "menu" => handle_menu(stream, data),
        "kill" => {
            let _ = log("Recieved 'kill' request, terminating listener".into(), Info);
            Err(HyprError::Other("kill".into()))
        }
        _ => log(
            format!("Unknown request: '{} {}'", data.req, data.msg),
            Warn,
        ),
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
                let buf = read_into_string(&mut stream)?;
                let (req, msg) = match buf.split_once('?') {
                    Some(t) => t,
                    None => {
                        let _ = log(format!("Unrecognized command format {buf}"), Warn);
                        continue;
                    }
                };

                let conf = &mut config.write().unwrap_log(file!(), line!());

                let data = RequestData::new(state, conf, req, msg);

                match handle_request(data, &mut stream) {
                    Ok(()) => (),
                    Err(HyprError::Other(e)) if e == "kill" => break,
                    Err(e) => log(format!("{e} in '{req} {msg}'"), Warn)?,
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
    for name in config.scratchpads.keys() {
        let rule = format!("special:{name}, persistent:true");
        Keyword::set("workspace", rule)?;
    }
    Ok(())
}

pub fn initialize_daemon(args: String, config_path: Option<String>, socket_path: Option<&str>) {
    let _ = send_request(socket_path, "kill", "");

    let (f, l) = (file!(), line!());
    let mut config = Config::new(config_path).unwrap_log(f, l);
    let mut state = DaemonState::new(&args, &config);
    make_workspaces_persistent(&config).log_err(f, l);

    if state.options.eager {
        autospawn(&mut config).log_err(f, l);
    }

    let config = Arc::new(RwLock::new(config));
    start_event_listeners(&config, &mut state);

    start_unix_listener(socket_path, &mut state, config).unwrap_log(file!(), line!());
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprland::data::{Clients, Workspace};
    use hyprland::dispatch::WorkspaceIdentifierWithSpecial;
    use std::io::prelude::*;
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
        let mut config_mut = config.clone();

        let test_cases = vec![
            ("special", "test_special"),
            ("normal", "test_sticky"),
            ("", "test_shiny"),
            ("", "test_pin"),
            ("", "test_normal"),
            ("unknown", "test_nonfloating"),
        ];

        for (message, expected_name) in test_cases {
            let mut data = RequestData::new(&mut state, &mut config_mut, "", message);
            assert_eq!(data.get_next_name(), Some(expected_name.into()));
        }
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
                        dispatchers()
                            .close_window(WindowIdentifier::Title(title))
                            .unwrap();
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
            .map(|command| dispatchers().exec(&command).unwrap());
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
                "[float; size monitor_w*0.3 monitor_h*0.3; move monitor_w*0.6 0] kitty --title test_sticky".to_string(),
                "[float; size monitor_w*0.3 monitor_h*0.3; move 0 0] kitty --title test_normal".to_string(),
                "[float; workspace special:test_special; size monitor_w*0.3 monitor_h*0.3; move monitor_w*0.3 0] kitty --title test_special".to_string(),
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

        dispatchers()
            .workspace(WorkspaceIdentifierWithSpecial::Relative(1))
            .unwrap();
        sleep(Duration::from_millis(500));
        dispatchers()
            .workspace(WorkspaceIdentifierWithSpecial::Relative(-1))
            .unwrap();

        verify_test(&resources);
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
                "[float; size monitor_w*0.3 monitor_h*0.3; move monitor_w*0.6 0] kitty --title test_sticky".to_string(),
                "[float; size monitor_w*0.3 monitor_h*0.3; move monitor_w*0.3 0] kitty --title test_shiny".to_string(),
                "[float; size monitor_w*0.3 monitor_h*0.3; move 0 0] kitty --title test_normal".to_string(),
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

        dispatchers()
            .focus_window(WindowIdentifier::Address(active_client.address))
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
                "[float; size monitor_w*0.3 monitor_h*0.3; move monitor_w*0.6 0] kitty --title test_sticky".to_string(),
                "[float; pin; size monitor_w*0.3 monitor_h*0.3; move monitor_w*0.3 0] kitty --title test_pin".to_string(),
                "[float; size monitor_w*0.3 monitor_h*0.3; move 0 0] kitty --title test_normal".to_string(),
                "[float; size monitor_w*0.3 monitor_h*0.3; move 0 monitor_h*0.3] kitty --title test_ephemeral".to_string(),
            ],
            expected_workspace: [
                active_workspace.name.clone(),
                (active_workspace.id + 1).to_string(),
                "special:test_normal".to_string(),
                "none".into(),
            ],
        };

        setup_test(&resources);
        dispatchers()
            .workspace(WorkspaceIdentifierWithSpecial::Relative(1))
            .unwrap();
        sleep(Duration::from_millis(500));

        verify_test(&resources);
        dispatchers()
            .workspace(WorkspaceIdentifierWithSpecial::Relative(-1))
            .unwrap();
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

        let config = Arc::new(RwLock::new(config));
        start_event_listeners(&config, &mut state);
        std::thread::sleep(std::time::Duration::from_millis(500));

        config_file
            .write_all(b"test_reload {\ntitle=test_reload\ncommand=test_reload\n}\n")
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1000));

        assert!(config
            .read()
            .unwrap()
            .names
            .iter()
            .any(|n| n == "test_reload"));

        let mut config_file = File::create(config_path).unwrap();
        config_file.write_all(content.as_bytes()).unwrap();
    }

}
