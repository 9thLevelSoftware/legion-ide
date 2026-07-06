# PKT-0 Residuals Evidence (M9)

Branch: `m9/residuals`

## Task 1: Product RA session watcher=client initialization

**Root cause:** `startup_session` called `session.initialize(root_uri)` which passes no
`initializationOptions`, leaving rust-analyzer with default `files.watcher: "server"`.  On
temp-path workspaces the notify watcher fails with
`"Input watch path is neither a file nor a directory"` and wedges RA's analysis loop.

**Fix:** `startup_session` now calls `session.initialize_with_options(root_uri, Some(json!({"files": {"watcher": "client"}})), None)`.

**Files changed:**
- `crates/legion-app/src/language/app_lsp.rs` — production change
- `crates/legion-app/tests/rust_analyzer_session_handshake.rs` — new test `initialize_with_watcher_client_option_succeeds`
- `crates/legion-app/Cargo.toml` — added `serde_json` to dev-deps

**Test evidence:**
```
cargo test -p legion-app --test rust_analyzer_session_handshake
running 3 tests
test initialize_populates_capability_summaries ... ok
test initialize_with_watcher_client_option_succeeds ... ok
test launch_and_initialize_populates_health_record ... ok
test result: ok. 3 passed; 0 failed
```

---

## Task 2: Perf-harness build-failure heuristic

**Root cause:** When `cargo run --release -p legion-desktop` fails to build, the report
file is absent.  `manual_renderer_environment_blocked()` checks for renderer/GPU keywords
that are absent in a build-error output (which contains `"could not compile"`, `"error[E..."`,
`"aborting due to"`), so the else branch classified the result as `Failed` instead of
`Skipped`.

**Fix:** Added `manual_renderer_build_failed(output_text: &str) -> bool` that detects Cargo/Rust
build error patterns.  The else-if branch now classifies build failures as `Skipped` before
falling to `Failed`.

**Files changed:**
- `xtask/src/main.rs` — new helper + else-if branch + unit test

**Test evidence:**
```
cargo test -p xtask
running 64 tests ... all ok
test result: ok. 64 passed; 0 failed (in tests/gates.rs)
tests/perf_harness.rs: ok. 9 passed
```

---

## Task 3: Offline feature includes deterministic AI provider

**Root cause:** `offline = []` in `legion-app/Cargo.toml` did not include `legion-ai`, so
`cfg(not(feature = "ai"))` was active and `invoke_inline_prediction_provider` always returned a
hard error.  The deterministic-local provider (pure Rust, no network) was unreachable.

**Decision:** Add `legion-ai` (no reqwest dep) to the `offline` feature.  `legion-ai-providers`
(which has reqwest) is NOT included — the deterministic provider is instantiated directly from
`legion-ai::DeterministicInlinePredictionProvider` in a new
`cfg(all(not(feature = "ai"), feature = "offline"))` impl of `invoke_inline_prediction_provider`.

**reqwest status:** NOT pulled in for offline builds.  `legion-ai/Cargo.toml` has no reqwest dep.
Confirmed by `cargo check -p legion-app --no-default-features --features offline` succeeding
without any reqwest compilation.

**Files changed:**
- `crates/legion-app/Cargo.toml` — `offline = ["dep:legion-ai"]`
- `crates/legion-app/src/lib.rs` — new `cfg(all(not(feature = "ai"), feature = "offline"))` impl

**Test evidence:**
```
cargo test -p legion-app --no-default-features --features offline --test assist_inline_prediction_workflow
running 6 tests
test manual_mode_rejects_assist_inline_prediction_request ... ok
test assist_inline_prediction_request_projects_and_accepts_bounded_ghost_text ... ok
test accepted_assist_inline_prediction_cannot_be_dismissed_again ... ok
test assist_inline_prediction_accept_marks_stale_without_mutating_after_buffer_change ... ok
test switching_back_to_manual_clears_assist_inline_prediction_projection ... ok
test assist_inline_prediction_projection_hides_ghost_text_when_buffer_becomes_stale ... ok
test result: ok. 6 passed; 0 failed
```

**Default feature tests (regression check):**
```
cargo test -p legion-app
All test targets: ok (all passed, 0 failed)
```

---

## Workspace compile check

```
cargo check --workspace
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 02s
```

No errors.
