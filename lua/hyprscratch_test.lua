#!/usr/bin/env lua5.4

local HELPER = "./hyprscratch.lua"

local SEP_FIELD = "\u{2C00}"
local SEP_SCRATCHPAD = "\u{2C01}"
local SEP_SECTION = "\u{2C03}"

local cases = {}
local function test(name, fn)
    cases[#cases + 1] = { name = name, fn = fn }
end

local function expect_eq(actual, expected, label)
    if actual ~= expected then
        error(("%s\n  expected: %q\n  actual:   %q"):format(
            label or "value", tostring(expected), tostring(actual)), 2)
    end
end

local function expect_contains(haystack, needle, label)
    if not haystack:find(needle, 1, true) then
        error(("%s\n  expected to contain: %q\n  actual: %q"):format(
            label or "value", tostring(needle), tostring(haystack)), 2)
    end
end

local function expect_not_contains(haystack, needle, label)
    if haystack:find(needle, 1, true) then
        error(("%s\n  expected NOT to contain: %q\n  actual: %q"):format(
            label or "value", tostring(needle), tostring(haystack)), 2)
    end
end

local function split(s, sep)
    local out, start = {}, 1
    while true do
        local i = s:find(sep, start, true)
        if not i then
            out[#out + 1] = s:sub(start)
            return out
        end
        out[#out + 1] = s:sub(start, i - 1)
        start = i + #sep
    end
end

local function new_env()
    local calls = { exec = {}, dispatched = {}, handlers = {} }

    _G.hl = {
        dsp = {
            exec_cmd = function(cmd)
                calls.exec[#calls.exec + 1] = cmd
                return { cmd = cmd }
            end,
        },
        dispatch = function(d)
            calls.dispatched[#calls.dispatched + 1] = d.cmd
        end,
        on = function(event, cb)
            calls.handlers[event] = cb
        end,
    }

    local helper = dofile(HELPER)
    return helper, calls
end

local function populate_message(cmd)
    local prefix = "hyprscratch populate '"
    local suffix = "'"
    expect_eq(cmd:sub(1, #prefix), prefix, "populate prefix")
    expect_eq(cmd:sub(- #suffix), suffix, "populate suffix")
    return cmd:sub(#prefix + 1, #cmd - #suffix)
end

local function populate_sections(cmd)
    local msg = populate_message(cmd)
    local i = msg:find(SEP_SECTION, 1, true)
    if not i then
        error("populate message has no section separator: " .. msg)
    end
    return msg:sub(1, i - 1), msg:sub(i + #SEP_SECTION)
end

local function payload_fields(sc_section)
    return split(sc_section, SEP_FIELD)
end

test("start handlers", function()
    local _, calls = new_env()
    expect_eq(type(calls.handlers["hyprland.start"]), "function", "hyprland.start")
    expect_eq(type(calls.handlers["config.reloaded"]), "function", "config.reloaded")
end)

test("init true options", function()
    local helper, calls = new_env()
    helper.setup({
        daemon = {
            clean = true,
            spotless = true,
            eager = true,
            no_auto_reload = true,
            config = "/tmp/x.conf",
        },
    })
    calls.handlers["hyprland.start"]()
    expect_eq(
        calls.dispatched[1],
        "hyprscratch init clean spotless eager no-auto-reload --config=/tmp/x.conf",
        "init command")
end)

test("init false options", function()
    local helper, calls = new_env()
    helper.setup({ daemon = { clean = false, eager = false } })
    calls.handlers["hyprland.start"]()
    expect_eq(calls.dispatched[1], "hyprscratch init ", "init command")
end)

test("reload empty", function()
    local _, calls = new_env()
    calls.handlers["config.reloaded"]()
    expect_eq(populate_message(calls.dispatched[1]), SEP_SECTION, "payload")
end)

test("reload single scratchpad", function()
    local helper, calls = new_env()
    helper.setup({
        scratchpads = { pad = { title = "Pad", command = "kitty" } },
    })
    calls.handlers["config.reloaded"]()

    local expected = table.concat({ "pad", "Pad", "kitty", "{}", "" }, SEP_FIELD)
        .. SEP_SECTION
    expect_eq(calls.dispatched[1], "hyprscratch populate '" .. expected .. "'", "populate command")
end)

test("inline group scratchpad", function()
    local helper, calls = new_env()
    helper.setup({
        groups = { g = { { title = "X", command = "x" } } },
    })
    calls.handlers["config.reloaded"]()

    local sc, gr = populate_sections(calls.dispatched[1])
    expect_eq(sc, table.concat({ "1", "X", "x", "{}", "" }, SEP_FIELD), "inline payload")
    expect_eq(gr, "g" .. SEP_SCRATCHPAD .. "1", "group reference")
end)

test("options formatting", function()
    local helper, calls = new_env()
    helper.setup({
        scratchpads = {
            pad = {
                title = "P",
                command = "p",
                options = { pin = true, special = true, monitor = "DP-1", sticky = false },
            },
        },
    })
    calls.handlers["config.reloaded"]()

    local sc = populate_sections(calls.dispatched[1])
    local fields = payload_fields(sc)
    expect_eq(fields[5], "pin special monitor DP-1 ", "options")
end)

test("global options", function()
    local helper, calls = new_env()
    helper.setup({
        global_options = { sticky = true },
        scratchpads = { pad = { title = "P", command = "p", options = { pin = true } } },
    })
    calls.handlers["config.reloaded"]()

    local fields = payload_fields(populate_sections(calls.dispatched[1]))
    expect_eq(fields[5], "pin sticky ", "merged options")
end)

test("global rules", function()
    local helper, calls = new_env()
    helper.setup({
        global_rules = { tag = "+scratchpad", size = { 100, 200 } },
        scratchpads = {
            pad = { title = "P", command = "p", rules = { size = { 300, 400 } } },
        },
    })
    calls.handlers["config.reloaded"]()

    local rules = payload_fields(populate_sections(calls.dispatched[1]))[4]
    expect_contains(rules, '["tag"] = "+scratchpad"', "global rule kept")
    expect_contains(rules, '["size"] = {[1] = 300, [2] = 400}', "local rule overwrite")
end)

test("class", function()
    local helper, calls = new_env()
    helper.setup({
        scratchpads = { pad = { class = "Firefox", command = "firefox" } },
    })
    calls.handlers["config.reloaded"]()

    local fields = payload_fields(populate_sections(calls.dispatched[1]))
    expect_eq(fields[2], "Firefox", "title/class field")
end)

test("missing title and class", function()
    local helper, calls = new_env()
    helper.setup({ scratchpads = { pad = { command = "p" } } })
    calls.handlers["config.reloaded"]()

    local fields = payload_fields(populate_sections(calls.dispatched[1]))
    expect_eq(fields[2], "", "title/class field")
end)

test("missing command", function()
    local helper, calls = new_env()
    helper.setup({ scratchpads = { pad = { title = "P" } } })
    calls.handlers["config.reloaded"]()

    local fields = payload_fields(populate_sections(calls.dispatched[1]))
    expect_eq(fields[3], "", "command field")
end)

test("toggle no dispatch", function()
    local helper, calls = new_env()
    local d = helper.toggle("x")
    expect_eq(d.cmd, "hyprscratch toggle 'x'", "toggle")
    expect_eq(#calls.dispatched, 0, "dispatch count")
end)

test("toggle fomatting", function()
    local helper = new_env()
    expect_eq(helper.toggle("it's").cmd, "hyprscratch toggle 'it'\\''s'", "toggle quote")
end)

test("show hide formatting", function()
    local helper, calls = new_env()
    expect_eq(helper.show("x").cmd, "hyprscratch show 'x'", "show")
    expect_eq(helper.hide("x").cmd, "hyprscratch hide 'x'", "hide")
    expect_eq(#calls.dispatched, 0, "dispatch count")
end)

test("cycle formatting", function()
    local helper, calls = new_env()
    expect_eq(helper.cycle().cmd, "hyprscratch cycle", "cycle nil")
    expect_eq(helper.cycle("").cmd, "hyprscratch cycle", "cycle empty")
    expect_eq(helper.cycle("special").cmd, "hyprscratch cycle 'special'", "cycle special")
    expect_eq(#calls.dispatched, 0, "dispatch count")
end)

test("previous formatting", function()
    local helper, calls = new_env()
    expect_eq(helper.previous().cmd, "hyprscratch previous", "previous nil")
    expect_eq(helper.previous("").cmd, "hyprscratch previous", "previous empty")
    expect_eq(helper.previous("normal").cmd, "hyprscratch previous 'normal'", "previous normal")
    expect_eq(#calls.dispatched, 0, "dispatch count")
end)

test("hide_all formatting", function()
    local helper, calls = new_env()
    expect_eq(helper.hide_all().cmd, "hyprscratch hide-all", "hide_all")
    expect_eq(#calls.dispatched, 0, "dispatch count")
end)

test("reload", function()
    local helper, calls = new_env()
    helper.setup({ scratchpads = { pad = { title = "P", command = "p" } } })

    local ret = helper.reload()
    expect_eq(ret, nil, "return value")
    expect_eq(#calls.dispatched, 1, "dispatch count")
    expect_eq(calls.dispatched[1]:sub(1, #"hyprscratch populate '"),
        "hyprscratch populate '", "populate prefix")
end)

test("setup overwrite", function()
    local helper, calls = new_env()
    helper.setup({ scratchpads = { a = { title = "A", command = "a" } } })
    helper.setup({ scratchpads = { b = { title = "B", command = "b" } } })
    calls.handlers["config.reloaded"]()

    local sc = populate_sections(calls.dispatched[1])
    expect_contains(sc, "b" .. SEP_FIELD .. "B", "second config kept")
    expect_not_contains(sc, "a" .. SEP_FIELD .. "A", "first config dropped")
end)

local failed = 0
for _, case in ipairs(cases) do
    local ok, err = pcall(case.fn)
    if ok then
        io.write("ok   " .. case.name .. "\n")
    else
        failed = failed + 1
        io.write("FAIL " .. case.name .. "\n     " .. tostring(err) .. "\n")
    end
end

io.write(("%d/%d passed\n"):format(#cases - failed, #cases))
