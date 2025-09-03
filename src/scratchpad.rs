use crate::logs::*;
use crate::utils::*;
use hyprland::data::{Client, Clients, Monitors, Workspace};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use std::collections::HashMap;

struct HyprlandState {
    active_client: Option<Client>,
    clients_with_title: Vec<Client>,
    monitors: HashMap<String, String>,
    active_workspace: Workspace,
}

impl HyprlandState {
    fn new(title: &str) -> Result<HyprlandState> {
        let mut monitors = HashMap::new();
        Monitors::get()?.into_iter().for_each(|x| {
            monitors.insert(x.name.clone(), x.active_workspace.name.clone());
            monitors.insert(x.id.to_string(), x.active_workspace.name.clone());
        });

        let active_workspace = Workspace::get_active()?;

        let active_client = Client::get_active()?;
        let clients_with_title = Clients::get()?
            .into_iter()
            .filter(|x| x.initial_title == title || x.initial_class == title)
            .collect();

        Ok(HyprlandState {
            clients_with_title,
            active_workspace,
            active_client,
            monitors,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScratchpadOptions {
    options_string: String,
    pub monitor: Option<String>,
    pub ephemeral: bool,
    pub persist: bool,
    pub special: bool,
    pub sticky: bool,
    pub shiny: bool,
    pub cover: bool,
    pub tiled: bool,
    pub show: bool,
    pub hide: bool,
    pub poly: bool,
    pub lazy: bool,
    pub pin: bool,
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
            ephemeral: opts.contains("ephemeral"),
            persist: opts.contains("persist"),
            special: opts.contains("special"),
            sticky: opts.contains("sticky"),
            shiny: opts.contains("shiny"),
            cover: opts.contains("cover"),
            tiled: opts.contains("tiled"),
            show: opts.contains("summon") || opts.contains("show"),
            hide: opts.contains("hide"),
            poly: opts.contains("poly"),
            lazy: opts.contains("lazy"),
            pin: opts.contains("pin"),
            monitor: get_arg("monitor"),
        }
    }

    pub fn toggle(&mut self, opt: &str) {
        match opt {
            "persist" => self.persist ^= true,
            "ephemeral" => self.ephemeral ^= true,
            "special" => self.special ^= true,
            "summon" => self.show ^= true,
            "sticky" => self.sticky ^= true,
            "shiny" => self.shiny ^= true,
            "cover" => self.cover ^= true,
            "tiled" => self.tiled ^= true,
            "show" => self.show ^= true,
            "hide" => self.hide ^= true,
            "poly" => self.poly ^= true,
            "lazy" => self.lazy ^= true,
            "pin" => self.pin ^= true,
            _ => (),
        };
    }

    pub fn as_str(&self) -> &str {
        self.options_string.trim()
    }
    pub fn as_string(&self) -> String {
        self.options_string.trim().into()
    }
}

use TriggerMode::*;
#[derive(PartialEq, Debug)]
enum TriggerMode<T> {
    Hide(Vec<T>),
    Refocus(T),
    Summon,
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

    pub fn add_rules(&mut self, rules: &str) {
        self.command = prepend_rules(&self.command, rules).join("?");
    }

    pub fn add_opts(&mut self, options: &str) {
        if options.is_empty() {
            return;
        }
        self.options = ScratchpadOptions::new(&format!("{} {}", self.options.as_str(), options));
    }

    pub fn matches_client(&self, client: &Client) -> bool {
        let title = self.title.to_lowercase();
        if title == client.initial_title.to_lowercase()
            || title == client.initial_class.to_lowercase()
        {
            return true;
        }
        false
    }

    fn capture_special(&self, state: &HyprlandState) -> Result<()> {
        let first_title = &state.clients_with_title[0];
        move_to_special(first_title);

        if !self.options.hide && first_title.workspace.id == state.active_workspace.id {
            hyprland::dispatch!(ToggleSpecialWorkspace, Some(self.name.clone()))?;
        }
        Ok(())
    }

    fn toggle_special(&self, state: &HyprlandState) -> Result<()> {
        if let Some(ac) = &state.active_client {
            let should_toggle = (self.matches_client(ac) && !self.options.show)
                || (!self.matches_client(ac) && !self.options.hide);

            if should_toggle {
                hyprland::dispatch!(ToggleSpecialWorkspace, Some(self.name.clone()))?;
            }
        } else if !self.options.hide {
            hyprland::dispatch!(ToggleSpecialWorkspace, Some(self.name.clone()))?;
        }
        Ok(())
    }

    fn spawn_special(&self) {
        prepare_commands(
            &self.command,
            Some(&self.name),
            false,
            false,
            !self.options.tiled,
        )
        .iter()
        .for_each(|cmd| {
            hyprland::dispatch!(Exec, &cmd).log_err(file!(), line!());
        });
    }

    fn summon_special(&self, state: &HyprlandState) -> Result<()> {
        let special_with_title: Vec<&Client> = state
            .clients_with_title
            .iter()
            .filter(|cl| is_on_special(cl))
            .collect();

        if state.clients_with_title.is_empty() {
            self.spawn_special();
        } else if special_with_title.is_empty() {
            self.capture_special(state)?;
        } else {
            self.toggle_special(state)?;
        }
        Ok(())
    }

    fn get_workspace_name(&self, state: &HyprlandState) -> String {
        if let Some(m) = &self.options.monitor {
            state
                .monitors
                .get(m)
                .unwrap_or_else(|| {
                    let _ = log(format!("Monitor {m} not found"), Warn);
                    &state.active_workspace.name
                })
                .to_owned()
        } else {
            state.active_workspace.name.clone()
        }
    }

    fn spawn_normal(&self, state: &HyprlandState) {
        if let Some(ac) = &state.active_client {
            hide_special(ac);
        }

        prepare_commands(
            &self.command,
            None,
            false,
            self.options.pin,
            !self.options.tiled,
        )
        .iter()
        .for_each(|cmd| {
            hyprland::dispatch!(Exec, &cmd).log_err(file!(), line!());
        });
    }

    fn show_normal(&self, state: &HyprlandState) -> Result<()> {
        for client in state
            .clients_with_title
            .iter()
            .filter(|cl| !self.is_on_workspace(cl, state))
        {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Name(&self.get_workspace_name(state)),
                Some(WindowIdentifier::Address(client.address.clone()))
            )?;

            hyprland::dispatch!(
                FocusWindow,
                WindowIdentifier::Address(client.address.clone())
            )?;

            if self.options.pin {
                Dispatch::call(DispatchType::TogglePin)?;
            }

            if !self.options.poly {
                break;
            }
        }

        Ok(())
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

    fn hide_active(&self, titles: &[String], state: &HyprlandState) {
        if self.options.cover || self.options.hide {
            return;
        }

        let should_hide = |cl: &&Client| {
            is_known(titles, cl)
                && !self.matches_client(cl)
                && cl.workspace.id == state.active_workspace.id
                && cl.floating
        };

        if let Some(ac) = &state.active_client {
            if should_hide(&ac) {
                move_to_special(ac);
            }
        }
    }

    fn is_on_workspace(&self, client: &Client, state: &HyprlandState) -> bool {
        if self.options.monitor.is_some() {
            state
                .monitors
                .values()
                .any(|name| *name == client.workspace.name)
        } else {
            state.active_workspace.id == client.workspace.id
        }
    }

    fn get_mode<'a>(&self, state: &'a HyprlandState) -> TriggerMode<&'a Client> {
        let mut clients_on_active = state
            .clients_with_title
            .iter()
            .filter(|cl| self.is_on_workspace(cl, state) && (self.options.tiled || cl.floating))
            .peekable();

        if self.options.special || self.options.show {
            return Summon;
        }

        let active = match &state.active_client {
            Some(cl) => cl,
            None => return Summon,
        };

        match clients_on_active.peek() {
            Some(client) => {
                if self.options.hide || !active.floating || self.matches_client(active) {
                    Hide(clients_on_active.collect())
                } else {
                    Refocus(client)
                }
            }
            None => Summon,
        }
    }

