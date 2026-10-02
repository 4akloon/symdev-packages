//! What the publisher refuses, and that a refusal leaves the bucket untouched.

use std::fs;

use super::fake_bucket::FakeBucket;
use super::{SDK, gcce_recipe, private, run, sdk_recipe, sdk_tree, tree, upload_to};
use crate::mode::Mode;
use crate::publication::Publication;
use crate::recipe::Recipe;
use crate::visibility::Visibility;

#[test]
fn an_id_already_in_the_index_is_refused_and_the_bucket_is_untouched() {
    let sdk = sdk_tree();
    let p = private(sdk.path(), super::pinned_sdk_recipe(sdk.path()));
    let bucket = FakeBucket::start();
    run(&p, &upload_to(&bucket, "private")).0.unwrap();
    let index = bucket.object("index.toml").unwrap();
    let again = FakeBucket::start().with("index.toml", &index);
    let e = run(&p, &upload_to(&again, "private"))
        .0
        .unwrap_err()
        .to_string();
    assert!(e.contains(SDK) && e.contains("already published"), "{e}");
    assert_eq!(again.calls(), ["GET index.toml"]);
    assert_eq!(again.object("index.toml").unwrap(), index);
}

#[test]
fn a_private_archive_whose_sha256_differs_from_the_recipe_stops_before_any_request() {
    let sdk = sdk_tree();
    let pinned = "f".repeat(64);
    let bucket = FakeBucket::start();
    let p = private(sdk.path(), sdk_recipe(Some(&pinned)));
    let e = run(&p, &upload_to(&bucket, "private"))
        .0
        .unwrap_err()
        .to_string();
    assert!(
        e.contains(&pinned) && e.contains("recipes/sdk/s60-3rd-fp2/1.1/recipe.toml"),
        "{e}"
    );
    assert!(bucket.calls().is_empty(), "{:?}", bucket.calls());
}

#[test]
fn a_private_recipe_without_sha256_prints_the_hash_to_record_and_stops() {
    let sdk = sdk_tree();
    let bucket = FakeBucket::start();
    let (result, _, _, out_dir) = run(
        &private(sdk.path(), sdk_recipe(None)),
        &upload_to(&bucket, "private"),
    );
    let e = result.unwrap_err().to_string();
    let archive = fs::read_dir(out_dir.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .file_name();
    let sha = archive.to_str().unwrap().trim_end_matches(".tar.gz");
    assert!(
        e.contains(&format!("sha256 = \"{sha}\"")) && e.contains("record"),
        "{e}"
    );
    assert!(bucket.calls().is_empty());
}

#[test]
fn a_pattern_that_matches_nothing_stops_before_any_request() {
    let sdk = tree(&[("epoc32/include/e32std.h", "header")]);
    let bucket = FakeBucket::start();
    let e = run(
        &private(sdk.path(), sdk_recipe(None)),
        &upload_to(&bucket, "private"),
    )
    .0
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("`epoc32/release/armv5/lib/*.dso` matches nothing"),
        "{e}"
    );
    assert!(bucket.calls().is_empty());
}

#[test]
fn a_bucket_that_fails_to_answer_the_index_is_not_taken_for_an_empty_one() {
    let sdk = sdk_tree();
    let bucket = FakeBucket::broken();
    let p = private(sdk.path(), super::pinned_sdk_recipe(sdk.path()));
    let e = run(&p, &upload_to(&bucket, "private"))
        .0
        .unwrap_err()
        .to_string();
    assert!(e.contains("index.toml") && e.contains("HTTP 500"), "{e}");
    assert_eq!(bucket.calls(), ["GET index.toml"]);
}

#[test]
fn the_id_argument_must_be_the_recipes() {
    let sdk = sdk_tree();
    let e = Publication::new(
        Visibility::Private,
        "sdk;s60-3rd-fp2;1.2",
        sdk_recipe(None),
        sdk.path(),
        None,
    )
    .err()
    .unwrap()
    .to_string();
    assert!(e.contains("sdk;s60-3rd-fp2;1.2") && e.contains(SDK), "{e}");
}

#[test]
fn a_public_package_needs_its_source_code_and_a_private_one_takes_none() {
    let prefix = tree(&[("bin/g++", "g++")]);
    let e = Publication::new(
        Visibility::Public,
        "gcce;12.1.0",
        gcce_recipe(),
        prefix.path(),
        None,
    )
    .err()
    .unwrap()
    .to_string();
    assert!(e.contains("--source-code"), "{e}");
    let src = prefix.path().join("bin/g++");
    let e = Publication::new(
        Visibility::Private,
        SDK,
        sdk_recipe(None),
        prefix.path(),
        Some(&src),
    )
    .err()
    .unwrap()
    .to_string();
    assert!(e.contains("--source-code"), "{e}");
}

#[test]
fn a_proprietary_licence_is_never_published_to_the_public_bucket() {
    let sdk = sdk_tree();
    let src = sdk.path().join("epoc32/include/e32std.h");
    let e = Publication::new(
        Visibility::Public,
        SDK,
        sdk_recipe(None),
        sdk.path(),
        Some(&src),
    )
    .err()
    .unwrap()
    .to_string();
    assert!(
        e.contains("LicenseRef-Nokia-S60-SDK-EULA") && e.contains("private"),
        "{e}"
    );
}

#[test]
fn a_public_recipe_that_pins_a_sha256_is_checked_too() {
    let prefix = tree(&[("bin/g++", "g++")]);
    let text = format!(
        "id = \"gcce;12.1.0\"\nlicense = \"GPL-3.0-or-later\"\nhost = \"x86_64-linux\"\n\
         sha256 = \"{}\"\n",
        "e".repeat(64)
    );
    let recipe = Recipe::parse(&text, "r.toml").unwrap();
    let src = prefix.path().join("bin/g++");
    let p = Publication::new(
        Visibility::Public,
        "gcce;12.1.0",
        recipe,
        prefix.path(),
        Some(&src),
    )
    .unwrap();
    let e = run(&p, &Mode::DryRun(None)).0.unwrap_err().to_string();
    assert!(e.contains(&"e".repeat(64)), "{e}");
}
