use chrono::Local;
use std::env::VarError;
use std::fs::{create_dir, File};
use std::io::Write;
use std::path::Path;
use std::process::exit;
use std::sync::LockResult;

#[derive(PartialEq)]
pub enum LogLevel {
    INFO,
    WARN,
    ERROR,
    DEBUG,
}

impl LogLevel {
    fn as_str<'a>(&self) -> &'a str {
        match self {
            Self::INFO => "INFO",
            Self::WARN => "WARN",
            Self::ERROR => "ERROR",
            Self::DEBUG => "DEBUG",
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
                        let _ = log(format!("{e} at {file}:{line}"), LogLevel::ERROR);
                        exit(1)
                    }
                }
            }

            fn log_err(self, file: &str, line: u32) {
                if let Err(e) = self {
                    let _ = log(format!("{e} at {file}:{line}"), LogLevel::WARN);
                }
            }
        })+
    }
}

impl_logerr!(hyprland::Result<T>, std::io::Result<T>, 
    Result<T, VarError>, LockResult<T>);

impl<T> LogErr<T> for Option<T> {
    fn unwrap_log(self, file: &str, line: u32) -> T {
        match self {
            Some(t) => t,
            None => {
                let _ = log(
                    format!("Function returned None at {file}:{line}"),
                    LogLevel::ERROR,
                );
                exit(1)
            }
        }
    }

    fn log_err(self, file: &str, line: u32) {
        if self.is_none() {
            let _ = log(format!("Received None at {file}:{line}"), LogLevel::WARN);
        }
    }
}

pub fn log(msg: String, level: LogLevel) -> hyprland::Result<()> {
    let temp_dir = Path::new("/tmp/hyprscratch/");
    if !temp_dir.exists() {
        create_dir(temp_dir)?;
    }

    let mut file = File::options()
        .create(true)
        .read(true)
        .append(true)
        .open("/tmp/hyprscratch/hyprscratch.log")?;

    file.write_all(
        format!(
            "{} {} {msg}\n",
            Local::now().format("%d.%m.%Y %H:%M:%S"),
            level.as_str()
        )
        .as_bytes(),
    )?;

    println!("{msg}");
    if level == LogLevel::ERROR {
        if cfg!(debug_assertions) {
            panic!("Fatal");
        } else {
            exit(1);
        }
    }

    Ok(())
}
