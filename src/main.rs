mod config;
mod daemon;
mod extra;
mod logs;
mod scratchpad;
mod utils;

use crate::daemon::initialize_daemon;
use crate::extra::*;
use crate::scratchpad::scratchpad;
use hyprland::shared::HyprError;
use hyprland::Result;
use logs::log;

fn warn_deprecated(feature: &str) -> Result<()> {
    log(format!("The '{feature}' feature is deprecated."), "WARN")?;
    println!("Try 'hyprscratch help' and change your configuration before it is removed.");
    Ok(())
}

fn flag_present(args: &[String], flag: &str) -> Option<String> {
    if flag.is_empty() {
        return None;
    }

    let long = format!("--{flag}");
    let short = format!("-{}", flag.as_bytes()[0] as char);

    if args.iter().any(|x| x == flag || *x == long || *x == short) {
        return Some(flag.to_string());
    }
    None
}

fn get_flag_arg(args: &[String], flag: &str) -> Option<String> {
    if flag.is_empty() {
        return None;
    }

    let long = format!("--{flag}");
    let short = format!("-{}", flag.as_bytes()[0] as char);

    if let Some(ci) = args
        .iter()
        .position(|x| x == flag || *x == long || *x == short)
    {
        return args.get(ci + 1).cloned();
    }
    None
}

fn hyprscratch(args: &[String]) -> Result<()> {
    for feature in ["hideall", "onstart", "stack"] {
        if args.contains(&feature.to_string()) {
            warn_deprecated(feature)?;
        }
    }

    let config = get_flag_arg(args, "config");
    let socket = get_flag_arg(args, "socket");

    for flag in ["help", "logs", "kill", "version", "get-config", "reload"] {
        if let Some(f) = flag_present(args, flag) {
            match f.as_str() {
                "get-config" => get_config(config.clone())?,
                "reload" => reload(socket)?,
                "kill" => kill(socket)?,
                "help" => help(),
                "logs" => logs()?,
                "version" => println!("hyprscratch v{}", env!("CARGO_PKG_VERSION")),
                _ => (),
            }
            return Ok(());
        }
    }

    match args.get(1).map_or("", |v| v.as_str()) {
        "clean" | "no-auto-reload" | "init" => initialize_daemon(args, config, socket.as_deref())?,
        s if s.starts_with("-") => initialize_daemon(args, config, socket.as_deref())?,
        "hideall" | "hide-all" => hide_all(socket)?,
        "previous" => previous(socket)?,
        "kill-all" => kill_all(socket)?,
        "cycle" => cycle(socket, args.join(" "))?,
        "" => {
            log(
                "Initializing the daemon with no arguments is deprecated".to_string(),
                "WARN",
            )?;
            println!("Use 'hyprscratch init'.");
            initialize_daemon(args, config, socket.as_deref())?;
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
                scratchpad(&args[1], &args[2], &args[3..].join(" "))?
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
