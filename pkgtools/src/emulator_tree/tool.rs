//! `EmulatorTreeTool`: `pkgtools emulator-tree <tree> --glibc <x.y>`.

use std::io::Write;
use std::path::Path;

use super::{EmulatorTree, GlibcVersion};

pub struct EmulatorTreeTool;

impl EmulatorTreeTool {
    /// Exit 0 when the tree has the layout and the recorded floor; 1 with the reason.
    pub fn run(tree: &Path, glibc: &str, out: &mut impl Write, err: &mut impl Write) -> u8 {
        let Some(recorded) = GlibcVersion::parse(glibc) else {
            let _ = writeln!(err, "error: --glibc {glibc} is not a version like 2.38");
            return 1;
        };
        let floor = EmulatorTree::at(tree).and_then(|t| t.glibc_floor());
        match floor {
            Ok(floor) if floor == recorded => {
                let _ = writeln!(out, "glibc floor {floor}");
                0
            }
            Ok(floor) => {
                let _ = writeln!(
                    err,
                    "error: the tree needs GLIBC_{floor}, artifact.toml records {recorded}: \
                     record what the tree needs; a floor above 2.38 goes to the owner first \
                     (plan finding F1)"
                );
                1
            }
            Err(e) => {
                let _ = writeln!(err, "error: {}: {e}", tree.display());
                1
            }
        }
    }
}
