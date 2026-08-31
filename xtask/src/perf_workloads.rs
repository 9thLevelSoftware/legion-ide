//! Real product workloads for the perf harness (P8.F4.T1).
//!
//! # Why a subprocess
//!
//! `xtask` may not depend on `legion-app` or `legion-editor` — `cargo run -p
//! xtask -- check-deps` enforces that, and it is the reason every workload in
//! `perf_harness.rs` used to be a synthetic stand-in. The workloads that need
//! the real editor therefore live in `legion-app`'s `product_perf` binary; this
//! module spawns it, reads its TOML report, and owns the budgets. It is the
//! same shape `golden-path-1`, `large_file_perf`, and `legion-desktop
//! --manual-perf` already use: the product measures, `xtask` judges.
//!
//! # Why the budgets live here and not in the binary
//!
//! A measurement that decides its own verdict cannot be audited against a
//! policy. Keeping the numbers on this side means one file answers "what is
//! Legion allowed to cost", and the binary answers only "what did it cost".

use std::path::Path;

use serde::Deserialize;

use crate::perf_harness::{SkeletonKind, SkeletonMeasurement, SkeletonStatus};

/// Report file the `product_perf` subprocess writes.
pub const PRODUCT_PERF_REPORT_FILE: &str = "product-perf.toml";

/// # How these numbers were chosen
///
/// Unlike the skeleton budgets, these are **not** relaxed by
/// `LEGION_PERF_FAIL_ON_BUDGET_MS` — see `product_budgets_ignore_the_skeleton_report_only_override`.
/// They therefore have to hold on the slowest supported CI runner, not only on
/// a quiet developer machine, and they are sized accordingly: each is a
/// catastrophe guard with room for a slow shared VM above it.
///
/// The tight, machine-specific gate is the trend baseline in
/// `plans/evidence/perf-harness-trend/baseline.toml`, which compares a run
/// against previous runs *on the same OS* at a 60% tolerance. That is where a
/// 2x regression is caught. These ceilings catch the case where something went
/// from milliseconds to seconds, which no tolerance-based comparison can be
/// trusted to catch on a runner that is itself 3x slower than the reference
/// machine.
///
/// ADR-0048's typing budgets are the exception: they are the product's stated
/// contract with the user, they are met with 8x headroom today, and softening
/// them to accommodate a CI runner would be softening the product's promise.
const KEYPRESS_P50_BUDGET_MILLIS: u64 = 16;
const KEYPRESS_P95_BUDGET_MILLIS: u64 = 32;
const SCROLL_P95_BUDGET_MILLIS: u64 = 32;

/// Open-to-ready ceiling for the Legion repository.
///
/// A regression guard, not a target. The measured value on the reference
/// machine is ~3.6 s, and ~3.3 s of that is `AppComposition::open_file` running
/// the lexical retrieval indexer synchronously on a 1.5 MB source file. The
/// ceiling is above today's number on purpose: pretending the product already
/// meets a 1 s target would make this workload red on arrival and teach
/// everyone to ignore the row. The defect is recorded in
/// `plans/evidence/production/P8.F4/perf-harness-product-workloads.md`; when it
/// is fixed, this constant comes down with it.
const STARTUP_BUDGET_MILLIS: u64 = 30_000;

/// Memory ceiling for one open 1.5 MB source file after a typing burst.
///
/// The measured value is ~24 MB: a ~5.9 MB snapshot plus three retained
/// snapshot descriptors of the same size, which is the retention policy doing
/// what it is configured to do. 48 MB leaves room for that policy without
/// leaving room for a leak. Byte-valued and therefore host-independent, so this
/// one is a real ceiling rather than a catastrophe guard.
const MEMORY_CEILING_BYTES: u64 = 48 * 1024 * 1024;

/// Full-repository product search ceiling. Measured ~1 s on the reference
/// machine with a warm page cache; 120 s absorbs a cold one behind real-time
/// antivirus.
const LEGION_REPO_BUDGET_MILLIS: u64 = 120_000;

