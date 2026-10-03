//! `Crate`: the licence files at the top of a crate, and those below it.

use super::super::DependencyGraph;
use super::fixture::{Fixture, Spec};

#[test]
fn takes_every_licence_notice_and_copyright_file_at_the_top() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = Fixture::new(tmp.path());
    let files: &[(&str, &[u8])] = &[
        ("LICENSE-MIT", b"m\n"),
        ("LICENCE", b"l\n"),
        ("COPYING", b"c\n"),
        ("NOTICE.txt", b"n\n"),
        ("copyright", b"cr\n"),
        ("LICENSES/Apache-2.0.txt", b"a\n"),
        ("README.md", b"r\n"),
        ("legal/TERMS.txt", b"t\n"),
        ("src/LICENSE", b"nested\n"),
    ];
    let license_file = Some("legal/TERMS.txt");
    f.add(Spec { name: "x", license_file, files, ..Spec::default() });
    let krate = DependencyGraph::new(&f.metadata()).unwrap().crate_by_id("x 1.0.0").unwrap();
    let rels: Vec<String> = krate.notice_files().unwrap().into_iter().map(|(r, _)| r).collect();
    let expected = [
        "COPYING",
        "LICENCE",
        "LICENSE-MIT",
        "LICENSES/Apache-2.0.txt",
        "NOTICE.txt",
        "copyright",
        "legal/TERMS.txt",
    ];
    assert_eq!(rels, expected);
    assert_eq!(krate.nested_notice_files().unwrap(), ["src/LICENSE"]);
}

#[test]
fn a_file_that_is_not_utf8_is_an_error_naming_it() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = Fixture::new(tmp.path());
    f.add(Spec { name: "x", files: &[("LICENSE", b"caf\xe9\n")], ..Spec::default() });
    let krate = DependencyGraph::new(&f.metadata()).unwrap().crate_by_id("x 1.0.0").unwrap();
    let error = krate.notice_files().unwrap_err().to_string();
    assert!(error.contains("x-1.0.0/LICENSE") && error.contains("UTF-8"), "{error}");
}

#[test]
fn a_licence_file_with_crlf_reads_as_python_text_mode_reads_it() {
    // Python's read_text: universal newlines, so the notices never hold a \r.
    let tmp = tempfile::tempdir().unwrap();
    let mut f = Fixture::new(tmp.path());
    f.add(Spec { name: "x", files: &[("LICENSE", b"a\r\nb\rc\n")], ..Spec::default() });
    let krate = DependencyGraph::new(&f.metadata()).unwrap().crate_by_id("x 1.0.0").unwrap();
    assert_eq!(krate.notice_files().unwrap(), [("LICENSE".to_string(), "a\nb\nc\n".to_string())]);
}
