use std::fs;
use std::path::Path;

use super::EmulatorNotices;

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// EKA2L1's source: its LICENSE, two submodules, one with a nested submodule.
fn source(src: &Path) {
    write(
        &src.join("LICENSE"),
        "GNU GENERAL PUBLIC LICENSE\nVersion 3\n",
    );
    write(
        &src.join(".gitmodules"),
        "[submodule \"fmt\"]\n\tpath = src/external/fmt\n\turl = x\n[submodule \"dyn\"]\n\tpath = src/external/dynarmic\n\turl = y\n",
    );
    write(&src.join("src/external/fmt/LICENSE.rst"), "MIT\n");
    write(&src.join("src/external/dynarmic/LICENSE.txt"), "0BSD\n");
    write(
        &src.join("src/external/dynarmic/.gitmodules"),
        "[submodule \"z\"]\n\tpath = externals/zydis\n\turl = z\n",
    );
    write(
        &src.join("src/external/dynarmic/externals/zydis/LICENSE"),
        "MIT zydis\n",
    );
}

fn tree(root: &Path) {
    write(&root.join("usr/share/doc/libfoo1/copyright"), "Format: …\n");
}

fn notices(src: &Path, tree: &Path, packages: Option<&Path>) -> EmulatorNotices {
    EmulatorNotices {
        src: src.to_path_buf(),
        tree: tree.to_path_buf(),
        id: "emulator;2026.10.04".into(),
        commit: "0123456789abcdef0123456789abcdef01234567".into(),
        packages: packages.map(Path::to_path_buf),
        extra: Vec::new(),
    }
}

#[test]
fn writes_the_gpl_every_submodules_licence_and_where_the_source_is() {
    let (src, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    source(src.path());
    tree(out.path());
    notices(src.path(), out.path(), None).write().unwrap();
    let doc = out.path().join("share/doc/eka2l1");
    assert_eq!(
        fs::read_to_string(doc.join("COPYING")).unwrap(),
        "GNU GENERAL PUBLIC LICENSE\nVersion 3\n"
    );
    for f in [
        "src/external/fmt/LICENSE.rst",
        "src/external/dynarmic/LICENSE.txt",
        "src/external/dynarmic/externals/zydis/LICENSE",
    ] {
        assert!(doc.join("third-party").join(f).is_file(), "{f}");
    }
    let source = fs::read_to_string(doc.join("SOURCE.txt")).unwrap();
    assert!(
        source.contains("emulator;2026.10.04")
            && source.contains("0123456789abcdef0123456789abcdef01234567"),
        "{source}"
    );
    assert!(source.contains("source-code"), "{source}");
}

#[test]
fn a_submodule_without_a_licence_file_is_named_until_listed_as_extra() {
    let (src, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    source(src.path());
    tree(out.path());
    fs::remove_file(src.path().join("src/external/fmt/LICENSE.rst")).unwrap();
    write(
        &src.path().join("src/external/fmt/README.md"),
        "MIT, see header\n",
    );
    let e = notices(src.path(), out.path(), None)
        .write()
        .unwrap_err()
        .to_string();
    assert!(e.contains("src/external/fmt"), "{e}");
    let mut n = notices(src.path(), out.path(), None);
    n.extra = vec!["src/external/fmt/README.md".into()];
    n.write().unwrap();
    assert!(
        out.path()
            .join("share/doc/eka2l1/third-party/src/external/fmt/README.md")
            .is_file()
    );
}

#[test]
fn the_bundled_list_points_at_each_packages_copyright_and_refuses_a_missing_one() {
    let (src, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    source(src.path());
    tree(out.path());
    let tsv = src.path().join("packages.tsv");
    write(&tsv, "libfoo1:amd64\t1.2-3\tfoo\t1.2-3\n");
    notices(src.path(), out.path(), Some(&tsv)).write().unwrap();
    let list = fs::read_to_string(out.path().join("share/doc/eka2l1/BUNDLED.tsv")).unwrap();
    assert_eq!(
        list,
        "package\tversion\tsource\tsource version\tcopyright\n\
         libfoo1:amd64\t1.2-3\tfoo\t1.2-3\tusr/share/doc/libfoo1/copyright\n"
    );
    write(&tsv, "libbar2:amd64\t2\tbar\t2\n");
    let e = notices(src.path(), out.path(), Some(&tsv))
        .write()
        .unwrap_err()
        .to_string();
    assert!(e.contains("usr/share/doc/libbar2/copyright"), "{e}");
}

#[test]
fn an_extra_list_skips_comments_and_blank_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let list = tmp.path().join("notices-extra.txt");
    write(
        &list,
        "# fmt: MIT, in its README\nsrc/external/fmt/README.md\n\n  \n",
    );
    let extra = EmulatorNotices::read_extra(&list).unwrap();
    assert_eq!(extra, [Path::new("src/external/fmt/README.md")]);
    let e = EmulatorNotices::read_extra(&tmp.path().join("none"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("none"), "{e}");
}
