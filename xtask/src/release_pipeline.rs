use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::signing::{SignerResolution, SigningConfig, UpdaterConfig, resolve_signer};

pub const DRY_RUN_SIGNER_STATUS: &str = "dry-run/no-production-signer";
pub const UNSIGNED_BETA_SIGNER_STATUS: &str = "unsigned-beta/no-signer-configured";
pub const SIGNED_ED25519_SIGNER_STATUS: &str = "signed/ed25519";
pub const VERSION_STAMP_FILE: &str = "version_stamp.toml";
pub const VERIFY_REPORT_FILE: &str = "verify_report.toml";
pub const RELEASE_MANIFEST_FILE: &str = "release-manifest.v1.toml";
pub const RELEASE_MANIFEST_SIG_FILE: &str = "release-manifest.v1.toml.sig";
const DRY_RUN_VERIFIER_STATUS: &str = "dry-run/unchecked";
const DRY_RUN_VERIFIER_MESSAGE: &str =
    "verification_command not executed in dry-run; pending real artifact hash and signer";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseChannel {
    Stable,
    Preview,
}

impl ReleaseChannel {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "stable" => Ok(Self::Stable),
            "preview" => Ok(Self::Preview),
            other => Err(format!(
                "unsupported release channel `{other}`; expected `stable` or `preview`"
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Preview => "preview",
        }
    }
}

