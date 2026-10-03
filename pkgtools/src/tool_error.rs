//! `ToolError`: what a tool could not do, said so that a person can act on it.

use std::fmt;

/// The failure of one of the tools: a message naming what failed and, when known, the fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError(String);

impl ToolError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    /// `<what> (<path>): <io error>`, for a file the tool could not read or write.
    pub fn io(what: impl fmt::Display, error: &std::io::Error) -> Self {
        Self(format!("{what}: {error}"))
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ToolError {}

pub type Result<T> = std::result::Result<T, ToolError>;
