use crate::config::Config;
use crate::daemon::DaemonOptions;
use crate::daemon::DaemonState;
use crate::logs::*;
use crate::scratchpad::ScratchpadOptions;
use crate::utils::*;
use hyprland::data::{Client, Clients, Monitor};
use hyprland::dispatch::*;
use hyprland::event_listener::EventListener;
use hyprland::prelude::*;
use hyprland::Result;
use std::sync::MutexGuard;
use std::sync::{Arc, Mutex};
use std::thread::*;

type ConfigMutex = Arc<Mutex<Config>>;

fn get_client_opts<'a>(cl: &Client, config: &'a MutexGuard<Config>) -> &'a ScratchpadOptions {
    let idx = config
        .scratchpads
        .iter()
        .position(|x| x.title == cl.initial_title)
        .unwrap_log(file!(), line!());

    &config.scratchpads[idx].options
}

fn should_move(cl: &Client, config: &MutexGuard<Config>) -> bool {
    let opts = get_client_opts(cl, config);
    if opts.special {
        return false;
    }

    if let (Some(monitor), Ok(active)) = (&opts.monitor, Monitor::get_active()) {
        if active.name != *monitor && active.id.to_string() != *monitor {
            return false;
        }
    }
    true
}

fn move_to_current(cl: &Client, config: &MutexGuard<Config>) {
    if !should_move(cl, config) {
        return;
    }

    hyprland::dispatch!(
        MoveToWorkspace,
        WorkspaceIdentifierWithSpecial::Relative(0),
        Some(WindowIdentifier::Address(cl.address.clone()))
    )
    .log_err(file!(), line!())
}

fn follow_workpace(config: &ConfigMutex) {
    let conf = config.lock().unwrap_log(file!(), line!());
    if let Ok(clients) = Clients::get() {
        clients
            .into_iter()
            .filter(|cl| !is_on_special(cl) && is_known(&conf.pinned_titles, cl))
            .for_each(|cl| move_to_current(&cl, &conf));
    }
}

fn add_pin(ev: &mut EventListener, config: ConfigMutex) {
    let follow = move || follow_workpace(&config);
    let follow_clone = follow.clone();

    ev.add_workspace_changed_handler(move |_| follow_clone());
    ev.add_active_monitor_changed_handler(move |_| follow());
}

fn add_clean(ev: &mut EventListener, config: ConfigMutex) {
    ev.add_workspace_changed_handler(move |_| {
        let (f, l) = (file!(), line!());
        let slick_titles = &config.lock().unwrap_log(f, l).slick_titles;
        move_floating(slick_titles).log_err(f, l);

        if let Ok(Some(ac)) = Client::get_active() {
            if is_known(slick_titles, &ac) {
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

            if !is_known(&conf.normal_titles, &cl) {
                move_floating(&conf.dirty_titles).log_err(f, l);
            }
        }
    });
}

fn add_auto_reload(ev: &mut EventListener, config: ConfigMutex) {
    ev.add_config_reloaded_handler(move || {
        let (f, l) = (file!(), line!());
        config.lock().unwrap_log(f, l).reload(None).log_err(f, l);
    });
}

fn start_events(options: Arc<DaemonOptions>, config: ConfigMutex) -> Result<()> {
    let mut ev = EventListener::new();

    if options.auto_reload {
        add_auto_reload(&mut ev, config.clone());
    }

    if options.clean {
        add_clean(&mut ev, config.clone());
    }

    if options.spotless {
        add_spotless(&mut ev, config.clone());
    }

    add_pin(&mut ev, config.clone());
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
            let _ = log("Event listener repeated panic".to_string(), Warn);
            break;
        }

        handle = spawn(|| start_events(options, config).log_err(file!(), line!()));
    }
}

pub fn start_event_listeners(config: &ConfigMutex, state: &mut DaemonState) {
    let config_c = config.clone();
    let options = state.options.clone();
    let handle = spawn(move || start_events(options, config_c).log_err(file!(), line!()));

    let config_c = config.clone();
    let options = state.options.clone();
    spawn(move || keep_alive(handle, options, config_c));
}
