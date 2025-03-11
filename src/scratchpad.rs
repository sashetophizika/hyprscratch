use crate::config::Scratchpad;
use crate::logs::LogErr;
use crate::utils::{move_to_special, prepend_rules};
use hyprland::data::{Client, Clients, FullscreenMode, Workspace};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;

struct HyprlandState {
    active_workspace: Workspace,
    clients_with_title: Vec<Client>,
}

impl HyprlandState {
    fn new(title: &str) -> HyprlandState {
        HyprlandState {
            active_workspace: Workspace::get_active().unwrap_log(file!(), line!()),
            clients_with_title: Clients::get()
                .unwrap_log(file!(), line!())
                .into_iter()
                .filter(|x| x.initial_title == title)
                .collect(),
        }
    }
}

fn summon_special(scratchpad: &mut Scratchpad, state: &HyprlandState) -> Result<()> {
    let special_with_title: Vec<&Client> = state
        .clients_with_title
        .iter()
        .filter(|x| x.workspace.id < 0 && x.workspace.id > -1000)
        .collect();

    if special_with_title.is_empty() && !state.clients_with_title.is_empty() {
        move_to_special(&state.clients_with_title[0], &mut scratchpad.workspace)?;
        if state.clients_with_title[0].workspace.id == state.active_workspace.id {
            hyprland::dispatch!(ToggleSpecialWorkspace, Some(scratchpad.workspace.clone()))?;
        }
    } else if state.clients_with_title.is_empty() {
        let special_cmd = prepend_rules(
            &scratchpad.command,
            Some(&scratchpad.workspace),
            false,
            !scratchpad.options.tiled,
        );
        hyprland::dispatch!(Exec, &special_cmd)?;
    } else {
        hyprland::dispatch!(ToggleSpecialWorkspace, Some(scratchpad.workspace.clone()))?;
    }
    Ok(())
}

fn summon_normal(scratchpad: &mut Scratchpad, state: &HyprlandState) -> Result<()> {
    if state.clients_with_title.is_empty() {
        scratchpad.command.split("?").for_each(|x| {
            let cmd = prepend_rules(x, None, false, !scratchpad.options.tiled);
            hyprland::dispatch!(Exec, &cmd).log_err(file!(), line!());
        });
    } else {
        for client in state
            .clients_with_title
            .iter()
            .filter(|x| x.workspace.id != state.active_workspace.id)
        {
            hyprland::dispatch!(
                MoveToWorkspace,
                WorkspaceIdentifierWithSpecial::Relative(0),
                Some(WindowIdentifier::Address(client.address.clone()))
            )?;
            if !scratchpad.options.poly {
                break;
            }
        }

        hyprland::dispatch!(
            FocusWindow,
            WindowIdentifier::Address(state.clients_with_title[0].address.clone())
        )?;
    }
    Ok(())
}

fn summon(scratchpad: &mut Scratchpad, state: &HyprlandState) -> Result<()> {
    if scratchpad.options.special {
        summon_special(scratchpad, state)?;
    } else if !scratchpad.options.hide {
        summon_normal(scratchpad, state)?;
    }
    Ok(())
}

fn hide_active(scratchpad: &mut Scratchpad, titles: &str, active_client: &Client) -> Result<()> {
    if !scratchpad.options.cover
        && !scratchpad.options.stack
        && active_client.floating
        && titles.contains(&active_client.initial_title)
    {
        move_to_special(active_client, &mut String::from("empty"))?;
    }
    Ok(())
}

