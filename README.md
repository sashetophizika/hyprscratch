# Hyprscratch

A scratchpad utility for Hyprland

## Installation
### Cargo:

```
cargo install hyprscratch
```
### AUR:
```
paru -S hyprscratch
```

## Usage
In `hyprland.conf`:

```bash
exec-once = hyprscratch [DAEMON_OPTIONS] #start the hyprscratch daemon

bind = $MOD, $KEY, exec, hyprscratch $WINDOW_TITLE "$HYPRLAND_EXEC_COMMAND" [SCRATCHPAD_OPTIONS] #configure scratchpads
```

Example scratchpad:

```bash
bind = $mainMod, b, exec, hyprscratch btop "[float;size 70% 80%;center] alacritty --title btop -e btop" onstart
```

### Daemon options:

* `clean [spotless]`: starts the daemon and hides all scratchpads on workspace change. The `spotless` option also hides them on losing focus to non-floating windows.

### Scratchpad options:

* `stack`: makes it so that the scratchpad doesn't hide one that is already present. This can be used to group multiple scratchpads by binding them to the same key and using `stack` on all except the first one. 

* `shiny`: makes it so that the scratchpad is not hidden by `clean spotless`.

* `onstart`: spawns the scratchpad at the start of the Hyprland session.

* `summon`: only creates or brings up the scratchpad.

* `hide`: only hides the scrachpad.

* `special`: uses the special workspace. Ignores all other scratchpad options and is ignored by `clean spotless` and `cycle`.

### Extra hyprscratch commands:

* `cycle`: cycles between non-special scratchpads in the order they are defined in the config file.

* `hideall`: hides all scratchpads, useful mostly for stacked ones.

* `reload`: reparses changes to the config file without restarting the daemon.

* `get-config`: prints out the parsed config, useful for debugging potential syntax issues.

## Other Relevant information
To find the title needed for a scratchpad, run `hyprctl clients` and check the `initialTitle` field. An incorrect title results in the scratchpad not being hidden and a new one being spawned instead.

Terminal applications often all use the title of the terminal emulator. Usually the title can be set with the `--title` flag to differentiate them.

If there are multiple scratchpads with the same initial title, the program just grabs the first one it finds.

Scratchpads don't have to be floating. This can also be used to just spawn a specific window, where the binding also hides it or grabs it from another workspace. Non-floating scratchpads are ignored by `clean`.

The program doesn't use hyprland's special workspace by default, it uses workspace 42.
