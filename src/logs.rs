use chrono::Local;
use core::panic;
use std::env::VarError;
use std::fs::{create_dir, File};
use std::io::Write;
use std::path::Path;
use std::sync::LockResult;

pub trait LogErr<T> {
    fn unwrap_log(self, file: &str, line: u32) -> T;
    fn log_err(self, file: &str, line: u32);
}

impl<T> LogErr<T> for hyprland::Result<T> {
    fn unwrap_log(self, file: &str, line: u32) -> T {
        match self {
            Ok(t) => t,
            Err(err) => {
                let msg = format!("{} at {}:{}", err, file, line);
                log(msg, "ERROR").unwrap();
                panic!()
            }
        }
    }
    fn log_err(self, file: &str, line: u32) {
        if let Err(e) = self {
            let _ = log(format!("{e} at {}:{}", file, line), "WARN");
        }
    }
}

impl<T> LogErr<T> for std::io::Result<T> {
    fn unwrap_log(self, file: &str, line: u32) -> T {
        match self {
            Ok(t) => t,
            Err(err) => {
                let msg = format!("{} at {}:{}", err, file, line);
                log(msg, "ERROR").unwrap();
                panic!()
            }
        }
    }
    fn log_err(self, file: &str, line: u32) {
        if let Err(e) = self {
            let _ = log(format!("{e} at {}:{}", file, line), "WARN");
        }
    }
}

impl<T> LogErr<T> for Result<T, VarError> {
    fn unwrap_log(self, file: &str, line: u32) -> T {
        match self {
            Ok(t) => t,
            Err(err) => {
                let msg = format!("{} at {}:{}", err, file, line);
                log(msg, "ERROR").unwrap();
                panic!()
            }
        }
    }
    fn log_err(self, file: &str, line: u32) {
        if let Err(e) = self {
            let _ = log(format!("{e} at {}:{}", file, line), "WARN");
        }
    }
}

impl<T> LogErr<T> for LockResult<T> {
    fn unwrap_log(self, file: &str, line: u32) -> T {
        match self {
            Ok(t) => t,
            Err(err) => {
                let msg = format!("{} at {}:{}", err, file, line);
                log(msg, "ERROR").unwrap();
                panic!()
            }
        }
    }
    fn log_err(self, file: &str, line: u32) {
        if let Err(e) = self {
            let _ = log(format!("{e} at {}:{}", file, line), "WARN");
        }
    }
}

impl<T> LogErr<T> for Option<T> {
    fn unwrap_log(self, file: &str, line: u32) -> T {
        match self {
            Some(t) => t,
            None => {
                let msg = format!("Function returned None in {} at line:{}", file, line);
                log(msg, "ERROR").unwrap();
                panic!()
            }
        }
    }

    fn log_err(self, file: &str, line: u32) {
        if self.is_none() {
            let _ = log(format!("Recieved None at {}:{}", file, line), "WARN");
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
            "[{}] [{level}] {msg}\n",
            Local::now().format("%d.%m.%Y %H:%M:%S")
        )
        .as_bytes(),
    )?;

    if level == "ERROR" {
        panic!("{msg}");
    } else {
        println!("{msg}");
    }

    Ok(())
}
