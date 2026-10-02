use std::io::Write;

use symdev_sdk::{Index, IndexSigningKey, Result, SdkError, SignedIndex, TrustedKeys};

use crate::bucket::Bucket;
use crate::mode::Mode;

/// The publisher's side of index signatures (symdev spec §14): the key it signs every
/// index with (`PUBLISH_SIGNING_KEY`, which an upload needs) and the keys an index already
/// in a bucket must be signed by — symdev's built-in keys and the signing key's own public
/// half. An index those keys do not verify is never extended or re-signed: otherwise the
/// next publish would sign whatever someone with the bucket's R2 key wrote.
pub struct IndexKeys {
    signer: Option<IndexSigningKey>,
    trusted: TrustedKeys,
}

/// What reading a bucket's index does when the index is unsigned.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Unsigned {
    /// An upload: refused, with the way to sign it.
    Refuse,
    /// A dry run: a warning on the progress output.
    Warn,
    /// `sign-index`, the one way an unsigned index gets its signature.
    Accept,
}

impl IndexKeys {
    pub fn new(signer: Option<IndexSigningKey>) -> IndexKeys {
        let trusted = match &signer {
            Some(key) => TrustedKeys::builtin().and(key.trusted()),
            None => TrustedKeys::builtin(),
        };
        IndexKeys { signer, trusted }
    }

    /// The signing key; a run that must sign fails here, before any request.
    pub fn signer(&self) -> Result<&IndexSigningKey> {
        self.signer.as_ref().ok_or_else(|| {
            SdkError::Other(
                "PUBLISH_SIGNING_KEY is not set; every index an upload writes is signed: set it \
                 to the project key's seed (base64), or pass --dry-run"
                    .into(),
            )
        })
    }

    /// `body` signed with the key, or as it is without one (a dry run).
    pub fn seal(&self, body: &str) -> SignedIndex {
        match &self.signer {
            Some(key) => SignedIndex::sign(body, key),
            None => SignedIndex::split(body),
        }
    }

    /// The bucket's index as stored, `None` if it has none (HTTP 404). A signature must
    /// verify; an unsigned index is refused, warned about on `progress`, or accepted.
    pub fn read(
        &self,
        bucket: &Bucket,
        unsigned: Unsigned,
        progress: &mut dyn Write,
    ) -> Result<Option<SignedIndex>> {
        let Some(text) = bucket.index_text()? else {
            return Ok(None);
        };
        let url = bucket.url("index.toml")?;
        let index = SignedIndex::split(&text);
        let sign_it = format!("publish sign-index --bucket {}", bucket.name());
        if index.is_signed() {
            index.verify(&self.trusted, &url).map_err(|e| {
                SdkError::Other(format!(
                    "{e}. publish neither extends nor re-signs it: find out who wrote it (the \
                     bucket's R2 key may have leaked) and restore a good index.toml first"
                ))
            })?;
        } else if unsigned == Unsigned::Refuse {
            return Err(SdkError::Other(format!(
                "{url} is unsigned, and an upload extends only a signed index; check it and \
                 sign it once with `{sign_it}` (with --dry-run it lists the packages first)"
            )));
        } else if unsigned == Unsigned::Warn {
            say(
                progress,
                &format!(
                    "warning: {url} is unsigned; an upload refuses to extend it until \
                     `{sign_it}` signs it"
                ),
            )?;
        }
        Ok(Some(index))
    }

    /// `publish sign-index`: signs the bucket's index as it is (the same body, byte for
    /// byte) once it parses and any signature it has verifies, listing its archives on
    /// `progress` for the owner to check. A dry run prints the signed index to `out`; an
    /// upload replaces the index (`no-cache`) unless that would change no byte.
    pub fn resign(&self, mode: &Mode, out: &mut dyn Write, progress: &mut dyn Write) -> Result<()> {
        let bucket = match mode {
            Mode::Upload(bucket) => {
                self.signer()?;
                bucket
            }
            Mode::DryRun(Some(bucket)) => bucket,
            Mode::DryRun(None) => {
                return Err(SdkError::Other(
                    "sign-index reads the bucket's index even in a dry run: set \
                     PUBLISH_PUBLIC_URL or PUBLISH_PRIVATE_URL to the bucket's S3 endpoint"
                        .into(),
                ));
            }
        };
        let url = bucket.url("index.toml")?;
        let existing = self
            .read(bucket, Unsigned::Accept, progress)?
            .ok_or_else(|| {
                SdkError::Other(format!(
                    "{url} does not exist, so there is nothing to sign; `publish` writes a signed \
                 index with the first package"
                ))
            })?;
        let index = Index::parse(existing.body(), bucket.name())?;
        let state = match existing.is_signed() {
            true => "signed by a trusted key",
            false => "UNSIGNED: check every archive below before it is signed",
        };
        say(progress, &format!("{url}: {state}"))?;
        for package in &index.packages {
            for a in &package.archives {
                let line = format!("  {} {} {} {}", package.id, a.host, a.sha256, a.size);
                say(progress, &line)?;
            }
        }
        match mode {
            Mode::DryRun(_) => {
                let signed = self.seal(existing.body());
                out.write_all(signed.to_text().as_bytes())
                    .map_err(|source| SdkError::Io {
                        path: "stdout".into(),
                        source,
                    })?;
                say(
                    progress,
                    match signed.is_signed() {
                        true => {
                            "dry run: would upload index.toml (printed above); nothing uploaded"
                        }
                        false => {
                            "dry run: PUBLISH_SIGNING_KEY is not set, so the index printed \
                                  above is unsigned; nothing uploaded"
                        }
                    },
                )
            }
            Mode::Upload(bucket) => {
                let signed = SignedIndex::sign(existing.body(), self.signer()?);
                if signed == existing {
                    return say(
                        progress,
                        &format!("{url} is already signed with this key; nothing to upload"),
                    );
                }
                say(progress, &format!("uploading {url}…"))?;
                bucket.put_index(&signed)?;
                say(progress, &format!("signed {url}"))
            }
        }
    }
}

fn say(progress: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(progress, "{line}").map_err(|source| SdkError::Io {
        path: "stderr".into(),
        source,
    })
}
