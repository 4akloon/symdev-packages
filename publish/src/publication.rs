use std::io::Write;
use std::path::{Path, PathBuf};

use symdev_sdk::{ArchiveEntry, Index, IndexPackage, PackageId, Result, SdkError};

use crate::archive::Archive;
use crate::index_keys::IndexKeys;
use crate::mode::Mode;
use crate::recipe::Recipe;
use crate::unsigned::Unsigned;
use crate::visibility::Visibility;

/// One package to publish: the recipe, the tree it is packed from, and for a public
/// package its corresponding source.
pub struct Publication {
    visibility: Visibility,
    id: PackageId,
    recipe: Recipe,
    from: PathBuf,
    source_code: Option<PathBuf>,
}

impl Publication {
    /// Checks that a public package has its source code and a proprietary licence
    /// (`LicenseRef-…`) goes only to the private bucket.
    pub fn new(
        visibility: Visibility,
        recipe: Recipe,
        from: &Path,
        source_code: Option<&Path>,
    ) -> Result<Publication> {
        let id = recipe.id().clone();
        let refuse = |detail: String| Err(SdkError::Other(detail));
        match (visibility, source_code) {
            (Visibility::Public, _) if recipe.license().starts_with("LicenseRef-") => {
                refuse(format!(
                    "{id} is licensed `{}`, which grants no right to publish it; only the \
                     private bucket may hold it (`publish private`)",
                    recipe.license()
                ))
            }
            (Visibility::Public, None) => refuse(format!(
                "{id} goes to the public bucket with its corresponding source; pass \
                 --source-code <tar.gz>"
            )),
            (Visibility::Private, Some(_)) => refuse(format!(
                "{id}: --source-code is for public packages; the private bucket holds no \
                 source archive"
            )),
            _ => Ok(Publication {
                visibility,
                id,
                recipe,
                from: from.to_path_buf(),
                source_code: source_code.map(Path::to_path_buf),
            }),
        }
    }

    /// Packs the archive into `out_dir/<sha256>.tar.gz`, checks it against the recipe,
    /// adds the package to the bucket's index (refusing an id already there, and an index
    /// whose signature `keys` do not verify), then uploads the archive, the source archive
    /// and last the index, signed. A dry run writes the new index to `out` instead of
    /// uploading anything, signed when `keys` has the signing key. Progress goes to
    /// `progress`.
    pub fn run(
        &self,
        mode: &Mode,
        keys: &IndexKeys,
        out_dir: &Path,
        out: &mut dyn Write,
        progress: &mut dyn Write,
    ) -> Result<()> {
        // An upload signs the index it writes: without the key, nothing is packed or sent.
        let unsigned = match mode {
            Mode::Upload(_) => {
                keys.signer()?;
                Unsigned::Refuse
            }
            Mode::DryRun(_) => Unsigned::Warn,
        };
        let archive = self.recipe.pack(&self.from, out_dir)?;
        let line = format!(
            "packed {}: {} ({} bytes, sha256 {})",
            self.id,
            archive.path.display(),
            archive.size,
            archive.sha256
        );
        say(progress, &line)?;
        self.check_pin(&archive)?;
        let source = self.source_code.as_deref().map(Archive::hash).transpose()?;
        let bucket = match mode {
            Mode::DryRun(bucket) => bucket.as_ref(),
            Mode::Upload(bucket) => Some(bucket),
        };
        let mut index = match bucket {
            Some(bucket) => match keys.read(bucket, unsigned, progress)? {
                Some(stored) => Index::parse(stored.body(), bucket.name())?,
                None => Index::empty(),
            },
            None => {
                say(
                    progress,
                    "dry run without a bucket URL: starting from an empty index",
                )?;
                Index::empty()
            }
        };
        let key = self.key("", &archive);
        let source = source.map(|s| (self.key("src/", &s), s));
        index.insert(IndexPackage {
            id: self.id.clone(),
            license: self.recipe.license().to_string(),
            source_code: source.as_ref().map(|(key, _)| key.clone()),
            depends: vec![],
            archives: vec![ArchiveEntry {
                host: self.recipe.host(),
                url: key.clone(),
                sha256: archive.sha256.clone(),
                size: archive.size,
            }],
        })?;
        let uploads = [Some((key, archive)), source].into_iter().flatten();
        let body = index.to_toml()?;
        match mode {
            Mode::DryRun(_) => {
                let signed = keys.seal(&body, progress)?;
                out.write_all(signed.to_text().as_bytes())
                    .map_err(|e| io_error("stdout", e))?;
                for (key, _) in uploads {
                    say(progress, &format!("dry run: would upload {key}"))?;
                }
                say(
                    progress,
                    "dry run: would upload index.toml (printed above); nothing uploaded",
                )
            }
            Mode::Upload(bucket) => {
                for (key, archive) in uploads {
                    let url = bucket.url(&key)?;
                    say(
                        progress,
                        &format!("uploading {url} ({} bytes)…", archive.size),
                    )?;
                    bucket.put_archive(&key, &archive)?;
                }
                say(
                    progress,
                    &format!("uploading {}…", bucket.url("index.toml")?),
                )?;
                bucket.put_index(&keys.sign(&body, progress)?)?;
                say(
                    progress,
                    &format!("published {} to {}", self.id, bucket.name()),
                )
            }
        }
    }

    /// A pinned SHA-256 must match; a private package must pin one (its tree is the
    /// owner's SDK, so the same tree always gives the same archive).
    fn check_pin(&self, archive: &Archive) -> Result<()> {
        let recipe = self.recipe.path();
        match (self.recipe.sha256(), self.visibility) {
            (Some(pinned), _) if pinned == archive.sha256 => Ok(()),
            (Some(pinned), _) => Err(SdkError::Other(format!(
                "{}: the packed archive has sha256 {}, but {recipe} pins {pinned}; --from is \
                 not the tree the recipe was made from, or the include list changed",
                self.id, archive.sha256
            ))),
            (None, Visibility::Private) => Err(SdkError::Other(format!(
                "{}: the packed archive has sha256 {} ({} bytes); record `sha256 = \"{}\"` \
                 in {recipe} and run again",
                self.id, archive.sha256, archive.size, archive.sha256
            ))),
            (None, Visibility::Public) => Ok(()),
        }
    }

    /// Where an archive is stored: `<prefix><id path>/<sha256>.tar.gz`.
    fn key(&self, prefix: &str, archive: &Archive) -> String {
        let path: Vec<&str> = self.id.segments().collect();
        format!("{prefix}{}/{}.tar.gz", path.join("/"), archive.sha256)
    }
}

fn say(progress: &mut dyn Write, line: &str) -> Result<()> {
    writeln!(progress, "{line}").map_err(|e| io_error("stderr", e))
}

fn io_error(path: &str, source: std::io::Error) -> SdkError {
    SdkError::Io {
        path: path.to_string(),
        source,
    }
}
