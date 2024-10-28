mod config;
mod daemon;
mod extra;
mod scratchpad;
mod utils;

use crate::daemon::initialize_daemon;
use crate::extra::*;
use crate::scratchpad::scratchpad;
use chrono::offset::Local;
use hyprland::shared::HyprError;
use hyprland::Result;
use std::fs::File;
use std::io::Write;

pub fn log(msg: String, level: &str) -> Result<()> {
    let mut file = File::options()
        .create(true)
        .read(true)
        .append(true)
        .open("/tmp/hyprscratch/hyprscratch.log")?;

    println!("{msg}");
    file.write_all(
        format!(
            "{} [{level}] {msg}\n",
            Local::now().format("%d.%m.%Y %H:%M:%S")
        )
        .as_bytes(),
    )?;
    Ok(())
}

fn warn_deprecated(feature: &str) -> Result<()> {
    let msg = format!("The '{feature}' feature is deprecated.");
    log(msg, "WARN")?;
    println!("Try 'hyprscratch help' and change your configuration before it is removed.");
    Ok(())
}

fn hyprscratch(args: &[String]) -> Result<()> {
    let title = match args.len() {
        0 | 1 => String::from(""),
        2.. => args[1].clone(),
    };

    for feature in ["hideall", "onstart"] {
        if args.contains(&feature.to_string()) {
            warn_deprecated(feature)?;
        }
    }

    match title.as_str() {
        "clean" | "no-auto-reload" | "init" | "" => initialize_daemon(args, None, None)?,
        "hideall" | "hide-all" => hideall()?,
        "get-config" => get_config(None)?,
        "reload" => reload()?,
        "cycle" => cycle(args.join(" "))?,
        "kill" => kill()?,
        "logs" => logs()?,
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
    let args = std::env::args().collect::<Vec<String>>();
    let log_err = |err: HyprError| {
        log(format!("{}: '{}'", err, args[1..].join(" ")), "ERROR").unwrap();
    };

    hyprscratch(&args).unwrap_or_else(log_err);
}
