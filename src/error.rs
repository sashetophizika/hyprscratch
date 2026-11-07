use std::env::VarError;
use std::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Json(serde_json::Error),
    Env(VarError),
    Ipc(String),
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(error) => write!(f, "{error}"),
            Error::Json(error) => write!(f, "{error}"),
            Error::Env(error) => write!(f, "{error}"),
            Error::Ipc(message) => write!(f, "{message}"),
            Error::Other(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(error) => Some(error),
            Error::Json(error) => Some(error),
            Error::Env(error) => Some(error),
            Error::Ipc(_) | Error::Other(_) => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Json(error)
    }
}

impl From<VarError> for Error {
    fn from(error: VarError) -> Self {
        Error::Env(error)
    }
}
