//! Every index `publish` writes is signed, and it extends only an index whose signature
//! verifies (symdev spec §14).

use symdev_sdk::{IndexSigningKey, SignedIndex, TrustedKeys};

use super::fake_bucket::FakeBucket;
use super::{SEED, upload_to};
use super::{index_keys, pinned_sdk_recipe, private, run, run_with, sdk_tree, signing_key};
use crate::bucket::Bucket;
use crate::index_keys::IndexKeys;
use crate::mode::Mode;

/// Another seed (32 bytes 0x08), whose signature the tests' keys do not trust.
const OTHER_SEED: &str = "CAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAg=";
const OLD: &str = "schema = 1\n";

fn uploaded_text(bucket: &FakeBucket) -> String {
    String::from_utf8(
        bucket
            .object("index.toml")
            .expect("index.toml was uploaded"),
    )
    .unwrap()
}

/// Publishes the SDK from `sdk_tree` to `bucket`; returns the error, if any, as text.
fn publish_sdk(bucket: &FakeBucket, keys: &IndexKeys) -> Option<String> {
    let sdk = sdk_tree();
    let p = private(sdk.path(), pinned_sdk_recipe(sdk.path()));
    run_with(&p, &upload_to(bucket, "private"), keys)
        .0
        .err()
        .map(|e| e.to_string())
}

#[test]
fn an_upload_signs_the_index_it_writes() {
    let bucket = FakeBucket::start();
    let sdk = sdk_tree();
    let p = private(sdk.path(), pinned_sdk_recipe(sdk.path()));
    let (result, _, progress, _) = run_with(&p, &upload_to(&bucket, "private"), &index_keys());
    result.unwrap();
    let fingerprint = &signing_key().trusted().fingerprints()[0];
    assert!(progress.contains(&fingerprint[..16]), "{progress}");
    let text = uploaded_text(&bucket);
    assert!(text.starts_with("# symdev-signature: ed25519 "), "{text}");
    SignedIndex::split(&text)
        .verify(&signing_key().trusted(), "index.toml")
        .unwrap();
}

#[test]
fn an_upload_without_the_signing_key_sends_nothing() {
    let bucket = FakeBucket::start();
    let e = publish_sdk(&bucket, &IndexKeys::new(None, signing_key().trusted())).unwrap();
    assert!(
        e.contains("PUBLISH_SIGNING_KEY") && e.contains("--dry-run"),
        "{e}"
    );
    assert!(bucket.calls().is_empty(), "{:?}", bucket.calls());
}

/// A stale or mistyped key would sign an index every client refuses: an outage that the
/// right key could not even repair, since it does not verify what the wrong one signed.
#[test]
fn an_upload_with_a_key_symdev_does_not_trust_sends_nothing() {
    let bucket = FakeBucket::start();
    let keys = IndexKeys::new(Some(signing_key()), TrustedKeys::builtin());
    let e = publish_sdk(&bucket, &keys).unwrap();
    let ours = &signing_key().trusted().fingerprints()[0];
    let builtin = &TrustedKeys::builtin().fingerprints()[0];
    assert!(e.starts_with("PUBLISH_SIGNING_KEY"), "{e}");
    assert!(e.contains(&ours[..16]) && e.contains(&builtin[..16]), "{e}");
    assert!(bucket.calls().is_empty(), "{:?}", bucket.calls());
}

#[test]
fn a_dry_run_with_a_key_symdev_does_not_trust_says_so() {
    let sdk = sdk_tree();
    let p = private(sdk.path(), pinned_sdk_recipe(sdk.path()));
    let keys = IndexKeys::new(Some(signing_key()), TrustedKeys::builtin());
    let (result, out, progress, _) = run_with(&p, &Mode::DryRun(None), &keys);
    result.unwrap();
    assert!(SignedIndex::split(&out).is_signed());
    assert!(progress.contains("not a key symdev trusts"), "{progress}");
}

#[test]
fn an_upload_does_not_extend_an_unsigned_index() {
    let bucket = FakeBucket::start().with("index.toml", OLD.as_bytes());
    let e = publish_sdk(&bucket, &index_keys()).unwrap();
    assert!(
        e.contains("unsigned") && e.contains("publish sign-index --bucket private"),
        "{e}"
    );
    assert_eq!(bucket.calls(), ["GET index.toml"]);
}

#[test]
fn an_index_signed_by_an_untrusted_key_is_not_extended() {
    let other = IndexSigningKey::from_base64(OTHER_SEED).unwrap();
    let forged = SignedIndex::sign(OLD, &other).to_text();
    let bucket = FakeBucket::start().with("index.toml", forged.as_bytes());
    let e = publish_sdk(&bucket, &index_keys()).unwrap();
    assert!(e.contains("does not verify") && e.contains("leaked"), "{e}");
    assert_eq!(bucket.calls(), ["GET index.toml"]);
}

#[test]
fn a_tampered_index_is_not_extended() {
    let tampered = SignedIndex::sign(OLD, &signing_key()).to_text() + "\n";
    let bucket = FakeBucket::start().with("index.toml", tampered.as_bytes());
    let e = publish_sdk(&bucket, &index_keys()).unwrap();
    assert!(e.contains("tampered"), "{e}");
    assert_eq!(bucket.calls(), ["GET index.toml"]);
}

#[test]
fn a_dry_run_signs_the_index_it_prints_when_the_key_is_set() {
    let sdk = sdk_tree();
    let p = private(sdk.path(), pinned_sdk_recipe(sdk.path()));
    let (result, out, progress, _) = run(&p, &Mode::DryRun(None));
    result.unwrap();
    SignedIndex::split(&out)
        .verify(&signing_key().trusted(), "stdout")
        .unwrap();
    assert!(!progress.contains("PUBLISH_SIGNING_KEY"), "{progress}");
}

#[test]
fn a_dry_run_without_the_key_prints_an_unsigned_index_and_says_so() {
    let sdk = sdk_tree();
    let p = private(sdk.path(), pinned_sdk_recipe(sdk.path()));
    let keys = IndexKeys::new(None, signing_key().trusted());
    let (result, out, progress, _) = run_with(&p, &Mode::DryRun(None), &keys);
    result.unwrap();
    assert!(!SignedIndex::split(&out).is_signed(), "{out}");
    assert!(
        progress.contains("PUBLISH_SIGNING_KEY is not set") && progress.contains("unsigned"),
        "{progress}"
    );
}

#[test]
fn a_dry_run_reads_an_unsigned_index_with_a_warning() {
    let bucket = FakeBucket::start().with("index.toml", OLD.as_bytes());
    let sdk = sdk_tree();
    let p = private(sdk.path(), pinned_sdk_recipe(sdk.path()));
    let mode = Mode::DryRun(Some(Bucket::new("private", &bucket.url, None).unwrap()));
    let (result, out, progress, _) = run(&p, &mode);
    result.unwrap();
    assert!(SignedIndex::split(&out).is_signed());
    assert!(
        progress.contains("warning: ") && progress.contains("index.toml is unsigned"),
        "{progress}"
    );
    assert_eq!(bucket.calls(), ["GET index.toml"]);
}

#[test]
fn the_seed_constant_is_the_tests_signing_key() {
    assert_eq!(
        IndexSigningKey::from_base64(SEED).unwrap().trusted(),
        signing_key().trusted()
    );
}
