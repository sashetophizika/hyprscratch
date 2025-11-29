use crate::{DEFAULT_LOGFILE, HYPRSCRATCH_DIR};
use chrono::Local;
use std::env::VarError;
use std::fs::{create_dir, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::exit;
use std::sync::LockResult;

pub use LogLevel::*;
#[derive(PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

impl LogLevel {
    fn as_str<'a>(&self) -> &'a str {
        match self {
            Info => "INFO",
            Warn => "WARN",
            Error => "ERROR",
            Debug => "DEBUG",
        }
    }
}

pub trait LogErr<T> {
    fn unwrap_log(self, file: &str, line: u32) -> T;
    fn log_err(self, file: &str, line: u32);
}

macro_rules! impl_logerr {
    ($($t:ty),+) => {
        $(impl<T> LogErr<T> for $t {
            fn unwrap_log(self, file: &str, line: u32) -> T {
                match self {
                    Ok(t) => t,
                    Err(e) => {
                        let _ = log(format!("{e} at {file}:{line}"), Error);
                        exit(1)
                    }
                }
            }

            fn log_err(self, file: &str, line: u32) {
                if let Err(e) = self {
                    let _ = log(format!("{e} at {file}:{line}"), Warn);
                }
            }
        })+
    }
}

impl_logerr!(hyprland::Result<T>, io::Result<T>, notify::Result<T>,
    Result<T, VarError>, LockResult<T>);

impl<T> LogErr<T> for Option<T> {
    fn unwrap_log(self, file: &str, line: u32) -> T {
        if let Some(t) = self { t } else {
            let _ = log(format!("Function returned None at {file}:{line}"), Error);
            exit(1)
        }
    }

    fn log_err(self, file: &str, line: u32) {
        if self.is_none() {
            let _ = log(format!("Received None at {file}:{line}"), Warn);
        }
    }
}

fn get_log_file() -> io::Result<File> {
    let temp_dir = Path::new(HYPRSCRATCH_DIR);
    if !temp_dir.exists() {
        create_dir(temp_dir)?;
    }

    File::options()
        .create(true)
        .read(true)
        .append(true)
        .open(DEFAULT_LOGFILE)
}

fn write_msg(msg: &String, level: &LogLevel) -> io::Result<()> {
    get_log_file()?.write_all(
        format!(
            "{} {} {msg}\n",
            Local::now().format("%d.%m.%Y %H:%M:%S"),
            level.as_str()
        )
        .as_bytes(),
    )
}

fn exit_on_err(level: LogLevel) {
    if level == Error {
        if cfg!(debug_assertions) {
            panic!("Fatal");
        } else {
            exit(1);
        }
    }
}

pub fn log(msg: String, level: LogLevel) -> hyprland::Result<()> {
    write_msg(&msg, &level)?;
    println!("{msg}");
    exit_on_err(level);
    Ok(())
}
