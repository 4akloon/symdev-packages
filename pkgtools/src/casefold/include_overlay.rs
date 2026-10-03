//! `IncludeOverlay`: one symlink per include name the SDK has only in another case.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use crate::py_path::PyPath;
use crate::py_text::PyText;
use crate::tool_error::{Result, ToolError};
use crate::tree_walk::TreeWalk;

/// The SDK was written on Windows, so its headers include each other in the wrong case
/// (`fbs.h` asks for <FbsMessage.h>, the file is `fbsmessage.h`). For every name an
/// `#include` line anywhere in the tree asks for that does not exist as written but does
/// exist in another case, the overlay gets a symlink by that name to the real file; it goes
/// on the include path after the SDK's own directories. The rule of symdev's
/// `SdkIncludeCaseFold`, except that where two files differ only in case the first in
/// sorted order wins (symdev takes the first its directory listing gives).
pub struct IncludeOverlay;

impl IncludeOverlay {
    /// Marks an overlay as built; one that has it is reused as it is.
    const MARKER: &str = ".symdev-casefold";

    /// Builds the overlay of `include` in `out` once; returns `out`.
    pub fn ensure(include: &Path, out: &Path) -> Result<PathBuf> {
        // Absolute, so the links work from wherever the overlay is used.
        let include = PyPath::absolute(include)?;
        let out = PyPath::normalized(out);
        if out.join(Self::MARKER).is_file() {
            return Ok(out);
        }
        if !include.is_dir() {
            return Err(ToolError::new(format!(
                "{} is not a directory",
                include.display()
            )));
        }
        let mut by_lower: HashMap<String, PathBuf> = HashMap::new();
        let mut names = BTreeSet::new();
        for path in TreeWalk::files(&include, |_| true)? {
            let rel = path.strip_prefix(&include).unwrap_or(&path);
            let rel = rel.to_string_lossy().replace('\\', "/");
            by_lower
                .entry(rel.to_lowercase())
                .or_insert_with(|| path.clone());
            let data = fs::read(&path)
                .map_err(|e| ToolError::io(format!("cannot read {}", path.display()), &e))?;
            names.extend(Self::include_names(&data));
        }
        Self::create_dir(&out)?;
        for name in names {
            if include.join(&name).exists() {
                continue;
            }
            let Some(real) = by_lower.get(&name.to_lowercase()) else {
                continue;
            };
            let link = out.join(&name);
            if let Some(parent) = link.parent() {
                Self::create_dir(parent)?;
            }
            if link.symlink_metadata().is_err() {
                std::os::unix::fs::symlink(real, &link).map_err(|e| {
                    ToolError::io(
                        format!("cannot link {} to {}", link.display(), real.display()),
                        &e,
                    )
                })?;
            }
        }
        let marker = out.join(Self::MARKER);
        fs::write(&marker, include.to_string_lossy().as_bytes())
            .map_err(|e| ToolError::io(format!("cannot write {}", marker.display()), &e))?;
        Ok(out)
    }

    fn create_dir(dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)
            .map_err(|e| ToolError::io(format!("cannot create {}", dir.display()), &e))
    }

    /// The names the `#include <…>` / `#include "…"` lines of `data` ask for, `\` as `/`;
    /// absolute names and names with `..` are left out.
    pub fn include_names(data: &[u8]) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        for raw in data.split(|&b| b == b'\n') {
            let line = String::from_utf8_lossy(raw);
            let Some(rest) = PyText::lstrip(&line).strip_prefix('#') else {
                continue;
            };
            let Some(rest) = PyText::lstrip(rest).strip_prefix("include") else {
                continue;
            };
            let rest = PyText::lstrip(rest);
            let close = match rest.chars().next() {
                Some('<') => '>',
                Some('"') => '"',
                _ => continue,
            };
            let Some(end) = rest[1..].find(close) else {
                continue;
            };
            let name = rest[1..1 + end].replace('\\', "/");
            if !name.is_empty() && !name.starts_with('/') && !name.contains("..") {
                found.insert(name);
            }
        }
        found
    }
}
