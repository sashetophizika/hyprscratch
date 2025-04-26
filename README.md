# Hyprscratch
A simple tool for Qtile-like scratchpads in Hyprland or simplifying usage of the built-in functionality, that can be configured entirely inside of `hyprland.conf`.

## Installation
### [Cargo](https://crates.io/crates/hyprscratch):

```
cargo install hyprscratch
```
### [AUR](https://aur.archlinux.org/packages/hyprscratch):
```
paru -S hyprscratch
```

## Usage
In `hyprland.conf`:

```hyprlang
#Start the hyprscratch daemon
exec-once = hyprscratch init [DAEMON_OPTIONS]

#Configure scratchpads
bind = $MOD, $KEY, exec, hyprscratch $CLIENT_TITLE "$HYPRLAND_EXEC_COMMAND" [SCRATCHPAD_OPTIONS]
```

Example scratchpad:

```hyprlang
bind = $mainMod, b, exec, hyprscratch btop "[size 70% 80%] alacritty --title btop -e btop" eager
```

## Configuration

### Daemon options:

* `clean`: automatically hides all scratchpads on workspace change.

* `spotless`: automatically hides all scratchpads on focus change.

* `eager`: spawns all scratchpads hidden on start.

* `no-auto-reload`: does not reload the configuration when `hyprland.conf` is updated.

* `config </path/to/config>`: specify a path to the configuration file.

### Scratchpad options:

* `persist`: prevents the scratchpad from getting replaced when a new one is summoned.

* `cover`: prevents the scratchpad from replacing an already active one.

* `sticky`: prevents the scratchpad from being hidden by `clean`.

* `shiny`: prevents the scratchpad from being hidden by `spotless`.

* `lazy`: prevents the scratchpad from being spawned by `eager`.

* `show`: only creates or brings up the scratchpad.

* `hide`: only hides the scratchpad if active.

* `poly`: toggle all scratchpads with the same title simultaneously.

* `pin`: keeps the scratchpad active through workspace changes

* `tiled`: spawns the scratchpad tiled instead of floating.

* `monitor <id>`: restricts the scratchpad to a specific monitor.

* `special`: uses the special workspace. Does not work with all other options.

### Extra subcommands:

* `cycle [normal|special]`: cycles between scratchpads (optionally only normal or special ones) in the order they are defined in the configuration file.

* `toggle <name>`: toggles the scratchpad with the given name.

* `show <name>`: shows the scratchpad with the given name.

* `hide <name>`: hides the scratchpad with the given name.

* `previous`: summons the last used scratchpad that is not currently active.

* `hide-all`: hides all scratchpads, useful mostly when stacking multiple of them.

* `kill-all`: closes all scratchpad clients that are open.

* `reload`: re-parses the configuration file without restarting the daemon.

* `get-config`: prints out the parsed configuration.

* `kill`: kills the hyprscratch daemon.

* `logs`: shows logs.

### Optional Configuration File
If you consider it more convenient to use a separate configuration file, you can create a  `~/.config/hypr/hyprscratch.conf` or `~/.config/hyprscratch/config.conf` and configure scratchpads in the following way:

```hyprlang
name {
#Mandatory fields
title = title                        
command = command

#Optional fields
options = option1 option2 option3
rules = rule1;rule2;rule3
}
```

And in `hyprland.conf`:

```hyprlang
exec-once = hyprscratch init

bind = $mainMod, t, hyprscratch toggle name
bind = $mainMod, s, hyprscratch show name
bind = $mainMod, h, hyprscratch hide name
```

## Other Relevant Information
To find the title needed for a scratchpad, run `hyprctl clients` and check the `initialTitle` field. An incorrect title results in the scratchpad not being hidden and a new one being spawned every time.

To group multiple scratchpads together, bind them to the same key and use `cover` and `persist` on all of them. 

Terminal applications often all use the title of the terminal emulator. Usually the title can be set with the `--title` flag to differentiate them.

If there are multiple scratchpads with the same title, the program just grabs the first one it finds.
