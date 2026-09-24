//! Bounded validation primitives for completion evidence files and UTC times.
//!
//! These helpers validate a quiescent evidence checkout. They do not provide
//! race-proof file opening, signature authenticity, or independent observation
//! of a running identity.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path};

use sha2::{Digest, Sha256};

/// A Gregorian timestamp in the restricted completion-record UTC format.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct UtcTimestamp {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// Parse exactly `YYYY-MM-DDTHH:MM:SSZ` and reject all other timestamp syntax.
pub fn parse_utc_timestamp(input: &str) -> Result<UtcTimestamp, String> {
    let bytes = input.as_bytes();
    if bytes.len() != 20 || !bytes.is_ascii() {
        return Err(
            "timestamp must be exactly 20 ASCII bytes in YYYY-MM-DDTHH:MM:SSZ format".into(),
        );
    }
    for (index, expected) in [
        (4, b'-'),
        (7, b'-'),
        (10, b'T'),
        (13, b':'),
        (16, b':'),
        (19, b'Z'),
    ] {
        if bytes[index] != expected {
            return Err(format!("timestamp has invalid separator at byte {index}"));
        }
    }
    for index in [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18] {
        if !bytes[index].is_ascii_digit() {
            return Err(format!("timestamp has non-digit at byte {index}"));
        }
    }
    let number = |start: usize, end: usize| -> u16 {
        bytes[start..end]
            .iter()
            .fold(0, |value, byte| value * 10 + u16::from(byte - b'0'))
    };
    let year = number(0, 4);
    let month = number(5, 7) as u8;
    let day = number(8, 10) as u8;
    let hour = number(11, 13) as u8;
    let minute = number(14, 16) as u8;
    let second = number(17, 19) as u8;
    if year == 0 || month == 0 || month > 12 {
        return Err("timestamp has an invalid Gregorian year or month".into());
    }
    let days = match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day == 0 || day > days {
        return Err("timestamp has an invalid Gregorian day".into());
    }
    if hour > 23 || minute > 59 || second > 59 {
        return Err("timestamp has an invalid time of day".into());
    }
    Ok(UtcTimestamp {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
}

fn is_leap_year(year: u16) -> bool {
    year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100))
}

/// Reasons a declared artifact is invalid. Filesystem access failures are
/// returned separately as `io::Error` so callers can distinguish operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactInvalidReason {
    InvalidSha256,
    InvalidPath(String),
    Missing,
    EvidenceRootNotDirectory,
    Directory,
    OutsideEvidenceSubtree,
    HashMismatch,
}

/// Result of validating one declared evidence artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactValidation {
    Valid,
    Invalid(ArtifactInvalidReason),
}

/// Validate a repository-relative artifact and stream its SHA-256 digest.
///
/// `evidence_subtree` may be absolute or repository-relative for isolated
/// tests; production composition supplies the canonical `plans/evidence/completion`
/// subtree. The checkout is assumed quiescent for the duration of validation.
pub fn validate_declared_artifact(
    repository_root: &Path,
    evidence_subtree: &Path,
    artifact_path: &str,
    declared_sha256: &str,
) -> io::Result<ArtifactValidation> {
    if !is_sha256(declared_sha256) {
        return Ok(ArtifactValidation::Invalid(
            ArtifactInvalidReason::InvalidSha256,
        ));
    }
    if let Some(reason) = reject_artifact_path(artifact_path) {
        return Ok(ArtifactValidation::Invalid(
            ArtifactInvalidReason::InvalidPath(reason),
        ));
    }

    let repository = fs::canonicalize(repository_root)?;
    let evidence_input = if evidence_subtree.is_absolute() {
        evidence_subtree.to_path_buf()
    } else {
        repository.join(evidence_subtree)
    };
    let evidence = match fs::canonicalize(&evidence_input) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(ArtifactValidation::Invalid(ArtifactInvalidReason::Missing));
        }
        Err(error) => return Err(error),
    };
    // Path::starts_with is component-wise: `/srv/evidence-other` is not under `/srv/evidence`.
    if !evidence.starts_with(&repository) {
        return Ok(ArtifactValidation::Invalid(
            ArtifactInvalidReason::OutsideEvidenceSubtree,
        ));
    }
    if !fs::metadata(&evidence)?.is_dir() {
        return Ok(ArtifactValidation::Invalid(
            ArtifactInvalidReason::EvidenceRootNotDirectory,
        ));
    }
    let candidate = repository.join(artifact_path);
    let canonical = match fs::canonicalize(&candidate) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(ArtifactValidation::Invalid(ArtifactInvalidReason::Missing));
        }
        Err(error) => return Err(error),
    };
    // Path::starts_with is component-wise; a sibling string-prefix is not a child.
    if !canonical.starts_with(&evidence) {
        return Ok(ArtifactValidation::Invalid(
            ArtifactInvalidReason::OutsideEvidenceSubtree,
        ));
    }
    let metadata = fs::metadata(&canonical)?;
    if !metadata.is_file() {
        return Ok(ArtifactValidation::Invalid(
            ArtifactInvalidReason::Directory,
        ));
    }
    let actual = digest_file(&canonical)?;
    if actual.eq_ignore_ascii_case(declared_sha256) {
        Ok(ArtifactValidation::Valid)
    } else {
        Ok(ArtifactValidation::Invalid(
            ArtifactInvalidReason::HashMismatch,
        ))
    }
}

fn is_sha256(value: &str) -> bool {
    crate::completion::hash::is_sha256(value)
}

fn reject_artifact_path(value: &str) -> Option<String> {
    if value.is_empty() || value.starts_with('/') || value.starts_with('\\') || value.contains(':')
    {
        return Some("absolute, drive, device, UNC, ADS, or empty path".into());
    }
    let path = Path::new(value);
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
        || value.split(['/', '\\']).any(|part| part == "..")
    {
        return Some("parent traversal is not allowed".into());
    }
    None
}

fn digest_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn sibling_directory_with_shared_string_prefix_is_outside() {
        assert!(!Path::new("/srv/evidence-other").starts_with(Path::new("/srv/evidence")));
        assert!(Path::new("/srv/evidence/nested").starts_with(Path::new("/srv/evidence")));
    }
}
