//! `publish file`: one local file put at a fixed key, with the headers given; no index.

use std::fs;
use std::path::Path;

use symdev_sdk::Result;

use super::fake_bucket::FakeBucket;
use super::upload_to;
use crate::bucket::Bucket;
use crate::file_upload::FileUpload;
use crate::mode::Mode;
use crate::object_key::ObjectKey;

const SCRIPT: &[u8] = b"#!/bin/sh\necho install\n";
const TEXT: &str = "text/plain; charset=utf-8";

fn script() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("install.sh"), SCRIPT).unwrap();
    dir
}

fn upload(file: &Path, key: &str) -> Result<FileUpload> {
    FileUpload::new(file, ObjectKey::parse(key).unwrap(), TEXT, "no-cache")
}

fn run(upload: &FileUpload, mode: &Mode) -> (Result<()>, String) {
    let mut progress = Vec::new();
    let result = upload.run(mode, &mut progress);
    (result, String::from_utf8(progress).unwrap())
}

#[test]
fn an_upload_puts_the_file_at_its_key_with_its_headers_and_nothing_else() {
    let dir = script();
    let old_index = b"schema = 1\n";
    let bucket = FakeBucket::start().with("index.toml", old_index);
    let file = upload(&dir.path().join("install.sh"), "install.sh").unwrap();
    let (result, progress) = run(&file, &upload_to(&bucket, "public"));
    result.unwrap();
    assert_eq!(bucket.calls(), ["PUT install.sh"]);
    assert_eq!(bucket.object("install.sh").unwrap(), SCRIPT);
    assert_eq!(bucket.object("index.toml").unwrap(), old_index);
    let put = &bucket.log()[0];
    assert_eq!(put.headers["content-type"], TEXT);
    assert_eq!(put.headers["cache-control"], "no-cache");
    let auth = &put.headers["authorization"];
    assert!(
        auth.starts_with("AWS4-HMAC-SHA256 Credential=AKIDPUBLISH/"),
        "{auth}"
    );
    assert!(
        progress.contains(&format!("{}install.sh", bucket.url)),
        "{progress}"
    );
}

#[test]
fn the_signed_hash_is_the_files_sha256() {
    let dir = script();
    let bucket = FakeBucket::start();
    let file = upload(&dir.path().join("install.sh"), "install.sh").unwrap();
    run(&file, &upload_to(&bucket, "public")).0.unwrap();
    let local = crate::archive::Archive::hash(&dir.path().join("install.sh")).unwrap();
    assert_eq!(
        bucket.log()[0].headers["x-amz-content-sha256"],
        local.sha256
    );
}

#[test]
fn a_dry_run_names_the_url_and_headers_and_sends_nothing() {
    let dir = script();
    let bucket = FakeBucket::start();
    let mode = Mode::DryRun(Some(Bucket::new("public", &bucket.url, None).unwrap()));
    let file = upload(&dir.path().join("install.sh"), "install.sh").unwrap();
    let (result, progress) = run(&file, &mode);
    result.unwrap();
    assert!(bucket.calls().is_empty(), "{:?}", bucket.calls());
    for part in [
        format!("{}install.sh", bucket.url),
        TEXT.to_string(),
        "no-cache".into(),
        format!("{} bytes", SCRIPT.len()),
        "nothing uploaded".into(),
    ] {
        assert!(progress.contains(&part), "{part}: {progress}");
    }
}

#[test]
fn a_dry_run_without_a_bucket_url_names_the_key() {
    let dir = script();
    let file = upload(&dir.path().join("install.sh"), "docs/install.sh").unwrap();
    let (result, progress) = run(&file, &Mode::DryRun(None));
    result.unwrap();
    assert!(progress.contains("docs/install.sh"), "{progress}");
    assert!(progress.contains("nothing uploaded"), "{progress}");
}

#[test]
fn a_header_value_with_a_control_character_is_refused() {
    let dir = script();
    let path = dir.path().join("install.sh");
    let key = || ObjectKey::parse("install.sh").unwrap();
    let e = FileUpload::new(&path, key(), "text/plain\r\nx-amz-acl: public", "no-cache")
        .err()
        .unwrap()
        .to_string();
    assert!(e.contains("--content-type"), "{e}");
    let e = FileUpload::new(&path, key(), TEXT, "")
        .err()
        .unwrap()
        .to_string();
    assert!(e.contains("--cache-control") && e.contains("empty"), "{e}");
}

#[test]
fn a_missing_file_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("install.sh");
    let e = upload(&missing, "install.sh").err().unwrap().to_string();
    assert!(e.contains(&missing.display().to_string()), "{e}");
}
