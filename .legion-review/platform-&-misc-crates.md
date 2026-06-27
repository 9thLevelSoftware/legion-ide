# Platform & Misc Crates Review

Scope reviewed:
- `crates/legion-platform/src/lib.rs`
- `crates/legion-project/src/lib.rs`
- `crates/legion-index/src/lib.rs`
- `crates/legion-tracker/src/lib.rs`
- `crates/legion-telemetry/src/lib.rs`
- `crates/legion-cli/src/main.rs`
- `crates/legion-vscode-compat/src/lib.rs`

Summary: 15 findings (high: 6, medium: 7, low: 2).

## crates/legion-platform/src/lib.rs

### Finding 1
- Category: bug
- Severity: high
- Line numbers: 990-1019
- Description: `NativeProcessService::execute` enforces `request.timeout` only after `Command::output()` returns. A command that hangs or produces no terminating output can block forever and the timeout branch is never reached until the child exits naturally.
- Suggested fix direction: Spawn the child with piped stdout/stderr, poll `try_wait()` against the timeout, and kill/reap the child (and process tree/session where appropriate) on timeout before returning `PlatformError::Timeout`.

### Finding 2
- Category: bug
- Severity: medium
- Line numbers: 1102-1110
- Description: Unix PTY spawning clears the environment and then calls `child_environment_vars(&[])`, ignoring `PtyRequest.env`. PTY sessions therefore cannot receive caller-supplied environment variables even though `PtyRequest` exposes that field.
- Suggested fix direction: Pass `&request.env` to `child_environment_vars`, matching the non-PTY process execution path, and add a test that a spawned PTY can observe an injected variable.

### Finding 3
- Category: failure-point
- Severity: medium
- Line numbers: 1798-1800, 1834-1845
- Description: `close_pty` on Unix calls `kill_pty(..., PtyKillMode::Terminate)`, and the terminate path signals only the direct child PID. Because PTY children are placed in a new session/process group, grandchildren or shells with active child jobs can survive a close and remain orphaned.
- Suggested fix direction: Treat close as a process-group/session cleanup, or add an explicit close mode that signals `-pid`, escalates after a timeout, and reaps the leader before removing the session.

### Finding 4
- Category: failure-point
- Severity: low
- Line numbers: 971-978
- Description: `list_directory` uses `filter_map(|entry| entry.ok())`, silently dropping directory entries whose metadata/path retrieval fails. Callers get an apparently successful partial listing with no diagnostic.
- Suggested fix direction: Collect `read_dir` entries with error propagation, or return a structured partial result with omitted-entry diagnostics if partial listings are intentional.

## crates/legion-project/src/lib.rs

