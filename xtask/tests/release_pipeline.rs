use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use xtask::release_pipeline::{
    DRY_RUN_SIGNER_STATUS, InstallerTargetConfig, ReleaseChannel, ReleasePipelineConfig,
    VersionStamp, channel_rollout_policy, plan_release_pipeline, verify_descriptors,
    write_descriptors,
};

struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    fn new(name: &str) -> Self {
        // Process id + monotonic counter keeps the temp dir unique even when
        // tests run in parallel within the same nanosecond (clock granularity
        // can be coarser than nanoseconds on some hosts).
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let pid = std::process::id();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "legion-release-pipeline-{name}-{pid}-{stamp}-{seq}"
        ));
        fs::create_dir_all(&root).expect("create temp repo root");
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"0.1.0\"\n",
        )
        .expect("write workspace manifest");
        Self { root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn test_config() -> ReleasePipelineConfig {
    ReleasePipelineConfig {
        package_name: "legion-desktop".to_string(),
        dist_tool: "cargo-dist".to_string(),
        installer_targets: vec![
            InstallerTargetConfig {
                name: "legion-desktop-linux-x64-deb".to_string(),
                platform: "linux".to_string(),
                target: "x86_64-unknown-linux-gnu".to_string(),
                artifact: "deb".to_string(),
                build_command: "cargo dist build --target x86_64-unknown-linux-gnu".to_string(),
                verification_command: "dpkg-deb --info <artifact>".to_string(),
            },
            InstallerTargetConfig {
                name: "legion-desktop-windows-x64-msi".to_string(),
                platform: "windows".to_string(),
                target: "x86_64-pc-windows-msvc".to_string(),
                artifact: "msi".to_string(),
                build_command: "cargo dist build --target x86_64-pc-windows-msvc".to_string(),
                verification_command: "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-windows.ps1 -DryRun".to_string(),
            },
        ],
    }
}

#[test]
fn release_pipeline_plan_is_deterministic_for_same_inputs() {
    let repo = TempRepo::new("deterministic");
    let config = test_config();

    let first = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, true)
        .expect("plan first release pipeline");
    let second = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, true)
        .expect("plan second release pipeline");

    assert_eq!(first, second);
}

#[test]
fn release_pipeline_descriptors_use_dry_run_signer_and_pending_sha256() {
    let repo = TempRepo::new("dry-run");
    let config = test_config();

    let plan = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, true)
        .expect("plan release pipeline");

    assert_eq!(plan.descriptors.len(), 2);
    for descriptor in &plan.descriptors {
        assert_eq!(descriptor.signer_status, DRY_RUN_SIGNER_STATUS);
        assert_eq!(descriptor.sha256, "pending");
        assert!(descriptor.sha256_status.contains("dry-run"));
        assert!(descriptor.build_command.starts_with("cargo dist build"));
        assert!(!descriptor.verification_command.is_empty());
    }
}

#[test]
fn release_pipeline_preview_channel_changes_version_label_only() {
    let repo = TempRepo::new("preview");
    let config = test_config();

    let stable = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, true)
        .expect("plan stable release pipeline");
    let preview = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Preview, true)
        .expect("plan preview release pipeline");

    assert_eq!(stable.descriptors.len(), preview.descriptors.len());
    assert_eq!(stable.descriptors[0].version, "0.1.0");
    assert_eq!(preview.descriptors[0].version, "0.1.0-preview");
    assert_eq!(stable.descriptors[0].target, preview.descriptors[0].target);
    assert_eq!(
        stable.descriptors[0].build_command,
        preview.descriptors[0].build_command
    );
}

#[test]
fn release_pipeline_write_descriptors_is_idempotent() {
    let repo = TempRepo::new("write");
    let config = test_config();
    let plan = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, true)
        .expect("plan release pipeline");
    let out_dir = repo.path("target/release-pipeline");

    let first = write_descriptors(&plan, &out_dir).expect("write descriptors");
    let first_contents = first
        .iter()
        .map(|path| fs::read_to_string(path).expect("read first descriptor"))
        .collect::<Vec<_>>();
    let second = write_descriptors(&plan, &out_dir).expect("rewrite descriptors");
    let second_contents = second
        .iter()
        .map(|path| fs::read_to_string(path).expect("read second descriptor"))
        .collect::<Vec<_>>();

    assert_eq!(first, second);
    assert_eq!(first_contents, second_contents);
}

#[test]
fn release_pipeline_write_descriptors_rejects_file_name_collision() {
    let repo = TempRepo::new("collision");
    // Two distinct installer-target names that normalize to the same
    // descriptor file stem (`legion-desktop-linux-x64`).
    let installer = |name: &str| InstallerTargetConfig {
        name: name.to_string(),
        platform: "linux".to_string(),
        target: "x86_64-unknown-linux-gnu".to_string(),
        artifact: "deb".to_string(),
        build_command: "cargo dist build --target x86_64-unknown-linux-gnu".to_string(),
        verification_command: "dpkg-deb --info <artifact>".to_string(),
    };
    let config = ReleasePipelineConfig {
        package_name: "legion-desktop".to_string(),
        dist_tool: "cargo-dist".to_string(),
        installer_targets: vec![
            installer("legion-desktop linux x64"),
            installer("legion-desktop-linux-x64"),
        ],
    };

    let plan = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, true)
        .expect("plan release pipeline");
    let out_dir = repo.path("target/release-pipeline");

    let err = write_descriptors(&plan, &out_dir)
        .expect_err("colliding descriptor file names should be rejected");
    assert!(err.contains("collision"), "unexpected error: {err}");
}

