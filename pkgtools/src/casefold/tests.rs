//! The Python tool's tests (tests/sdk_casefold_test.py), case for case, on a fake
//! epoc32/include tree.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::IncludeOverlay;

fn tree(root: &Path, files: &[(&str, &[u8])]) {
    for (rel, data) in files {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }
}

/// {relative link: target} of every symlink under `out`.
fn links(out: &Path) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let mut stack = vec![out.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.file_type().is_symlink() {
                let rel = path
                    .strip_prefix(out)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();
                let target = fs::read_link(&path).unwrap();
                found.insert(rel, target.to_str().unwrap().to_string());
            } else if meta.is_dir() {
                stack.push(path);
            }
        }
    }
    found
}

fn map(pairs: &[(&str, &Path)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
        .collect()
}

#[test]
fn links_each_include_name_the_tree_has_only_in_another_case() {
    let tmp = tempfile::tempdir().unwrap();
    let (inc, out) = (tmp.path().join("include"), tmp.path().join("overlay"));
    tree(
        &inc,
        &[
            ("fbs.h", b"#include <FbsMessage.h>\n#include <e32std.h>\n"),
            ("fbsmessage.h", b""),
            ("e32std.h", b"  #  include \"Variant\\Symbian_OS.hrh\"\n"),
            ("variant/symbian_os.hrh", b""),
        ],
    );
    IncludeOverlay::ensure(&inc, &out).unwrap();
    assert_eq!(
        links(&out),
        map(&[
            ("FbsMessage.h", &inc.join("fbsmessage.h")),
            (
                "Variant/Symbian_OS.hrh",
                &inc.join("variant/symbian_os.hrh")
            ),
        ])
    );
    let marker = fs::read_to_string(out.join(".symdev-casefold")).unwrap();
    assert_eq!(marker, inc.to_str().unwrap());
}

#[test]
fn names_that_exist_as_written_or_not_at_all_get_no_link() {
    let tmp = tempfile::tempdir().unwrap();
    let (inc, out) = (tmp.path().join("include"), tmp.path().join("overlay"));
    let a = b"#include <e32std.h>\n#include <Missing.h>\n#include </abs/Path.h>\n\
              #include <../Up.h>\n#include MACRO_NAME\n// #include <A.H>\n";
    tree(
        &inc,
        &[
            ("a.h", a),
            ("e32std.h", b""),
            ("up.h", b""),
            ("path.h", b""),
        ],
    );
    IncludeOverlay::ensure(&inc, &out).unwrap();
    assert_eq!(links(&out), BTreeMap::new());
}

#[test]
fn a_name_two_files_share_up_to_case_takes_the_first_in_sorted_order() {
    let tmp = tempfile::tempdir().unwrap();
    let (inc, out) = (tmp.path().join("include"), tmp.path().join("overlay"));
    tree(
        &inc,
        &[
            ("x.h", b"#include <DUP.h>\n"),
            ("Dup.h", b""),
            ("dup.h", b""),
        ],
    );
    IncludeOverlay::ensure(&inc, &out).unwrap();
    assert_eq!(links(&out), map(&[("DUP.h", &inc.join("Dup.h"))]));
}

#[test]
fn an_existing_overlay_is_reused() {
    let tmp = tempfile::tempdir().unwrap();
    let (inc, out) = (tmp.path().join("include"), tmp.path().join("overlay"));
    tree(&inc, &[("x.h", b"#include <Y.h>\n"), ("y.h", b"")]);
    IncludeOverlay::ensure(&inc, &out).unwrap();
    tree(&inc, &[("z.h", b"#include <W.h>\n"), ("w.h", b"")]);
    IncludeOverlay::ensure(&inc, &out).unwrap();
    assert_eq!(links(&out).into_keys().collect::<Vec<_>>(), ["Y.h"]);
}

#[test]
fn bytes_that_are_not_utf8_are_read_as_far_as_they_go() {
    let tmp = tempfile::tempdir().unwrap();
    let (inc, out) = (tmp.path().join("include"), tmp.path().join("overlay"));
    tree(
        &inc,
        &[("x.h", b"// caf\xe9\n#include <Y.h>\n"), ("y.h", b"")],
    );
    IncludeOverlay::ensure(&inc, &out).unwrap();
    assert_eq!(links(&out).into_keys().collect::<Vec<_>>(), ["Y.h"]);
}

#[test]
fn a_missing_include_directory_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let error = IncludeOverlay::ensure(&tmp.path().join("nope"), &tmp.path().join("overlay"));
    let error = error.unwrap_err().to_string();
    assert!(error.contains("not a directory"), "{error}");
}