### Finding 5
- Category: bug
- Severity: medium
- Line numbers: 279-285, 460-466, 1793-1796, 3451-3454
- Description: The project-local `stable_hash` uses `std::collections::hash_map::DefaultHasher` for identifiers that are intended to be deterministic (`WorkspaceId`, hunk ids, content-version digests). `DefaultHasher` is not a stable serialization/hash contract across Rust versions or implementations, so these ids can drift across builds even though the surrounding code labels them stable/deterministic.
- Suggested fix direction: Use an explicit stable hash algorithm (for example the platform crate's FNV-1a helper, xxHash with fixed seed, or SHA-256 truncated to the required width) and version the algorithm in any persisted/protocol-facing ids.

### Finding 6
- Category: failure-point
- Severity: high
- Line numbers: 1573-1643
- Description: The Gix backend parses `gix` status items by formatting them with `Debug` and scraping substrings such as `path: "..."`, `untracked`, and `modified`. This is not a stable API and can silently misclassify or drop paths whenever the debug format changes, contains escaped quotes, or includes words that trip the heuristic status detector.
- Suggested fix direction: Use typed `gix` status item accessors for path and status, or keep the CLI porcelain parser as the authoritative implementation until the typed Gix mapping is implemented and covered by rename/copy/delete/untracked tests.

### Finding 7
- Category: bug
- Severity: medium
- Line numbers: 3108-3127, 4718-4736
- Description: Workspace search reports `line_number: line_number as u32` directly from `enumerate()`, so search hits are zero-based. `WorkspaceSearchHit::line_number` is a user-facing line field rather than a protocol `TextCoordinate`, and returning zero for the first line is likely to mislead UI/search consumers that display normal one-based line numbers.
- Suggested fix direction: Decide and document the coordinate convention. If this is display/search output, return `line_number.saturating_add(1) as u32`; if it is intentionally zero-based, rename/document it accordingly and ensure every consumer applies the same convention.

## crates/legion-index/src/lib.rs

### Finding 8
- Category: bug
- Severity: high
- Line numbers: 2461-2466, 2503-2511, 2520-2528, 2620-2624, 2653-2678
- Description: Registering a plugin tree-sitter grammar makes `tree_sitter_supports_language` return true for that language, but the parser/highlighter/chunker still call the bundled Rust tree-sitter routines unconditionally. A registered non-Rust plugin language will therefore be parsed/highlighted as Rust instead of using its plugin grammar or falling back safely.
- Suggested fix direction: Separate bundled Rust support from plugin registration. Only call `parse_tree_sitter_rust`/Rust highlight queries for Rust, and route plugin grammars through a real loaded grammar worker; until then, registered plugin languages should fall back to `LexicalFallbackParser` with a diagnostic.

### Finding 9
- Category: failure-point
- Severity: medium
- Line numbers: 2644-2650, 2658-2660, 2681-2685, 2688-2693
- Description: The global plugin grammar registry is protected by a `Mutex`, but every lock uses `expect(...)`. Any panic while holding the registry lock poisons it and turns later grammar registration/support checks into process panics.
- Suggested fix direction: Replace `expect` with fallible lock handling that returns zero/false or a structured `IndexError`, and emit a diagnostic so one poisoned plugin path does not take down indexing.

### Finding 10
- Category: bug
- Severity: medium
- Line numbers: 5241-5324
- Description: `build_rename_preview_payload` always marks the target coverage as `Complete` with `omitted_target_count: 0`, even though it only uses the ranges present in a single `SymbolFileMapRecord`. If the symbol map is file-scoped or stale, consumers may treat an incomplete rename preview as a complete workspace rename.
- Suggested fix direction: Mark coverage as partial unless the symbol record carries an explicit workspace-complete proof/freshness token, or add omitted-target diagnostics when cross-file references were not searched.

## crates/legion-tracker/src/lib.rs

### Finding 11
- Category: bug
- Severity: high
- Line numbers: 194-209
- Description: `LegionWorkflowTrackerRecord::validate` allows `merge_readiness_state == Ready` while `failed_verification_count > 0`. The ready-state guard requires gate ids, sign-off counts, and no unresolved conflicts, but it does not reject failed or blocked verification gates.
- Suggested fix direction: Add `failed_verification_count == 0` to the Ready-state validation and add a regression test for a Ready record with failed verification metadata.

## crates/legion-telemetry/src/lib.rs

### Finding 12
- Category: bug
- Severity: high
- Line numbers: 580-604, 697-705
- Description: `pending_batch` accepts `consent` and `endpoint` independently and does not verify that `consent.endpoint` matches the endpoint used for upload. `HostedTelemetryExporter::export_once` then uploads to `batch.endpoint`, so a caller can combine consent for one hosted endpoint with a different allowlisted endpoint descriptor.
- Suggested fix direction: Validate endpoint binding inside `pending_batch`/`export_once` (endpoint id, label, region, and schema) before constructing the batch, or derive the upload endpoint only from the consent grant.

### Finding 13
- Category: failure-point
- Severity: medium
- Line numbers: 534-555
- Description: `FileBackedTelemetrySpool::open` deserializes persisted spool state but does not validate the decoded schema version, records, or capacity invariants. A corrupted/stale spool file can remain loaded until later operations fail in less obvious places, and invalid records can poison export attempts.
- Suggested fix direction: Validate `schema_version`, cap `records.len()` against configuration, and run `validate_hosted_telemetry_spool_record` on every decoded record during open; quarantine or reject invalid spool files with a clear error.

## crates/legion-cli/src/main.rs

No concrete bugs, stubs, or failure points found in the reviewed CLI file beyond expected test-only `expect` calls and literal marker strings.

## crates/legion-vscode-compat/src/lib.rs

### Finding 14
- Category: failure-point
- Severity: high
- Line numbers: 148-173, 197-235
- Description: Manifest classification ignores executable extension entrypoints such as `main` and `browser`. A package with extension host code but only declarative-looking contributions can be classified as Tier0/`NoneRequired`, even though executing that extension would require a host policy decision.
- Suggested fix direction: Parse `main`/`browser`/extension entrypoint metadata and raise the required tier/status/capabilities when executable code is present, even if activation events are absent or declarative.

### Finding 15
- Category: failure-point
- Severity: low
- Line numbers: 148-152, 260-268
- Description: `required_string` filters out trim-empty fields but returns the original untrimmed string. Publisher/name/version values with leading or trailing whitespace can produce malformed extension ids and fail identity comparisons later in `load_open_vsx_extension`.
- Suggested fix direction: Normalize manifest identity fields with `trim()` before storing/comparing them, or reject values where trimming would change the identity.
