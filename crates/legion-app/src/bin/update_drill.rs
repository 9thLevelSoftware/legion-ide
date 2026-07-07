//! Update drill — deterministic 19th standing gate (M12 / PKT-UPDATER).
//!
//! Invoked by `cargo run -p xtask -- update-drill` (subprocess model — xtask
//! cannot depend on legion-app, so this binary is spawned and its exit code +
//! report TOML are read back).
//!
//! # What it does
//!
//! 1. Generates an ephemeral Ed25519 keypair (in-memory, never persisted).
//! 2. Fabricates two artifact versions (`v0.1.0` and `v0.2.0`) as temp files.
//! 3. Builds a `ReleaseManifestV1` for each with real SHA-256 hashes.
//! 4. Signs the `v0.2.0` manifest using the ephemeral key.
//! 5. Exercises the real `Updater`:
//!    - `check_for_update` (v0.1.0 → v0.2.0) → expects `Available`.
//!    - `stage_update` → SHA-256 verified, artifact copied.
//!    - `apply_update` → journal written.
//!    - `rollback` → journal reverted (toggle 1).
//!    - double rollback → journal back to updated state (idempotent toggle 2).
//! 6. Negative cases (all must fail closed):
//!    - Bad signature → `SignatureInvalid`.
//!    - Bad artifact hash → `HashMismatch`.
//!    - Cross-channel (preview manifest, stable policy) → `ChannelMismatch`.
//!    - Downgrade (v0.2.0 → v0.1.0 manifest) → `NoUpdate`.
//! 7. Writes report to `<out_dir>/update_drill_report.toml`.
//!
//! # Constraints
//!
//! - Zero egress: all operations are local; no HTTP.
//! - Binary swap / restart: explicitly out of scope (ADR-0042 D5).
//! - Temp directories are cleaned on success, left for inspection on failure.

use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use legion_app::updater::{
    LocalDirManifestSource, UpdateCheck, UpdateError, UpdatePolicy, Updater,
};
use legion_protocol::{ReleaseArtifact, ReleaseManifestV1};

// ─────────────────────────────────────────────────────────────────────────────
// Step status
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum StepStatus {
    Passed,
    Failed,
    #[allow(dead_code)]
    Skipped,
}

impl StepStatus {
    fn as_str(&self) -> &'static str {
        match self {
            StepStatus::Passed => "passed",
            StepStatus::Failed => "failed",
            StepStatus::Skipped => "skipped",
        }
    }
}

struct StepRecord {
    id: &'static str,
    started_utc: String,
    finished_utc: String,
    duration_ms: u128,
    status: StepStatus,
    detail: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Convert Unix epoch seconds to a compact RFC 3339 UTC string.
fn epoch_secs_to_rfc3339(secs: u64) -> String {
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (year, month, day) = days_to_ymd(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn days_to_ymd(days: i64) -> (u32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mon = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe as i64 + era * 400;
    let y = if mon <= 2 { y + 1 } else { y };
    (y as u32, mon as u32, d as u32)
}

fn utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    epoch_secs_to_rfc3339(secs)
}

fn run_timer<F, T>(f: F) -> (T, u128)
where
    F: FnOnce() -> T,
{
    let start = Instant::now();
    let result = f();
    (result, start.elapsed().as_millis())
}

/// Compute SHA-256 hex digest of `data`.
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(data))
}

// ─────────────────────────────────────────────────────────────────────────────
// Ephemeral keypair
// ─────────────────────────────────────────────────────────────────────────────

/// Generate an ephemeral Ed25519 seed from system entropy (time + PID).
///
/// The seed is never persisted — it lives only in this process for the
/// duration of the drill run.
fn make_ephemeral_seed() -> [u8; 32] {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = ts.as_secs();
    let nanos = ts.subsec_nanos() as u64;
    let pid = process::id() as u64;
    let mut seed = [0u8; 32];
    for i in 0..8usize {
        seed[i] = ((secs >> (i * 8)) & 0xff) as u8;
        seed[8 + i] = ((nanos >> (i * 4)) & 0xff) as u8;
        seed[16 + i] = ((pid >> (i * 8)) & 0xff) as u8;
        // Mix secs + nanos + index for the last 8 bytes.
        seed[24 + i] = ((secs ^ nanos ^ (i as u64 * 0x9e37)) & 0xff) as u8;
    }
    seed
}

