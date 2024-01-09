# Hyprscratch

A scratchpad utility for Hyprland

## Installation
Using cargo (Make sure `~/.cargo/bin` is in $PATH)

```
cargo install hyprscratch
```

## Usage
In `hyprland.conf`:

```bash
bind = $MOD, $KEY, exec, ~/.cargo/bin/hyprscratch $WINDOW_TITLE "$HYPRLAND_EXEC_COMMAND" [OPTIONS]
```

For example:

```bash
bind = $mainNod, b, exec, ~/.cargo/bin/hyprscratch btop "[float;size 70% 80%;center] kitty -e btop"
```

You can optionally append `stack` to the end of the line so that the new scratchpad doesn't hide the old one. If you like stacking scratchpads, there is a command `hyprscratch hideall` that you can call to hide all scratchpads.


The scratchpads are just floating windows so by default they remain on the workspace they are spawned if not explicitly hidden. To hide them on workspace change add:
```bash
exec-once = ~/.cargo/bin/hyprscratch clean
```
To also hide when losing focus to a non-floating window:
```bash
exec-once = ~/.cargo/bin/hyprscratch clean spotless
```

You can use the `shiny` option to prevent a specific scratchpad from being cleaned. Useful for graphical program where you would want to drag and drop.

## Other Relevant information
If there are multiple scratchpads with the same initial title, the program just grabs the first one it finds.

Scratchpads don't have to be floating. This can also be used to just spawn a specific window, where the binding also hides it or grabs it from another workspace. Non-floating scratchpads don't get cleaned.

If you want a scratchpad centered properly, the `center` option needs to be last.

The program doesn't use Hyprland's special workspace, it uses workspace 42. If you want to spawn a scratchpad on startup, spawn it there.
