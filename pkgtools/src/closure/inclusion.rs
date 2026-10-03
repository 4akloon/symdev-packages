//! `Inclusion`: one archive member a link took, who wanted it and for which symbol.

use super::archive_member::ArchiveMember;

/// A line pair of the map's "Archive member included" section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inclusion {
    /// The archive as the map spells it.
    pub path: String,
    pub member: String,
    /// `None`: an `-u` or entry symbol of the command line.
    pub referrer: Option<String>,
    pub symbol: String,
}

impl Inclusion {
    pub fn archive(&self) -> &str {
        ArchiveMember::basename(&self.path)
    }

    /// `libgcc.a(pr-support.o)`.
    pub fn name(&self) -> String {
        format!("{}({})", self.archive(), self.member)
    }

    /// `libsupc++.a(eh_personality.o) (__gnu_unwind_frame)`.
    pub fn reason(&self) -> String {
        let referrer = match &self.referrer {
            None => "the command line".to_string(),
            Some(file) => ArchiveMember::short(file),
        };
        format!("{referrer} ({})", self.symbol)
    }
}