    fn refocus(client: &Client) -> Result<()> {
        hyprland::dispatch!(
            FocusWindow,
            WindowIdentifier::Address(client.address.clone())
        )?;
        Ok(())
    }

    fn hide(&self, clients: Vec<&Client>) {
        if self.options.pin {
            Self::refocus(clients[0]).log_err(file!(), line!());
            Dispatch::call(DispatchType::TogglePin).log_err(file!(), line!());
        }

        clients.into_iter().for_each(move_to_special)
    }

    pub fn trigger(&mut self, titles: &[String]) -> Result<()> {
        let state = HyprlandState::new(&self.title)?;
        match self.get_mode(&state) {
            Refocus(client) => Self::refocus(client)?,
            Hide(clients) => self.hide(clients),
            Summon => self.summon(&state)?,
        }

        self.hide_active(titles, &state);
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

        move_to_special(&active_client);
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
        );
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
            .hide_active(&vec![], &HyprlandState::new(&resources.title).unwrap());
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

    #[test]
    fn test_named_workspace() {
        let resources = TestResources {
            title: "test_named_workspace".to_string(),
            command: "[size 30% 30%] kitty --title test_named_workspace".to_string(),
        };

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            false
        );

        hyprland::dispatch!(Workspace, WorkspaceIdentifierWithSpecial::Name("test")).unwrap();
        sleep(Duration::from_millis(500));
        assert_eq!(Workspace::get_active().unwrap().name, "test");

        Scratchpad::new(
            &resources.title,
            &resources.title,
            &resources.command,
            "summon",
        )
        .trigger(&vec![resources.title.clone()])
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );

        assert_eq!(Workspace::get_active().unwrap().name, "test");
        hyprland::dispatch!(Workspace, WorkspaceIdentifierWithSpecial::Id(1)).unwrap();
    }
}
