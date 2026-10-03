//! `ArArchive`: the members of a System V / GNU (or BSD) `ar` archive, read as binutils'
//! `ar t` lists them and `ar p` prints them.

use std::path::Path;

use crate::tool_error::{Result, ToolError};

/// Every member but the archive's own tables (symbol index, long-name table), in order.
pub struct ArArchive {
    members: Vec<(String, Vec<u8>)>,
}

impl ArArchive {
    const MAGIC: &[u8] = b"!<arch>\n";
    const HEADER: usize = 60;

    pub fn read(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)
            .map_err(|e| ToolError::io(format!("cannot read {}", path.display()), &e))?;
        Self::parse(&data).map_err(|e| ToolError::new(format!("{}: {e}", path.display())))
    }

    fn parse(data: &[u8]) -> std::result::Result<Self, String> {
        if !data.starts_with(Self::MAGIC) {
            return Err("not an ar archive (no !<arch> magic)".into());
        }
        let (mut members, mut long_names) = (Vec::new(), Vec::new());
        let mut at = Self::MAGIC.len();
        while at < data.len() {
            let header = data
                .get(at..at + Self::HEADER)
                .ok_or_else(|| format!("truncated member header at byte {at}"))?;
            if &header[58..60] != b"`\n" {
                return Err(format!("bad member header at byte {at}"));
            }
            let size = std::str::from_utf8(&header[48..58])
                .ok()
                .and_then(|s| s.trim_end().parse::<usize>().ok())
                .ok_or_else(|| format!("bad member size at byte {at}"))?;
            let start = at + Self::HEADER;
            let mut body = data
                .get(start..start + size)
                .ok_or_else(|| format!("member at byte {at} runs past the end"))?;
            let field = String::from_utf8_lossy(&header[..16])
                .trim_end()
                .to_string();
            at = start + size + (size & 1);
            let name = match field.as_str() {
                "/" | "/SYM64/" | "__.SYMDEF" | "__.SYMDEF SORTED" => continue,
                "//" => {
                    long_names = body.to_vec();
                    continue;
                }
                _ if field.starts_with("#1/") => {
                    let length: usize = field[3..]
                        .parse()
                        .map_err(|_| format!("bad BSD name {field}"))?;
                    let name = body
                        .get(..length)
                        .ok_or_else(|| format!("bad BSD name {field}"))?;
                    let name = String::from_utf8_lossy(name)
                        .trim_end_matches('\0')
                        .to_string();
                    body = &body[length..];
                    name
                }
                _ if field.len() > 1 && field.starts_with('/') => {
                    let offset: usize = field[1..]
                        .parse()
                        .map_err(|_| format!("bad name {field}"))?;
                    let rest = long_names
                        .get(offset..)
                        .ok_or_else(|| format!("long name {field} past the name table"))?;
                    let end = rest
                        .windows(2)
                        .position(|w| w == b"/\n")
                        .unwrap_or(rest.len());
                    String::from_utf8_lossy(&rest[..end]).into_owned()
                }
                _ => field.strip_suffix('/').unwrap_or(&field).to_string(),
            };
            members.push((name, body.to_vec()));
        }
        Ok(Self { members })
    }

    pub fn names(&self) -> Vec<&str> {
        self.members.iter().map(|(name, _)| name.as_str()).collect()
    }

    /// `ar p <archive> <name>`: every member by that name, in order; `None` if there is none.
    pub fn member(&self, name: &str) -> Option<Vec<u8>> {
        let mut found = self.members.iter().filter(|(n, _)| n == name).peekable();
        found.peek()?;
        Some(found.flat_map(|(_, data)| data.iter().copied()).collect())
    }
}

#[cfg(test)]
mod tests;
