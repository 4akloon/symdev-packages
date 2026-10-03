//! `Toolchain`: the Rust toolchain's runtime for the target, from its own files.

use std::path::{Path, PathBuf};

use super::licence_text::LicenceText;
use super::{Entry, HtmlText};
use crate::ar_archive::ArArchive;
use crate::py_text::PyText;
use crate::tool_error::{Result, ToolError};

/// musl's COPYRIGHT by version, from its signed release tarball (tools/notices/musl-*/,
/// SOURCE says where): the toolchain ships libc.a without it.
const MUSL_COPYRIGHTS: &[(&str, &str)] = &[(
    "1.2.5",
    include_str!("../../../tools/notices/musl-1.2.5/COPYRIGHT"),
)];

/// The runtime as observed for rustc 1.98.1's x86_64-unknown-linux-musl (its link line:
/// self-contained crt objects, libunwind, libc). Anything else is an error, not a guess.
pub struct Toolchain {
    sysroot: PathBuf,
    pub(super) target: String,
    pub(super) rustc: String,
}

impl Toolchain {
    const TARGETS: [&str; 1] = ["x86_64-unknown-linux-musl"];

    /// `rustc` is `rustc -V`'s line.
    pub fn new(sysroot: &Path, target: &str, rustc: &str) -> Self {
        Self {
            sysroot: sysroot.to_path_buf(),
            target: target.into(),
            rustc: rustc.into(),
        }
    }

    fn doc_text(&self, rel: &str) -> Result<String> {
        let path = self.sysroot.join("share/doc/rust").join(rel);
        LicenceText::read(&path, &format!("share/doc/rust/{rel}"))
    }

    /// The standard library, musl, and LLVM's runtime pieces, in that order.
    pub fn entries(&self) -> Result<Vec<Entry>> {
        if !Self::TARGETS.contains(&self.target.as_str()) {
            return Err(ToolError::new(format!(
                "the runtime of {} was never examined; only {} is known",
                self.target,
                Self::TARGETS.join(", ")
            )));
        }
        let lib = self
            .sysroot
            .join("lib/rustlib")
            .join(&self.target)
            .join("lib");
        let sc = lib.join("self-contained");
        let libc = ArArchive::read(&sc.join("libc.a"))?;
        let version_lo = libc.member("version.lo");
        Self::expect(
            version_lo.is_some(),
            &format!("version.lo in {}", sc.join("libc.a").display()),
        )?;
        let musl = Self::versions(&version_lo.unwrap_or_default());
        if musl.is_empty() || musl.iter().any(|v| *v != musl[0]) {
            return Err(ToolError::new(format!(
                "no single musl version in {}/libc.a(version.lo): {musl:?}",
                sc.display()
            )));
        }
        let builtins = Self::compiler_builtins(&lib)?;
        let unwind = ArArchive::read(&sc.join("libunwind.a"))?;
        let has_unwind = unwind.names().iter().any(|m| m.ends_with("libunwind.o"));
        Self::expect(
            has_unwind,
            &format!("LLVM's libunwind.o in {}/libunwind.a", sc.display()),
        )?;
        let crtbegin = std::fs::read(sc.join("crtbeginS.o")).unwrap_or_default();
        let has_list = crtbegin.windows(17).any(|w| w == b"__EH_FRAME_LIST__");
        let what = format!(
            "compiler-rt's crtbegin (__EH_FRAME_LIST__) in {}/crtbeginS.o",
            sc.display()
        );
        Self::expect(has_list, &what)?;
        let has_int_util = ArArchive::read(&builtins)?
            .names()
            .iter()
            .any(|m| m.ends_with("int_util.o"));
        let what = format!(
            "compiler-rt's C builtins (int_util.o) in {}",
            builtins.display()
        );
        Self::expect(has_int_util, &what)?;
        let std_text = HtmlText::of(&self.doc_text("COPYRIGHT-library.html")?)?;
        let std = Entry {
            title: format!(
                "The Rust standard library (rust-std of {}, {})",
                self.rustc, self.target
            ),
            license: "Apache-2.0 OR MIT, and the exceptions below".into(),
            files: vec![(
                "share/doc/rust/COPYRIGHT-library.html (as text)".into(),
                std_text,
            )],
            source: String::new(),
            note: "std, core, alloc and the crates they use, as the toolchain's own notice file \
                   lists them."
                .into(),
        };
        Ok(vec![std, Self::musl(&musl[0])?, self.llvm()?])
    }

    /// `re.findall(rb"\x00(\d+\.\d+\.\d+)\x00", data)`.
    fn versions(data: &[u8]) -> Vec<String> {
        let mut found = Vec::new();
        let mut at = 0;
        while at < data.len() {
            if data[at] == 0
                && let Some(len) = Self::version_at(&data[at + 1..])
            {
                found.push(String::from_utf8_lossy(&data[at + 1..at + 1 + len]).into_owned());
                at += len + 2;
                continue;
            }
            at += 1;
        }
        found
    }

