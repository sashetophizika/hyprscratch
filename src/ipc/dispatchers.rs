use crate::error::Result;
use crate::ipc;
use crate::scratchpad::Rules;
use crate::utils::prepend;
use std::fmt;

pub struct Dispatchers;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum WindowIdentifier<'a> {
    Address(String),
    Class(&'a str),
    Title(&'a str),
    ProcessId(u32),
}

impl fmt::Display for WindowIdentifier<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WindowIdentifier::Address(address) => write!(f, "address:{address}"),
            WindowIdentifier::Class(class) => write!(f, "class:{class}"),
            WindowIdentifier::Title(title) => write!(f, "title:{title}"),
            WindowIdentifier::ProcessId(pid) => write!(f, "pid:{pid}"),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum WorkspaceIdentifier<'a> {
    Id(i32),
    Relative(i32),
    Name(&'a str),
    Special(Option<&'a str>),
}

impl fmt::Display for WorkspaceIdentifier<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceIdentifier::Id(id) => write!(f, "{id}"),
            WorkspaceIdentifier::Relative(offset) => {
                write!(f, "e{offset:+}")
            }
            WorkspaceIdentifier::Name(name) => write!(f, "{name}"),
            WorkspaceIdentifier::Special(Some(name)) => write!(f, "special:{name}"),
            WorkspaceIdentifier::Special(None) => write!(f, "special"),
        }
    }
}

fn dispatch(expression: &str) -> Result<()> {
    ipc::eval(&format!("return hl.dispatch({expression})"))?;
    Ok(())
}

fn eval(expression: &str) -> Result<()> {
    ipc::eval(expression)?;
    Ok(())
}

impl Dispatchers {
    pub fn exec(cmd: &str, rules: &Rules) -> Result<()> {
        match rules {
            Rules::Legacy(rules) => {
                let cmd = prepend(cmd, rules);
                dispatch(&format!("hl.dsp.exec_cmd({})", quote(&cmd)))
            }
            Rules::Lua(table) if !rules.is_empty() => {
                dispatch(&format!("hl.dsp.exec_cmd({}, {table})", quote(cmd)))
            }
            Rules::Lua(_) => dispatch(&format!("hl.dsp.exec_cmd({})", quote(cmd))),
        }
    }

    pub fn close_window(win: WindowIdentifier<'_>) -> Result<()> {
        dispatch(&format!("hl.dsp.window.close({})", win_selector(&win)))
    }

    pub fn focus_window(win: WindowIdentifier<'_>) -> Result<()> {
        dispatch(&format!("hl.dsp.focus({})", win_selector(&win)))
    }

    pub fn toggle_pin_window(win: WindowIdentifier<'_>) -> Result<()> {
        dispatch(&format!(
            "hl.dsp.window.pin({{action=\"toggle\", window={}}})",
            quote(&win.to_string())
        ))
    }

    pub fn move_to_workspace_silent(
        ws: WorkspaceIdentifier<'_>,
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

    pub fn toggle_special_workspace(name: Option<String>) -> Result<()> {
        match name {
            Some(n) => dispatch(&format!("hl.dsp.workspace.toggle_special({})", quote(&n))),
            None => dispatch("hl.dsp.workspace.toggle_special()"),
        }
    }

    pub fn set_workspace_persistent(name: &str) -> Result<()> {
        eval(&format!(
            "hl.workspace_rule({{ workspace = {}, persistent = true }})",
            quote(&format!("special:{name}"))
        ))
    }

    #[cfg(test)]
    pub fn workspace(ws: WorkspaceIdentifier<'_>) -> Result<()> {
        dispatch(&format!(
            "hl.dsp.focus({{workspace={}}})",
            quote(&ws.to_string())
        ))
    }

    pub fn raise_active_window() -> Result<()> {
        dispatch(r#"hl.dsp.window.alter_zorder({ mode = "top" })"#)
    }

    pub fn reload_lua() -> Result<()> {
        eval(r#"require("hyprscratch").reload()"#)
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
