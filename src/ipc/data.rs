use crate::error::Result;
use crate::ipc;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::fmt;

fn query<T: DeserializeOwned>(command: &str) -> Result<T> {
    let response = ipc::request(&format!("j/{command}"))?;
    Ok(serde_json::from_str(response.trim())?)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum WorkspaceId {
    Number(i32),
    Name(String),
}

impl WorkspaceId {
    pub fn as_number(&self) -> Option<i32> {
        match self {
            WorkspaceId::Number(id) => Some(*id),
            WorkspaceId::Name(_) => None,
        }
    }
}

impl Default for WorkspaceId {
    fn default() -> Self {
        WorkspaceId::Name(String::new())
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceId::Number(id) => write!(f, "{id}"),
            WorkspaceId::Name(name) => write!(f, "{name}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Workspace {
    #[serde(default)]
    pub id: WorkspaceId,
    pub name: String,
}

impl Workspace {
    pub fn get_active() -> Result<Workspace> {
        query("activeworkspace")
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub id: i32,
    pub name: String,
    pub active_workspace: Workspace,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Client {
    pub address: String,
    pub workspace: Workspace,
    pub floating: bool,
    pub pinned: bool,
    #[serde(rename = "initialTitle")]
    pub initial_title: String,
    #[serde(rename = "initialClass")]
    pub initial_class: String,
}

impl Client {
    pub fn get_active() -> Result<Option<Client>> {
        let response = ipc::request("j/activewindow")?;
        let value: serde_json::Value = serde_json::from_str(response.trim())?;

        match value {
            serde_json::Value::Null => Ok(None),
            serde_json::Value::Object(map) if map.is_empty() => Ok(None),
            value => Ok(Some(serde_json::from_value(value)?)),
        }
    }
}

pub struct Clients;

impl Clients {
    pub fn get() -> Result<Vec<Client>> {
        query("clients")
    }
}

pub struct Monitors;

impl Monitors {
    pub fn get() -> Result<Vec<Monitor>> {
        query("monitors")
    }
}
