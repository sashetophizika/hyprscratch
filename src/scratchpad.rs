use hyprland::data::{Client, Clients, Workspace};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use std::io::prelude::*;
use std::os::unix::net::UnixStream;

fn summon_special(args: &[String]) -> Result<()> {
    let title = args[0].clone();
    let special_with_title = &Clients::get()?
        .into_iter()
        .filter(|x| x.initial_title == title && x.workspace.id < 0)
        .collect::<Vec<_>>();

    if special_with_title.is_empty() {
        let cmd = args[1].replacen('[', &format!("[workspace special:{title}; "), 1);
        hyprland::dispatch!(Exec, &cmd)?;
    } else {
        hyprland::dispatch!(ToggleSpecialWorkspace, Some(title))?;
    }
    Ok(())
}

fn summon_normal(args: &[String]) -> Result<()> {
    let clients_with_title = &Clients::get()?
        .into_iter()
        .filter(|x| x.initial_title == args[0])
        .collect::<Vec<_>>();

    if clients_with_title.is_empty() {
        hyprland::dispatch!(Exec, &args[1])?;
    } else {
        let pid = clients_with_title[0].pid as u32;
        if clients_with_title[0].workspace.id == Workspace::get_active()?.id {
            hyprland::dispatch!(FocusWindow, WindowIdentifier::ProcessId(pid))?;
        } else {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Relative(0),
                Some(WindowIdentifier::ProcessId(pid))
            )?;
            hyprland::dispatch!(FocusWindow, WindowIdentifier::ProcessId(pid))?;
        }
    }
    Ok(())
}

pub fn scratchpad(args: &[String]) -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(b"\0")?;

    let mut titles = String::new();
    stream.read_to_string(&mut titles)?;
    if args[2..].contains(&"special".to_string()) {
        summon_special(args)?;
        return Ok(());
    }

    let active_client = Client::get_active()?;
    match active_client {
        Some(active_client) => {
            let mut clients_with_title = Clients::get()?
                .into_iter()
                .filter(|x| {
                    x.initial_title == args[0]
                        && x.workspace.id == Workspace::get_active().unwrap().id
                })
                .peekable();

            if active_client.initial_title == args[0]
                || (!active_client.floating && clients_with_title.peek().is_some())
            {
                clients_with_title.for_each(|x| {
                    hyprland::dispatch!(
                        MoveToWorkspaceSilent,
                        WorkspaceIdentifierWithSpecial::Id(42),
                        Some(WindowIdentifier::ProcessId(x.pid as u32))
                    )
                    .unwrap()
                });
            } else {
                summon_normal(args)?;

                if !args[2..].contains(&"stack".to_string())
                    && active_client.floating
                    && titles.contains(&active_client.initial_title)
                {
                    hyprland::dispatch!(
                        MoveToWorkspaceSilent,
                        WorkspaceIdentifierWithSpecial::Id(42),
                        Some(WindowIdentifier::ProcessId(active_client.pid as u32))
                    )?;
                }
            }
        }
        None => summon_normal(args)?,
    }

    Dispatch::call(DispatchType::BringActiveToTop)?;
    Ok(())
}
