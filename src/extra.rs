use hyprland::Result;
use std::io::prelude::*;
use std::os::unix::net::UnixStream;
use crate::scratchpad::scratchpad;

pub fn cycle() -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(b"c")?;

    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;
    stream.flush()?;

    let args: Vec<String> = buf.split(':').map(|x| x.to_owned()).collect();
    scratchpad(&args)?;
    Ok(())
}

pub fn reload() -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(b"r")?;
    Ok(())
}
