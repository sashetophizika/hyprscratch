use hyprland::dispatch::{
    Dispatch, DispatchType, WindowIdentifier, WorkspaceIdentifierWithSpecial,
};
use hyprland::Result;
use std::process::Command;
use std::sync::OnceLock;

static DISPATCHERS: OnceLock<Dispatchers> = OnceLock::new();

pub fn dispatchers() -> &'static Dispatchers {
    DISPATCHERS.get_or_init(Dispatchers::init)
}

enum ConfigLanguage {
    Hyprlang,
    Lua,
}

pub struct Dispatchers {
    lang: ConfigLanguage,
}

fn call(name: &str, args: &str) -> Result<()> {
    Dispatch::call(DispatchType::Custom(name, args))
}

fn call_lua(expr: &str) -> Result<()> {
    Dispatch::call(DispatchType::Custom(expr, ""))
}

impl Dispatchers {
    fn init() -> Self {
        Self {
            lang: detect_config_language(),
        }
    }

    pub fn exec(&self, cmd: &str) -> Result<()> {
        match self.lang {
            ConfigLanguage::Hyprlang => call("exec", cmd),
            ConfigLanguage::Lua => call_lua(&format!("hl.dsp.exec_cmd({})", lua_str(cmd))),
        }
    }

    pub fn close_window(&self, win: WindowIdentifier<'_>) -> Result<()> {
        match self.lang {
            ConfigLanguage::Hyprlang => call("closewindow", &win.to_string()),
            ConfigLanguage::Lua => call_lua(&format!("hl.dsp.window.close({})", lua_win_selector(&win))),
        }
    }

    pub fn focus_window(&self, win: WindowIdentifier<'_>) -> Result<()> {
        match self.lang {
            ConfigLanguage::Hyprlang => call("focuswindow", &win.to_string()),
            ConfigLanguage::Lua => call_lua(&format!("hl.dsp.focus({})", lua_win_selector(&win))),
        }
    }

    pub fn toggle_pin_window(&self, win: WindowIdentifier<'_>) -> Result<()> {
        match self.lang {
            ConfigLanguage::Hyprlang => call("pin", &win.to_string()),
            ConfigLanguage::Lua => call_lua(&format!("hl.dsp.window.pin({})", lua_win_selector(&win))),
        }
    }

    pub fn move_to_workspace_silent(
        &self,
        ws: WorkspaceIdentifierWithSpecial<'_>,
        win: Option<WindowIdentifier<'_>>,
    ) -> Result<()> {
        match self.lang {
            ConfigLanguage::Hyprlang => {
                let args = match win {
                    Some(w) => format!("{ws},{w}"),
                    None => ws.to_string(),
                };
                call("movetoworkspacesilent", &args)
            }
            ConfigLanguage::Lua => {
                let ws_arg = lua_str(&ws.to_string());
                let win_field = match &win {
                    Some(w) => format!(", window={}", lua_str(&w.to_string())),
                    None => String::new(),
                };
                call_lua(&format!(
                    "hl.dsp.window.move({{workspace={ws_arg}, follow=false{win_field}}})"
                ))
            }
        }
    }

    pub fn toggle_special_workspace(&self, name: Option<String>) -> Result<()> {
        match self.lang {
            ConfigLanguage::Hyprlang => match name {
                Some(n) => call("togglespecialworkspace", &n),
                None => call("togglespecialworkspace", ""),
            },
            ConfigLanguage::Lua => match name {
                Some(n) => {
                    call_lua(&format!("hl.dsp.workspace.toggle_special({})", lua_str(&n)))
                }
                None => call_lua("hl.dsp.workspace.toggle_special()"),
            },
        }
    }

    #[cfg(test)]
    pub fn workspace(&self, ws: WorkspaceIdentifierWithSpecial<'_>) -> Result<()> {
        match self.lang {
            ConfigLanguage::Hyprlang => call("workspace", &ws.to_string()),
            ConfigLanguage::Lua => call_lua(&format!(
                "hl.dsp.focus({{workspace={}}})",
                lua_str(&ws.to_string())
            )),
        }
    }

    pub fn bring_active_to_top(&self) -> Result<()> {
        match self.lang {
            ConfigLanguage::Hyprlang => call("bringactivetotop", ""),
            ConfigLanguage::Lua => call_lua("hl.dsp.window.bring_to_top()"),
        }
    }
}

fn lua_str(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{escaped}\"")
}

fn lua_win_selector(win: &WindowIdentifier<'_>) -> String {
    format!("{{window={}}}", lua_str(&win.to_string()))
}

fn detect_config_language() -> ConfigLanguage {
    let output = Command::new("hyprctl").arg("status").output().ok();
    if let Some(out) = output {
        if let Ok(text) = String::from_utf8(out.stdout) {
            for line in text.lines() {
                if let Some(lang) = line.strip_prefix("configProvider: ") {
                    return match lang.trim() {
                        "lua" => ConfigLanguage::Lua,
                        _ => ConfigLanguage::Hyprlang,
                    };
                }
            }
        }
    }
    ConfigLanguage::Hyprlang
}
