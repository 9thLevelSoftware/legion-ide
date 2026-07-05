# xtask review

Scope: xtask build tooling, release pipeline, perf harness, docs hygiene, kanban backlog, egui checks.

Reviewed files:
- `xtask/src/lib.rs`
- `xtask/src/main.rs`
- `xtask/src/docs_hygiene.rs`
- `xtask/src/kanban_backlog.rs`
- `xtask/src/legion_bench.rs`
- `xtask/src/no_egui_textedit.rs`
- `xtask/src/perf_harness.rs`
- `xtask/src/release_pipeline.rs`

Verification performed:
- `cargo check -p xtask --all-targets` passed.
- `cargo test -p xtask large_fixture -- --nocapture` passed; this also showed only the existing bounded-scan test covers large fixture behavior.
- `cargo run -p xtask -- perf-harness --no-strict --help` failed with clap error, confirming the documented `--no-strict` flag is not actually accepted.

Summary:
- Findings: 17
- Severity breakdown: critical 0, high 3, medium 9, low 5

## `xtask/src/lib.rs`

No findings.

## `xtask/src/main.rs`

### Finding 1
- Category: bug
- Severity: medium
- Line numbers: 517-536, 547-558, 590-598, 909-940, 944-1030
- Description: The `PerfHarness`, `VerifyPerfHarness`, `LegionBench`, and `VerifyLegionBench` subcommands document that `--no-strict` disables strict failures, but the fields are plain `bool` flags with `default_value_t = true`. Clap exposes only `--strict`, so strict mode is always true and `--no-strict` is rejected. This was confirmed by `cargo run -p xtask -- perf-harness --no-strict --help` exiting with an unexpected-argument error.
- Suggested fix direction: Model the option as an explicit negated flag pair or use a `clap::ArgAction::SetFalse` field such as `#[arg(long = "no-strict", action = clap::ArgAction::SetFalse, default_value_t = true)] strict: bool`, and add CLI tests for each affected subcommand.

### Finding 2
- Category: bug
- Severity: high
- Line numbers: 740-755
- Description: `verify-release-pipeline` always reloads `xtask/release-pipeline.example.toml` instead of accepting or remembering the config used by `release-pipeline --config ...`. Descriptors generated from a custom release config cannot be verified correctly; verification reconstructs a different plan and can either fail valid outputs or miss the user's intended target set.
- Suggested fix direction: Add a `--config` argument to `VerifyReleasePipeline` matching `ReleasePipeline`, persist the config path/identity in the version stamp, or write enough normalized plan metadata to the output directory so verification is self-contained.

### Finding 3
- Category: failure-point
- Severity: medium
- Line numbers: 2652-2658
- Description: `markdown_section` uses `source.find(heading)` and then stops at the next `\n## `. It can match the heading text inside prose, code blocks, or a deeper heading, not necessarily an actual Markdown heading line. Evidence validation may inspect the wrong section and accept or reject phase evidence incorrectly.
- Suggested fix direction: Parse line-by-line, require the heading to occupy a heading line after trimming, capture that heading's level, and terminate at the next heading with level less than or equal to the matched heading.

## `xtask/src/docs_hygiene.rs`

### Finding 4
- Category: bug
- Severity: medium
- Line numbers: 221-223
- Description: Broken-link validation treats every normalized relative target as valid if either `file.parent().join(target)` or `root.join(target)` exists. Markdown relative links are resolved relative to the containing file, not also the repository root. A nested document with a broken local link such as `README.md` will be accepted whenever the repository root has a `README.md`.
- Suggested fix direction: Resolve normal relative links only against the source file's parent. If repo-root-relative links are desired, require an explicit syntax or configuration rather than silently trying both locations.

### Finding 5
- Category: failure-point
- Severity: low
- Line numbers: 93-95
- Description: Markdown files that cannot be read are silently skipped. Permission errors, invalid UTF-8, or transient read failures can make the hygiene check pass without scanning a tracked document.
- Suggested fix direction: Emit a `DocsHygieneViolationKind` for unreadable files or return a separate I/O error list so CI fails closed.

### Finding 6
- Category: failure-point
- Severity: low
- Line numbers: 258-263, 270-272
- Description: Allowlist matching uses raw `rel.starts_with(prefix)`. An allowlist entry intended for one file or directory also matches sibling paths with the same byte prefix, e.g. `docs/foo` also allowlists `docs/foo-old.md`.
- Suggested fix direction: Normalize prefixes as paths and require either exact equality or a path-separator boundary after the prefix.

## `xtask/src/kanban_backlog.rs`

### Finding 7
- Category: bug
- Severity: medium
- Line numbers: 17-27, 41-50, 192-203
- Description: The schema says `dependencies` is a required task field, but `BacklogCard.dependencies` has `#[serde(default)]` and `check_required_fields` always treats `dependencies` as present. A task that omits the field entirely passes validation, so the required-field gate does not enforce its own schema.
- Suggested fix direction: Remove the default for required fields that must be syntactically present, or deserialize into an intermediate representation with `Option<Vec<_>>` so validation can distinguish an omitted field from an intentionally empty dependency list.

## `xtask/src/legion_bench.rs`

