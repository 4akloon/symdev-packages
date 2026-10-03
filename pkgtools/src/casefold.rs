//! `sdk-casefold`: a case-insensitive overlay of the S60 SDK's epoc32/include.

mod include_overlay;

pub use include_overlay::IncludeOverlay;

#[cfg(test)]
mod tests;
