//! `ArchiveMember`: `<archive>(<member>)`, as the map and the shipped set spell a member.

/// The regex `^(?P<path>.+)\((?P<member>[^()]+)\)$` of the Python tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveMember<'a> {
    pub path: &'a str,
    pub member: &'a str,
}

impl<'a> ArchiveMember<'a> {
    pub fn parse(text: &'a str) -> Option<Self> {
        let inner = text.strip_suffix(')')?;
        let open = inner.rfind('(')?;
        let (path, member) = (&inner[..open], &inner[open + 1..]);
        let well_formed = !path.is_empty() && !member.is_empty() && !member.contains(')');
        (well_formed && !path.contains('\n')).then_some(Self { path, member })
    }

    /// The last segment of `path`, as `os.path.basename`.
    pub fn basename(path: &str) -> &str {
        path.rsplit('/').next().unwrap_or(path)
    }

    /// `/a/b/libx.a(m.o)` -> `libx.a(m.o)`, `/a/b/c.o` -> `c.o`.
    pub fn short(file: &str) -> String {
        match ArchiveMember::parse(file) {
            Some(m) => format!("{}({})", Self::basename(m.path), m.member),
            None => Self::basename(file).to_string(),
        }
    }
}
