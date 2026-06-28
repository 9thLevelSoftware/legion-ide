//! Metadata-only desktop session persistence.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use legion_protocol::WorkspaceSessionRecord;
use thiserror::Error;

/// Markers that indicate raw buffer/source payload leaked into a
/// raw-payload-carrying record field (currently only `memory_snapshot_json`).
///
/// These are inspected *only* against the known free-form payload field rather
/// than against the whole serialized document, so benign metadata (session ids,
/// file titles, panel labels, paths) that happens to contain a marker-like
/// substring is not falsely rejected.
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
        let record = serde_json::from_str::<WorkspaceSessionRecord>(&json).map_err(|source| {
            DesktopSessionError::Json {
                path: path.to_path_buf(),
                source,
            }
        })?;
        reject_raw_source_markers(&record)?;
        validate_record(&record)?;
        Ok(Some(record))
    }

    /// Save a workspace session record as metadata-only JSON.
    pub fn save(
        path: impl AsRef<Path>,
        record: &WorkspaceSessionRecord,
    ) -> Result<(), DesktopSessionError> {
        validate_record(record)?;
        reject_raw_source_markers(record)?;
        let path = path.as_ref();
        let json =
            serde_json::to_string_pretty(record).map_err(|source| DesktopSessionError::Json {
                path: path.to_path_buf(),
                source,
            })?;
        save_crash_safe(path, &json)
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

/// Inspect only the record's free-form raw-payload-carrying field for leaked
/// buffer/source markers. Restricting the scan to `memory_snapshot_json` (the
/// single field that can legitimately carry an opaque embedded JSON blob)
/// avoids false positives where benign metadata such as a `session_id`, file
/// title, or panel label happens to contain a marker-like substring.
fn reject_raw_source_markers(
    record: &WorkspaceSessionRecord,
) -> Result<(), DesktopSessionError> {
    let Some(memory_snapshot) = record.memory_snapshot_json.as_deref() else {
        return Ok(());
    };
    if let Some(marker) = RAW_SOURCE_MARKERS
        .iter()
        .find(|marker| memory_snapshot.contains(**marker))
    {
        return Err(DesktopSessionError::RawSourceMarker((*marker).to_string()));
    }
    Ok(())
}

fn save_crash_safe(path: &Path, json: &str) -> Result<(), DesktopSessionError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| DesktopSessionError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let temp_path = temporary_session_path(path, "tmp");
    let _ = fs::remove_file(&temp_path);

    write_and_verify_temp(path, &temp_path, json)?;
    publish_temp_session(path, &temp_path)
}

fn write_and_verify_temp(
    final_path: &Path,
    temp_path: &Path,
    json: &str,
) -> Result<(), DesktopSessionError> {
    {
        let mut temp = fs::File::create(temp_path).map_err(|source| DesktopSessionError::Io {
            path: temp_path.to_path_buf(),
            source,
        })?;
        temp.write_all(json.as_bytes())
            .and_then(|()| temp.sync_all())
            .map_err(|source| DesktopSessionError::Io {
                path: temp_path.to_path_buf(),
                source,
            })?;
    }

    let written = fs::read_to_string(temp_path).map_err(|source| DesktopSessionError::Io {
        path: temp_path.to_path_buf(),
        source,
    })?;
    let record = serde_json::from_str::<WorkspaceSessionRecord>(&written).map_err(|source| {
        DesktopSessionError::Json {
            path: final_path.to_path_buf(),
            source,
        }
    })?;
    reject_raw_source_markers(&record)?;
    validate_record(&record)?;
    Ok(())
}

fn publish_temp_session(final_path: &Path, temp_path: &Path) -> Result<(), DesktopSessionError> {
    replace_session_file(temp_path, final_path).map_err(|source| DesktopSessionError::Io {
        path: final_path.to_path_buf(),
        source,
    })?;
    sync_parent_dir(final_path).map_err(|source| DesktopSessionError::Io {
        path: final_path.to_path_buf(),
        source,
    })
}

/// Monotonic counter giving each in-process save a unique temp-file nonce so
/// two concurrent saves of the same destination never collide.
static TEMP_SESSION_NONCE: AtomicU64 = AtomicU64::new(0);

fn temporary_session_path(path: &Path, suffix: &str) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("session.json");
    let nonce = TEMP_SESSION_NONCE.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(
        ".{file_name}.{}.{nonce}.{suffix}",
        std::process::id()
    ))
}

#[cfg(windows)]
fn replace_session_file(temp_path: &Path, final_path: &Path) -> io::Result<()> {
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let existing = wide_path(temp_path);
    let new = wide_path(final_path);
    let ok = unsafe {
        MoveFileExW(
            existing.as_ptr(),
            new.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn wide_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(not(windows))]
fn replace_session_file(temp_path: &Path, final_path: &Path) -> io::Result<()> {
    fs::rename(temp_path, final_path)
}

#[cfg(windows)]
fn sync_parent_dir(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
fn sync_parent_dir(path: &Path) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::File::open(parent)?.sync_all()
}
