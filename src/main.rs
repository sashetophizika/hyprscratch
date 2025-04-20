mod config;
mod daemon;
mod extra;
mod logs;
mod scratchpad;
mod utils;

use crate::daemon::initialize_daemon;
use crate::extra::*;
use crate::logs::*;
use crate::utils::*;
use hyprland::shared::HyprError;
use hyprland::Result;

fn cli_commands(args: &[String], config: Option<String>, socket: Option<&str>) -> bool {
    let known_flags = [
        "get-config",
        "version",
        "reload",
        "config",
        "socket",
        "help",
        "logs",
        "kill",
    ];

    for arg in args {
        if let Some(f) = flag_present(arg, &known_flags) {
            match f {
                "config" | "socket" => continue,
                "get-config" => get_config(socket).log_err(file!(), line!()),
                "kill" => send(socket, "kill", "").log_err(file!(), line!()),
                "help" => print_help(),
                "logs" => print_logs().log_err(file!(), line!()),
                "version" => println!("hyprscratch v{}", env!("CARGO_PKG_VERSION")),
                "reload" => {
                    send(socket, "reload", &config.unwrap_or("".into())).log_err(file!(), line!())
                }
                _ => (),
            }
            return true;
        } else if arg.starts_with("-") {
            let _ = log(format!("Unknown flag: {arg}"), "WARN");
        }
    }
    false
}

fn send_manual(args: &[String], socket: Option<&str>) -> Result<()> {
    if args[2..].is_empty() {
        let msg = format!(
            "Unknown command or not enough arguments for scratchpad in '{}'",
            args[1..].join(" ")
        );
        log(msg, "WARN")?;
    } else {
        send(socket, "manual", &args[1..].join("^"))?
    }
    Ok(())
}

fn main_commands(args: &[String], config: Option<String>, socket: Option<&str>) -> Result<()> {
    let get_arg = |i| args.get(i).map_or("", |x: &String| x.as_str());
    let (req, msg) = (get_arg(1), get_arg(2));
    match req {
        "toggle" | "summon" | "show" | "hide" | "cycle" | "hide-all" | "kill-all" | "previous" => {
            send(socket, req, msg)?
        }
        "init" => initialize_daemon(args.join(" "), config, socket),
        "" => print_help(),
        _ => send_manual(args, socket)?,
    }
    Ok(())
}

fn resolve_command(args: &[String], config: Option<String>, socket: Option<&str>) -> Result<()> {
    if cli_commands(args, config.clone(), socket) {
        return Ok(());
    }
    main_commands(args, config, socket)
}

fn hyprscratch(args: &[String]) -> Result<()> {
    for feature in ["summon"] {
        if args.contains(&feature.to_string()) {
            warn_deprecated(feature)?;
        }
    }

    let config = get_flag_arg(args, "config");
    let sock = get_flag_arg(args, "socket");
    let socket = sock.as_deref();
    resolve_command(args, config, socket)
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
