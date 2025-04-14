use crate::logs::log;
use crate::logs::LogErr;
use crate::utils::*;
use hyprland::data::{Client, Clients, Monitors, Workspace};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::shared::Address;
use hyprland::Result;
use std::collections::HashMap;

struct HyprlandState {
    active_client: Option<Client>,
    clients_with_title: Vec<Client>,
    monitors: HashMap<String, i32>,
    active_workspace_id: i32,
}

impl HyprlandState {
    fn new(title: &str) -> Result<HyprlandState> {
        let mut monitors = HashMap::new();
        Monitors::get()?.into_iter().for_each(|x| {
            monitors.insert(x.name, x.active_workspace.id);
            monitors.insert(x.id.to_string(), x.active_workspace.id);
        });

        let active_client = Client::get_active()?;
        let active_workspace_id = Workspace::get_active()?.id;
        let clients_with_title = Clients::get()?
            .into_iter()
            .filter(|x| x.initial_title == title)
            .collect();

        Ok(HyprlandState {
            active_client,
            clients_with_title,
            monitors,
            active_workspace_id,
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

    fn show_special(&self, state: &HyprlandState) -> Result<()> {
        move_to_special(&state.clients_with_title[0])?;
        if state.clients_with_title[0].workspace.id == state.active_workspace_id {
            hyprland::dispatch!(ToggleSpecialWorkspace, Some(self.name.clone()))?;
        }
        Ok(())
    }

    fn spawn_special(&self) -> Result<()> {
        let special_cmd =
            prepend_rules(&self.command, Some(&self.name), false, !self.options.tiled);
        hyprland::dispatch!(Exec, &special_cmd)
    }

    fn summon_special(&self, state: &HyprlandState) -> Result<()> {
        let special_with_title: Vec<&Client> = state
            .clients_with_title
            .iter()
            .filter(|x| x.workspace.id < 0)
            .collect();

        if special_with_title.is_empty() && !state.clients_with_title.is_empty() {
            self.show_special(state)?;
        } else if state.clients_with_title.is_empty() {
            self.spawn_special()?;
        } else {
            hyprland::dispatch!(ToggleSpecialWorkspace, Some(self.name.clone()))?;
        }
        Ok(())
    }

    fn get_workspace_id(&self, state: &HyprlandState) -> i32 {
        if let Some(m) = &self.options.monitor {
            if let Some(id) = state.monitors.get(m) {
                *id
            } else {
                let _ = log(format!("Monitor {m} not found"), "WARN");
                state.active_workspace_id
            }
        } else {
            state.active_workspace_id
        }
    }

    fn show_normal(&self, state: &HyprlandState) -> Result<()> {
        for client in state
            .clients_with_title
            .iter()
            .filter(|x| !self.is_on_workspace(x, state))
        {
            let workspace_id = self.get_workspace_id(state);
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
        )
    }

    fn spawn_normal(&self, state: &HyprlandState) {
        self.command.split("?").for_each(|x| {
            hide_special(&state.active_client);
            let cmd = prepend_rules(x, None, false, !self.options.tiled);
            hyprland::dispatch!(Exec, &cmd).log_err(file!(), line!());
        });
    }

    fn summon_normal(&mut self, state: &HyprlandState) -> Result<()> {
        if state.clients_with_title.is_empty() {
            self.spawn_normal(state);
        } else {
            self.show_normal(state)?;
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

    fn hide_active(&self, titles: &[String], state: &HyprlandState) -> Result<()> {
        if self.options.cover {
            return Ok(());
        }

        let should_hide = |cl: &Client| {
            titles.contains(&cl.initial_title)
                && cl.initial_title != self.title
                && cl.workspace.id == state.active_workspace_id
                && cl.floating
        };

        Clients::get()?
            .into_iter()
            .filter(should_hide)
            .for_each(|cl| move_to_special(&cl).log_err(file!(), line!()));
        Ok(())
    }

    fn is_on_workspace(&self, client: &Client, state: &HyprlandState) -> bool {
        if self.options.monitor.is_some() {
            state.monitors.values().any(|id| *id == client.workspace.id)
        } else {
            state.active_workspace_id == client.workspace.id
        }
    }

    fn shoot(&mut self, titles: &[String], state: &HyprlandState, active: &Client) -> Result<()> {
        let mut clients_on_active = state
            .clients_with_title
            .clone()
            .into_iter()
            .filter(|cl| self.is_on_workspace(cl, state))
            .peekable();

        let focus = |adr: &Address| {
            hyprland::dispatch!(FocusWindow, WindowIdentifier::Address(adr.clone()))
                .log_err(file!(), line!());
        };

        let should_refocus = clients_on_active.peek().is_some()
            && !self.options.special
            && !self.options.summon
            && active.initial_title != self.title
            && active.floating;

        let should_hide = active.initial_title == self.title || !active.floating;
        let should_summon =
            self.options.special || self.options.summon || clients_on_active.peek().is_none();

        if should_refocus {
            self.hide_active(titles, state)?;
            focus(&clients_on_active.peek().unwrap().address);
        } else if should_summon {
            self.summon(state)?;
            self.hide_active(titles, state)?;
        } else if should_hide {
            clients_on_active.for_each(|cl| {
                move_to_special(&cl).log_err(file!(), line!());
            });
        } else {
            focus(&clients_on_active.peek().unwrap().address);
        }

        if !should_hide {
            Dispatch::call(DispatchType::BringActiveToTop)?;
        }
        Ok(())
    }

    pub fn trigger(&mut self, titles: &[String]) -> Result<()> {
        let state = HyprlandState::new(&self.title)?;
        if let Some(active_client) = &state.active_client {
            self.shoot(titles, &state, active_client)?;
        } else {
            self.summon(&state)?;
        }

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

        move_to_special(&active_client).unwrap();
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
        .hide_active(
            &vec![resources.title.clone()],
            &HyprlandState::new(&resources.title).unwrap(),
        )
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
            .hide_active(&vec![], &HyprlandState::new(&resources.title).unwrap())
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
        .trigger(&vec![resources.title.clone()])
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
        .trigger(&vec![resources.title.clone()])
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
        .trigger(&vec![resources[0].title.clone()])
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
        .trigger(&vec![resources[1].title.clone()])
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
        .trigger(&vec![resources.title.clone()])
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
        .trigger(&vec![resources.title.clone()])
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
        .trigger(&vec![resources.title.clone()])
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
        .trigger(&vec![resources.title.clone()])
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
