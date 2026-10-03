//! `Crate`: one package of the dependency graph and its source directory.

use std::path::{Path, PathBuf};

use super::Entry;
use super::licence_text::LicenceText;
use crate::tool_error::{Result, ToolError};
use crate::tree_walk::TreeWalk;

/// A package as `cargo metadata` describes it, with the licence files of its source.
#[derive(Debug, Clone)]
pub struct Crate {
    pub(super) name: String,
    pub(super) version: String,
    /// The declared SPDX expression, `NOASSERTION` without one.
    pub(super) license: String,
    pub(super) license_file: Option<String>,
    pub(super) links: Option<String>,
    /// `registry+…`, `git+…`, or `path`.
    pub(super) source: String,
    pub(super) dir: PathBuf,
}

impl Crate {
    pub fn name(&self) -> &str {
        &self.name
    }

    /// A licence, notice or copyright file by its name: LICENSE*, LICENCE*, COPYING*,
    /// NOTICE*, COPYRIGHT*, in any case.
    pub fn is_notice(name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        ["licence", "license", "copying", "notice", "copyright"]
            .iter()
            .any(|p| lower.starts_with(p))
    }

    /// `<crate dir>/<rel>`, as errors name a file.
    pub(super) fn shown(&self, rel: &str) -> String {
        let dir = self
            .dir
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        format!("{dir}/{rel}")
    }

    pub(super) fn text(&self, rel: &str) -> Result<String> {
        LicenceText::read(&self.dir.join(rel), &self.shown(rel))
    }

    /// (relative path, text) of the licence files at the top (every file of such a
    /// directory), and of the `license-file` the manifest names.
    pub fn notice_files(&self) -> Result<Vec<(String, String)>> {
        let mut rels = Vec::new();
        for name in Self::sorted_names(&self.dir)? {
            if !Self::is_notice(&name) {
                continue;
            }
            let path = self.dir.join(&name);
            if path.is_dir() {
                let mut inner: Vec<String> = TreeWalk::files(&path, |_| true)?
                    .into_iter()
                    .filter(|p| p.is_file())
                    .map(|p| {
                        p.strip_prefix(&self.dir)
                            .unwrap_or(&p)
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect();
                inner.sort();
                rels.extend(inner);
            } else {
                rels.push(name);
            }
        }
        if let Some(file) = self.license_file.as_ref().filter(|f| !f.is_empty()) {
            if !rels.contains(file) {
                rels.push(file.clone());
            }
        }
        rels.into_iter()
            .map(|rel| Ok((rel.clone(), self.text(&rel)?)))
            .collect()
    }

    /// The licence files below the top, outside a top-level licence directory, in walk order.
    pub fn nested_notice_files(&self) -> Result<Vec<String>> {
        let top = self.dir.as_path();
        let keep = |d: &Path| d.parent() != Some(top) || !Self::is_notice(&Self::file_name(d));
        Ok(TreeWalk::files(top, keep)?
            .into_iter()
            .filter(|p| p.parent() != Some(top) && Self::is_notice(&Self::file_name(p)))
            .map(|p| {
                p.strip_prefix(top)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .into_owned()
            })
            .collect())
    }

    pub fn entry(&self) -> Result<Entry> {
        Ok(Entry {
            title: format!("{} {}", self.name, self.version),
            license: self.license.clone(),
            files: self.notice_files()?,
            source: self.source.clone(),
            note: String::new(),
        })
    }

    fn file_name(path: &Path) -> String {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn sorted_names(dir: &Path) -> Result<Vec<String>> {
        let unreadable =
            |e: std::io::Error| ToolError::io(format!("cannot read {}", dir.display()), &e);
        let mut names = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(unreadable)? {
            names.push(
                entry
                    .map_err(unreadable)?
                    .file_name()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        names.sort();
        Ok(names)
    }
}
