"""Tests of tools/third_party_notices.py on fake cargo metadata, crate trees and sysroots.

    python3 -m unittest discover -s tests -p '*_test.py'

Needs `ar` (binutils) for the fake toolchain archives.
"""

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import third_party_notices as tpn  # noqa: E402

TARGET = "x86_64-unknown-linux-musl"
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(text, bytes):
        path.write_bytes(text)
    else:
        path.write_text(text, encoding="utf-8")


class Fixture:
    """A temporary tree of crates plus the cargo metadata that names them."""

    def __init__(self, root):
        self.root = Path(root)
        self.packages = []
        self.nodes = []
        self.members = []

    def crate(self, name, version="1.0.0", license="MIT", files=None, member=False,
              links=None, license_file=None, deps=()):
        crate_dir = self.root / f"{name}-{version}"
        for rel, text in (files or {}).items():
            write(crate_dir / rel, text)
        crate_dir.mkdir(parents=True, exist_ok=True)
        pkg_id = f"{name} {version}"
        self.packages.append({
            "id": pkg_id, "name": name, "version": version, "license": license,
            "license_file": license_file, "links": links,
            "source": None if member else REGISTRY,
            "manifest_path": str(crate_dir / "Cargo.toml"),
        })
        self.nodes.append({"id": pkg_id, "deps": [
            {"pkg": f"{dep} 1.0.0" if " " not in dep else dep,
             "dep_kinds": [{"kind": kind, "target": None} for kind in kinds]}
            for dep, kinds in deps]})
        if member:
            self.members.append(pkg_id)
        return crate_dir

    def metadata(self):
        return {"packages": self.packages, "workspace_members": self.members,
                "resolve": {"nodes": self.nodes, "root": None}}


def graph_fixture(root):
    f = Fixture(root)
    f.crate("app", member=True, deps=[("a", [None]), ("b", ["build"]), ("c", ["dev"]),
                                       ("app-core", [None])])
    f.crate("app-core", member=True, deps=[("d", [None])])
    f.crate("a", files={"LICENSE-MIT": "MIT text a\n"}, deps=[("e", [None, "build"])])
    f.crate("b", files={"LICENSE": "build-only\n"})
    f.crate("c", files={"LICENSE": "dev-only\n"})
    f.crate("d", license="Apache-2.0", files={"LICENSE-APACHE": "Apache text d\n"})
    f.crate("e", license="BSD-3-Clause")
    return f


class GraphTest(unittest.TestCase):
    def test_walks_normal_dependencies_and_leaves_out_workspace_members(self):
        with tempfile.TemporaryDirectory() as tmp:
            graph = tpn.DependencyGraph(graph_fixture(tmp).metadata())
            crates, members = graph.third_party("app")
            self.assertEqual([c.name for c in crates], ["a", "d", "e"])
            self.assertEqual(members, 2)

    def test_an_unknown_root_is_an_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            graph = tpn.DependencyGraph(graph_fixture(tmp).metadata())
            with self.assertRaisesRegex(tpn.NoticeError, "no package `nope`"):
                graph.third_party("nope")


