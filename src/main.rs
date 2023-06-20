use hyprland::data::{Client, Clients};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;

fn scratchpad(title: &str, cmd: &str) -> Result<()> {
    let work42 = &Clients::get()?
        .filter(|x| x.workspace.id == 42 && x.title == title)
        .collect::<Vec<_>>();

    if work42.len() == 0 {
        hyprland::dispatch!(Exec, cmd)?;
    } else {
        let addr = work42[0].clone().address;
        hyprland::dispatch!(
            MoveToWorkspace,
            WorkspaceIdentifierWithSpecial::Relative(0),
            Some(WindowIdentifier::Address(addr))
        )?;
    }

    Ok(())
}

fn main() -> Result<()> {
    let [_, title, cmd] = &std::env::args().collect::<Vec<String>>()[..] else {panic!("Bad args")};
    let cl = Client::get_active()?;

    match cl {
        Some(cl) => {
            if cl.floating {
                hyprland::dispatch!(
                    MoveToWorkspaceSilent,
                    WorkspaceIdentifierWithSpecial::Id(42),
                    None
                )?;
            }

            if &cl.title != title {
                scratchpad(title, cmd)?;
            }
        }
        None => {
            scratchpad(title, cmd)?;
        }
    }
    Ok(())
}
