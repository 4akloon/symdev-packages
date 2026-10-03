//! `recipes/symdev/0.3.0`: the rust-sdk package also ships `symbian-rs/prebuilt`, which
//! prebuilt.sh writes beside the tag's tree after build.sh (symdev experiment 109).

use std::fs;

use super::super::Recipe;
use super::{files, unpack};

const ID: &str = "rust-sdk;0.3.0";

fn recipe() -> Recipe {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../recipes/symdev/0.3.0/recipe.toml"
    );
    Recipe::parse(&fs::read_to_string(path).unwrap(), path, ID).unwrap()
}

/// A tree shaped like build.sh's `rust-sdk/` (the tag's checkout), with `prebuilt` as
/// prebuilt.sh leaves it when `with_prebuilt`.
fn tree(with_prebuilt: bool) -> tempfile::TempDir {
    let tree = tempfile::tempdir().unwrap();
    let mut list = vec![
        "LICENSE",
        "Cargo.toml",
        "crates/symdev-locale/Cargo.toml",
        "crates/symdev-cli/Cargo.toml",
        "symbian-rs/.cargo/config.toml",
        "symbian-rs/Cargo.toml",
        "symbian-rs/Cargo.lock",
        "symbian-rs/rust-toolchain.toml",
        "symbian-rs/targets/arm-symbian-e32.json",
        "symbian-rs/crates/symbian-macros/Cargo.toml",
        "symbian-rs/rust-src/overlay.toml",
        "symbian-rs/shims/s60/symrs_avkon.cpp",
        "symbian-rs/examples/async/Cargo.toml",
        "symbian-rs/corpus/65-hello/hello.exe",
    ];
    if with_prebuilt {
        list.extend([
            "symbian-rs/prebuilt/lib/libsymrs.a",
            "symbian-rs/prebuilt/lib/libsymrs_ui.a",
            "symbian-rs/prebuilt/lib/libsupc++.a",
            "symbian-rs/prebuilt/lib/libgcc.a",
            "symbian-rs/prebuilt/NOTICE",
            "symbian-rs/prebuilt/COPYING3",
            "symbian-rs/prebuilt/COPYING.RUNTIME",
        ]);
    }
    for file in list {
        let full = tree.path().join(file);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, file).unwrap();
    }
    tree
}

/// The GCC runtime members in prebuilt/lib are GPL-3.0-or-later WITH GCC-exception-3.1;
/// the index says so beside the MIT of the rest.
#[test]
fn the_rust_sdk_with_prebuilt_members_names_both_licences() {
    assert_eq!(
        recipe().license(),
        "MIT AND GPL-3.0-or-later WITH GCC-exception-3.1"
    );
}

#[test]
fn the_rust_sdk_ships_prebuilt_beside_the_tags_tree() {
    let tree = tree(true);
    let out = tempfile::tempdir().unwrap();
    let archive = recipe().pack(tree.path(), out.path()).unwrap();
    let (_keep, unpacked) = unpack(&archive.path);
    let prebuilt: Vec<String> = files(&unpacked)
        .into_iter()
        .filter(|f| f.starts_with("symbian-rs/prebuilt/"))
        .collect();
    assert_eq!(
        prebuilt,
        [
            "symbian-rs/prebuilt/COPYING.RUNTIME",
            "symbian-rs/prebuilt/COPYING3",
            "symbian-rs/prebuilt/NOTICE",
            "symbian-rs/prebuilt/lib/libgcc.a",
            "symbian-rs/prebuilt/lib/libsupc++.a",
            "symbian-rs/prebuilt/lib/libsymrs.a",
            "symbian-rs/prebuilt/lib/libsymrs_ui.a",
        ]
    );
    assert!(!unpacked.join("symbian-rs/corpus").exists());
}

/// prebuilt.sh did not run: the package is not made without the prebuilt set.
#[test]
fn the_rust_sdk_is_refused_without_prebuilt() {
    let tree = tree(false);
    let out = tempfile::tempdir().unwrap();
    let e = recipe()
        .pack(tree.path(), out.path())
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("symbian-rs/prebuilt") && e.contains("matches nothing"),
        "{e}"
    );
}