    /// The length of `<digits>.<digits>.<digits>` at the start of `data` if a NUL follows.
    fn version_at(data: &[u8]) -> Option<usize> {
        let mut at = 0;
        for part in 0..3 {
            let digits = data[at..].iter().take_while(|b| b.is_ascii_digit()).count();
            if digits == 0 {
                return None;
            }
            at += digits;
            let wanted = if part < 2 { b'.' } else { 0 };
            if data.get(at) != Some(&wanted) {
                return None;
            }
            at += usize::from(part < 2);
        }
        Some(at)
    }

    fn compiler_builtins(lib: &Path) -> Result<PathBuf> {
        let unreadable =
            |e: std::io::Error| ToolError::io(format!("cannot read {}", lib.display()), &e);
        let mut found = Vec::new();
        for entry in std::fs::read_dir(lib).map_err(unreadable)? {
            let name = entry
                .map_err(unreadable)?
                .file_name()
                .to_string_lossy()
                .into_owned();
            if name.starts_with("libcompiler_builtins-") && name.ends_with(".rlib") {
                found.push(lib.join(name));
            }
        }
        found.sort();
        let what = format!(
            "one libcompiler_builtins-*.rlib in {}, found {found:?}",
            lib.display()
        );
        Self::expect(found.len() == 1, &what)?;
        Ok(found.remove(0))
    }

    fn expect(ok: bool, what: &str) -> Result<()> {
        if ok {
            return Ok(());
        }
        Err(ToolError::new(format!(
            "expected {what}; the toolchain's runtime changed, so check what the binary links \
             and update Toolchain in pkgtools/src/notices/toolchain.rs"
        )))
    }

    fn musl(version: &str) -> Result<Entry> {
        let copyright = MUSL_COPYRIGHTS
            .iter()
            .find(|(v, _)| *v == version)
            .map(|(_, text)| *text);
        let what = format!(
            "musl {version}'s COPYRIGHT at tools/notices/musl-{version}/COPYRIGHT (the toolchain \
             ships libc.a without it; add it from the signed musl-{version} release tarball, \
             and to MUSL_COPYRIGHTS)"
        );
        Self::expect(copyright.is_some(), &what)?;
        Ok(Entry {
            title: format!("musl libc {version}"),
            license: "MIT".into(),
            files: vec![(
                format!("musl-{version}/COPYRIGHT"),
                PyText::universal_newlines(copyright.unwrap_or_default()),
            )],
            source: String::new(),
            note: format!(
                "libc.a and the crt1/rcrt1/crti/crtn objects of the toolchain's self-contained \
                 directory; the licence file comes from musl's signed release tarball \
                 (tools/notices/musl-{version}/SOURCE)."
            ),
        })
    }

    /// The `src/llvm-project` entry of COPYRIGHT.html (a <div> of File/Directory, License
    /// and Copyright lines) and the licence texts its expression names.
    fn llvm(&self) -> Result<Entry> {
        let page = self.doc_text("COPYRIGHT.html")?;
        let missing =
            || ToolError::new("share/doc/rust/COPYRIGHT.html has no <div> for src/llvm-project");
        let at = page
            .find("<code>src/llvm-project</code>")
            .ok_or_else(missing)?;
        let start = page[..at].rfind("<div").ok_or_else(missing)?;
        let end = page[at..]
            .find("</div>")
            .map(|i| at + i)
            .ok_or_else(missing)?;
        let entry = HtmlText::of(&page[start..end])?;
        let spdx = entry
            .split('\n')
            .find_map(|line| {
                line.strip_prefix("License: ")
                    .filter(|rest| !rest.is_empty())
            })
            .map(|rest| PyText::strip(rest).to_string())
            .ok_or_else(|| ToolError::new("the src/llvm-project entry has no License: line"))?;
        let ids = spdx
            .split(|c: char| PyText::is_space(c) || c == '(' || c == ')')
            .filter(|id| !id.is_empty() && !["AND", "OR", "WITH"].contains(id));
        let mut files = vec![(
            "share/doc/rust/COPYRIGHT.html, the src/llvm-project entry".into(),
            entry.clone(),
        )];
        for id in ids {
            let rel = format!("licenses/{id}.txt");
            files.push((format!("share/doc/rust/{rel}"), self.doc_text(&rel)?));
        }
        Ok(Entry {
            title: "LLVM's runtime: libunwind, compiler-rt's crtbegin/crtend and builtins".into(),
            license: spdx,
            files,
            source: String::new(),
            note: "libunwind.a, crtbeginS.o/crtendS.o and the C objects in compiler_builtins, \
                   built from the toolchain's src/llvm-project."
                .into(),
        })
    }
}
