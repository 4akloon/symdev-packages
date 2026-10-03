//! `Recipe` parsing and packing, on trees built in temporary directories.

use std::fs;
use std::path::{Path, PathBuf};

use symdev_sdk::{Host, TarGz};

use super::Recipe;

mod packages;
mod prebuilt;

const SDK: &str = "sdk;s60-3rd-fp2;1.1";
const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn sdk_recipe(include: &str) -> String {
    format!(
        "id = \"sdk;s60-3rd-fp2;1.1\"\nlicense = \"LicenseRef-Nokia-S60-SDK-EULA\"\n\
         host = \"any\"\ninclude = [{include}]\n"
    )
}

/// A small tree shaped like the SDK: what the recipe takes and what it leaves out.
fn sdk_tree() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, bytes) in [
        ("epoc32/include/e32std.h", "e32std"),
        ("epoc32/include/gcce/gcce.h", "gcce"),
        ("epoc32/release/armv5/lib/euser.dso", "dso"),
        ("epoc32/release/armv5/lib/euser.lib", "rvct lib"),
        ("epoc32/release/armv5/lib/avkon.dso", "dso 2"),
        ("epoc32/release/armv5/urel/eexe.lib", "eexe"),
        ("epoc32/release/armv5/urel/euser.dll", "dll"),
        ("epoc32/tools/variant/variant.cfg", "cfg"),
        ("epoc32/tools/makesis.exe", "exe"),
    ] {
        let full = dir.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, bytes).unwrap();
    }
    dir
}

/// Every file in `root`, relative, sorted.
fn files(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path.strip_prefix(root).unwrap();
                out.push(rel.to_str().unwrap().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn unpack(archive: &Path) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let into = dir.path().join("x");
    fs::create_dir(&into).unwrap();
    TarGz::new(archive, "test").extract(&into).unwrap();
    (dir, into)
}

#[test]
fn parses_the_sdk_recipe() {
    let text = format!("{}sha256 = \"{SHA}\"\n", sdk_recipe("\"epoc32/include\""));
    let r = Recipe::parse(&text, "r.toml", SDK).unwrap();
    assert_eq!(r.id().as_str(), SDK);
    assert_eq!(r.license(), "LicenseRef-Nokia-S60-SDK-EULA");
    assert_eq!(r.host(), Host::Any);
    assert_eq!(r.sha256(), Some(SHA));
}

#[test]
fn parses_the_gcce_recipe_with_its_build_script_and_sources() {
    let text = "id = \"gcce;12.1.0\"\nlicense = \"GPL-3.0-or-later\"\nhost = \"x86_64-linux\"\n\
                build = \"build.sh\"\n\n[[source]]\nurl = \"https://ftp.gnu.org/gnu/gcc/a.tar.xz\"\n\
                sha256 = \"62fd634889f31c02b64af2c468f064b47ad1ca78411c45abe6ac4b5f8dd19c7b\"\n";
    let r = Recipe::parse(text, "r.toml", "gcce;12.1.0").unwrap();
    assert_eq!(r.host(), Host::X86_64Linux);
    assert_eq!(r.sha256(), None);
}

#[test]
fn an_id_the_recipe_does_not_make_is_refused_naming_the_one_it_makes() {
    let text = sdk_recipe("\"epoc32/include\"");
    let e = Recipe::parse(&text, "recipes/sdk/r.toml", "sdk;s60-3rd-fp2;1.2")
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("`sdk;s60-3rd-fp2;1.2`") && e.contains(SDK) && e.contains("recipes/sdk/r.toml"),
        "{e}"
    );
}

#[test]
fn an_unknown_key_is_refused_naming_it_and_the_recipe() {
    let text = sdk_recipe("\"epoc32/include\"").replace("include", "inlcude");
    let e = Recipe::parse(&text, "recipes/sdk/r.toml", SDK)
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("recipes/sdk/r.toml") && e.contains("inlcude"),
        "{e}"
    );
}

#[test]
fn a_sha256_that_is_not_64_lowercase_hex_is_refused() {
    for bad in ["abc", &SHA.to_uppercase()] {
        let text = format!("{}sha256 = \"{bad}\"\n", sdk_recipe("\"epoc32/include\""));
        let e = Recipe::parse(&text, "r.toml", SDK).unwrap_err().to_string();
        assert!(e.contains("sha256") && e.contains(bad), "{e}");
    }
}

