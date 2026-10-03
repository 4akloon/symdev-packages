//! `LicenceText`: a licence file's text, as the Python tool's `read_text` gave it.

use std::path::Path;

use crate::py_text::PyText;
use crate::tool_error::{Result, ToolError};

/// UTF-8 or an error naming the file; `\r\n` and `\r` read as `\n` (Python's text mode).
pub struct LicenceText;

impl LicenceText {
    /// `shown` names the file in errors (`<crate dir>/<path>`).
    pub fn read(path: &Path, shown: &str) -> Result<String> {
        let bytes =
            std::fs::read(path).map_err(|e| ToolError::io(format!("cannot read {shown}"), &e))?;
        let text = String::from_utf8(bytes).map_err(|e| {
            ToolError::new(format!(
                "{shown} is not UTF-8 (invalid byte at offset {}); convert it by hand in a copy",
                e.utf8_error().valid_up_to()
            ))
        })?;
        Ok(PyText::universal_newlines(&text))
    }
}
