mod config;
mod daemon;
mod extra;
mod logs;
mod scratchpad;
mod utils;

use crate::daemon::initialize_daemon;
use crate::extra::*;
use crate::utils::*;
use hyprland::shared::HyprError;
use hyprland::Result;
use logs::log;

fn hyprscratch(args: &[String]) -> Result<()> {
    for feature in ["stack"] {
        if args.contains(&feature.to_string()) {
            warn_deprecated(feature)?;
        }
    }

    let config = get_flag_arg(args, "config");
    let sock = get_flag_arg(args, "socket");
    let socket = sock.as_deref();

    for flag in ["help", "logs", "kill", "version", "get-config", "reload"] {
        if let Some(f) = flag_present(args, flag) {
            match f.as_str() {
                "get-config" => get_config(socket)?,
                "reload" => reload(socket, config)?,
                "kill" => kill(socket)?,
                "help" => print_help(),
                "logs" => print_logs()?,
                "version" => println!("hyprscratch v{}", env!("CARGO_PKG_VERSION")),
                _ => (),
            }
            return Ok(());
        }
    }

    let cmd = args.get(1).map_or("", |v| v.as_str());
    match cmd {
        "init" | "eager" | "clean" | "no-auto-reload" | "config" | "socket" => {
            initialize_daemon(args.join(" "), config, socket)
        }
        "toggle" | "summon" | "hide" => call(socket, &args[1..], cmd)?,
        "cycle" => cycle(socket, args.join(" "))?,
        "hide-all" => hide_all(socket)?,
        "kill-all" => kill_all(socket)?,
        "previous" => previous(socket)?,
        "" => {
            log(
                "Initializing the daemon with no arguments is deprecated".to_string(),
                "WARN",
            )?;
            println!("Use 'hyprscratch init'.");
            initialize_daemon(args.join(" "), config, socket);
        }
        s if s.starts_with("-") => {
            if config.is_some() || socket.is_some() {
                initialize_daemon(args.join(" "), config, socket)
            } else {
                log("Unknown flags".to_string(), "WARN")?;
                print_help();
            }
        }
        _ => {
            if args[2..].is_empty() {
                log(
                    format!(
                        "Unknown command or not enough arguments for scratchpad: '{}'.",
                        args[1..].join(" ")
                    ),
                    "WARN",
                )?;
            } else {
                call(socket, args, "toggle")?
            }
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let log_err = |err: HyprError| {
        let _ = log(
            format!("{}, command: '{}'.", err, args[1..].join(" ")),
            "ERROR",
        );
    };

    hyprscratch(&args).unwrap_or_else(log_err);
}
