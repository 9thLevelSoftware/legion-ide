use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use xtask::docs_hygiene::{DocsHygieneConfig, normalize_relative_target, run_docs_hygiene};

struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("legion-docs-hygiene-{name}-{stamp}"));
        fs::create_dir_all(&root).expect("create temp repo root");
        Self { root }
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, text).expect("write fixture file");
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

#[test]
fn docs_hygiene_passes_for_clean_minimal_repo() {
    let repo = TempRepo::new("clean-minimal");
    repo.write(
        "README.md",
        "# Test\n\n- `plans/legion-production-master-plan-v0.2.md` - current production master plan.\n",
    );
    repo.write("plans/legion-production-master-plan-v0.2.md", "# Plan\n");

    run_docs_hygiene(&repo.root, &DocsHygieneConfig::default())
        .expect("clean minimal repo should pass docs hygiene");
}

#[test]
fn normalize_relative_target_strips_anchors_and_line_suffixes() {
    assert_eq!(
        normalize_relative_target("plans/file.md:123"),
        Some("plans/file.md".to_string())
    );
    assert_eq!(
        normalize_relative_target("crates/legion-app/src/lib.rs:123-130"),
        Some("crates/legion-app/src/lib.rs".to_string())
    );
    assert_eq!(
        normalize_relative_target("docs/MODES.md#manual"),
        Some("docs/MODES.md".to_string())
    );
    assert_eq!(
        normalize_relative_target("https://example.com/file.md"),
        None
    );
    assert_eq!(normalize_relative_target("#local-anchor"), None);
}

#[test]
fn docs_hygiene_reports_broken_relative_markdown_link() {
    let repo = TempRepo::new("broken-link");
    repo.write("README.md", "# Test\n\nSee [missing](docs/missing.md).\n");

    let result = run_docs_hygiene(&repo.root, &DocsHygieneConfig::default());
    let violations = result.expect_err("expected broken link violation");

    assert!(
        violations.iter().any(|violation| {
            violation.message.contains("docs/missing.md") && violation.line == 3
        })
    );
}

#[test]
fn docs_hygiene_accepts_existing_relative_markdown_link_with_line_suffix() {
    let repo = TempRepo::new("line-suffix-link");
    repo.write(
        "README.md",
        "# Test\n\n- `plans/legion-production-master-plan-v0.2.md` - current production master plan.\n\nSee [source](crates/legion-app/src/lib.rs:123).\n",
    );
    repo.write("plans/legion-production-master-plan-v0.2.md", "# Plan\n");
    repo.write("crates/legion-app/src/lib.rs", "pub fn example() {}\n");

    run_docs_hygiene(&repo.root, &DocsHygieneConfig::default())
        .expect("existing source link with line suffix should pass");
}

#[test]
fn docs_hygiene_reports_unallowlisted_devil_reference() {
    let repo = TempRepo::new("devil-reference");
    repo.write("README.md", "# Test\n\nRun `cargo test -p devil-app`.\n");

    let result = run_docs_hygiene(&repo.root, &DocsHygieneConfig::default());
    let violations = result.expect_err("expected stale devil reference violation");

    assert!(
        violations
            .iter()
            .any(|violation| { violation.message.contains("devil-app") && violation.line == 3 })
    );
}

