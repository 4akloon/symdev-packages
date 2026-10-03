use serde::de::IgnoredAny;
use symdev_sdk::{Host, PackageId};

/// One package's keys as a recipe writes them: at the top of a recipe that makes one
/// package, or in each `[[package]]` table of a recipe whose build makes several (the
/// symdev release makes `symdev` and `rust-sdk` from one tag).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PackageKeys {
    pub(super) id: PackageId,
    pub(super) license: String,
    pub(super) host: Host,
    pub(super) include: Option<Vec<String>>,
    pub(super) sha256: Option<String>,
}

/// A recipe that makes one package. `git` and `tag` (a source tree), `build` and
/// `[[source]]` belong to the build script and CI; the publisher only accepts them, and
/// checks `commit`, the commit the tag must point at (build.sh compares the clone's).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OnePackage {
    id: PackageId,
    license: String,
    host: Host,
    include: Option<Vec<String>>,
    sha256: Option<String>,
    #[serde(rename = "git")]
    _git: Option<IgnoredAny>,
    #[serde(rename = "tag")]
    _tag: Option<IgnoredAny>,
    commit: Option<String>,
    #[serde(rename = "build")]
    _build: Option<IgnoredAny>,
    #[serde(rename = "source")]
    _source: Option<IgnoredAny>,
}

/// A recipe whose one build makes several packages: the build keys at the top, every
/// package in its own `[[package]]` table.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SeveralPackages {
    package: Vec<PackageKeys>,
    #[serde(rename = "git")]
    _git: Option<IgnoredAny>,
    #[serde(rename = "tag")]
    _tag: Option<IgnoredAny>,
    commit: Option<String>,
    #[serde(rename = "build")]
    _build: Option<IgnoredAny>,
    #[serde(rename = "source")]
    _source: Option<IgnoredAny>,
}

impl PackageKeys {
    /// Every package `text` describes, in file order. A `package` key at the top makes it
    /// a several-package recipe, which then takes no package key beside the tables.
    pub(super) fn all_in(text: &str) -> Result<Vec<PackageKeys>, String> {
        let parse_error = |e: toml::de::Error| e.to_string();
        let top: toml::Table = toml::from_str(text).map_err(parse_error)?;
        if top.contains_key("package") {
            let several: SeveralPackages = toml::from_str(text).map_err(parse_error)?;
            Self::check_commit(several.commit.as_deref(), several._tag.is_some())?;
            return Ok(several.package);
        }
        let one: OnePackage = toml::from_str(text).map_err(parse_error)?;
        Self::check_commit(one.commit.as_deref(), one._tag.is_some())?;
        Ok(vec![PackageKeys {
            id: one.id,
            license: one.license,
            host: one.host,
            include: one.include,
            sha256: one.sha256,
        }])
    }

    /// `commit`, when given, is the full SHA-1 of the commit `tag` names: symdev's release
    /// tags cannot move, but the recipe does not rely on that alone.
    fn check_commit(commit: Option<&str>, has_tag: bool) -> Result<(), String> {
        let Some(commit) = commit else {
            return Ok(());
        };
        let hex = |c: char| c.is_ascii_digit() || ('a'..='f').contains(&c);
        if commit.len() != 40 || !commit.chars().all(hex) {
            return Err(format!(
                "commit `{commit}` is not 40 lowercase hex digits (`git rev-parse <tag>^{{commit}}`)"
            ));
        }
        if !has_tag {
            return Err("`commit` pins the commit of `tag`; give `git` and `tag` with it".into());
        }
        Ok(())
    }
}
