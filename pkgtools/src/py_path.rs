//! `PyPath`: paths spelled as Python's `pathlib` spells them, for the same messages and
//! the same marker text as the Python tools wrote.

use std::path::{Component, Path, PathBuf};

use crate::tool_error::{Result, ToolError};

pub struct PyPath;

impl PyPath {
    /// `Path(p)`: no `.` segment, no repeated or trailing `/`; an empty path is `.`.
    pub fn normalized(path: &Path) -> PathBuf {
        let clean: PathBuf = path
            .components()
            .filter(|c| *c != Component::CurDir)
            .collect();
        if clean.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            clean
        }
    }

    /// `Path(p).absolute()`: normalized, relative to the current directory; `..` stays.
    pub fn absolute(path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            return Ok(Self::normalized(path));
        }
        let cwd = std::env::current_dir()
            .map_err(|e| ToolError::io("cannot read the current directory", &e))?;
        Ok(Self::normalized(&cwd.join(path)))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::PyPath;

    #[test]
    fn paths_are_spelled_as_pathlib_spells_them() {
        assert_eq!(
            PyPath::normalized(Path::new("./a//b/./c/")),
            Path::new("a/b/c")
        );
        assert_eq!(
            PyPath::normalized(Path::new("/x/../y")),
            Path::new("/x/../y")
        );
        assert_eq!(PyPath::normalized(Path::new("")), Path::new("."));
    }
}