#[test]
fn docs_hygiene_reports_stale_legacy_mode_label_as_section() {
    let repo = TempRepo::new("legacy-mode-section");
    repo.write(
        "docs/MODES.md",
        "# Modes\n\n## Manual\n\nManual mode.\n\n## Automate\n\nAutomate mode is legacy.\n",
    );

    let result = run_docs_hygiene(&repo.root, &DocsHygieneConfig::default());
    let violations = result.expect_err("expected stale Automate mode section violation");

    assert!(
        violations.iter().any(|violation| {
            violation.message.contains("stale mode-taxonomy section")
                && violation.message.contains("Automate")
        }),
        "expected violation mentioning stale Automate section, got: {:?}",
        violations
            .iter()
            .map(|v| format!("{}:{}: {}", v.path.display(), v.line, v.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn docs_hygiene_reports_stale_delegates_label_as_section() {
    let repo = TempRepo::new("legacy-delegates-section");
    repo.write(
        "docs/MODES.md",
        "# Modes\n\n## Manual\n\n## Delegates\n\nOld wording.\n",
    );

    let result = run_docs_hygiene(&repo.root, &DocsHygieneConfig::default());
    let violations = result.expect_err("expected stale Delegates section violation");

    assert!(
        violations.iter().any(|violation| {
            violation.message.contains("stale mode-taxonomy section")
                && violation.message.contains("Delegates")
        }),
        "expected violation mentioning stale Delegates section, got: {:?}",
        violations
            .iter()
            .map(|v| format!("{}:{}: {}", v.path.display(), v.line, v.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn docs_hygiene_accepts_canonical_mode_sections() {
    let repo = TempRepo::new("canonical-mode-sections");
    repo.write(
        "docs/MODES.md",
        "# Modes\n\n## Manual\n\n## Assist\n\n## Delegate\n\n## Legion Workflows\n",
    );

    run_docs_hygiene(&repo.root, &DocsHygieneConfig::default())
        .expect("all four canonical mode sections must pass");
}

#[test]
fn docs_hygiene_allows_stale_mode_label_in_allowlisted_archive() {
    let repo = TempRepo::new("allowlist-mode-archive");
    repo.write(
        "plans/archive/old-modes.md",
        "# Old\n\n## Automate\n\nOld mode taxonomy.\n",
    );
    let config = DocsHygieneConfig {
        allowlisted_paths: vec!["plans/archive/".to_string()],
    };

    run_docs_hygiene(&repo.root, &config)
        .expect("allowlisted historical path should be allowed to keep legacy mode labels");
}

#[test]
fn docs_hygiene_allows_devil_reference_in_allowlisted_path() {
    let repo = TempRepo::new("allowlisted-devil-reference");
    repo.write(
        "plans/archive/old.md",
        "# Old\n\nHistorical `devil-app` transcript.\n",
    );
    let config = DocsHygieneConfig {
        allowlisted_paths: vec!["plans/archive/".to_string()],
    };

    run_docs_hygiene(&repo.root, &config).expect("allowlisted historical path should pass");
}

#[test]
fn docs_hygiene_requires_readme_to_reference_latest_production_master_plan() {
    let repo = TempRepo::new("readme-latest-production-plan");
    repo.write(
        "README.md",
        "# Test\n\n- `plans/legion-production-master-plan-v0.1.md` - current production master plan.\n",
    );

    let result = run_docs_hygiene(&repo.root, &DocsHygieneConfig::default());
    let violations = result.expect_err("expected stale production plan reference violation");

    assert!(violations.iter().any(|violation| {
        violation.path == Path::new("README.md")
            && violation
                .message
                .contains("legion-production-master-plan-v0.2.md")
    }));
}

#[test]
fn docs_hygiene_requires_docs_index_to_reference_latest_production_master_plan() {
    let repo = TempRepo::new("index-latest-production-plan");
    repo.write(
        "README.md",
        "# Test\n\n- `plans/legion-production-master-plan-v0.2.md` - current production master plan.\n",
    );
    repo.write(
        "docs/INDEX.md",
        "# Index\n\n- `../plans/legion-production-master-plan-v0.1.md` - historical production master plan.\n",
    );

    let result = run_docs_hygiene(&repo.root, &DocsHygieneConfig::default());
    let violations = result.expect_err("expected docs index latest-plan violation");

    assert!(violations.iter().any(|violation| {
        violation.path == Path::new("docs/INDEX.md")
            && violation
                .message
                .contains("legion-production-master-plan-v0.2.md")
    }));
}

#[test]
fn docs_hygiene_accepts_current_production_master_plan_entrypoints() {
    let repo = TempRepo::new("current-production-plan-entrypoints");
    repo.write(
        "README.md",
        "# Test\n\n- `plans/legion-production-master-plan-v0.2.md` - current production master plan.\n- `plans/legion-production-master-plan-v0.1.md` - historical production master plan.\n",
    );
    repo.write(
        "docs/INDEX.md",
        "# Index\n\n- `../plans/legion-production-master-plan-v0.2.md` - current production master plan.\n- `../plans/legion-production-master-plan-v0.1.md` - historical production master plan.\n",
    );

    run_docs_hygiene(&repo.root, &DocsHygieneConfig::default())
        .expect("current production plan entrypoints should pass");
}

#[test]
fn docs_hygiene_loads_allowlist_from_toml() {
    let repo = TempRepo::new("allowlist-toml");
    repo.write(
        "docs/hygiene-allowlist.toml",
        "allowlisted_paths = [\"plans/archive/\"]\n",
    );
    repo.write("plans/archive/old.md", "Historical `devil-app`.\n");

    let config = DocsHygieneConfig::from_file(&repo.path("docs/hygiene-allowlist.toml"))
        .expect("load allowlist");

    assert_eq!(config.allowlisted_paths, vec!["plans/archive/".to_string()]);
    run_docs_hygiene(&repo.root, &config).expect("loaded allowlist should apply");
}

#[test]
fn docs_hygiene_skips_git_target_almanac_directories() {
    let repo = TempRepo::new("skip-dirs");
    repo.write(".git/HEAD", "ref: refs/heads/main\n");
    repo.write("target/some.md", "unused\n");
    repo.write(".almanac/cache.md", "unused\n");
    repo.write(".hermes/plans/local.md", "unused\n");
    repo.write(".serena/memory.md", "unused\n");
    repo.write(
        "README.md",
        "# Visible\n\n- `plans/legion-production-master-plan-v0.2.md` - current production master plan.\n",
    );
    repo.write("plans/legion-production-master-plan-v0.2.md", "# Plan\n");

    let result = run_docs_hygiene(&repo.root, &DocsHygieneConfig::default());
    // Should pass: skipped dirs not visited, README has no broken link/devil marker
    result.expect("expected no violations because the only visible file is clean");
    let _ = Path::new("unused-skip");
}

#[test]
fn docs_hygiene_checks_untracked_markdown_in_git_repo() {
    let repo = TempRepo::new("git-untracked-markdown");
    let init = Command::new("git")
        .arg("init")
        .arg(&repo.root)
        .output()
        .expect("git init should run");
    assert!(
        init.status.success(),
        "git init should succeed; stdout={} stderr={}",
        String::from_utf8_lossy(&init.stdout),
        String::from_utf8_lossy(&init.stderr)
    );
    repo.write(
        "README.md",
        "# Clean\n\n- `plans/legion-production-master-plan-v0.2.md` - current production master plan.\n",
    );
    repo.write("plans/legion-production-master-plan-v0.2.md", "# Plan\n");
    let add = Command::new("git")
        .arg("-C")
        .arg(&repo.root)
        .arg("add")
        .arg("README.md")
        .output()
        .expect("git add should run");
    assert!(
        add.status.success(),
        "git add should succeed; stdout={} stderr={}",
        String::from_utf8_lossy(&add.stdout),
        String::from_utf8_lossy(&add.stderr)
    );
    repo.write("NEW.md", "# New\n\nStale `devil-app` marker.\n");
    repo.write(
        ".almanac/page.md",
        "# Local wiki\n\nStale `devil-app` marker.\n",
    );

    let result = run_docs_hygiene(&repo.root, &DocsHygieneConfig::default());
    let violations = result.expect_err("untracked NEW.md should be checked");

    assert!(violations.iter().any(|violation| {
        violation.path == Path::new("NEW.md") && violation.message.contains("devil-app")
    }));
    assert!(
        violations
            .iter()
            .all(|violation| !violation.path.starts_with(".almanac"))
    );
}
