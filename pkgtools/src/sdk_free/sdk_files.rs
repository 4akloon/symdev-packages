//! `SdkFiles`: the SHA-256 of every non-empty regular file of an SDK tree, and the search
//! of build outputs for any of them.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::leak_scan::LeakScan;
use crate::py_path::PyPath;
use crate::tool_error::{Result, ToolError};
use crate::tree_walk::TreeWalk;

/// Every file of the SDK by content: the first path (in walk order) with those bytes.
pub struct SdkFiles {
    by_hash: HashMap<[u8; 32], String>,
}

impl SdkFiles {
    pub fn of(root: &Path) -> Result<Self> {
        let root = PyPath::normalized(root);
        if !root.is_dir() {
            return Err(ToolError::new(format!(
                "{} is not a directory",
                root.display()
            )));
        }
        let mut by_hash = HashMap::new();
        for path in TreeWalk::files(&root, |_| true)? {
            if path.is_symlink() || !path.is_file() {
                continue;
            }
            let data = Self::read(&path)?;
            if !data.is_empty() {
                let rel = path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                by_hash.entry(Self::sha256(&data)).or_insert(rel);
            }
        }
        if by_hash.is_empty() {
            return Err(ToolError::new(format!(
                "{} has no file to compare against: is it the SDK?",
                root.display()
            )));
        }
        Ok(Self { by_hash })
    }

    /// How many distinct files the SDK has.
    pub fn len(&self) -> usize {
        self.by_hash.len()
    }

    /// The SDK path of a file with `data`'s bytes; never one for empty data.
    pub fn source_of(&self, data: &[u8]) -> Option<&str> {
        if data.is_empty() {
            return None;
        }
        self.by_hash.get(&Self::sha256(data)).map(String::as_str)
    }

    /// A line for each file under `paths` (directories, tars, .tar.gz) that comes from the
    /// SDK, in the order found; archives are searched member by member, nested ones too.
    pub fn leaks(&self, paths: &[PathBuf]) -> Result<Vec<String>> {
        let mut scan = LeakScan::new(self);
        for path in paths {
            let path = PyPath::normalized(path);
            if path.is_dir() {
                for file in TreeWalk::files(&path, |_| true)? {
                    let rel = file.strip_prefix(&path).unwrap_or(&file).to_string_lossy();
                    scan.file(&file.display().to_string(), &rel, &Self::read(&file)?)?;
                }
            } else if path.is_file() {
                scan.file(&path.display().to_string(), "", &Self::read(&path)?)?;
            } else {
                return Err(ToolError::new(format!("{} does not exist", path.display())));
            }
        }
        Ok(scan.found())
    }

    fn read(path: &Path) -> Result<Vec<u8>> {
        fs::read(path).map_err(|e| ToolError::io(format!("cannot read {}", path.display()), &e))
    }

    fn sha256(data: &[u8]) -> [u8; 32] {
        Sha256::digest(data).into()
    }
}
