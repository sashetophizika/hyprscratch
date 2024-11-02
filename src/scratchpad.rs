use crate::utils::log;
use hyprland::data::{Client, Clients, Workspace};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use std::io::prelude::*;
use std::os::unix::net::UnixStream;

fn summon_special(
    title: &str,
    command: &str,
    active_workspace: &Workspace,
    clients_with_title: &[Client],
) -> Result<()> {
    let special_with_title = clients_with_title
        .iter()
        .filter(|x| x.workspace.id < 0)
        .collect::<Vec<_>>();

    if special_with_title.is_empty() {
        if !clients_with_title.is_empty() {
            hyprland::dispatch!(
                MoveToWorkspace,
                WorkspaceIdentifierWithSpecial::Special(Some(title)),
                Some(WindowIdentifier::Address(
                    clients_with_title[0].address.clone()
                ))
            )?;

            if clients_with_title[0].workspace.id == active_workspace.id {
                hyprland::dispatch!(ToggleSpecialWorkspace, Some(title.to_string()))?;
            }
        } else {
            let mut special_cmd = command.to_string();
            if command.find('[').is_none() {
                special_cmd.insert_str(0, "[]");
            }

            let cmd = special_cmd.replacen('[', &format!("[workspace special:{title}; "), 1);
            hyprland::dispatch!(Exec, &cmd)?;
        }
    } else {
        hyprland::dispatch!(ToggleSpecialWorkspace, Some(title.to_string()))?;
    }
    Ok(())
}

fn summon_normal(
    command: &str,
    options: &str,
    active_workspace: &Workspace,
    clients_with_title: &[Client],
) -> Result<()> {
    if clients_with_title.is_empty() {
        hyprland::dispatch!(Exec, &command)?;
    } else {
        for client in clients_with_title
            .into_iter()
            .filter(|x| x.workspace.id != active_workspace.id)
        {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Relative(0),
                Some(WindowIdentifier::Address(client.address.clone()))
            )
            .unwrap_or_else(|err| log(err.to_string(), "ERROR").unwrap());
            if !options.contains("poly") {
                break;
            }
        }

        hyprland::dispatch!(
            FocusWindow,
            WindowIdentifier::Address(clients_with_title[0].address.clone())
        )?;
    }
    Ok(())
}

fn summon(
    title: &str,
    command: &str,
    options: &str,
    active_workspace: &Workspace,
    clients_with_title: &[Client],
) -> Result<()> {
    if options.contains("special") {
        summon_special(title, command, active_workspace, clients_with_title)?;
    } else if !options.contains("hide") {
        summon_normal(command, options, active_workspace, clients_with_title)?;
    }
    Ok(())
}

