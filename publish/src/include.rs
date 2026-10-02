use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use symdev_sdk::{Result, SdkError};

/// One entry of a recipe's `include` list: a path relative to the tree being packed, whose
/// last segment may hold `*` (any run of characters), e.g. `epoc32/release/armv5/lib/*.dso`.
/// A directory it names is taken whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Include {
    pattern: String,
}

impl Include {
    pub fn parse(pattern: &str) -> Result<Include> {
        match Self::problem(pattern) {
            Some(reason) => Err(SdkError::Other(format!(
                "include `{pattern}` {reason}; list paths relative to the packed tree, with `*` \
                 only in the last segment"
            ))),
            None => Ok(Include {
                pattern: pattern.to_string(),
            }),
        }
    }

    fn problem(pattern: &str) -> Option<&'static str> {
        let segments: Vec<&str> = pattern.split('/').collect();
        if pattern.is_empty() {
            Some("is empty")
        } else if pattern.starts_with('/') {
            Some("is absolute")
        } else if pattern.contains('\\') || pattern.chars().any(char::is_control) {
            Some("has a `\\` or a control character")
        } else if segments
            .iter()
            .any(|s| s.is_empty() || *s == "." || *s == "..")
        {
            Some("has an empty, `.` or `..` segment")
        } else if segments[..segments.len() - 1]
            .iter()
            .any(|s| s.contains('*'))
        {
            Some("has a `*` before its last segment")
        } else {
            None
        }
    }

    /// Copies what the pattern names in `from` to the same relative paths under `into`
    /// (byte copies). Matching nothing is an error naming the pattern.
    pub fn copy(&self, from: &Path, into: &Path) -> Result<()> {
        let (dir, name) = self.pattern.rsplit_once('/').unwrap_or(("", &self.pattern));
        let mut matched = Vec::new();
        if name.contains('*') {
            let full = from.join(dir);
            match fs::read_dir(&full) {
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(source) => return Err(io_error(&full, source)),
                Ok(entries) => {
                    for entry in entries {
                        let file_name = entry.map_err(|e| io_error(&full, e))?.file_name();
                        if file_name.to_str().is_some_and(|n| glob(name, n)) {
                            matched.push(Path::new(dir).join(file_name));
                        }
                    }
                }
            }
        } else {
            let full = from.join(&self.pattern);
            match fs::symlink_metadata(&full) {
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(source) => return Err(io_error(&full, source)),
                Ok(_) => matched.push(Path::new(&self.pattern).to_path_buf()),
            }
        }
        if matched.is_empty() {
            return Err(SdkError::Other(format!(
                "include `{}` matches nothing in {}; fix the recipe or the --from directory",
                self.pattern,
                from.display()
            )));
        }
        matched
            .iter()
            .try_for_each(|rel| Self::copy_entry(from, into, rel))
    }

    fn copy_entry(from: &Path, into: &Path, rel: &Path) -> Result<()> {
        let source = from.join(rel);
        let dest = into.join(rel);
        let meta = fs::symlink_metadata(&source).map_err(|e| io_error(&source, e))?;
        if meta.is_dir() {
            fs::create_dir_all(&dest).map_err(|e| io_error(&dest, e))?;
            for child in fs::read_dir(&source).map_err(|e| io_error(&source, e))? {
                let child = child.map_err(|e| io_error(&source, e))?;
                Self::copy_entry(from, into, &rel.join(child.file_name()))?;
            }
            Ok(())
        } else if meta.is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
            }
            fs::copy(&source, &dest).map_err(|e| io_error(&source, e))?;
            Ok(())
        } else {
            Err(SdkError::Other(format!(
                "cannot take {}: only files and directories are packed from an include list \
                 (a symlink there was never observed)",
                source.display()
            )))
        }
    }
}

/// Whether `name` matches `pattern`, where `*` stands for any run of characters.
fn glob(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}

fn io_error(path: &Path, source: std::io::Error) -> SdkError {
    SdkError::Io {
        path: path.display().to_string(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::glob;

    #[test]
    fn a_star_matches_any_run_of_characters() {
        assert!(glob("*.dso", "euser.dso"));
        assert!(glob("*.dso", ".dso"));
        assert!(!glob("*.dso", "euser.lib"));
        assert!(!glob("*.dso", "euser.dso.bak"));
        assert!(glob("e*r*.dso", "euser.dso"));
        assert!(!glob("e*z*.dso", "euser.dso"));
        assert!(glob("a*a", "aa"));
        assert!(!glob("a*a", "a"));
        assert!(glob("usrt2_2.lib", "usrt2_2.lib"));
        assert!(!glob("usrt2_2.lib", "usrt2_2.lib2"));
    }
}
