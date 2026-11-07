use crate::error::{Error, Result};
use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

pub mod data;
pub mod dispatchers;

const CONTROL_SOCKET: &str = ".socket.sock";
const EVENT_SOCKET: &str = ".socket2.sock";

fn runtime_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(path));
    }

    if let Ok(uid) = env::var("UID") {
        return Ok(PathBuf::from(format!("/run/user/{uid}")));
    }

    Err(Error::Ipc(
        "Could not find XDG_RUNTIME_DIR or UID".to_string(),
    ))
}

fn instance_dir() -> Result<PathBuf> {
    let signature = env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .map_err(|_| Error::Ipc("Could not find HYPRLAND_INSTANCE_SIGNATURE".to_string()))?;

    Ok(runtime_dir()?.join("hypr").join(signature))
}

pub fn socket_path() -> Result<PathBuf> {
    Ok(instance_dir()?.join(CONTROL_SOCKET))
}

pub fn event_socket_path() -> Result<PathBuf> {
    Ok(instance_dir()?.join(EVENT_SOCKET))
}

pub fn request(command: &str) -> Result<String> {
    let mut stream = UnixStream::connect(socket_path()?)?;
    stream.write_all(command.as_bytes())?;
    stream.flush()?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    if response.trim_start().starts_with("error:") {
        return Err(Error::Ipc(response.trim().to_string()));
    }

    Ok(response)
}

pub fn eval(expression: &str) -> Result<String> {
    request(&format!("eval {expression}"))
}
