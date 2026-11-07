use crate::config::Config;
use crate::daemon::{DaemonOptions, DaemonState};
use crate::ipc::data::{Client, Clients, Workspace};
use crate::ipc::dispatchers::{dispatchers, WindowIdentifier};
use crate::error::Result;
use crate::ipc;
use crate::logs::*;
use crate::utils::*;
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, RwLock};
use std::thread::*;
use std::time::Duration;

type ConfigMutex = Arc<RwLock<Config>>;

fn moved_to_active(active: &Workspace, workspace_name: &str) -> bool {
    workspace_name == active.name
}

fn parse_moved_window(data: &str) -> Option<(String, &str)> {
    let mut fields = data.splitn(3, ',');
    let address = fields.next()?;
    let workspace_name = fields.next()?;

    let address = if address.starts_with("0x") {
        address.to_string()
    } else {
        format!("0x{address}")
    };

    Some((address, workspace_name))
}

fn handle_vanish(data: &str, config: &ConfigMutex) {
    let (f, l) = (file!(), line!());
    let ephemeral_titles = &config.read().unwrap_log(f, l).cache.ephemeral_titles;

    if ephemeral_titles.is_empty() {
        return;
    }

    let Some((address, workspace_name)) = parse_moved_window(data) else {
        return;
    };

    if let (Ok(clients), Ok(active)) = (Clients::get(), Workspace::get_active()) {
        clients
            .iter()
            .filter(|cl| cl.address == address && !moved_to_active(&active, workspace_name))
            .filter(|cl| is_known(ephemeral_titles, cl))
            .for_each(|cl| {
                dispatchers()
                    .close_window(WindowIdentifier::Address(cl.address.clone()))
                    .log_err(f, l);
            });
    }
}

fn handle_clean(config: &ConfigMutex) {
    let (f, l) = (file!(), line!());
    let slick_map = &config.read().unwrap_log(f, l).cache.clean_map;
    move_floating(slick_map).log_err(f, l);

    if let Ok(Some(ac)) = Client::get_active() {
        if is_known_map(slick_map, &ac) {
            hide_special(&ac);
        }
    }
}

fn handle_spotless(config: &ConfigMutex) {
    if let Ok(Some(cl)) = Client::get_active() {
        let (f, l) = (file!(), line!());
        let conf = &config.read().unwrap_log(f, l);

        if !is_known(&conf.cache.normal_titles, &cl) {
            move_floating(&conf.cache.spotless_map).log_err(f, l);
        }
    }
}

fn handle_config_reload(config: &ConfigMutex) {
    let (f, l) = (file!(), line!());
    config.write().unwrap_log(f, l).reload(None).log_err(f, l);
}

fn start_events(options: Arc<DaemonOptions>, config: ConfigMutex) -> Result<()> {
    let stream = UnixStream::connect(ipc::event_socket_path()?)?;
    let reader = BufReader::new(stream);

    for line in reader.lines() {
        let line = line?;
        let Some((event, data)) = line.split_once(">>") else {
            continue;
        };

        match event {
            "movewindowv2" | "windowmoved" | "windowmovedv2" => handle_vanish(data, &config),
            "workspace" | "workspacev2" if options.clean => handle_clean(&config),
            "activewindow" | "activewindowv2" if options.spotless => handle_spotless(&config),
            "configreloaded" if options.auto_reload => handle_config_reload(&config),
            _ => {}
        }
    }

    Ok(())
}

fn keep_alive(mut handle: JoinHandle<()>, options: Arc<DaemonOptions>, config: ConfigMutex) {
    let max_restarts = 50;
    let mut restarts = 0;

    loop {
        let _ = handle.join();
        let config = config.clone();
        let options = options.clone();

        restarts += 1;
        if restarts >= max_restarts {
            log(
                "Event listener repeated panic, terminating thread.".to_string(),
                Warn,
            );
            break;
        }

        log("Event listener panic, restarting thread".to_string(), Warn);
        handle = spawn(|| start_events(options, config).log_err(file!(), line!()));
    }
}

fn reload_on_modify(res: notify::Result<Event>, config: ConfigMutex) {
    let (f, l) = (file!(), line!());
    let mut config_guard = config.write().unwrap_log(f, l);
    let config_path = PathBuf::from(&config_guard.config_file);

    match res {
        Ok(e) if e.paths.contains(&config_path) => {
            if let EventKind::Modify(ModifyKind::Data(_)) = e.kind {
                sleep(Duration::from_millis(100));
                config_guard.reload(None).log_err(f, l);
            }
        }
        Err(err) => {
            log(format!("Watcher returned error: {err}"), Warn);
        }
        _ => (),
    }
}

fn start_auto_reload(config: ConfigMutex) -> notify::Result<()> {
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(tx)?;

    let (f, l) = (file!(), line!());
    watcher.watch(
        Path::new(&config.read().unwrap_log(f, l).config_file)
            .parent()
            .unwrap_log(f, l),
        RecursiveMode::NonRecursive,
    )?;

    for res in rx {
        reload_on_modify(res, config.clone());
    }
    Ok(())
}

pub fn start_event_listeners(config: &ConfigMutex, state: &mut DaemonState) {
    let (f, l) = (file!(), line!());
    if state.options.auto_reload {
        let config_c = config.clone();
        spawn(move || start_auto_reload(config_c).log_err(f, l));
    }

    let config_c = config.clone();
    let options = state.options.clone();
    let handle = spawn(move || start_events(options, config_c).log_err(f, l));

    let config_c = config.clone();
    let options = state.options.clone();
    spawn(move || keep_alive(handle, options, config_c));
}
