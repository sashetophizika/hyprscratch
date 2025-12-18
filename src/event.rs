use crate::config::Config;
use crate::daemon::{DaemonOptions, DaemonState};
use crate::logs::*;
use crate::utils::*;
use hyprland::data::{Client, Clients, Workspace};
use hyprland::dispatch::*;
use hyprland::event_listener::EventListener;
use hyprland::prelude::*;
use hyprland::Result;
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::*;
use std::time::Duration;

type ConfigMutex = Arc<Mutex<Config>>;

fn add_vanish(ev: &mut EventListener, config: ConfigMutex) {
    ev.add_window_moved_handler(move |data| {
        let (f, l) = (file!(), line!());
        let ephemeral_titles = &config.lock().unwrap_log(f, l).cache.ephemeral_titles;

        if ephemeral_titles.is_empty() {
            return;
        }

        if let (Ok(clients), Ok(active)) = (Clients::get(), Workspace::get_active()) {
            clients
                .iter()
                .filter(|cl| cl.address == data.window_address && active.id != data.workspace_id)
                .filter(|cl| is_known(ephemeral_titles, cl))
                .for_each(|cl| {
                    hyprland::dispatch!(CloseWindow, WindowIdentifier::Address(cl.address.clone()))
                        .log_err(f, l);
                });
        }
    });
}

fn add_clean(ev: &mut EventListener, config: ConfigMutex) {
    ev.add_workspace_changed_handler(move |_| {
        let (f, l) = (file!(), line!());
        let slick_map = &config.lock().unwrap_log(f, l).cache.clean_map;
        move_floating(slick_map).log_err(f, l);

        if let Ok(Some(ac)) = Client::get_active() {
            if is_known_map(slick_map, &ac) {
                hide_special(&ac);
            }
        }
    });
}

fn add_spotless(ev: &mut EventListener, config: ConfigMutex) {
    ev.add_active_window_changed_handler(move |_| {
        if let Ok(Some(cl)) = Client::get_active() {
            let (f, l) = (file!(), line!());
            let conf = &config.lock().unwrap_log(f, l);

            if !is_known(&conf.cache.normal_titles, &cl) {
                move_floating(&conf.cache.spotless_map).log_err(f, l);
            }
        }
    });
}

fn add_builtin_reload(ev: &mut EventListener, config: ConfigMutex) {
    ev.add_config_reloaded_handler(move || {
        let (f, l) = (file!(), line!());
        config.lock().unwrap_log(f, l).reload(None).log_err(f, l);
    });
}

fn start_events(options: Arc<DaemonOptions>, config: ConfigMutex) -> Result<()> {
    let mut ev = EventListener::new();

    if options.auto_reload {
        add_builtin_reload(&mut ev, config.clone());
    }

    if options.clean {
        add_clean(&mut ev, config.clone());
    }

    if options.spotless {
        add_spotless(&mut ev, config.clone());
    }

    add_vanish(&mut ev, config.clone());
    ev.start_listener()
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
            let _ = log(
                "Event listener repeated panic, terminating thread.".to_string(),
                Warn,
            );
            break;
        }

        let _ = log("Event listener panic, restarting thread".to_string(), Warn);
        handle = spawn(|| start_events(options, config).log_err(file!(), line!()));
    }
}

fn reload_on_modify(res: notify::Result<Event>, config: ConfigMutex) {
    let (f, l) = (file!(), line!());
    let mut config_guard = config.lock().unwrap_log(f, l);
    let config_path = PathBuf::from(&config_guard.config_file);

    match res {
        Ok(e) if e.paths.contains(&config_path) => {
            if let EventKind::Modify(ModifyKind::Data(_)) = e.kind {
                sleep(Duration::from_millis(100));
                config_guard.reload(None).log_err(f, l);
            }
        }
        Err(err) => {
            let _ = log(format!("Watcher returned error: {err}"), Warn);
        }
        _ => (),
    }
}

fn start_auto_reload(config: ConfigMutex) -> notify::Result<()> {
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(tx)?;

    let (f, l) = (file!(), line!());
    let config_file = &config.lock().unwrap_log(f, l).config_file;
    watcher.watch(
        Path::new(&config_file).parent().unwrap_log(f, l),
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
