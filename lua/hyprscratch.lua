local M = {}

local function table_to_string(t)
    local parts = {}
    for k, v in pairs(t) do
        local key = type(k) == "string" and string.format("%q", k) or tostring(k)
        local val
        if type(v) == "table" then
            val = table_to_string(v)
        elseif type(v) == "string" then
            val = string.format("%q", v)
        else
            val = tostring(v)
        end
        parts[#parts + 1] = string.format("[%s] = %s", key, val)
    end
    return "{" .. table.concat(parts, ", ") .. "}"
end

local function quote(s)
    return "'" .. tostring(s):gsub("'", "'\\''") .. "'"
end

local config = { daemon = {}, scratchpads = {} }

function M.setup(cfg)
    config.daemon = cfg.daemon or {}
    config.scratchpads = cfg.scratchpads or {}
end

local function payload(args)
    return table.concat({
        tostring(args["name"] or ""),
        tostring(args["title"] or ""),
        tostring(args["command"] or ""),
        table_to_string(args["rules"] or {}),
        tostring(args["options"] or ""),
    }, "^")
end

local function daemon_opts(opts)
    local opts = opts or {}
    local args = {}

    if opts.clean then table.insert(args, "clean") end
    if opts.spotless then table.insert(args, "spotless") end
    if opts.eager then table.insert(args, "eager") end
    if opts.no_auto_reload then table.insert(args, "no-auto-reload") end
    if opts.config then table.insert(args, "--config=" .. opts.config) end

    return table.concat(args, " ")
end

local function init()
    local d = config.daemon
    hl.dispatch(hl.dsp.exec_cmd("hyprscratch init" .. daemon_opts(d)))
end

local function reload()
    for _, sc in ipairs(config.scratchpads) do
        hl.dispatch(hl.dsp.exec_cmd("hyprscratch add " .. quote(payload(sc))))
    end
end

hl.on("hyprland.start", init)
hl.on("hyprland.start", reload)
hl.on("config.reloaded", reload)


M.toggle = function(name)
    return hl.dsp.exec_cmd("hyprscratch toggle " .. quote(name or ""))
end

M.show = function(name)
    return hl.dsp.exec_cmd("hyprscratch show " .. quote(name or ""))
end

M.hide = function(name)
    return hl.dsp.exec_cmd("hyprscratch hide " .. quote(name or ""))
end

M.cycle = function(mode)
    if mode and mode ~= "" then
        return hl.dsp.exec_cmd("hyprscratch cycle " .. quote(mode))
    end
    return hl.dsp.exec_cmd("hyprscratch cycle")
end

M.previous = function(action)
    if action and action ~= "" then
        return hl.dsp.exec_cmd("hyprscratch previous " .. quote(action))
    end
    return hl.dsp.exec_cmd("hyprscratch previous")
end


M.reload = function(config_path)
    local cmd = "hyprscratch reload"
    if config_path and config_path ~= "" then
        cmd = cmd .. " " .. quote(config_path)
    end
    return hl.dsp.exec_cmd(cmd)
end

M.hide_all = function()
    return hl.dsp.exec_cmd("hyprscratch hide-all")
end

return M
