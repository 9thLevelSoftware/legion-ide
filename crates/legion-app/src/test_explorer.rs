//! Test explorer discovery + per-item run (P2.F3.T4).
//!
//! Discovers Cargo tests via `cargo test -- --list` and runs a single filtered
//! item via `cargo test -- --exact <id>`. Projections stay metadata-only
//! (counts/exit codes/labels; no raw test logs).

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use legion_protocol::{
    ProposalPrivacyLabel, ProposalRiskLabel, RedactionHint, TimestampMillis, VerificationRunRow,
    VerificationRunState,
};
use legion_ui::{TestExplorerItemProjection, TestExplorerProjection};

/// Default hard timeout for discovery subprocesses.
pub const DEFAULT_DISCOVER_TIMEOUT: Duration = Duration::from_secs(60);

/// Default hard timeout for a single filtered test run.
pub const DEFAULT_RUN_TIMEOUT: Duration = Duration::from_secs(120);

/// Maximum items retained in the projection (metadata-bounded).
pub const MAX_PROJECTED_ITEMS: usize = 500;

/// Maximum retained per-item run rows for the verification projection.
pub const MAX_RECENT_RUNS: usize = 20;

/// Maximum stdout bytes retained transiently for summary parsing (not projected).
const MAX_PARSE_STDOUT_BYTES: usize = 16 * 1024;

/// Parse `cargo test -- --list` stdout into projection items.
///
/// Lines look like `module::test_name: test` or `module::bench_name: bench`.
/// Summary lines (`N tests, M benchmarks`) and blanks are ignored.
pub fn parse_cargo_test_list(stdout: &str) -> Vec<TestExplorerItemProjection> {
    let mut items = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.contains(" test") && line.contains("benchmark") {
            // e.g. "12 tests, 0 benchmarks"
            continue;
        }
        let Some((name, kind)) = line.rsplit_once(':') else {
            continue;
        };
        let kind = kind.trim();
        if kind != "test" && kind != "bench" {
            continue;
        }
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let parent_label = name.rsplit_once("::").map(|(parent, _)| parent.to_string());
        let label = name
            .rsplit_once("::")
            .map(|(_, leaf)| leaf.to_string())
            .unwrap_or_else(|| name.to_string());
        items.push(TestExplorerItemProjection {
            item_id: name.to_string(),
            label,
            kind_label: kind.to_string(),
            parent_label,
        });
        if items.len() >= MAX_PROJECTED_ITEMS {
            break;
        }
    }
    items
}

/// Build a projection from parsed items and discovery metadata.
pub fn projection_from_items(
    items: Vec<TestExplorerItemProjection>,
    diagnostics: Vec<String>,
    status_label: impl Into<String>,
    generated_at: TimestampMillis,
) -> TestExplorerProjection {
    let mut items = items;
    let mut diagnostics = diagnostics;
    if items.len() > MAX_PROJECTED_ITEMS {
        let omitted = items.len() - MAX_PROJECTED_ITEMS;
        items.truncate(MAX_PROJECTED_ITEMS);
        diagnostics.push(format!("omitted_items={omitted}"));
    }
    TestExplorerProjection {
        status_label: status_label.into(),
        controller_label: "cargo-test".to_string(),
        items,
        diagnostics,
        last_run_item_id: None,
        last_run_status: None,
        last_run_exit_code: None,
        last_run_duration_ms: None,
        generated_at,
        schema_version: 1,
    }
}

/// Fail closed on item ids that could be shell-dangerous or empty.
///
/// Cargo test paths are expected to be identifiers with `::` separators.
pub fn validate_test_item_id(item_id: &str) -> Result<(), String> {
    let id = item_id.trim();
    if id.is_empty() {
        return Err("empty-item-id".to_string());
    }
    if id.len() > 512 {
        return Err("item-id-too-long".to_string());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '.')
    {
        return Err("item-id-invalid-chars".to_string());
    }
    if id.contains(":::") || id.starts_with(':') || id.ends_with(':') {
        return Err("item-id-invalid-shape".to_string());
    }
    Ok(())
}

