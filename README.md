# Hyprscratch

A scratchpad utility for Hyprland

## Installation
Using cargo (Make sure `~/.cargo/bin` is in $PATH)
`cargo install hyprscratch`

## Usage
In `hyprland.conf`:

`bind = $MOD, $KEY, exec, hyprscratch $WINDOW_TITLE "$HYPRLAND_EXEC_COMMAND"`

For example:
`bind = $mainmod, b, exec, hyprscratch btop "[float;size 70% 60%] kitty -e btop"`

You can optionally append `stack` to the end of the line so that the new scratchpad doesn't hide the old one. If you like stacking scratchpads, there is a command `hyprscratch hideall` that you can call to hide all scratchpads.

The scratchpads are just floating windows so by default they remain on the workspace they are spawned if not explicitly hidden. To hide them on workspace change add:
`exec-once = hyprscratch clean`

## Other Relevant information
Scratchpads don't have to be floating. This can also be used to just spawn a specific window, where using the key binding again hides it or grabs it from another workspace (or focuses it if it's on the current workspace).

The program assumes that you don't use floating windows outside of scratchpads. For example, spawning a scratchpad, while you have a floating window focused will hide by default. The same is true for `clean` and `hideall`.

The program doesn't use the special workspace, it uses workspace 42. If you lose a window, it's there.

There are some bugs and I blame Hyprland for them. I have noticed Thunar refusing to tile and most windows not spawing in the center the first time they are spawned in a session. 
