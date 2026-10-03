//! `NoticesTool`: `pkgtools notices --manifest-path … --package … --target … --output …`.

use std::io::Write;
use std::path::Path;

use super::checkout::Checkout;
use super::{DependencyGraph, Notices, Toolchain};
use crate::tool_error::{Result, ToolError};

/// Writes the notices through `<output>.partial` and a rename, so a failure leaves no file
/// that looks complete; prints `<output>: <bytes> bytes, <first line>`.
pub struct NoticesTool;

impl NoticesTool {
    pub fn run(
        manifest: &Path,
        package: &str,
        target: &str,
        output: &Path,
        out: &mut impl Write,
        err: &mut impl Write,
    ) -> u8 {
        match Self::write(manifest, package, target, output) {
            Ok(text) => {
                let first = text.split('\n').next().unwrap_or_default();
                let _ = writeln!(out, "{}: {} bytes, {first}", output.display(), text.len());
                0
            }
            Err(e) => {
                let _ = writeln!(err, "error: {e}");
                1
            }
        }
    }

    fn write(manifest: &Path, package: &str, target: &str, output: &Path) -> Result<String> {
        let checkout = Checkout::new(manifest)?;
        let graph = DependencyGraph::new(&checkout.metadata(target)?)?;
        let toolchain = Toolchain::new(&checkout.sysroot()?, target, &checkout.rustc_version()?);
        let text = Notices::new(&graph, package, &toolchain).render()?;
        let mut partial = output.as_os_str().to_owned();
        partial.push(".partial");
        let partial = std::path::PathBuf::from(partial);
        std::fs::write(&partial, &text)
            .map_err(|e| ToolError::io(format!("cannot write {}", partial.display()), &e))?;
        std::fs::rename(&partial, output)
            .map_err(|e| ToolError::io(format!("cannot rename {}", partial.display()), &e))?;
        Ok(text)
    }
}
