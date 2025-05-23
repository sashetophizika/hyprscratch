use crate::config::Config;
use crate::logs::{log, LogErr};
use hyprland::data::{Client, Clients};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use std::io::Write;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;

pub fn warn_deprecated(feature: &str) -> Result<()> {
    log(format!("The '{feature}' feature is deprecated."), "WARN")?;
    println!("Try 'hyprscratch help' and change your configuration before it is removed.");
    Ok(())
}

pub fn flag_present<'a>(arg: &str, flags: &[&'a str]) -> Option<&'a str> {
    if flags.is_empty() {
        return None;
    }

    for flag in flags {
        if flag.is_empty() {
            continue;
        }

        let long = format!("--{flag}");
        let is_short = |x: &str| {
            x.len() > 1
                && !x.contains('=')
                && x.starts_with("-")
                && !x[1..].starts_with("-")
                && x.contains(flag.as_bytes()[0] as char)
        };

        let is_present = |x: &str| x == *flag || *x == long || is_short(x);
        if is_present(arg) {
            return Some(flag);
        }

        if let Some((key, _)) = arg.split_once('=') {
            if is_present(key) {
                return Some(flag);
            }
        }
    }
    None
}

pub fn get_flag_arg(args: &[String], flag: &str) -> Option<String> {
    if flag.is_empty() {
        return None;
    }

    let long = format!("--{flag}");
    let short = format!("-{}", flag.as_bytes()[0] as char);

    let is_present = |x: &str| x == flag || *x == long || *x == short;
    if let Some(ci) = args.iter().position(|x| is_present(x)) {
        return args.get(ci + 1).cloned();
    }

    args.iter().find_map(|x| {
        if let Some((key, val)) = x.split_once('=') {
            if is_present(key) {
                return Some(val.to_string());
            }
        }
        None
    })
}

pub fn dequote(s: &str) -> String {
    let tr = s.trim();
    if tr.is_empty() {
        return String::new();
    }

    match &tr[..1] {
        "\"" | "'" => tr[1..tr.len() - 1].into(),
        _ => tr.into(),
    }
}

pub fn send(socket: Option<&str>, request: &str, message: &str) -> Result<()> {
    let mut stream = UnixStream::connect(socket.unwrap_or("/tmp/hyprscratch/hyprscratch.sock"))?;
    stream.write_all(format!("{request}?{message}").as_bytes())?;
    stream.shutdown(Shutdown::Write)?;
    Ok(())
}

pub fn move_to_special(client: &Client) -> Result<()> {
    hyprland::dispatch!(
        MoveToWorkspaceSilent,
        WorkspaceIdentifierWithSpecial::Special(Some(&client.initial_title.clone())),
        Some(WindowIdentifier::Address(client.address.clone()))
    )
    .unwrap_or_else(|_| {
        log("MoveToSpecial returned Err".into(), "DEBUG").unwrap();
    });
    Ok(())
}

pub fn hide_special(cl: &Client) {
    if cl.workspace.id <= 0 {
        hyprland::dispatch!(ToggleSpecialWorkspace, Some(cl.initial_title.clone()))
            .log_err(file!(), line!());
    }
}

pub fn is_on_special(client: &Client) -> bool {
    client.workspace.name.contains("special")
}

pub fn is_known(titles: &[String], client: &Client) -> bool {
    titles.contains(&client.initial_title)
}

pub fn move_floating(titles: &[String]) -> Result<()> {
    Clients::get()?
        .into_iter()
        .filter(|cl| cl.floating && !is_on_special(cl) && is_known(titles, cl))
        .for_each(|cl| move_to_special(&cl).log_err(file!(), line!()));
    Ok(())
}

pub fn prepend_rules(
    command: &str,
    workspace: Option<&String>,
    silent: bool,
    float: bool,
) -> String {
    let mut rules = String::from("[");
    if let Some(workspace) = workspace {
        let silent = if silent { "silent" } else { "" };
        rules += &format!("workspace special:{workspace} {silent};");
    }

    if float {
        rules += "float;";
    }

    if command.find('[').is_none() {
        format!("{rules}] {command}")
    } else {
        command.replacen('[', &rules, 1)
    }
}

pub fn autospawn(config: &mut Config) -> Result<()> {
    let client_titles: Vec<String> = Clients::get()?
        .into_iter()
        .map(|x| x.initial_title)
        .collect();

    config
        .scratchpads
        .clone()
        .into_iter()
        .filter(|sc| !sc.options.lazy && !client_titles.contains(&sc.title))
        .for_each(|sc| {
            let cmd = prepend_rules(&sc.command, Some(&sc.name), true, !sc.options.tiled);
            hyprland::dispatch!(Exec, &cmd).log_err(file!(), line!())
        });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratchpad::Scratchpad;
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
            sleep(Duration::from_millis(500));
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
        sleep(Duration::from_millis(1000));

        clients = Clients::get().unwrap().into_iter();
        resources
            .titles
            .clone()
            .map(|title| assert_eq!(clients.clone().any(|x| x.initial_title == title), true));

        move_floating(&[
            "test_nonfloating_move".to_owned(),
            "test_scratchpad_move".to_owned(),
        ])
        .unwrap();
        sleep(Duration::from_millis(500));

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

        sleep(Duration::from_millis(500));
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

        let options = vec![
            "".to_string(),
            "special eager".to_string(),
            "lazy".to_string(),
        ];

        let scratchpads: Vec<Scratchpad> = resources
            .titles
            .iter()
            .zip(resources.commands.clone())
            .zip(options)
            .map(|((t, c), o)| Scratchpad::new(&t, &t, &c, &o))
            .collect();

        let mut config = Config {
            scratchpads,
            config_file: "".to_string(),
            special_titles: Vec::new(),
            normal_titles: Vec::new(),
            pinned_titles: Vec::new(),
            slick_titles: Vec::new(),
            dirty_titles: Vec::new(),
            fickle_titles: resources.titles.to_vec(),
        };

        autospawn(&mut config).unwrap();
        sleep(Duration::from_millis(1000));

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
