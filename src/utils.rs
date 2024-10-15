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
                    Some(WindowIdentifier::Address(x.address.clone()))
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

    #[test]
    fn test_move_floating() {
        let mut cls = Clients::get().unwrap().into_iter();
        assert_eq!(cls.any(|x| x.initial_title == "test_nonfloating"), false);
        assert_eq!(cls.any(|x| x.initial_title == "test_notcontained"), false);
        assert_eq!(cls.any(|x| x.initial_title == "test_scratchpad"), false);

        hyprland::dispatch!(Exec, "kitty --title test_nonfloating").unwrap();
        hyprland::dispatch!(
            Exec,
            "[float;size 30% 30%;move 0 0] kitty --title test_notcontained"
        )
        .unwrap();
        hyprland::dispatch!(
            Exec,
            "[float;size 30% 30%;move 30% 0] kitty --title test_scratchpad"
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2000));

        cls = Clients::get().unwrap().into_iter();
        assert_eq!(
            cls.clone().any(|x| x.initial_title == "test_nonfloating"),
            true
        );
        assert_eq!(
            cls.clone().any(|x| x.initial_title == "test_notcontained"),
            true
        );
        assert_eq!(
            cls.clone().any(|x| x.initial_title == "test_scratchpad"),
            true
        );

        move_floating(vec![
            "test_nonfloating".to_owned(),
            "test_scratchpad".to_owned(),
        ]);
        std::thread::sleep(std::time::Duration::from_millis(500));

        cls = Clients::get().unwrap().into_iter();
        let clnf: Vec<Client> = cls
            .clone()
            .filter(|x| x.initial_title == "test_nonfloating")
            .collect();
        let clnc: Vec<Client> = cls
            .clone()
            .filter(|x| x.initial_title == "test_notcontained")
            .collect();
        let clns: Vec<Client> = cls
            .filter(|x| x.initial_title == "test_scratchpad")
            .collect();

        assert_eq!(clnf.len(), 1);
        assert_eq!(clnc.len(), 1);
        assert_eq!(clns.len(), 1);

        let aw = Workspace::get_active().unwrap();
        assert_eq!(clnf[0].workspace.id, aw.id);
        assert_eq!(clnc[0].workspace.id, aw.id);
        assert_eq!(clns[0].workspace.id, 42);

        hyprland::dispatch!(CloseWindow, WindowIdentifier::Title("test_nonfloating")).unwrap();
        hyprland::dispatch!(CloseWindow, WindowIdentifier::Title("test_notcontained")).unwrap();
        hyprland::dispatch!(CloseWindow, WindowIdentifier::Title("test_scratchpad")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}
