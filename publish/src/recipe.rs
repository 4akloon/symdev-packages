use std::fs;
use std::path::Path;

use serde::de::IgnoredAny;
use symdev_sdk::{Host, PackageId, ReproducibleTarGz, Result, SdkError};

use crate::archive::Archive;
use crate::include::Include;

/// A package's `recipe.toml`: what the package is and, for a package packed from a tree
/// that holds more than it (the SDK), which paths of that tree go into it.
#[derive(Clone, Debug)]
pub struct Recipe {
    path: String,
    id: PackageId,
    license: String,
    host: Host,
    include: Option<Vec<Include>>,
    sha256: Option<String>,
}

/// The file as written. `build` and `[[source]]` belong to the build script and CI.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeFile {
    id: PackageId,
    license: String,
    host: Host,
    include: Option<Vec<String>>,
    sha256: Option<String>,
    #[serde(rename = "build")]
    _build: Option<IgnoredAny>,
    #[serde(rename = "source")]
    _source: Option<IgnoredAny>,
}

impl Recipe {
    /// Parses and checks `text`; `path_for_errors` names the file in every error.
    pub fn parse(text: &str, path_for_errors: &str) -> Result<Recipe> {
        let bad = |detail: String| SdkError::Other(format!("recipe {path_for_errors}: {detail}"));
        let file: RecipeFile = toml::from_str(text).map_err(|e| bad(e.to_string()))?;
        if file.license.trim().is_empty() {
            return Err(bad("`license` is empty".into()));
        }
        if let Some(sha) = &file.sha256 {
            let hex = |c: char| c.is_ascii_digit() || ('a'..='f').contains(&c);
            if sha.len() != 64 || !sha.chars().all(hex) {
                return Err(bad(format!(
                    "sha256 `{sha}` is not 64 lowercase hex digits"
                )));
            }
        }
        let include = match file.include {
            None => None,
            Some(list) if list.is_empty() => {
                return Err(bad(
                    "`include` lists nothing; remove it to pack the whole tree".into(),
                ));
            }
            Some(list) => Some(
                list.iter()
                    .map(|p| Include::parse(p).map_err(|e| bad(e.to_string())))
                    .collect::<Result<Vec<_>>>()?,
            ),
        };
        Ok(Recipe {
            path: path_for_errors.to_string(),
            id: file.id,
            license: file.license,
            host: file.host,
            include,
            sha256: file.sha256,
        })
    }

    /// Where the recipe was read from, for messages.
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn id(&self) -> &PackageId {
        &self.id
    }

    pub fn license(&self) -> &str {
        &self.license
    }

    pub fn host(&self) -> Host {
        self.host
    }

    /// The archive's SHA-256 the recipe pins, if it pins one.
    pub fn sha256(&self) -> Option<&str> {
        self.sha256.as_deref()
    }

    /// Packs the package from `from` into `out_dir/<sha256>.tar.gz`: with an include list,
    /// byte copies of what it names, staged in a temporary directory; without one, all of
    /// `from`. The archive is reproducible, so the same tree gives the same SHA-256.
    pub fn pack(&self, from: &Path, out_dir: &Path) -> Result<Archive> {
        let part = out_dir.join(format!(".publish-{}.tar.gz.part", std::process::id()));
        let packed = match &self.include {
            None => ReproducibleTarGz::pack(from, &["."], &part),
            Some(list) => Self::staged(from, list, &part),
        };
        let (sha256, size) = packed.inspect_err(|_| {
            let _ = fs::remove_file(&part);
        })?;
        let path = out_dir.join(format!("{sha256}.tar.gz"));
        fs::rename(&part, &path).map_err(|source| SdkError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Ok(Archive { path, sha256, size })
    }

    fn staged(from: &Path, list: &[Include], out: &Path) -> Result<(String, u64)> {
        let staging = tempfile::tempdir().map_err(|source| SdkError::Io {
            path: std::env::temp_dir().display().to_string(),
            source,
        })?;
        for include in list {
            include.copy(from, staging.path())?;
        }
        ReproducibleTarGz::pack(staging.path(), &["."], out)
    }
}

#[cfg(test)]
mod tests;
