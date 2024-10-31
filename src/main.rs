mod config;
mod daemon;
mod extra;
mod scratchpad;
mod utils;

use crate::daemon::initialize_daemon;
use crate::extra::*;
use crate::scratchpad::scratchpad;
use hyprland::Result;

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let title = match args.len() {
        0 | 1 => String::from(""),
        2.. => args[1].clone(),
    };

    match title.as_str() {
        "clean" | "" => initialize_daemon(&args, None, None)?,
        "get-config" => get_config(None)?,
        "hideall" => hideall()?,
        "reload" => reload()?,
        "cycle" => cycle(args.join(" "))?,
        "kill" => kill()?,
        "help" => help(),
        "version" => println!("hyprscratch v{}", env!("CARGO_PKG_VERSION")),
        _ => {
            if args[2..].is_empty() {
                println!("Unknown command or not enough arguments given for scratchpad.\nTry 'hyprscratch help'.");
            } else {
                scratchpad(&args[1], &args[2], &args[3..].join(" "))?;
            }
        }
    }
    Ok(())
}
