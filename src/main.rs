use hyprland::data::{Client, Clients};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;


fn scratchpad(cmd: &str) -> Result<()> {
    let work42 = &Clients::get()?.filter(|x| x.workspace.id == 42 && x.title == cmd).collect::<Vec<_>>();

    if work42.len() == 0 {
        match cmd {
            "btop" => hyprland::dispatch!(Exec, "[float;size 70% 80%] kitty -e btop ")?,
            "ranger" => hyprland::dispatch!(Exec, "[float;size 70% 80%] kitty -e ranger ")?,
            "pulsemixer" => hyprland::dispatch!(Exec, "[float;size 50% 40%] kitty -e pulsemixer ")?,
            "batock" => hyprland::dispatch!(Exec, "[float;size 50% 80%] kitty --session batock_session --title batock")?,
            "mpd" => hyprland::dispatch!(Exec, "[float;size 80% 80%] kitty --session mpd_session --title mpd")?,
            "thunar" => hyprland::dispatch!(Exec, "[float;size 70% 80%] thunar")?,
            "spotify" => hyprland::dispatch!(Exec, "[float;size 80% 80%] spotify")?,
            _ => ()
        }
    }
    else {
        let addr = work42[0].clone().address;
        hyprland::dispatch!(MoveToWorkspace, WorkspaceIdentifierWithSpecial::Relative(0), Some(WindowIdentifier::Address(addr)))?;
    }

    Ok(())
}

fn main() -> Result<()> {
    let cl = Client::get_active()?.unwrap();
    if cl.floating {
        hyprland::dispatch!(MoveToWorkspaceSilent, WorkspaceIdentifierWithSpecial::Id(42), None)?;
    }

    let cmd = &std::env::args().collect::<Vec<String>>()[1];
    if &cl.title != cmd {
        scratchpad(cmd)?;
    }
    Ok(())
}
