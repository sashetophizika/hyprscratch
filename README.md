# Hyprscratch
A small tool for Qtile-like scratchpads in Hyprland or simplifying usage of the built-in functionality, that can be configured entirely inside of `hyprland.conf`.

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

```bash
#start the hyprscratch daemon
exec-once = hyprscratch init [DAEMON_OPTIONS]

#configure scratchpads
bind = $MOD, $KEY, exec, hyprscratch $CLIENT_TITLE "$HYPRLAND_EXEC_COMMAND" [SCRATCHPAD_OPTIONS]
```

Example scratchpad:

```bash
bind = $mainMod, b, exec, hyprscratch btop "[float;size 70% 80%;center] alacritty --title btop -e btop" eager
```

## Configuration

### Daemon options:

* `clean`: automatically hides all scratchpads on workspace change.

* `spotless`: automatically hides all scratchpads on focus change.

* `no-auto-reload`: does not reload the configuration when the configuration file is updated.

* `config /path/to/config`: specify a path to the configuration file

### Scratchpad options:

* `persist`: makes it so that the scratchpad doesn't get replaced when a new one is summoned.

* `cover`: makes it so that the scratchpad doesn't replace another one if one is already present.

* `sticky`: makes it so that the scratchpad isn't hidden by `clean`.

* `shiny`: makes it so that the scratchpad isn't hidden by `spotless`.

* `eager`: spawns the scratchpad hidden when the daemon is started.

* `summon`: only creates or brings up the scratchpad.

* `hide`: only hides the scratchpad.

* `poly`: toggle all scratchpads with the same title

* `special`: uses the special workspace. Ignores most other scratchpad options and is ignored by `clean` and `spotless`.

### Extra subcommands:

* `cycle [normal|special]`: cycles between scratchpads (optionally only normal or special ones) in the order they are defined in the configuration file.

* `toggle name`: toggles the scratchpad with the given name

* `summon name`: summons the scratchpad with the given name

* `hide name`: hides the scratchpad with the given name

* `previous`: summon the last used scratchpad that is not currently active.

* `hide-all`: hides all scratchpads, useful mostly when stacking multiple of them.

* `kill-all`: closes all scratchpad clients that are open

* `reload`: re-parses the configuration file without restarting the daemon.

* `get-config`: prints out the parsed configuration.

* `kill`: kills the hyprscratch daemon

* `logs`: show logs

### Optional Configuration File
If you consider it more convenient to use a separate configuration file, you can create a `~/.config/hyprscratch/config.conf` or `~/.config/hypr/hyprscratch.conf` and configure scratchpads in the following way:

```py
name = {
#Mandatory fields
title = title                        
command = command

#Optional fields
options = option1 option2 option3
rules = rule1;rule2;rule3
}
```

And in `hyprland.conf`:

```
exec-once = hyprscratch init

bind = $mainMod, t, hyprscratch toggle name
bind = $mainMod, s, hyprscratch summon name
bind = $mainMod, h, hyprscratch hide name
```

## Other Relevant Information
To find the title needed for a scratchpad, run `hyprctl clients` and check the `initialTitle` field. An incorrect title results in the scratchpad not being hidden and a new one being spawned instead.

To group multiple scratchpads together, bind them to the same key and use `stack` on all of them. 

Terminal applications often all use the title of the terminal emulator. Usually the title can be set with the `--title` flag to differentiate them.

If there are multiple scratchpads with the same initial title, this program just grabs the first one it finds.

Scratchpads don't have to be floating. This can also be used to just spawn a specific client, where the binding also hides it or grabs it from another workspace. Non-floating scratchpads are ignored by `clean`.
