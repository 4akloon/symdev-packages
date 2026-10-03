//! `BundledList`: the artifact's package list (D1 = A), each package with the copyright file
//! linuxdeploy put into the tree for it.

use std::fs;
use std::path::Path;

use crate::tool_error::{Result, ToolError};

pub struct BundledList {
    /// The list's line and the copyright file, relative to the tree.
    rows: Vec<(String, String)>,
}

impl BundledList {
    pub fn read(tsv: &Path, tree: &Path) -> Result<BundledList> {
        let text = fs::read_to_string(tsv).map_err(|e| ToolError::io(tsv.display(), &e))?;
        let (mut rows, mut missing) = (Vec::new(), Vec::new());
        for line in text.lines().filter(|l| !l.is_empty()) {
            let fields: Vec<&str> = line.split('\t').collect();
            let [package, _, _, _] = fields[..] else {
                return Err(ToolError::new(format!(
                    "{}: `{line}` is not package, version, source, source version",
                    tsv.display()
                )));
            };
            let name = package.split(':').next().unwrap_or(package);
            let copyright = format!("usr/share/doc/{name}/copyright");
            if !tree.join(&copyright).is_file() {
                missing.push(copyright.clone());
            }
            rows.push((line.to_string(), copyright));
        }
        if !missing.is_empty() {
            return Err(ToolError::new(format!(
                "the tree has no {}: a bundled package without its licence",
                missing.join(", ")
            )));
        }
        Ok(BundledList { rows })
    }

    pub fn count(&self) -> usize {
        self.rows.len()
    }

    pub fn to_tsv(&self) -> String {
        let mut out = String::from("package\tversion\tsource\tsource version\tcopyright\n");
        for (line, copyright) in &self.rows {
            out.push_str(&format!("{line}\t{copyright}\n"));
        }
        out
    }
}
