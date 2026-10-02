use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use symdev_sdk::{Result, SdkError};

/// A file on disk with its SHA-256 and size: a packed package, a source archive, or a
/// plain file such as install.sh.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Archive {
    pub path: PathBuf,
    pub sha256: String,
    pub size: u64,
}

impl Archive {
    /// An existing file with its hash and size, read from disk.
    pub fn hash(path: &Path) -> Result<Archive> {
        let io_error = |source| SdkError::Io {
            path: path.display().to_string(),
            source,
        };
        let mut file = BufReader::new(File::open(path).map_err(io_error)?);
        let mut hasher = Sha256::new();
        let size = io::copy(&mut file, &mut hasher).map_err(io_error)?;
        Ok(Archive {
            path: path.to_path_buf(),
            sha256: format!("{:x}", hasher.finalize()),
            size,
        })
    }
}