#[test]
fn an_include_pattern_must_stay_relative_with_a_star_only_in_its_last_segment() {
    for bad in [
        "epoc32/*/lib",
        "../epoc32",
        "/epoc32",
        "",
        "epoc32//include",
        "a\\b",
    ] {
        let text = sdk_recipe(&format!("{bad:?}"));
        let e = Recipe::parse(&text, "r.toml", SDK).unwrap_err().to_string();
        assert!(e.contains(&format!("`{bad}`")), "{bad}: {e}");
    }
}

#[test]
fn packs_only_what_the_include_list_names() {
    let tree = sdk_tree();
    let text = sdk_recipe(
        "\"epoc32/include\", \"epoc32/release/armv5/lib/*.dso\", \
         \"epoc32/release/armv5/urel/eexe.lib\", \"epoc32/tools/variant/variant.cfg\"",
    );
    let out = tempfile::tempdir().unwrap();
    let archive = Recipe::parse(&text, "r.toml", SDK)
        .unwrap()
        .pack(tree.path(), out.path())
        .unwrap();
    assert_eq!(
        archive.path,
        out.path().join(format!("{}.tar.gz", archive.sha256))
    );
    assert_eq!(fs::metadata(&archive.path).unwrap().len(), archive.size);
    let (_keep, unpacked) = unpack(&archive.path);
    assert_eq!(
        files(&unpacked),
        [
            "epoc32/include/e32std.h",
            "epoc32/include/gcce/gcce.h",
            "epoc32/release/armv5/lib/avkon.dso",
            "epoc32/release/armv5/lib/euser.dso",
            "epoc32/release/armv5/urel/eexe.lib",
            "epoc32/tools/variant/variant.cfg",
        ]
    );
    let bytes = fs::read(unpacked.join("epoc32/release/armv5/lib/euser.dso")).unwrap();
    assert_eq!(bytes, b"dso");
    assert_eq!(files(out.path()), [format!("{}.tar.gz", archive.sha256)]);
}

#[test]
fn packing_the_same_tree_twice_gives_the_same_sha256() {
    let tree = sdk_tree();
    let recipe =
        Recipe::parse(&sdk_recipe("\"epoc32/release/armv5/lib/*.dso\""), "r", SDK).unwrap();
    let a = recipe
        .pack(tree.path(), tempfile::tempdir().unwrap().path())
        .unwrap();
    let b = recipe
        .pack(tree.path(), tempfile::tempdir().unwrap().path())
        .unwrap();
    assert_eq!((a.sha256, a.size), (b.sha256, b.size));
}

#[test]
fn a_pattern_that_matches_nothing_is_an_error_naming_it() {
    let tree = sdk_tree();
    for pattern in [
        "epoc32/release/armv5/lib/*.DSO",
        "epoc32/release/armv5/lib/usrt2_2.lib",
        "epoc32/release/gcce/*.dso",
    ] {
        let recipe = Recipe::parse(&sdk_recipe(&format!("{pattern:?}")), "r", SDK).unwrap();
        let out = tempfile::tempdir().unwrap();
        let e = recipe
            .pack(tree.path(), out.path())
            .unwrap_err()
            .to_string();
        assert!(
            e.contains(&format!("`{pattern}`")) && e.contains("matches nothing"),
            "{e}"
        );
        assert!(
            files(out.path()).is_empty(),
            "{pattern}: nothing is written"
        );
    }
}

#[test]
fn without_an_include_list_the_whole_tree_is_packed() {
    let tree = sdk_tree();
    let text = "id = \"gcce;12.1.0\"\nlicense = \"GPL-3.0-or-later\"\nhost = \"x86_64-linux\"\n";
    let out = tempfile::tempdir().unwrap();
    let archive = Recipe::parse(text, "r", "gcce;12.1.0")
        .unwrap()
        .pack(tree.path(), out.path())
        .unwrap();
    let (_keep, unpacked) = unpack(&archive.path);
    assert_eq!(files(&unpacked), files(tree.path()));
}
