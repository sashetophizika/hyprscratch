use crate::logs::LogErr;
use crate::utils::move_to_special;
use hyprland::data::{Client, Clients, FullscreenMode, Workspace};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use std::io::prelude::*;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;

struct Options {
    summon: bool,
    hide: bool,
    poly: bool,
    cover: bool,
    stack: bool,
    shiny: bool,
    special: bool,
}

impl Options {
    fn new(opts: &str) -> Options {
        Options {
            summon: opts.contains("summon"),
            hide: opts.contains("hide"),
            poly: opts.contains("poly"),
            cover: opts.contains("cover"),
            stack: opts.contains("stack"),
            shiny: opts.contains("shiny"),
            special: opts.contains("special"),
        }
    }
}

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

fn summon_special(title: &str, command: &str, state: &HyprlandState) -> Result<()> {
    let special_with_title: Vec<&Client> = state
        .clients_with_title
        .iter()
        .filter(|x| x.workspace.id < 0)
        .collect();

    if special_with_title.is_empty() && !state.clients_with_title.is_empty() {
        move_to_special(&state.clients_with_title[0])?;

        if state.clients_with_title[0].workspace.id == state.active_workspace.id {
            hyprland::dispatch!(ToggleSpecialWorkspace, Some(title.to_string()))?;
        }
    } else if state.clients_with_title.is_empty() {
        let mut special_cmd = command.to_string();
        if special_cmd.find('[').is_none() {
            special_cmd.insert_str(0, "[]");
        }

        special_cmd = special_cmd.replacen('[', &format!("[workspace special:{title}; "), 1);
        hyprland::dispatch!(Exec, &special_cmd)?;
    } else {
        hyprland::dispatch!(ToggleSpecialWorkspace, Some(title.to_string()))?;
    }
    Ok(())
}

