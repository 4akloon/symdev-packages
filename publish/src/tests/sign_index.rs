//! `publish sign-index`: the existing index, signed as it is.

use symdev_sdk::{IndexSigningKey, SignedIndex};

use super::fake_bucket::FakeBucket;
use super::{index_keys, signing_key, upload_to};
use crate::bucket::Bucket;
use crate::index_keys::IndexKeys;
use crate::mode::Mode;

/// An index as an older publisher wrote it, with a blank line and comment it would not
/// write today: sign-index must keep every byte.
const UNSIGNED: &str = "schema = 1\n\n# kept as it is\n[[package]]\nid = \"gcce;12.1.0\"\n\
                        license = \"GPL-3.0-or-later\"\ndepends = []\n\n[[package.archive]]\n\
                        host = \"x86_64-linux\"\nurl = \"gcce/12.1.0/a.tar.gz\"\n\
                        sha256 = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\n\
                        size = 67463658\n";

/// Runs sign-index; returns the result as text, stdout and stderr.
fn resign(keys: &IndexKeys, mode: &Mode) -> (Result<(), String>, String, String) {
    let (mut out, mut progress) = (Vec::new(), Vec::new());
    let result = keys.resign(mode, &mut out, &mut progress);
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap();
    (result.map_err(|e| e.to_string()), text(out), text(progress))
}

#[test]
fn an_unsigned_index_is_signed_byte_for_byte_and_its_packages_listed() {
    let bucket = FakeBucket::start().with("index.toml", UNSIGNED.as_bytes());
    let (result, _, progress) = resign(&index_keys(), &upload_to(&bucket, "public"));
    result.unwrap();
    assert_eq!(bucket.calls(), ["GET index.toml", "PUT index.toml"]);
    let stored = String::from_utf8(bucket.object("index.toml").unwrap()).unwrap();
    assert_eq!(
        stored,
        SignedIndex::sign(UNSIGNED, &signing_key()).to_text()
    );
    let put = &bucket.log()[1];
    assert_eq!(put.headers["cache-control"], "no-cache");
    assert_eq!(put.headers["content-type"], "application/toml");
    assert!(progress.contains("UNSIGNED"), "{progress}");
    let line = format!("gcce;12.1.0 x86_64-linux {} 67463658", "a".repeat(64));
    assert!(progress.contains(&line), "{progress}");
}

#[test]
fn an_index_already_signed_with_the_key_is_not_uploaded_again() {
    let signed = SignedIndex::sign(UNSIGNED, &signing_key()).to_text();
    let bucket = FakeBucket::start().with("index.toml", signed.as_bytes());
    let (result, _, progress) = resign(&index_keys(), &upload_to(&bucket, "public"));
    result.unwrap();
    assert_eq!(bucket.calls(), ["GET index.toml"]);
    assert!(progress.contains("already signed"), "{progress}");
}

#[test]
fn a_tampered_index_is_not_re_signed() {
    let tampered = SignedIndex::sign(UNSIGNED, &signing_key())
        .to_text()
        .replace("67463658", "67463659");
    let bucket = FakeBucket::start().with("index.toml", tampered.as_bytes());
    let (result, _, _) = resign(&index_keys(), &upload_to(&bucket, "public"));
    let e = result.unwrap_err();
    assert!(e.contains("tampered"), "{e}");
    assert_eq!(bucket.calls(), ["GET index.toml"]);
}

#[test]
fn an_index_signed_by_another_key_is_not_re_signed() {
    let other =
        IndexSigningKey::from_base64("CAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAg=").unwrap();
    let forged = SignedIndex::sign(UNSIGNED, &other).to_text();
    let bucket = FakeBucket::start().with("index.toml", forged.as_bytes());
    let e = resign(&index_keys(), &upload_to(&bucket, "public"))
        .0
        .unwrap_err();
    assert!(e.contains("does not verify"), "{e}");
    assert_eq!(bucket.calls(), ["GET index.toml"]);
}

#[test]
fn an_index_that_does_not_parse_is_not_signed() {
    let broken = "schema = 1\n[[package]]\nid = \"gcce;12.1.0\"\n";
    let bucket = FakeBucket::start().with("index.toml", broken.as_bytes());
    let e = resign(&index_keys(), &upload_to(&bucket, "public"))
        .0
        .unwrap_err();
    assert!(e.contains("bad index"), "{e}");
    assert_eq!(bucket.calls(), ["GET index.toml"]);
}

#[test]
fn a_bucket_without_an_index_has_nothing_to_sign() {
    let bucket = FakeBucket::start();
    let e = resign(&index_keys(), &upload_to(&bucket, "public"))
        .0
        .unwrap_err();
    assert!(
        e.contains("index.toml") && e.contains("nothing to sign"),
        "{e}"
    );
    assert_eq!(bucket.calls(), ["GET index.toml"]);
}

#[test]
fn an_upload_without_the_key_sends_nothing() {
    let bucket = FakeBucket::start().with("index.toml", UNSIGNED.as_bytes());
    let e = resign(&IndexKeys::new(None), &upload_to(&bucket, "public"))
        .0
        .unwrap_err();
    assert!(e.contains("PUBLISH_SIGNING_KEY"), "{e}");
    assert!(bucket.calls().is_empty());
}

#[test]
fn a_dry_run_prints_the_signed_index_and_uploads_nothing() {
    let bucket = FakeBucket::start().with("index.toml", UNSIGNED.as_bytes());
    let mode = Mode::DryRun(Some(Bucket::new("public", &bucket.url, None).unwrap()));
    let (result, out, progress) = resign(&index_keys(), &mode);
    result.unwrap();
    assert_eq!(out, SignedIndex::sign(UNSIGNED, &signing_key()).to_text());
    assert_eq!(bucket.calls(), ["GET index.toml"]);
    assert!(progress.contains("nothing uploaded"), "{progress}");
}

#[test]
fn a_dry_run_needs_the_bucket_url() {
    let e = resign(&index_keys(), &Mode::DryRun(None)).0.unwrap_err();
    assert!(
        e.contains("PUBLISH_PUBLIC_URL") && e.contains("PUBLISH_PRIVATE_URL"),
        "{e}"
    );
}
