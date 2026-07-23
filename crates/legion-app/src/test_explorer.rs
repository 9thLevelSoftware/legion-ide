//! Test explorer discovery (P2.F3.T4 thin slice).
//!
//! Discovers Cargo tests via `cargo test -- --list` and projects metadata-only
//! rows. Does not run tests and does not claim LSP-runnable parity.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use legion_protocol::TimestampMillis;
use legion_ui::{TestExplorerItemProjection, TestExplorerProjection};

/// Default hard timeout for discovery subprocesses.
pub const DEFAULT_DISCOVER_TIMEOUT: Duration = Duration::from_secs(60);

/// Maximum items retained in the projection (metadata-bounded).
pub const MAX_PROJECTED_ITEMS: usize = 500;

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
        generated_at,
        schema_version: 1,
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
}
