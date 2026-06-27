# Desktop Core Review

Scope: legion-desktop core files listed in kanban task t_27484126.

Verification performed:
- `cargo check -p legion-desktop` completed successfully.
- `cargo test -p legion-desktop` completed successfully: all legion-desktop unit/integration tests passed.

Summary:
- Findings: 18
- Severity breakdown: high 3, medium 7, low 8
- Stub/TODO findings: 0 in assigned files

## crates/legion-desktop/src/lib.rs

No findings.

## crates/legion-desktop/src/main.rs

No findings.

## crates/legion-desktop/src/platform.rs

### Finding 1
- Category: failure-point
- Severity: low
- Line numbers: 194-198
- Description: `high_dpi_status` reports `not observed` for an observed scale of exactly `1.0` or lower. That conflates a valid OS observation on standard-DPI displays with the absence of any observation, so smoke/diagnostic evidence can falsely claim high-DPI data was not collected.
- Suggested fix direction: Distinguish `None` from `Some(scale)`. Report something like `os-observed scale 1.000` for all observed finite positive scales, and reserve `not observed` for `None` or invalid values.

## crates/legion-desktop/src/session.rs

### Finding 2
- Category: failure-point
- Severity: medium
- Line numbers: 126-183
- Description: Crash-safe saves use a temporary path based only on the process id (`.<file>.<pid>.tmp`). Concurrent saves of the same session path within one process can remove or overwrite each other's temp file, causing lost saves or publish failures.
- Suggested fix direction: Include a per-save nonce/counter or use an atomic tempfile API in the destination directory. Avoid deleting a temp path that could belong to another in-flight save.

### Finding 3
- Category: failure-point
- Severity: low
- Line numbers: 106-112
- Description: `reject_raw_source_markers` rejects any serialized session containing marker substrings such as `source_body` anywhere in the JSON. A legitimate metadata field, file path, branch name, or label containing one of those substrings would make load/save fail even though no raw source payload is present.
- Suggested fix direction: Validate structured fields that are known raw-payload carriers, or use schema-level redaction markers rather than broad substring matching over the whole JSON document.

## crates/legion-desktop/src/bridge.rs

### Finding 4
- Category: bug
- Severity: medium
- Line numbers: 1249-1255
- Description: `OpenGitPullRequestUrl` falls back to using the current branch as the base branch when `remote_default_branch` is missing. That produces a self-compare URL (`branch...branch`) rather than surfacing that the default/base branch is unknown, so the PR action can open a misleading or useless forge page.
- Suggested fix direction: Treat missing or empty remote default branch as a typed bridge error, or choose a validated configured fallback such as `main` only when the projection explicitly supports it.

## crates/legion-desktop/src/workflow.rs

### Finding 5
- Category: bug
- Severity: high
- Line numbers: 872-879
- Description: The `OpenWorkspace` app request opens the new root in `AppComposition`, but `DesktopRuntime.workspace_root` is never updated. Diagnostics, operational health, and later session captures continue to label the original workspace after a successful workspace switch; adapter-local explorer expansion also remains from the previous root.
- Suggested fix direction: On successful workspace open, assign `self.workspace_root = root`, reset workspace-scoped adapter-local state as appropriate, and ensure session/diagnostics paths are either rebased or explicitly kept per launch policy.

### Finding 6
- Category: failure-point
- Severity: medium
- Line numbers: 2029-2030, 2103-2104, 2136-2138, 2191-2194, 2200-2205, 2210-2213, 2222-2225, 2233-2236, 2385-2386, 2443-2447, 2505-2514
- Description: Multiple UI event paths call `runtime.handle_action(...)` and discard the `Result` with `let _ = ...`. If refresh, session persistence, diagnostics, or app authority returns an error, production UI/headless harness can silently continue without surfacing the failure to the user or tests.
- Suggested fix direction: Capture errors, set an error status, and request a projection refresh. In test/headless paths, consider returning or storing the error so failures are assertable.

### Finding 7
- Category: bug
- Severity: high
- Line numbers: 2683-2731
- Description: `editor_text_input_actions` computes the insertion coordinate once before iterating all text/paste/IME events. If a single egui frame contains multiple text-like events, all generated actions target the same stale cursor. Production `handle_keyboard` then executes the batched actions after collection, so inserted text can be ordered incorrectly or inserted at the wrong offset.
- Suggested fix direction: Dispatch text actions sequentially while refreshing the projection/cursor after each edit, or accumulate text-like payloads into one insertion preserving event order before creating a single action.

### Finding 8
- Category: failure-point
- Severity: high
- Line numbers: 2945-2958
- Description: On Windows, `open_url_in_system_browser` invokes `cmd /C start` and passes the URL as a raw argument. Forge URLs can contain shell metacharacters such as `&` in query strings (notably GitLab merge-request URLs), and cmd parsing can split or reinterpret them. This can make valid URLs fail to open and may become command-injection-prone if any URL component is attacker-controlled.
- Suggested fix direction: Avoid `cmd /C start` for untrusted URLs. Use a Windows shell-open API (for example `open::that`/ShellExecute) or carefully quote/escape the entire URL for `cmd` semantics.

