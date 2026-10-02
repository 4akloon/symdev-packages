use std::io::Write;
use std::path::Path;

use symdev_sdk::{Result, SdkError};

use crate::archive::Archive;
use crate::mode::Mode;
use crate::object_key::ObjectKey;

/// One local file put at a fixed key of a bucket with the headers given (`install.sh` at
/// the public bucket's root). The object is mutable: a later upload replaces it, unlike the
/// content-addressed archives, and the bucket's index is neither read nor written.
pub struct FileUpload {
    file: Archive,
    key: ObjectKey,
    content_type: String,
    cache_control: String,
}

impl FileUpload {
    /// Hashes `path` and checks that both header values can be sent as they are.
    pub fn new(
        path: &Path,
        key: ObjectKey,
        content_type: &str,
        cache_control: &str,
    ) -> Result<FileUpload> {
        header_value("--content-type", content_type)?;
        header_value("--cache-control", cache_control)?;
        Ok(FileUpload {
            file: Archive::hash(path)?,
            key,
            content_type: content_type.to_string(),
            cache_control: cache_control.to_string(),
        })
    }

    /// Uploads the file (signed when the bucket has keys); a dry run only says what it
    /// would upload. Progress goes to `progress`.
    pub fn run(&self, mode: &Mode, progress: &mut dyn Write) -> Result<()> {
        let key = self.key.as_str();
        let what = format!(
            "{} ({} bytes, sha256 {}) as {}, Content-Type: {}, Cache-Control: {}",
            self.file.path.display(),
            self.file.size,
            self.file.sha256,
            key,
            self.content_type,
            self.cache_control
        );
        match mode {
            Mode::DryRun(bucket) => {
                let target = match bucket {
                    Some(bucket) => bucket.url(key)?,
                    None => format!("{key} (no bucket URL set)"),
                };
                say(
                    progress,
                    &format!("dry run: would upload {what} to {target}; nothing uploaded"),
                )
            }
            Mode::Upload(bucket) => {
                say(
                    progress,
                    &format!("uploading {what} to {}…", bucket.url(key)?),
                )?;
                bucket.put_object(key, &self.file, &self.content_type, &self.cache_control)?;
                say(progress, &format!("uploaded {key} to {}", bucket.name()))
            }
        }
    }
}

/// A header value goes on the wire unchanged: it must be non-empty visible ASCII or spaces.
fn header_value(flag: &str, value: &str) -> Result<()> {
    let problem = if value.trim().is_empty() {
        "is empty"
    } else if !value.chars().all(|c| c == ' ' || c.is_ascii_graphic()) {
        "has a character other than visible ASCII and spaces"
    } else {
        return Ok(());
    };
    Err(SdkError::Other(format!(
        "{flag} `{}` {problem}; give a plain header value, e.g. `text/plain; charset=utf-8`",
        value.escape_debug()
    )))
}

fn say(progress: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(progress, "{line}").map_err(|source| SdkError::Io {
        path: "stderr".into(),
        source,
    })
}
