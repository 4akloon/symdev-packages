//! `TreeWalk`: the files of a tree in the order the Python tools walked them.

use std::fs;
use std::path::{Path, PathBuf};

use crate::tool_error::{Result, ToolError};

/// Python's `os.walk(top)` with `dirnames.sort()` and `sorted(filenames)`, flattened: a
/// directory's own files in name order, then each subdirectory's, in name order. A
/// symlink to a directory is neither walked nor listed (`followlinks=False`); any other
/// entry that is not a directory is listed. Unlike `os.walk`, a directory that cannot be
/// read is an error, not skipped.
pub struct TreeWalk;

impl TreeWalk {
    /// The files under `top`, walking only into the directories `keep_dir` accepts.
    pub fn files(top: &Path, keep_dir: impl Fn(&Path) -> bool) -> Result<Vec<PathBuf>> {
        let mut found = Vec::new();
        Self::walk(top, &keep_dir, &mut found)?;
        Ok(found)
    }

    fn walk(dir: &Path, keep_dir: &impl Fn(&Path) -> bool, found: &mut Vec<PathBuf>) -> Result<()> {
        let unreadable =
            |e: std::io::Error| ToolError::io(format!("cannot read {}", dir.display()), &e);
        let (mut dirs, mut files) = (Vec::new(), Vec::new());
        for entry in fs::read_dir(dir).map_err(unreadable)? {
            let entry = entry.map_err(unreadable)?;
            let path = entry.path();
            let kind = entry.file_type().map_err(unreadable)?;
            if kind.is_dir() {
                dirs.push(path);
            } else if !(kind.is_symlink() && path.is_dir()) {
                files.push(path);
            }
        }
        files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        dirs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        found.extend(files);
        for sub in dirs.iter().filter(|d| keep_dir(d)) {
            Self::walk(sub, keep_dir, found)?;
        }
        Ok(())
    }
}
