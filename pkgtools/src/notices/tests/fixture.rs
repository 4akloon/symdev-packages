//! Fake cargo metadata, crate trees and sysroots, as tests/third_party_notices_test.py
//! built them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

pub const TARGET: &str = "x86_64-unknown-linux-musl";
const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";

pub fn write(path: &Path, data: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}

/// One crate of a [`Fixture`]; `deps` are (name or id, dependency kinds), `None` = normal.
pub struct Spec<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub license: &'a str,
    pub files: &'a [(&'a str, &'a [u8])],
    pub member: bool,
    pub links: Option<&'a str>,
    pub license_file: Option<&'a str>,
    pub deps: &'a [(&'a str, &'a [Option<&'a str>])],
}

impl Default for Spec<'_> {
    fn default() -> Self {
        Spec {
            name: "",
            version: "1.0.0",
            license: "MIT",
            files: &[],
            member: false,
            links: None,
            license_file: None,
            deps: &[],
        }
    }
}

/// A temporary tree of crates plus the cargo metadata that names them.
pub struct Fixture {
    root: PathBuf,
    packages: Vec<Value>,
    nodes: Vec<Value>,
    members: Vec<String>,
}

impl Fixture {
    pub fn new(root: &Path) -> Self {
        let root = root.to_path_buf();
        Fixture { root, packages: Vec::new(), nodes: Vec::new(), members: Vec::new() }
    }

    pub fn add(&mut self, spec: Spec) -> PathBuf {
        let dir = self.root.join(format!("{}-{}", spec.name, spec.version));
        for (rel, data) in spec.files {
            write(&dir.join(rel), data);
        }
        fs::create_dir_all(&dir).unwrap();
        let id = format!("{} {}", spec.name, spec.version);
        self.packages.push(json!({
            "id": id, "name": spec.name, "version": spec.version, "license": spec.license,
            "license_file": spec.license_file, "links": spec.links,
            "source": if spec.member { None } else { Some(REGISTRY) },
            "manifest_path": dir.join("Cargo.toml"),
        }));
        let deps: Vec<Value> = spec
            .deps
            .iter()
            .map(|(dep, kinds)| {
                let pkg = if dep.contains(' ') { dep.to_string() } else { format!("{dep} 1.0.0") };
                let kinds: Vec<Value> =
                    kinds.iter().map(|k| json!({"kind": k, "target": null})).collect();
                json!({"pkg": pkg, "dep_kinds": kinds})
            })
            .collect();
        self.nodes.push(json!({"id": id, "deps": deps}));
        if spec.member {
            self.members.push(id);
        }
        dir
    }

    pub fn metadata(&self) -> Value {
        json!({"packages": self.packages, "workspace_members": self.members,
               "resolve": {"nodes": self.nodes, "root": null}})
    }
}

pub fn graph_fixture(root: &Path) -> Fixture {
    let mut f = Fixture::new(root);
    let (normal, build, dev): (&[_], &[_], &[_]) = (&[None], &[Some("build")], &[Some("dev")]);
    let app_deps = [("a", normal), ("b", build), ("c", dev), ("app-core", normal)];
    f.add(Spec { name: "app", member: true, deps: &app_deps, ..Spec::default() });
    f.add(Spec { name: "app-core", member: true, deps: &[("d", normal)], ..Spec::default() });
    let a_deps = [("e", &[None, Some("build")][..])];
    let a_files = [("LICENSE-MIT", &b"MIT text a\n"[..])];
    f.add(Spec { name: "a", files: &a_files, deps: &a_deps, ..Spec::default() });
    f.add(Spec { name: "b", files: &[("LICENSE", b"build-only\n")], ..Spec::default() });
    f.add(Spec { name: "c", files: &[("LICENSE", b"dev-only\n")], ..Spec::default() });
    let d_files = [("LICENSE-APACHE", &b"Apache text d\n"[..])];
    f.add(Spec { name: "d", license: "Apache-2.0", files: &d_files, ..Spec::default() });
    f.add(Spec { name: "e", license: "BSD-3-Clause", ..Spec::default() });
    f
}

/// Writes an ar archive holding `members` with binutils' `ar rcD`.
pub fn ar(archive: &Path, members: &[(&str, &[u8])]) {
    let tmp = tempfile::tempdir().unwrap();
    for (name, data) in members {
        write(&tmp.path().join(name), data);
    }
    fs::create_dir_all(archive.parent().unwrap()).unwrap();
    let names: Vec<&str> = members.iter().map(|(n, _)| *n).collect();
    let status =
        Command::new("ar").arg("rcD").arg(archive).args(names).current_dir(tmp.path()).status();
    assert!(status.unwrap().success(), "ar rcD {}", archive.display());
}
