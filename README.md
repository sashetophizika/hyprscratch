# Hyprscratch
[![Static Badge](https://img.shields.io/badge/Hyprland-grey?style=for-the-badge&logo=hyprland&logoSize=auto)](https://github.com/hyprwm/Hyprland)
[![GitHub top language](https://img.shields.io/github/languages/top/sashetophizika/hyprscratch?style=for-the-badge&logo=rust&logoSize=auto&logoColor=black&color=brightgreen)](https://www.rust-lang.org/)
[![Crates.io Version](https://img.shields.io/crates/v/hyprscratch?style=for-the-badge&color=orange)](https://crates.io/crates/hyprscratch)
[![Crates.io Total Downloads](https://img.shields.io/crates/d/hyprscratch?style=for-the-badge&color=orange)](https://crates.io/crates/hyprscratch)
[![AUR Version](https://img.shields.io/aur/version/hyprscratch?style=for-the-badge&color=blue)](https://aur.archlinux.org/packages/hyprscratch)
[![AUR Votes](https://img.shields.io/aur/votes/hyprscratch?style=for-the-badge&color=blue)](https://aur.archlinux.org/packages/hyprscratch)

Hyprscratch makes scratchpads in Hyprland painless in a well-integrated and flexible way.

## Features
* Scratchpads are part of your current workspace by default.
* Can be configured in `hyprland.conf` or a dedicated configuration file.
* Makes special workspace scratchpads simpler to set up.
* Many options to easily tweak behaviour of scratchpads.
* Event listeners to automate scratchpad management.
* Allows cycling and restoring previous scratchpads.
* Supports many scratchpads active at the same time.

## Installation
### [Crates](https://crates.io/crates/hyprscratch):

```bash
cargo install hyprscratch
```
### [AUR](https://aur.archlinux.org/packages/hyprscratch):
```bash
paru -S hyprscratch
```

### [Nix](https://github.com/sashetophizika/hyprscratch/blob/master/NIX.md):

Nix installation options can be found [here](https://github.com/sashetophizika/hyprscratch/blob/master/NIX.md).

## Usage

### Basic
In `hyprland.conf`:

```bash
# Start the hyprscratch daemon
exec-once = hyprscratch init [DAEMON_OPTIONS]

# Configure scratchpads
bind = $mainMod, $KEY, exec, hyprscratch $CLIENT_TITLE "$HYPRLAND_EXEC_COMMAND" [SCRATCHPAD_OPTIONS]
```

Example scratchpad configuration:

```bash
exec-once = hyprscratch init clean eager

bind = $mainMod, b, exec, hyprscratch btop "[size 70% 80%] alacritty --title btop -e btop" persist pin
bind = $mainMod, f, exec, hyprscratch "Mozilla Firefox" firefox special monitor 1
```

### Optional Configuration File
If you consider it more convenient to use a separate configuration file, you can create a `~/.config/hypr/hyprscratch.conf` or `~/.config/hyprscratch/config.conf` and configure scratchpads in the following way:

```hyprlang
# Optional globals that apply to all scratchpads
daemon_options = clean
global_options = special
global_rules = size 90% 90%

scratchpad1 {
    # Mandatory fields
    command = command

    # At least one is mandatory, title takes priority
    title = title
    class = class

    # Optional fields
    options = option1 option2 option3
    rules = rule1;rule2;rule3
}

# Groups that manage multiple scratchpads are also supported
group:group1 {
    # Add the already defined scratchpad to the group
    name = scratchpad1

    # Define a new scratchpad that can also be used independently
    scratchpad2 {
        title = title
        command = command
    }
}
```

And in `hyprland.conf`, using the given name:

```bash
exec-once = hyprscratch init

bind = $mainMod, t, hyprscratch toggle scratchpad1
bind = $mainMod, s, hyprscratch show scratchpad1
bind = $mainMod, h, hyprscratch hide scratchpad1

bind = $mainMod, g, hyprscratch toggle group1
```

Using a configuration file can be combined with normally configured scratchpads.

## Options:

### Daemon options:

* `clean`: automatically hides all scratchpads on workspace change.

* `spotless`: automatically hides all scratchpads on focus change.

* `eager`: spawns all scratchpads hidden on start.

* `no-auto-reload`: does not reload the configuration when the files are modified.

* `config </path/to/config>`: specify a path to the configuration file.

### Scratchpad options:

* `ephemeral`: closes the scratchpad when it is hidden.

* `persist`: prevents the scratchpad from getting replaced when a new one is summoned.

* `cover`: prevents the scratchpad from replacing an already active one.

* `sticky`: prevents the scratchpad from being hidden by `clean`.

* `shiny`: prevents the scratchpad from being hidden by `spotless`.

* `lazy`: prevents the scratchpad from being spawned by `eager`.

* `show`: only creates or brings up the scratchpad.

* `hide`: only hides the scratchpad if active.

* `poly`: toggles all scratchpads with the same title simultaneously.

* `pin`: keeps the scratchpad active through workspace changes.

* `tiled`: spawns the scratchpad tiled instead of floating.

* `monitor <id|name>`: restricts the scratchpad to a specific monitor.

* `special`: uses the special workspace. Does not work with all other options.

### Extra subcommands:

* `toggle <name>`: toggles the scratchpad with the given name.

* `show <name>`: shows the scratchpad with the given name.

* `hide <name>`: hides the scratchpad with the given name.

* `cycle [normal|special]`: cycles between scratchpads (optionally only normal or special ones) in the order they are defined in the configuration file.

* `previous [show|hide]`: toggles the last used scratchpad that is not currently active.

* `hide-all`: hides all scratchpads, useful mostly when stacking multiple of them.

* `kill-all`: closes all scratchpad clients that are open.

* `reload [config]`: re-parses the configuration file without restarting the daemon.

* `menu [fzf|rofi] [show|hide]`: spawns a menu to search through and trigger scratchpads (`rofi` for keybinds, `fzf` for cli).

* `get-config`: prints out the parsed configuration.

* `kill`: kills the hyprscratch daemon.

* `logs`: shows logs.


## Other Relevant Information
The title of a scratchpad has to be either the `initialTitle` or `initialClass` field of the client, which can be found with `hyprctl clients`. An incorrect title results in the scratchpad not being hidden and a new one being spawned every time.

Terminal applications often all use the title of the terminal emulator. Usually the title can be set with the `--title` flag to differentiate them.

Multiple commands can be bound to a single scratchpad by separating them with `?` (e.g. `hyprscratch title "[rules1] command1 ? [rules2] command2"`). Can be useful with the `poly` option to allow different window rules for each client.

If there are multiple clients with the same title, the program just grabs the first one it finds. This usually results in cycling between them, every time one is shown.
