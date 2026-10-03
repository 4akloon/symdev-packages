//! `target2-abs32`: R_ARM_TARGET2 -> R_ARM_ABS32 in GCCE objects, in place (prebuilt.sh,
//! step 2), with symdev's own `Target2Rewrite`.

mod target2_tool;

pub use target2_tool::Target2Tool;

#[cfg(test)]
mod tests;
