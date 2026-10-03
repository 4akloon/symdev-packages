//! The `pkgtools` binary as the scripts and workflows call it: exit codes and the lines
//! they print.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn pkgtools(args: &[&dyn AsRef<std::ffi::OsStr>]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pkgtools"));
    for arg in args {
        command.arg(arg);
    }
    command.output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn tree(root: &Path, files: &[(&str, &[u8])]) {
    for (rel, data) in files {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }
}

/// tests/sdk_free_test.py's `test_main_exits_1_on_a_leak_and_0_without`.
#[test]
fn sdk_free_exits_1_on_a_leak_and_0_without() {
    let tmp = tempfile::tempdir().unwrap();
    let (sdk, out) = (tmp.path().join("sdk"), tmp.path().join("out"));
    let header: &[u8] = b"// e32std.h, a file of the SDK\nclass TDesC;\n";
    tree(
        &sdk,
        &[
            ("epoc32/include/e32std.h", header),
            ("epoc32/include/empty.h", b""),
        ],
    );
    tree(&out, &[("ok", b"fine")]);
    let clean = pkgtools(&[&"sdk-free", &sdk, &out]);
    assert_eq!(clean.status.code(), Some(0), "{}", text(&clean.stderr));
    let said = format!(
        "no file of the SDK (1 distinct files) in {}\n",
        out.display()
    );
    assert_eq!(text(&clean.stdout), said);
    tree(&out, &[("copy.h", header)]);
    let leak = pkgtools(&[&"sdk-free", &sdk, &out]);
    assert_eq!(leak.status.code(), Some(1));
    let err = text(&leak.stderr);
    assert!(
        err.contains("copy.h: the bytes of the SDK's epoc32/include/e32std.h"),
        "{err}"
    );
    assert!(err.ends_with(&format!(
        "error: 1 file(s) of the S60 SDK in {}\n",
        out.display()
    )));
    assert_eq!(pkgtools(&[&"sdk-free", &sdk]).status.code(), Some(2));
    let missing = pkgtools(&[&"sdk-free", &sdk, &tmp.path().join("missing.tar")]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(text(&missing.stderr).contains("missing.tar does not exist"));
}

#[test]
fn device_entry_prints_the_entry_and_exits_1_without_it() {
    let tmp = tempfile::tempdir().unwrap();
    let yml = tmp.path().join("devices.yml");
    fs::write(&yml, "RM-469:\n  firmcode: RM-469\n").unwrap();
    let ok = pkgtools(&[&"device-entry", &yml, &"RM-469"]);
    assert_eq!(ok.status.code(), Some(0), "{}", text(&ok.stderr));
    assert_eq!(text(&ok.stdout), "RM-469:\n  firmcode: RM-469\n");
    let missing = pkgtools(&[&"device-entry", &yml, &"RM-1"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        text(&missing.stderr).starts_with("error: "),
        "{}",
        text(&missing.stderr)
    );
}