fn summon_normal(command: &str, options: &Options, state: &HyprlandState) -> Result<()> {
    if state.clients_with_title.is_empty() {
        command
            .split("?")
            .for_each(|x| hyprland::dispatch!(Exec, &x).unwrap_log(file!(), line!()));
    } else {
        for client in state
            .clients_with_title
            .iter()
            .filter(|x| x.workspace.id != state.active_workspace.id)
        {
            hyprland::dispatch!(
                MoveToWorkspaceSilent,
                WorkspaceIdentifierWithSpecial::Relative(0),
                Some(WindowIdentifier::Address(client.address.clone()))
            )
            .unwrap_log(file!(), line!());
            if !options.poly {
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

fn summon(title: &str, command: &str, options: &Options, state: &HyprlandState) -> Result<()> {
    if options.special {
        summon_special(title, command, state)?;
    } else if !options.hide {
        summon_normal(command, options, state)?;
    }
    Ok(())
}

fn hide_active(options: &Options, titles: &str, active_client: &Client) -> Result<()> {
    if !options.cover
        && !options.stack
        && active_client.floating
        && titles.contains(&active_client.initial_title)
    {
        move_to_special(active_client)?;
    }
    Ok(())
}

fn remove_temp_shiny(title: &str, options: &Options, socket: Option<&str>) -> Result<()> {
    if !options.shiny {
        let mut stream =
            UnixStream::connect(socket.unwrap_or("/tmp/hyprscratch/hyprscratch.sock"))?;
        stream.write_all(format!("return?{title}").as_bytes())?;
        stream.shutdown(Shutdown::Write)?;
    }
    Ok(())
}

pub fn scratchpad(title: &str, command: &str, opts: &str, socket: Option<&str>) -> Result<()> {
    let mut stream = UnixStream::connect(socket.unwrap_or("/tmp/hyprscratch/hyprscratch.sock"))?;
    stream.write_all(format!("scratchpad?{title}").as_bytes())?;
    stream.shutdown(Shutdown::Write)?;

    let mut titles = String::new();
    stream.read_to_string(&mut titles)?;

    let options = Options::new(opts);
    let state = HyprlandState::new(title);

    if let Some(active_client) = Client::get_active()? {
        let mut clients_on_active = state
            .clients_with_title
            .clone()
            .into_iter()
            .filter(|x| x.workspace.id == state.active_workspace.id)
            .peekable();

        if options.special || clients_on_active.peek().is_none() {
            summon(title, command, &options, &state)?;
            hide_active(&options, &titles, &active_client)?;
        } else if (!active_client.floating
            || active_client.initial_title == title
            || active_client.fullscreen == FullscreenMode::None)
            && !options.summon
        {
            clients_on_active.for_each(|x| move_to_special(&x).unwrap());
        } else {
            hyprland::dispatch!(
                FocusWindow,
                WindowIdentifier::Address(clients_on_active.peek().unwrap().address.clone())
            )?;
        }
    } else {
        summon(title, command, &options, &state)?;
    }

    Dispatch::call(DispatchType::BringActiveToTop)?;
    remove_temp_shiny(title, &options, socket)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::daemon::initialize_daemon;
    use crate::kill;

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
            &resources.command,
            &Options::new(""),
            &HyprlandState::new(""),
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);

        hide_active(&Options::new(""), &resources.title, &active_client).unwrap();
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
            &resources.command,
            &Options::new(""),
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
            command: "[float;size 30% 30%] kitty --title test_special_scratchpad".to_string(),
        };

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            false
        );

        summon_special(
            &resources.title,
            &resources.command,
            &HyprlandState::new(""),
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );

        summon_special(
            &resources.title,
            &resources.command,
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
            &resources.title,
            &resources.command,
            &HyprlandState::new(&resources.title),
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);

        hide_active(&Options::new("cover"), &resources.title, &active_client).unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);
    }

    #[test]
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

        summon_normal(
            &resources.command,
            &Options::new(""),
            &HyprlandState::new(""),
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        let active_client = Client::get_active().unwrap().unwrap();
        assert_eq!(active_client.initial_title, resources.title);

        hide_active(&Options::new(""), "", &active_client).unwrap();
        sleep(Duration::from_millis(1000));

        assert!(Clients::get()
            .unwrap()
            .into_iter()
            .filter(|x| x.workspace.id == Workspace::get_active().unwrap().id)
            .any(|x| x.initial_title == resources.title));
    }

    #[test]
    fn test_poly() {
        std::thread::spawn(|| {
            initialize_daemon(
                "".to_string(),
                Some("test_configs/test_config3.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            )
        });
        sleep(Duration::from_millis(1000));

        let socket = Some("/tmp/hyprscratch_test.sock");
        let resources = TestResources {
            title: "test_poly".to_string(),
            command: "[float;size 30% 30%; move 0 0] kitty --title test_poly ? [float;size 30% 30%; move 30% 0] kitty --title test_poly".to_string(),
        };

        assert_eq!(
            Clients::get()
                .unwrap()
                .iter()
                .any(|x| x.initial_title == resources.title),
            false
        );

        scratchpad(&resources.title, &resources.command, "poly", socket).unwrap();
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

        scratchpad(&resources.title, &resources.command, "poly", socket).unwrap();
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
        kill(socket).unwrap();
        sleep(Duration::from_millis(1000));
    }

    #[test]
    fn test_summon_hide() {
        std::thread::spawn(|| {
            initialize_daemon(
                "".to_string(),
                Some("test_configs/test_config3.txt".to_string()),
                Some("/tmp/hyprscratch_test.sock"),
            )
        });
        sleep(Duration::from_millis(500));

        let socket = Some("/tmp/hyprscratch_test.sock");
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

        scratchpad(
            &resources.title,
            &resources.command,
            "summon",
            socket.clone(),
        )
        .unwrap();
        sleep(Duration::from_millis(1000));

        assert_eq!(
            Client::get_active().unwrap().unwrap().initial_title,
            resources.title
        );

        scratchpad(
            &resources.title,
            &resources.command,
            "summon",
            socket.clone(),
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

        scratchpad(&resources.title, &resources.command, "hide", socket.clone()).unwrap();
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

        scratchpad(&resources.title, &resources.command, "hide", socket.clone()).unwrap();
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
        kill(socket).unwrap();
        sleep(Duration::from_millis(1000));
    }
}