/// 100K-file product search ceiling — a liveness guard, not a performance
/// budget, and the difference is worth being explicit about.
///
/// The same workload on the same machine measured 74 s with a warm page cache
/// and ~950 s with a cold one behind Windows Defender: a 13x spread that has
/// nothing to do with Legion. Product search opens and reads all 100 000 files
/// on a single thread (`WalkBuilder::build`, not `build_parallel`), so the
/// number is dominated by the host's per-file-open cost.
///
/// A tight ceiling here would gate on the machine. 30 minutes still catches the
/// failures worth catching — a search that does not terminate, or one that got
/// an order of magnitude slower — without firing on a busy laptop. The
/// throughput question this workload really asks is answered by the archived
/// trend numbers, not by this ceiling.
const FIXTURE_100K_BUDGET_MILLIS: u64 = 1_800_000;

const GIT_DISPATCH_P95_BUDGET_MILLIS: u64 = 4;

/// What a workload is allowed to cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductBudget {
    /// Latency ceilings in milliseconds. A `0` ceiling means that percentile
    /// is not gated (the single-sample workloads have no meaningful p95).
    Latency { p50_millis: u64, p95_millis: u64 },
    /// Memory ceiling in bytes.
    Memory { ceiling_bytes: u64 },
}

/// One product workload and the policy applied to it.
#[derive(Debug, Clone, Copy)]
pub struct ProductWorkloadPolicy {
    /// Row name, matching the `name` the subprocess writes.
    pub name: &'static str,
    pub budget: ProductBudget,
    /// What the number means, in the report, for someone who will not read
    /// this file.
    pub summary: &'static str,
}

/// Every product workload the harness enforces.
///
/// This list is also the coverage contract: `verify-perf-harness` fails when a
/// report is missing a row from it, which is what stops an OS-specific failure
/// from turning into a quietly shorter report (P8.F4.T2).
pub fn product_workload_policies() -> Vec<ProductWorkloadPolicy> {
    vec![
        ProductWorkloadPolicy {
            name: "p8.startup",
            budget: ProductBudget::Latency {
                p50_millis: STARTUP_BUDGET_MILLIS,
                p95_millis: 0,
            },
            summary: "real AppComposition open-to-ready on the Legion repository",
        },
        ProductWorkloadPolicy {
            name: "p8.input_to_paint",
            budget: ProductBudget::Latency {
                p50_millis: KEYPRESS_P50_BUDGET_MILLIS,
                p95_millis: KEYPRESS_P95_BUDGET_MILLIS,
            },
            summary: "real keystroke through the editor plus the real viewport projection",
        },
        ProductWorkloadPolicy {
            name: "p8.scroll_jank",
            budget: ProductBudget::Latency {
                p50_millis: 0,
                p95_millis: SCROLL_P95_BUDGET_MILLIS,
            },
            summary: "real viewport projections at scroll offsets across a whole 36K-line file",
        },
        ProductWorkloadPolicy {
            name: "p8.memory_ceiling",
            budget: ProductBudget::Memory {
                ceiling_bytes: MEMORY_CEILING_BYTES,
            },
            summary: "real snapshot footprint of a real 1.5MB source file after a typing burst",
        },
        ProductWorkloadPolicy {
            name: "p8.legion_repo",
            budget: ProductBudget::Latency {
                p50_millis: LEGION_REPO_BUDGET_MILLIS,
                p95_millis: 0,
            },
            summary: "real product workspace search over this repository",
        },
        ProductWorkloadPolicy {
            name: "p8.fixture_100k_files",
            budget: ProductBudget::Latency {
                p50_millis: FIXTURE_100K_BUDGET_MILLIS,
                p95_millis: 0,
            },
            summary: "real product workspace search over a generated 100K-file workspace",
        },
        ProductWorkloadPolicy {
            name: "git.ui_dispatch_refresh",
            budget: ProductBudget::Latency {
                p50_millis: GIT_DISPATCH_P95_BUDGET_MILLIS,
                p95_millis: GIT_DISPATCH_P95_BUDGET_MILLIS,
            },
            summary: "RefreshGit intent-to-return on the cheap projection path; no paint wait",
        },
        ProductWorkloadPolicy {
            name: "git.remote_push_does_not_block_dispatch",
            budget: ProductBudget::Latency {
                p50_millis: GIT_DISPATCH_P95_BUDGET_MILLIS,
                p95_millis: GIT_DISPATCH_P95_BUDGET_MILLIS,
            },
            summary: "policy-denied PushGitRemote intent-to-return; no remote process or network",
        },
    ]
}

