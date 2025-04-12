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
                "reload" => connect_to_sock(socket, "reload", &config.unwrap_or("".into()))?,
                "kill" => connect_to_sock(socket, "kill", "")?,
                "help" => print_help(),
                "logs" => print_logs()?,
                "version" => println!("hyprscratch v{}", env!("CARGO_PKG_VERSION")),
                _ => (),
            }
            return Ok(());
        }
    }

    let req = args.get(1).map_or("", |v| v.as_str());
    let msg = args.get(2).map_or("", |v| v.as_str());
    match req {
        "toggle" | "summon" | "hide" | "cycle" | "hide-all" | "kill-all" | "previous" => {
            connect_to_sock(socket, req, msg)?
        }
        "init" | "eager" | "clean" | "no-auto-reload" | "config" | "socket" => {
            initialize_daemon(args.join(" "), config, socket)
        }
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
                connect_to_sock(socket, "manual", &args[1..].join("^"))?
            }
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let log_err = |err: HyprError| {
        if let HyprError::IoError(e) = err {
            if e.to_string() == "Connection refused (os error 111)" {
                let _ = log("Could not connect to daemon. Is it running?".into(), "WARN");
            }
        } else {
            {
                let _ = log(
                    format!("{}, command: '{}'.", err, args[1..].join(" ")),
                    "WARN",
                );
            }
        }
    };

    hyprscratch(&args).unwrap_or_else(log_err);
}
