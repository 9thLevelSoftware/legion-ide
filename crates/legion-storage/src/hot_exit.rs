//! Crash-safe unsaved-buffer snapshots, kept out of session JSON.
//!
//! Session metadata (`session.json`) stays free of buffer bodies. Dirty text
//! lives in a sibling `unsaved/` directory so a killed desktop can restore
//! edits without a proposal-mediated save having landed.
//!
//! Filenames are SHA-256 of the canonical path (not a 64-bit hasher). The
//! store refuses to follow a symlinked `.legion` or `unsaved` directory, and
//! each snapshot carries the pre-crash disk fingerprint so restore can refuse
//! to clobber an external edit.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

use legion_protocol::FileFingerprint;
use serde::{Deserialize, Serialize};

use super::StorageError;

/// Schema version for the unsaved-buffer manifest.
pub const HOT_EXIT_SCHEMA_VERSION: u32 = 2;

/// One dirty buffer captured for hot-exit restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotExitSnapshot {
    /// Canonical path of the file the buffer was editing.
    pub path: String,
    /// Editor buffer version at capture (informational; ids do not survive restart).
    pub buffer_version: u64,
    /// Unsaved UTF-8 body.
    pub body: String,
    /// Disk fingerprint observed at capture, used to detect external edits while down.
    pub disk_fingerprint: Option<FileFingerprint>,
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
    #[serde(default)]
    disk_fingerprint_algorithm: Option<String>,
    #[serde(default)]
    disk_fingerprint_value: Option<String>,
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
            return clear_hot_exit_dir(dir);
        }

        let dir = ensure_contained_hot_exit_dir(dir)?;

        let mut entries = Vec::with_capacity(snapshots.len());
        let mut keep = BTreeMap::new();
        keep.insert("manifest.json".to_string(), String::new());
        for snapshot in snapshots {
            let file = snapshot_file_name(&snapshot.path);
            if let Some(existing) = keep.insert(file.clone(), snapshot.path.clone())
                && existing != snapshot.path
            {
                return Err(StorageError::Failed {
                    message: format!(
                        "hot-exit filename collision between `{existing}` and `{}`",
                        snapshot.path
                    ),
                });
            }
            write_atomically(&dir.join(&file), snapshot.body.as_bytes())?;
            entries.push(HotExitManifestEntry {
                path: snapshot.path.clone(),
                buffer_version: snapshot.buffer_version,
                file,
                disk_fingerprint_algorithm: snapshot
                    .disk_fingerprint
                    .as_ref()
                    .map(|fingerprint| fingerprint.algorithm.clone()),
                disk_fingerprint_value: snapshot
                    .disk_fingerprint
                    .as_ref()
                    .map(|fingerprint| fingerprint.value.clone()),
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
        remove_orphans(&dir, &keep);
        Ok(())
    }

    /// Load snapshots. Missing directory is an empty list, not an error.
    pub fn load(dir: &Path) -> Result<Vec<HotExitSnapshot>, StorageError> {
        if is_symlink(dir) {
            return Err(symlink_error(dir));
        }
        if let Some(parent) = dir.parent()
            && is_symlink(parent)
        {
            return Err(symlink_error(parent));
        }
        let manifest_path = dir.join("manifest.json");
        if !manifest_path.is_file() {
            return Ok(Vec::new());
        }
        if is_symlink(&manifest_path) {
            return Err(symlink_error(&manifest_path));
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
            let body_path = contained_snapshot_path(dir, &entry.file)?;
            if is_symlink(&body_path) {
                return Err(symlink_error(&body_path));
            }
            let body = fs::read_to_string(&body_path).map_err(|err| StorageError::Failed {
                message: format!("read hot-exit snapshot `{}` failed: {err}", entry.file),
            })?;
            let disk_fingerprint = match (
                entry.disk_fingerprint_algorithm,
                entry.disk_fingerprint_value,
            ) {
                (Some(algorithm), Some(value)) if !algorithm.is_empty() && !value.is_empty() => {
                    Some(FileFingerprint { algorithm, value })
                }
                _ => None,
            };
            snapshots.push(HotExitSnapshot {
                path: entry.path,
                buffer_version: entry.buffer_version,
                body,
                disk_fingerprint,
            });
        }
        Ok(snapshots)
    }
}

fn snapshot_file_name(path: &str) -> String {
    let digest = super::sha256_digest(path.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("{hex}.txt")
}

fn contained_snapshot_path(dir: &Path, file: &str) -> Result<PathBuf, StorageError> {
    if file.is_empty()
        || file.contains('/')
        || file.contains('\\')
        || Path::new(file)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(StorageError::Failed {
            message: format!("hot-exit snapshot file `{file}` is not a contained name"),
        });
    }
    let path = dir.join(file);
    if path.parent() != Some(dir) {
        return Err(StorageError::Failed {
            message: format!("hot-exit snapshot file `{file}` escaped the store directory"),
        });
    }
    Ok(path)
}

fn ensure_contained_hot_exit_dir(dir: &Path) -> Result<PathBuf, StorageError> {
    if let Some(parent) = dir.parent() {
        reject_symlink(parent)?;
        if !parent.exists() {
            fs::create_dir(parent).map_err(|err| StorageError::Failed {
                message: format!("create hot-exit parent failed: {err}"),
            })?;
        }
        reject_symlink(parent)?;
        if !parent.is_dir() {
            return Err(StorageError::Failed {
                message: format!("hot-exit parent is not a directory: {}", parent.display()),
            });
        }
    }
    reject_symlink(dir)?;
    if dir.exists() {
        let metadata = fs::symlink_metadata(dir).map_err(|err| StorageError::Failed {
            message: format!("stat hot-exit directory failed: {err}"),
        })?;
        if !metadata.is_dir() {
            return Err(StorageError::Failed {
                message: format!("hot-exit path is not a directory: {}", dir.display()),
            });
        }
    } else {
        fs::create_dir(dir).map_err(|err| StorageError::Failed {
            message: format!("create hot-exit directory failed: {err}"),
        })?;
    }
    reject_symlink(dir)?;
    let canonical = fs::canonicalize(dir).map_err(|err| StorageError::Failed {
        message: format!("canonicalize hot-exit directory failed: {err}"),
    })?;
    reject_symlink(&canonical)?;
    if let Some(parent) = dir.parent() {
        let canonical_parent = fs::canonicalize(parent).map_err(|err| StorageError::Failed {
            message: format!("canonicalize hot-exit parent failed: {err}"),
        })?;
        if !canonical.starts_with(&canonical_parent) {
            return Err(StorageError::Failed {
                message: format!("hot-exit directory escaped parent: {}", canonical.display()),
            });
        }
    }
    Ok(canonical)
}

fn clear_hot_exit_dir(dir: &Path) -> Result<(), StorageError> {
    if !dir.exists() && !is_symlink(dir) {
        return Ok(());
    }
    reject_symlink(dir)?;
    if let Some(parent) = dir.parent() {
        reject_symlink(parent)?;
    }
    fs::remove_dir_all(dir).map_err(|err| StorageError::Failed {
        message: format!("clear hot-exit directory failed: {err}"),
    })
}

fn remove_orphans(dir: &Path, keep: &BTreeMap<String, String>) {
    if is_symlink(dir) {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if keep.contains_key(name) {
            continue;
        }
        let path = dir.join(name);
        if is_symlink(&path) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_file() {
            let _ = fs::remove_file(path);
        }
    }
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn reject_symlink(path: &Path) -> Result<(), StorageError> {
    if is_symlink(path) {
        Err(symlink_error(path))
    } else {
        Ok(())
    }
}

fn symlink_error(path: &Path) -> StorageError {
    StorageError::Failed {
        message: format!(
            "hot-exit path is a symlink and was rejected: {}",
            path.display()
        ),
    }
}

fn write_atomically(dest: &Path, body: &[u8]) -> Result<(), StorageError> {
    reject_symlink(dest)?;
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    reject_symlink(parent)?;
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

    fn snapshot(path: &str, body: &str) -> HotExitSnapshot {
        HotExitSnapshot {
            path: path.to_string(),
            buffer_version: 2,
            body: body.to_string(),
            disk_fingerprint: Some(FileFingerprint {
                algorithm: "test".to_string(),
                value: format!("fp:{path}"),
            }),
        }
    }

    #[test]
    fn round_trip_and_replace_snapshots() {
        let dir = temp_dir();
        let first = vec![snapshot("/ws/a.rs", "fn dirty() {}")];
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

    #[test]
    fn distinct_paths_use_distinct_snapshot_files() {
        let dir = temp_dir();
        let snapshots = vec![snapshot("/ws/a.rs", "alpha"), snapshot("/ws/b.rs", "beta")];
        HotExitStore::save(&dir, &snapshots).expect("save");
        let loaded = HotExitStore::load(&dir).expect("load");
        assert_eq!(loaded, snapshots);
        let names: Vec<_> = fs::read_dir(&dir)
            .expect("read")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name != "manifest.json")
            .collect();
        assert_eq!(names.len(), 2);
        assert_ne!(
            snapshot_file_name("/ws/a.rs"),
            snapshot_file_name("/ws/b.rs")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn save_rejects_symlinked_hot_exit_dir() {
        let tmp = temp_dir();
        let victim = tmp.join("victim");
        fs::create_dir_all(&victim).expect("victim");
        fs::write(victim.join("secret"), "keep-me").expect("secret");
        let dir = tmp.join("unsaved");
        std::os::unix::fs::symlink(&victim, &dir).expect("symlink");
        let result = HotExitStore::save(&dir, &[snapshot("/ws/a.rs", "nope")]);
        assert!(result.is_err(), "symlinked hot-exit dir must be rejected");
        assert_eq!(
            fs::read_to_string(victim.join("secret")).expect("secret survived"),
            "keep-me"
        );
        let _ = fs::remove_dir_all(&tmp);
    }

    #[cfg(unix)]
    #[test]
    fn remove_orphans_does_not_follow_symlinked_dir() {
        let tmp = temp_dir();
        let victim = tmp.join("victim");
        fs::create_dir_all(&victim).expect("victim");
        fs::write(victim.join("secret"), "keep-me").expect("secret");
        let dir = tmp.join("unsaved");
        std::os::unix::fs::symlink(&victim, &dir).expect("symlink");
        let result = HotExitStore::save(&dir, &[]);
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(victim.join("secret")).expect("secret survived"),
            "keep-me"
        );
        let _ = fs::remove_dir_all(&tmp);
    }
}