/// Git rows whose strict gates require instrumentation or the typed backend.
/// They remain visible in every report without pretending that a product
/// workload measured a worker-job or process count it cannot observe yet.
pub fn git_report_only_measurements() -> Vec<SkeletonMeasurement> {
    [
        (
            "git.jobs_per_refresh_burst",
            "report-only: PR-2 deterministic 50-refresh regression owns the <=2 worker-job proof; product_perf has no worker counter",
        ),
        (
            "git.spawn_count_per_snapshot",
            "report-only: deferred until post-PR-3 typed-gix/process instrumentation; no process count is inferred",
        ),
        (
            "git.status_legion_repo",
            "report-only: strict status-row gate deferred until post-PR-3 typed-gix parity evidence",
        ),
    ]
    .into_iter()
    .map(|(name, message)| SkeletonMeasurement {
        name: name.to_string(),
        kind: SkeletonKind::ProductWorkload,
        fixture_bytes: 0,
        sample_count: 0,
        total_micros: 0,
        p50_micros: 0,
        p95_micros: 0,
        budget_millis: 0,
        status: SkeletonStatus::Skipped,
        message: message.to_string(),
        measured: false,
        bytes_value: 0,
        synthetic_stand_in: false,
    })
    .collect()
}

/// The `product_perf` subprocess's report.
#[derive(Debug, Clone, Deserialize)]
pub struct ProductPerfReport {
    pub schema_version: u32,
    pub os: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub workspace_root: String,
    #[serde(default)]
    pub workload: Vec<ProductWorkloadRow>,
}

/// One measured workload as the subprocess reported it.
#[derive(Debug, Clone, Deserialize)]
pub struct ProductWorkloadRow {
    pub name: String,
    pub measured: bool,
    #[serde(default)]
    pub sample_count: usize,
    #[serde(default)]
    pub fixture_bytes: u64,
    #[serde(default)]
    pub p50_micros: u64,
    #[serde(default)]
    pub p95_micros: u64,
    #[serde(default)]
    pub total_micros: u64,
    #[serde(default)]
    pub bytes_value: u64,
    #[serde(default)]
    pub detail: String,
}

/// Parse a `product_perf` report.
pub fn parse_product_perf_report(text: &str) -> Result<ProductPerfReport, String> {
    let report: ProductPerfReport = toml::from_str(text)
        .map_err(|err| format!("unable to parse product perf report: {err}"))?;
    if report.schema_version != 1 {
        return Err(format!(
            "product perf report uses unsupported schema_version {}",
            report.schema_version
        ));
    }
    Ok(report)
}

/// Turn a parsed report into one harness row per policy entry.
///
/// A policy entry with no matching row becomes an unmeasured row rather than
/// vanishing: a workload that disappears from a report is the exact failure
/// mode P8.F4.T2 exists to prevent, and silence is not a measurement.
pub fn product_measurements(report: &ProductPerfReport) -> Vec<SkeletonMeasurement> {
    product_workload_policies()
        .into_iter()
        .map(
            |policy| match report.workload.iter().find(|row| row.name == policy.name) {
                Some(row) => classify_product_row(&policy, row, &report.os),
                None => unmeasured_product_measurement(
                    &policy,
                    format!(
                        "workload `{}` is absent from the {} product perf report",
                        policy.name, report.os
                    ),
                ),
            },
        )
        .collect()
}

