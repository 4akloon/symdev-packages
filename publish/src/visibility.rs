/// Which bucket a package goes to: `private` (the owner's SDK, never public) or `public`
/// (GPL packages with their source code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Public,
}
