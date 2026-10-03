//! `Bundled`: the code inside a crate that has a licence of its own.

use std::path::Path;

use super::super::{Bundled, DependencyGraph};
use super::fixture::{Fixture, Spec};

fn zlib_fixture(root: &Path, extra: &[(&str, &[u8])]) -> Fixture {
    let mut f = Fixture::new(root);
    let mut files: Vec<(&str, &[u8])> = vec![
        ("LICENSE-MIT", b"mit\n"),
        ("src/zlib/LICENSE", b"zlib licence text\n"),
        ("src/zlib/zlib.h", b"#define ZLIB_VERSION \"1.3.2\"\n"),
        ("src/zlib-ng/LICENSE.md", b"ng\n"),
        ("src/zlib/contrib/dotzlib/LICENSE_1_0.txt", b"boost\n"),
        ("src/zlib/contrib/minizip/LICENSE.Info-Zip", b"info-zip\n"),
    ];
    files.extend_from_slice(extra);
    let spec = Spec { name: "libz-sys", version: "1.1.29", links: Some("z"), ..Spec::default() };
    f.add(Spec { files: &files, ..spec });
    f
}

fn error_of(f: &Fixture, id: &str) -> String {
    let krate = DependencyGraph::new(&f.metadata()).unwrap().crate_by_id(id).unwrap();
    Bundled::entries(&krate).err().unwrap().to_string()
}

#[test]
fn zlib_in_libz_sys_is_recorded_with_its_version_and_licence() {
    let tmp = tempfile::tempdir().unwrap();
    let f = zlib_fixture(tmp.path(), &[]);
    let krate = DependencyGraph::new(&f.metadata()).unwrap().crate_by_id("libz-sys 1.1.29");
    let entries = Bundled::entries(&krate.unwrap()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "zlib 1.3.2 (in libz-sys 1.1.29)");
    assert_eq!(entries[0].license, "Zlib");
    let file = ("src/zlib/LICENSE".to_string(), "zlib licence text\n".to_string());
    assert_eq!(entries[0].files, [file]);
}

#[test]
fn a_nested_licence_file_the_table_does_not_name_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let f = zlib_fixture(tmp.path(), &[("src/new/COPYING", b"?\n")]);
    let error = error_of(&f, "libz-sys 1.1.29");
    assert!(error.contains("src/new/COPYING") && error.contains("BUNDLED"), "{error}");
}

#[test]
fn a_crate_that_links_native_code_needs_an_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = Fixture::new(tmp.path());
    let spec = Spec { name: "openssl-sys", links: Some("openssl"), ..Spec::default() };
    f.add(Spec { files: &[("LICENSE", b"mit\n")], ..spec });
    let error = error_of(&f, "openssl-sys 1.0.0");
    assert!(error.contains("openssl-sys") && error.contains("links"), "{error}");
    assert!(error.contains("BUNDLED"), "{error}");
}

#[test]
fn a_file_the_table_names_must_exist() {
    let tmp = tempfile::tempdir().unwrap();
    let f = zlib_fixture(tmp.path(), &[]);
    std::fs::remove_file(tmp.path().join("libz-sys-1.1.29/src/zlib/LICENSE")).unwrap();
    let error = error_of(&f, "libz-sys 1.1.29");
    assert!(error.contains("src/zlib/LICENSE"), "{error}");
}

#[test]
fn a_pure_rust_crate_has_no_bundled_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = Fixture::new(tmp.path());
    f.add(Spec { name: "x", files: &[("LICENSE", b"mit\n")], ..Spec::default() });
    let krate = DependencyGraph::new(&f.metadata()).unwrap().crate_by_id("x 1.0.0").unwrap();
    assert!(Bundled::entries(&krate).unwrap().is_empty());
}
