use chrono::Local;
use core::panic;
use std::env::VarError;
use std::fs::File;
use std::io::Write;
use std::sync::LockResult;

pub trait LogErr<T> {
    fn unwrap_log(self, file: &str, line: u32) -> T;
}

impl<T> LogErr<T> for hyprland::Result<T> {
    fn unwrap_log(self, file: &str, line: u32) -> T {
        match self {
            Ok(t) => t,
            Err(err) => {
                let msg = format!("{} at {}:{}", err, file, line);
                log(msg.clone(), "ERROR").unwrap();
                panic!()
            }
        }
    }
}

impl<T> LogErr<T> for Result<T, VarError> {
    fn unwrap_log(self, file: &str, line: u32) -> T {
        match self {
            Ok(t) => t,
            Err(err) => {
                let msg = format!("{} at {}:{}", err, file, line);
                log(msg.clone(), "ERROR").unwrap();
                panic!()
            }
        }
    }
}

impl<T> LogErr<T> for LockResult<T> {
    fn unwrap_log(self, file: &str, line: u32) -> T {
        match self {
            Ok(t) => t,
            Err(err) => {
                let msg = format!("{} at {}:{}", err, file, line);
                log(msg.clone(), "ERROR").unwrap();
                panic!()
            }
        }
    }
}

impl<T> LogErr<T> for Option<T> {
    fn unwrap_log(self, file: &str, line: u32) -> T {
        match self {
            Some(t) => t,
            None => {
                let msg = format!("Function returned None in {} at line:{}", file, line);
                log(msg.clone(), "ERROR").unwrap();
                panic!()
            }
        }
    }
}

pub fn log(msg: String, level: &str) -> hyprland::Result<()> {
    let mut file = File::options()
        .create(true)
        .read(true)
        .append(true)
        .open("/tmp/hyprscratch/hyprscratch.log")?;

    println!("{msg}");
    file.write_all(
        format!(
            "[{}] [{level}] {msg}\n",
            Local::now().format("%d.%m.%Y %H:%M:%S")
        )
        .as_bytes(),
    )?;
    Ok(())
}
