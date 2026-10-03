//! `runtime-closure`: the GCC runtime members a GNU ld link took must be exactly the set
//! the rust-sdk package ships (prebuilt.sh, step 4).

mod archive_member;
mod closure_tool;
mod inclusion;
mod link_map;
mod shipped_set;

pub use closure_tool::ClosureTool;
pub use inclusion::Inclusion;
pub use link_map::LinkMap;
pub use shipped_set::ShippedSet;

#[cfg(test)]
mod tests;
