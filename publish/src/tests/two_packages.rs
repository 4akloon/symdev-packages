//! A release that makes two packages from one recipe (`symdev` and `rust-sdk` from one
//! tag), published one after the other into one bucket.

use std::fs;

use symdev_sdk::{Host, PackageId, TarGz};

use super::fake_bucket::FakeBucket;
use super::{run, tree, upload_to, uploaded_index};
use crate::publication::Publication;
use crate::recipe::Recipe;
use crate::visibility::Visibility;

const PATH: &str = "recipes/symdev/0.1.0/recipe.toml";
const RECIPE: &str = "git = \"https://github.com/4akloon/symdev\"\ntag = \"v0.1.0\"\n\
                      build = \"build.sh\"\n\n\
                      [[package]]\nid = \"symdev;0.1.0\"\nlicense = \"MIT\"\n\
                      host = \"x86_64-linux\"\n\n\
                      [[package]]\nid = \"rust-sdk;0.1.0\"\nlicense = \"MIT\"\nhost = \"any\"\n\
                      include = [\"targets\", \"crates\"]\n";

#[test]
fn both_packages_of_one_recipe_end_up_in_one_index_with_their_source_code() {
    let out = tree(&[
        ("symdev/bin/symdev", "static binary"),
        ("rust-sdk/targets/arm-symbian-e32.json", "{}"),
        ("rust-sdk/crates/symbian-std/Cargo.toml", "[package]"),
        ("rust-sdk/corpus/65-hello/hello.exe", "left out"),
    ]);
    let source_code = out.path().join("symdev-0.1.0-source.tar.gz");
    fs::write(&source_code, b"git archive v0.1.0").unwrap();
    let bucket = FakeBucket::start();
    // rust-sdk first: the index never shows a symdev whose Rust SDK is missing.
    for (id, from) in [("rust-sdk;0.1.0", "rust-sdk"), ("symdev;0.1.0", "symdev")] {
        let recipe = Recipe::parse(RECIPE, PATH, id).unwrap();
        let from = out.path().join(from);
        let p = Publication::new(Visibility::Public, recipe, &from, Some(&source_code)).unwrap();
        run(&p, &upload_to(&bucket, "public")).0.unwrap();
    }
    let index = uploaded_index(&bucket);
    let ids: Vec<&str> = index.packages.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["rust-sdk;0.1.0", "symdev;0.1.0"]);
    for (id, host, prefix) in [
        ("rust-sdk;0.1.0", Host::Any, "rust-sdk/0.1.0/"),
        ("symdev;0.1.0", Host::X86_64Linux, "symdev/0.1.0/"),
    ] {
        let package = index.find(&PackageId::parse(id).unwrap()).unwrap();
        assert_eq!(package.license, "MIT");
        let archive = &package.archives[0];
        assert_eq!(archive.host, host);
        assert!(archive.url.starts_with(prefix), "{}", archive.url);
        let source = package.source_code.as_deref().unwrap();
        assert!(source.starts_with(&format!("src/{prefix}")), "{source}");
        assert_eq!(bucket.object(source).unwrap(), b"git archive v0.1.0");
    }
    let sdk = index
        .find(&PackageId::parse("rust-sdk;0.1.0").unwrap())
        .unwrap();
    let packed = out.path().join("rust-sdk.tar.gz");
    fs::write(&packed, bucket.object(&sdk.archives[0].url).unwrap()).unwrap();
    let unpacked = out.path().join("unpacked");
    fs::create_dir(&unpacked).unwrap();
    TarGz::new(&packed, "test").extract(&unpacked).unwrap();
    assert!(unpacked.join("targets/arm-symbian-e32.json").is_file());
    assert!(unpacked.join("crates/symbian-std/Cargo.toml").is_file());
    assert!(
        !unpacked.join("corpus").exists(),
        "only the include list is packed"
    );
    let calls = bucket.calls();
    assert_eq!(calls.len(), 8, "{calls:?}");
    assert_eq!(calls[3], "PUT index.toml");
    assert_eq!(calls[4], "GET index.toml");
}