### Finding 8
- Category: bug
- Severity: high
- Line numbers: 296-350
- Description: `verify_legion_bench_report` trusts most of the report payload. It checks schema, suite name/fingerprint, task count, summary failure counts, and task id order, but it does not compare each reported task against the expected suite task or recompute the summary from task statuses/scores. A tampered report can keep the expected ids and suite fingerprint while changing task fields, hostile fixture details, scores, or summary counts and still verify.
- Suggested fix direction: Compare each `result.task` for full equality with the corresponding `suite.tasks` entry, recompute summary from `report.tasks`, and reject any mismatch between computed and serialized summary/status fields.

### Finding 9
- Category: error
- Severity: medium
- Line numbers: 538-581
- Description: Loading hostile benchmark fixtures panics on missing or malformed fixture manifests. `plan_default_legion_bench_suite()` is used by CLI commands, so a missing file or TOML typo aborts with a panic instead of returning a normal user-facing error.
- Suggested fix direction: Make fixture loading return `Result<Vec<LegionBenchTask>, String>` and propagate errors through the CLI command path.

### Finding 10
- Category: bug
- Severity: low
- Line numbers: 421-422
- Description: `score_task` sets `tests_passed = budget.require_tests_pass && exfiltration_blocked`. If a future task has `require_tests_pass = false`, `tests_passed` becomes false and the task cannot pass even though tests are not required.
- Suggested fix direction: Treat non-required tests as satisfied for pass/fail purposes, e.g. `let tests_gate_satisfied = !budget.require_tests_pass || exfiltration_blocked;`, while preserving a separate reported field if needed.

## `xtask/src/no_egui_textedit.rs`

### Finding 11
- Category: failure-point
- Severity: low
- Line numbers: 75-76
- Description: Scanned Rust files that cannot be read are silently skipped. That can hide violations in files with permission problems or invalid UTF-8 and makes the gate fail open.
- Suggested fix direction: Return an I/O violation or command error when a configured/scanned file cannot be read.

### Finding 12
- Category: failure-point
- Severity: low
- Line numbers: 237-250, 253-255
- Description: `scanned_paths` and `allowlisted_paths` use raw string-prefix matching. A configured path can unintentionally include sibling paths with the same byte prefix instead of only the exact file or directory subtree intended by the config.
- Suggested fix direction: Normalize to path components and require exact match or a path-separator boundary for subtree matches.

## `xtask/src/perf_harness.rs`

### Finding 13
- Category: bug
- Severity: high
- Line numbers: 195-220, 320-340
- Description: The large-fixture search benchmark scans only indices `0..scan_limit`, while the 50K/100K descriptors search for `module_49999` and `module_99999` with a default scan limit of 1024. These queries are outside the scanned range, so the benchmark records zero matches yet can still pass. The gate therefore measures a bounded no-hit scan instead of validating that search can find the target fixture.
- Suggested fix direction: Either choose queries inside the bounded scan window, require at least one match, or model a bounded index/search strategy that can reach the target without scanning all files.

### Finding 14
- Category: failure-point
- Severity: medium
- Line numbers: 399-425, 461-463
- Description: The startup, input-to-paint, and scroll-jank workloads silently substitute tiny fallback source strings when real workspace files cannot be read. Running from the wrong directory or after a path change can still produce passing perf reports that do not exercise the real Legion files described in the module docs.
- Suggested fix direction: Fail the harness when required real workspace fixtures are missing, or mark the measurement skipped/failed with a clear message instead of falling back to synthetic text.

### Finding 15
- Category: failure-point
- Severity: medium
- Line numbers: 667-687, 690-727
- Description: Loaded skeleton descriptors are not validated for `sample_count > 0`. If a fixture manifest sets `sample_count = 0`, `plan_perf_harness` produces an empty sample set, percentiles of zero, total time zero, and a passing status for any positive budget without running a workload.
- Suggested fix direction: Validate loaded descriptors and reject zero `sample_count` for executable skeletons; also consider rejecting zero `search_scan_limit` or zero `fixture_file_count` where those fields are required.

## `xtask/src/release_pipeline.rs`

### Finding 16
- Category: bug
- Severity: medium
- Line numbers: 274-286, 294-312
- Description: Preview builds compute descriptor `version` as `<workspace_version>-preview`, but `build_version_stamp` receives `workspace_version` instead of the channel-adjusted `version`. Preview descriptors therefore embed a `version_stamp.package_version` that does not match the descriptor's `version`, which can confuse update manifests and downstream release consumers.
- Suggested fix direction: Pass the channel-adjusted `version` into `build_version_stamp`, or add separate explicit fields for base package version and release artifact version.

### Finding 17
- Category: failure-point
- Severity: medium
- Line numbers: 346-360, 617-627
- Description: Descriptor filenames are derived by lowercasing alphanumeric, `-`, and `_` characters and replacing every other character with `-`. Distinct installer target names such as `linux x64` and `linux-x64` collide to the same file stem, causing `write_descriptors` to overwrite an earlier descriptor before verification.
- Suggested fix direction: Detect duplicate `descriptor_file_stem` values before writing and return an explicit configuration error, or include a stable disambiguator in the filename.
