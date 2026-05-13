//! OS abstractions: keychain, filesystem, processes, window integration.

#![warn(missing_docs)]

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

/// Errors surfaced by platform file helpers.
#[derive(Debug, Error)]
pub enum PlatformError {
    /// I/O error with an associated path.
    #[error("I/O error for `{path}` while {operation}: {source}")]
    Io {
        /// Operation attempted.
        operation: String,
        /// Target file path.
        path: PathBuf,
        /// Underlying system error.
        #[source]
        source: io::Error,
    },
}

/// Opens a UTF-8 text file as-is.
///
/// Parent directories are not automatically created.
pub fn open_text_file(path: impl AsRef<Path>) -> Result<String, PlatformError> {
    let path = path.as_ref().to_path_buf();
    fs::read_to_string(&path).map_err(|source| PlatformError::Io {
        operation: "read".to_string(),
        path: path.clone(),
        source,
    })
}

/// Persists a UTF-8 text buffer to disk.
pub fn save_text_file(path: impl AsRef<Path>, text: impl AsRef<str>) -> Result<(), PlatformError> {
    let path = path.as_ref().to_path_buf();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| PlatformError::Io {
                operation: "create parent directories".to_string(),
                path: path.clone(),
                source,
            })?;
        }
    }

    fs::write(&path, text.as_ref()).map_err(|source| PlatformError::Io {
        operation: "write".to_string(),
        path,
        source,
    })
}

/// Returns a platform shell title for spike-time manual inspection.
pub const fn shell_title() -> &'static str {
    "Devil IDE — Native Shell + Text Model"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_file_roundtrip() {
        let tmp_dir = std::env::temp_dir();
        let path = tmp_dir.join("devil-spike-001a-platform-roundtrip.txt");

        let payload = "hello spike\n";
        save_text_file(&path, payload).expect("failed to save temporary spike file");
        let read = open_text_file(&path).expect("failed to read temporary spike file");
        assert_eq!(read, payload);

        let _ = fs::remove_file(path);
    }
}
