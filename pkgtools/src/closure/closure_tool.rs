//! `ClosureTool`: `pkgtools runtime-closure <ld.map> <archive>(<member>)...`.

use std::io::Write;
use std::path::Path;

use super::{Inclusion, LinkMap, ShippedSet};
use crate::py_text::PyText;
use crate::tool_error::{Result, ToolError};

/// Reads the map, prints each member taken from a shipped archive and why, and fails
/// (exit 1, each problem on `err`) unless the shipped set is exactly what the link took.
pub struct ClosureTool;

impl ClosureTool {
    pub fn run(map: &Path, shipped: &[String], out: &mut impl Write, err: &mut impl Write) -> u8 {
        let (inclusions, problems) = match Self::read(map, shipped) {
            Ok(found) => found,
            Err(e) => {
                let _ = writeln!(err, "error: {}: {e}", map.display());
                return 1;
            }
        };
        let archives: Vec<&str> = shipped
            .iter()
            .map(|name| name.split('(').next().unwrap_or(name))
            .collect();
        for inclusion in inclusions
            .iter()
            .filter(|i| archives.contains(&i.archive()))
        {
            let _ = writeln!(out, "{} <- {}", inclusion.name(), inclusion.reason());
        }
        for problem in &problems {
            let _ = writeln!(err, "error: {problem}");
        }
        u8::from(!problems.is_empty())
    }

    fn read(map: &Path, shipped: &[String]) -> Result<(Vec<Inclusion>, Vec<String>)> {
        let bytes = std::fs::read(map).map_err(|e| ToolError::io("cannot read the map", &e))?;
        let text = String::from_utf8(bytes)
            .map_err(|e| ToolError::new(format!("the map is not UTF-8 ({e})")))?;
        let inclusions = LinkMap::included_members(&PyText::universal_newlines(&text))?;
        let problems = ShippedSet::new(shipped)?.check(&inclusions);
        Ok((inclusions, problems))
    }
}
