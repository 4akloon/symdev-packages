//! `LeakScan`: one search of build outputs, archives opened member by member.

use std::io::Read;

use flate2::read::MultiGzDecoder;

use super::SdkFiles;
use crate::tool_error::{Result, ToolError};

/// What one search found so far, in the order found.
pub struct LeakScan<'a> {
    sdk: &'a SdkFiles,
    found: Vec<String>,
}

impl<'a> LeakScan<'a> {
    pub fn new(sdk: &'a SdkFiles) -> Self {
        Self {
            sdk,
            found: Vec::new(),
        }
    }

    pub fn found(self) -> Vec<String> {
        self.found
    }

    /// `name` is shown; `rel` is the part below what the caller named (a member's name, a
    /// path inside a directory), which alone is checked for epoc32/.
    pub fn file(&mut self, name: &str, rel: &str, data: &[u8]) -> Result<()> {
        if Self::through_epoc32(rel) {
            self.found.push(format!("{name}: a path through epoc32/"));
        }
        if let Some(source) = self.sdk.source_of(data) {
            self.found
                .push(format!("{name}: the bytes of the SDK's {source}"));
        }
        if [".tar", ".tar.gz", ".tgz"]
            .iter()
            .any(|s| name.ends_with(s))
        {
            self.archive(name, data)?;
        }
        Ok(())
    }

    /// The SDK's own layout: a path segment `epoc32`, in any case.
    fn through_epoc32(name: &str) -> bool {
        name.split('/').any(|part| part.to_lowercase() == "epoc32")
    }

    /// A tar, plain or gzipped, as Python's `tarfile.open(mode="r:*")` read it, except that
    /// bzip2/xz/zstd are refused and a damaged header after the first is an error. An
    /// error names the innermost archive that cannot be read.
    fn archive(&mut self, name: &str, data: &[u8]) -> Result<()> {
        let unreadable =
            |e: &dyn std::fmt::Display| ToolError::new(format!("cannot read {name} as a tar: {e}"));
        let other = [
            (&b"BZh"[..], "bzip2"),
            (b"\xfd7zXZ\0", "xz"),
            (b"\x28\xb5\x2f\xfd", "zstd"),
        ];
        if let Some((_, kind)) = other.iter().find(|(magic, _)| data.starts_with(magic)) {
            return Err(unreadable(&format!(
                "{kind} is not read here; the pipeline makes tar and gzip only"
            )));
        }
        let mut unpacked = Vec::new();
        let tar_bytes = if data.starts_with(&[0x1f, 0x8b]) {
            MultiGzDecoder::new(data)
                .read_to_end(&mut unpacked)
                .map_err(|e| unreadable(&e))?;
            unpacked.as_slice()
        } else {
            data
        };
        // tarfile refuses a stream with no header at all; a tar of only zero blocks is empty.
        if tar_bytes.is_empty() {
            return Err(unreadable(&"empty file"));
        }
        let mut tar = tar::Archive::new(tar_bytes);
        for entry in tar.entries().map_err(|e| unreadable(&e))? {
            let mut entry = entry.map_err(|e| unreadable(&e))?;
            let kind = entry.header().entry_type();
            if kind.is_pax_global_extensions() {
                continue;
            }
            let mut member = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
            // Python's tarfile: a V7 regular entry whose name ends in `/` is a directory, and
            // a directory's name has no trailing `/`.
            let v7_dir = entry.header().as_bytes()[156] == 0 && member.ends_with('/');
            let is_dir = kind.is_dir() || v7_dir;
            if is_dir {
                member = member.trim_end_matches('/').to_string();
            }
            let inner = format!("{name}!{member}");
            let is_file =
                !is_dir && (kind.is_file() || kind.is_contiguous() || kind.is_gnu_sparse());
            if is_file {
                let mut content = Vec::new();
                entry
                    .read_to_end(&mut content)
                    .map_err(|e| unreadable(&e))?;
                self.file(&inner, &member, &content)?;
            } else if Self::through_epoc32(&member) {
                self.found.push(format!("{inner}: a path through epoc32/"));
            }
        }
        Ok(())
    }
}
