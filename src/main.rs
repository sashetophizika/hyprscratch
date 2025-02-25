mod config;
mod daemon;
mod extra;
mod logs;
mod scratchpad;
mod utils;

use crate::daemon::initialize_daemon;
use crate::extra::*;
use crate::scratchpad::scratchpad;
use crate::utils::*;
use hyprland::shared::HyprError;
use hyprland::Result;
use logs::log;

fn hyprscratch(args: &[String]) -> Result<()> {
    for feature in ["hideall", "onstart", "stack"] {
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
                "help" => help(),
                "logs" => print_logs()?,
                "version" => println!("hyprscratch v{}", env!("CARGO_PKG_VERSION")),
                _ => (),
            }
            return Ok(());
        }
    }

    match args.get(1).map_or("", |v| v.as_str()) {
        "clean" | "no-auto-reload" | "config" | "init" => {
            initialize_daemon(args.join(" "), config, socket)?
        }
        "hideall" | "hide-all" => hide_all(socket)?,
        "previous" => previous(socket)?,
        "kill-all" => kill_all(socket)?,
        "cycle" => cycle(socket, args.join(" "))?,
        "toggle" => call(socket, args, "toggle")?,
        "summon" => call(socket, args, "summon")?,
        "hide" => call(socket, args, "hide")?,
        "" => {
            log(
                "Initializing the daemon with no arguments is deprecated".to_string(),
                "WARN",
            )?;
            println!("Use 'hyprscratch init'.");
            initialize_daemon(args.join(" "), config, socket)?;
        }
        s if s.starts_with("-") => {
            log("Unknown flags".to_string(), "Error")?;
            help();
        }
        _ => {
            if args[2..].is_empty() {
                log(
                    format!(
                        "Unknown command or not enough arguments for scratchpad: '{}'.",
                        args[1..].join(" ")
                    ),
                    "ERROR",
                )?;
                println!("Try 'hyprscratch help'.");
            } else {
                scratchpad(&args[1], &args[2], &args[3..].join(" "), None)?
            }
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let log_err = |err: HyprError| {
        log(
            format!("{}, command: '{}'.", err, args[1..].join(" ")),
            "ERROR",
        )
        .unwrap();
    };

    hyprscratch(&args).unwrap_or_else(log_err);
}
