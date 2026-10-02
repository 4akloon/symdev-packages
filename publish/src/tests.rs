//! The publish flow against a fake bucket on 127.0.0.1; nothing here touches the network.

mod fake_bucket;
mod file_upload;
mod refusals;
mod sign_index;
mod signing;
mod two_packages;

use std::fs;
use std::path::Path;

use symdev_sdk::{Host, Index, IndexSigningKey, PackageId, Result, S3Keys, SignedIndex};

use self::fake_bucket::FakeBucket;
use crate::bucket::Bucket;
use crate::index_keys::IndexKeys;
use crate::mode::Mode;
use crate::publication::Publication;
use crate::recipe::Recipe;
use crate::visibility::Visibility;

const SDK: &str = "sdk;s60-3rd-fp2;1.1";
const GCCE: &str = "gcce;12.1.0";
const ARCHIVE_TYPE: (&str, &str) = ("application/gzip", "public, max-age=31536000, immutable");
/// The tests' index signing key: the base64 of the seed of 32 bytes 0x07.
const SEED: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";

fn signing_key() -> IndexSigningKey {
    IndexSigningKey::from_base64(SEED).unwrap()
}

/// What a run with `PUBLISH_SIGNING_KEY` set to [`SEED`] signs and checks with.
fn index_keys() -> IndexKeys {
    IndexKeys::new(Some(signing_key()))
}

fn keys() -> S3Keys {
    S3Keys {
        access_key_id: "AKIDPUBLISH".into(),
        secret_access_key: "publisher-secret".into(),
    }
}

fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, bytes) in files {
        let full = dir.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, bytes).unwrap();
    }
    dir
}

fn sdk_tree() -> tempfile::TempDir {
    tree(&[
        ("epoc32/include/e32std.h", "header"),
        ("epoc32/release/armv5/lib/euser.dso", "dso"),
        ("epoc32/release/armv5/lib/euser.lib", "rvct import library"),
    ])
}

fn sdk_recipe(sha256: Option<&str>) -> Recipe {
    let mut text = format!(
        "id = \"{SDK}\"\nlicense = \"LicenseRef-Nokia-S60-SDK-EULA\"\nhost = \"any\"\n\
         include = [\"epoc32/include\", \"epoc32/release/armv5/lib/*.dso\"]\n"
    );
    if let Some(sha) = sha256 {
        text.push_str(&format!("sha256 = \"{sha}\"\n"));
    }
    Recipe::parse(&text, "recipes/sdk/s60-3rd-fp2/1.1/recipe.toml", SDK).unwrap()
}

/// The SDK recipe with the SHA-256 that `tree` packs to.
fn pinned_sdk_recipe(tree: &Path) -> Recipe {
    let scratch = tempfile::tempdir().unwrap();
    let sha = sdk_recipe(None).pack(tree, scratch.path()).unwrap().sha256;
    sdk_recipe(Some(&sha))
}

fn gcce_recipe() -> Recipe {
    let text = format!(
        "id = \"{GCCE}\"\nlicense = \"GPL-3.0-or-later\"\nhost = \"x86_64-linux\"\n\
         build = \"build.sh\"\n"
    );
    Recipe::parse(&text, "recipes/gcce/12.1.0/recipe.toml", GCCE).unwrap()
}

fn private(tree: &Path, recipe: Recipe) -> Publication {
    Publication::new(Visibility::Private, recipe, tree, None).unwrap()
}

/// Runs `p` with a fresh output directory and the tests' signing key; returns the result,
/// stdout, stderr, and the output directory.
fn run(p: &Publication, mode: &Mode) -> (Result<()>, String, String, tempfile::TempDir) {
    run_with(p, mode, &index_keys())
}

fn run_with(
    p: &Publication,
    mode: &Mode,
    keys: &IndexKeys,
) -> (Result<()>, String, String, tempfile::TempDir) {
    let out_dir = tempfile::tempdir().unwrap();
    let (mut out, mut progress) = (Vec::new(), Vec::new());
    let result = p.run(mode, keys, out_dir.path(), &mut out, &mut progress);
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap();
    (result, text(out), text(progress), out_dir)
}

fn upload_to(bucket: &FakeBucket, name: &str) -> Mode {
    Mode::Upload(Bucket::new(name, &bucket.url, Some(keys())).unwrap())
}

fn uploaded_index(bucket: &FakeBucket) -> Index {
    let bytes = bucket
        .object("index.toml")
        .expect("index.toml was uploaded");
    Index::parse(&String::from_utf8(bytes).unwrap(), "fake").unwrap()
}

#[test]
fn a_private_upload_puts_the_archive_then_the_index() {
    let sdk = sdk_tree();
    let recipe = pinned_sdk_recipe(sdk.path());
    let sha = recipe.sha256().unwrap().to_string();
    let bucket = FakeBucket::start();
    let (result, _, progress, out_dir) =
        run(&private(sdk.path(), recipe), &upload_to(&bucket, "private"));
    result.unwrap();
    let key = format!("sdk/s60-3rd-fp2/1.1/{sha}.tar.gz");
    assert_eq!(
        bucket.calls(),
        [
            "GET index.toml".to_string(),
            format!("PUT {key}"),
            "PUT index.toml".into()
        ]
    );
    let local = fs::read(out_dir.path().join(format!("{sha}.tar.gz"))).unwrap();
    assert_eq!(bucket.object(&key).unwrap(), local);
    let log = bucket.log();
    let header = |n: usize, name: &str| log[n].headers[name].clone();
    assert_eq!(
        (header(1, "content-type"), header(1, "cache-control")),
        (ARCHIVE_TYPE.0.into(), ARCHIVE_TYPE.1.into())
    );
    assert_eq!(
        (header(2, "content-type"), header(2, "cache-control")),
        ("application/toml".into(), "no-cache".into())
    );
    let index = uploaded_index(&bucket);
    let p = index.find(&PackageId::parse(SDK).unwrap()).unwrap();
    assert_eq!(p.license, "LicenseRef-Nokia-S60-SDK-EULA");
    assert_eq!(p.source_code, None);
    let a = &p.archives[0];
    assert_eq!(
        (a.host, a.url.as_str(), a.sha256.as_str()),
        (Host::Any, key.as_str(), sha.as_str())
    );
    assert_eq!(a.size, local.len() as u64);
    assert!(
        progress.contains(&format!("published {SDK} to private")),
        "{progress}"
    );
}

