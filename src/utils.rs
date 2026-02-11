use crate::config::Config;
use crate::scratchpad::Scratchpad;
use crate::DEFAULT_SOCKET;
use crate::{logs::*, KNOWN_CLI_COMMANDS};
use hyprland::data::{Client, Clients, Monitors};
use hyprland::dispatch::*;
use hyprland::prelude::*;
use hyprland::Result;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

pub fn warn_deprecated(feature: &str) -> Result<()> {
    log(format!("The '{feature}' feature is deprecated."), Warn)?;
    println!("Try 'hyprscratch help' and change your configuration before it is removed.");
    Ok(())
}

pub fn read_into_string(stream: &mut UnixStream) -> Result<String> {
    let mut buf = [0; 2048];
    let count = stream.read(&mut buf)?;
    let list = String::from_utf8(buf[..count].to_vec()).unwrap_or("invalid utf-8".into());
    Ok(list)
}

fn is_flag<'a>(arg: &str, flag: &&'a str) -> Option<&'a str> {
    let long = format!("--{flag}");
    let is_short = |x: &str| {
        x.len() > 1
            && !x.contains('=')
            && x.starts_with('-')
            && !x[1..].starts_with('-')
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
    None
}

pub fn get_flag_name<'a>(arg: &str) -> Option<&'a str> {
    let flags = &KNOWN_CLI_COMMANDS;
    if flags.is_empty() {
        return None;
    }

    for flag in flags {
        if flag.is_empty() {
            continue;
        }

        let f = is_flag(arg, flag);
        if f.is_some() {
            return f;
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

pub fn send_request(socket: Option<&str>, req: &str, msg: &str) -> Result<()> {
    let mut stream = UnixStream::connect(socket.unwrap_or(DEFAULT_SOCKET))?;
    stream.write_all(format!("{req}?{msg}").as_bytes())?;
    Ok(())
}

pub fn move_to_special(cl: &Client, workspace: &str) {
    if cl.pinned {
        hyprland::dispatch!(
            TogglePinWindow,
            WindowIdentifier::Address(cl.address.clone())
        )
        .log_err(file!(), line!());
    }

    hyprland::dispatch!(
        MoveToWorkspaceSilent,
        WorkspaceIdentifierWithSpecial::Special(Some(workspace)),
        Some(WindowIdentifier::Address(cl.address.clone()))
    )
    .unwrap_or_else(|e| {
        log(format!("MoveToSpecial returned Err: {e}"), Debug).unwrap();
    });
}

pub fn is_known(titles: &[String], cl: &Client) -> bool {
    titles.contains(&cl.initial_title) || titles.contains(&cl.initial_class)
}

pub fn is_known_map(map: &HashMap<String, String>, cl: &Client) -> bool {
    map.contains_key(&cl.initial_title) || map.contains_key(&cl.initial_class)
}

pub fn auto_hide(cl: &Client, title_map: &HashMap<String, String>) {
    if title_map.contains_key(&cl.initial_title) {
        move_to_special(cl, &title_map[&cl.initial_title]);
    } else if title_map.contains_key(&cl.initial_class) {
        move_to_special(cl, &title_map[&cl.initial_class]);
    }
}

pub fn hide_special(cl: &Client) {
    if let Some(("special", workspace)) = cl.workspace.name.split_once(":") {
        hyprland::dispatch!(ToggleSpecialWorkspace, Some(workspace.into()))
            .log_err(file!(), line!());
    }
}

pub fn is_on_special(cl: &Client) -> bool {
    cl.workspace.name.contains("special")
}

pub fn move_floating(titles: &HashMap<String, String>) -> Result<()> {
    Clients::get()?
        .iter()
        .filter(|cl| cl.floating && !is_on_special(cl))
        .for_each(|cl| auto_hide(cl, titles));
    Ok(())
}

fn prepend(command: &str, rules: &str) -> String {
    if rules.is_empty() {
        return command.into();
    }

    let mut rules = rules.to_owned();
    if !rules.trim().starts_with('[') {
        rules.insert(0, '[');
    }

    if command.trim().starts_with('[') {
        if !rules.ends_with(';') {
            rules.push(';');
        }
        command.replacen('[', &(rules + " "), 1)
    } else {
        format!("{rules}] {command}")
    }
}

pub fn prepend_rules(command: &str, rules: &str) -> Vec<String> {
    command.split('?').map(|c| prepend(c, rules)).collect()
}

pub fn prepare_commands(sc: &Scratchpad, on_special: Option<bool>, workspace: &str) -> Vec<String> {
    let mut rules = String::from("[");
    if let Some(sil) = on_special {
        let silent = if sil { "silent" } else { "" };
        rules += &format!("workspace special:{} {silent};", &workspace);
    }

    if sc.options.pin && on_special.is_none() {
        rules += "pin;";
    }

    if !sc.options.tiled {
        rules += "float;";
    }

    prepend_rules(&sc.command, &rules)
}

pub fn autospawn(config: &mut Config) -> Result<()> {
    let spawn = |(n, sc): (&String, &Scratchpad)| {
        prepare_commands(sc, Some(true), n)
            .iter()
            .for_each(|cmd| hyprland::dispatch!(Exec, &cmd).log_err(file!(), line!()));
    };

    let clients = Clients::get()?;
    config
        .scratchpads
        .iter()
        .filter(|(_, sc)| !sc.options.lazy && !clients.iter().any(|cl| sc.matches_client(cl)))
        .for_each(spawn);

    Ok(())
}

/// Evaluate a size/position expression like "monitor_w*0.95", "30%", "800"
fn eval_expr(expr: &str, monitor_w: f64, monitor_h: f64, is_width: bool) -> Option<i64> {
    let expr = expr.trim();
    if expr.is_empty() {
        return None;
    }

    // Percentage: "30%"
    if let Some(pct) = expr.strip_suffix('%') {
        let p = pct.trim().parse::<f64>().ok()?;
        let base = if is_width { monitor_w } else { monitor_h };
        return Some((base * p / 100.0) as i64);
    }

    // Expression with monitor variables: "monitor_w*0.95"
    if expr.contains("monitor_w") || expr.contains("monitor_h") {
        let replaced = expr
            .replace("monitor_w", &monitor_w.to_string())
            .replace("monitor_h", &monitor_h.to_string());

        // Handle multiplication: "2560*0.95"
        if let Some((a, b)) = replaced.split_once('*') {
            let a = a.trim().parse::<f64>().ok()?;
            // Handle subtraction after multiplication: "2560*0.95-60"
            if let Some((mul, sub)) = b.trim().split_once('-') {
                let mul = mul.trim().parse::<f64>().ok()?;
                let sub = sub.trim().parse::<f64>().ok()?;
                return Some((a * mul - sub) as i64);
            }
            let b = b.trim().parse::<f64>().ok()?;
            return Some((a * b) as i64);
        }

        // Just a variable reference: "monitor_w"
        return replaced.trim().parse::<f64>().ok().map(|v| v as i64);
    }

    // Plain number
    expr.parse::<f64>().ok().map(|v| v as i64)
}

/// Reapply scratchpad rules (size, position, float) to an existing window.
/// Called after showing or refocusing a scratchpad so rules are enforced
/// even if the window was previously resized or moved.
pub fn reapply_rules(client: &Client, rules: &str, should_float: bool) {
    let (monitor_w, monitor_h) = match get_focused_monitor_dimensions() {
        Some(dims) => dims,
        None => return,
    };

    let addr = client.address.clone();

    // Ensure floating if the scratchpad isn't configured as tiled
    if should_float && !client.floating {
        Dispatch::call(DispatchType::ToggleFloating(Some(
            WindowIdentifier::Address(addr.clone()),
        )))
        .log_err(file!(), line!());
    }

    if rules.is_empty() {
        return;
    }

    // Parse rules to find the final size/position state.
    // Later rules override earlier ones (per-scratchpad overrides global).
    let mut final_size: Option<(i64, i64)> = None;
    let mut final_center = false;
    let mut final_move: Option<(i64, i64)> = None;

    for rule in rules.split(';') {
        let rule = rule.trim();
        if rule.is_empty() {
            continue;
        }

        if rule == "float" {
            // Already handled above
        } else if let Some(size_args) = rule.strip_prefix("size ") {
            let parts: Vec<&str> = size_args.trim().splitn(2, char::is_whitespace).collect();
            if parts.len() == 2 {
                if let (Some(w), Some(h)) = (
                    eval_expr(parts[0], monitor_w, monitor_h, true),
                    eval_expr(parts[1], monitor_w, monitor_h, false),
                ) {
                    final_size = Some((w, h));
                    // New size invalidates previous center/move
                    final_center = false;
                    final_move = None;
                }
            }
        } else if let Some(move_args) = rule.strip_prefix("move ") {
            let parts: Vec<&str> = move_args.trim().splitn(2, char::is_whitespace).collect();
            if parts.len() == 2 {
                if let (Some(x), Some(y)) = (
                    eval_expr(parts[0], monitor_w, monitor_h, true),
                    eval_expr(parts[1], monitor_w, monitor_h, false),
                ) {
                    final_move = Some((x, y));
                    final_center = false;
                }
            }
        } else if rule == "center" {
            final_center = true;
            final_move = None;
        }
    }

    // Apply the final computed state
    if let Some((w, h)) = final_size {
        Dispatch::call(DispatchType::Custom(
            "resizewindowpixel",
            &format!("exact {} {},address:{}", w, h, addr),
        ))
        .log_err(file!(), line!());
    }

    if let Some((x, y)) = final_move {
        Dispatch::call(DispatchType::Custom(
            "movewindowpixel",
            &format!("exact {} {},address:{}", x, y, addr),
        ))
        .log_err(file!(), line!());
    } else if final_center {
        // centerwindow operates on the focused window (no address targeting)
        Dispatch::call(DispatchType::Custom("centerwindow", "1"))
            .log_err(file!(), line!());
    }
}

fn get_focused_monitor_dimensions() -> Option<(f64, f64)> {
    let monitors = Monitors::get().ok()?;
    let monitor = monitors.into_iter().find(|m| m.focused)?;
    Some((monitor.width as f64, monitor.height as f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigCache;
    use crate::scratchpad::Scratchpad;
    use hyprland::data::{Client, Workspace};
    use std::collections::HashMap;
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
                    hyprland::dispatch!(CloseWindow, WindowIdentifier::Title(&title)).unwrap();
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
            .map(|title| assert!(!clients.clone().any(|x| x.initial_title == title)));

        resources
            .commands
            .clone()
            .map(|command| hyprland::dispatch!(Exec, &command).unwrap());
        sleep(Duration::from_millis(1000));

        clients = Clients::get().unwrap().into_iter();
        resources
            .titles
            .clone()
            .map(|title| assert!(clients.clone().any(|x| x.initial_title == title)));

        let titles = HashMap::from([(
            "test_scratchpad_move".to_string(),
            "test_scratchpad_move".to_string(),
        )]);
        move_floating(&titles).unwrap();
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
                String::new(),
            ],
            spawned: [1, 1, 0],
        };

        let mut clients = Clients::get().unwrap().into_iter();
        resources
            .titles
            .clone()
            .map(|title| assert!(!clients.clone().any(|x| x.initial_title == title)));

        let options = vec![
            String::new(),
            "special eager".to_string(),
            "lazy".to_string(),
        ];

        let scratchpads: HashMap<String, Scratchpad> = resources
            .titles
            .iter()
            .zip(resources.commands.clone())
            .zip(options)
            .map(|((t, c), o)| (t.clone(), Scratchpad::new(t, &c, &o)))
            .collect();

        let mut config = Config {
            daemon_options: String::new(),
            config_file: String::new(),
            scratchpads,
            groups: HashMap::new(),
            names: Vec::new(),
            cache: ConfigCache::new(&HashMap::new()),
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
