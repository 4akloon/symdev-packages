use std::io::Write;

use sha2::{Digest, Sha256};
use symdev_sdk::{
    Auth, Fetch, HttpFetch, Result, S3Keys, SdkError, SigV4, SignedIndex, SourceSpec,
};

use crate::archive::Archive;

/// Archives are content-addressed and never change, so caches may keep them for ever.
const ARCHIVE: (&str, &str) = ("application/gzip", "public, max-age=31536000, immutable");
/// The index changes with every publish, so no cache may serve an old one.
const INDEX: (&str, &str) = ("application/toml", "no-cache");

/// A bucket's S3 endpoint (`https://<account>.r2.cloudflarestorage.com/symdev-private/`),
/// or for reading only its public URL. With keys every request is signed (SigV4, region
/// `auto` for R2).
pub struct Bucket {
    spec: SourceSpec,
    fetch: HttpFetch,
}

impl Bucket {
    pub fn new(name: &str, url: &str, keys: Option<S3Keys>) -> Result<Bucket> {
        let auth = if keys.is_some() { Auth::S3 } else { Auth::None };
        let spec = SourceSpec::new(name, url, auth)?;
        if spec.is_file() {
            return Err(SdkError::Other(format!(
                "bucket `{name}`: `{url}` is a directory; give the bucket's S3 endpoint \
                 (https://…)"
            )));
        }
        let signer = keys.map(|keys| SigV4::s3(keys, "auto"));
        Ok(Bucket {
            fetch: HttpFetch::new(&spec, signer),
            spec,
        })
    }

    pub fn name(&self) -> &str {
        &self.spec.name
    }

    /// The URL of the object `key` (relative to the bucket).
    pub fn url(&self, key: &str) -> Result<String> {
        self.spec.resolve(key)
    }

    /// The bucket's `index.toml` as stored, `None` if it has none (HTTP 404). Any other
    /// failure is an error, so an unreadable index is never replaced.
    pub fn index_text(&self) -> Result<Option<String>> {
        match self.fetch.text(&self.spec.index_url()) {
            Ok(text) => Ok(Some(text)),
            Err(SdkError::Fetch { detail, .. }) if detail == "HTTP 404" => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn put_archive(&self, key: &str, archive: &Archive) -> Result<()> {
        let (content_type, cache_control) = ARCHIVE;
        self.put_object(key, archive, content_type, cache_control)
    }

    /// Uploads `file` as the object `key` with the headers given.
    pub fn put_object(
        &self,
        key: &str,
        file: &Archive,
        content_type: &str,
        cache_control: &str,
    ) -> Result<()> {
        let url = self.url(key)?;
        self.fetch
            .put_file(&url, &file.path, &file.sha256, content_type, cache_control)
    }

    /// Uploads `index` as the bucket's `index.toml`.
    pub fn put_index(&self, index: &SignedIndex) -> Result<()> {
        let text = index.to_text();
        let mut file = tempfile::NamedTempFile::new().map_err(|source| SdkError::Io {
            path: std::env::temp_dir().display().to_string(),
            source,
        })?;
        file.write_all(text.as_bytes())
            .and_then(|()| file.flush())
            .map_err(|source| SdkError::Io {
                path: file.path().display().to_string(),
                source,
            })?;
        let sha256 = format!("{:x}", Sha256::digest(text.as_bytes()));
        let (content_type, cache_control) = INDEX;
        self.fetch.put_file(
            &self.spec.index_url(),
            file.path(),
            &sha256,
            content_type,
            cache_control,
        )
    }
}
