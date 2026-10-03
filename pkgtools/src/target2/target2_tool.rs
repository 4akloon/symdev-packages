//! `Target2Tool`: `pkgtools target2-abs32 <object.o>...`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use symdev_elf2e32::Target2Rewrite;

/// Rewrites each object in turn and prints how many relocations it changed; an object
/// it refuses (anything but an ELF32 little-endian ARM ET_REL) stops the run with exit 1,
/// that file left as it was. A file without TARGET2 relocations is not written.
pub struct Target2Tool;

impl Target2Tool {
    pub fn run(objects: &[PathBuf], out: &mut impl Write, err: &mut impl Write) -> u8 {
        for path in objects {
            let rewritten = fs::read(path)
                .map_err(|e| e.to_string())
                .and_then(|data| Target2Rewrite::object(&data).map_err(|e| e.to_string()));
            let rewritten = match rewritten {
                Ok(rewritten) => rewritten,
                Err(e) => {
                    let _ = writeln!(err, "error: {}: {e}", path.display());
                    return 1;
                }
            };
            if rewritten.rewritten() > 0
                && let Err(e) = Self::replace(path, rewritten.bytes())
            {
                let _ = writeln!(err, "error: {}: cannot write it back: {e}", path.display());
                return 1;
            }
            let count = rewritten.rewritten();
            let _ = writeln!(
                out,
                "{}: {count} R_ARM_TARGET2 -> R_ARM_ABS32",
                path.display()
            );
        }
        0
    }

    /// Writes `<path>.partial` and renames it over `path`, so a failed write never leaves
    /// half an object.
    fn replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        let mut partial = path.as_os_str().to_owned();
        partial.push(".partial");
        fs::write(&partial, bytes)?;
        fs::rename(&partial, path)
    }
}