/// Classify one reported row against its policy.
// Product-workload budgets are enforced on every host, including hosted CI.
//
// This function reads no environment at all, and that is the point.
// `LEGION_PERF_FAIL_ON_BUDGET_MS=0` relaxes the skeleton budgets on shared
// runners, because a 2 ms wall-clock microbenchmark on a hosted VM is noise.
// Applying that override here would make every budget on every OS unfailable,
// which is exactly P8.F4.T2's stop condition — "no OS job may silently skip".
// The product ceilings are sized to survive a slow runner instead, so they can
// stay armed everywhere.
//
// `product_budgets_ignore_the_skeleton_report_only_override` sets the variable
// and asserts an over-budget row still fails.
pub fn classify_product_row(
    policy: &ProductWorkloadPolicy,
    row: &ProductWorkloadRow,
    os: &str,
) -> SkeletonMeasurement {
    if !row.measured {
        return unmeasured_product_measurement(
            policy,
            format!("[{os}] not measured: {}", row.detail),
        );
    }

    let (status, budget_millis, verdict) = match policy.budget {
        ProductBudget::Latency {
            p50_millis,
            p95_millis,
        } => {
            let p50_over = p50_millis > 0 && row.p50_micros > p50_millis.saturating_mul(1_000);
            let p95_over = p95_millis > 0 && row.p95_micros > p95_millis.saturating_mul(1_000);
            let status = if p50_over || p95_over {
                SkeletonStatus::Failed
            } else {
                SkeletonStatus::Passed
            };
            let verdict = format!(
                "p50={:.1}ms p95={:.1}ms against p50<{}ms p95<{}ms{}",
                row.p50_micros as f64 / 1_000.0,
                row.p95_micros as f64 / 1_000.0,
                p50_millis,
                p95_millis,
                if p50_over || p95_over {
                    " — OVER BUDGET"
                } else {
                    ""
                },
            );
            // The gated percentile is what the row's budget field advertises,
            // so a reader comparing `p95_micros` to `budget_millis` is
            // comparing the two numbers the verdict actually used.
            let advertised = if p95_millis > 0 {
                p95_millis
            } else {
                p50_millis
            };
            (status, advertised, verdict)
        }
        ProductBudget::Memory { ceiling_bytes } => {
            let over = row.bytes_value > ceiling_bytes;
            let verdict = format!(
                "{:.1}MB against a {:.0}MB ceiling{}",
                row.bytes_value as f64 / (1024.0 * 1024.0),
                ceiling_bytes as f64 / (1024.0 * 1024.0),
                if over { " — OVER CEILING" } else { "" },
            );
            (
                if over {
                    SkeletonStatus::Failed
                } else {
                    SkeletonStatus::Passed
                },
                // Byte-valued: no millisecond budget exists to advertise.
                0,
                verdict,
            )
        }
    };

    SkeletonMeasurement {
        name: row.name.clone(),
        kind: SkeletonKind::ProductWorkload,
        fixture_bytes: row.fixture_bytes as usize,
        sample_count: row.sample_count,
        total_micros: row.total_micros,
        p50_micros: row.p50_micros,
        p95_micros: row.p95_micros,
        budget_millis,
        status,
        message: format!("[{os}] {}: {verdict}. {}", policy.summary, row.detail),
        measured: true,
        bytes_value: row.bytes_value,
        synthetic_stand_in: false,
    }
}

/// A row saying, in the report, that this workload did not run.
pub fn unmeasured_product_measurement(
    policy: &ProductWorkloadPolicy,
    message: String,
) -> SkeletonMeasurement {
    let budget_millis = match policy.budget {
        ProductBudget::Latency {
            p50_millis,
            p95_millis,
        } => {
            if p95_millis > 0 {
                p95_millis
            } else {
                p50_millis
            }
        }
        ProductBudget::Memory { .. } => 0,
    };
    SkeletonMeasurement {
        name: policy.name.to_string(),
        kind: SkeletonKind::ProductWorkload,
        fixture_bytes: 0,
        sample_count: 0,
        total_micros: 0,
        p50_micros: 0,
        p95_micros: 0,
        budget_millis,
        status: SkeletonStatus::Skipped,
        message,
        measured: false,
        bytes_value: 0,
        synthetic_stand_in: false,
    }
}

/// Rows for every policy entry when the subprocess itself could not run.
pub fn all_unmeasured_product_measurements(reason: &str) -> Vec<SkeletonMeasurement> {
    product_workload_policies()
        .into_iter()
        .map(|policy| unmeasured_product_measurement(&policy, reason.to_string()))
        .collect()
}