#[test]
fn every_request_is_signed_with_the_publisher_key() {
    let sdk = sdk_tree();
    let bucket = FakeBucket::start();
    let p = private(sdk.path(), pinned_sdk_recipe(sdk.path()));
    run(&p, &upload_to(&bucket, "private")).0.unwrap();
    for request in bucket.log() {
        let auth = &request.headers["authorization"];
        assert!(
            auth.starts_with("AWS4-HMAC-SHA256 Credential=AKIDPUBLISH/"),
            "{auth}"
        );
        assert!(auth.contains("/auto/s3/aws4_request"), "{auth}");
    }
}

#[test]
fn a_public_upload_puts_archive_and_source_code_before_the_index() {
    let prefix = tree(&[
        ("bin/arm-none-symbianelf-g++", "g++"),
        ("lib/libgcc.a", "a"),
    ]);
    let sources = tempfile::tempdir().unwrap();
    let source_code = sources.path().join("gcce-12.1.0-src.tar.gz");
    fs::write(&source_code, b"corresponding source").unwrap();
    let source_sha = "a58166d1b1149d4131e322f51de4a6722d4f04dc0735f1605c2b9665e7d0bf91";
    let p = Publication::new(
        Visibility::Public,
        gcce_recipe(),
        prefix.path(),
        Some(&source_code),
    )
    .unwrap();
    let bucket = FakeBucket::start();
    let (result, _, _, _) = run(&p, &upload_to(&bucket, "public"));
    result.unwrap();
    let index = uploaded_index(&bucket);
    let package = index.find(&PackageId::parse(GCCE).unwrap()).unwrap();
    let archive = &package.archives[0];
    let source_key = format!("src/gcce/12.1.0/{source_sha}.tar.gz");
    assert_eq!(archive.host, Host::X86_64Linux);
    assert_eq!(package.source_code.as_deref(), Some(source_key.as_str()));
    assert_eq!(
        bucket.calls(),
        [
            "GET index.toml".to_string(),
            format!("PUT {}", archive.url),
            format!("PUT {source_key}"),
            "PUT index.toml".into()
        ]
    );
    assert_eq!(bucket.object(&source_key).unwrap(), b"corresponding source");
    let log = bucket.log();
    assert_eq!(log[2].headers["content-type"], ARCHIVE_TYPE.0);
    assert_eq!(log[2].headers["cache-control"], ARCHIVE_TYPE.1);
    assert_eq!(log[2].headers["x-amz-content-sha256"], source_sha);
}

#[test]
fn packages_already_in_the_index_are_kept() {
    let sdk = sdk_tree();
    let old = "schema = 1\n\n[[package]]\nid = \"gcce;12.1.0\"\nlicense = \"GPL-3.0-or-later\"\n\
               depends = []\n\n[[package.archive]]\nhost = \"x86_64-linux\"\n\
               url = \"gcce/12.1.0/a.tar.gz\"\nsha256 = \""
        .to_string()
        + &"a".repeat(64)
        + "\"\nsize = 1\n";
    let old = SignedIndex::sign(&old, &signing_key()).to_text();
    let bucket = FakeBucket::start().with("index.toml", old.as_bytes());
    run(
        &private(sdk.path(), pinned_sdk_recipe(sdk.path())),
        &upload_to(&bucket, "private"),
    )
    .0
    .unwrap();
    let ids: Vec<String> = uploaded_index(&bucket)
        .packages
        .iter()
        .map(|p| p.id.to_string())
        .collect();
    assert_eq!(ids, [GCCE, SDK]);
}

#[test]
fn a_dry_run_reads_the_index_and_uploads_nothing() {
    let sdk = sdk_tree();
    let bucket = FakeBucket::start();
    let mode = Mode::DryRun(Some(Bucket::new("private", &bucket.url, None).unwrap()));
    let (result, out, progress, _) =
        run(&private(sdk.path(), pinned_sdk_recipe(sdk.path())), &mode);
    result.unwrap();
    assert_eq!(bucket.calls(), ["GET index.toml"]);
    assert!(!bucket.log()[0].headers.contains_key("authorization"));
    let index = Index::parse(&out, "stdout").unwrap();
    assert!(index.find(&PackageId::parse(SDK).unwrap()).is_some());
    assert!(progress.contains("dry run"), "{progress}");
}

#[test]
fn a_dry_run_without_a_bucket_starts_from_an_empty_index() {
    let sdk = sdk_tree();
    let (result, out, progress, out_dir) = run(
        &private(sdk.path(), pinned_sdk_recipe(sdk.path())),
        &Mode::DryRun(None),
    );
    result.unwrap();
    let index = Index::parse(&out, "stdout").unwrap();
    assert_eq!(index.packages.len(), 1);
    let sha = &index.packages[0].archives[0].sha256;
    assert!(
        out_dir.path().join(format!("{sha}.tar.gz")).is_file(),
        "the archive stays for inspection"
    );
    assert!(progress.contains("empty index"), "{progress}");
}
