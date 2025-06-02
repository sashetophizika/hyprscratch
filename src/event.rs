use crate::config::Config;
use crate::daemon::DaemonOptions;
use crate::daemon::DaemonState;
use crate::logs::*;
use crate::scratchpad::*;
use crate::utils::*;
use hyprland::data::{Client, Clients, Monitor};
use hyprland::dispatch::*;
use hyprland::event_listener::EventListener;
use hyprland::prelude::*;
use hyprland::Result;
use std::sync::{Arc, Mutex};
use std::thread::*;

type ConfigMutex = Arc<Mutex<Config>>;

fn add_pin(ev: &mut EventListener, config: ConfigMutex) {
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
                .filter(|cl| !is_on_special(cl) && is_known(&conf.pinned_titles, cl))
                .for_each(move_to_current);
        }
    };

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
    let max_restartx = 50;
    let mut restarts = 0;

    loop {
        let _ = handle.join();
        let config = config.clone();
        let options = options.clone();

        restarts += 1;
        if restarts >= max_restartx {
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
