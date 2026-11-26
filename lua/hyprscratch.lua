local M = {}

local config = {
    daemon = {},
    scratchpads = {},
    groups = {},
    global_options = {},
    global_rules = {},
}

local function merge(base, override)
    local merged = {}
    for k, v in pairs(base or {}) do
        merged[k] = v
    end
    for k, v in pairs(override or {}) do
        merged[k] = v
    end
    return merged
end

local function sort_keys(t)
    local keys = {}
    for k in pairs(t or {}) do
        keys[#keys + 1] = k
    end
    table.sort(keys)
    return keys
end

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

local function table_to_options(opts)
    local options = ""
    local keys = {
        "pin",
        "cover",
        "persist",
        "sticky",
        "shiny",
        "lazy",
        "show",
        "hide",
        "poly",
        "tiled",
        "group",
        "special",
        "monitor",
        "ephemeral",
    }

    for _, k in ipairs(keys) do
        if opts[k] and type(opts[k]) == "boolean" then
            options = options .. k .. " "
        elseif opts[k] then
            options = options .. k .. " " .. opts[k] .. " "
        end
    end
    return options
end

local function quote(s)
    return "'" .. tostring(s):gsub("'", "'\\''") .. "'"
end

local function payload(name, args)
    return table.concat({
        name,
        tostring(args["title"] or args["class"] or ""),
        tostring(args["command"] or ""),
        table_to_string(merge(config.global_rules, args["rules"])),
        table_to_options(merge(config.global_options, args["options"])),
    }, "\u{2C00}")
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

function M.setup(cfg)
    config.daemon = cfg.daemon or {}
    config.scratchpads = cfg.scratchpads or {}
    config.groups = cfg.groups or {}
    config.global_options = cfg.global_options or {}
    config.global_rules = cfg.global_rules or {}
end

local function init()
    local d = config.daemon
    hl.dispatch(hl.dsp.exec_cmd("hyprscratch init " .. daemon_opts(d)))
end

local function reload()
    local scratchpads = {}
    for _, name in ipairs(sort_keys(config.scratchpads)) do
        table.insert(scratchpads, payload(name, config.scratchpads[name]))
    end

    local groups = {}
    for _, name in ipairs(sort_keys(config.groups)) do
        local group_scratchpads = {}
        for sc_name, sc in ipairs(config.groups[name]) do
            if type(sc) == "string" then
                table.insert(group_scratchpads, sc)
            elseif type(sc) == "table" then
                table.insert(scratchpads, payload(tostring(sc_name), sc))
                table.insert(group_scratchpads, tostring(sc_name))
            end
        end
        table.insert(groups, name .. "\u{2C01}" .. table.concat(group_scratchpads, "\u{2C00}"))
    end

    local scratchpads_str = table.concat(scratchpads, "\u{2C01}")
    local groups_str = table.concat(groups, "\u{2C02}")
    hl.dispatch(hl.dsp.exec_cmd("hyprscratch populate " .. quote(scratchpads_str .. "\u{2C03}" .. groups_str)))
end

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


M.reload = function()
    reload()
end

M.hide_all = function()
    return hl.dsp.exec_cmd("hyprscratch hide-all")
end

hl.on("hyprland.start", init)
hl.on("config.reloaded", reload)

return M
