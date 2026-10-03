//! `EmulatorTree`: the extracted AppImage an `emulator` package is, checked for the layout
//! symdev starts it by (symdev experiment 115 §1.2) and for its glibc floor.

mod glibc_version;
mod tool;

use std::fs;
use std::path::{Path, PathBuf};

pub use glibc_version::GlibcVersion;
pub use tool::EmulatorTreeTool;

use crate::tool_error::{Result, ToolError};
use crate::tree_walk::TreeWalk;

pub struct EmulatorTree {
    root: PathBuf,
}

impl EmulatorTree {
    /// What symdev starts (its EmulatorPackage::PROGRAM).
    pub const PROGRAM: &'static str = "usr/bin/eka2l1_qt";

    /// The tree at `root`, if it has the observed layout: `AppRun` a link to the program,
    /// no AppRun hooks, the program an ELF file, `qt.conf` pointing Qt at the bundle.
    pub fn at(root: &Path) -> Result<EmulatorTree> {
        let apprun = root.join("AppRun");
        let target = fs::read_link(&apprun)
            .map_err(|e| ToolError::io(format!("{} is not a symlink", apprun.display()), &e))?;
        if target != Path::new(Self::PROGRAM) {
            return Err(ToolError::new(format!(
                "AppRun links to {}, not {}: symdev starts {} itself (experiment 115 §1.2), so \
                 another layout needs a new observation",
                target.display(),
                Self::PROGRAM,
                Self::PROGRAM
            )));
        }
        for hook in ["apprun-hooks", "AppRun.wrapped"] {
            if root.join(hook).symlink_metadata().is_ok() {
                return Err(ToolError::new(format!(
                    "{hook} exists: the AppImage sets up an environment that symdev does not \
                     give eka2l1_qt; observe what it needs before packing it"
                )));
            }
        }
        let program = root.join(Self::PROGRAM);
        let head = fs::read(&program).map_err(|e| ToolError::io(program.display(), &e))?;
        if !head.starts_with(b"\x7fELF") {
            return Err(ToolError::new(format!(
                "{} is not an ELF file",
                program.display()
            )));
        }
        let qt_conf = root.join("usr/bin/qt.conf");
        let text =
            fs::read_to_string(&qt_conf).map_err(|e| ToolError::io(qt_conf.display(), &e))?;
        for line in ["Prefix = ../", "Plugins = plugins"] {
            if !text.lines().any(|l| l.trim() == line) {
                return Err(ToolError::new(format!(
                    "{} has no `{line}`: Qt would not find the bundled plugins",
                    qt_conf.display()
                )));
            }
        }
        Ok(EmulatorTree {
            root: root.to_path_buf(),
        })
    }

    /// The newest glibc version any ELF file of the tree needs (links are skipped: they
    /// name files the walk reads anyway).
    pub fn glibc_floor(&self) -> Result<GlibcVersion> {
        let mut newest = None;
        for file in TreeWalk::files(&self.root, |_| true)? {
            if file
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
            {
                continue;
            }
            let data = fs::read(&file).map_err(|e| ToolError::io(file.display(), &e))?;
            let needed = GlibcVersion::needed_by(&data)
                .map_err(|e| ToolError::new(format!("{}: {e}", file.display())))?;
            newest = newest.max(needed);
        }
        newest.ok_or_else(|| ToolError::new("no ELF file of the tree needs a glibc version"))
    }
}

#[cfg(test)]
mod tests;
