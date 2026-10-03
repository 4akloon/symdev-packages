//! The Python tool's tests (tests/sdk_free_test.py), case for case, on a fake SDK tree and
//! fake build outputs; its `main` test is in tests/cli.rs.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::SdkFiles;

const HEADER: &[u8] = b"// e32std.h, a file of the SDK\nclass TDesC;\n";

fn tree(root: &Path, files: &[(&str, &[u8])]) {
    for (rel, data) in files {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }
}

/// A tar (gzipped if `gz`) of `files`.
fn tar_bytes(files: &[(&str, &[u8])], gz: bool) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, data) in files {
        let mut header = tar::Header::new_ustar();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, name, *data).unwrap();
    }
    let data = builder.into_inner().unwrap();
    if !gz {
        return data;
    }
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&data).unwrap();
    gz.finish().unwrap()
}

/// A temporary directory with the fake SDK in `sdk/`.
struct Setup {
    tmp: tempfile::TempDir,
}

impl Setup {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let sdk = tmp.path().join("sdk");
        tree(
            &sdk,
            &[
                ("epoc32/include/e32std.h", HEADER),
                ("epoc32/include/empty.h", b""),
            ],
        );
        Self { tmp }
    }

    fn root(&self) -> &Path {
        self.tmp.path()
    }

    fn sdk(&self) -> PathBuf {
        self.root().join("sdk")
    }

    fn check(&self, paths: &[&Path]) -> Vec<String> {
        let paths: Vec<PathBuf> = paths.iter().map(|p| p.to_path_buf()).collect();
        SdkFiles::of(&self.sdk()).unwrap().leaks(&paths).unwrap()
    }
}

#[test]
fn outputs_without_sdk_content_pass() {
    let s = Setup::new();
    let out = s.root().join("out");
    tree(
        &out,
        &[
            (
                "rust-sdk/symbian-rs/prebuilt/lib/libsymrs.a",
                b"!<arch>\nobjects",
            ),
            ("rust-sdk/symbian-rs/shims/s60/symrs_avkon.cpp", b"// MIT\n"),
        ],
    );
    assert_eq!(s.check(&[&out]), Vec::<String>::new());
}

#[test]
fn an_sdk_file_under_another_name_in_a_nested_archive_is_found() {
    let s = Setup::new();
    let inner = tar_bytes(&[("a/renamed.txt", HEADER)], true);
    let outer = s.root().join("symdev-out.tar");
    fs::write(
        &outer,
        tar_bytes(
            &[("out/x-source.tar.gz", &inner), ("out/ok", b"fine")],
            false,
        ),
    )
    .unwrap();
    let leaks = s.check(&[&outer]);
    assert_eq!(leaks.len(), 1, "{leaks:?}");
    assert!(leaks[0].contains("out/x-source.tar.gz"), "{leaks:?}");
    assert!(leaks[0].contains("a/renamed.txt"), "{leaks:?}");
    assert!(leaks[0].contains("epoc32/include/e32std.h"), "{leaks:?}");
}

#[test]
fn a_path_through_epoc32_is_found_whatever_its_case() {
    let s = Setup::new();
    let packed = s.root().join("packed.tar.gz");
    fs::write(
        &packed,
        tar_bytes(&[("x/EPOC32/release/note.txt", b"not the SDK's")], true),
    )
    .unwrap();
    let leaks = s.check(&[&packed]);
    assert_eq!(leaks.len(), 1, "{leaks:?}");
    assert!(leaks[0].contains("x/EPOC32/release/note.txt"), "{leaks:?}");
}

#[test]
fn only_the_part_below_a_named_directory_is_checked_for_epoc32() {
    let s = Setup::new();
    let named = s.root().join("EPOC32").join("notes");
    tree(&named, &[("epoc32/x.txt", b"one"), ("y.txt", b"two")]);
    let leaks = s.check(&[&named]);
    assert_eq!(leaks.len(), 1, "{leaks:?}");
    assert!(leaks[0].contains("epoc32/x.txt"), "{leaks:?}");
}

#[test]
fn a_link_into_the_sdk_in_a_directory_is_found() {
    let s = Setup::new();
    let out = s.root().join("out");
    fs::create_dir(&out).unwrap();
    std::os::unix::fs::symlink(s.sdk().join("epoc32/include/e32std.h"), out.join("header"))
        .unwrap();
    assert_eq!(s.check(&[&out]).len(), 1);
}

#[test]
fn empty_files_match_nothing() {
    let s = Setup::new();
    let out = s.root().join("out");
    tree(&out, &[("empty", b"")]);
    assert_eq!(s.check(&[&out]), Vec::<String>::new());
}

#[test]
fn an_sdk_without_files_is_refused_so_the_check_cannot_pass_vacuously() {
    let s = Setup::new();
    let empty = s.root().join("nothing");
    fs::create_dir(&empty).unwrap();
    assert!(SdkFiles::of(&empty).is_err());
    assert!(SdkFiles::of(&s.root().join("missing")).is_err());
}

#[test]
fn a_path_that_does_not_exist_is_refused() {
    let s = Setup::new();
    let sdk = SdkFiles::of(&s.sdk()).unwrap();
    assert!(sdk.leaks(&[s.root().join("missing.tar")]).is_err());
}

#[test]
fn a_tar_gz_of_nothing_cannot_be_read_as_python_s_tarfile_could_not() {
    let s = Setup::new();
    let packed = s.root().join("nothing.tar.gz");
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(b"").unwrap();
    fs::write(&packed, gz.finish().unwrap()).unwrap();
    let sdk = SdkFiles::of(&s.sdk()).unwrap();
    let error = sdk.leaks(&[packed]).unwrap_err().to_string();
    assert!(
        error.contains("nothing.tar.gz as a tar: empty file"),
        "{error}"
    );
}
