//! `Notices`: THIRD-PARTY-NOTICES.txt as a whole.

use super::{Bundled, DependencyGraph, Entry, Toolchain};
use crate::tool_error::Result;

/// The crates `package` links, the code they bundle and the toolchain's runtime.
pub struct Notices<'a> {
    graph: &'a DependencyGraph,
    package: &'a str,
    toolchain: &'a Toolchain,
}

impl<'a> Notices<'a> {
    pub fn new(graph: &'a DependencyGraph, package: &'a str, toolchain: &'a Toolchain) -> Self {
        Self {
            graph,
            package,
            toolchain,
        }
    }

    pub fn render(&self) -> Result<String> {
        let (crates, members) = self.graph.third_party(self.package)?;
        let crate_entries = crates
            .iter()
            .map(|c| c.entry())
            .collect::<Result<Vec<_>>>()?;
        let mut bundled = Vec::new();
        for krate in &crates {
            bundled.extend(Bundled::entries(krate)?);
        }
        let sections: [(String, Vec<Entry>); 3] = [
            (format!("1. {} Rust crates", crates.len()), crate_entries),
            ("2. Code bundled inside those crates".into(), bundled),
            (
                "3. The Rust toolchain's runtime".into(),
                self.toolchain.entries()?,
            ),
        ];
        let gaps: Vec<&str> = sections
            .iter()
            .flat_map(|(_, entries)| entries.iter().filter(|e| e.files.is_empty()))
            .map(|e| e.title.as_str())
            .collect();
        let target = &self.toolchain.target;
        let mut text = format!(
            "THIRD-PARTY NOTICES for bin/symdev ({target})\n\n\
             bin/symdev is symdev (MIT, see LICENSE beside this file; its {members} workspace \
             crates are covered by it) linked statically with the software below. Each entry \
             gives the licence its authors declare (an SPDX expression) and the full text of \
             every licence, notice and copyright file it ships.\n\n\
             Crates: `cargo metadata --filter-platform {target}` from {} through normal \
             dependencies. Toolchain: {}.\n\n",
            self.package, self.toolchain.rustc
        );
        for (title, entries) in &sections {
            text.push_str(&format!("  {title} ({} entries)\n", entries.len()));
        }
        let gaps = if gaps.is_empty() {
            "none".to_string()
        } else {
            gaps.join(", ")
        };
        text.push_str(&format!("\nEntries without a licence file: {gaps}\n"));
        let rule = "=".repeat(79);
        for (title, entries) in &sections {
            let rendered: Vec<String> = entries.iter().map(Entry::render).collect();
            text.push_str(&format!(
                "\n{rule}\n{title}\n{rule}\n\n{}",
                rendered.join("\n")
            ));
        }
        Ok(text)
    }
}