/// Sign `data` with the 32-byte Ed25519 `seed`; return raw 64-byte signature.
fn sign_with_seed(seed: &[u8; 32], data: &[u8]) -> Vec<u8> {
    use ed25519_dalek::Signer as _;
    let sk = ed25519_dalek::SigningKey::from_bytes(seed);
    sk.sign(data).to_bytes().to_vec()
}

/// Derive the verifying (public) key bytes from a seed.
fn verifying_key_bytes(seed: &[u8; 32]) -> Vec<u8> {
    ed25519_dalek::SigningKey::from_bytes(seed)
        .verifying_key()
        .to_bytes()
        .to_vec()
}

// ─────────────────────────────────────────────────────────────────────────────
// Manifest construction
// ─────────────────────────────────────────────────────────────────────────────

fn make_manifest(
    channel: &str,
    version: &str,
    artifact_name: &str,
    artifact_bytes: &[u8],
    previous_version: Option<&str>,
) -> ReleaseManifestV1 {
    let artifact = ReleaseArtifact::new(
        artifact_name.to_string(),
        "multi".to_string(),
        "x86_64-unknown".to_string(),
        format!("{artifact_name}.bin"),
        sha256_hex(artifact_bytes),
    );
    ReleaseManifestV1::new(
        "legion-desktop".to_string(),
        channel.to_string(),
        version.to_string(),
        Some("full".to_string()),
        previous_version.map(|s| s.to_string()),
        vec![artifact],
        utc_now(),
        Some("drill/ephemeral".to_string()),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s1: generate ephemeral keypair
// ─────────────────────────────────────────────────────────────────────────────

struct S1Result {
    seed: [u8; 32],
    verifying_key: Vec<u8>,
}

fn run_s1() -> Result<S1Result, String> {
    let seed = make_ephemeral_seed();
    let verifying_key = verifying_key_bytes(&seed);
    // Sanity: keys must be 32 bytes.
    if verifying_key.len() != 32 {
        return Err(format!(
            "verifying key has unexpected length: {}",
            verifying_key.len()
        ));
    }
    Ok(S1Result { seed, verifying_key })
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s2: fabricate artifacts + signed v0.2.0 manifest
// ─────────────────────────────────────────────────────────────────────────────

struct S2Result {
    temp_dir: PathBuf,
    #[allow(dead_code)]
    manifest_v2: ReleaseManifestV1,
    manifest_toml_bytes: Vec<u8>,
    sig_bytes: Vec<u8>,
}

fn run_s2(seed: &[u8; 32], base_dir: &Path) -> Result<S2Result, String> {
    let temp_dir = base_dir.to_path_buf();
    fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("create temp_dir: {e}"))?;

    // Fabricate artifact bytes for v0.2.0.
    let artifact_bytes_v2: &[u8] = b"legion-desktop v0.2.0 binary payload (drill)";
    fs::write(temp_dir.join("legion-test.bin"), artifact_bytes_v2)
        .map_err(|e| format!("write artifact: {e}"))?;

    let manifest_v2 = make_manifest(
        "stable",
        "0.2.0",
        "legion-test",
        artifact_bytes_v2,
        Some("0.1.0"),
    );

    let manifest_toml = toml::to_string_pretty(&manifest_v2)
        .map_err(|e| format!("serialize manifest: {e}"))?;
    let manifest_toml_bytes = manifest_toml.into_bytes();

    // Write manifest.
    fs::write(
        temp_dir.join("release-manifest.v1.toml"),
        &manifest_toml_bytes,
    )
    .map_err(|e| format!("write manifest: {e}"))?;

    // Sign manifest bytes with the ephemeral key.
    let sig_bytes = sign_with_seed(seed, &manifest_toml_bytes);
    fs::write(
        temp_dir.join("release-manifest.v1.toml.sig"),
        &sig_bytes,
    )
    .map_err(|e| format!("write sig: {e}"))?;

    Ok(S2Result {
        temp_dir,
        manifest_v2,
        manifest_toml_bytes,
        sig_bytes,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s3: check_for_update (v0.1.0 → v0.2.0 — expects Available)
// ─────────────────────────────────────────────────────────────────────────────

struct S3Result {
    manifest: ReleaseManifestV1,
    signer_status: String,
    previous_version: String,
}

fn run_s3(temp_dir: &Path, verifying_key: &[u8]) -> Result<S3Result, String> {
    let source = LocalDirManifestSource::new(temp_dir);
    let policy = UpdatePolicy {
        current_version: "0.1.0".to_string(),
        current_channel: "stable".to_string(),
        allow_unsigned_beta: false,
    };

    let check = Updater::new()
        .check_for_update(&source, &policy, Some(verifying_key))
        .map_err(|e| format!("check_for_update failed: {e}"))?;

    match check {
        UpdateCheck::Available { manifest, signer_status, previous_version } => {
            if manifest.version != "0.2.0" {
                return Err(format!(
                    "expected version 0.2.0, got {}",
                    manifest.version
                ));
            }
            if signer_status != "signed/ed25519" {
                return Err(format!(
                    "expected signer_status=signed/ed25519, got {signer_status}"
                ));
            }
            Ok(S3Result { manifest, signer_status, previous_version })
        }
        UpdateCheck::NoUpdate => Err("expected Available, got NoUpdate".to_string()),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s4: stage_update → SHA-256 verified
// ─────────────────────────────────────────────────────────────────────────────

fn run_s4(
    manifest: ReleaseManifestV1,
    temp_dir: &Path,
    signer_status: String,
    previous_version: String,
) -> Result<legion_app::updater::StagedUpdate, String> {
    Updater::new()
        .stage_update(manifest, temp_dir, signer_status, Some(previous_version))
        .map_err(|e| format!("stage_update failed: {e}"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s5: apply_update → journal written
// ─────────────────────────────────────────────────────────────────────────────

fn run_s5(
    staged: &legion_app::updater::StagedUpdate,
    journal_path: &Path,
) -> Result<legion_app::updater::UpdateJournal, String> {
    Updater::new()
        .apply_update(staged, journal_path, &utc_now())
        .map_err(|e| format!("apply_update failed: {e}"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s6: rollback → journal reverted (toggle 1)
// ─────────────────────────────────────────────────────────────────────────────

fn run_s6(journal_path: &Path) -> Result<legion_app::updater::UpdateJournal, String> {
    Updater::new()
        .rollback(journal_path, &utc_now())
        .map_err(|e| format!("rollback failed: {e}"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s7: double rollback → idempotent toggle back to updated state
// ─────────────────────────────────────────────────────────────────────────────

fn run_s7(
    journal_path: &Path,
    applied_journal: &legion_app::updater::UpdateJournal,
) -> Result<(), String> {
    let double_rolled = Updater::new()
        .rollback(journal_path, &utc_now())
        .map_err(|e| format!("double rollback failed: {e}"))?;

    if double_rolled.current_version != applied_journal.current_version {
        return Err(format!(
            "idempotent toggle failed: expected current_version={} after double rollback, got {}",
            applied_journal.current_version, double_rolled.current_version
        ));
    }
    if double_rolled.previous_version != applied_journal.previous_version {
        return Err(format!(
            "idempotent toggle failed: expected previous_version={:?} after double rollback, got {:?}",
            applied_journal.previous_version, double_rolled.previous_version
        ));
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s8 (negative): bad signature → SignatureInvalid
// ─────────────────────────────────────────────────────────────────────────────

fn run_s8(
    s2: &S2Result,
    verifying_key: &[u8],
    scratch_dir: &Path,
) -> Result<(), String> {
    let bad_dir = scratch_dir.join("bad_sig");
    fs::create_dir_all(&bad_dir).map_err(|e| format!("s8 mkdir: {e}"))?;

    // Write the manifest…
    fs::write(
        bad_dir.join("release-manifest.v1.toml"),
        &s2.manifest_toml_bytes,
    )
    .map_err(|e| format!("s8 write manifest: {e}"))?;

    // …with a corrupted signature (flip byte 0).
    let mut bad_sig = s2.sig_bytes.clone();
    bad_sig[0] ^= 0xff;
    fs::write(bad_dir.join("release-manifest.v1.toml.sig"), &bad_sig)
        .map_err(|e| format!("s8 write bad sig: {e}"))?;

    let source = LocalDirManifestSource::new(&bad_dir);
    let policy = UpdatePolicy {
        current_version: "0.1.0".to_string(),
        current_channel: "stable".to_string(),
        allow_unsigned_beta: false,
    };

    match Updater::new().check_for_update(&source, &policy, Some(verifying_key)) {
        Err(UpdateError::SignatureInvalid(_)) => Ok(()),
        other => Err(format!(
            "expected SignatureInvalid, got {other:?}"
        )),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s9 (negative): bad artifact hash → HashMismatch
// ─────────────────────────────────────────────────────────────────────────────

fn run_s9(s2: &S2Result, seed: &[u8; 32], scratch_dir: &Path) -> Result<(), String> {
    let bad_dir = scratch_dir.join("bad_hash");
    fs::create_dir_all(&bad_dir).map_err(|e| format!("s9 mkdir: {e}"))?;

    // Write the original manifest (with correct hashes)…
    fs::write(
        bad_dir.join("release-manifest.v1.toml"),
        &s2.manifest_toml_bytes,
    )
    .map_err(|e| format!("s9 write manifest: {e}"))?;

    // …and a valid sig…
    fs::write(
        bad_dir.join("release-manifest.v1.toml.sig"),
        &s2.sig_bytes,
    )
    .map_err(|e| format!("s9 write sig: {e}"))?;

    // …but write WRONG content to the artifact file.
    fs::write(bad_dir.join("legion-test.bin"), b"WRONG BYTES")
        .map_err(|e| format!("s9 write wrong artifact: {e}"))?;

    let source = LocalDirManifestSource::new(&bad_dir);
    let policy = UpdatePolicy {
        current_version: "0.1.0".to_string(),
        current_channel: "stable".to_string(),
        allow_unsigned_beta: false,
    };
    let verifying_key = verifying_key_bytes(seed);

    let check = Updater::new()
        .check_for_update(&source, &policy, Some(&verifying_key))
        .map_err(|e| format!("s9 check_for_update: {e}"))?;

    match check {
        UpdateCheck::Available { manifest, signer_status, previous_version } => {
            match Updater::new().stage_update(manifest, &bad_dir, signer_status, Some(previous_version)) {
                Err(UpdateError::HashMismatch { .. }) => Ok(()),
                other => Err(format!("expected HashMismatch, got {other:?}")),
            }
        }
        UpdateCheck::NoUpdate => Err("s9: expected Available, got NoUpdate".to_string()),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s10 (negative): cross-channel → ChannelMismatch
// ─────────────────────────────────────────────────────────────────────────────

fn run_s10(scratch_dir: &Path) -> Result<(), String> {
    let bad_dir = scratch_dir.join("cross_channel");
    fs::create_dir_all(&bad_dir).map_err(|e| format!("s10 mkdir: {e}"))?;

    // Preview-channel manifest.
    let artifact_bytes: &[u8] = b"preview artifact";
    let manifest = make_manifest("preview", "0.2.0", "legion-test", artifact_bytes, None);
    let manifest_toml = toml::to_string_pretty(&manifest)
        .map_err(|e| format!("s10 serialize: {e}"))?;
    fs::write(
        bad_dir.join("release-manifest.v1.toml"),
        manifest_toml.as_bytes(),
    )
    .map_err(|e| format!("s10 write manifest: {e}"))?;
    fs::write(bad_dir.join("legion-test.bin"), artifact_bytes)
        .map_err(|e| format!("s10 write artifact: {e}"))?;

    let source = LocalDirManifestSource::new(&bad_dir);
    let policy = UpdatePolicy {
        current_version: "0.1.0".to_string(),
        current_channel: "stable".to_string(), // mismatch: manifest is "preview"
        allow_unsigned_beta: true,
    };

    match Updater::new().check_for_update(&source, &policy, None) {
        Err(UpdateError::ChannelMismatch { .. }) => Ok(()),
        other => Err(format!("expected ChannelMismatch, got {other:?}")),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Step s11 (negative): downgrade → NoUpdate
// ─────────────────────────────────────────────────────────────────────────────

fn run_s11(scratch_dir: &Path) -> Result<(), String> {
    let bad_dir = scratch_dir.join("downgrade");
    fs::create_dir_all(&bad_dir).map_err(|e| format!("s11 mkdir: {e}"))?;

    // Manifest at v0.1.0 but policy says we're already at v0.2.0.
    let artifact_bytes: &[u8] = b"old artifact";
    let manifest = make_manifest("stable", "0.1.0", "legion-test", artifact_bytes, None);
    let manifest_toml = toml::to_string_pretty(&manifest)
        .map_err(|e| format!("s11 serialize: {e}"))?;
    fs::write(
        bad_dir.join("release-manifest.v1.toml"),
        manifest_toml.as_bytes(),
    )
    .map_err(|e| format!("s11 write manifest: {e}"))?;
    fs::write(bad_dir.join("legion-test.bin"), artifact_bytes)
        .map_err(|e| format!("s11 write artifact: {e}"))?;

    let source = LocalDirManifestSource::new(&bad_dir);
    let policy = UpdatePolicy {
        current_version: "0.2.0".to_string(), // already at this version
        current_channel: "stable".to_string(),
        allow_unsigned_beta: true,
    };

    match Updater::new().check_for_update(&source, &policy, None) {
        Ok(UpdateCheck::NoUpdate) => Ok(()),
        other => Err(format!(
            "expected NoUpdate for downgrade attempt, got {other:?}"
        )),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Report writer
// ─────────────────────────────────────────────────────────────────────────────

fn write_report(
    out_dir: &Path,
    git_sha: &str,
    started_utc: &str,
    finished_utc: &str,
    steps: &[StepRecord],
) -> Result<PathBuf, String> {
    fs::create_dir_all(out_dir)
        .map_err(|e| format!("create out_dir {}: {e}", out_dir.display()))?;

    let overall_status = if steps.iter().any(|s| s.status == StepStatus::Failed) {
        "failed"
    } else {
        "passed"
    };

    let mut toml = String::new();
    toml.push_str("schema_version = 1\n");
    toml.push_str(&format!("git_sha = \"{git_sha}\"\n"));
    toml.push_str(&format!("started_utc = \"{started_utc}\"\n"));
    toml.push_str(&format!("finished_utc = \"{finished_utc}\"\n"));
    toml.push_str(&format!("overall_status = \"{overall_status}\"\n\n"));

    for step in steps {
        toml.push_str("[[steps]]\n");
        toml.push_str(&format!("id = \"{}\"\n", step.id));
        toml.push_str(&format!("status = \"{}\"\n", step.status.as_str()));
        toml.push_str(&format!("started_utc = \"{}\"\n", step.started_utc));
        toml.push_str(&format!("finished_utc = \"{}\"\n", step.finished_utc));
        toml.push_str(&format!("duration_ms = {}\n", step.duration_ms));
        let detail = if step.detail.chars().count() > 256 {
            format!("{}...", step.detail.chars().take(256).collect::<String>())
        } else {
            step.detail.clone()
        };
        toml.push_str(&format!("detail = {:?}\n\n", detail));
    }

    let report_path = out_dir.join("update_drill_report.toml");
    let report_tmp = report_path.with_extension("toml.tmp");
    fs::write(&report_tmp, &toml)
        .map_err(|e| format!("write report tmp {}: {e}", report_tmp.display()))?;
    fs::rename(&report_tmp, &report_path)
        .map_err(|e| format!("rename report {}: {e}", report_path.display()))?;
    eprintln!("[update-drill] report written: {}", report_path.display());
    Ok(report_path)
}

// ─────────────────────────────────────────────────────────────────────────────
// CLI args
// ─────────────────────────────────────────────────────────────────────────────

struct Args {
    out_dir: PathBuf,
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let mut out_dir = PathBuf::from("target/update-drill");
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--out-dir" {
            i += 1;
            if let Some(p) = args.get(i) {
                out_dir = PathBuf::from(p);
            }
        }
        i += 1;
    }
    Args { out_dir }
}

fn resolve_git_sha() -> String {
    let output = process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    let args = parse_args();
    let started_utc = utc_now();
    let git_sha = resolve_git_sha();
    let mut steps: Vec<StepRecord> = Vec::new();

    eprintln!("[update-drill] starting — git sha: {git_sha}");
    eprintln!("[update-drill] out_dir: {}", args.out_dir.display());

    macro_rules! record_step {
        ($id:expr, $status:expr, $detail:expr, $ms:expr, $start:expr, $end:expr) => {
            steps.push(StepRecord {
                id: $id,
                started_utc: $start,
                finished_utc: $end,
                duration_ms: $ms,
                status: $status,
                detail: $detail,
            });
        };
    }

    // Create a temp working directory for drill artifacts.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let scratch_root = std::env::temp_dir()
        .join(format!("legion-update-drill-{}-{nanos}", process::id()));

    // ── s1: generate ephemeral keypair ──────────────────────────────────────
    let s1_start = utc_now();
    let (s1_result, s1_ms) = run_timer(run_s1);
    let s1_end = utc_now();
    let (seed, verifying_key) = match s1_result {
        Ok(r) => {
            eprintln!("[s1] passed ({}ms): ephemeral keypair generated", s1_ms);
            record_step!(
                "s1",
                StepStatus::Passed,
                format!("ephemeral Ed25519 keypair generated in-memory ({}ms)", s1_ms),
                s1_ms, s1_start, s1_end
            );
            (r.seed, r.verifying_key)
        }
        Err(e) => {
            eprintln!("[s1] FAILED: {e}");
            record_step!("s1", StepStatus::Failed, e.clone(), s1_ms, s1_start, s1_end);
            let _ = write_report(&args.out_dir, &git_sha, &started_utc, &utc_now(), &steps);
            process::exit(1);
        }
    };

    // ── s2: fabricate artifacts + signed manifest ───────────────────────────
    let s2_dir = scratch_root.join("s2");
    let s2_start = utc_now();
    let (s2_result, s2_ms) = run_timer(|| run_s2(&seed, &s2_dir));
    let s2_end = utc_now();
    let s2 = match s2_result {
        Ok(r) => {
            eprintln!("[s2] passed ({}ms): artifacts + signed manifest ready", s2_ms);
            record_step!(
                "s2",
                StepStatus::Passed,
                format!(
                    "v0.2.0 artifact + manifest fabricated; manifest signed with ephemeral key ({}ms)",
                    s2_ms
                ),
                s2_ms, s2_start, s2_end
            );
            r
        }
        Err(e) => {
            eprintln!("[s2] FAILED: {e}");
            record_step!("s2", StepStatus::Failed, e.clone(), s2_ms, s2_start, s2_end);
            let _ = write_report(&args.out_dir, &git_sha, &started_utc, &utc_now(), &steps);
            process::exit(1);
        }
    };

    // ── s3: check_for_update ────────────────────────────────────────────────
    let s3_start = utc_now();
    let (s3_result, s3_ms) = run_timer(|| run_s3(&s2.temp_dir, &verifying_key));
    let s3_end = utc_now();
    let (manifest_for_stage, signer_status_for_stage, previous_version_for_stage) = match s3_result {
        Ok(r) => {
            eprintln!("[s3] passed ({}ms): Available v0.2.0 signed/ed25519", s3_ms);
            record_step!(
                "s3",
                StepStatus::Passed,
                format!(
                    "check_for_update: v0.1.0 → v0.2.0 Available; signer_status=signed/ed25519 ({}ms)",
                    s3_ms
                ),
                s3_ms, s3_start, s3_end
            );
            (r.manifest, r.signer_status, r.previous_version)
        }
        Err(e) => {
            eprintln!("[s3] FAILED: {e}");
            record_step!("s3", StepStatus::Failed, e.clone(), s3_ms, s3_start, s3_end);
            let _ = write_report(&args.out_dir, &git_sha, &started_utc, &utc_now(), &steps);
            process::exit(1);
        }
    };

    // ── s4: stage_update ─────────────────────────────────────────────────────
    let s4_start = utc_now();
    let (s4_result, s4_ms) = run_timer(|| {
        run_s4(manifest_for_stage, &s2.temp_dir, signer_status_for_stage, previous_version_for_stage)
    });
    let s4_end = utc_now();
    let staged = match s4_result {
        Ok(s) => {
            eprintln!("[s4] passed ({}ms): artifact staged + hash verified", s4_ms);
            record_step!(
                "s4",
                StepStatus::Passed,
                format!("stage_update: SHA-256 verified; artifact copied to staged/ ({}ms)", s4_ms),
                s4_ms, s4_start, s4_end
            );
            s
        }
        Err(e) => {
            eprintln!("[s4] FAILED: {e}");
            record_step!("s4", StepStatus::Failed, e.clone(), s4_ms, s4_start, s4_end);
            let _ = write_report(&args.out_dir, &git_sha, &started_utc, &utc_now(), &steps);
            process::exit(1);
        }
    };

    // ── s5: apply_update ─────────────────────────────────────────────────────
    let journal_path = scratch_root.join("update_journal.toml");
    let s5_start = utc_now();
    let (s5_result, s5_ms) = run_timer(|| run_s5(&staged, &journal_path));
    let s5_end = utc_now();
    let applied_journal = match s5_result {
        Ok(j) => {
            eprintln!(
                "[s5] passed ({}ms): journal current={} previous={:?}",
                s5_ms, j.current_version, j.previous_version
            );
            let ok = j.current_version == "0.2.0"
                && j.previous_version.as_deref() == Some("0.1.0")
                && j.signer_status == "signed/ed25519";
            if !ok {
                let e = format!("journal assertion failed: {j:?}");
                eprintln!("[s5] FAILED: {e}");
                record_step!("s5", StepStatus::Failed, e.clone(), s5_ms, s5_start, s5_end);
                let _ = write_report(&args.out_dir, &git_sha, &started_utc, &utc_now(), &steps);
                process::exit(1);
            }
            record_step!(
                "s5",
                StepStatus::Passed,
                format!(
                    "apply_update: journal current=0.2.0 previous=0.1.0 signer_status=signed/ed25519 ({}ms)",
                    s5_ms
                ),
                s5_ms, s5_start, s5_end
            );
            j
        }
        Err(e) => {
            eprintln!("[s5] FAILED: {e}");
            record_step!("s5", StepStatus::Failed, e.clone(), s5_ms, s5_start, s5_end);
            let _ = write_report(&args.out_dir, &git_sha, &started_utc, &utc_now(), &steps);
            process::exit(1);
        }
    };

    // ── s6: rollback (toggle 1) ──────────────────────────────────────────────
    let s6_start = utc_now();
    let (s6_result, s6_ms) = run_timer(|| run_s6(&journal_path));
    let s6_end = utc_now();
    match s6_result {
        Ok(j) => {
            let ok = j.current_version == "0.1.0"
                && j.previous_version.as_deref() == Some("0.2.0");
            if !ok {
                let e = format!("rollback journal assertion failed: {j:?}");
                eprintln!("[s6] FAILED: {e}");
                record_step!("s6", StepStatus::Failed, e, s6_ms, s6_start, s6_end);
            } else {
                eprintln!("[s6] passed ({}ms): current=0.1.0 previous=0.2.0", s6_ms);
                record_step!(
                    "s6",
                    StepStatus::Passed,
                    format!(
                        "rollback: journal current=0.1.0 previous=0.2.0 ({}ms)",
                        s6_ms
                    ),
                    s6_ms, s6_start, s6_end
                );
            }
        }
        Err(e) => {
            eprintln!("[s6] FAILED: {e}");
            record_step!("s6", StepStatus::Failed, e, s6_ms, s6_start, s6_end);
        }
    }

    // ── s7: double rollback — idempotent toggle ──────────────────────────────
    let s7_start = utc_now();
    let (s7_result, s7_ms) = run_timer(|| run_s7(&journal_path, &applied_journal));
    let s7_end = utc_now();
    match s7_result {
        Ok(()) => {
            eprintln!("[s7] passed ({}ms): double rollback idempotent", s7_ms);
            record_step!(
                "s7",
                StepStatus::Passed,
                format!("double rollback: back to current=0.2.0 previous=0.1.0 ({}ms)", s7_ms),
                s7_ms, s7_start, s7_end
            );
        }
        Err(e) => {
            eprintln!("[s7] FAILED: {e}");
            record_step!("s7", StepStatus::Failed, e, s7_ms, s7_start, s7_end);
        }
    }

    // ── s8: negative — bad signature ─────────────────────────────────────────
    let s8_start = utc_now();
    let (s8_result, s8_ms) = run_timer(|| run_s8(&s2, &verifying_key, &scratch_root));
    let s8_end = utc_now();
    match s8_result {
        Ok(()) => {
            eprintln!("[s8] passed ({}ms): bad sig correctly rejected", s8_ms);
            record_step!(
                "s8",
                StepStatus::Passed,
                format!("negative: bad signature → SignatureInvalid ({}ms)", s8_ms),
                s8_ms, s8_start, s8_end
            );
        }
        Err(e) => {
            eprintln!("[s8] FAILED: {e}");
            record_step!("s8", StepStatus::Failed, e, s8_ms, s8_start, s8_end);
        }
    }

    // ── s9: negative — bad artifact hash ────────────────────────────────────
    let s9_start = utc_now();
    let (s9_result, s9_ms) = run_timer(|| run_s9(&s2, &seed, &scratch_root));
    let s9_end = utc_now();
    match s9_result {
        Ok(()) => {
            eprintln!("[s9] passed ({}ms): bad hash correctly rejected", s9_ms);
            record_step!(
                "s9",
                StepStatus::Passed,
                format!("negative: bad artifact hash → HashMismatch ({}ms)", s9_ms),
                s9_ms, s9_start, s9_end
            );
        }
        Err(e) => {
            eprintln!("[s9] FAILED: {e}");
            record_step!("s9", StepStatus::Failed, e, s9_ms, s9_start, s9_end);
        }
    }

    // ── s10: negative — cross-channel ────────────────────────────────────────
    let s10_start = utc_now();
    let (s10_result, s10_ms) = run_timer(|| run_s10(&scratch_root));
    let s10_end = utc_now();
    match s10_result {
        Ok(()) => {
            eprintln!("[s10] passed ({}ms): cross-channel correctly rejected", s10_ms);
            record_step!(
                "s10",
                StepStatus::Passed,
                format!("negative: preview manifest + stable policy → ChannelMismatch ({}ms)", s10_ms),
                s10_ms, s10_start, s10_end
            );
        }
        Err(e) => {
            eprintln!("[s10] FAILED: {e}");
            record_step!("s10", StepStatus::Failed, e, s10_ms, s10_start, s10_end);
        }
    }

    // ── s11: negative — downgrade ─────────────────────────────────────────────
    let s11_start = utc_now();
    let (s11_result, s11_ms) = run_timer(|| run_s11(&scratch_root));
    let s11_end = utc_now();
    match s11_result {
        Ok(()) => {
            eprintln!("[s11] passed ({}ms): downgrade correctly returned NoUpdate", s11_ms);
            record_step!(
                "s11",
                StepStatus::Passed,
                format!("negative: downgrade v0.2.0 → v0.1.0 → NoUpdate ({}ms)", s11_ms),
                s11_ms, s11_start, s11_end
            );
        }
        Err(e) => {
            eprintln!("[s11] FAILED: {e}");
            record_step!("s11", StepStatus::Failed, e, s11_ms, s11_start, s11_end);
        }
    }

    // ── s12: write report ────────────────────────────────────────────────────
    let finished_utc = utc_now();
    let report_result = write_report(
        &args.out_dir,
        &git_sha,
        &started_utc,
        &finished_utc,
        &steps,
    );

    eprintln!("\n[update-drill] SUMMARY");
    for step in &steps {
        eprintln!(
            "  {} {} ({}ms): {}",
            step.id,
            step.status.as_str(),
            step.duration_ms,
            step.detail.chars().take(80).collect::<String>()
        );
    }

    let any_failed = steps.iter().any(|s| s.status == StepStatus::Failed);

    // Clean up scratch dir on success; leave for inspection on failure.
    if any_failed {
        eprintln!(
            "\n[update-drill] FAILED — scratch dir left for inspection: {}",
            scratch_root.display()
        );
        process::exit(1);
    } else {
        let _ = fs::remove_dir_all(&scratch_root);
        match report_result {
            Ok(path) => eprintln!("\n[update-drill] PASSED — report: {}", path.display()),
            Err(e) => eprintln!("\n[update-drill] PASSED (report write failed: {e})"),
        }
        process::exit(0);
    }
}
