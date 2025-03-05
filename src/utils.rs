use crate::config::Config;
use crate::logs::{log, LogErr};
use hyprland::data::Client;
use hyprland::data::Clients;
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;

pub fn warn_deprecated(feature: &str) -> Result<()> {
    log(format!("The '{feature}' feature is deprecated."), "WARN")?;
    println!("Try 'hyprscratch help' and change your configuration before it is removed.");
    Ok(())
}

pub fn flag_present(args: &[String], flag: &str) -> Option<String> {
    if flag.is_empty() {
        return None;
    }

    let long = format!("--{flag}");
    let short = flag.as_bytes()[0] as char;

    if args.iter().any(|x| {
        x == flag
            || x == &long
            || (x.len() > 1 && x.starts_with("-") && !x[1..].starts_with("-") && x.contains(short))
    }) {
        return Some(flag.to_string());
    }
    None
}

pub fn get_flag_arg(args: &[String], flag: &str) -> Option<String> {
    if flag.is_empty() {
        return None;
    }

    let long = format!("--{flag}");
    let short = format!("-{}", flag.as_bytes()[0] as char);

    if let Some(ci) = args
        .iter()
        .position(|x| x == flag || *x == long || *x == short)
    {
        return args.get(ci + 1).cloned();
    }
    None
}

pub fn move_to_special(client: &Client) -> Result<()> {
    hyprland::dispatch!(
        MoveToWorkspaceSilent,
        WorkspaceIdentifierWithSpecial::Special(Some(&client.initial_title)),
        Some(WindowIdentifier::Address(client.address.clone()))
    )
    .unwrap_or(
        hyprland::dispatch!(
            MoveToWorkspaceSilent,
            WorkspaceIdentifierWithSpecial::Name(&format!("special:{}", client.initial_title)),
            Some(WindowIdentifier::Address(client.address.clone()))
        )
        .unwrap_log(file!(), line!()),
    );
    Ok(())
}

pub fn move_floating(titles: Vec<String>) -> Result<()> {
    Clients::get()?
        .into_iter()
        .filter(|x| x.floating && x.workspace.id > 0 && titles.contains(&x.initial_title))
        .for_each(|x| move_to_special(&x).unwrap());
    Ok(())
}

pub fn prepend_rules(
    command: &str,
    special_title: Option<&str>,
    silent: bool,
    float: bool,
) -> String {
    let mut rules = String::from("[");
    if let Some(title) = special_title {
        let silent = if silent { "silent" } else { "" };
        rules += &format!("workspace special:{title} {silent};");
    }

    if float {
        rules += "float;";
    }

    if command.find('[').is_none() {
        return format!("{rules}] {command}");
    } else {
        return command.replacen('[', &rules, 1);
    }
}

pub fn autospawn(config: &mut Config, eager: bool) -> Result<()> {
    let client_titles = Clients::get()?
        .into_iter()
        .map(|x| x.initial_title)
        .collect::<Vec<_>>();

    let auto_spawn_commands: Vec<((&String, &String), &String)> = if eager {
        config
            .commands
            .iter()
            .zip(&config.titles)
            .zip(&config.options)
            .filter(|((_, title), options)| {
                !options.contains("lazy") && !client_titles.contains(title)
            })
            .collect()
    } else {
        config
            .commands
            .iter()
            .zip(&config.titles)
            .zip(&config.options)
            .filter(|((_, title), options)| {
                options.contains("eager") && !client_titles.contains(title)
            })
            .collect()
    };

    auto_spawn_commands
        .into_iter()
        .for_each(|((command, title), options)| {
            let cmd = prepend_rules(command, Some(title), true, !options.contains("tiled"));
            hyprland::dispatch!(Exec, &cmd).unwrap_log(file!(), line!())
        });

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
                "special:test_scratchpad_move".to_string(),
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
                "special:test_normal_autospawn".to_string(),
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
            config_file: "".to_string(),
            names: resources.titles.to_vec(),
            titles: resources.titles.to_vec(),
            normal_titles: Vec::new(),
            special_titles: Vec::new(),
            slick_titles: Vec::new(),
            dirty_titles: Vec::new(),
            non_persist_titles: resources.titles.to_vec(),
            commands: resources.commands.to_vec(),
            options: vec![
                "".to_string(),
                "special eager".to_string(),
                "lazy".to_string(),
            ],
        };

        autospawn(&mut config, true).unwrap();
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