/// Spawn `legion-app --bin product_perf` and classify what it reports.
///
/// Errors never propagate as `Err`: a failure to run is itself a result the
/// report has to carry, and it carries it as `measured = false` rows, which
/// `verify-perf-harness` fails on.
pub fn run_product_workloads(workspace_root: &Path, out_dir: &Path) -> Vec<SkeletonMeasurement> {
    let report_path = out_dir.join(PRODUCT_PERF_REPORT_FILE);
    let _ = std::fs::create_dir_all(out_dir);
    // Remove any stale report so a previous run's numbers cannot be reported
    // as this run's.
    match std::fs::remove_file(&report_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return all_unmeasured_product_measurements(&format!(
                "unable to clear stale product perf report `{}`: {err}",
                report_path.display()
            ));
        }
    }

    let output = std::process::Command::new("cargo")
        .current_dir(workspace_root)
        .args([
            "run",
            "--release",
            "-q",
            "-p",
            "legion-app",
            "--bin",
            "product_perf",
            "--",
            "--workspace-root",
        ])
        .arg(workspace_root)
        .arg("--report")
        .arg(&report_path)
        .output();

    let output = match output {
        Ok(output) => output,
        Err(err) => {
            return all_unmeasured_product_measurements(&format!(
                "unable to spawn the product_perf subprocess: {err}"
            ));
        }
    };

    // A non-zero exit is expected when a workload could not run — the binary
    // still writes the report, and the per-row reasons are more useful than
    // the exit code, so the report is read either way.
    let text = match std::fs::read_to_string(&report_path) {
        Ok(text) => text,
        Err(err) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return all_unmeasured_product_measurements(&format!(
                "product perf report `{}` unreadable ({err}); subprocess exited {} with stderr: {}",
                report_path.display(),
                output.status,
                truncate(&stderr)
            ));
        }
    };

    match parse_product_perf_report(&text) {
        Ok(report) => product_measurements(&report),
        Err(err) => all_unmeasured_product_measurements(&err),
    }
}

fn truncate(text: &str) -> String {
    const LIMIT: usize = 600;
    let normalized = text.replace("\r\n", "\n");
    let trimmed = normalized.trim();
    if trimmed.chars().count() <= LIMIT {
        trimmed.to_string()
    } else {
        format!("{}...", trimmed.chars().take(LIMIT).collect::<String>())
    }
}

/// Re-gate a product measurement against an explicit millisecond ceiling.
///
/// Not wired to `LEGION_PERF_FAIL_ON_BUDGET_MS` — see
/// `product_budgets_ignore_the_skeleton_report_only_override` for why product ceilings stay armed on
/// every host. This exists for the failing-gate drill: a caller that wants to
/// prove the gate can fail hands it a ceiling below the measured value.
///
/// A ceiling of `0` means report-only. An unmeasured row is left alone:
/// "the runner was slow" and "the workload did not run" are different problems,
/// and relaxing a budget must never turn the second into a pass.
pub fn regate_with_ceiling(measurement: &mut SkeletonMeasurement, ceiling_millis: u64) {
    if !measurement.measured {
        return;
    }
    if ceiling_millis == 0 {
        if measurement.status == SkeletonStatus::Failed {
            measurement.status = SkeletonStatus::Skipped;
            measurement.message = format!("ceiling 0; report-only. {}", measurement.message);
        }
        measurement.budget_millis = 0;
        return;
    }
    // The memory-valued row has no millisecond budget, so a millisecond
    // ceiling cannot decide it — and says so rather than silently passing it.
    if measurement.bytes_value > 0 {
        measurement.message = format!(
            "ceiling {ceiling_millis}ms does not apply to a byte-valued workload. {}",
            measurement.message
        );
        return;
    }
    measurement.budget_millis = ceiling_millis;
    let ceiling_micros = ceiling_millis.saturating_mul(1_000);
    let observed = measurement.p95_micros.max(measurement.p50_micros);
    measurement.status = if observed > ceiling_micros {
        SkeletonStatus::Failed
    } else {
        SkeletonStatus::Passed
    };
}
