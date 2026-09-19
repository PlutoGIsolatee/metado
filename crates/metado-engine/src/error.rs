use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    PluginError,
    PermissionDenied,
    CapabilityError,
    Fault,
}

#[derive(Debug, Clone)]
pub struct ExecutionError {
    pub entry: String,
    pub kind: ErrorKind,
    pub message: String,
}

impl ExecutionError {
    pub fn new(entry: &str, kind: ErrorKind, message: &str) -> Self {
        Self {
            entry: entry.to_string(),
            kind,
            message: message.to_string(),
        }
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}: {}", self.kind, self.entry, self.message)
    }
}

impl std::error::Error for ExecutionError {}