fn hide_active(options: &str, titles: String, active_client: &Client) -> Result<()> {
    if !options.contains(&"cover".to_string())
        && !options.contains(&"stack".to_string())
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

fn remove_temp_shiny(title: &str, options: &str) -> Result<()> {
    if !options.contains("shiny") {
        let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
        stream.write_all(format!("r?{title}").as_bytes())?;
        stream.shutdown(std::net::Shutdown::Write)?;
    }
    Ok(())
}

pub fn scratchpad(title: &str, command: &str, options: &str) -> Result<()> {
    let mut stream = UnixStream::connect("/tmp/hyprscratch/hyprscratch.sock")?;
    stream.write_all(format!("s?{title}").as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut titles = String::new();
    stream.read_to_string(&mut titles)?;

    let active_workspace = Workspace::get_active()?;
    let clients_with_title: Vec<Client> = Clients::get()?
        .into_iter()
        .filter(|x| x.initial_title == title)
        .collect();

    if !options.contains("special") && options.contains("summon") {
        summon_normal(command, options, &active_workspace, &clients_with_title)?;
        return Ok(());
    }

    if let Some(active_client) = Client::get_active()? {
        let mut clients_on_active = clients_with_title
            .clone()
            .into_iter()
            .filter(|x| x.workspace.id == active_workspace.id)
            .peekable();

        if options.contains(&"special".to_string()) || clients_on_active.peek().is_none() {
            hide_active(options, titles, &active_client)?;
            summon(
                title,
                command,
                options,
                &active_workspace,
                &clients_with_title,
            )?;
        } else if active_client.floating
            && active_client.initial_title != title
            && clients_on_active.peek().is_some()
        {
            hyprland::dispatch!(
                FocusWindow,
                WindowIdentifier::Address(clients_on_active.peek().unwrap().address.clone())
            )?;
            Dispatch::call(DispatchType::BringActiveToTop)?;
        } else {
            clients_on_active.for_each(|x| {
                hyprland::dispatch!(
                    MoveToWorkspaceSilent,
                    WorkspaceIdentifierWithSpecial::Id(42),
                    Some(WindowIdentifier::Address(x.address))
                )
                .unwrap_or_else(|err| log(err.to_string(), "ERROR").unwrap())
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
    remove_temp_shiny(title, options)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    struct TestResources {
        title: String,
        command: String,
    }

    impl Drop for TestResources {
        fn drop(&mut self) {
            hyprland::dispatch!(CloseWindow, WindowIdentifier::Title(&self.title)).unwrap();
            sleep(Duration::from_millis(1000));
        }
    }

    fn test_summon_normal() {
        let resources = TestResources {
            title: "test_normal_scratchpad".to_string(),
            command: "[float;size 30% 30%] kitty --title test_normal_scratchpad".to_string(),
        };

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            false
        );

        let clients_with_title: Vec<Client> = Vec::new();
        summon_normal(
            &resources.command,
            "",
            &Workspace::get_active().unwrap(),
            &clients_with_title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);

        hide_active("", resources.title.clone(), &active_client).unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            true
        );
        assert_ne!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );

        let clients_with_title: Vec<Client> = Clients::get()
            .unwrap()
            .into_iter()
            .filter(|x| x.initial_title == resources.title)
            .collect();

        let active_workspace = Workspace::get_active().unwrap();
        summon_normal(
            &resources.command,
            "",
            &active_workspace,
            &clients_with_title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(Workspace::get_active().unwrap().id, active_workspace.id);
        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );
    }

    fn test_summon_special() {
        let resources = TestResources {
            title: "test_special_scratchpad".to_string(),
            command: "[float;size 30% 30%] kitty --title test_special_scratchpad".to_string(),
        };

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            false
        );

        let clients_with_title: Vec<Client> = Vec::new();
        summon_special(
            &resources.title,
            &resources.command,
            &Workspace::get_active().unwrap(),
            &clients_with_title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );

        let clients_with_title: Vec<Client> = Clients::get()
            .unwrap()
            .into_iter()
            .filter(|x| x.initial_title == resources.title)
            .collect();
        summon_special(
            &resources.title,
            &resources.command,
            &Workspace::get_active().unwrap(),
            &clients_with_title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            true
        );
        assert_ne!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );

        let active_workspace = Workspace::get_active().unwrap();
        summon_special(
            &resources.title,
            &resources.command,
            &active_workspace,
            &clients_with_title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(Workspace::get_active().unwrap().id, active_workspace.id);
        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );
    }

    fn test_cover() {
        let resources = TestResources {
            title: "test_cover".to_string(),
            command: "[float;size 30% 30%] kitty --title test_cover".to_string(),
        };

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            false
        );

        let clients_with_title: Vec<Client> = Vec::new();
        summon_normal(
            &resources.command,
            "",
            &Workspace::get_active().unwrap(),
            &clients_with_title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);

        hide_active("cover", resources.title.clone(), &active_client).unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);
    }

    fn test_persist() {
        let resources = TestResources {
            title: "test_persist".to_string(),
            command: "[float;size 30% 30%] kitty --title test_persist".to_string(),
        };

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            false
        );

        let clients_with_title: Vec<Client> = Vec::new();
        summon_normal(
            &resources.command,
            "",
            &Workspace::get_active().unwrap(),
            &clients_with_title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);

        hide_active("", "".to_string(), &active_client).unwrap();
        sleep(Duration::from_millis(1000));

        assert!(Clients::get()
            .unwrap()
            .into_iter()
            .filter(|x| x.workspace.id == Workspace::get_active().unwrap().id)
            .any(|x| x.initial_title == resources.title));
    }

    fn test_summon_hide() {
        let resources = TestResources {
            title: "test_summon_hide".to_string(),
            command: "[float;size 30% 30%] kitty --title test_summon_hide".to_string(),
        };

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            false
        );

        scratchpad(&resources.title, &resources.command, "summon").unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );

        scratchpad(&resources.title, &resources.command, "summon").unwrap();
        sleep(Duration::from_millis(1000));

        let clients_with_title: Vec<Client> = Clients::get()
            .unwrap()
            .into_iter()
            .filter(|x| x.initial_title == resources.title)
            .collect();

        assert_eq!(clients_with_title.len(), 1);
        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );

        scratchpad(&resources.title, &resources.command, "hide").unwrap();
        sleep(Duration::from_millis(1000));

        assert_ne!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title,
        );

        let clients_with_title: Vec<Client> = Clients::get()
            .unwrap()
            .into_iter()
            .filter(|x| x.initial_title == resources.title)
            .collect();

        assert_eq!(clients_with_title.len(), 1);
        assert_eq!(clients_with_title[0].workspace.name, "42");

        scratchpad(&resources.title, &resources.command, "hide").unwrap();
        sleep(Duration::from_millis(1000));

        let clients_with_title: Vec<Client> = Clients::get()
            .unwrap()
            .into_iter()
            .filter(|x| x.initial_title == resources.title)
            .collect();

        assert_eq!(clients_with_title.len(), 1);
        assert_eq!(clients_with_title[0].workspace.name, "42");
    }

    #[test]
    fn test_summon() {
        test_summon_normal();
        test_summon_special();
    }

    #[test]
    fn test_options() {
        test_cover();
        test_persist();
        test_summon_hide();
    }
}
