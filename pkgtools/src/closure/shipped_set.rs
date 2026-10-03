//! `ShippedSet`: the runtime members the package ships, as `<archive>(<member>)`.

use super::Inclusion;
use super::archive_member::ArchiveMember;
use crate::tool_error::{Result, ToolError};

/// The members prebuilt.sh ships; only the archives they name are checked.
pub struct ShippedSet {
    names: Vec<String>,
    archives: Vec<String>,
}

impl ShippedSet {
    pub fn new(names: &[impl AsRef<str>]) -> Result<Self> {
        let mut archives: Vec<String> = Vec::new();
        for name in names {
            let name = name.as_ref();
            let member = ArchiveMember::parse(name).filter(|m| !m.path.contains('/'));
            let Some(member) = member else {
                return Err(ToolError::new(format!(
                    "'{name}' is not <archive>(<member>), e.g. libgcc.a(pr-support.o)"
                )));
            };
            if !archives.iter().any(|a| a == member.path) {
                archives.push(member.path.to_string());
            }
        }
        let names = names.iter().map(|n| n.as_ref().to_string()).collect();
        Ok(Self { names, archives })
    }

    /// Problems, none when the members `inclusions` took from the named archives are
    /// exactly the shipped ones.
    pub fn check(&self, inclusions: &[Inclusion]) -> Vec<String> {
        // The first inclusion of each member of a named archive, in map order.
        let mut taken: Vec<(String, &Inclusion)> = Vec::new();
        for inclusion in inclusions {
            let name = inclusion.name();
            let named = self.archives.iter().any(|a| a == inclusion.archive());
            if named && !taken.iter().any(|(n, _)| *n == name) {
                taken.push((name, inclusion));
            }
        }
        let mut problems = Vec::new();
        for (name, inclusion) in &taken {
            if !self.names.contains(name) {
                problems.push(format!(
                    "{name} is needed by {} but not shipped: add it to the prebuilt runtime set \
                     (and check its licence)",
                    inclusion.reason()
                ));
            }
        }
        for name in &self.names {
            if !taken.iter().any(|(n, _)| n == name) {
                problems.push(format!("{name} is shipped but no shim needs it"));
            }
        }
        problems
    }
}
