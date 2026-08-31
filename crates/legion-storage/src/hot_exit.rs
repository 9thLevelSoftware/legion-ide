//! Crash-safe unsaved-buffer snapshots, kept out of session JSON.
//!
//! Session metadata (`session.json`) stays free of buffer bodies. Dirty text
//! lives in a sibling `unsaved/` directory so a killed desktop can restore
//! edits without a proposal-mediated save having landed.

use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    hash::{Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::StorageError;

/// Schema version for the unsaved-buffer manifest.
pub const HOT_EXIT_SCHEMA_VERSION: u32 = 1;

/// One dirty buffer captured for hot-exit restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotExitSnapshot {
    /// Canonical path of the file the buffer was editing.
    pub path: String,
    /// Editor buffer version at capture (informational; ids do not survive restart).
    pub buffer_version: u64,
    /// Unsaved UTF-8 body.
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HotExitManifest {
    schema_version: u32,
    entries: Vec<HotExitManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HotExitManifestEntry {
    path: String,
    buffer_version: u64,
    file: String,
}

/// Filesystem store for [`HotExitSnapshot`] values.
pub struct HotExitStore;

impl HotExitStore {
    /// Directory that holds unsaved snapshots for a session JSON path.
    #[must_use]
    pub fn dir_for_session(session_json: &Path) -> PathBuf {
        session_json
            .parent()
            .map(|parent| parent.join("unsaved"))
            .unwrap_or_else(|| PathBuf::from("unsaved"))
    }

    /// Replace the store with `snapshots` (empty list clears the directory).
    pub fn save(dir: &Path, snapshots: &[HotExitSnapshot]) -> Result<(), StorageError> {
        if snapshots.is_empty() {
            if dir.exists() {
                fs::remove_dir_all(dir).map_err(|err| StorageError::Failed {
                    message: format!("clear hot-exit directory failed: {err}"),
                })?;
            }
            return Ok(());
        }

        fs::create_dir_all(dir).map_err(|err| StorageError::Failed {
            message: format!("create hot-exit directory failed: {err}"),
        })?;

        let mut entries = Vec::with_capacity(snapshots.len());
        let mut keep = HashSet::new();
        keep.insert("manifest.json".to_string());
        for snapshot in snapshots {
            let file = snapshot_file_name(&snapshot.path);
            keep.insert(file.clone());
            write_atomically(&dir.join(&file), snapshot.body.as_bytes())?;
            entries.push(HotExitManifestEntry {
                path: snapshot.path.clone(),
                buffer_version: snapshot.buffer_version,
                file,
            });
        }

        let manifest = HotExitManifest {
            schema_version: HOT_EXIT_SCHEMA_VERSION,
            entries,
        };
        let encoded = serde_json::to_vec_pretty(&manifest).map_err(|err| StorageError::Failed {
            message: format!("serialize hot-exit manifest failed: {err}"),
        })?;
        write_atomically(&dir.join("manifest.json"), &encoded)?;
        remove_orphans(dir, &keep);
        Ok(())
    }

    /// Load snapshots. Missing directory is an empty list, not an error.
    pub fn load(dir: &Path) -> Result<Vec<HotExitSnapshot>, StorageError> {
        let manifest_path = dir.join("manifest.json");
        if !manifest_path.is_file() {
            return Ok(Vec::new());
        }
        let encoded = fs::read(&manifest_path).map_err(|err| StorageError::Failed {
            message: format!("read hot-exit manifest failed: {err}"),
        })?;
        let manifest: HotExitManifest =
            serde_json::from_slice(&encoded).map_err(|err| StorageError::Failed {
                message: format!("parse hot-exit manifest failed: {err}"),
            })?;
        let mut snapshots = Vec::new();
        for entry in manifest.entries {
            let body =
                fs::read_to_string(dir.join(&entry.file)).map_err(|err| StorageError::Failed {
                    message: format!("read hot-exit snapshot `{}` failed: {err}", entry.file),
                })?;
            snapshots.push(HotExitSnapshot {
                path: entry.path,
                buffer_version: entry.buffer_version,
                body,
            });
        }
        Ok(snapshots)
    }
}

fn snapshot_file_name(path: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    format!("{:016x}.txt", hasher.finish())
}

fn remove_orphans(dir: &Path, keep: &HashSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if keep.contains(name) {
            continue;
        }
        let _ = fs::remove_file(entry.path());
    }
}

fn write_atomically(dest: &Path, body: &[u8]) -> Result<(), StorageError> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|err| StorageError::Failed {
        message: format!("create hot-exit parent failed: {err}"),
    })?;
    let temp = parent.join(format!(
        ".hot-exit-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let write_result = (|| -> Result<(), StorageError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|err| StorageError::Failed {
                message: format!("create hot-exit temp failed: {err}"),
            })?;
        file.write_all(body).map_err(|err| StorageError::Failed {
            message: format!("write hot-exit temp failed: {err}"),
        })?;
        file.sync_all().map_err(|err| StorageError::Failed {
            message: format!("sync hot-exit temp failed: {err}"),
        })?;
        drop(file);
        atomic_replace(&temp, dest).map_err(|err| StorageError::Failed {
            message: format!("publish hot-exit file failed: {err}"),
        })
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    write_result
}

#[cfg(windows)]
fn atomic_replace(temp: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new_name: *const u16, flags: u32) -> i32;
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    let ok = unsafe {
        MoveFileExW(
            wide(temp).as_ptr(),
            wide(target).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(temp: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(temp, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("legion-hot-exit-{}-{}", std::process::id(), nanos));
        fs::create_dir_all(&dir).expect("temp");
        dir
    }

    #[test]
    fn round_trip_and_replace_snapshots() {
        let dir = temp_dir();
        let first = vec![HotExitSnapshot {
            path: "/ws/a.rs".to_string(),
            buffer_version: 2,
            body: "fn dirty() {}".to_string(),
        }];
        HotExitStore::save(&dir, &first).expect("save");
        let loaded = HotExitStore::load(&dir).expect("load");
        assert_eq!(loaded, first);

        HotExitStore::save(&dir, &[]).expect("clear");
        assert!(HotExitStore::load(&dir).expect("load empty").is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_directory_loads_empty() {
        let dir = temp_dir().join("does-not-exist");
        assert!(HotExitStore::load(&dir).expect("load").is_empty());
    }
}