#[test]
fn release_pipeline_rejects_non_dry_run_until_signing_policy_exists() {
    let repo = TempRepo::new("rejects-non-dry-run");
    let config = test_config();

    let error = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, false)
        .expect_err("non-dry-run release planning should be blocked");

    assert!(error.contains("dry-run only"));
}

#[test]
fn release_pipeline_loads_config_from_toml() {
    let repo = TempRepo::new("config");
    let config_path = repo.path("xtask/release-pipeline.example.toml");
    fs::create_dir_all(config_path.parent().expect("config parent")).expect("create config dir");
    fs::write(
        &config_path,
        "package_name = \"legion-desktop\"\ndist_tool = \"cargo-dist\"\n\n[[installer_targets]]\nname = \"legion-desktop-linux-x64-deb\"\nplatform = \"linux\"\ntarget = \"x86_64-unknown-linux-gnu\"\nartifact = \"deb\"\nbuild_command = \"cargo dist build --target x86_64-unknown-linux-gnu\"\nverification_command = \"dpkg-deb --info <artifact>\"\n",
    )
    .expect("write release pipeline config");

    let config = ReleasePipelineConfig::from_file(&config_path).expect("parse release config");

    assert_eq!(config.package_name, "legion-desktop");
    assert_eq!(config.dist_tool, "cargo-dist");
    assert_eq!(config.installer_targets.len(), 1);
}

#[test]
fn release_pipeline_preview_channel_changes_rollout_policy() {
    assert_eq!(channel_rollout_policy(ReleaseChannel::Stable), "full");
    assert_eq!(channel_rollout_policy(ReleaseChannel::Preview), "staged");
}

#[test]
fn release_pipeline_records_reproducible_version_stamp() {
    let repo = TempRepo::new("version-stamp");
    let _ = init_temp_git_repo(&repo.root);
    let config = test_config();

    let plan = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, true)
        .expect("plan release pipeline");

    for descriptor in &plan.descriptors {
        let stamp = &descriptor.version_stamp;
        assert_eq!(stamp.package_version, "0.1.0");
        assert_eq!(stamp.channel, "stable");
        assert_eq!(stamp.dist_tool, "cargo-dist");
        assert!(
            stamp.git_sha.len() == 40 && stamp.git_sha.chars().all(|ch| ch.is_ascii_hexdigit()),
            "git_sha should be a 40-char hex commit, got {:?}",
            stamp.git_sha
        );
        assert!(
            stamp.built_at_utc.starts_with("1970-")
                || stamp.built_at_utc.starts_with("202")
                || stamp.built_at_utc.starts_with("203"),
            "built_at_utc should be an RFC3339 timestamp, got {:?}",
            stamp.built_at_utc
        );
        assert_eq!(stamp.rollout_policy, "full");
    }
}

#[test]
fn release_pipeline_preview_channel_overrides_rollout_policy_in_stamp() {
    let repo = TempRepo::new("preview-stamp");
    let config = test_config();

    let plan = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Preview, true)
        .expect("plan release pipeline");

    for descriptor in &plan.descriptors {
        assert_eq!(descriptor.version_stamp.channel, "preview");
        assert_eq!(descriptor.version_stamp.rollout_policy, "staged");
    }
}

#[test]
fn release_pipeline_stamp_git_sha_is_workspace_head() {
    let repo = TempRepo::new("stamp-git");
    fs::create_dir_all(repo.path(".git-keep")).expect("init temp dir");
    let head = init_temp_git_repo(&repo.root).expect("init temp git repo");

    let config = test_config();
    let plan = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, true)
        .expect("plan release pipeline");

    for descriptor in &plan.descriptors {
        assert_eq!(descriptor.version_stamp.git_sha, head);
    }
}

fn init_temp_git_repo(root: &Path) -> Option<String> {
    let run = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .ok()
            .filter(|out| out.status.success())
    };
    run(&["init", "--quiet", "--initial-branch=main"])?;
    // Local committer required for `git commit` to succeed in a CI temp dir.
    run(&["config", "user.email", "[email protected]"])?;
    run(&["config", "user.name", "Legion Release Pipeline Test"])?;
    fs::write(root.join("README.md"), "fixture\n").expect("write readme");
    run(&["add", "README.md"])?;
    run(&["commit", "--quiet", "-m", "fixture"])?;
    head_short_sha(root)
}

