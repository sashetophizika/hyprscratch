use chrono::Local;
use std::env::VarError;
use std::fs::{create_dir, File};
use std::io::Write;
use std::path::Path;
use std::process::exit;
use std::sync::LockResult;

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
                        let _ = log(format!("{e} at {file}:{line}"), "ERROR");
                        exit(0)
                    }
                }
            }

            fn log_err(self, file: &str, line: u32) {
                if let Err(e) = self {
                    let _ = log(format!("{e} at {file}:{line}"), "WARN");
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
                let _ = log(format!("Function returned None at {file}:{line}"), "ERROR");
                exit(0)
            }
        }
    }

    fn log_err(self, file: &str, line: u32) {
        if self.is_none() {
            let _ = log(format!("Recieved None at {file}:{line}"), "WARN");
        }
    }
}

pub fn log(msg: String, level: &str) -> hyprland::Result<()> {
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
            "{} {level} {msg}\n",
            Local::now().format("%d.%m.%Y %H:%M:%S")
        )
        .as_bytes(),
    )?;

    println!("{msg}");
    if level == "ERROR" {
        if cfg!(debug_assertions) {
            panic!("Fatal");
        } else {
            exit(0);
        }
    }

    Ok(())
}
