use hyprland::data::{Client, Clients, Workspace};
use hyprland::event_listener::EventListenerMutable;
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;

fn scratchpad(title: &str, cmd: &str) -> Result<()> {
    let work42 = &Clients::get()?
        .filter(|x| x.title == title)
        .collect::<Vec<_>>();

    if work42.len() == 0 {
        hyprland::dispatch!(Exec, cmd)?;
    } else {
        let addr = work42[0].clone().address;
        if work42[0].workspace.id == Workspace::get_active()?.id {
            hyprland::dispatch!(FocusWindow, WindowIdentifier::Address(addr))?;
            hyprland::dispatch::Dispatch::call(hyprland::dispatch::DispatchType::BringActiveToTop)?;
        } else {
            hyprland::dispatch!(
                MoveToWorkspace,
                WorkspaceIdentifierWithSpecial::Relative(0),
                Some(WindowIdentifier::Address(addr))
            )?;
        }
    }

    Ok(())
}

fn clean() -> Result<()> {
    let mut ev = EventListenerMutable::new();

    ev.add_workspace_change_handler(|_, _| match Clients::get() {
        Ok(clients) => clients
            .filter(|x| x.floating && x.workspace.id != 42)
            .for_each(|x| {
                hyprland::dispatch!(
                    MoveToWorkspaceSilent,
                    WorkspaceIdentifierWithSpecial::Id(42),
                    Some(WindowIdentifier::Title(&x.title))
                )
                .expect(" ");
            }),
        Err(_) => (),
    });
    ev.start_listener()
}

fn main() -> Result<()> {
    let [_, title, cmd @ ..] = &std::env::args().collect::<Vec<String>>()[..] else {panic!("Bad args")};

    if title == "clean" && cmd.len() == 0 {
        clean();
    } else if title == "stack" && cmd.len() == 0 {
        std::env::set_var("HYPRSCRATCH_STACK", "1");
    } else {
        let cl = Client::get_active()?;

        match cl {
            Some(cl) => {
                if cl.floating && std::env::var("HYPRSCRATCH_STACK").expect(" ") != "1" {
                    hyprland::dispatch!(
                        MoveToWorkspaceSilent,
                        WorkspaceIdentifierWithSpecial::Id(42),
                        None
                    )?;
                }

                if &cl.title != title {
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
