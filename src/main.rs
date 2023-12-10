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
        let pid = work42[0].clone().pid as u32;
        if work42[0].workspace.id == Workspace::get_active()?.id {
            hyprland::dispatch!(FocusWindow, WindowIdentifier::ProcessId(pid))?;
        } else {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Relative(0),
                Some(WindowIdentifier::ProcessId(pid))
            )?;
            hyprland::dispatch!(FocusWindow, WindowIdentifier::ProcessId(pid))?;
        }
        Dispatch::call(hyprland::dispatch::DispatchType::BringActiveToTop)?;
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
                    Some(WindowIdentifier::ProcessId(x.pid as u32))
                )
                .expect(" ");
            })
    }
}

fn clean(opt: Option<&String>) -> Result<()> {
    let re_simple = Regex::new(r"hyprscratch \w+.+").unwrap();
    let re_quotes = Regex::new("hyprscratch \".+\".+").unwrap();
    static mut BUF: String = String::new();

    //It is unsafe because I need a mutable reference to a static variable
    unsafe {
        std::fs::File::open(format!(
            "{}/.config/hypr/hyprland.conf",
            std::env::var("HOME").unwrap()
        ))?
        .read_to_string(&mut BUF)?;

        let mut scratchpads = re_simple
            .find_iter(&BUF)
            .filter(|x| !x.as_str().contains("shiny"))
            .map(|x| x.as_str().split(' ').nth(1).unwrap())
            .collect::<Vec<_>>();

        scratchpads.splice(0..0, re_quotes
            .find_iter(&BUF)
            .filter(|x| !x.as_str().contains("shiny"))
            .map(|x| x.as_str().split('"').nth(1).unwrap())
            .collect::<Vec<_>>());

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
                if (cl.floating && !cmd.contains(&String::from("stack"))) || &cl.initial_title == title {
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