class CrateTest(unittest.TestCase):
    def test_takes_every_licence_notice_and_copyright_file_at_the_top(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Fixture(tmp)
            f.crate("x", license_file="legal/TERMS.txt", files={
                "LICENSE-MIT": "m\n", "LICENCE": "l\n", "COPYING": "c\n", "NOTICE.txt": "n\n",
                "copyright": "cr\n", "LICENSES/Apache-2.0.txt": "a\n", "README.md": "r\n",
                "legal/TERMS.txt": "t\n", "src/LICENSE": "nested\n"})
            crate = tpn.DependencyGraph(f.metadata()).crate("x 1.0.0")
            self.assertEqual([rel for rel, _ in crate.notice_files()], [
                "COPYING", "LICENCE", "LICENSE-MIT", "LICENSES/Apache-2.0.txt",
                "NOTICE.txt", "copyright", "legal/TERMS.txt"])
            self.assertEqual(crate.nested_notice_files(), ["src/LICENSE"])

    def test_a_file_that_is_not_utf8_is_an_error_naming_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Fixture(tmp)
            f.crate("x", files={"LICENSE": b"caf\xe9\n"})
            crate = tpn.DependencyGraph(f.metadata()).crate("x 1.0.0")
            with self.assertRaisesRegex(tpn.NoticeError, "x-1.0.0/LICENSE.*UTF-8"):
                crate.notice_files()


class BundledTest(unittest.TestCase):
    def zlib_fixture(self, tmp, extra=None):
        f = Fixture(tmp)
        files = {"LICENSE-MIT": "mit\n", "src/zlib/LICENSE": "zlib licence text\n",
                 "src/zlib/zlib.h": '#define ZLIB_VERSION "1.3.2"\n',
                 "src/zlib-ng/LICENSE.md": "ng\n",
                 "src/zlib/contrib/dotzlib/LICENSE_1_0.txt": "boost\n",
                 "src/zlib/contrib/minizip/LICENSE.Info-Zip": "info-zip\n"}
        files.update(extra or {})
        f.crate("libz-sys", version="1.1.29", links="z", files=files)
        return f

    def test_zlib_in_libz_sys_is_recorded_with_its_version_and_licence(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = self.zlib_fixture(tmp)
            crate = tpn.DependencyGraph(f.metadata()).crate("libz-sys 1.1.29")
            [entry] = tpn.bundled_entries(crate)
            self.assertEqual(entry.title, "zlib 1.3.2 (in libz-sys 1.1.29)")
            self.assertEqual(entry.license, "Zlib")
            self.assertEqual(entry.files, [("src/zlib/LICENSE", "zlib licence text\n")])

    def test_a_nested_licence_file_the_table_does_not_name_is_an_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = self.zlib_fixture(tmp, {"src/new/COPYING": "?\n"})
            crate = tpn.DependencyGraph(f.metadata()).crate("libz-sys 1.1.29")
            with self.assertRaisesRegex(tpn.NoticeError, "src/new/COPYING.*BUNDLED"):
                tpn.bundled_entries(crate)

    def test_a_crate_that_links_native_code_needs_an_entry(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Fixture(tmp)
            f.crate("openssl-sys", links="openssl", files={"LICENSE": "mit\n"})
            crate = tpn.DependencyGraph(f.metadata()).crate("openssl-sys 1.0.0")
            with self.assertRaisesRegex(tpn.NoticeError, "openssl-sys.*links.*BUNDLED"):
                tpn.bundled_entries(crate)

    def test_a_file_the_table_names_must_exist(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = self.zlib_fixture(tmp)
            os.remove(Path(tmp) / "libz-sys-1.1.29/src/zlib/LICENSE")
            crate = tpn.DependencyGraph(f.metadata()).crate("libz-sys 1.1.29")
            with self.assertRaisesRegex(tpn.NoticeError, "src/zlib/LICENSE"):
                tpn.bundled_entries(crate)

    def test_a_pure_rust_crate_has_no_bundled_entry(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Fixture(tmp)
            f.crate("x", files={"LICENSE": "mit\n"})
            self.assertEqual(tpn.bundled_entries(tpn.DependencyGraph(f.metadata()).crate("x 1.0.0")), [])


def ar(archive, members):
    """Writes an ar archive holding `members` ({name: bytes})."""
    with tempfile.TemporaryDirectory() as tmp:
        for name, data in members.items():
            write(Path(tmp) / name, data)
        archive.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["ar", "rcD", str(archive), *members], cwd=tmp, check=True)


STD_HTML = """<!DOCTYPE html><html><head><title>Copyright notices for The Rust Standard Library</title></head>
<body><h1>Copyright notices for The Rust Standard Library</h1>
<p>Licensed under <a href="x">Apache</a> &amp; MIT.</p>
<details><summary>LICENSE-MIT</summary><pre>
The MIT License (MIT)

    Copyright (c) 2015 Someone &lt;a@b&gt;
</pre></details></body></html>
"""

TOOLCHAIN_HTML = """<html><body>
<div style="border"><p><b>File/Directory:</b> <code>src/tools</code></p><p><b>License:</b> MIT</p></div>
<div style="border"><p><b>File/Directory:</b> <code>src/llvm-project</code></p>
<p><b>License:</b> Apache-2.0 WITH LLVM-exception AND NCSA</p>
<p><b>Copyright:</b> 2003-2019 University of Illinois at Urbana-Champaign</p></div>
<h2>Out-of-tree dependencies</h2>
<details><summary>LICENSE-MIT</summary><pre>a crate of the compiler, not of std</pre></details>
</body></html>
"""


def sysroot(root, unwind_member="h1-libunwind.o"):
    root = Path(root)
    lib = root / "lib/rustlib" / TARGET / "lib"
    sc = lib / "self-contained"
    ar(sc / "libc.a", {"version.lo": b"\x00\x001.2.5\x00GCC\x00", "printf.lo": b"x"})
    ar(sc / "libunwind.a", {unwind_member: b"u", "h1-UnwindLevel1.o": b"u"})
    write(sc / "crtbeginS.o", b"\x7fELF\x00crtbegin.c\x00__EH_FRAME_LIST__\x00")
    ar(lib / "libcompiler_builtins-abc.rlib", {"lib.rmeta": b"m", "h2-int_util.o": b"c"})
    doc = root / "share/doc/rust"
    write(doc / "COPYRIGHT-library.html", STD_HTML)
    write(doc / "COPYRIGHT.html", TOOLCHAIN_HTML)
    for spdx in ["Apache-2.0", "LLVM-exception", "NCSA", "MIT"]:
        write(doc / "licenses" / f"{spdx}.txt", f"{spdx} licence text\n")
    return root


class ToolchainTest(unittest.TestCase):
    def test_the_musl_runtime_is_recorded_from_the_toolchain_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            toolchain = tpn.Toolchain(sysroot(tmp), TARGET, "rustc 1.98.1 (x 2026-09-01)")
            std, musl, llvm = toolchain.entries()
            self.assertIn("Rust standard library", std.title)
            [(name, text)] = std.files
            self.assertEqual(name, "share/doc/rust/COPYRIGHT-library.html (as text)")
            self.assertIn("Licensed under Apache & MIT.", text)
            self.assertEqual(text.count("Copyright notices for The Rust Standard Library"), 1)
            self.assertIn("\nThe MIT License (MIT)\n\n    Copyright (c) 2015 Someone <a@b>\n",
                          text)
            self.assertEqual((musl.title, musl.license), ("musl libc 1.2.5", "MIT"))
            [(name, text)] = musl.files
            self.assertEqual(name, "musl-1.2.5/COPYRIGHT")
            self.assertIn("musl as a whole is licensed under the following standard MIT", text)
            self.assertIn("SOURCE", musl.note)
            self.assertEqual(llvm.license, "Apache-2.0 WITH LLVM-exception AND NCSA")
            names = [name for name, _ in llvm.files]
            self.assertEqual(names[1:], [f"share/doc/rust/licenses/{s}.txt"
                                         for s in ["Apache-2.0", "LLVM-exception", "NCSA"]])
            self.assertIn("University of Illinois", llvm.files[0][1])
            self.assertNotIn("src/tools", llvm.files[0][1])
            self.assertNotIn("Out-of-tree", llvm.files[0][1])
            self.assertNotIn("a crate of the compiler", llvm.files[0][1])

    def test_a_runtime_other_than_the_one_observed_is_an_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            toolchain = tpn.Toolchain(sysroot(tmp, unwind_member="h1-other.o"), TARGET, "rustc")
            with self.assertRaisesRegex(tpn.NoticeError, "libunwind.a"):
                toolchain.entries()

    def test_a_musl_version_without_its_copyright_is_an_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = sysroot(tmp)
            libc = root / "lib/rustlib" / TARGET / "lib/self-contained/libc.a"
            libc.write_bytes(libc.read_bytes().replace(b"1.2.5", b"9.9.9"))
            with self.assertRaisesRegex(tpn.NoticeError, "musl 9.9.9's COPYRIGHT"):
                tpn.Toolchain(root, TARGET, "rustc").entries()

    def test_another_target_is_an_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(tpn.NoticeError, "x86_64-unknown-linux-gnu"):
                tpn.Toolchain(sysroot(tmp), "x86_64-unknown-linux-gnu", "rustc").entries()


class NoticesTest(unittest.TestCase):
    def test_the_file_lists_every_crate_with_its_texts_and_names_the_gaps(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = graph_fixture(Path(tmp) / "crates")
            toolchain = tpn.Toolchain(sysroot(Path(tmp) / "sysroot"), TARGET, "rustc 1.98.1")
            text = tpn.Notices(tpn.DependencyGraph(f.metadata()), "app", toolchain).render()
            self.assertIn("3 Rust crates", text)
            self.assertIn("2 workspace crates", text)
            for part in ["a 1.0.0\nLicense: MIT\n", "MIT text a\n", "d 1.0.0\nLicense: Apache-2.0\n",
                         "Apache text d\n", "e 1.0.0\nLicense: BSD-3-Clause\n", "musl libc 1.2.5"]:
                self.assertIn(part, text)
            self.assertNotIn("build-only", text)
            self.assertNotIn("dev-only", text)
            self.assertIn("Entries without a licence file: e 1.0.0\n", text)
            self.assertEqual(text.count("No licence file"), 1)


if __name__ == "__main__":
    unittest.main()
