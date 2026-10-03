//! `Toolchain`: the Rust toolchain's runtime for the musl target, from its own files.

use std::fs;

use super::super::{HtmlText, Toolchain};
use super::fixture::TARGET;
use super::sysroot::sysroot;

#[test]
fn the_musl_runtime_is_recorded_from_the_toolchain_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = sysroot(tmp.path(), "h1-libunwind.o");
    let entries = Toolchain::new(&root, TARGET, "rustc 1.98.1 (x 2026-09-01)")
        .entries()
        .unwrap();
    let [std, musl, llvm] = &entries[..] else {
        panic!("{} entries", entries.len())
    };
    assert!(std.title.contains("Rust standard library"));
    let [(name, text)] = &std.files[..] else {
        panic!("{:?}", std.files)
    };
    assert_eq!(name, "share/doc/rust/COPYRIGHT-library.html (as text)");
    assert!(text.contains("Licensed under Apache & MIT."), "{text}");
    assert_eq!(
        text.matches("Copyright notices for The Rust Standard Library")
            .count(),
        1
    );
    assert!(text.contains("\nThe MIT License (MIT)\n\n    Copyright (c) 2015 Someone <a@b>\n"));
    assert_eq!(
        (musl.title.as_str(), musl.license.as_str()),
        ("musl libc 1.2.5", "MIT")
    );
    let [(name, text)] = &musl.files[..] else {
        panic!("{:?}", musl.files)
    };
    assert_eq!(name, "musl-1.2.5/COPYRIGHT");
    assert!(text.contains("musl as a whole is licensed under the following standard MIT"));
    assert!(musl.note.contains("SOURCE"));
    assert_eq!(llvm.license, "Apache-2.0 WITH LLVM-exception AND NCSA");
    let names: Vec<&str> = llvm.files.iter().map(|(n, _)| n.as_str()).collect();
    let licences = ["Apache-2.0", "LLVM-exception", "NCSA"];
    let expected: Vec<String> = licences
        .iter()
        .map(|s| format!("share/doc/rust/licenses/{s}.txt"))
        .collect();
    assert_eq!(names[1..], expected);
    let entry = &llvm.files[0].1;
    assert!(entry.contains("University of Illinois"));
    for absent in ["src/tools", "Out-of-tree", "a crate of the compiler"] {
        assert!(!entry.contains(absent), "{absent}: {entry}");
    }
}

#[test]
fn a_runtime_other_than_the_one_observed_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let toolchain = Toolchain::new(&sysroot(tmp.path(), "h1-other.o"), TARGET, "rustc");
    let error = toolchain.entries().err().unwrap().to_string();
    assert!(error.contains("libunwind.a"), "{error}");
}

#[test]
fn a_musl_version_without_its_copyright_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let root = sysroot(tmp.path(), "h1-libunwind.o");
    let libc = root
        .join("lib/rustlib")
        .join(TARGET)
        .join("lib/self-contained/libc.a");
    let bytes = fs::read(&libc).unwrap();
    let at = bytes.windows(5).position(|w| w == b"1.2.5").unwrap();
    let mut changed = bytes.clone();
    changed[at..at + 5].copy_from_slice(b"9.9.9");
    fs::write(&libc, changed).unwrap();
    let error = Toolchain::new(&root, TARGET, "rustc")
        .entries()
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("musl 9.9.9's COPYRIGHT"), "{error}");
}

#[test]
fn another_target_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let root = sysroot(tmp.path(), "h1-libunwind.o");
    let toolchain = Toolchain::new(&root, "x86_64-unknown-linux-gnu", "rustc");
    let error = toolchain.entries().err().unwrap().to_string();
    assert!(error.contains("x86_64-unknown-linux-gnu"), "{error}");
}

#[test]
fn text_between_inline_tags_keeps_its_spaces_as_python_parsed_it() {
    // HTMLParser hands the text between two tags over in one piece, and the Python tool
    // collapsed each piece alone: "a " + " b" stays "a  b".
    let text = HtmlText::of("<p>a <b>x</b> b\n\n<i>c</i>&#34;q&#39;</p>").unwrap();
    assert_eq!(text, "a x b c\"q'\n");
    assert_eq!(HtmlText::of("<p>a <b> </b> b</p>").unwrap(), "a   b\n");
}

#[test]
fn an_entity_never_seen_in_the_toolchain_pages_is_an_error() {
    let error = HtmlText::of("<p>&copy; 2026</p>").unwrap_err().to_string();
    assert!(error.contains("&copy;"), "{error}");
}

#[test]
fn a_long_unknown_entity_name_with_wide_characters_is_an_error_not_a_panic() {
    let page = format!("<p>&{}éé;</p>", "a".repeat(31));
    let error = HtmlText::of(&page).unwrap_err().to_string();
    assert!(error.contains("never seen here"), "{error}");
}

#[test]
fn an_unreadable_crtbegin_is_an_io_error_naming_it() {
    let tmp = tempfile::tempdir().unwrap();
    let root = sysroot(tmp.path(), "h1-libunwind.o");
    let crtbegin = root
        .join("lib/rustlib")
        .join(TARGET)
        .join("lib/self-contained/crtbeginS.o");
    fs::remove_file(&crtbegin).unwrap();
    let error = Toolchain::new(&root, TARGET, "rustc")
        .entries()
        .err()
        .unwrap()
        .to_string();
    assert!(
        error.starts_with("cannot read ") && error.contains("crtbeginS.o"),
        "{error}"
    );
}
