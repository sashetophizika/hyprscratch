use hyprland::data::{Client, Clients, Workspace};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use std::io::prelude::*;
use std::os::unix::net::UnixStream;

fn summon_special(
    title: &String,
    command: &String,
    active_workspace: &Workspace,
    clients_with_title: &[Client],
) -> Result<()> {
    let title = title.clone();
    let special_with_title = clients_with_title
        .iter()
        .filter(|x| x.workspace.id < 0)
        .collect::<Vec<_>>();

    if special_with_title.is_empty() {
        if !clients_with_title.is_empty() {
            hyprland::dispatch!(
                MoveToWorkspace,
                WorkspaceIdentifierWithSpecial::Special(Some(&title)),
                Some(WindowIdentifier::Address(
                    clients_with_title[0].address.clone()
                ))
            )?;

            if clients_with_title[0].workspace.id == active_workspace.id {
                hyprland::dispatch!(ToggleSpecialWorkspace, Some(title))?;
            }
        } else {
            let mut special_cmd = command.clone();
            if command.find('[').is_none() {
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
    command: &String,
    active_workspace: &Workspace,
    clients_with_title: &[Client],
) -> Result<()> {
    if clients_with_title.is_empty() {
        hyprland::dispatch!(Exec, &command)?;
    } else {
        let addr = clients_with_title[0].address.clone();
        if clients_with_title[0].workspace.id != active_workspace.id {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Relative(0),
                Some(WindowIdentifier::Address(addr.clone()))
            )?;
        }
        hyprland::dispatch!(FocusWindow, WindowIdentifier::Address(addr))?;
    }
    Ok(())
}

fn summon(
    title: &String,
    command: &String,
    options: &String,
    active_workspace: &Workspace,
    clients_with_title: &[Client],
) -> Result<()> {
    if options.contains("special") {
        summon_special(title, command, active_workspace, clients_with_title)?;
    } else if !options.contains("hide") {
        summon_normal(command, active_workspace, clients_with_title)?;
    }
    Ok(())
}

fn hide_active(options: &String, titles: String, active_client: &Client) -> Result<()> {
    if !options.contains(&"stack".to_string())
        && active_client.floating
        && titles.contains(&active_client.initial_title)
    {
        hyprland::dispatch!(
            MoveToWorkspaceSilent,
            WorkspaceIdentifierWithSpecial::Id(42),
            Some(WindowIdentifier::Address(active_client.address.clone()))
        )?;
    }
    Ok(())
}

pub fn scratchpad(title: &String, command: &String, options: &String) -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(b"s")?;

    let mut titles = String::new();
    stream.read_to_string(&mut titles)?;

    let active_workspace = Workspace::get_active()?;
    let clients_with_title: Vec<Client> = Clients::get()?
        .into_iter()
        .filter(|x| &x.initial_title == title)
        .collect();

    if options.contains("summon") && !options.contains("special") {
        summon(
            title,
            command,
            options,
            &active_workspace,
            &clients_with_title,
        )?;
        return Ok(());
    }

    if let Some(active_client) = Client::get_active()? {
        let mut clients_on_active = clients_with_title
            .clone()
            .into_iter()
            .filter(|x| x.workspace.id == active_workspace.id)
            .peekable();
        if options.contains(&"special".to_string())
            || (clients_on_active.peek().is_none() && &active_client.initial_title != title)
        {
            hide_active(options, titles, &active_client)?;
            summon(
                title,
                command,
                options,
                &active_workspace,
                &clients_with_title,
            )?;
        } else {
            clients_on_active.for_each(|x| {
                hyprland::dispatch!(
                    MoveToWorkspaceSilent,
                    WorkspaceIdentifierWithSpecial::Id(42),
                    Some(WindowIdentifier::Address(x.address))
                )
                .unwrap()
            });
        }
    } else {
        summon(
            title,
            command,
            options,
            &active_workspace,
            &clients_with_title,
        )?;
    }

    Dispatch::call(DispatchType::BringActiveToTop)?;
    Ok(())
}
