//! `LinkMap`: the "Archive member included" section of a GNU ld `-Map`.

use super::Inclusion;
use super::archive_member::ArchiveMember;
use crate::py_text::PyText;
use crate::tool_error::{Result, ToolError};

/// The section's header line; the second is older binutils' wording.
const HEADERS: [&str; 2] = [
    "Archive member included to satisfy reference by file (symbol)",
    "Archive member included because of file (symbol)",
];

pub struct LinkMap;

impl LinkMap {
    /// Every archive member the map's inclusion section lists, in order.
    pub fn included_members(text: &str) -> Result<Vec<Inclusion>> {
        let lines = PyText::split_lines(text);
        let start = lines
            .iter()
            .position(|line| HEADERS.contains(&PyText::strip(line)))
            .ok_or_else(|| ToolError::new("no 'Archive member included' section in the map"))?;
        let mut found = Vec::new();
        // A member line still waiting for its reference line.
        let mut pending: Option<(String, String)> = None;
        for (number, line) in lines.iter().enumerate().skip(start + 2) {
            if PyText::strip(line).is_empty() {
                break;
            }
            if !line.starts_with(PyText::is_space) {
                if pending.is_some() {
                    // The Python tool numbered this one from 0; kept as it was.
                    return Err(ToolError::new(format!(
                        "map line {number}: member without a reference"
                    )));
                }
                let (head, rest) = line.split_once(' ').unwrap_or((line, ""));
                let member = ArchiveMember::parse(head).ok_or_else(|| {
                    ToolError::new(format!(
                        "map line {}: not an archive member: '{line}'",
                        number + 1
                    ))
                })?;
                let (path, member) = (member.path.to_string(), member.member.to_string());
                if PyText::strip(rest).is_empty() {
                    pending = Some((path, member));
                } else {
                    found.push(Self::inclusion(path, member, rest, number)?);
                }
            } else {
                let (path, member) = pending.take().ok_or_else(|| {
                    ToolError::new(format!(
                        "map line {}: a reference with no member before it",
                        number + 1
                    ))
                })?;
                found.push(Self::inclusion(path, member, line, number)?);
            }
        }
        if pending.is_some() {
            return Err(ToolError::new("map ends with a member without a reference"));
        }
        Ok(found)
    }

    /// `referrer (symbol)` or `(symbol)`.
    fn inclusion(path: String, member: String, text: &str, number: usize) -> Result<Inclusion> {
        let text = PyText::strip(text);
        let (referrer, symbol) = if text.starts_with('(') {
            (None, text)
        } else {
            let (referrer, symbol) = text.split_once(' ').unwrap_or((text, ""));
            (Some(referrer.to_string()), symbol)
        };
        let symbol = symbol
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| {
                ToolError::new(format!("map line {}: no (symbol) in '{text}'", number + 1))
            })?;
        Ok(Inclusion {
            path,
            member,
            referrer,
            symbol: symbol.to_string(),
        })
    }
}