pub fn do_the_scratchpad(scratchpad: &mut Scratchpad, titles: &str) -> Result<()> {
    let state = HyprlandState::new(&scratchpad.title);

    if let Some(active_client) = Client::get_active()? {
        let mut clients_on_active = state
            .clients_with_title
            .clone()
            .into_iter()
            .filter(|x| x.workspace.id == state.active_workspace.id)
            .peekable();

        let hide_all = !active_client.floating
            || active_client.initial_title == scratchpad.title
            || active_client.fullscreen == FullscreenMode::None;

        if scratchpad.options.special || clients_on_active.peek().is_none() {
            summon(scratchpad, &state)?;
            hide_active(scratchpad, titles, &active_client)?;
        } else if hide_all && !scratchpad.options.summon {
            clients_on_active.for_each(|x| {
                move_to_special(&x, &mut scratchpad.workspace).log_err(file!(), line!());
            });
        } else {
            hyprland::dispatch!(
                FocusWindow,
                WindowIdentifier::Address(clients_on_active.peek().unwrap().address.clone())
            )?;
        }
    } else {
        summon(scratchpad, &state)?;
    }

    Dispatch::call(DispatchType::BringActiveToTop)?;
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
            self.command.split("?").for_each(|_| {
                hyprland::dispatch!(CloseWindow, WindowIdentifier::Title(&self.title)).unwrap();
                sleep(Duration::from_millis(1000));
            });
        }
    }

    #[test]
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

        summon_normal(
            &mut Scratchpad::new(&resources.title, &resources.title, &resources.command, ""),
            &HyprlandState::new(""),
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);

        hide_active(
            &mut Scratchpad::new(&resources.title, &resources.title, &resources.command, ""),
            &resources.title,
            &active_client,
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
        summon_normal(
            &mut Scratchpad::new(&resources.title, &resources.title, &resources.command, ""),
            &HyprlandState::new(&resources.title),
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(Workspace::get_active().unwrap().id, active_workspace.id);
        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );
    }

    #[test]
    fn test_summon_special() {
        let resources = TestResources {
            title: "test_special_scratchpad".to_string(),
            command: "[size 30% 30%] kitty --title test_special_scratchpad".to_string(),
        };

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            false
        );

        summon_special(
            &mut Scratchpad::new(&resources.title, &resources.title, &resources.command, ""),
            &HyprlandState::new(""),
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );

        summon_special(
            &mut Scratchpad::new(&resources.title, &resources.title, &resources.command, ""),
            &HyprlandState::new(&resources.title),
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

        summon_special(
            &mut Scratchpad::new(&resources.title, &resources.title, &resources.command, ""),
            &HyprlandState::new(&resources.title),
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);

        hide_active(
            &mut Scratchpad::new(
                &resources.title,
                &resources.title,
                &resources.command,
                "cover",
            ),
            &resources.title,
            &active_client,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);
    }

    #[test]
    fn test_persist() {
        let resources = TestResources {
            title: "test_persist".to_string(),
            command: "[size 30% 30%] kitty --title test_persist".to_string(),
        };

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            false
        );

        summon_normal(
            &mut Scratchpad::new(&resources.title, &resources.title, &resources.command, ""),
            &HyprlandState::new(""),
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);

        hide_active(
            &mut Scratchpad::new(&resources.title, &resources.title, &resources.command, ""),
            "",
            &active_client,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert!(Clients::get()
            .unwrap()
            .into_iter()
            .filter(|x| x.workspace.id == Workspace::get_active().unwrap().id)
            .any(|x| x.initial_title == resources.title));
    }

    #[test]
    fn test_poly() {
        let resources = TestResources {
            title: "test_poly".to_string(),
            command: "[size 30% 30%; move 0 0] kitty --title test_poly ? [size 30% 30%; move 30% 0] kitty --title test_poly".to_string(),
        };

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            false
        );

        do_the_scratchpad(
            &mut Scratchpad::new(
                &resources.title,
                &resources.title,
                &resources.command,
                "poly",
            ),
            &resources.title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .filter(|x| x.initial_title == resources.title
                    && x.workspace.name == Workspace::get_active().unwrap().name)
                .count(),
            2
        );

        do_the_scratchpad(
            &mut Scratchpad::new(
                &resources.title,
                &resources.title,
                &resources.command,
                "poly",
            ),
            &resources.title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .filter(|x| x.initial_title == resources.title
                    && x.workspace.name == Workspace::get_active().unwrap().name)
                .count(),
            0
        );
    }

    #[test]
    fn test_tiled() {
        let resources = [
            TestResources {
                title: "test_tiled".to_string(),
                command: "kitty --title test_tiled".to_string(),
            },
            TestResources {
                title: "test_floating".to_string(),
                command: "kitty --title test_floating".to_string(),
            },
        ];

        assert_eq!(
            Clients::get().unwrap().iter().any(|x| resources
                .iter()
                .filter(|y| y.title == x.initial_title)
                .next()
                .is_none()),
            true
        );

        do_the_scratchpad(
            &mut Scratchpad::new(
                &resources[0].title,
                &resources[0].title,
                &resources[0].command,
                "tiled",
            ),
            &resources[0].title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources[0].title);
        assert_eq!(active_client.floating, false);

        do_the_scratchpad(
            &mut Scratchpad::new(
                &resources[1].title,
                &resources[1].title,
                &resources[1].command,
                "",
            ),
            &resources[1].title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources[1].title);
        assert_eq!(active_client.floating, true);
    }

    #[test]
    fn test_summon_hide() {
        let resources = TestResources {
            title: "test_summon_hide".to_string(),
            command: "[size 30% 30%] kitty --title test_summon_hide".to_string(),
        };

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            false
        );

        do_the_scratchpad(
            &mut Scratchpad::new(
                &resources.title,
                &resources.title,
                &resources.command,
                "summon",
            ),
            &resources.title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );

        do_the_scratchpad(
            &mut Scratchpad::new(
                &resources.title,
                &resources.title,
                &resources.command,
                "summon",
            ),
            &resources.title,
        )
        .unwrap();
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

        do_the_scratchpad(
            &mut Scratchpad::new(
                &resources.title,
                &resources.title,
                &resources.command,
                "hide",
            ),
            &resources.title,
        )
        .unwrap();
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
        assert_eq!(
            clients_with_title[0].workspace.name,
            "special:".to_owned() + &resources.title
        );

        do_the_scratchpad(
            &mut Scratchpad::new(
                &resources.title,
                &resources.title,
                &resources.command,
                "hide",
            ),
            &resources.title,
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let clients_with_title: Vec<Client> = Clients::get()
            .unwrap()
            .into_iter()
            .filter(|x| x.initial_title == resources.title)
            .collect();

        assert_eq!(clients_with_title.len(), 1);
        assert_eq!(
            clients_with_title[0].workspace.name,
            "special:".to_owned() + &resources.title
        );
    }
}
