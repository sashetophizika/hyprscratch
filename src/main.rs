mod daemon;
mod scratchpad;
mod extra;
mod utils;
mod config;

use hyprland::Result;
use crate::daemon::initialize;
use crate::scratchpad::scratchpad;
use crate::extra::*;

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let title = match args.len() {
        0 | 1 => String::from(""),
        2.. => args[1].clone(),
    };

    match title.as_str() {
        "clean" | "" => initialize(&args)?,
        "get-config" => get_config()?,
        "hideall" => hideall()?,
        "reload" => reload()?,
        "cycle" => cycle()?,
        "help" => help(),
        _ => {
            if args[2..].is_empty() {
                println!("Unknown command or not enough arguments given for scratchpad.\nTry 'hyprscratch help'.");
            } else {
                scratchpad(&args[1..])?;
            }
        }
    }
    Ok(())
}