/// Metadata-only result of a single filtered cargo test run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoTestItemRunResult {
    /// Item id that was requested.
    pub item_id: String,
    /// Status label (`passed`, `failed`, `timeout`, `error`, `empty`).
    pub status_label: String,
    /// Process exit code when available.
    pub exit_code: Option<i32>,
    /// Passed count from cargo summary when parsed.
    pub passed: u32,
    /// Failed count from cargo summary when parsed.
    pub failed: u32,
    /// Skipped/ignored count from cargo summary when parsed.
    pub skipped: u32,
    /// Wall-clock duration.
    pub duration_ms: u64,
    /// Display-safe diagnostics.
    pub diagnostics: Vec<String>,
}

impl CargoTestItemRunResult {
    /// Map to a metadata-only verification run row (no raw command body).
    pub fn to_verification_row(&self, started_at: TimestampMillis) -> VerificationRunRow {
        let state = match self.status_label.as_str() {
            "passed" => VerificationRunState::Passed,
            "failed" => VerificationRunState::Failed,
            "timeout" => VerificationRunState::Blocked,
            "empty" => VerificationRunState::Blocked,
            _ => VerificationRunState::Failed,
        };
        let completed_at = TimestampMillis(started_at.0.saturating_add(self.duration_ms));
        VerificationRunRow {
            run_id: format!("test-explorer:{}:{}", self.item_id, started_at.0),
            label: format!("cargo-test exact {}", self.item_id),
            state,
            command_class_label: "cargo-test-exact".to_string(),
            command_body_redacted: true,
            exit_code: self.exit_code,
            target_labels: vec![self.item_id.clone()],
            evidence_artifact_id: None,
            started_at: Some(started_at),
            completed_at: Some(completed_at),
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }
    }
}

/// Parse cargo's `test result: ...` summary line for counts.
pub fn parse_cargo_test_summary(stdout: &str) -> (u32, u32, u32, bool) {
    for line in stdout.lines().rev() {
        let line = line.trim();
        if !line.starts_with("test result:") {
            continue;
        }
        // test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
        let passed = extract_count(line, "passed");
        let failed = extract_count(line, "failed");
        let skipped = extract_count(line, "ignored");
        return (passed, failed, skipped, true);
    }
    (0, 0, 0, false)
}

fn extract_count(line: &str, label: &str) -> u32 {
    // Match adjacent tokens `N <label...>` anywhere on the summary line.
    let tokens: Vec<&str> = line.split_whitespace().collect();
    for window in tokens.windows(2) {
        if window[1].starts_with(label)
            && let Ok(n) = window[0].parse::<u32>()
        {
            return n;
        }
    }
    0
}

