use std::io::Write;

use sha2::{Digest, Sha256};
use symdev_sdk::{Index, IndexSigningKey, Result, SdkError, SignedIndex, TrustedKeys};

use crate::bucket::Bucket;
use crate::mode::Mode;
use crate::unsigned::Unsigned;

/// The publisher's side of index signatures (symdev spec §14): the key it signs every
/// index with (`PUBLISH_SIGNING_KEY`), the keys clients trust (symdev's built-in keys),
/// which an upload's key must be one of, and the keys an index already in a bucket must be
/// signed by — those and the signing key's own. An index those keys do not verify is never
/// extended or re-signed: otherwise the next publish would sign whatever someone with the
/// bucket's R2 key wrote.
pub struct IndexKeys {
    signer: Option<IndexSigningKey>,
    clients: TrustedKeys,
    trusted: TrustedKeys,
}

impl IndexKeys {
    pub fn new(signer: Option<IndexSigningKey>, clients: TrustedKeys) -> IndexKeys {
        let trusted = match &signer {
            Some(key) => clients.clone().and(key.trusted()),
            None => clients.clone(),
        };
        IndexKeys {
            signer,
            clients,
            trusted,
        }
    }

    /// The key an upload signs with, which clients must trust: a missing key, or one a
    /// stale or mistyped `PUBLISH_SIGNING_KEY` holds, fails here, before any request.
    pub fn signer(&self) -> Result<&IndexSigningKey> {
        let key = self.signer.as_ref().ok_or_else(|| {
            SdkError::Other(
                "PUBLISH_SIGNING_KEY is not set; every index an upload writes is signed: set it \
                 to the project key's seed (base64), or pass --dry-run"
                    .into(),
            )
        })?;
        if !self.clients.includes(&key.trusted()) {
            return Err(SdkError::Other(format!(
                "PUBLISH_SIGNING_KEY is the key {}, which symdev does not trust (it trusts {}); \
                 every client would refuse the index it signs: set it to the project key's seed",
                short(&key.trusted()),
                short(&self.clients)
            )));
        }
        Ok(key)
    }

    /// What a dry run prints: `body` signed with the key if there is one, saying on
    /// `progress` when there is none or clients would not trust it.
    pub fn seal(&self, body: &str, progress: &mut dyn Write) -> Result<SignedIndex> {
        let Some(key) = &self.signer else {
            say(
                progress,
                "dry run: PUBLISH_SIGNING_KEY is not set, so the index printed is unsigned; an \
                 upload signs it",
            )?;
            return Ok(SignedIndex::split(body));
        };
        if !self.clients.includes(&key.trusted()) {
            let line = format!(
                "dry run: PUBLISH_SIGNING_KEY is the key {}, not a key symdev trusts ({}); an \
                 upload refuses it",
                short(&key.trusted()),
                short(&self.clients)
            );
            say(progress, &line)?;
        }
        Ok(SignedIndex::sign(body, key))
    }

    /// `body` signed for an upload, saying on `progress` which key signs it.
    pub fn sign(&self, body: &str, progress: &mut dyn Write) -> Result<SignedIndex> {
        let key = self.signer()?;
        say(
            progress,
            &format!("signing with the key {}", short(&key.trusted())),
        )?;
        Ok(SignedIndex::sign(body, key))
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
                "{url} is unsigned, and an upload extends only a signed index. Every index \
                 publish writes is signed, so this is either one published before signing \
                 existed, to be signed once with `{sign_it}` after its --dry-run is checked, \
                 or one someone else wrote (the bucket's R2 key may have leaked): then restore \
                 a known-good index.toml"
            )));
        } else if unsigned == Unsigned::Warn {
            let line = format!(
                "warning: {url} is unsigned; an upload refuses to extend it (see `{sign_it}`)"
            );
            say(progress, &line)?;
        }
        Ok(Some(index))
    }

    /// `publish sign-index`: signs the bucket's index as it is (the same body, byte for
    /// byte) once it parses and any signature it has verifies, listing its archives on
    /// `progress` for the owner to check. An unsigned index is signed only when
    /// `accept_unsigned` is the SHA-256 its dry run printed, so exactly the reviewed bytes
    /// are signed. A dry run prints the signed index to `out`; an upload replaces the index
    /// (`no-cache`) unless that would change no byte.
    pub fn resign(
        &self,
        mode: &Mode,
        accept_unsigned: Option<&str>,
        out: &mut dyn Write,
        progress: &mut dyn Write,
    ) -> Result<()> {
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
        let digest = format!("{:x}", Sha256::digest(existing.body().as_bytes()));
        let state = match existing.is_signed() {
            true => "signed by a trusted key".to_string(),
            false => format!("UNSIGNED (sha256 {digest}): check every archive below"),
        };
        say(progress, &format!("{url}: {state}"))?;
        for package in &index.packages {
            for a in &package.archives {
                let line = format!("  {} {} {} {}", package.id, a.host, a.sha256, a.size);
                say(progress, &line)?;
            }
        }
        Self::check_acceptance(&existing, &digest, accept_unsigned, mode)?;
        match mode {
            Mode::DryRun(_) => {
                let signed = self.seal(existing.body(), progress)?;
                out.write_all(signed.to_text().as_bytes())
                    .map_err(|source| SdkError::Io {
                        path: "stdout".into(),
                        source,
                    })?;
                if !existing.is_signed() {
                    let line = format!(
                        "dry run: if every archive above is right, sign exactly this index \
                         with `publish sign-index --bucket {} --accept-unsigned {digest}`",
                        bucket.name()
                    );
                    say(progress, &line)?;
                }
                say(
                    progress,
                    "dry run: index.toml printed above; nothing uploaded",
                )
            }
            Mode::Upload(bucket) => {
                let signed = self.sign(existing.body(), progress)?;
                if signed == existing {
                    let line = format!("{url} is already signed with this key; nothing to upload");
                    return say(progress, &line);
                }
                say(progress, &format!("uploading {url}…"))?;
                bucket.put_index(&signed)?;
                say(progress, &format!("signed {url}"))
            }
        }
    }

    /// An upload signs an unsigned index only when `accept` is its body's SHA-256 (`digest`);
    /// `accept` is refused for a signed index, which needs no such review.
    fn check_acceptance(
        existing: &SignedIndex,
        digest: &str,
        accept: Option<&str>,
        mode: &Mode,
    ) -> Result<()> {
        let refuse = |detail: &str| Err(SdkError::Other(detail.to_string()));
        match (existing.is_signed(), accept, mode) {
            (true, Some(_), _) => refuse(
                "the index is signed, so --accept-unsigned does not apply; drop it to re-sign \
                 the index",
            ),
            (false, None, Mode::Upload(_)) => refuse(
                "the index is unsigned: run sign-index with --dry-run, check every archive it \
                 lists against your records, and pass the SHA-256 it prints as \
                 --accept-unsigned",
            ),
            (false, Some(given), _) if given != digest => refuse(
                "the unsigned index changed since it was reviewed (its SHA-256 is not the one \
                 --accept-unsigned gives); run sign-index --dry-run again and check it",
            ),
            _ => Ok(()),
        }
    }
}

/// `keys` as a message names them: the first 16 hex digits of each fingerprint.
fn short(keys: &TrustedKeys) -> String {
    let short: Vec<String> = keys
        .fingerprints()
        .iter()
        .map(|f| format!("{}…", &f[..16.min(f.len())]))
        .collect();
    short.join(", ")
}

fn say(progress: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(progress, "{line}").map_err(|source| SdkError::Io {
        path: "stderr".into(),
        source,
    })
}
