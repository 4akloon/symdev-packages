//! A fake Rust sysroot for x86_64-unknown-linux-musl: the runtime archives and the
//! toolchain's notice pages.

use std::path::{Path, PathBuf};

use super::fixture::{TARGET, ar, write};

pub const STD_HTML: &str = r#"<!DOCTYPE html><html><head><title>Copyright notices for The Rust Standard Library</title></head>
<body><h1>Copyright notices for The Rust Standard Library</h1>
<p>Licensed under <a href="x">Apache</a> &amp; MIT.</p>
<details><summary>LICENSE-MIT</summary><pre>
The MIT License (MIT)

    Copyright (c) 2015 Someone &lt;a@b&gt;
</pre></details></body></html>
"#;

pub const TOOLCHAIN_HTML: &str = r#"<html><body>
<div style="border"><p><b>File/Directory:</b> <code>src/tools</code></p><p><b>License:</b> MIT</p></div>
<div style="border"><p><b>File/Directory:</b> <code>src/llvm-project</code></p>
<p><b>License:</b> Apache-2.0 WITH LLVM-exception AND NCSA</p>
<p><b>Copyright:</b> 2003-2019 University of Illinois at Urbana-Champaign</p></div>
<h2>Out-of-tree dependencies</h2>
<details><summary>LICENSE-MIT</summary><pre>a crate of the compiler, not of std</pre></details>
</body></html>
"#;

/// The sysroot under `root`; `unwind_member` names libunwind.a's first member.
pub fn sysroot(root: &Path, unwind_member: &str) -> PathBuf {
    let lib = root.join("lib/rustlib").join(TARGET).join("lib");
    let sc = lib.join("self-contained");
    ar(&sc.join("libc.a"), &[("version.lo", b"\x00\x001.2.5\x00GCC\x00"), ("printf.lo", b"x")]);
    ar(&sc.join("libunwind.a"), &[(unwind_member, b"u"), ("h1-UnwindLevel1.o", b"u")]);
    write(&sc.join("crtbeginS.o"), b"\x7fELF\x00crtbegin.c\x00__EH_FRAME_LIST__\x00");
    let rlib = lib.join("libcompiler_builtins-abc.rlib");
    ar(&rlib, &[("lib.rmeta", b"m"), ("h2-int_util.o", b"c")]);
    let doc = root.join("share/doc/rust");
    write(&doc.join("COPYRIGHT-library.html"), STD_HTML.as_bytes());
    write(&doc.join("COPYRIGHT.html"), TOOLCHAIN_HTML.as_bytes());
    for spdx in ["Apache-2.0", "LLVM-exception", "NCSA", "MIT"] {
        let text = format!("{spdx} licence text\n");
        write(&doc.join("licenses").join(format!("{spdx}.txt")), text.as_bytes());
    }
    root.to_path_buf()
}
