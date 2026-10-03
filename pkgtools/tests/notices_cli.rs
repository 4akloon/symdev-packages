//! `pkgtools notices` as build.sh calls it, with a fake `cargo` and `rustc` on PATH: the
//! argv it gives them, the file it writes and the line it prints.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const TARGET: &str = "x86_64-unknown-linux-musl";

fn write(path: &Path, data: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}

fn ar(archive: &Path, members: &[(&str, &[u8])]) {
    let tmp = tempfile::tempdir().unwrap();
    for (name, data) in members {
        write(&tmp.path().join(name), data);
    }
    fs::create_dir_all(archive.parent().unwrap()).unwrap();
    let names: Vec<&str> = members.iter().map(|(n, _)| *n).collect();
    let mut command = Command::new("ar");
    command
        .arg("rcD")
        .arg(archive)
        .args(names)
        .current_dir(tmp.path());
    assert!(command.status().unwrap().success());
}

/// The smallest sysroot the notices accept: musl 1.2.5, LLVM's runtime, the two pages.
fn sysroot(root: &Path) {
    let lib = root.join("lib/rustlib").join(TARGET).join("lib");
    let sc = lib.join("self-contained");
    ar(
        &sc.join("libc.a"),
        &[("version.lo", b"\x00\x001.2.5\x00GCC\x00")],
    );
    ar(&sc.join("libunwind.a"), &[("libunwind.o", b"u")]);
    write(&sc.join("crtbeginS.o"), b"\x00__EH_FRAME_LIST__\x00");
    ar(
        &lib.join("libcompiler_builtins-abc.rlib"),
        &[("int_util.o", b"c")],
    );
    let doc = root.join("share/doc/rust");
    write(
        &doc.join("COPYRIGHT-library.html"),
        b"<html><body><p>std</p></body></html>",
    );
    let llvm = "<div><p><code>src/llvm-project</code></p><p>License: NCSA</p></div>";
    write(&doc.join("COPYRIGHT.html"), llvm.as_bytes());
    write(&doc.join("licenses/NCSA.txt"), b"NCSA text\n");
}

/// `cargo metadata` of a checkout whose `app` uses one crate, `dep`.
fn metadata(crates: &Path) -> String {
    write(&crates.join("dep-1.0.0/LICENSE"), b"dep licence\n");
    let app = crates.join("app/Cargo.toml");
    let dep = crates.join("dep-1.0.0/Cargo.toml");
    format!(
        r#"{{"packages": [
  {{"id": "app 0.1.0", "name": "app", "version": "0.1.0", "license": "MIT", "source": null, "manifest_path": "{}"}},
  {{"id": "dep 1.0.0", "name": "dep", "version": "1.0.0", "license": "MIT", "source": "registry+x", "manifest_path": "{}"}}],
 "workspace_members": ["app 0.1.0"],
 "resolve": {{"nodes": [
  {{"id": "app 0.1.0", "deps": [{{"pkg": "dep 1.0.0", "dep_kinds": [{{"kind": null}}]}}]}},
  {{"id": "dep 1.0.0", "deps": []}}]}}}}"#,
        app.display(),
        dep.display()
    )
}

/// A fake `cargo` and `rustc` on PATH that answer as a checkout with one crate and the
/// sysroot `sysroot` would, and log their argv.
fn fake_toolchain(dir: &Path, metadata: &str, sysroot: &Path) -> std::ffi::OsString {
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(dir.join("metadata.json"), metadata).unwrap();
    let log = dir.join("argv.log");
    let cargo = format!(
        "#!/bin/sh\necho \"cargo $* (in $PWD)\" >>{log}\ncat {json}\n",
        log = log.display(),
        json = dir.join("metadata.json").display()
    );
    let rustc = format!(
        "#!/bin/sh\necho \"rustc $* (in $PWD)\" >>{log}\n\
         if [ \"$1\" = -V ]; then echo 'rustc 1.98.1 (fake)'; else echo {sysroot}; fi\n",
        log = log.display(),
        sysroot = sysroot.display()
    );
    for (name, script) in [("cargo", cargo), ("rustc", rustc)] {
        fs::write(bin.join(name), script).unwrap();
        let mode = std::os::unix::fs::PermissionsExt::from_mode(0o755);
        fs::set_permissions(bin.join(name), mode).unwrap();
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap()
}

fn notices(tmp: &Path, path: &std::ffi::OsStr, output: &Path) -> Output {
    let manifest = tmp.join("checkout/Cargo.toml");
    write(&manifest, b"[workspace]\n");
    Command::new(env!("CARGO_BIN_EXE_pkgtools"))
        .args(["notices", "--manifest-path"])
        .arg(&manifest)
        .args(["--package", "app", "--target", TARGET, "--output"])
        .arg(output)
        .env("PATH", path)
        .output()
        .unwrap()
}

#[test]
fn notices_writes_the_file_and_names_its_size_and_first_line() {
    let tmp = tempfile::tempdir().unwrap();
    sysroot(&tmp.path().join("sysroot"));
    let metadata = metadata(&tmp.path().join("crates"));
    let path = fake_toolchain(tmp.path(), &metadata, &tmp.path().join("sysroot"));
    let output = tmp.path().join("THIRD-PARTY-NOTICES.txt");
    let done = notices(tmp.path(), &path, &output);
    assert_eq!(
        done.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
    let text = fs::read_to_string(&output).unwrap();
    assert!(text.starts_with(&format!("THIRD-PARTY NOTICES for bin/symdev ({TARGET})\n")));
    assert!(
        text.contains("dep 1.0.0\nLicense: MIT\nSource: registry+x\n")
            && text.contains("dep licence")
    );
    let said = format!(
        "{}: {} bytes, THIRD-PARTY NOTICES for bin/symdev ({TARGET})\n",
        output.display(),
        text.len()
    );
    assert_eq!(String::from_utf8_lossy(&done.stdout), said);
    assert!(!tmp.path().join("THIRD-PARTY-NOTICES.txt.partial").exists());
    let checkout = tmp.path().join("checkout");
    let argv = fs::read_to_string(tmp.path().join("argv.log")).unwrap();
    let cargo = format!(
        "cargo metadata --format-version 1 --locked --filter-platform {TARGET} --manifest-path {} (in {})",
        checkout.join("Cargo.toml").display(),
        checkout.display()
    );
    assert!(argv.lines().any(|l| l == cargo), "{argv}");
    let rustc = format!("rustc --print sysroot (in {})", checkout.display());
    assert!(argv.lines().any(|l| l == rustc), "{argv}");
}

#[test]
fn notices_writes_nothing_when_a_crate_cannot_be_accounted_for() {
    let tmp = tempfile::tempdir().unwrap();
    sysroot(&tmp.path().join("sysroot"));
    let metadata = metadata(&tmp.path().join("crates")).replace(
        "\"license\": \"MIT\", \"source\": \"registry+x\"",
        "\"links\": \"z\", \"source\": \"registry+x\"",
    );
    let path = fake_toolchain(tmp.path(), &metadata, &tmp.path().join("sysroot"));
    let output = tmp.path().join("THIRD-PARTY-NOTICES.txt");
    let done = notices(tmp.path(), &path, &output);
    assert_eq!(done.status.code(), Some(1));
    let err = String::from_utf8_lossy(&done.stderr);
    assert!(err.starts_with("error: dep 1.0.0 links `z`"), "{err}");
    assert!(!output.exists());
}
