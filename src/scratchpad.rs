use crate::logs::log;
use crate::logs::LogErr;
use crate::utils::dequote;
use crate::utils::{get_flag_arg, move_to_special, prepend_rules};
use hyprland::data::{Client, Clients, FullscreenMode, Monitors, Workspace};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use std::collections::HashMap;

struct HyprlandState {
    active_workspace_id: i32,
    clients_with_title: Vec<Client>,
    monitors: HashMap<String, i32>,
}

impl HyprlandState {
    fn new(title: &str) -> Result<HyprlandState> {
        let mut monitors = HashMap::new();
        Monitors::get()?.into_iter().for_each(|x| {
            monitors.insert(x.name, x.active_workspace.id);
            monitors.insert(x.id.to_string(), x.active_workspace.id);
        });

        let active_workspace_id = Workspace::get_active()?.id;
        let clients_with_title = Clients::get()?
            .into_iter()
            .filter(|x| x.initial_title == title)
            .collect();

        Ok(HyprlandState {
            active_workspace_id,
            monitors,
            clients_with_title,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScratchpadOptions {
    options_string: String,
    pub summon: bool,
    pub hide: bool,
    pub shiny: bool,
    pub sticky: bool,
    pub poly: bool,
    pub cover: bool,
    pub persist: bool,
    pub stack: bool,
    pub tiled: bool,
    pub lazy: bool,
    pub special: bool,
    pub monitor: Option<String>,
}

impl ScratchpadOptions {
    pub fn new(opts: &str) -> ScratchpadOptions {
        let get_arg = |opt| {
            get_flag_arg(
                &dequote(opts)
                    .split(" ")
                    .map(|x| x.to_owned())
                    .collect::<Vec<String>>(),
                opt,
            )
        };

        ScratchpadOptions {
            options_string: opts.to_string(),
            summon: opts.contains("summon"),
            hide: opts.contains("hide"),
            shiny: opts.contains("shiny"),
            sticky: opts.contains("sticky"),
            poly: opts.contains("poly"),
            cover: opts.contains("cover"),
            persist: opts.contains("persist"),
            stack: opts.contains("stack"),
            tiled: opts.contains("tiled"),
            lazy: opts.contains("lazy"),
            special: opts.contains("special"),
            monitor: get_arg("monitor"),
        }
    }

    pub fn toggle(&mut self, opt: &str) {
        match opt {
            "summon" => self.summon ^= true,
            "hide" => self.hide ^= true,
            "shiny" => self.shiny ^= true,
            "sticky" => self.sticky ^= true,
            "poly" => self.poly ^= true,
            "cover" => self.cover ^= true,
            "persist" => self.persist ^= true,
            "stack" => self.stack ^= true,
            "tiled" => self.tiled ^= true,
            "lazy" => self.lazy ^= true,
            "special" => self.special ^= true,
            _ => (),
        };
    }

    pub fn get_string(&self) -> String {
        self.options_string.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scratchpad {
    pub name: String,
    pub title: String,
    pub command: String,
    pub options: ScratchpadOptions,
}

impl Scratchpad {
    pub fn new(name: &str, title: &str, command: &str, options: &str) -> Scratchpad {
        Scratchpad {
            name: name.into(),
            title: title.into(),
            command: command.into(),
            options: ScratchpadOptions::new(options),
        }
    }

    fn summon_special(&mut self, state: &HyprlandState) -> Result<()> {
        let special_with_title: Vec<&Client> = state
            .clients_with_title
            .iter()
            .filter(|x| x.workspace.id < 0)
            .collect();

        if special_with_title.is_empty() && !state.clients_with_title.is_empty() {
            move_to_special(&state.clients_with_title[0])?;
            if state.clients_with_title[0].workspace.id == state.active_workspace_id {
                hyprland::dispatch!(ToggleSpecialWorkspace, Some(self.name.clone()))?;
            }
        } else if state.clients_with_title.is_empty() {
            let special_cmd =
                prepend_rules(&self.command, Some(&self.name), false, !self.options.tiled);
            hyprland::dispatch!(Exec, &special_cmd)?;
        } else {
            hyprland::dispatch!(ToggleSpecialWorkspace, Some(self.name.clone()))?;
        }
        Ok(())
    }

    fn summon_normal(&mut self, state: &HyprlandState) -> Result<()> {
        if state.clients_with_title.is_empty() {
            self.command.split("?").for_each(|x| {
                let cmd = prepend_rules(x, None, false, !self.options.tiled);
                hyprland::dispatch!(Exec, &cmd).log_err(file!(), line!());
            });
        } else {
            let workspace_id = if let Some(m) = &self.options.monitor {
                if let Some(id) = state.monitors.get(m) {
                    *id
                } else {
                    log(format!("Monitor {m} not found"), "WARN")?;
                    state.active_workspace_id
                }
            } else {
                state.active_workspace_id
            };

            for client in state
                .clients_with_title
                .iter()
                .filter(|x| !self.is_on_active(x, state))
            {
                hyprland::dispatch!(
                    MoveToWorkspace,
                    WorkspaceIdentifierWithSpecial::Id(workspace_id),
                    Some(WindowIdentifier::Address(client.address.clone()))
                )?;
                if !self.options.poly {
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

    fn summon(&mut self, state: &HyprlandState) -> Result<()> {
        if self.options.special {
            self.summon_special(state)?;
        } else if !self.options.hide {
            self.summon_normal(state)?;
        }
        Ok(())
    }

    fn hide_active(&self, titles: &[String], active_client: &Client) -> Result<()> {
        if !self.options.cover
            && !self.options.stack
            && active_client.floating
            && titles.contains(&active_client.initial_title)
        {
            move_to_special(active_client)?;
        }
        Ok(())
    }

    fn is_on_active(&self, client: &Client, state: &HyprlandState) -> bool {
        if self.options.monitor.is_some() {
            state.monitors.values().any(|id| *id == client.workspace.id)
        } else {
            state.active_workspace_id == client.workspace.id
        }
    }

    pub fn run(&mut self, titles: &[String]) -> Result<()> {
        let state = HyprlandState::new(&self.title)?;

        if let Some(active_client) = Client::get_active()? {
            let mut clients_on_active = state
                .clients_with_title
                .clone()
                .into_iter()
                .filter(|x| self.is_on_active(x, &state))
                .peekable();

            let hide_all = !active_client.floating
                || active_client.initial_title == self.title
                || active_client.fullscreen == FullscreenMode::None;

            if self.options.special || clients_on_active.peek().is_none() {
                self.summon(&state)?;
                self.hide_active(titles, &active_client)?;
            } else if hide_all && !self.options.summon {
                clients_on_active.for_each(|x| {
                    move_to_special(&x).log_err(file!(), line!());
                });
            } else {
                hyprland::dispatch!(
                    FocusWindow,
                    WindowIdentifier::Address(clients_on_active.peek().unwrap().address.clone())
                )?;
            }
        } else {
            self.summon(&state)?;
        }

        Dispatch::call(DispatchType::BringActiveToTop)?;
        Ok(())
    }
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
                sleep(Duration::from_millis(500));
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

        Scratchpad::new(&resources.title, &resources.title, &resources.command, "")
            .summon_normal(&HyprlandState::new(&resources.title).unwrap())
            .unwrap();
        sleep(Duration::from_millis(500));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);

        Scratchpad::new(&resources.title, &resources.title, &resources.command, "")
            .hide_active(&vec![resources.title.clone()], &active_client)
            .unwrap();
        sleep(Duration::from_millis(500));

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
        Scratchpad::new(&resources.title, &resources.title, &resources.command, "")
            .summon_normal(&HyprlandState::new(&resources.title).unwrap())
            .unwrap();
        sleep(Duration::from_millis(500));

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

        Scratchpad::new(&resources.title, &resources.title, &resources.command, "")
            .summon_special(&HyprlandState::new(&resources.title).unwrap())
            .unwrap();
        sleep(Duration::from_millis(500));

        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );

        Scratchpad::new(&resources.title, &resources.title, &resources.command, "")
            .summon_special(&HyprlandState::new(&resources.title).unwrap())
            .unwrap();
        sleep(Duration::from_millis(500));

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

        Scratchpad::new(&resources.title, &resources.title, &resources.command, "")
            .summon_special(&HyprlandState::new(&resources.title).unwrap())
            .unwrap();
        sleep(Duration::from_millis(500));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);

        Scratchpad::new(
            &resources.title,
            &resources.title,
            &resources.command,
            "cover",
        )
        .hide_active(&vec![resources.title.clone()], &active_client)
        .unwrap();
        sleep(Duration::from_millis(500));

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

        Scratchpad::new(&resources.title, &resources.title, &resources.command, "")
            .summon_normal(&HyprlandState::new(&resources.title).unwrap())
            .unwrap();
        sleep(Duration::from_millis(500));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);

        Scratchpad::new(&resources.title, &resources.title, &resources.command, "")
            .hide_active(&vec![], &active_client)
            .unwrap();
        sleep(Duration::from_millis(500));

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
        Scratchpad::new(
            &resources.title,
            &resources.title,
            &resources.command,
            "poly",
        )
        .run(&vec![resources.title.clone()])
        .unwrap();
        sleep(Duration::from_millis(500));

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .filter(|x| x.initial_title == resources.title
                    && x.workspace.name == Workspace::get_active().unwrap().name)
                .count(),
            2
        );

        Scratchpad::new(
            &resources.title,
            &resources.title,
            &resources.command,
            "poly",
        )
        .run(&vec![resources.title.clone()])
        .unwrap();
        sleep(Duration::from_millis(500));

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

        Scratchpad::new(
            &resources[0].title,
            &resources[0].title,
            &resources[0].command,
            "tiled",
        )
        .run(&vec![resources[0].title.clone()])
        .unwrap();
        sleep(Duration::from_millis(500));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources[0].title);
        assert_eq!(active_client.floating, false);

        Scratchpad::new(
            &resources[1].title,
            &resources[1].title,
            &resources[1].command,
            "",
        )
        .run(&vec![resources[1].title.clone()])
        .unwrap();
        sleep(Duration::from_millis(500));

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

        Scratchpad::new(
            &resources.title,
            &resources.title,
            &resources.command,
            "summon",
        )
        .run(&vec![resources.title.clone()])
        .unwrap();
        sleep(Duration::from_millis(500));

        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );

        Scratchpad::new(
            &resources.title,
            &resources.title,
            &resources.command,
            "summon",
        )
        .run(&vec![resources.title.clone()])
        .unwrap();
        sleep(Duration::from_millis(500));

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

        Scratchpad::new(
            &resources.title,
            &resources.title,
            &resources.command,
            "hide",
        )
        .run(&vec![resources.title.clone()])
        .unwrap();
        sleep(Duration::from_millis(500));

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

        Scratchpad::new(
            &resources.title,
            &resources.title,
            &resources.command,
            "hide",
        )
        .run(&vec![resources.title.clone()])
        .unwrap();
        sleep(Duration::from_millis(500));

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
