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

fn hyprscratch(args: &[String]) -> Result<()> {
    let title = match args.len() {
        0 | 1 => String::from(""),
        2.. => args[1].clone(),
    };

    for feature in ["hideall", "onstart", "stack"] {
        if args.contains(&feature.to_string()) {
            warn_deprecated(feature)?;
        }
    }

    match title.as_str() {
        "clean" | "no-auto-reload" | "init" | "" => initialize_daemon(args, None, None)?,
        "hideall" | "hide-all" => hide_all()?,
        "get-config" => get_config(None)?,
        "previous" => previous()?,
        "kill-all" => kill_all()?,
        "reload" => reload()?,
        "cycle" => cycle(args.join(" "))?,
        "kill" => kill()?,
        "logs" | "-l" | "--logs" => logs()?,
        "help" | "-h" | "--help" => help(),
        "version" | "-v" | "--version" => println!("hyprscratch v{}", env!("CARGO_PKG_VERSION")),
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
            format!("{} in command '{}'.", err, args[1..].join(" ")),
            "ERROR",
        )
        .unwrap();
    };

    hyprscratch(&args).unwrap_or_else(log_err);
}
