use hyprland::dispatch::{
    Dispatch, DispatchType, WindowIdentifier, WorkspaceIdentifierWithSpecial,
};
use hyprland::error::HyprError;
use hyprland::Result;
use std::process::Command;
use std::sync::OnceLock;

static DISPATCHERS: OnceLock<Dispatchers> = OnceLock::new();

pub fn dispatchers() -> &'static Dispatchers {
    DISPATCHERS.get_or_init(Dispatchers::init)
}

pub struct Dispatchers;

fn dispatch(expr: &str) -> Result<()> {
    Dispatch::call(DispatchType::Custom(expr, ""))
}

impl Dispatchers {
    fn init() -> Self {
        Dispatchers
    }

    pub fn exec(&self, cmd: &str) -> Result<()> {
        dispatch(&format!("hl.dsp.exec_cmd({})", quote(cmd)))
    }

    pub fn close_window(&self, win: WindowIdentifier<'_>) -> Result<()> {
        dispatch(&format!("hl.dsp.window.close({})", win_selector(&win)))
    }

    pub fn focus_window(&self, win: WindowIdentifier<'_>) -> Result<()> {
        dispatch(&format!("hl.dsp.focus({})", win_selector(&win)))
    }

    pub fn toggle_pin_window(&self, win: WindowIdentifier<'_>) -> Result<()> {
        dispatch(&format!("hl.dsp.window.pin({})", win_selector(&win)))
    }

    pub fn move_to_workspace_silent(
        &self,
        ws: WorkspaceIdentifierWithSpecial<'_>,
        win: Option<WindowIdentifier<'_>>,
    ) -> Result<()> {
        let ws_arg = quote(&ws.to_string());
        let win_field = match &win {
            Some(w) => format!(", window={}", quote(&w.to_string())),
            None => String::new(),
        };
        dispatch(&format!(
            "hl.dsp.window.move({{workspace={ws_arg}, follow=false{win_field}}})"
        ))
    }

    pub fn toggle_special_workspace(&self, name: Option<String>) -> Result<()> {
        match name {
            Some(n) => dispatch(&format!("hl.dsp.workspace.toggle_special({})", quote(&n))),
            None => dispatch("hl.dsp.workspace.toggle_special()"),
        }
    }

    pub fn set_workspace_persistent(&self, name: &str) -> Result<()> {
        let expr = format!(
            "hl.workspace_rule({{ workspace = {}, persistent = true }})",
            quote(&format!("special:{name}"))
        );
        let out = Command::new("hyprctl").arg("eval").arg(expr).output()?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        if !out.status.success() || stdout.to_lowercase().contains("error") {
            return Err(HyprError::Other(stdout.trim().to_string().into()));
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn workspace(&self, ws: WorkspaceIdentifierWithSpecial<'_>) -> Result<()> {
        dispatch(&format!(
            "hl.dsp.focus({{workspace={}}})",
            quote(&ws.to_string())
        ))
    }

    pub fn bring_active_to_top(&self) -> Result<()> {
        dispatch("hl.dsp.window.bring_to_top()")
    }
}

fn quote(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{escaped}\"")
}

fn win_selector(win: &WindowIdentifier<'_>) -> String {
    format!("{{window={}}}", quote(&win.to_string()))
}
