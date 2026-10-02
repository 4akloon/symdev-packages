use symdev_sdk::{IndexSigningKey, Result, S3Keys, SdkError};

use crate::bucket::Bucket;
use std::fmt;

use symdev_sdk::TrustedKeys;

use crate::index_keys::IndexKeys;
use crate::mode::Mode;
use crate::visibility::Visibility;

/// The publisher's environment: the buckets' S3 endpoints, the publisher key and the index
/// signing key. `Debug` shows the URLs and only whether each secret is set.
#[derive(Clone, Default)]
pub struct Settings {
    pub private_url: Option<String>,
    pub public_url: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    /// The base64 of the Ed25519 seed every index is signed with (symdev spec §14).
    pub signing_key: Option<String>,
}

const PRIVATE_URL: &str = "PUBLISH_PRIVATE_URL";
const PUBLIC_URL: &str = "PUBLISH_PUBLIC_URL";
const ACCESS_KEY_ID: &str = "PUBLISH_ACCESS_KEY_ID";
const SECRET_ACCESS_KEY: &str = "PUBLISH_SECRET_ACCESS_KEY";
const SIGNING_KEY: &str = "PUBLISH_SIGNING_KEY";

impl Settings {
    /// Reads the five `PUBLISH_*` variables; an empty one counts as unset.
    pub fn from_env() -> Settings {
        let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
        Settings {
            private_url: var(PRIVATE_URL),
            public_url: var(PUBLIC_URL),
            access_key_id: var(ACCESS_KEY_ID),
            secret_access_key: var(SECRET_ACCESS_KEY),
            signing_key: var(SIGNING_KEY),
        }
    }

    /// The keys that sign the indexes a run writes, checked against the keys symdev
    /// trusts. An upload needs the signing key; a dry run signs with it when it is set. A
    /// malformed key is an error even in a dry run, and the error never shows its value.
    pub fn index_keys(&self, dry_run: bool) -> Result<IndexKeys> {
        match (&self.signing_key, dry_run) {
            (Some(text), _) => {
                let key = IndexSigningKey::from_base64(text)
                    .map_err(|e| SdkError::Other(format!("{SIGNING_KEY}: {e}")))?;
                Ok(IndexKeys::new(Some(key), TrustedKeys::builtin()))
            }
            (None, true) => Ok(IndexKeys::new(None, TrustedKeys::builtin())),
            (None, false) => Err(SdkError::Other(format!(
                "{SIGNING_KEY} is not set; every index an upload writes is signed: set it to \
                 the project key's seed (base64, in the owner's ~/.config/symdev/keys.env and \
                 the `publish` environment's secret), or pass --dry-run"
            ))),
        }
    }

    /// An upload needs the bucket URL of `visibility` and the publisher key. A dry run
    /// needs neither: with the URL it reads the bucket's index (signed if the key is set),
    /// without it it starts from an empty index.
    pub fn mode(&self, visibility: Visibility, dry_run: bool) -> Result<Mode> {
        let (name, variable, url) = match visibility {
            Visibility::Private => ("private", PRIVATE_URL, &self.private_url),
            Visibility::Public => ("public", PUBLIC_URL, &self.public_url),
        };
        let keys = match (&self.access_key_id, &self.secret_access_key) {
            (Some(id), Some(secret)) => Some(S3Keys {
                access_key_id: id.clone(),
                secret_access_key: secret.clone(),
            }),
            (None, None) => None,
            (Some(_), None) => return Err(half_a_key(SECRET_ACCESS_KEY, ACCESS_KEY_ID)),
            (None, Some(_)) => return Err(half_a_key(ACCESS_KEY_ID, SECRET_ACCESS_KEY)),
        };
        match (url, dry_run) {
            (None, true) => Ok(Mode::DryRun(None)),
            (Some(url), true) => Ok(Mode::DryRun(Some(Bucket::new(name, url, keys)?))),
            (None, false) => Err(SdkError::Other(format!(
                "{variable} is not set; set it to the {name} bucket's S3 endpoint \
                 (https://<account>.r2.cloudflarestorage.com/symdev-{name}/), or pass --dry-run"
            ))),
            (Some(_), false) if keys.is_none() => Err(SdkError::Other(format!(
                "{ACCESS_KEY_ID} and {SECRET_ACCESS_KEY} are not set; an upload needs the \
                 publisher key (a --dry-run needs neither)"
            ))),
            (Some(url), false) => Ok(Mode::Upload(Bucket::new(name, url, keys)?)),
        }
    }
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let set = |secret: &Option<String>| if secret.is_some() { "<set>" } else { "<unset>" };
        f.debug_struct("Settings")
            .field("private_url", &self.private_url)
            .field("public_url", &self.public_url)
            .field("access_key_id", &set(&self.access_key_id))
            .field("secret_access_key", &set(&self.secret_access_key))
            .field("signing_key", &set(&self.signing_key))
            .finish()
    }
}

fn half_a_key(missing: &str, set: &str) -> SdkError {
    SdkError::Other(format!(
        "{missing} is not set while {set} is; set both or neither"
    ))
}

