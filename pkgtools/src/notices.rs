//! `notices`: THIRD-PARTY-NOTICES.txt of the static symdev binary (spec 2026-10-02 §12):
//! every crate it links, the code those crates bundle, and the Rust toolchain's runtime.

mod bundled;
mod checkout;
mod crate_dir;
mod dependency_graph;
mod entry;
mod html_text;
mod licence_text;
mod notices_file;
mod notices_tool;
mod toolchain;

pub use bundled::Bundled;
pub use crate_dir::Crate;
pub use dependency_graph::DependencyGraph;
pub use entry::Entry;
pub use html_text::HtmlText;
pub use notices_file::Notices;
pub use notices_tool::NoticesTool;
pub use toolchain::Toolchain;

#[cfg(test)]
mod tests;
