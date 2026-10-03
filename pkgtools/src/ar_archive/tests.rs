//! `ArArchive` against binutils: what `ar t` lists and `ar p` prints.

use std::fs;
use std::path::Path;
use std::process::Command;

use super::ArArchive;

fn ar_t(archive: &Path) -> String {
    let out = Command::new("ar").arg("t").arg(archive).output().unwrap();
    assert!(
        out.status.success(),
        "ar t: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// An archive made by `ar rcD` of members with short and long (GNU `//` table) names.
fn archive(dir: &Path) -> std::path::PathBuf {
    let members = [
        ("version.lo", &b"\x00\x001.2.5\x00"[..]),
        ("45c91108d938afe8-int_util.o", b"odd length"),
        (
            "compiler_builtins-1c2d.compiler_builtins.4f-cgu.000.rcgu.o",
            b"\x7fELF",
        ),
        ("x.o", b""),
    ];
    for (name, data) in members {
        fs::write(dir.join(name), data).unwrap();
    }
    let path = dir.join("lib.a");
    let names: Vec<&str> = members.iter().map(|(n, _)| *n).collect();
    let mut args = vec!["rcD"];
    args.push(path.to_str().unwrap());
    args.extend(names);
    let status = Command::new("ar")
        .args(&args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());
    path
}

#[test]
fn names_are_what_ar_t_lists() {
    let tmp = tempfile::tempdir().unwrap();
    let path = archive(tmp.path());
    let listed = ar_t(&path);
    let read = ArArchive::read(&path).unwrap();
    assert_eq!(read.names(), listed.lines().collect::<Vec<_>>());
}

#[test]
fn a_member_is_what_ar_p_prints() {
    let tmp = tempfile::tempdir().unwrap();
    let path = archive(tmp.path());
    let read = ArArchive::read(&path).unwrap();
    for name in read.names() {
        let printed = Command::new("ar")
            .arg("p")
            .arg(&path)
            .arg(name)
            .output()
            .unwrap();
        assert_eq!(read.member(name), Some(printed.stdout), "{name}");
    }
    assert_eq!(read.member("missing.o"), None);
}

#[test]
fn a_file_that_is_not_an_archive_is_an_error_naming_it() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("libc.a");
    fs::write(&path, b"\x7fELF not an archive").unwrap();
    let error = ArArchive::read(&path).err().unwrap().to_string();
    assert!(
        error.contains("libc.a") && error.contains("not an ar archive"),
        "{error}"
    );
}
