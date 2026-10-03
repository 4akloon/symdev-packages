//! `Checkout`: the source checkout whose binary the notices are for, asked through cargo
//! and rustc in its own directory (so its rust-toolchain.toml picks the toolchain).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::tool_error::{Result, ToolError};

pub struct Checkout {
    manifest: PathBuf,
    dir: PathBuf,
}

impl Checkout {
    pub fn new(manifest: &Path) -> Result<Self> {
        let manifest = manifest
            .canonicalize()
            .map_err(|e| ToolError::io(format!("cannot find {}", manifest.display()), &e))?;
        let dir = manifest.parent().unwrap_or(Path::new("/")).to_path_buf();
        Ok(Self { manifest, dir })
    }

    /// `cargo metadata --format-version 1 --locked --filter-platform <target>`.
    pub fn metadata(&self, target: &str) -> Result<Value> {
        let manifest = self.manifest.to_string_lossy();
        let args = [
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--filter-platform",
            target,
        ];
        let out = self.run(
            "cargo",
            &[&args[..], &["--manifest-path", &manifest]].concat(),
        )?;
        serde_json::from_slice(&out)
            .map_err(|e| ToolError::new(format!("cargo metadata printed no JSON: {e}")))
    }

    pub fn sysroot(&self) -> Result<PathBuf> {
        let out = self.run("rustc", &["--print", "sysroot"])?;
        Ok(PathBuf::from(String::from_utf8_lossy(&out).trim()))
    }

    /// `rustc -V`'s line.
    pub fn rustc_version(&self) -> Result<String> {
        Ok(String::from_utf8_lossy(&self.run("rustc", &["-V"])?)
            .trim()
            .to_string())
    }

    fn run(&self, program: &str, args: &[&str]) -> Result<Vec<u8>> {
        let shown = format!("{program} {}", args.join(" "));
        let out = Command::new(program)
            .args(args)
            .current_dir(&self.dir)
            .output()
            .map_err(|e| ToolError::new(format!("`{shown}` failed: {e}")))?;
        if !out.status.success() {
            return Err(ToolError::new(format!(
                "`{shown}` failed: {} {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim_end()
            )));
        }
        Ok(out.stdout)
    }
}
