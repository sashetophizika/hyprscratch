mod config;
mod daemon;
mod event;
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
use std::env::args;

const HYPRSCRATCH_DIR: &str = "/tmp/hyprscratch/";
const DEFAULT_LOGFILE: &str = "/tmp/hyprscratch/hyprscratch.log";
const DEFAULT_SOCKET: &str = "/tmp/hyprscratch/hyprscratch.sock";

const DEFAULT_CONFIG_FILES: [&str; 7] = [
    "hypr/hyprscratch.conf",
    "hypr/hyprscratch.toml",
    "hyprscratch/config.conf",
    "hyprscratch/config.toml",
    "hyprscratch/hyprscratch.conf",
    "hyprscratch/hyprscratch.toml",
    "hypr/hyprland.conf",
];

const KNOWN_FLAGS: [&str; 9] = [
    "get-config",
    "version",
    "reload",
    "config",
    "socket",
    "full",
    "help",
    "logs",
    "kill",
];

const KNOWN_COMMANDS: [&str; 18] = [
    "no-auto-reload",
    "get-config",
    "spotless",
    "hide-all",
    "kill-all",
    "previous",
    "version",
    "reload",
    "toggle",
    "clean",
    "eager",
    "cycle",
    "init",
    "show",
    "hide",
    "kill",
    "logs",
    "help",
];

fn exec_command(command: &str, socket: Option<&str>, config: &Option<String>) -> bool {
    match command {
        "config" | "socket" => return false,
        "get-config" => get_config(socket, false).log_err(file!(), line!()),
        "kill" => send(socket, "kill", "").log_err(file!(), line!()),
        "full" => print_full_raw(socket),
        "help" => print_help(),
        "logs" => print_logs(false).log_err(file!(), line!()),
        "version" => println!("hyprscratch v{}", env!("CARGO_PKG_VERSION")),
        "reload" => {
            send(socket, "reload", &config.clone().unwrap_or("".into())).log_err(file!(), line!())
        }
        _ => (),
    }
    true
}

fn cli_commands(args: &[String], config: &Option<String>, socket: Option<&str>) -> bool {
    for arg in args {
        if let Some(flag) = flag_present(arg, &KNOWN_FLAGS) {
            if exec_command(flag, socket, config) {
                return true;
            }
        } else if arg.starts_with("-") {
            let _ = log(format!("Unknown flag: {arg}"), Warn);
        }
    }
    false
}

fn send_manual(args: &[String], socket: Option<&str>) -> Result<()> {
    if args.len() < 3 {
        let msg = format!(
            "Unknown command or not enough arguments for scratchpad in '{}'",
            args[1..].join(" ")
        );
        log(msg, Warn)?;
        return Ok(());
    }
    send(socket, "manual", &args[1..].join("^"))
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
    if cli_commands(args, &config, socket) {
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

fn catch_err(args: &[String], err: HyprError) {
    if let HyprError::IoError(e) = err {
        if e.to_string() == "Connection refused (os error 111)" {
            let _ = log("Could not connect to daemon. Is it running?".into(), Warn);
        }
    } else {
        {
            let _ = log(
                format!("{}, command: '{}'.", err, args[1..].join(" ")),
                Warn,
            );
        }
    }
}

fn main() {
    let args: Vec<String> = args().collect();
    hyprscratch(&args).unwrap_or_else(|e| catch_err(&args, e));
}
