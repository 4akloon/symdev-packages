use symdev_sdk::{Result, SdkError};

/// Where a plain file goes in a bucket (`install.sh`): a relative path of `/`-separated
/// segments made of RFC 3986's unreserved characters only. The key becomes the URL path,
/// which the request signer percent-decodes before signing, so `%` (or any character that
/// would need escaping) could make the stored name differ from the one given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectKey(String);

/// The bucket's index, which only `publish public|private|sign-index` write, signed.
const INDEX: &str = "index.toml";

impl ObjectKey {
    pub fn parse(key: &str) -> Result<ObjectKey> {
        let refuse = |reason: &str| {
            Err(SdkError::Other(format!(
                "key `{key}` {reason}; give a path relative to the bucket root, e.g. `install.sh`"
            )))
        };
        let unreserved = |c: char| c.is_ascii_alphanumeric() || "-._~/".contains(c);
        if key == INDEX {
            return Err(SdkError::Other(format!(
                "key `{INDEX}` is the bucket's index, which only `publish public`, `publish \
                 private` and `publish sign-index` write, signed; upload the file under another \
                 key"
            )));
        }
        if key.is_empty() {
            return refuse("is empty");
        }
        if key.starts_with('/') {
            return refuse("starts with `/`");
        }
        if !key.chars().all(unreserved) {
            return refuse("has a character outside A-Z a-z 0-9 - . _ ~ and `/`");
        }
        for segment in key.split('/') {
            match segment {
                "" => return refuse("has an empty segment"),
                "." => return refuse("has a `.` segment"),
                ".." => return refuse("has a `..` segment"),
                _ => {}
            }
        }
        Ok(ObjectKey(key.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests;
