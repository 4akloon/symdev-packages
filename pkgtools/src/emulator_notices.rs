//! `EmulatorNotices`: `share/doc/eka2l1/` of the emulator package (symdev's emulator packages
//! spec §3): EKA2L1's GPL-3.0, the licence files of every submodule it builds from, the list of
//! bundled Ubuntu packages with their copyright files, and where the corresponding source is.

mod bundled_list;
mod submodules;

use std::fs;
use std::path::{Path, PathBuf};

pub use bundled_list::BundledList;
pub use submodules::Submodules;

use crate::tool_error::{Result, ToolError};

pub struct EmulatorNotices {
    /// The fork commit's checkout, submodules included.
    pub src: PathBuf,
    /// The extracted AppImage the notices go into.
    pub tree: PathBuf,
    pub id: String,
    pub commit: String,
    /// The artifact's `eka2l1-qt-x64.packages.tsv` (D1 = A).
    pub packages: Option<PathBuf>,
    /// Licence files, relative to `src`, for submodules that have none under a usual name.
    pub extra: Vec<PathBuf>,
}

impl EmulatorNotices {
    /// The paths of an `--extra` list: one per non-empty line, `#` lines skipped.
    pub fn read_extra(list: &Path) -> Result<Vec<PathBuf>> {
        let text = fs::read_to_string(list).map_err(|e| ToolError::io(list.display(), &e))?;
        Ok(text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(PathBuf::from)
            .collect())
    }

    /// Writes the notices; returns how many licence files and bundled packages it listed.
    pub fn write(&self) -> Result<(usize, usize)> {
        let doc = self.tree.join("share/doc/eka2l1");
        let third = doc.join("third-party");
        fs::create_dir_all(&third).map_err(|e| ToolError::io(third.display(), &e))?;
        copy(&self.src.join("LICENSE"), &doc.join("COPYING"))?;
        let files = Submodules::read(&self.src)?.licence_files(&self.extra)?;
        for rel in &files {
            copy(&self.src.join(rel), &third.join(rel))?;
        }
        let bundled = match &self.packages {
            Some(tsv) => {
                let list = BundledList::read(tsv, &self.tree)?;
                let out = doc.join("BUNDLED.tsv");
                fs::write(&out, list.to_tsv()).map_err(|e| ToolError::io(out.display(), &e))?;
                list.count()
            }
            None => 0,
        };
        let text = format!(
            "{id} is EKA2L1 (GPL-3.0-or-later, COPYING) built by the CI of\n\
             https://github.com/4akloon/EKA2L1 at commit {commit}, with the libraries listed\n\
             in BUNDLED.tsv (each one's licence: usr/share/doc/<package>/copyright).\n\n\
             The corresponding source of everything in this package is the archive the\n\
             index.toml beside it lists as `source-code` for {id}.\n",
            id = self.id,
            commit = self.commit
        );
        let out = doc.join("SOURCE.txt");
        fs::write(&out, text).map_err(|e| ToolError::io(out.display(), &e))?;
        Ok((files.len(), bundled))
    }
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| ToolError::io(parent.display(), &e))?;
    }
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| ToolError::io(from.display(), &e))
}

#[cfg(test)]
mod tests;
