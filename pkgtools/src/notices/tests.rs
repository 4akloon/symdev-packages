//! The Python tool's tests (tests/third_party_notices_test.py), case for case, on fake
//! cargo metadata, crate trees and sysroots; they need binutils' `ar` for the fixtures.

mod bundled;
mod crates;
mod fixture;
mod sysroot;
mod toolchain;

use self::fixture::{TARGET, graph_fixture};
use self::sysroot::sysroot;
use super::{DependencyGraph, Notices, Toolchain};

#[test]
fn walks_normal_dependencies_and_leaves_out_workspace_members() {
    let tmp = tempfile::tempdir().unwrap();
    let graph = DependencyGraph::new(&graph_fixture(tmp.path()).metadata()).unwrap();
    let (crates, members) = graph.third_party("app").unwrap();
    let names: Vec<&str> = crates.iter().map(|c| c.name()).collect();
    assert_eq!(names, ["a", "d", "e"]);
    assert_eq!(members, 2);
}

#[test]
fn an_unknown_root_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let graph = DependencyGraph::new(&graph_fixture(tmp.path()).metadata()).unwrap();
    let error = graph.third_party("nope").err().unwrap().to_string();
    assert!(error.contains("no package `nope`"), "{error}");
}

#[test]
fn the_file_lists_every_crate_with_its_texts_and_names_the_gaps() {
    let tmp = tempfile::tempdir().unwrap();
    let f = graph_fixture(&tmp.path().join("crates"));
    let toolchain = Toolchain::new(
        &sysroot(&tmp.path().join("sysroot"), "h1-libunwind.o"),
        TARGET,
        "rustc 1.98.1",
    );
    let graph = DependencyGraph::new(&f.metadata()).unwrap();
    let text = Notices::new(&graph, "app", &toolchain).render().unwrap();
    assert!(text.contains("3 Rust crates"));
    assert!(text.contains("2 workspace crates"));
    for part in [
        "a 1.0.0\nLicense: MIT\n",
        "MIT text a\n",
        "d 1.0.0\nLicense: Apache-2.0\n",
        "Apache text d\n",
        "e 1.0.0\nLicense: BSD-3-Clause\n",
        "musl libc 1.2.5",
    ] {
        assert!(text.contains(part), "{part:?}");
    }
    assert!(!text.contains("build-only"));
    assert!(!text.contains("dev-only"));
    assert!(text.contains("Entries without a licence file: e 1.0.0\n"));
    assert_eq!(text.matches("No licence file").count(), 1);
}
