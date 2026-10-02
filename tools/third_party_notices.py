#!/usr/bin/env python3
"""Write THIRD-PARTY-NOTICES.txt for a static symdev binary (spec 2026-10-02 §12).

    third_party_notices.py --manifest-path <checkout>/Cargo.toml --package symdev-cli \\
        --target x86_64-unknown-linux-musl --output <file>

Lists what the binary links: (1) every crate `cargo metadata --filter-platform <target>`
resolves from <package> through normal dependencies, but the workspace's own crates, with
its SPDX expression and the full text of each LICENSE*/LICENCE*/COPYING*/NOTICE*/COPYRIGHT*
file at the top of its source; (2) the code those crates bundle (BUNDLED below); (3) the
Rust toolchain's runtime for <target>: the standard library, musl and LLVM's runtime
pieces, from the toolchain's own notice files. Nothing is guessed: an entry without a file
says so, and a crate or runtime this script does not know is an error.
Needs python3, cargo, rustc and ar (binutils).
"""

import argparse
import html
import json
import os
import re
import subprocess
import sys
from html.parser import HTMLParser
from pathlib import Path

NOTICE = re.compile(r"(?i)^(licen[cs]e|copying|notice|copyright)")
RULE = "-" * 79


class NoticeError(Exception):
    pass


class Entry:
    """One section of the file: a title, an SPDX expression and the texts that go with it."""

    def __init__(self, title, license, files, source="", note=""):
        self.title, self.license, self.files = title, license, files
        self.source, self.note = source, note

    def render(self):
        head = [RULE, self.title, f"License: {self.license}"]
        head += [f"Source: {self.source}"] if self.source else []
        body = [self.note] if self.note else []
        if not self.files:
            body.append("No licence file was shipped with it; its SPDX expression is above.")
        for name, text in self.files:
            body.append(f"== {name} ==\n\n{text.rstrip()}")
        return "\n".join(head + [RULE, ""]) + "\n\n".join(body) + "\n"


def read_text(path, shown):
    try:
        return Path(path).read_text(encoding="utf-8")
    except UnicodeDecodeError as e:
        raise NoticeError(f"{shown} is not UTF-8 ({e}); convert it by hand in a copy") from e
    except OSError as e:
        raise NoticeError(f"cannot read {shown}: {e.strerror}") from e


class Crate:
    def __init__(self, package):
        self.name, self.version = package["name"], package["version"]
        self.license = package.get("license") or "NOASSERTION"
        self.license_file, self.links = package.get("license_file"), package.get("links")
        self.source = package.get("source") or "path"
        self.dir = Path(package["manifest_path"]).parent

    def shown(self, rel):
        return f"{self.dir.name}/{rel}"

    def notice_files(self):
        """[(relative path, text)] of the licence files at the top, and of a license-file."""
        rels = []
        for entry in sorted(os.listdir(self.dir)):
            if not NOTICE.match(entry):
                continue
            if (self.dir / entry).is_dir():
                rels += sorted(str(p.relative_to(self.dir)) for p in (self.dir / entry).rglob("*")
                               if p.is_file())
            else:
                rels.append(entry)
        if self.license_file and self.license_file not in rels:
            rels.append(self.license_file)
        return [(rel, read_text(self.dir / rel, self.shown(rel))) for rel in rels]

    def nested_notice_files(self):
        found = []
        for top, dirs, files in os.walk(self.dir):
            dirs.sort()
            if Path(top) == self.dir:
                dirs[:] = [d for d in dirs if not NOTICE.match(d)]
                continue
            found += [str((Path(top) / f).relative_to(self.dir)) for f in sorted(files)
                      if NOTICE.match(f)]
        return found

    def entry(self):
        return Entry(f"{self.name} {self.version}", self.license, self.notice_files(), self.source)


