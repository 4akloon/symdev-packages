/// What reading a bucket's index does when the index is unsigned.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Unsigned {
    /// An upload: refused, with what an unsigned index can mean.
    Refuse,
    /// A dry run: a warning on the progress output.
    Warn,
    /// `sign-index`, which signs one only with the SHA-256 its dry run printed.
    Accept,
}
