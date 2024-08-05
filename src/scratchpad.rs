use hyprland::data::{Client, Clients, Workspace};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use std::io::prelude::*;
use std::os::unix::net::UnixStream;

fn summon_special(
    args: &[String],
    active_workspace: &Workspace,
    clients_with_title: &[Client],
) -> Result<()> {
    let title = args[0].clone();
    let special_with_title = clients_with_title
        .iter()
        .filter(|x| x.workspace.id < 0)
        .collect::<Vec<_>>();

    if special_with_title.is_empty() {
        if !clients_with_title.is_empty() {
            hyprland::dispatch!(
                MoveToWorkspace,
                WorkspaceIdentifierWithSpecial::Special(Some(&title)),
                Some(WindowIdentifier::ProcessId(
                    clients_with_title[0].pid as u32
                ))
            )?;

            if clients_with_title[0].workspace.id == active_workspace.id {
                hyprland::dispatch!(ToggleSpecialWorkspace, Some(title))?;
            }
        } else {
            let mut special_cmd = args[1].clone();
            if args[1].find('[').is_none() {
                special_cmd.insert_str(0, "[]");
            }

            let cmd = special_cmd.replacen('[', &format!("[workspace special:{title}; "), 1);
            hyprland::dispatch!(Exec, &cmd)?;
        }
    } else {
        hyprland::dispatch!(ToggleSpecialWorkspace, Some(title))?;
    }
    Ok(())
}

fn summon_normal(
    args: &[String],
    active_workspace: &Workspace,
    clients_with_title: &[Client],
) -> Result<()> {
    if clients_with_title.is_empty() {
        hyprland::dispatch!(Exec, &args[1])?;
    } else {
        let pid = clients_with_title[0].pid as u32;
        if clients_with_title[0].workspace.id != active_workspace.id {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Relative(0),
                Some(WindowIdentifier::ProcessId(pid))
            )?;
        }
        hyprland::dispatch!(FocusWindow, WindowIdentifier::ProcessId(pid))?;
    }
    Ok(())
}

fn summon(
    args: &[String],
    active_workspace: &Workspace,
    clients_with_title: &[Client],
) -> Result<()> {
    if args[2..].contains(&"special".to_string()) {
        summon_special(args, active_workspace, clients_with_title)?;
    } else if !args[2..].contains(&"hide".to_string()) {
        summon_normal(args, active_workspace, clients_with_title)?;
    }
    Ok(())
}

fn hide_active(args: &[String], titles: String, active_client: &Client) -> Result<()> {
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
    Ok(())
}

pub fn scratchpad(args: &[String]) -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(b"s")?;

    let mut titles = String::new();
    stream.read_to_string(&mut titles)?;

    let active_workspace = Workspace::get_active()?;
    let clients_with_title: Vec<Client> = Clients::get()?
        .into_iter()
        .filter(|x| x.initial_title == args[0])
        .collect();

    if args[2..].contains(&"summon".to_string()) && !args[2..].contains(&"special".to_string()) {
        summon(args, &active_workspace, &clients_with_title)?;
        return Ok(());
    }

    if let Some(active_client) = Client::get_active()? {
        let mut clients_on_active = clients_with_title
            .clone()
            .into_iter()
            .filter(|x| x.workspace.id == active_workspace.id)
            .peekable();

        if args[2..].contains(&"special".to_string())
            || (clients_on_active.peek().is_none() && active_client.initial_title != args[0])
        {
            hide_active(args, titles, &active_client)?;
            summon(args, &active_workspace, &clients_with_title)?;
        } else {
            clients_on_active.for_each(|x| {
                hyprland::dispatch!(
                    MoveToWorkspaceSilent,
                    WorkspaceIdentifierWithSpecial::Id(42),
                    Some(WindowIdentifier::ProcessId(x.pid as u32))
                )
                .unwrap()
            });
        }
    } else {
        summon(args, &active_workspace, &clients_with_title)?;
    }

    Dispatch::call(DispatchType::BringActiveToTop)?;
    Ok(())
}
