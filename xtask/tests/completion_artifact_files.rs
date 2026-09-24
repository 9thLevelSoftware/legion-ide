use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use xtask::completion::artifact_files::{
    ArtifactInvalidReason, ArtifactValidation, parse_utc_timestamp, validate_declared_artifact,
};

fn temp_tree() -> (PathBuf, PathBuf) {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "legion-artifact-files-{suffix}-{}",
        std::process::id()
    ));
    let evidence = root.join("plans/evidence/completion");
    fs::create_dir_all(&evidence).unwrap();
    (root, evidence)
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn check(root: &Path, evidence: &Path, path: &str, hash: &str) -> ArtifactValidation {
    validate_declared_artifact(root, evidence, path, hash).unwrap()
}

#[test]
fn validates_nested_and_empty_files_and_rejects_changed_bytes() {
    let (root, evidence) = temp_tree();
    fs::create_dir_all(evidence.join("nested")).unwrap();
    fs::write(evidence.join("nested/result.txt"), b"evidence").unwrap();
    fs::write(evidence.join("empty"), b"").unwrap();
    assert_eq!(
        check(
            &root,
            &evidence,
            "plans/evidence/completion/nested/result.txt",
            &digest(b"evidence")
        ),
        ArtifactValidation::Valid
    );
    assert_eq!(
        check(
            &root,
            &evidence,
            "plans/evidence/completion/empty",
            &digest(b"")
        ),
        ArtifactValidation::Valid
    );
    assert_eq!(
        check(
            &root,
            &evidence,
            "plans/evidence/completion/nested/result.txt",
            &digest(b"altered")
        ),
        ArtifactValidation::Invalid(ArtifactInvalidReason::HashMismatch)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_bad_hash_paths_missing_files_and_directories() {
    let (root, evidence) = temp_tree();
    fs::create_dir_all(evidence.join("nested")).unwrap();
    assert!(matches!(
        check(
            &root,
            &evidence,
            "plans/evidence/completion/missing",
            &digest(b"")
        ),
        ArtifactValidation::Invalid(ArtifactInvalidReason::Missing)
    ));
    assert!(matches!(
        check(
            &root,
            &evidence,
            "plans/evidence/completion/nested",
            &digest(b"")
        ),
        ArtifactValidation::Invalid(ArtifactInvalidReason::Directory)
    ));
    fs::write(evidence.join("uppercase"), b"uppercase").unwrap();
    assert_eq!(
        check(
            &root,
            &evidence,
            "plans/evidence/completion/uppercase",
            &digest(b"uppercase").to_uppercase()
        ),
        ArtifactValidation::Valid
    );
    let empty_hash = digest(b"");
    for (path, hash) in [
        ("/absolute", empty_hash.as_str()),
        ("../escape", empty_hash.as_str()),
        ("C:relative", empty_hash.as_str()),
        (r"\\server\share", empty_hash.as_str()),
        (r"\\.\device", empty_hash.as_str()),
        ("file:stream", empty_hash.as_str()),
        ("file", "ABC"),
    ] {
        assert!(matches!(
            check(&root, &evidence, path, hash),
            ArtifactValidation::Invalid(
                ArtifactInvalidReason::InvalidSha256 | ArtifactInvalidReason::InvalidPath(_)
            )
        ));
    }
    let outside =
        std::env::temp_dir().join(format!("legion-artifact-outside-{}", std::process::id()));
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("outside"), b"outside").unwrap();
    assert!(matches!(
        check(&root, &outside, "outside", &digest(b"outside")),
        ArtifactValidation::Invalid(ArtifactInvalidReason::OutsideEvidenceSubtree)
    ));
    let _ = fs::remove_dir_all(outside);
    let direct_outside = root.join("outside-file");
    fs::write(&direct_outside, b"outside-file").unwrap();
    assert!(matches!(
        check(&root, &evidence, "outside-file", &digest(b"outside-file")),
        ArtifactValidation::Invalid(ArtifactInvalidReason::OutsideEvidenceSubtree)
    ));
    let evidence_file = root.join("evidence-file");
    fs::write(&evidence_file, b"file").unwrap();
    assert!(matches!(
        check(
            &root,
            &evidence_file,
            "plans/evidence/completion/nested",
            &digest(b"")
        ),
        ArtifactValidation::Invalid(ArtifactInvalidReason::EvidenceRootNotDirectory)
    ));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn parses_strict_utc_and_orders_chronologically() {
    assert!(parse_utc_timestamp("2000-02-29T23:59:59Z").is_ok());
    assert!(parse_utc_timestamp("1900-02-29T00:00:00Z").is_err());
    for value in [
        "2024-04-31T00:00:00Z",
        "2024-01-00T00:00:00Z",
        "2024-01-32T00:00:00Z",
        "2024-02-30T00:00:00Z",
        "2024-13-01T00:00:00Z",
        "2024-00-01T00:00:00Z",
        "2024-01-01T24:00:00Z",
        "2024-01-01T00:60:00Z",
        "2024-01-01T00:00:60Z",
        "0000-01-01T00:00:00Z",
        "2024-1-01T00:00:00Z",
        "2024-01-01T00:00:00+00:00",
        "2024-01-01T00:00:00.000Z",
        " 2024-01-01T00:00:00Z",
        "2024-01-01T00:00:00Z\n",
        "２０２４-01-01T00:00:00Z",
    ] {
        assert!(parse_utc_timestamp(value).is_err(), "accepted {value}");
    }
    assert!(parse_utc_timestamp("2024-01-31T00:00:00Z").is_ok());
    assert!(parse_utc_timestamp("2024-02-01T00:00:00Z").is_ok());
    assert!(
        parse_utc_timestamp("1999-12-31T23:59:59Z").unwrap()
            < parse_utc_timestamp("2000-01-01T00:00:00Z").unwrap()
    );
    assert!(
        parse_utc_timestamp("2024-02-28T23:59:59Z").unwrap()
            < parse_utc_timestamp("2024-03-01T00:00:00Z").unwrap()
    );
}

#[cfg(unix)]
#[test]
fn validates_internal_symlink_and_rejects_escape() {
    use std::os::unix::fs::symlink;
    let (root, evidence) = temp_tree();
    fs::write(evidence.join("inside"), b"inside").unwrap();
    let outside = root.join("outside");
    fs::write(&outside, b"outside").unwrap();
    symlink("inside", evidence.join("internal-link")).unwrap();
    symlink(&outside, evidence.join("external-link")).unwrap();
    assert_eq!(
        check(
            &root,
            &evidence,
            "plans/evidence/completion/internal-link",
            &digest(b"inside")
        ),
        ArtifactValidation::Valid
    );
    assert!(matches!(
        check(
            &root,
            &evidence,
            "plans/evidence/completion/external-link",
            &digest(b"outside")
        ),
        ArtifactValidation::Invalid(ArtifactInvalidReason::OutsideEvidenceSubtree)
    ));
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn validates_windows_symlink_when_creation_is_available() {
    use std::os::windows::fs::symlink_file;
    let (root, evidence) = temp_tree();
    fs::write(evidence.join("inside"), b"inside").unwrap();
    let outside = root.join("outside-file");
    fs::write(&outside, b"outside").unwrap();
    let link = evidence.join("internal-link");
    if let Err(error) = symlink_file("inside", &link) {
        assert!(
            error.kind() == std::io::ErrorKind::PermissionDenied
                || error.raw_os_error() == Some(1314),
            "unexpected Windows symlink creation error: {error}"
        );
        eprintln!("SKIPPED Windows symlink coverage: {error}");
        let _ = fs::remove_dir_all(root);
        return;
    }
    let external_link = evidence.join("external-link");
    if let Err(error) = symlink_file(&outside, &external_link) {
        assert!(
            error.kind() == std::io::ErrorKind::PermissionDenied
                || error.raw_os_error() == Some(1314),
            "unexpected Windows symlink escape creation error: {error}"
        );
        eprintln!("SKIPPED Windows symlink escape coverage: {error}");
        let _ = fs::remove_dir_all(root);
        return;
    }
    assert_eq!(
        check(
            &root,
            &evidence,
            "plans/evidence/completion/internal-link",
            &digest(b"inside")
        ),
        ArtifactValidation::Valid
    );
    assert!(matches!(
        check(
            &root,
            &evidence,
            "plans/evidence/completion/external-link",
            &digest(b"outside")
        ),
        ArtifactValidation::Invalid(ArtifactInvalidReason::OutsideEvidenceSubtree)
    ));
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
fn create_junction(link: &Path, target: &Path) -> std::io::Result<()> {
    let status = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:LEGION_ARTIFACT_JUNCTION_LINK -Target $env:LEGION_ARTIFACT_JUNCTION_TARGET | Out-Null",
        ])
        .env("LEGION_ARTIFACT_JUNCTION_LINK", link)
        .env("LEGION_ARTIFACT_JUNCTION_TARGET", target)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "junction command exited with {status}"
        )))
    }
}