class DependencyGraph:
    """`cargo metadata --format-version 1` output."""

    def __init__(self, metadata):
        self.packages = {p["id"]: p for p in metadata["packages"]}
        self.nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
        self.members = set(metadata["workspace_members"])

    def crate(self, pkg_id):
        return Crate(self.packages[pkg_id])

    def third_party(self, root_name):
        """The crates `root_name` reaches through normal dependencies, sorted, and how many
        workspace crates the walk passed through."""
        roots = [i for i in self.members if self.packages[i]["name"] == root_name]
        if len(roots) != 1:
            raise NoticeError(f"the workspace has no package `{root_name}`; pass --package "
                              "with the binary's package name")
        seen, stack = set(), roots
        while stack:
            pkg_id = stack.pop()
            if pkg_id not in seen:
                seen.add(pkg_id)
                stack += [d["pkg"] for d in self.nodes[pkg_id]["deps"]
                          if any(k["kind"] is None for k in d["dep_kinds"])]
        crates = sorted((self.crate(i) for i in seen - self.members),
                        key=lambda c: (c.name, c.version))
        return crates, len(seen & self.members)


# Code inside a crate that has its own licence. Every crate with `links` (native code) and
# every licence file below a crate's top must be named here: as a component the binary
# links, or under `not_linked` with the reason (checked against the crate's build.rs).
BUNDLED = {
    "libz-sys": {
        "components": [{
            "title": "zlib {version}", "license": "Zlib", "files": ["src/zlib/LICENSE"],
            "version": ("src/zlib/zlib.h", r'#define ZLIB_VERSION "([^"]+)"'),
            "note": "The C library in src/zlib, compiled by libz-sys' build script and linked "
                    "statically (build.sh sets LIBZ_SYS_STATIC=1, so no system libz is used)."}],
        "not_linked": {
            "src/zlib-ng/LICENSE.md": "zlib-ng is built only with libz-sys' zlib-ng features",
            "src/zlib/contrib/dotzlib/LICENSE_1_0.txt": "zlib's contrib/ is not compiled",
            "src/zlib/contrib/minizip/LICENSE.Info-Zip": "zlib's contrib/ is not compiled"}},
    "ring": {
        "components": [
            {"title": "BoringSSL-derived C and assembly", "license": "Apache-2.0 AND ISC",
             "files": ["LICENSE", "LICENSE-BoringSSL", "LICENSE-other-bits"],
             "note": "crypto/, include/ and the pregenerated assembly, compiled by ring's "
                     "build script; each file's header names its licence."},
            {"title": "fiat-crypto", "license": "Apache-2.0", "files": ["third_party/fiat/LICENSE"],
             "note": "third_party/fiat: C headers and x86_64 assembly ring compiles."},
            {"title": "once_cell (ring's polyfill)", "license": "MIT OR Apache-2.0",
             "files": ["src/polyfill/once_cell/LICENSE-APACHE", "src/polyfill/once_cell/LICENSE-MIT"],
             "note": "Rust code from once_cell in src/polyfill/once_cell, compiled into ring."}],
        "not_linked": {}},
}


def bundled_entries(crate):
    table = BUNDLED.get(crate.name)
    nested = crate.nested_notice_files()
    if table is None:
        if crate.links or nested:
            why = f"links `{crate.links}`" if crate.links else f"has {', '.join(nested)}"
            raise NoticeError(f"{crate.name} {crate.version} {why}: find the licence of the code "
                              f"it bundles and add {crate.name} to BUNDLED in {__file__}")
        return []
    named = {f for c in table["components"] for f in c["files"]} | set(table["not_linked"])
    for rel in nested:
        if rel not in named:
            raise NoticeError(f"{crate.name} {crate.version} has {rel}, which BUNDLED in "
                              f"{__file__} does not name; decide whether that code is linked")
    entries = []
    for component in table["components"]:
        title = component["title"]
        if "version" in component:
            rel, pattern = component["version"]
            found = re.search(pattern, read_text(crate.dir / rel, crate.shown(rel)))
            if not found:
                raise NoticeError(f"no version in {crate.shown(rel)} (looked for {pattern})")
            title = title.format(version=found.group(1))
        files = [(rel, read_text(crate.dir / rel, crate.shown(rel))) for rel in component["files"]]
        entries.append(Entry(f"{title} (in {crate.name} {crate.version})", component["license"],
                             files, note=component["note"]))
    return entries