#[cfg(test)]
mod tests {
    use super::Settings;
    use crate::mode::Mode;
    use crate::visibility::Visibility;

    fn all() -> Settings {
        Settings {
            private_url: Some("https://acct.r2.cloudflarestorage.com/symdev-private/".into()),
            public_url: Some("https://acct.r2.cloudflarestorage.com/symdev-public/".into()),
            access_key_id: Some("AKID".into()),
            secret_access_key: Some("secret".into()),
            signing_key: Some(SEED.into()),
        }
    }

    /// The base64 of the seed of 32 bytes 0x07.
    const SEED: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";

    fn keys_error(s: &Settings, dry_run: bool) -> String {
        match s.index_keys(dry_run) {
            Ok(_) => panic!("expected an error"),
            Err(e) => e.to_string(),
        }
    }

    /// The test seed is not the project key, so an upload with it is refused.
    #[test]
    fn the_signing_key_must_be_one_symdev_trusts() {
        let e = match all().index_keys(false).unwrap().signer() {
            Ok(_) => panic!("the test seed is not a key symdev trusts"),
            Err(e) => e.to_string(),
        };
        assert!(e.starts_with("PUBLISH_SIGNING_KEY"), "{e}");
        assert!(e.contains("bdf5345cc3ca8c30"), "names the trusted key: {e}");
    }

    #[test]
    fn debug_hides_every_secret() {
        let shown = format!("{:?}", all());
        for value in [SEED, "\"secret\"", "AKID"] {
            assert!(!shown.contains(value), "{value} in {shown}");
        }
        assert!(shown.contains("symdev-private"), "{shown}");
    }

    #[test]
    fn an_upload_without_the_signing_key_names_it() {
        let s = Settings {
            signing_key: None,
            ..all()
        };
        let e = keys_error(&s, false);
        assert!(e.starts_with("PUBLISH_SIGNING_KEY is not set"), "{e}");
        assert!(e.contains("--dry-run"), "{e}");
    }

    #[test]
    fn a_dry_run_without_the_signing_key_signs_nothing() {
        let s = Settings {
            signing_key: None,
            ..all()
        };
        assert!(s.index_keys(true).unwrap().signer().is_err());
    }

    #[test]
    fn a_malformed_signing_key_is_named_but_not_shown_even_in_a_dry_run() {
        let s = Settings {
            signing_key: Some("c2VjcmV0LXNlZWQ=".into()),
            ..all()
        };
        let e = keys_error(&s, true);
        assert!(e.starts_with("PUBLISH_SIGNING_KEY: "), "{e}");
        assert!(!e.contains("c2VjcmV0LXNlZWQ"), "{e}");
    }

    fn error(s: &Settings, visibility: Visibility, dry_run: bool) -> String {
        match s.mode(visibility, dry_run) {
            Ok(_) => panic!("expected an error"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn a_dry_run_needs_no_variable() {
        let mode = Settings::default().mode(Visibility::Private, true).unwrap();
        assert!(matches!(mode, Mode::DryRun(None)));
    }

    #[test]
    fn a_dry_run_reads_the_bucket_when_its_url_is_set_even_without_keys() {
        let s = Settings {
            public_url: Some("https://pub-1.r2.dev".into()),
            ..Settings::default()
        };
        let mode = s.mode(Visibility::Public, true).unwrap();
        assert!(matches!(mode, Mode::DryRun(Some(b)) if b.name() == "public"));
    }

    #[test]
    fn an_upload_uses_the_bucket_of_its_visibility() {
        let mode = all().mode(Visibility::Private, false).unwrap();
        let Mode::Upload(bucket) = mode else {
            panic!("expected an upload")
        };
        assert_eq!(bucket.name(), "private");
        assert_eq!(
            bucket.url("index.toml").unwrap(),
            "https://acct.r2.cloudflarestorage.com/symdev-private/index.toml"
        );
    }

    #[test]
    fn an_upload_without_the_bucket_url_names_the_variable() {
        let s = Settings {
            private_url: None,
            public_url: None,
            ..all()
        };
        assert!(error(&s, Visibility::Private, false).contains("PUBLISH_PRIVATE_URL"));
        assert!(error(&s, Visibility::Public, false).contains("PUBLISH_PUBLIC_URL"));
    }

    #[test]
    fn an_upload_without_keys_names_both_variables() {
        let s = Settings {
            access_key_id: None,
            secret_access_key: None,
            ..all()
        };
        let e = error(&s, Visibility::Private, false);
        assert!(
            e.contains("PUBLISH_ACCESS_KEY_ID") && e.contains("PUBLISH_SECRET_ACCESS_KEY"),
            "{e}"
        );
        assert!(e.contains("--dry-run"), "{e}");
    }

    #[test]
    fn half_a_key_is_an_error_naming_the_missing_half_even_in_a_dry_run() {
        let s = Settings {
            secret_access_key: None,
            ..all()
        };
        let e = error(&s, Visibility::Private, true);
        assert!(e.starts_with("PUBLISH_SECRET_ACCESS_KEY is not set"), "{e}");
    }
}