#[test]
fn release_pipeline_written_descriptors_round_trip_version_stamp() {
    let repo = TempRepo::new("round-trip-stamp");
    let config = test_config();

    let plan = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, true)
        .expect("plan release pipeline");
    let out_dir = repo.path("target/release-pipeline");
    let written = write_descriptors(&plan, &out_dir).expect("write descriptors");

    let stamp_text = fs::read_to_string(
        written
            .iter()
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name == "version_stamp.toml")
            })
            .expect("version_stamp.toml should be written"),
    )
    .expect("read version stamp");
    let stamp: VersionStamp = toml::from_str(&stamp_text).expect("parse version stamp");

    assert_eq!(stamp.package_version, "0.1.0");
    assert_eq!(stamp.channel, "stable");
    assert!(stamp.built_at_utc.contains('T'));
    assert!(!stamp.git_sha.is_empty());
}

#[test]
fn release_pipeline_verify_descriptors_marks_dry_run_verifiers_unchecked() {
    let repo = TempRepo::new("verify-dry");
    let config = test_config();

    let plan = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, true)
        .expect("plan release pipeline");
    let out_dir = repo.path("target/release-pipeline");
    write_descriptors(&plan, &out_dir).expect("write descriptors");

    let report = verify_descriptors(&repo.root, &plan, &out_dir).expect("verify descriptors");

    assert_eq!(report.descriptors.len(), plan.descriptors.len());
    for entry in &report.descriptors {
        assert_eq!(entry.signer_status, DRY_RUN_SIGNER_STATUS);
        assert_eq!(entry.verifier_status, "dry-run/unchecked");
        assert_eq!(entry.sha256, "pending");
        assert!(
            entry.verifier_message.contains("dry-run"),
            "verifier_message should explain dry-run status, got {:?}",
            entry.verifier_message
        );
    }
    assert!(
        report
            .descriptors
            .iter()
            .all(|entry| entry.descriptor_path.is_file()),
        "all referenced descriptor paths should exist on disk"
    );
}

#[test]
fn release_pipeline_verify_descriptors_uses_written_version_stamp() {
    let repo = TempRepo::new("verify-version-stamp");
    let config = test_config();

    let plan = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, true)
        .expect("plan release pipeline");
    let out_dir = repo.path("target/release-pipeline");
    write_descriptors(&plan, &out_dir).expect("write descriptors");

    let mut stale_plan = plan.clone();
    stale_plan.version_stamp.built_at_utc = "2099-01-01T00:00:00Z".to_string();
    for descriptor in &mut stale_plan.descriptors {
        descriptor.version_stamp.built_at_utc = stale_plan.version_stamp.built_at_utc.clone();
    }

    let report = verify_descriptors(&repo.root, &stale_plan, &out_dir).expect("verify descriptors");

    assert_eq!(report.summary.failed, 0);
    assert_eq!(report.summary.unchecked, stale_plan.descriptors.len());
    assert_eq!(report.summary.total, stale_plan.descriptors.len());
}

#[test]
fn release_pipeline_verify_descriptors_rejects_tampered_descriptor_bytes() {
    let repo = TempRepo::new("verify-tamper");
    let config = test_config();

    let plan = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Stable, true)
        .expect("plan release pipeline");
    let out_dir = repo.path("target/release-pipeline");
    let written = write_descriptors(&plan, &out_dir).expect("write descriptors");

    let tampered_path = written
        .iter()
        .find(|path| {
            path.extension().is_some_and(|ext| ext == "toml")
                && path
                    .file_name()
                    .is_some_and(|name| name != "version_stamp.toml")
        })
        .expect("descriptor path should exist")
        .clone();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&tampered_path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, b"\n# tampered\n"))
        .expect("tamper descriptor bytes");

    let report = verify_descriptors(&repo.root, &plan, &out_dir).expect("verify descriptors");
    let tampered_entry = report
        .descriptors
        .iter()
        .find(|entry| entry.descriptor_path == tampered_path)
        .expect("tampered entry should be reported");

    assert_eq!(tampered_entry.verifier_status, "failed/tampered-descriptor");
    assert!(
        tampered_entry
            .verifier_message
            .contains("integrity comparison")
    );
    assert_eq!(report.summary.failed, 1);
    assert_eq!(report.summary.unchecked, plan.descriptors.len() - 1);
    assert_eq!(report.summary.total, plan.descriptors.len());
}

#[test]
fn release_pipeline_verify_descriptors_aggregates_summary_counts() {
    let repo = TempRepo::new("verify-summary");
    let config = test_config();

    let plan = plan_release_pipeline(&repo.root, &config, ReleaseChannel::Preview, true)
        .expect("plan release pipeline");
    let out_dir = repo.path("target/release-pipeline");
    write_descriptors(&plan, &out_dir).expect("write descriptors");

    let report = verify_descriptors(&repo.root, &plan, &out_dir).expect("verify descriptors");

    assert_eq!(report.summary.total, plan.descriptors.len());
    assert_eq!(report.summary.unchecked, plan.descriptors.len());
    assert_eq!(report.summary.failed, 0);
    assert_eq!(report.summary.passed, 0);
    assert_eq!(report.channel, "preview");
    assert!(report.verified_at_utc.contains('T'));
}

fn head_short_sha(workspace_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let sha = String::from_utf8(output.stdout).ok()?;
    let sha = sha.trim();
    if sha.is_empty() {
        return None;
    }
    Some(sha.to_string())
}
