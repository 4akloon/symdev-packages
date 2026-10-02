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
/// `[[source]]` belong to the build script and CI; the publisher only accepts them.
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
    #[serde(rename = "build")]
    _build: Option<IgnoredAny>,
    #[serde(rename = "source")]
    _source: Option<IgnoredAny>,
}

impl PackageKeys {
    /// Every package `text` describes, in file order. A `package` key at the top makes it
    /// a several-package recipe, which then takes no package key beside the tables.
    pub(super) fn all_in(text: &str) -> Result<Vec<PackageKeys>, toml::de::Error> {
        let top: toml::Table = toml::from_str(text)?;
        if top.contains_key("package") {
            return Ok(toml::from_str::<SeveralPackages>(text)?.package);
        }
        let one: OnePackage = toml::from_str(text)?;
        Ok(vec![PackageKeys {
            id: one.id,
            license: one.license,
            host: one.host,
            include: one.include,
            sha256: one.sha256,
        }])
    }
}
