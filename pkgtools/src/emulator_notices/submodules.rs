//! `Submodules`: a checkout's submodule folders, nested ones included (each `.gitmodules`'s
//! `path = …` lines), and the licence files at their top.

use std::fs;
use std::path::{Path, PathBuf};

use crate::tool_error::{Result, ToolError};

/// The names a licence file starts with, in any case.
const NAMES: [&str; 5] = ["LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT"];

pub struct Submodules {
    src: PathBuf,
    /// Relative to `src`, sorted.
    paths: Vec<PathBuf>,
}

impl Submodules {
    pub fn read(src: &Path) -> Result<Submodules> {
        let mut paths = Vec::new();
        let mut todo = vec![PathBuf::new()];
        while let Some(base) = todo.pop() {
            let file = src.join(&base).join(".gitmodules");
            let Ok(text) = fs::read_to_string(&file) else {
                continue;
            };
            for line in text.lines() {
                if let Some(p) = line.trim().strip_prefix("path = ") {
                    let rel = base.join(p.trim());
                    todo.push(rel.clone());
                    paths.push(rel);
                }
            }
        }
        paths.sort();
        Ok(Submodules {
            src: src.to_path_buf(),
            paths,
        })
    }

    /// Every submodule's licence files and `extra`, relative to the checkout. A submodule
    /// with neither is an error naming all such submodules: read each and list its licence
    /// file in the recipe's `notices-extra.txt`.
    pub fn licence_files(&self, extra: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let (mut found, mut missing) = (Vec::new(), Vec::new());
        for sub in &self.paths {
            let dir = self.src.join(sub);
            let entries = fs::read_dir(&dir).map_err(|e| ToolError::io(dir.display(), &e))?;
            let mut here: Vec<PathBuf> = entries
                .flatten()
                .filter(|e| e.path().is_file())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_uppercase();
                    NAMES.iter().any(|n| name.starts_with(n))
                })
                .map(|e| sub.join(e.file_name()))
                .collect();
            here.sort();
            if here.is_empty() && !extra.iter().any(|x| self.owner(x) == Some(sub)) {
                missing.push(sub.display().to_string());
            }
            found.extend(here);
        }
        if !missing.is_empty() {
            return Err(ToolError::new(format!(
                "no licence file at the top of: {}; read each one and list its licence file, \
                 relative to the checkout, in the recipe's notices-extra.txt",
                missing.join(", ")
            )));
        }
        found.extend(extra.iter().cloned());
        found.sort();
        found.dedup();
        Ok(found)
    }

    /// The deepest submodule that holds `path`.
    fn owner(&self, path: &Path) -> Option<&PathBuf> {
        self.paths
            .iter()
            .filter(|p| path.starts_with(p))
            .max_by_key(|p| p.components().count())
    }
}
