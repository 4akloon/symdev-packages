//! `GlibcVersion`: a `GLIBC_x.y[.z]` symbol version, and the newest one an ELF file needs.

use std::fmt;

use object::Endianness;
use object::elf::FileHeader64;
use object::read::elf::FileHeader;

use crate::tool_error::{Result, ToolError};

/// Compared number by number, so 2.2.5 < 2.17 < 2.38.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlibcVersion(Vec<u32>);

impl GlibcVersion {
    /// `2.38`, `2.2.5`; `None` for anything else (`PRIVATE`).
    pub fn parse(text: &str) -> Option<GlibcVersion> {
        let parts: Option<Vec<u32>> = text.split('.').map(|p| p.parse().ok()).collect();
        parts.filter(|p| p.len() >= 2).map(GlibcVersion)
    }

    /// The newest `GLIBC_` version the file's version-needs section names; `None` for a
    /// file that is not ELF or needs none. A 32-bit or damaged ELF file is an error.
    pub fn needed_by(data: &[u8]) -> Result<Option<GlibcVersion>> {
        if !data.starts_with(b"\x7fELF") {
            return Ok(None);
        }
        let bad =
            |e: object::read::Error| ToolError::new(format!("not a readable 64-bit ELF file: {e}"));
        let header = FileHeader64::<Endianness>::parse(data).map_err(bad)?;
        let endian = header.endian().map_err(bad)?;
        let sections = header.sections(endian, data).map_err(bad)?;
        let Some((mut needs, link)) = sections.gnu_verneed(endian, data).map_err(bad)? else {
            return Ok(None);
        };
        let strings = sections.strings(endian, data, link).map_err(bad)?;
        let mut newest = None;
        while let Some((_, mut names)) = needs.next().map_err(bad)? {
            while let Some(aux) = names.next().map_err(bad)? {
                let name = aux.name(endian, strings).map_err(bad)?;
                let version = name
                    .strip_prefix(b"GLIBC_")
                    .and_then(|v| std::str::from_utf8(v).ok())
                    .and_then(GlibcVersion::parse);
                newest = newest.max(version);
            }
        }
        Ok(newest)
    }
}

impl fmt::Display for GlibcVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.0.iter().map(u32::to_string).collect();
        f.write_str(&parts.join("."))
    }
}