#[cfg(windows)]
#[test]
fn validates_internal_junction_and_rejects_external_junction() {
    let (root, evidence) = temp_tree();
    let inside_dir = evidence.join("inside-dir");
    let outside_dir = root.join("outside-dir");
    fs::create_dir_all(&inside_dir).unwrap();
    fs::create_dir_all(&outside_dir).unwrap();
    fs::write(inside_dir.join("inside.txt"), b"inside-junction").unwrap();
    fs::write(outside_dir.join("outside.txt"), b"outside-junction").unwrap();

    let internal_link = evidence.join("internal-junction");
    if let Err(error) = create_junction(&internal_link, &inside_dir) {
        eprintln!("SKIPPED Windows junction coverage: {error}");
        let _ = fs::remove_dir_all(root);
        return;
    }
    let external_link = evidence.join("external-junction");
    if let Err(error) = create_junction(&external_link, &outside_dir) {
        eprintln!("SKIPPED Windows junction escape coverage: {error}");
        let _ = fs::remove_dir_all(root);
        return;
    }

    assert_eq!(
        check(
            &root,
            &evidence,
            "plans/evidence/completion/internal-junction/inside.txt",
            &digest(b"inside-junction")
        ),
        ArtifactValidation::Valid
    );
    assert!(matches!(
        check(
            &root,
            &evidence,
            "plans/evidence/completion/external-junction/outside.txt",
            &digest(b"outside-junction")
        ),
        ArtifactValidation::Invalid(ArtifactInvalidReason::OutsideEvidenceSubtree)
    ));
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn reports_read_failure_when_permissions_are_enforced() {
    use std::os::unix::fs::PermissionsExt;
    let (root, evidence) = temp_tree();
    let file = evidence.join("unreadable");
    fs::write(&file, b"secret").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
    let result = validate_declared_artifact(
        &root,
        &evidence,
        "plans/evidence/completion/unreadable",
        &digest(b"secret"),
    );
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    if let Err(error) = result {
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    } else {
        eprintln!("SKIPPED read-failure coverage: current user bypasses mode permissions");
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_evidence_root_is_invalid() {
    let root = std::env::temp_dir().join(format!("legion-missing-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let result = check(&root, Path::new("missing-evidence"), "file", &digest(b""));
    assert!(matches!(
        result,
        ArtifactValidation::Invalid(ArtifactInvalidReason::Missing)
    ));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_repository_root_is_operational_error() {
    let root = std::env::temp_dir().join(format!(
        "legion-missing-repository-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let error = validate_declared_artifact(
        &root,
        Path::new("plans/evidence/completion"),
        "plans/evidence/completion/file",
        &digest(b""),
    )
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}
