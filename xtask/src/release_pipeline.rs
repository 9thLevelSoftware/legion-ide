use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

pub const DRY_RUN_SIGNER_STATUS: &str = "dry-run/no-production-signer";
pub const VERSION_STAMP_FILE: &str = "version_stamp.toml";
pub const VERIFY_REPORT_FILE: &str = "verify_report.toml";
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
}

impl Default for ReleasePipelineConfig {
    fn default() -> Self {
        Self {
            package_name: "legion-desktop".to_string(),
            dist_tool: "cargo-dist".to_string(),
            installer_targets: Vec::new(),
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

pub fn plan_release_pipeline(
    workspace_root: &Path,
    config: &ReleasePipelineConfig,
    channel: ReleaseChannel,
    dry_run: bool,
) -> Result<ReleasePipelinePlan, String> {
    if !workspace_root.exists() {
        return Err(format!(
            "workspace root `{}` does not exist",
            workspace_root.display()
        ));
    }
    if !dry_run {
        return Err("release pipeline currently supports descriptor dry-run only".to_string());
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
        &workspace_version,
        channel,
        &config.dist_tool,
        workspace_root,
    )?;

    let mut descriptors = config
        .installer_targets
        .iter()
        .map(|target| InstallerDescriptor {
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
            signer_status: DRY_RUN_SIGNER_STATUS.to_string(),
            sha256: "pending".to_string(),
            sha256_status: "dry-run descriptor only; artifact hash is unavailable until build"
                .to_string(),
            version_stamp: version_stamp.clone(),
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
/// written file, and record the verifier status. Until real artifacts exist,
/// the verifier reports `dry-run/unchecked` for every entry — the same fail
/// posture as the dry-run signer status. The signature is stable so that
/// WS17.T2 (signing) and WS17.T3 (auto-update) can replace the body with real
/// SHA-256 / signature checks without changing callers.
pub fn verify_descriptors(
    _workspace_root: &Path,
    plan: &ReleasePipelinePlan,
    out_dir: &Path,
) -> Result<VerificationReport, String> {
    if !out_dir.is_dir() {
        return Err(format!(
            "release pipeline output dir `{}` does not exist; run `write_descriptors` first",
            out_dir.display()
        ));
    }

    let mut entries = Vec::with_capacity(plan.descriptors.len());
    let mut summary = VerificationSummary::default();
    for descriptor in &plan.descriptors {
        let descriptor_path =
            out_dir.join(format!("{}.toml", descriptor_file_stem(&descriptor.name)));
        let exists = descriptor_path.is_file();
        let (verifier_status, verifier_message) = if exists {
            (
                DRY_RUN_VERIFIER_STATUS.to_string(),
                DRY_RUN_VERIFIER_MESSAGE.to_string(),
            )
        } else {
            (
                "failed/missing-descriptor".to_string(),
                format!(
                    "expected descriptor file `{}` was not written by `write_descriptors`",
                    descriptor_path.display()
                ),
            )
        };
        if verifier_status == DRY_RUN_VERIFIER_STATUS {
            summary.unchecked += 1;
        } else {
            summary.failed += 1;
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
