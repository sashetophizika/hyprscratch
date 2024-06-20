use crate::config::Config;
use hyprland::data::Clients;
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;

pub fn move_floating(titles: Vec<String>) {
    if let Ok(clients) = Clients::get() {
        clients
            .iter()
            .filter(|x| x.floating && x.workspace.id != 42 && titles.contains(&x.initial_title))
            .for_each(|x| {
                hyprland::dispatch!(
                    MoveToWorkspaceSilent,
                    WorkspaceIdentifierWithSpecial::Id(42),
                    Some(WindowIdentifier::ProcessId(x.pid as u32))
                )
                .unwrap()
            })
    }
}

pub fn autospawn(config: &mut Config) -> Result<()> {
    let client_titles = Clients::get()?
        .into_iter()
        .map(|x| x.initial_title)
        .collect::<Vec<_>>();

    config
        .commands
        .iter()
        .enumerate()
        .filter(|&(i, _)| {
            config.options[i].contains("onstart") && !client_titles.contains(&config.titles[i])
        })
        .for_each(|(i, x)| {
            let mut cmd = x.clone();
            if x.find('[').is_none() {
                cmd.insert_str(0, "[]");
            }

            if config.options[i].contains("special") {
                hyprland::dispatch!(
                    Exec,
                    &cmd.replacen(
                        '[',
                        &format!("[workspace special:{} silent;", config.titles[i]),
                        1
                    )
                )
                .unwrap()
            } else {
                hyprland::dispatch!(Exec, &cmd.replacen('[', "[workspace 42 silent;", 1)).unwrap()
            }
        });

    Ok(())
}

pub fn shuffle_normal_special(normal_titles: &[String], special_titles: &[String]) -> Result<()> {
    let clients = Clients::get()?;

    for title in special_titles.iter() {
        clients.iter().filter(|x| &x.title == title).for_each(|x| {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Special(Some(title)),
                Some(WindowIdentifier::ProcessId(x.pid as u32))
            )
            .unwrap()
        });
    }

    for title in normal_titles.iter() {
        clients.iter().filter(|x| &x.title == title).for_each(|x| {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Id(42),
                Some(WindowIdentifier::ProcessId(x.pid as u32))
            )
            .unwrap()
        });
    }
    Ok(())
}
