# Hyprscratch

[![Static Badge](https://img.shields.io/badge/Hyprland-grey?style=for-the-badge&logo=hyprland&logoSize=auto)](https://github.com/hyprwm/Hyprland)
[![GitHub top language](https://img.shields.io/github/languages/top/sashetophizika/hyprscratch?style=for-the-badge&logo=rust&logoSize=auto&logoColor=black&color=brightgreen)](https://www.rust-lang.org/)
[![Crates.io Version](https://img.shields.io/crates/v/hyprscratch?style=for-the-badge&color=orange)](https://crates.io/crates/hyprscratch)
[![Crates.io Total Downloads](https://img.shields.io/crates/d/hyprscratch?style=for-the-badge&color=orange)](https://crates.io/crates/hyprscratch)
[![AUR Version](https://img.shields.io/aur/version/hyprscratch?style=for-the-badge&color=blue)](https://aur.archlinux.org/packages/hyprscratch)
[![AUR Votes](https://img.shields.io/aur/votes/hyprscratch?style=for-the-badge&color=blue)](https://aur.archlinux.org/packages/hyprscratch)

Hyprscratch makes scratchpads in Hyprland painless in a well-integrated and
flexible way.

## Features

* Scratchpads are part of your current workspace by default.
* Can be configured in `hyprland.lua` or a dedicated configuration file.
* Makes special workspace scratchpads simpler to set up.
* Many options to easily tweak behaviour of scratchpads.
* Event listeners to automate scratchpad management.
* Allows cycling and restoring previous scratchpads.
* Supports many scratchpads active at the same time.

## Installation

### Prerequisites

Hyprland's Lua config is required as of hyprscratch `0.7`.

### [Crates](https://crates.io/crates/hyprscratch)

```bash
cargo install hyprscratch
```

### [AUR](https://aur.archlinux.org/packages/hyprscratch)

```bash
paru -S hyprscratch
```

### [Nix](https://github.com/sashetophizika/hyprscratch/blob/master/nix/README.md)

Nix installation options can be found
[here](https://github.com/sashetophizika/hyprscratch/blob/master/nix/README.md).

## Usage

### Basic

In `hyprland.lua`:

```lua
-- Start the hyprscratch daemon
hl.on("hyprland.start", function()
    hl.exec_cmd("hyprscratch init [DAEMON_OPTIONS]")
end)

-- Configure scratchpads
hl.bind("SUPER + KEY", hl.dsp.exec_cmd("hyprscratch [CLIENT_TITLE] '[HYPRLAND_EXEC_COMMAND]' [SCRATCHPAD_OPTIONS]"))
```

Example scratchpad configuration:

```lua
hl.on("hyprland.start", function()
    hl.exec_cmd("hyprscratch init clean eager")
end)

hl.bind("SUPER + T", hl.dsp.exec_cmd("hyprscratch terminal 'alacritty --title terminal'"))
hl.bind("SUPER + B", hl.dsp.exec_cmd("hyprscratch btop '[size monitor_w*0.7 monitor_h*0.8] alacritty --title btop -e btop' persist pin"))
hl.bind("SUPER + F", hl.dsp.exec_cmd("hyprscratch 'Mozilla Firefox' firefox special monitor 1"))
```

A Lua helper with wrappers for these commands is available in
[`lua/hyprscratch.lua`](lua/hyprscratch.lua).

### Optional Configuration File

If you consider it more convenient to use a separate configuration file, you
can create a `~/.config/hypr/hyprscratch.conf` or
`~/.config/hyprscratch/config.conf` and configure scratchpads in the following
way:

```perl
# Create a new scratchpad
scratchpad1 {
    # Mandatory fields
    command = command

    # At least one is mandatory, title takes priority for matching
    title = title # Hyprland initialTitle
    class = class # Hyprland initialClass

    # Optional fields
    options = option1 option2 option3 # Scratchpad options
    rules = rule1;rule2;rule3 # Hyprland window rules
}

# Groups that manage multiple scratchpads are also supported
group:group1 {
    # Add an existing scratchpad to the group
    name = scratchpad1

    # Create a new scratchpad that can also be used by itself
    scratchpad2 {
        title = title
        command = command
    }
}

# Optional globals that apply to all scratchpads
daemon_options = clean
global_options = special
global_rules = monitor_w*0.9 monitor_h*0.9
```

And in `hyprland.lua`, using the given names:

```lua
hl.on("hyprland.start", function()
    hl.exec_cmd("hyprscratch init")
end)

hl.bind("SUPER + T", hl.dsp.exec_cmd("hyprscratch toggle scratchpad1"))
hl.bind("SUPER + S", hl.dsp.exec_cmd("hyprscratch show scratchpad1"))
hl.bind("SUPER + H", hl.dsp.exec_cmd("hyprscratch hide scratchpad1"))

hl.bind("SUPER + G", hl.dsp.exec_cmd("hyprscratch toggle group1"))
```

Using a configuration file can be combined with normally configured scratchpads.

## Options

### Daemon options

* `clean`: automatically hides all scratchpads on workspace change.

* `spotless`: automatically hides all scratchpads on focus change.

* `eager`: spawns all scratchpads hidden on start.

* `no-auto-reload`: does not reload the configuration when the files are modified.

* `config </path/to/config>`: specify a path to the configuration file.

### Scratchpad options

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

* `group <name>`: adds the scratchpad to the specified group.

* `special`: uses the special workspace. Does not work with most other options
and groups.

### Extra subcommands

* `toggle <name>`: toggles the scratchpad with the given name.

* `show <name>`: shows the scratchpad with the given name.

* `hide <name>`: hides the scratchpad with the given name.

* `cycle [normal|special]`: cycles between scratchpads (optionally only normal
or special ones) in the order they are defined in the configuration file.

* `previous [show|hide]`: toggles the last used scratchpad that is not
currently active.

* `hide-all`: hides all scratchpads, useful mostly when stacking multiple of them.

* `kill-all`: closes all scratchpad clients that are open.

* `reload [config]`: re-parses the configuration file without restarting the daemon.

* `menu [fzf|rofi] [show|hide]`: spawns a menu to search through and trigger
scratchpads (`rofi` for keybinds, `fzf` for cli).

* `get-config`: prints out the parsed configuration.

* `kill`: kills the hyprscratch daemon.

* `logs`: shows logs.

## Other Relevant Information

The title of a scratchpad has to be either the `initialTitle` or `initialClass`
field of the client, which can be found with `hyprctl clients`. An incorrect
title results in the scratchpad not being hidden and a new one being spawned
every time.

Terminal applications often all use the title of the terminal emulator. Usually
the title can be set with the `--title` flag to differentiate them.

Multiple commands can be bound to a single scratchpad by separating them with
`?` (e.g. `hyprscratch title "[rules1] command1 ? [rules2] command2"`). Can be
useful with the `poly` option to allow different window rules for each client.

If there are multiple clients with the same title, the program just grabs the
first one it finds. This usually results in cycling between them, every time
one is shown.
