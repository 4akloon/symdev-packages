//! `Component`: one piece of bundled code in a crate, as the `Bundled` table names it.

/// One piece of bundled code the binary links.
pub(super) struct Component {
    /// `{version}` is replaced by what `version` finds.
    pub(super) title: &'static str,
    pub(super) license: &'static str,
    pub(super) files: &'static [&'static str],
    /// A file and the text before the version, which ends at the next `"`.
    pub(super) version: Option<(&'static str, &'static str)>,
    pub(super) note: &'static str,
}
