# Hyprland Lua Helper File

## API

```lua
hsc.init({ clean: bool, spotless: bool, eager: bool, no_auto_reload: bool, config: string })
hsc.toggle(name: string)
hsc.show(name: string)
hsc.hide(name: string)
hsc.cycle([mode: string])
hsc.previous([action: string])
hsc.scratchpad(title: string, command: string, [opts: string])
hsc.reload([config_path: string])
hsc.hide_all()
```

## Example

```lua
local hsc = require("hyprscratch")

hl.on("hyprland.start", function()
    hsc.init { clean = true, eager = true }
end)

hl.bind("SUPER + key", hsc.toggle("name"))
```
