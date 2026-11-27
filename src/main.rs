mod config;
mod daemon;
mod error;
mod event;
mod extra;
mod ipc;
mod logs;
mod scratchpad;
mod utils;

use crate::daemon::initialize_daemon;
use crate::error::{Error, Result};
use crate::extra::*;
use crate::logs::*;
use crate::utils::*;
use std::env::{self, args};
use std::fs;
use std::path::PathBuf;

const LUA_HELPER: &str = include_str!("../lua/hyprscratch.lua");

const HYPRSCRATCH_DIR: &str = "/tmp/hyprscratch/";
const DEFAULT_LOGFILE: &str = "/tmp/hyprscratch/hyprscratch.log";
const DEFAULT_SOCKET: &str = "/tmp/hyprscratch/hyprscratch.sock";

const DEFAULT_CONFIG_FILES: &[&str] = &[
    "hypr/hyprscratch.conf",
    "hyprscratch/config.conf",
    "hyprscratch/hyprscratch.conf",
    "hypr/hyprland.lua",
];

const KNOWN_CLI_COMMANDS: &[&str] = &[
    "install-lua",
    "get-config",
    "version",
    "reload",
    "full",
    "help",
    "logs",
    "kill",
];

const KNOWN_COMMANDS: &[&str] = &[
    "no-auto-reload",
    "install-lua",
    "get-config",
    "populate",
    "spotless",
    "hide-all",
    "kill-all",
    "previous",
    "version",
    "attach",
    "reload",
    "toggle",
    "clean",
    "eager",
    "cycle",
    "init",
    "show",
    "hide",
    "menu",
    "kill",
    "logs",
    "help",
];

struct CLIArgs {
    socket: Option<String>,
    config: Option<String>,
    path: Option<String>,
    args: Vec<String>,
}

impl CLIArgs {
    pub fn new(args: &[String]) -> CLIArgs {
        CLIArgs {
            socket: get_flag_arg(args, "socket"),
            config: get_flag_arg(args, "config"),
            path: get_flag_arg(args, "path"),
            args: args.to_vec(),
        }
    }

    pub fn config_path(&self) -> Option<&str> {
        if let Some(conf) = &self.config {
            Some(conf)
        } else if let Some(path) = self.args.get(2) {
            Some(path)
        } else {
            None
        }
    }

    fn dir_or_path(path: &str, ft: &str) -> PathBuf {
        let p = path.trim_end_matches('/');
        if p.ends_with(ft) {
            PathBuf::from(p)
        } else {
            PathBuf::from(format!("{p}/hyprscratch{ft}"))
        }
    }

    pub fn install_path(&self) -> PathBuf {
        if let Some(path) = &self.path {
            Self::dir_or_path(path, ".lua")
        } else if let Some(conf) = &self.config {
            Self::dir_or_path(conf, ".lua")
        } else if let Some(path) = self.args.get(2) {
            Self::dir_or_path(path, ".lua")
        } else {
            let config_home = env::var_os("XDG_CONFIG_HOME")
                .filter(|path| !path.is_empty())
                .map(PathBuf::from)
                .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
                .expect("Neither XDG_CONFIG_HOME nor HOME is set");

            config_home.join("hypr/hyprscratch.lua")
        }
    }
}

fn install_lua(cli_args: CLIArgs) -> Result<()> {
    let target = cli_args.install_path();

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&target, LUA_HELPER)?;

    log(
        format!("Installed Lua helper to {}", target.display()),
        Info,
    );
    Ok(())
}

fn exec_cli_command(command: &str, cli_args: CLIArgs) -> Result<()> {
    let sock = cli_args.socket.as_deref();
    match command {
        "install-lua" => install_lua(cli_args),
        "get-config" => print_config(sock, false),
        "kill" => send_request(sock, "kill", ""),
        "full" => print_full_raw(sock),
        "logs" => print_logs(false),
        "help" => Ok(print_help()),
        "version" => Ok(println!("hyprscratch v{}", env!("CARGO_PKG_VERSION"))),
        "reload" => send_request(sock, "reload", &cli_args.config_path().unwrap_or_default()),
        _ => Ok(()),
    }
}

fn get_cli_command(args: &[String]) -> Option<&str> {
    for arg in args {
        if let Some(flag) = get_flag_name(arg) {
            return Some(flag);
        } else if arg.starts_with('-') {
            log(format!("Unknown flag: {arg}"), Warn);
        }
    }
    None
}

fn send_manual(cli_args: CLIArgs) -> Result<()> {
    if cli_args.args.len() < 3 {
        let msg = format!(
            "Unknown command or not enough arguments for scratchpad in '{}'",
            cli_args.args[1..].join(" ")
        );
        log(msg, Warn);
        return Ok(());
    }
    send_request(
        cli_args.socket.as_deref(),
        "manual",
        &cli_args.args[1..].join("^"),
    )
}

fn exec_main_command(cli_args: CLIArgs) -> Result<()> {
    let get_arg = |i| cli_args.args.get(i).map_or("", String::as_str);
    let (req, msg) = (get_arg(1), get_arg(2));
    match req {
        "init" => initialize_daemon(
            cli_args.args.join(" "),
            cli_args.config,
            cli_args.socket.as_deref(),
        ),
        "menu" => menu(cli_args.socket.as_deref(), msg, get_arg(3))?,
        "" => print_help(),
        _ if KNOWN_COMMANDS.contains(&req) => send_request(cli_args.socket.as_deref(), req, msg)?,
        _ => send_manual(cli_args)?,
    }
    Ok(())
}

fn resolve_command(cli_args: CLIArgs) -> Result<()> {
    let command = get_cli_command(&cli_args.args).map(str::to_string);
    if let Some(cmd) = command {
        return exec_cli_command(&cmd, cli_args);
    }
    exec_main_command(cli_args)
}

fn hyprscratch(args: &[String]) -> Result<()> {
    let deprecated_features = ["summon"];
    for feature in deprecated_features {
        if args.contains(&feature.to_string()) {
            warn_deprecated(feature)?;
        }
    }

    let cli_args = CLIArgs::new(args);
    resolve_command(cli_args)
}

fn catch_err(args: &[String], err: Error) {
    if let Error::Io(e) = &err {
        if e.raw_os_error() == Some(111) {
            log("Could not connect to daemon. Is it running?".into(), Warn);
            return;
        }
    }

    log(
        format!("{}, command: '{}'.", err, args[1..].join(" ")),
        Warn,
    );
}

fn main() {
    let args: Vec<String> = args().collect();
    hyprscratch(&args).unwrap_or_else(|e| catch_err(&args, e));
}
