//! Shared atomic file replace used by local-history, hot-exit, checkpoints,
//! and file-backed storage.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use super::StorageError;

/// True when `path` exists and is a symlink (broken links included).
pub(crate) fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

/// Reject `path` when it is a symlink.
pub(crate) fn reject_symlink(path: &Path, label: &str) -> Result<(), StorageError> {
    if is_symlink(path) {
        Err(StorageError::Failed {
            message: format!(
                "{label} path is a symlink and was rejected: {}",
                path.display()
            ),
        })
    } else {
        Ok(())
    }
}

/// Write `body` to `dest` via a sibling temp file, refusing to follow a
/// symlink at `dest` or its parent.
pub(crate) fn write_atomically_with_symlink_guard(
    dest: &Path,
    temp_prefix: &str,
    body: &[u8],
    label: &str,
) -> Result<(), StorageError> {
    reject_symlink(dest, label)?;
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    reject_symlink(parent, label)?;
    fs::create_dir_all(parent).map_err(|err| StorageError::Failed {
        message: format!("create {label} parent failed: {err}"),
    })?;
    let temp = parent.join(format!(
        ".{temp_prefix}-{}-{}.tmp",
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
                message: format!("create {label} temp failed: {err}"),
            })?;
        file.write_all(body).map_err(|err| StorageError::Failed {
            message: format!("write {label} temp failed: {err}"),
        })?;
        file.sync_all().map_err(|err| StorageError::Failed {
            message: format!("sync {label} temp failed: {err}"),
        })?;
        drop(file);
        atomic_replace(&temp, dest).map_err(|err| StorageError::Failed {
            message: format!("publish {label} file failed: {err}"),
        })
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    write_result
}

#[cfg(windows)]
pub(crate) fn atomic_replace(temp: &Path, target: &Path) -> std::io::Result<()> {
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
pub(crate) fn atomic_replace(temp: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(temp, target)
}
