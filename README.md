# Hyprscratch
A small tool for Qtile-like scratchpads in Hyprland or simplifying usage of the built-in functionality, configured entirely inside of `hyprland.conf`.

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
exec-once = hyprscratch [DAEMON_OPTIONS] #start the hyprscratch daemon

bind = $MOD, $KEY, exec, hyprscratch $WINDOW_TITLE "$HYPRLAND_EXEC_COMMAND" [SCRATCHPAD_OPTIONS] #configure scratchpads
```

Example scratchpad:

```bash
bind = $mainMod, b, exec, hyprscratch btop "[float;size 70% 80%;center] alacritty --title btop -e btop" onstart
```

### Daemon options:

* `clean [spotless]`: automatically hides all scratchpads on workspace change. The `spotless` option also hides them on losing focus to non-floating windows.

### Scratchpad options:

* `stack`: makes it so that the scratchpad doesn't hide one that is already present. This can be used to group multiple scratchpads by binding them to the same key and using `stack` on all except the first one. 

* `shiny`: makes it so that the scratchpad is not hidden by `clean spotless`.

* `onstart`: spawns the scratchpad hidden when the daemon is started.

* `summon`: only creates or brings up the scratchpad.

* `hide`: only hides the scratchpad.

* `special`: uses the special workspace. Ignores all other scratchpad options and is ignored by `clean spotless` and `cycle`.

### Extra hyprscratch commands:

* `cycle`: cycles between non-special scratchpads in the order they are defined in the configuration file.

* `hideall`: hides all scratchpads, useful mostly when stacking multiple of them.

* `reload`: re-parses the configuration file without restarting the daemon.

* `get-config`: prints out the parsed configuration, useful for debugging potential syntax issues.

## Other Relevant information
The program doesn't use Hyprland's special workspace by default, it uses workspace 42.

To find the title needed for a scratchpad, run `hyprctl clients` and check the `initialTitle` field. An incorrect title results in the scratchpad not being hidden and a new one being spawned instead.

Terminal applications often all use the title of the terminal emulator. Usually the title can be set with the `--title` flag to differentiate them.

If there are multiple scratchpads with the same initial title, the program just grabs the first one it finds.

Scratchpads don't have to be floating. This can also be used to just spawn a specific window, where the binding also hides it or grabs it from another workspace. Non-floating scratchpads are ignored by `clean`.
