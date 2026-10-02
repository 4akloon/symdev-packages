//! A recipe that makes several packages from one build (`[[package]]` tables), and the
//! git source a recipe may name.

use std::fs;

use symdev_sdk::Host;

use super::super::Recipe;
use super::{files, unpack};

const PATH: &str = "recipes/symdev/0.1.0/recipe.toml";

/// The shape of `recipes/symdev/0.1.0/recipe.toml`: one tag, two packages.
const TWO: &str = "git = \"https://github.com/4akloon/symdev\"\ntag = \"v0.1.0\"\n\
                   build = \"build.sh\"\n\n\
                   [[package]]\nid = \"symdev;0.1.0\"\nlicense = \"MIT\"\nhost = \"x86_64-linux\"\n\n\
                   [[package]]\nid = \"rust-sdk;0.1.0\"\nlicense = \"MIT\"\nhost = \"any\"\n\
                   include = [\"targets\", \"crates\"]\n";

fn error(text: &str, id: &str) -> String {
    match Recipe::parse(text, PATH, id) {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn each_package_of_a_recipe_that_makes_two_is_taken_by_its_id() {
    let symdev = Recipe::parse(TWO, PATH, "symdev;0.1.0").unwrap();
    assert_eq!(symdev.id().as_str(), "symdev;0.1.0");
    assert_eq!(
        (symdev.license(), symdev.host()),
        ("MIT", Host::X86_64Linux)
    );
    let sdk = Recipe::parse(TWO, PATH, "rust-sdk;0.1.0").unwrap();
    assert_eq!(sdk.id().as_str(), "rust-sdk;0.1.0");
    assert_eq!((sdk.license(), sdk.host()), ("MIT", Host::Any));
    assert_eq!(sdk.path(), PATH);
}

#[test]
fn a_package_packs_with_its_own_include_list_only() {
    let tree = tempfile::tempdir().unwrap();
    for path in [
        "targets/t.json",
        "crates/a/Cargo.toml",
        "examples/e/main.rs",
    ] {
        let full = tree.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, path).unwrap();
    }
    let out = tempfile::tempdir().unwrap();
    let sdk = Recipe::parse(TWO, PATH, "rust-sdk;0.1.0").unwrap();
    let (_keep, unpacked) = unpack(&sdk.pack(tree.path(), out.path()).unwrap().path);
    assert_eq!(files(&unpacked), ["crates/a/Cargo.toml", "targets/t.json"]);
    let symdev = Recipe::parse(TWO, PATH, "symdev;0.1.0").unwrap();
    let (_keep, unpacked) = unpack(&symdev.pack(tree.path(), out.path()).unwrap().path);
    assert_eq!(
        files(&unpacked),
        files(tree.path()),
        "no include list: all of --from"
    );
}

#[test]
fn an_id_the_recipe_does_not_make_is_refused_naming_the_ids_it_makes() {
    let e = error(TWO, "symdev;0.2.0");
    assert!(e.contains("`symdev;0.2.0`") && e.contains(PATH), "{e}");
    assert!(e.contains("`symdev;0.1.0`, `rust-sdk;0.1.0`"), "{e}");
}

#[test]
fn a_recipe_with_package_tables_has_no_package_key_at_its_top() {
    let e = error(&format!("license = \"MIT\"\n{TWO}"), "symdev;0.1.0");
    assert!(e.contains(PATH) && e.contains("license"), "{e}");
}

#[test]
fn two_packages_with_one_id_are_refused() {
    let e = error(
        &TWO.replace("rust-sdk;0.1.0", "symdev;0.1.0"),
        "symdev;0.1.0",
    );
    assert!(
        e.contains(PATH) && e.contains("`symdev;0.1.0`") && e.contains("twice"),
        "{e}"
    );
}

#[test]
fn a_recipe_that_makes_no_package_is_refused() {
    let text = "git = \"https://github.com/4akloon/symdev\"\ntag = \"v0.1.0\"\npackage = []\n";
    let e = error(text, "symdev;0.1.0");
    assert!(e.contains(PATH) && e.contains("no package"), "{e}");
}

#[test]
fn every_package_is_checked_not_only_the_one_asked_for() {
    let e = error(&TWO.replace("\"crates\"", "\"../crates\""), "symdev;0.1.0");
    assert!(e.contains("`../crates`"), "{e}");
}

#[test]
fn a_one_package_recipe_may_name_its_git_source() {
    let text = "id = \"symdev;0.1.0\"\nlicense = \"MIT\"\nhost = \"x86_64-linux\"\n\
                git = \"https://github.com/4akloon/symdev\"\ntag = \"v0.1.0\"\n";
    assert_eq!(
        Recipe::parse(text, PATH, "symdev;0.1.0").unwrap().host(),
        Host::X86_64Linux
    );
}

/// The real `recipes/symdev/0.1.0/recipe.toml` against a tree shaped like the tag's
/// checkout: the package keeps the repository layout symdev expects (`<package>/symbian-rs`
/// is the Rust SDK; symbian-macros reaches `crates/symdev-locale` through `../../../`), and
/// leaves out the corpus, build output and the host crates the SDK does not use.
#[test]
fn the_rust_sdk_keeps_the_repository_layout_without_corpus_or_build_output() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../recipes/symdev/0.1.0/recipe.toml"
    );
    let recipe = Recipe::parse(&fs::read_to_string(path).unwrap(), path, "rust-sdk;0.1.0");
    let tree = tempfile::tempdir().unwrap();
    for file in [
        "LICENSE",
        "Cargo.toml",
        "Cargo.lock",
        "crates/symdev-locale/Cargo.toml",
        "crates/symdev-cli/Cargo.toml",
        "symbian-rs/.cargo/config.toml",
        "symbian-rs/Cargo.toml",
        "symbian-rs/Cargo.lock",
        "symbian-rs/rust-toolchain.toml",
        "symbian-rs/targets/arm-symbian-e32.json",
        "symbian-rs/crates/symbian-macros/Cargo.toml",
        "symbian-rs/rust-src/overlay.toml",
        "symbian-rs/shims/common/symrs_shim.h",
        "symbian-rs/examples/async/Cargo.toml",
        "symbian-rs/corpus/65-hello/hello.exe",
        "symbian-rs/target/release/libhello.a",
        "target/release/symdev",
    ] {
        let full = tree.path().join(file);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, file).unwrap();
    }
    let out = tempfile::tempdir().unwrap();
    let archive = recipe.unwrap().pack(tree.path(), out.path()).unwrap();
    let (_keep, unpacked) = unpack(&archive.path);
    assert_eq!(
        files(&unpacked),
        [
            "Cargo.toml",
            "LICENSE",
            "crates/symdev-locale/Cargo.toml",
            "symbian-rs/.cargo/config.toml",
            "symbian-rs/Cargo.lock",
            "symbian-rs/Cargo.toml",
            "symbian-rs/crates/symbian-macros/Cargo.toml",
            "symbian-rs/examples/async/Cargo.toml",
            "symbian-rs/rust-src/overlay.toml",
            "symbian-rs/rust-toolchain.toml",
            "symbian-rs/shims/common/symrs_shim.h",
            "symbian-rs/targets/arm-symbian-e32.json",
        ]
    );
}