/// Run one discovered test with `cargo test -- --exact <item_id>`.
///
/// Stdio is captured only to parse the summary line; raw output is dropped.
pub fn run_cargo_test_item(
    workspace_root: &Path,
    item_id: &str,
    timeout: Duration,
) -> Result<CargoTestItemRunResult, String> {
    validate_test_item_id(item_id)?;
    if !workspace_root.is_dir() {
        return Ok(CargoTestItemRunResult {
            item_id: item_id.to_string(),
            status_label: "error".to_string(),
            exit_code: None,
            passed: 0,
            failed: 0,
            skipped: 0,
            duration_ms: 0,
            diagnostics: vec!["workspace-root-missing".to_string()],
        });
    }

    let started = Instant::now();
    let mut child = Command::new("cargo")
        .args(["test", "--", "--exact", item_id])
        .current_dir(workspace_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("spawn-failed:{err}"))?;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(CargoTestItemRunResult {
                    item_id: item_id.to_string(),
                    status_label: "timeout".to_string(),
                    exit_code: None,
                    passed: 0,
                    failed: 0,
                    skipped: 0,
                    duration_ms: started.elapsed().as_millis() as u64,
                    diagnostics: vec![format!("timeout-{}s", timeout.as_secs())],
                });
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(err) => {
                return Ok(CargoTestItemRunResult {
                    item_id: item_id.to_string(),
                    status_label: "error".to_string(),
                    exit_code: None,
                    passed: 0,
                    failed: 0,
                    skipped: 0,
                    duration_ms: started.elapsed().as_millis() as u64,
                    diagnostics: vec![format!("wait-failed:{err}")],
                });
            }
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("output-failed:{err}"))?;
    let duration_ms = started.elapsed().as_millis() as u64;
    let exit_code = output.status.code();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let truncated = if stdout.len() > MAX_PARSE_STDOUT_BYTES {
        &stdout[stdout.len() - MAX_PARSE_STDOUT_BYTES..]
    } else {
        &stdout
    };
    let (passed, failed, skipped, has_summary) = parse_cargo_test_summary(truncated);
    let mut diagnostics = Vec::new();
    if !has_summary {
        diagnostics.push("summary-unparsed".to_string());
    }
    if !output.status.success() {
        diagnostics.push(format!("exit-code={}", exit_code.unwrap_or(-1)));
    }

    let status_label = if !output.status.success() || failed > 0 {
        "failed"
    } else if has_summary && passed == 0 && failed == 0 {
        "empty"
    } else if output.status.success() {
        "passed"
    } else {
        "error"
    };

    Ok(CargoTestItemRunResult {
        item_id: item_id.to_string(),
        status_label: status_label.to_string(),
        exit_code,
        passed,
        failed,
        skipped,
        duration_ms,
        diagnostics,
    })
}

/// Apply a run result onto an existing explorer projection (preserves items).
pub fn apply_run_to_projection(
    mut projection: TestExplorerProjection,
    result: &CargoTestItemRunResult,
    generated_at: TimestampMillis,
) -> TestExplorerProjection {
    projection.last_run_item_id = Some(result.item_id.clone());
    projection.last_run_status = Some(result.status_label.clone());
    projection.last_run_exit_code = result.exit_code;
    projection.last_run_duration_ms = Some(result.duration_ms);
    projection.generated_at = generated_at;
    // Keep discovery status; surface run outcome in diagnostics.
    let mut diags = projection.diagnostics;
    diags.retain(|d| !d.starts_with("last-run:"));
    diags.push(format!(
        "last-run:{}:{}:exit={}:p={}:f={}:s={}:{}ms",
        result.item_id,
        result.status_label,
        result
            .exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        result.passed,
        result.failed,
        result.skipped,
        result.duration_ms
    ));
    for d in &result.diagnostics {
        diags.push(format!("last-run-diag:{d}"));
    }
    projection.diagnostics = diags;
    projection
}

/// Prepend a run row and cap recent history.
pub fn push_recent_run(runs: &mut Vec<VerificationRunRow>, row: VerificationRunRow) {
    runs.insert(0, row);
    if runs.len() > MAX_RECENT_RUNS {
        runs.truncate(MAX_RECENT_RUNS);
    }
}

