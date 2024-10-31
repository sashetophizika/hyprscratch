use std::fs::File;
use std::io::Write;

use crate::config::Config;
use chrono::Local;
use hyprland::data::Clients;
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;

pub fn log(msg: String, level: &str) -> Result<()> {
    let mut file = File::options()
        .create(true)
        .read(true)
        .append(true)
        .open("/tmp/hyprscratch/hyprscratch.log")?;

    println!("{msg}");
    file.write_all(
        format!(
            "{} [{level}] {msg}\n",
            Local::now().format("%d.%m.%Y %H:%M:%S")
        )
        .as_bytes(),
    )?;
    Ok(())
}

pub fn move_floating(titles: Vec<String>) -> Result<()> {
    Clients::get()?
        .into_iter()
        .filter(|x| x.floating && x.workspace.id != 42 && titles.contains(&x.initial_title))
        .for_each(|x| {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Id(42),
                Some(WindowIdentifier::Address(x.address.clone()))
            )
            .unwrap()
        });
    Ok(())
}

pub fn autospawn(config: &mut Config) -> Result<()> {
    let client_titles = Clients::get()?
        .into_iter()
        .map(|x| x.initial_title)
        .collect::<Vec<_>>();

    config
        .commands
        .iter()
        .zip(&config.titles)
        .zip(&config.options)
        .filter(|((_, title), option)| {
            (option.contains("on-start") || option.contains("onstart"))
                && !client_titles.contains(title)
        })
        .for_each(|((command, title), option)| {
            let mut cmd = command.clone();
            if command.find('[').is_none() {
                cmd.insert_str(0, "[]");
            }

            if option.contains("special") {
                hyprland::dispatch!(
                    Exec,
                    &cmd.replacen('[', &format!("[workspace special:{} silent;", title), 1)
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
                Some(WindowIdentifier::Address(x.address.clone()))
            )
            .unwrap()
        });
    }

    for title in normal_titles.iter() {
        clients.iter().filter(|x| &x.title == title).for_each(|x| {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Id(42),
                Some(WindowIdentifier::Address(x.address.clone()))
            )
            .unwrap()
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprland::data::{Client, Workspace};
    use std::thread::sleep;
    use std::time::Duration;

    struct TestResources {
        titles: [String; 3],
        commands: [String; 3],
        expected_workspace: [String; 3],
        spawned: [usize; 3],
    }

    impl Drop for TestResources {
        fn drop(&mut self) {
            self.titles
                .clone()
                .into_iter()
                .zip(self.spawned)
                .filter(|(_, spawned)| *spawned == 1)
                .for_each(|(title, _)| {
                    hyprland::dispatch!(CloseWindow, WindowIdentifier::Title(&title)).unwrap()
                });
            sleep(Duration::from_millis(1000));
        }
    }

    #[test]
    fn test_move_floating() {
        let active_workspace = Workspace::get_active().unwrap();
        let resources = TestResources {
            titles: [
                "test_nonfloating_move".to_string(),
                "test_notcontained_move".to_string(),
                "test_scratchpad_move".to_string(),
            ],
            commands: [
                "kitty --title test_nonfloating_move".to_string(),
                "[float; size 30% 30%; move 0 0] kitty --title test_notcontained_move".to_string(),
                "[float; size 30% 30%; move 30% 0] kitty --title test_scratchpad_move".to_string(),
            ],
            expected_workspace: [
                active_workspace.name.clone(),
                active_workspace.name,
                "42".to_string(),
            ],
            spawned: [1; 3],
        };

        let mut clients = Clients::get().unwrap().into_iter();
        resources
            .titles
            .clone()
            .map(|title| assert_eq!(clients.clone().any(|x| x.initial_title == title), false));

        resources
            .commands
            .clone()
            .map(|command| hyprland::dispatch!(Exec, &command).unwrap());
        sleep(Duration::from_millis(2000));

        clients = Clients::get().unwrap().into_iter();
        resources
            .titles
            .clone()
            .map(|title| assert_eq!(clients.clone().any(|x| x.initial_title == title), true));

        move_floating(vec![
            "test_nonfloating_move".to_owned(),
            "test_scratchpad_move".to_owned(),
        ])
        .unwrap();
        sleep(Duration::from_millis(1000));

        clients = Clients::get().unwrap().into_iter();
        resources
            .titles
            .clone()
            .into_iter()
            .zip(&resources.expected_workspace)
            .for_each(|(title, workspace)| {
                let clients_with_title: Vec<Client> = clients
                    .clone()
                    .filter(|x| x.initial_title == title)
                    .collect();

                assert_eq!(clients_with_title.len(), 1);
                assert_eq!(&clients_with_title[0].workspace.name, workspace);
            });

        sleep(Duration::from_millis(1000));
    }

    #[test]
    fn test_autospawn() {
        let resources = TestResources {
            titles: [
                "test_normal_autospawn".to_string(),
                "test_special_autospawn".to_string(),
                "test_notonstart_autospawn".to_string(),
            ],
            commands: [
                "kitty --title test_normal_autospawn".to_string(),
                "[float] kitty --title test_special_autospawn".to_string(),
                "kitty --title test_notonstart_autospawn".to_string(),
            ],
            expected_workspace: [
                "42".to_string(),
                "special:test_special_autospawn".to_string(),
                "".to_string(),
            ],
            spawned: [1, 1, 0],
        };

        let mut clients = Clients::get().unwrap().into_iter();
        resources
            .titles
            .clone()
            .map(|title| assert_eq!(clients.clone().any(|x| x.initial_title == title), false));

        let mut config = Config {
            titles: resources.titles.to_vec(),
            normal_titles: Vec::new(),
            special_titles: Vec::new(),
            commands: resources.commands.to_vec(),
            options: vec![
                "onstart".to_string(),
                "special onstart".to_string(),
                "".to_string(),
            ],
            unshiny_titles: Vec::new(),
        };

        autospawn(&mut config).unwrap();
        sleep(Duration::from_millis(2000));

        clients = Clients::get().unwrap().into_iter();
        resources
            .titles
            .clone()
            .into_iter()
            .zip(&resources.expected_workspace)
            .zip(resources.spawned)
            .for_each(|((title, workspace), spawned)| {
                let clients_with_title: Vec<Client> = clients
                    .clone()
                    .filter(|x| x.initial_title == title)
                    .collect();

                assert_eq!(clients_with_title.len(), spawned);
                if spawned == 1 {
                    assert_eq!(&clients_with_title[0].workspace.name, workspace);
                }
            });
    }
}