## crates/legion-desktop/src/health.rs

### Finding 9
- Category: failure-point
- Severity: low
- Line numbers: 171-233, 238-312
- Description: Health rows and markdown interpolate workspace labels and unsupported-surface labels without escaping or line normalization. A workspace path or label containing newlines/markdown characters can produce malformed rows or misleading diagnostics output.
- Suggested fix direction: Normalize labels used in row-oriented evidence by replacing control characters/newlines and escaping markdown-sensitive content where the output is intended to be parsed or audited.

## crates/legion-desktop/src/diagnostics.rs

### Finding 10
- Category: failure-point
- Severity: low
- Line numbers: 109-118
- Description: The opt-in raw snapshot appendix is placed inside a fixed triple-backtick fence without escaping embedded fences. If the debug representation ever contains ``` text, it can terminate the fence early and corrupt the diagnostics markdown.
- Suggested fix direction: Use a dynamically chosen fence length longer than any fence in the payload, indent raw data, or otherwise escape fence markers before writing markdown.

## crates/legion-desktop/src/metrics.rs

### Finding 11
- Category: failure-point
- Severity: medium
- Line numbers: 50-52, 87-102, 111-127
- Description: `FrameTimingRecorder` stores every input-to-paint sample and every frame duration with no retention bound. If reused outside short smoke runs, a long-running desktop session can accumulate unbounded timing vectors and increasingly expensive summaries.
- Suggested fix direction: Bound samples with a ring buffer or windowed aggregation, and make the retention policy explicit in the recorder API.

## crates/legion-desktop/src/search.rs

### Finding 12
- Category: failure-point
- Severity: low
- Line numbers: 61-68
- Description: Search result rows interpolate `row.snippet` directly into a one-line display string. Snippets containing newlines, tabs, or other control characters can break row-oriented rendering/logging and make result diagnostics ambiguous.
- Suggested fix direction: Normalize snippets for display rows by replacing control characters and newlines, while keeping any raw/snippet-rich data in structured fields if needed.

## crates/legion-desktop/src/theme.rs

No findings.

## crates/legion-desktop/src/package.rs

### Finding 13
- Category: bug
- Severity: medium
- Line numbers: 35-40, 116-120
- Description: The Windows package plan always builds `cargo build -p legion-desktop` and points at `target/<profile>/legion-desktop.exe`. On non-Windows hosts this command produces the host binary, not a Windows `.exe`; cross-compiled Windows builds would usually live under `target/<triple>/<profile>/`. The plan can therefore reference an executable that was never built.
- Suggested fix direction: Include an explicit Windows target triple in the plan/config when packaging from non-Windows hosts, and derive `executable_source` from Cargo's target-dir/target-triple layout.

## crates/legion-desktop/src/beta.rs

### Finding 14
- Category: failure-point
- Severity: medium
- Line numbers: 345-360
- Description: `prepare_beta_workspace` resolves relative beta workspace paths against `std::env::current_dir()` instead of `BetaWorkflowConfig.real_workspace_root`. Programmatic callers or launches from a different current directory can create/delete the smoke workspace under the wrong `target/` tree while the report still labels the configured real workspace.
- Suggested fix direction: Resolve relative beta workspace paths against the configured real workspace root, or document/enforce that the process current directory must be the repository root before running the beta workflow.

### Finding 15
- Category: failure-point
- Severity: low
- Line numbers: 647-659
- Description: `beta_smoke_command` builds a copy/paste command by joining unquoted paths. Workspaces, evidence paths, or diagnostics paths containing spaces or shell metacharacters produce evidence commands that cannot be reliably rerun.
- Suggested fix direction: Render command evidence as an argv list or shell-quote each path/argument for the documented shell.

## crates/legion-desktop/src/smoke.rs

### Finding 16
- Category: bug
- Severity: medium
- Line numbers: 315-323
- Description: Native smoke pass/fail only checks whether at least one frame was observed. Adapter checks, focus/high-DPI observation, accessibility projection, and guardrail statuses can all be `not observed` or failed while the overall status is still `Passed`.
- Suggested fix direction: Add explicit gate checks for required smoke signals and include failed adapter/platform checks in `errors` before selecting `RendererSmokeStatus::Passed`.

### Finding 17
- Category: failure-point
- Severity: low
- Line numbers: 437-441
- Description: Smoke high-DPI status has the same false-negative as `platform.rs`: an observed scale of `1.0` is reported as `not observed`, masking successful standard-DPI observation.
- Suggested fix direction: Report all observed finite positive scales distinctly from `None`; only absence of a sample should become `not observed`.

### Finding 18
- Category: failure-point
- Severity: low
- Line numbers: 351-368
- Description: `smoke_command` renders rerun evidence by concatenating unquoted paths and file arguments. Paths with spaces or shell metacharacters will not round-trip as a usable command.
- Suggested fix direction: Store command evidence as structured argv or shell-quote path arguments.