/// Channel-derived rollout policy used by the auto-update workstream (WS17.T3).
/// Stable is intended for full rollout; preview is staged. The pipeline records
/// the policy in each descriptor and the version stamp so downstream consumers
/// do not need to re-derive it from the channel label.
pub fn channel_rollout_policy(channel: ReleaseChannel) -> &'static str {
    match channel {
        ReleaseChannel::Stable => "full",
        ReleaseChannel::Preview => "staged",
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ReleasePipelineConfig {
    pub package_name: String,
    pub dist_tool: String,
    pub installer_targets: Vec<InstallerTargetConfig>,
    /// Signer reference configuration (no key material — reference only).
    pub signing: Option<SigningConfig>,
    /// Updater strategy configuration.
    pub updater: Option<UpdaterConfig>,
}

impl Default for ReleasePipelineConfig {
    fn default() -> Self {
        Self {
            package_name: "legion-desktop".to_string(),
            dist_tool: "cargo-dist".to_string(),
            installer_targets: Vec::new(),
            signing: None,
            updater: None,
        }
    }
}

impl ReleasePipelineConfig {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|err| {
            format!(
                "unable to read release pipeline config `{}`: {err}",
                path.display()
            )
        })?;
        toml::from_str(&text).map_err(|err| {
            format!(
                "unable to parse release pipeline config `{}`: {err}",
                path.display()
            )
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallerTargetConfig {
    pub name: String,
    pub platform: String,
    pub target: String,
    pub artifact: String,
    pub build_command: String,
    pub verification_command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionStamp {
    pub schema_version: u32,
    pub package_name: String,
    pub package_version: String,
    pub channel: String,
    pub rollout_policy: String,
    pub dist_tool: String,
    pub git_sha: String,
    pub built_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallerDescriptor {
    pub schema_version: u32,
    pub package_name: String,
    pub channel: String,
    pub version: String,
    pub dist_tool: String,
    pub name: String,
    pub platform: String,
    pub target: String,
    pub artifact: String,
    pub build_command: String,
    pub verification_command: String,
    pub signer_status: String,
    pub sha256: String,
    pub sha256_status: String,
    pub version_stamp: VersionStamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasePipelinePlan {
    pub version_stamp: VersionStamp,
    pub descriptors: Vec<InstallerDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescriptorVerificationEntry {
    pub name: String,
    pub platform: String,
    pub target: String,
    pub descriptor_path: PathBuf,
    pub signer_status: String,
    pub sha256: String,
    pub verifier_status: String,
    pub verifier_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VerificationSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub unchecked: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub schema_version: u32,
    pub package_name: String,
    pub channel: String,
    pub version: String,
    pub dist_tool: String,
    pub verified_at_utc: String,
    pub summary: VerificationSummary,
    pub descriptors: Vec<DescriptorVerificationEntry>,
}

/// Plan a release pipeline.
///
/// # Modes
///
/// * `dry_run = true` — generate descriptor stubs only; no artifact hashing or
///   signing.  Signer status is `"dry-run/no-production-signer"` and sha256 is
///   `"pending"`.
/// * `artifacts_dir = Some(dir)` — locate artifacts in `dir`, compute real
///   SHA-256 hashes; missing files get `sha256_status = "artifact-absent"`.
///   The signer from `config.signing` is resolved and the status reflects its
///   availability.
/// * Neither — returns a clear error indicating that `--dry-run` or
///   `--from-artifacts` is required.
pub fn plan_release_pipeline(
    workspace_root: &Path,
    config: &ReleasePipelineConfig,
    channel: ReleaseChannel,
    dry_run: bool,
    artifacts_dir: Option<&Path>,
) -> Result<ReleasePipelinePlan, String> {
    if !workspace_root.exists() {
        return Err(format!(
            "workspace root `{}` does not exist",
            workspace_root.display()
        ));
    }
    if !dry_run && artifacts_dir.is_none() {
        return Err(
            "release pipeline requires --dry-run or --from-artifacts mode; \
             neither was provided"
                .to_string(),
        );
    }
    if config.installer_targets.is_empty() {
        return Err(
            "release pipeline config must declare at least one installer target".to_string(),
        );
    }

    let workspace_version = workspace_version(workspace_root)?;
    let version = match channel {
        ReleaseChannel::Stable => workspace_version.clone(),
        ReleaseChannel::Preview => format!("{workspace_version}-preview"),
    };

    let version_stamp = build_version_stamp(
        &config.package_name,
        // Use the channel-adjusted version so the stamp's package_version
        // matches each descriptor's `version` (e.g. `0.1.0-preview` for the
        // preview channel) instead of the unsuffixed workspace version.
        &version,
        channel,
        &config.dist_tool,
        workspace_root,
    )?;

    // Resolve signer when in from-artifacts mode (not dry-run).
    let signer_status_for_mode: Option<String> = if !dry_run {
        // from-artifacts mode: resolve signing infrastructure
        let status = match &config.signing {
            None => UNSIGNED_BETA_SIGNER_STATUS.to_string(),
            Some(signing_cfg) => match resolve_signer(signing_cfg) {
                SignerResolution::Available(_) => SIGNED_ED25519_SIGNER_STATUS.to_string(),
                SignerResolution::Unavailable { .. } => UNSIGNED_BETA_SIGNER_STATUS.to_string(),
            },
        };
        Some(status)
    } else {
        None
    };

    let mut descriptors = config
        .installer_targets
        .iter()
        .map(|target| {
            let (signer_status, sha256, sha256_status) = if dry_run {
                (
                    DRY_RUN_SIGNER_STATUS.to_string(),
                    "pending".to_string(),
                    "dry-run descriptor only; artifact hash is unavailable until build".to_string(),
                )
            } else {
                // from-artifacts mode
                let dir = artifacts_dir.expect("artifacts_dir checked above");
                let artifact_file = dir.join(format!("{}.{}", target.name, target.artifact));
                let (sha256, sha256_status) = if artifact_file.is_file() {
                    match compute_sha256_file(&artifact_file) {
                        Ok(hash) => (hash, "computed".to_string()),
                        Err(err) => ("".to_string(), format!("sha256-error: {err}")),
                    }
                } else {
                    ("".to_string(), "artifact-absent".to_string())
                };
                let signer_status = signer_status_for_mode
                    .as_deref()
                    .unwrap_or(UNSIGNED_BETA_SIGNER_STATUS)
                    .to_string();
                (signer_status, sha256, sha256_status)
            };

            InstallerDescriptor {
                schema_version: 1,
                package_name: config.package_name.clone(),
                channel: version_stamp.channel.clone(),
                version: version.clone(),
                dist_tool: config.dist_tool.clone(),
                name: target.name.clone(),
                platform: target.platform.clone(),
                target: target.target.clone(),
                artifact: target.artifact.clone(),
                build_command: target.build_command.clone(),
                verification_command: target.verification_command.clone(),
                signer_status,
                sha256,
                sha256_status,
                version_stamp: version_stamp.clone(),
            }
        })
        .collect::<Vec<_>>();
    descriptors.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(ReleasePipelinePlan {
        version_stamp,
        descriptors,
    })
}

pub fn write_descriptors(
    plan: &ReleasePipelinePlan,
    out_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    // Detect descriptor file-name collisions before writing anything: the
    // stem normalization maps distinct installer-target names (e.g.
    // `linux x64` and `linux-x64`) onto the same `<stem>.toml`, which would
    // otherwise silently overwrite a previously written descriptor.
    let mut seen_stems = HashSet::with_capacity(plan.descriptors.len());
    for descriptor in &plan.descriptors {
        let stem = descriptor_file_stem(&descriptor.name);
        if !seen_stems.insert(stem.clone()) {
            return Err(format!(
                "release pipeline descriptor file-name collision on `{stem}.toml`: \
                 multiple installer targets normalize to the same descriptor file \
                 (conflicting name `{}`); rename the installer target(s) to disambiguate",
                descriptor.name
            ));
        }
    }

    fs::create_dir_all(out_dir).map_err(|err| {
        format!(
            "unable to create release pipeline output dir `{}`: {err}",
            out_dir.display()
        )
    })?;
    let mut written = Vec::new();

    let stamp_path = out_dir.join(VERSION_STAMP_FILE);
    let stamp_text = toml::to_string_pretty(&plan.version_stamp)
        .map_err(|err| format!("unable to serialize version stamp: {err}"))?;
    fs::write(&stamp_path, stamp_text).map_err(|err| {
        format!(
            "unable to write release pipeline version stamp `{}`: {err}",
            stamp_path.display()
        )
    })?;
    written.push(stamp_path);

    for descriptor in &plan.descriptors {
        let path = out_dir.join(format!("{}.toml", descriptor_file_stem(&descriptor.name)));
        let text = toml::to_string_pretty(descriptor).map_err(|err| {
            format!(
                "unable to serialize descriptor `{}`: {err}",
                descriptor.name
            )
        })?;
        fs::write(&path, text).map_err(|err| {
            format!(
                "unable to write release pipeline descriptor `{}`: {err}",
                path.display()
            )
        })?;
        written.push(path);
    }
    written.sort();
    Ok(written)
}

/// Walk the on-disk descriptors, cross-check that each plan descriptor has a
/// written file, and record the verifier status. The current verifier is a
/// fail-closed integrity check: missing files or on-disk tampering cause a hard
/// rejection so the dry-run surface still proves descriptor integrity before
/// real signing / checksum manifests land.
///
/// # Extended options
///
/// * `artifacts_dir` — when provided, recompute the SHA-256 of each artifact
///   file and compare against the recorded hash (real pass/fail instead of
///   unchecked).
/// * `verifying_key_bytes` — when provided, verify the Ed25519 detached
///   signature on the release manifest file in `out_dir`.
pub fn verify_descriptors(
    _workspace_root: &Path,
    plan: &ReleasePipelinePlan,
    out_dir: &Path,
    artifacts_dir: Option<&Path>,
    verifying_key_bytes: Option<&[u8]>,
) -> Result<VerificationReport, String> {
    if !out_dir.is_dir() {
        return Err(format!(
            "release pipeline output dir `{}` does not exist; run `write_descriptors` first",
            out_dir.display()
        ));
    }

    let written_stamp = read_written_version_stamp(out_dir)?;
    let mut expected_stamp = plan.version_stamp.clone();
    expected_stamp.built_at_utc = written_stamp.built_at_utc.clone();
    if expected_stamp != written_stamp {
        return Err(format!(
            "release pipeline version stamp at `{}` does not match the planned descriptor metadata",
            out_dir.join(VERSION_STAMP_FILE).display()
        ));
    }

    // Optionally verify the manifest signature when a verifying key is provided.
    if let Some(vk_bytes) = verifying_key_bytes {
        verify_manifest_signature(out_dir, vk_bytes)?;
    }

    let mut entries = Vec::with_capacity(plan.descriptors.len());
    let mut summary = VerificationSummary::default();
    for descriptor in &plan.descriptors {
        let descriptor_path =
            out_dir.join(format!("{}.toml", descriptor_file_stem(&descriptor.name)));
        let mut expected_descriptor = descriptor.clone();
        expected_descriptor.version_stamp = written_stamp.clone();
        let expected_text = toml::to_string_pretty(&expected_descriptor).map_err(|err| {
            format!(
                "unable to serialize expected descriptor `{}` for verification: {err}",
                descriptor.name
            )
        })?;
        let (verifier_status, verifier_message) = if !descriptor_path.is_file() {
            (
                "failed/missing-descriptor".to_string(),
                format!(
                    "expected descriptor file `{}` was not written by `write_descriptors`",
                    descriptor_path.display()
                ),
            )
        } else {
            let actual_text = fs::read_to_string(&descriptor_path).map_err(|err| {
                format!(
                    "unable to read release pipeline descriptor `{}`: {err}",
                    descriptor_path.display()
                )
            })?;
            if actual_text != expected_text {
                (
                    "failed/tampered-descriptor".to_string(),
                    format!(
                        "descriptor `{}` failed integrity comparison; on-disk bytes differ from the planned checksum manifest",
                        descriptor_path.display()
                    ),
                )
            } else if let Some(art_dir) = artifacts_dir {
                // Artifact files present: recompute sha256 and compare.
                let artifact_file =
                    art_dir.join(format!("{}.{}", descriptor.name, descriptor.artifact));
                if artifact_file.is_file() {
                    match compute_sha256_file(&artifact_file) {
                        Ok(computed)
                            if !descriptor.sha256.is_empty() && computed == descriptor.sha256 =>
                        {
                            (
                                "passed/sha256-verified".to_string(),
                                format!("artifact `{}` sha256 verified", artifact_file.display()),
                            )
                        }
                        Ok(computed) if descriptor.sha256.is_empty() => {
                            // sha256 field is blank (e.g. artifact-absent mode)
                            (
                                DRY_RUN_VERIFIER_STATUS.to_string(),
                                format!(
                                    "artifact `{}` found (sha256={computed}) but descriptor sha256 is empty",
                                    artifact_file.display()
                                ),
                            )
                        }
                        Ok(computed) => (
                            "failed/sha256-mismatch".to_string(),
                            format!(
                                "artifact `{}` sha256 mismatch: expected `{}`, computed `{computed}`",
                                artifact_file.display(),
                                descriptor.sha256
                            ),
                        ),
                        Err(err) => (
                            "failed/sha256-error".to_string(),
                            format!(
                                "unable to compute sha256 for artifact `{}`: {err}",
                                artifact_file.display()
                            ),
                        ),
                    }
                } else {
                    // Artifact file is genuinely absent — unchecked is appropriate.
                    (
                        DRY_RUN_VERIFIER_STATUS.to_string(),
                        format!(
                            "artifact file `{}` not present; sha256 check skipped",
                            artifact_file.display()
                        ),
                    )
                }
            } else {
                (
                    DRY_RUN_VERIFIER_STATUS.to_string(),
                    DRY_RUN_VERIFIER_MESSAGE.to_string(),
                )
            }
        };
        match verifier_status.as_str() {
            s if s.starts_with("passed") => summary.passed += 1,
            s if s.starts_with("failed") => summary.failed += 1,
            _ => summary.unchecked += 1,
        }
        entries.push(DescriptorVerificationEntry {
            name: descriptor.name.clone(),
            platform: descriptor.platform.clone(),
            target: descriptor.target.clone(),
            descriptor_path,
            signer_status: descriptor.signer_status.clone(),
            sha256: descriptor.sha256.clone(),
            verifier_status,
            verifier_message,
        });
    }
    summary.total = entries.len();

    let report = VerificationReport {
        schema_version: 1,
        package_name: plan.version_stamp.package_name.clone(),
        channel: plan.version_stamp.channel.clone(),
        version: plan
            .descriptors
            .first()
            .map(|d| d.version.clone())
            .unwrap_or_default(),
        dist_tool: plan.version_stamp.dist_tool.clone(),
        verified_at_utc: current_utc_rfc3339(),
        summary,
        descriptors: entries,
    };

    let report_path = out_dir.join(VERIFY_REPORT_FILE);
    let report_text = toml::to_string_pretty(&report)
        .map_err(|err| format!("unable to serialize verification report: {err}"))?;
    fs::write(&report_path, report_text).map_err(|err| {
        format!(
            "unable to write release pipeline verification report `{}`: {err}",
            report_path.display()
        )
    })?;

    Ok(report)
}

/// Build a `ReleaseManifestV1` from the pipeline plan and write it to
/// `out_dir` (defaulting to `artifacts_dir`).
///
/// If a signer is configured and available, also writes a detached `.sig`
/// file (`release-manifest.v1.toml.sig`).  If no signer is available, only
/// the manifest TOML is written and `"unsigned-beta/no-signer-configured"` is
/// logged.
///
/// # Parameters
/// * `plan` — the release pipeline plan (version stamp + descriptors)
/// * `artifacts_dir` — directory containing built artifact files
/// * `out_dir` — directory to write the manifest to (often == `artifacts_dir`)
/// * `previous_version` — optional rollback pointer
/// * `signing_cfg` — signer reference config (no key material)
pub fn write_release_manifest(
    plan: &ReleasePipelinePlan,
    artifacts_dir: &Path,
    out_dir: &Path,
    previous_version: Option<&str>,
    signing_cfg: Option<&SigningConfig>,
) -> Result<(PathBuf, Option<PathBuf>, String), String> {
    use legion_protocol::ReleaseArtifact;
    use legion_protocol::ReleaseManifestV1;

    // Build artifact entries with real sha256 hashes.
    let artifacts: Vec<ReleaseArtifact> = plan
        .descriptors
        .iter()
        .map(|descriptor| {
            let artifact_file =
                artifacts_dir.join(format!("{}.{}", descriptor.name, descriptor.artifact));
            let sha256 = if artifact_file.is_file() {
                compute_sha256_file(&artifact_file).unwrap_or_else(|_| String::new())
            } else {
                String::new()
            };
            ReleaseArtifact::new(
                descriptor.name.clone(),
                descriptor.platform.clone(),
                descriptor.target.clone(),
                artifact_file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string(),
                sha256,
            )
        })
        .collect();

    let signer_reference = signing_cfg.map(|cfg| cfg.reference.clone());

    let manifest = ReleaseManifestV1::new(
        plan.version_stamp.package_name.clone(),
        plan.version_stamp.channel.clone(),
        plan.version_stamp.package_version.clone(),
        Some(plan.version_stamp.rollout_policy.clone()),
        previous_version.map(ToOwned::to_owned),
        artifacts,
        current_utc_rfc3339(),
        signer_reference,
    );

    manifest
        .validate()
        .map_err(|err| format!("release manifest validation failed: {err}"))?;

    // Serialize the manifest to TOML.
    let manifest_toml = toml::to_string_pretty(&manifest)
        .map_err(|err| format!("unable to serialize release manifest: {err}"))?;

    fs::create_dir_all(out_dir).map_err(|err| {
        format!(
            "unable to create manifest output dir `{}`: {err}",
            out_dir.display()
        )
    })?;

    let manifest_path = out_dir.join(RELEASE_MANIFEST_FILE);
    fs::write(&manifest_path, &manifest_toml).map_err(|err| {
        format!(
            "unable to write release manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;

    // Resolve signer and optionally write .sig file.
    let (sig_path, signer_status) = match signing_cfg {
        None => {
            println!(
                "release-manifest: {} (no signing config)",
                UNSIGNED_BETA_SIGNER_STATUS
            );
            (None, UNSIGNED_BETA_SIGNER_STATUS.to_string())
        }
        Some(cfg) => match resolve_signer(cfg) {
            SignerResolution::Available(signer) => {
                let sig_bytes = signer
                    .sign_bytes(manifest_toml.as_bytes())
                    .map_err(|err| format!("signing failed: {err}"))?;
                let sig_path = out_dir.join(RELEASE_MANIFEST_SIG_FILE);
                fs::write(&sig_path, &sig_bytes).map_err(|err| {
                    format!(
                        "unable to write signature file `{}`: {err}",
                        sig_path.display()
                    )
                })?;
                println!(
                    "release-manifest: {} — manifest signed with Ed25519",
                    SIGNED_ED25519_SIGNER_STATUS
                );
                (Some(sig_path), SIGNED_ED25519_SIGNER_STATUS.to_string())
            }
            SignerResolution::Unavailable { reason } => {
                println!(
                    "release-manifest: {} — {reason}",
                    UNSIGNED_BETA_SIGNER_STATUS
                );
                (None, UNSIGNED_BETA_SIGNER_STATUS.to_string())
            }
        },
    };

    Ok((manifest_path, sig_path, signer_status))
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn verify_manifest_signature(out_dir: &Path, vk_bytes: &[u8]) -> Result<(), String> {
    use crate::signing::verify_ed25519_signature;

    let manifest_path = out_dir.join(RELEASE_MANIFEST_FILE);
    let sig_path = out_dir.join(RELEASE_MANIFEST_SIG_FILE);

    if !manifest_path.is_file() {
        return Err(format!(
            "release manifest `{}` not found; run `xtask release-manifest` first",
            manifest_path.display()
        ));
    }
    if !sig_path.is_file() {
        return Err(format!(
            "manifest signature `{}` not found; the manifest was produced unsigned-beta",
            sig_path.display()
        ));
    }

    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "unable to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let sig_bytes = fs::read(&sig_path)
        .map_err(|err| format!("unable to read signature `{}`: {err}", sig_path.display()))?;

    verify_ed25519_signature(&manifest_bytes, &sig_bytes, vk_bytes)
        .map_err(|err| format!("manifest signature verification failed: {err}"))
}

pub fn compute_sha256_file(path: &Path) -> Result<String, String> {
    use sha2::Digest as _;
    let bytes = fs::read(path)
        .map_err(|err| format!("unable to read `{}` for sha256: {err}", path.display()))?;
    let hash = sha2::Sha256::digest(&bytes);
    Ok(hex::encode(hash))
}

fn read_written_version_stamp(out_dir: &Path) -> Result<VersionStamp, String> {
    let stamp_path = out_dir.join(VERSION_STAMP_FILE);
    let stamp_text = fs::read_to_string(&stamp_path).map_err(|err| {
        format!(
            "unable to read release pipeline version stamp `{}`: {err}",
            stamp_path.display()
        )
    })?;
    toml::from_str(&stamp_text).map_err(|err| {
        format!(
            "unable to parse release pipeline version stamp `{}`: {err}",
            stamp_path.display()
        )
    })
}

fn build_version_stamp(
    package_name: &str,
    package_version: &str,
    channel: ReleaseChannel,
    dist_tool: &str,
    workspace_root: &Path,
) -> Result<VersionStamp, String> {
    Ok(VersionStamp {
        schema_version: 1,
        package_name: package_name.to_string(),
        package_version: package_version.to_string(),
        channel: channel.as_str().to_string(),
        rollout_policy: channel_rollout_policy(channel).to_string(),
        dist_tool: dist_tool.to_string(),
        git_sha: resolve_workspace_git_sha(workspace_root),
        built_at_utc: current_utc_rfc3339(),
    })
}

fn resolve_workspace_git_sha(workspace_root: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args(["rev-parse", "HEAD"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if sha.is_empty() {
                "unknown".to_string()
            } else {
                sha
            }
        }
        _ => "unknown".to_string(),
    }
}

fn current_utc_rfc3339() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86_400;
    let secs_of_day = secs % 86_400;
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z",)
}

/// Howard Hinnant's `civil_from_days` algorithm. Returns (year, month, day)
/// for the given count of days since the Unix epoch (1970-01-01).
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i32 + (era as i32) * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn workspace_version(workspace_root: &Path) -> Result<String, String> {
    let manifest_path = workspace_root.join("Cargo.toml");
    let text = fs::read_to_string(&manifest_path).map_err(|err| {
        format!(
            "unable to read workspace manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let parsed: toml::Value = toml::from_str(&text).map_err(|err| {
        format!(
            "unable to parse workspace manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    parsed
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| "workspace manifest is missing [workspace.package].version".to_string())
}

fn descriptor_file_stem(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}