class HtmlText(HTMLParser):
    """The text of an HTML page's body: <pre> verbatim, other text with its whitespace
    collapsed, a line break around each block."""

    BLOCKS = {"p", "div", "h1", "h2", "h3", "h4", "li", "ul", "details", "summary", "br", "pre",
              "table", "tr"}

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.parts, self.line, self.pre, self.head = [], "", 0, 0

    def flush(self):
        if self.line.strip():
            self.parts.append(self.line.strip() + "\n")
        self.line = ""

    def handle_starttag(self, tag, attrs):
        if tag in self.BLOCKS:
            self.flush()
        self.pre += tag == "pre"
        self.head += tag == "head"

    def handle_endtag(self, tag):
        self.pre -= tag == "pre"
        self.head -= tag == "head"
        if tag in self.BLOCKS:
            self.flush()

    def handle_data(self, data):
        if self.head:
            return
        if self.pre:
            self.parts.append(data)
        else:
            self.line += re.sub(r"\s+", " ", data)

    @staticmethod
    def of(text):
        parser = HtmlText()
        parser.feed(text)
        parser.close()
        parser.flush()
        return "".join(parser.parts)


class Toolchain:
    """The Rust toolchain's runtime for the target, as observed for rustc 1.98.1's
    x86_64-unknown-linux-musl (its link line: self-contained crt objects, libunwind, libc)."""

    TARGETS = {"x86_64-unknown-linux-musl"}

    def __init__(self, sysroot, target, rustc_version):
        self.sysroot, self.target, self.rustc = Path(sysroot), target, rustc_version
        self.doc = self.sysroot / "share/doc/rust"

    def doc_text(self, rel):
        return read_text(self.doc / rel, f"share/doc/rust/{rel}")

    def entries(self):
        if self.target not in self.TARGETS:
            raise NoticeError(f"the runtime of {self.target} was never examined; only "
                              f"{', '.join(sorted(self.TARGETS))} is known")
        lib = self.sysroot / "lib/rustlib" / self.target / "lib"
        sc = lib / "self-contained"
        musl = re.findall(rb"\x00(\d+\.\d+\.\d+)\x00", ar_member(sc / "libc.a", "version.lo"))
        if len(set(musl)) != 1:
            raise NoticeError(f"no single musl version in {sc}/libc.a(version.lo): {musl}")
        builtins = sorted(lib.glob("libcompiler_builtins-*.rlib"))
        expect(len(builtins) == 1, f"one libcompiler_builtins-*.rlib in {lib}, found {builtins}")
        expect(any(m.endswith("libunwind.o") for m in ar_names(sc / "libunwind.a")),
               f"LLVM's libunwind.o in {sc}/libunwind.a")
        expect(b"__EH_FRAME_LIST__" in (sc / "crtbeginS.o").read_bytes(),
               f"compiler-rt's crtbegin (__EH_FRAME_LIST__) in {sc}/crtbeginS.o")
        expect(any(m.endswith("int_util.o") for m in ar_names(builtins[0])),
               f"compiler-rt's C builtins (int_util.o) in {builtins[0]}")
        std = Entry(f"The Rust standard library (rust-std of {self.rustc}, {self.target})",
                    "Apache-2.0 OR MIT, and the exceptions below",
                    [("share/doc/rust/COPYRIGHT-library.html (as text)",
                      HtmlText.of(self.doc_text("COPYRIGHT-library.html")))],
                    note="std, core, alloc and the crates they use, as the toolchain's own "
                         "notice file lists them.")
        version = musl[0].decode()
        copyright = Path(__file__).resolve().parent / "notices" / f"musl-{version}" / "COPYRIGHT"
        expect(copyright.is_file(),
               f"musl {version}'s COPYRIGHT at {copyright} (the toolchain ships libc.a without "
               f"it; add it from the signed musl-{version} release tarball)")
        musl_entry = Entry(f"musl libc {version}", "MIT",
                           [(f"musl-{version}/COPYRIGHT", read_text(copyright, str(copyright)))],
                           note=("libc.a and the crt1/rcrt1/crti/crtn objects of the "
                                 "toolchain's self-contained directory; the licence file comes "
                                 "from musl's signed release tarball (tools/notices/"
                                 f"musl-{version}/SOURCE)."))
        return [std, musl_entry, self.llvm()]

    def llvm(self):
        """The `src/llvm-project` entry of COPYRIGHT.html (a <div> of File/Directory,
        License and Copyright lines) and the licence texts its expression names."""
        page = self.doc_text("COPYRIGHT.html")
        at = page.find("<code>src/llvm-project</code>")
        start, end = page.rfind("<div", 0, at), page.find("</div>", at)
        if min(at, start, end) < 0:
            raise NoticeError("share/doc/rust/COPYRIGHT.html has no <div> for src/llvm-project")
        entry = HtmlText.of(page[start:end])
        spdx = re.search(r"^License: (.+)$", entry, re.M).group(1).strip()
        ids = [i for i in re.split(r"[\s()]+", spdx) if i and i not in {"AND", "OR", "WITH"}]
        files = [("share/doc/rust/COPYRIGHT.html, the src/llvm-project entry", entry)]
        files += [(f"share/doc/rust/licenses/{i}.txt", self.doc_text(f"licenses/{i}.txt"))
                  for i in ids]
        return Entry("LLVM's runtime: libunwind, compiler-rt's crtbegin/crtend and builtins",
                     spdx, files, note="libunwind.a, crtbeginS.o/crtendS.o and the C objects in "
                                       "compiler_builtins, built from the toolchain's "
                                       "src/llvm-project.")


