//! Metadata-only desktop session persistence.

use std::{
    fs,
    path::{Path, PathBuf},
};

use devil_protocol::WorkspaceSessionRecord;
use thiserror::Error;

const RAW_SOURCE_MARKERS: &[&str] = &[
    "small_buffer_preview",
    "source_body",
    "SECRET_DIRTY_BODY",
    "DIRTY_EDITED_BODY",
    "UNSAVED_DIRTY_BODY",
];

/// Desktop session persistence errors.
#[derive(Debug, Error)]
pub enum DesktopSessionError {
    /// Session file IO failed.
    #[error("session IO failed for {path}: {source}")]
    Io {
        /// Session path.
        path: PathBuf,
        /// Source IO error.
        source: std::io::Error,
    },
    /// Session JSON parse or serialization failed.
    #[error("session JSON failed for {path}: {source}")]
    Json {
        /// Session path.
        path: PathBuf,
        /// Source JSON error.
        source: serde_json::Error,
    },
    /// Session record failed metadata-only validation.
    #[error("invalid session record: {0}")]
    InvalidRecord(String),
    /// Serialized session appeared to contain raw source payload data.
    #[error("session JSON contains forbidden raw-source marker: {0}")]
    RawSourceMarker(String),
}

/// Metadata-only JSON store for desktop session records.
#[derive(Debug, Default, Clone, Copy)]
pub struct DesktopSessionStore;

impl DesktopSessionStore {
    /// Load a workspace session record from JSON. Missing files are a no-op.
    pub fn load(
        path: impl AsRef<Path>,
    ) -> Result<Option<WorkspaceSessionRecord>, DesktopSessionError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(None);
        }
        let json = fs::read_to_string(path).map_err(|source| DesktopSessionError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        reject_raw_source_markers(&json)?;
        let record = serde_json::from_str::<WorkspaceSessionRecord>(&json).map_err(|source| {
            DesktopSessionError::Json {
                path: path.to_path_buf(),
                source,
            }
        })?;
        validate_record(&record)?;
        Ok(Some(record))
    }

    /// Save a workspace session record as metadata-only JSON.
    pub fn save(
        path: impl AsRef<Path>,
        record: &WorkspaceSessionRecord,
    ) -> Result<(), DesktopSessionError> {
        validate_record(record)?;
        let path = path.as_ref();
        let json =
            serde_json::to_string_pretty(record).map_err(|source| DesktopSessionError::Json {
                path: path.to_path_buf(),
                source,
            })?;
        reject_raw_source_markers(&json)?;
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|source| DesktopSessionError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(path, json).map_err(|source| DesktopSessionError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    }
}

fn validate_record(record: &WorkspaceSessionRecord) -> Result<(), DesktopSessionError> {
    if record.schema_version == 0 {
        return Err(DesktopSessionError::InvalidRecord(
            "schema_version must be non-zero".to_string(),
        ));
    }
    if record.session_id.trim().is_empty() {
        return Err(DesktopSessionError::InvalidRecord(
            "session_id must be non-empty".to_string(),
        ));
    }
    Ok(())
}

fn reject_raw_source_markers(json: &str) -> Result<(), DesktopSessionError> {
    if let Some(marker) = RAW_SOURCE_MARKERS
        .iter()
        .find(|marker| json.contains(**marker))
    {
        return Err(DesktopSessionError::RawSourceMarker((*marker).to_string()));
    }
    Ok(())
}
