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

fn get_flag_arg(args: &[String], long: &str, short: &str) -> Option<String> {
    if let Some(ci) = args.into_iter().position(|x| x == long || x == short) {
        args.get(ci + 1).cloned()
    } else {
        None
    }
}

fn hyprscratch(args: &[String]) -> Result<()> {
    for feature in ["hideall", "onstart", "stack"] {
        if args.contains(&feature.to_string()) {
            warn_deprecated(feature)?;
        }
    }

    let flag_present = |long, short| args.iter().any(|x| x == long || x == short);
    if flag_present("--help", "-h") {
        help();
        return Ok(());
    }

    if flag_present("--version", "-v") {
        println!("hyprscratch v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let config = get_flag_arg(args, "--config", "-c");
    let socket = get_flag_arg(args, "--socket", "-s");

    match args.get(1).map_or("", |v| v.as_str()) {
        "clean" | "no-auto-reload" | "" => initialize_daemon(args, config, socket.as_deref())?,
        "hideall" | "hide-all" => hide_all()?,
        "get-config" => get_config(config)?,
        "previous" => previous()?,
        "kill-all" => kill_all()?,
        "reload" => reload()?,
        "cycle" => cycle(args.join(" "))?,
        "kill" => kill()?,
        "help" => help(),
        "logs" | "-l" | "--logs" => logs()?,
        "version" => println!("hyprscratch v{}", env!("CARGO_PKG_VERSION")),
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