def expect(ok, what):
    if not ok:
        raise NoticeError(f"expected {what}; the toolchain's runtime changed, so check what "
                          "the binary links and update Toolchain in " + __file__)


def run(args, cwd=None):
    try:
        done = subprocess.run(args, cwd=cwd, check=True, capture_output=True)
    except (OSError, subprocess.CalledProcessError) as e:
        detail = getattr(e, "stderr", b"") or b""
        raise NoticeError(f"`{' '.join(map(str, args))}` failed: {e} {detail.decode()}") from e
    return done.stdout


def ar_names(archive):
    return run(["ar", "t", archive]).decode().split()


def ar_member(archive, member):
    expect(member in ar_names(archive), f"{member} in {archive}")
    return run(["ar", "p", archive, member])


class Notices:
    def __init__(self, graph, package, toolchain):
        self.graph, self.package, self.toolchain = graph, package, toolchain

    def render(self):
        crates, members = self.graph.third_party(self.package)
        sections = [
            (f"1. {len(crates)} Rust crates", [c.entry() for c in crates]),
            ("2. Code bundled inside those crates", [e for c in crates for e in bundled_entries(c)]),
            ("3. The Rust toolchain's runtime", self.toolchain.entries())]
        gaps = [e.title for _, entries in sections for e in entries if not e.files]
        head = (f"THIRD-PARTY NOTICES for bin/symdev ({self.toolchain.target})\n\n"
                f"bin/symdev is symdev (MIT, see LICENSE beside this file; its {members} workspace "
                "crates are covered by it) linked statically with the software below. Each "
                "entry gives the licence its authors declare (an SPDX expression) and the full "
                "text of every licence, notice and copyright file it ships.\n\n"
                f"Crates: `cargo metadata --filter-platform {self.toolchain.target}` from "
                f"{self.package} through normal dependencies. Toolchain: {self.toolchain.rustc}.\n\n"
                + "".join(f"  {title} ({len(entries)} entries)\n" for title, entries in sections)
                + f"\nEntries without a licence file: {', '.join(gaps) or 'none'}\n")
        body = [f"\n{'=' * 79}\n{title}\n{'=' * 79}\n\n" + "\n".join(e.render() for e in entries)
                for title, entries in sections]
        return head + "".join(body)


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--manifest-path", required=True, type=Path)
    p.add_argument("--package", required=True)
    p.add_argument("--target", required=True)
    p.add_argument("--output", required=True, type=Path)
    a = p.parse_args()
    checkout = a.manifest_path.resolve().parent
    try:
        metadata = json.loads(run(["cargo", "metadata", "--format-version", "1", "--locked",
                                   "--filter-platform", a.target, "--manifest-path",
                                   a.manifest_path], cwd=checkout))
        sysroot = run(["rustc", "--print", "sysroot"], cwd=checkout).decode().strip()
        rustc = run(["rustc", "-V"], cwd=checkout).decode().strip()
        text = Notices(DependencyGraph(metadata), a.package,
                       Toolchain(sysroot, a.target, rustc)).render()
        partial = a.output.with_name(a.output.name + ".partial")
        partial.write_text(text, encoding="utf-8")
        partial.replace(a.output)
    except (NoticeError, OSError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    print(f"{a.output}: {len(text.encode())} bytes, {text.split(chr(10))[0]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