/// Run `cargo test -- --list` in `workspace_root` and project the results.
///
/// Metadata-only: stdout is parsed for names/kinds; raw logs are not retained.
pub fn discover_cargo_tests(
    workspace_root: &Path,
    timeout: Duration,
    generated_at: TimestampMillis,
) -> TestExplorerProjection {
    if !workspace_root.is_dir() {
        return projection_from_items(
            Vec::new(),
            vec!["workspace-root-missing".to_string()],
            "error",
            generated_at,
        );
    }

    let started = Instant::now();
    let mut child = match Command::new("cargo")
        .args(["test", "--", "--list"])
        .current_dir(workspace_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return projection_from_items(
                Vec::new(),
                vec![format!("spawn-failed:{err}")],
                "error",
                generated_at,
            );
        }
    };

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return projection_from_items(
                    Vec::new(),
                    vec![format!("timeout-{}s", timeout.as_secs())],
                    "timeout",
                    generated_at,
                );
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(err) => {
                return projection_from_items(
                    Vec::new(),
                    vec![format!("wait-failed:{err}")],
                    "error",
                    generated_at,
                );
            }
        }
    }

    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(err) => {
            return projection_from_items(
                Vec::new(),
                vec![format!("output-failed:{err}")],
                "error",
                generated_at,
            );
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let items = parse_cargo_test_list(&stdout);
    let mut diagnostics = Vec::new();
    if !output.status.success() {
        diagnostics.push(format!("exit-code={}", output.status.code().unwrap_or(-1)));
    }
    let status = if items.is_empty() && !output.status.success() {
        "error"
    } else if items.is_empty() {
        "empty"
    } else {
        "ready"
    };
    projection_from_items(items, diagnostics, status, generated_at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cargo_test_list_extracts_tests_and_benches() {
        let stdout = r#"
foo::bar_works: test
foo::baz_bench: bench
nested::deep::case: test

3 tests, 1 benchmarks
"#;
        let items = parse_cargo_test_list(stdout);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].item_id, "foo::bar_works");
        assert_eq!(items[0].label, "bar_works");
        assert_eq!(items[0].kind_label, "test");
        assert_eq!(items[0].parent_label.as_deref(), Some("foo"));
        assert_eq!(items[1].kind_label, "bench");
        assert_eq!(items[2].parent_label.as_deref(), Some("nested::deep"));
    }

    #[test]
    fn parse_cargo_test_list_ignores_noise() {
        let stdout = "running 0 tests\n\n0 tests, 0 benchmarks\nnot a list line\n";
        assert!(parse_cargo_test_list(stdout).is_empty());
    }

    #[test]
    fn projection_from_items_caps_and_records_omission() {
        let items = (0..MAX_PROJECTED_ITEMS + 3)
            .map(|i| TestExplorerItemProjection {
                item_id: format!("t{i}"),
                label: format!("t{i}"),
                kind_label: "test".to_string(),
                parent_label: None,
            })
            .collect();
        let projection = projection_from_items(items, Vec::new(), "ready", TimestampMillis(1));
        assert_eq!(projection.items.len(), MAX_PROJECTED_ITEMS);
        assert!(
            projection
                .diagnostics
                .iter()
                .any(|d| d.starts_with("omitted_items="))
        );
    }

    #[test]
    fn validate_test_item_id_accepts_cargo_paths() {
        assert!(validate_test_item_id("tests::fixture_ok").is_ok());
        assert!(validate_test_item_id("a::b_c.d").is_ok());
        assert!(validate_test_item_id("").is_err());
        assert!(validate_test_item_id("evil;rm").is_err());
        assert!(validate_test_item_id("a b").is_err());
    }

    #[test]
    fn parse_cargo_test_summary_reads_counts() {
        let out = "running 1 test\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
        let (p, f, s, ok) = parse_cargo_test_summary(out);
        assert!(ok);
        assert_eq!((p, f, s), (1, 0, 0));
    }

    #[test]
    fn apply_run_to_projection_records_last_run_metadata() {
        let base = projection_from_items(
            vec![TestExplorerItemProjection {
                item_id: "t::one".to_string(),
                label: "one".to_string(),
                kind_label: "test".to_string(),
                parent_label: Some("t".to_string()),
            }],
            Vec::new(),
            "ready",
            TimestampMillis(1),
        );
        let result = CargoTestItemRunResult {
            item_id: "t::one".to_string(),
            status_label: "passed".to_string(),
            exit_code: Some(0),
            passed: 1,
            failed: 0,
            skipped: 0,
            duration_ms: 12,
            diagnostics: Vec::new(),
        };
        let projection = apply_run_to_projection(base, &result, TimestampMillis(2));
        assert_eq!(projection.last_run_item_id.as_deref(), Some("t::one"));
        assert_eq!(projection.last_run_status.as_deref(), Some("passed"));
        assert_eq!(projection.last_run_exit_code, Some(0));
        assert_eq!(projection.last_run_duration_ms, Some(12));
        assert!(
            projection
                .diagnostics
                .iter()
                .any(|d| d.starts_with("last-run:t::one:passed"))
        );
    }
}
