use hyprland::data::{Client, Clients, Workspace};
use hyprland::dispatch::*;
use hyprland::event_listener::EventListenerMutable;
use hyprland::prelude::*;
use hyprland::Result;
use regex::Regex;
use std::io::prelude::*;

fn scratchpad(title: &str, cmd: &str) -> Result<()> {
    let work42 = &Clients::get()?
        .filter(|x| x.initial_title == title)
        .collect::<Vec<_>>();

    if work42.is_empty() {
        hyprland::dispatch!(Exec, cmd)?;
    } else {
        let addr = work42[0].clone().address;
        if work42[0].workspace.id == Workspace::get_active()?.id {
            hyprland::dispatch!(FocusWindow, WindowIdentifier::Address(addr))?;
        } else {
            hyprland::dispatch!(
                MoveToWorkspace,
                WorkspaceIdentifierWithSpecial::Relative(0),
                Some(WindowIdentifier::Address(addr))
            )?;
        }
        hyprland::dispatch::Dispatch::call(hyprland::dispatch::DispatchType::BringActiveToTop)?;
    }

    Ok(())
}

fn move_floating(scratchpads: &[&str]) {
    if let Ok(clients) = Clients::get() {
        clients
            .filter(|x| x.floating && x.workspace.id != 42 && scratchpads.contains(&&x.initial_title[..]))
            .for_each(|x| {
                hyprland::dispatch!(
                    MoveToWorkspaceSilent,
                    WorkspaceIdentifierWithSpecial::Id(42),
                    Some(WindowIdentifier::Title(&x.initial_title))
                )
                .expect(" ");
            })
    }
}

fn clean(opt: Option<&String>) -> Result<()> {
    let re = Regex::new(r"hyprscratch \w+").unwrap();
    static mut BUF: String = String::new();

    //It is unsafe because I need a mutable reference to a static variable
    unsafe {
        std::fs::File::open(format!(
            "{}/.config/hypr/hyprland.conf",
            std::env::var("HOME").unwrap()
        ))?
        .read_to_string(&mut BUF)?;

        let scratchpads = re
            .find_iter(&BUF)
            .map(|x| x.as_str().split(' ').last().unwrap())
            .collect::<Vec<_>>();
        let scratchpads2 = scratchpads.clone();
        let mut ev = EventListenerMutable::new();

        ev.add_workspace_change_handler(move |_, _| {
            move_floating(&scratchpads);
        });

        let _spotless = String::from("spotless");
        if let Some(_spotless) = opt {
            ev.add_active_window_change_handler(move |_, _| {
                if let Some(cl) = Client::get_active().unwrap() {
                    if !cl.floating {
                        move_floating(&scratchpads2);
                    }
                }
            });
        }

        ev.start_listener()
    }
}

fn hideall() -> Result<()> {
    Clients::get()?.filter(|x| x.floating).for_each(|x| {
        hyprland::dispatch!(
            MoveToWorkspaceSilent,
            WorkspaceIdentifierWithSpecial::Id(42),
            Some(WindowIdentifier::Address(x.address))
        )
        .unwrap()
    });
    Ok(())
}

fn main() -> Result<()> {
    let [_, title, cmd @ ..] = &std::env::args().collect::<Vec<String>>()[..] else {panic!("Bad args")};

    if title == "clean" {
        clean(cmd.get(0)).unwrap();
    } else if title == "hideall" && cmd.is_empty() {
        hideall().unwrap();
    } else {
        let cl = Client::get_active()?;

        match cl {
            Some(cl) => {
                if (cl.floating && !(cmd.len() == 2 && &cmd[1] == "stack")) || &cl.initial_title == title {
                    hyprland::dispatch!(
                        MoveToWorkspaceSilent,
                        WorkspaceIdentifierWithSpecial::Id(42),
                        None
                    )?;
                }

                if &cl.initial_title != title {
                    scratchpad(title, &cmd[0])?;
                }
            }
            None => {
                scratchpad(title, &cmd[0])?;
            }
        }
    }

    Ok(())
}
