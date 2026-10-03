//! `SdkFreeTool`: `pkgtools sdk-free <sdk-dir> <path>...`.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::SdkFiles;

/// Exit 0 when no output holds a file of the SDK, 1 when one does (each on `err`), 2 when
/// the check cannot be made, so it never passes by looking at nothing.
pub struct SdkFreeTool;

impl SdkFreeTool {
    pub fn run(sdk: &Path, paths: &[PathBuf], out: &mut impl Write, err: &mut impl Write) -> u8 {
        let shown: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        let shown = shown.join(" ");
        let found = SdkFiles::of(sdk).and_then(|files| Ok((files.leaks(paths)?, files.len())));
        let (leaks, distinct) = match found {
            Ok(found) => found,
            Err(e) => {
                let _ = writeln!(err, "error: {e}");
                return 2;
            }
        };
        for leak in &leaks {
            let _ = writeln!(err, "error: {leak}");
        }
        if !leaks.is_empty() {
            let _ = writeln!(
                err,
                "error: {} file(s) of the S60 SDK in {shown}",
                leaks.len()
            );
            return 1;
        }
        let _ = writeln!(
            out,
            "no file of the SDK ({distinct} distinct files) in {shown}"
        );
        0
    }
}
