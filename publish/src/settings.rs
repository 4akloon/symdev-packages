use symdev_sdk::{Result, S3Keys, SdkError};

use crate::bucket::Bucket;
use crate::mode::Mode;
use crate::visibility::Visibility;

/// The publisher's environment: the buckets' S3 endpoints and the publisher key.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub private_url: Option<String>,
    pub public_url: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

const PRIVATE_URL: &str = "PUBLISH_PRIVATE_URL";
const PUBLIC_URL: &str = "PUBLISH_PUBLIC_URL";
const ACCESS_KEY_ID: &str = "PUBLISH_ACCESS_KEY_ID";
const SECRET_ACCESS_KEY: &str = "PUBLISH_SECRET_ACCESS_KEY";

impl Settings {
    /// Reads the four `PUBLISH_*` variables; an empty one counts as unset.
    pub fn from_env() -> Settings {
        let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
        Settings {
            private_url: var(PRIVATE_URL),
            public_url: var(PUBLIC_URL),
            access_key_id: var(ACCESS_KEY_ID),
            secret_access_key: var(SECRET_ACCESS_KEY),
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
        }
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
