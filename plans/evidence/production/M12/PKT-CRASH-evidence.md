# PKT-CRASH Evidence — Consent-Gated Local Crash Capture

**Branch:** `m12/crash-capture`
**Campaign:** M12 (packet 6/6, final)
**Date:** 2026-07-07

---

## What Was Implemented

### 1. Consent-Gated Panic Hook (`crates/legion-observability/src/crash_capture.rs`)

- `install_panic_hook(config)` — installs a `std::panic::set_hook` that writes crash bundles.
  - **Fail-closed**: returns `Ok(())` immediately (no hook installed) when `consent.crash_reports_enabled == false`.
  - Creates `bundle_dir` and installs the hook only when consent is on.
- `uninstall_panic_hook()` — restores the default panic hook via `take_hook()`.
- `write_crash_bundle` — internal; writes:
  - `panic.txt` — panic message, sanitized location, and `Backtrace::force_capture()` backtrace.
  - `summary.toml` — metadata-only: `crash_id`, `timestamp`, `version`, `os`, `arch`, `panic_message` (sanitized), `panic_location` (redacted to `<crate>/src/file.rs:line` form), `signer_status = "unsigned-beta"`.
- Path redaction: `redact_path()` strips absolute prefixes and keeps only `<crate>/src/file.rs` form.
- Message sanitization: `sanitize_message()` replaces path-like tokens with `<path>`.
- No new external dependencies — uses only `std::panic`, `std::backtrace`, and existing `uuid`.

### 2. Diagnostics Export (`crates/legion-observability/src/export.rs`)

- `DiagnosticsExportBuilder` — consent-aware export builder.
  - Default `build()` → metadata-only: only `summary.toml` paths returned, `raw_paths` is empty.
  - `.with_include_raw(true)` + `consent.raw_source_allowed == true` → includes raw files.
  - `.with_include_raw(true)` + `consent.raw_source_allowed == false` → `ExportError::RawNotAllowed` (no silent degradation).
- `DiagnosticsBundle` / `DiagnosticEntry` types.

### 3. App-Side Support Bundle Surface (`crates/legion-app/src/diagnostics.rs`)

- `SupportBundleAssembler` — thin wrapper; delegates to `DiagnosticsExportBuilder`.
  - `list_crash_reports()` — parses `summary.toml` for crash_id, timestamp, os (no raw contents returned).
  - `build_metadata_bundle()` — metadata-only.
  - `build_raw_bundle()` — requires raw consent.

### 4. Module Declarations

- `crates/legion-observability/src/lib.rs` — added `pub mod minidump; pub mod crash_capture; pub mod export;`.
- `crates/legion-app/src/lib.rs` — added `pub mod diagnostics;`.

---

## What Was Honestly Deferred

**D7 (binding from brief):** Native fault capture is out of scope for v1.

- `crash-handler` / `minidumper` / `minidump` crates: IPC watchdog subprocess complexity and platform-specific out-of-process crash capture are explicitly deferred.
- Minidump generation from native faults (SIGSEGV, access violations) is NOT implemented.
- The existing `minidump.rs` module (crash summary for VS Code extension hosts) is now declared as a public module but is NOT used by the Rust panic capture path — its `VsCodeExtensionCrashReport` DTO does not map to Rust panics.
- **Crash-report upload**: no upload path exists or may be added (by design — local-only constraint).
- **Symbol upload**: re-mapped to "local debug-id recording + operator runbook". Symbols are not uploaded anywhere.

---

## Test Coverage

**File:** `crates/legion-observability/tests/crash_capture_tests.rs`

All 8 tests pass: `cargo test -p legion-observability --test crash_capture_tests -- --test-threads=1`

### Consent-Off Tests (pinned first — TDD red→green)

| Test | Description | Result |
|------|-------------|--------|
| `consent_disabled_installs_no_hook` | Config with `crash_reports_enabled: false` → `install_panic_hook` returns Ok, bundle_dir NOT created, panic does NOT write files | PASS |
| `consent_disabled_leaves_default_hook` | Consent off → no hook installed → panic via `catch_unwind` → bundle_dir remains absent | PASS |

### Panic Capture Tests

| Test | Description | Result |
|------|-------------|--------|
| `induced_panic_produces_bundle` | Consent on → `catch_unwind(|| panic!(...))` → exactly one crash subdir with `panic.txt` and `summary.toml` | PASS |
| `panic_txt_contains_backtrace` | `panic.txt` contains "stack backtrace" or "backtrace" | PASS |
| `summary_toml_is_metadata_only` | `summary.toml` contains crash_id/timestamp/os/arch/version/signer_status; does NOT contain raw source code or absolute paths | PASS |

### Export Double-Opt-In Tests

| Test | Description | Result |
|------|-------------|--------|
| `export_default_is_metadata_only` | Default build → `metadata_only == true`, all `raw_paths` empty | PASS |
| `export_raw_requires_consent_and_flag` | `include_raw: true` but `raw_source_allowed: false` → `ExportError::RawNotAllowed` | PASS |
| `export_raw_allowed_when_both_set` | Both `include_raw: true` AND `raw_source_allowed: true` → includes raw paths | PASS |

### Unit Tests (inline in modules)

- `crash_capture.rs`: 4 unit tests (path redaction, message sanitization, TOML escaping)
- `export.rs`: 3 unit tests (metadata-only default, raw denied, raw allowed)
- `diagnostics.rs`: 4 unit tests (list rows, empty dir, metadata bundle, raw denied)

---

## Consent Gate Evidence (Fail-Closed)

- `consent_disabled_installs_no_hook` proves the fail-closed gate: when consent is off, `install_panic_hook` returns `Ok(())` without any side effects. The bundle directory is never created. A panic triggered after this call writes no files.
- Code path: `if !config.consent.crash_reports_enabled { return Ok(()); }` — the first statement in `install_panic_hook`.

---

## `manual_zero_egress` Status

**GREEN** — `cargo test -p legion-app --test manual_zero_egress` passes (1/1).

The new crash capture code does not add any network egress paths. Crash bundles are local files only. No HTTP client, no upload endpoint, no background transmission.

---

## Kanban Updates

- P8.F3.T1 → `status = "done"`
- P8.F3.T2 → `status = "done"` (with caveat: native-fault/minidump deferred per D7)
- P8.F3.T3 → `status = "done"`

All verified via `cargo run -p xtask -- verify-kanban-backlog` (green).

---

## Cargo-Deny Spike

`cargo deny check` was run against the workspace after PKT-CRASH landed.

**Result: PASS** — no new dependencies were introduced by this packet. All code in
`crash_capture.rs` and `export.rs` uses only crates that were already in the workspace
dependency tree (`serde_json`, `uuid`, `thiserror`, `legion-protocol`). No advisory
violations, license conflicts, or banned crates were added.

---

## PR-REL-001 Final M12 Refresh

The product-readiness-ledger.md PR-REL-001 row was updated with the complete M12 campaign summary:

- PKT-SIGN: unsigned-beta shipped-with-policy; real sha256/manifest-signing code paths in place, credentials pending.
- PKT-UPDATER: auto-update/rollback validated deterministically via update-drill (19th standing gate).
- PKT-CRASH: consent-gated panic capture, metadata-only export, double opt-in for raw data.

**Remaining gaps (honestly listed):**
1. Signed installers (credentials/notarization pending)
2. Fresh-VM Gatekeeper/SmartScreen evidence (no multi-OS CI)
3. Installer-swap/process-restart (ADR-0042 D5 deferred)
4. Native fault capture / minidump (explicitly deferred, D7)
5. Crash-report upload (local-only by design)
6. 3-OS CI smoke

---

## Files Changed

| File | Change |
|------|--------|
| `crates/legion-observability/src/crash_capture.rs` | NEW — consent-gated panic hook + bundle writer |
| `crates/legion-observability/src/export.rs` | NEW — diagnostics export with double-opt-in |
| `crates/legion-observability/src/lib.rs` | Added `pub mod minidump; pub mod crash_capture; pub mod export;` |
| `crates/legion-app/src/diagnostics.rs` | NEW — app-side support bundle surface |
| `crates/legion-app/src/lib.rs` | Added `pub mod diagnostics;` |
| `crates/legion-observability/tests/crash_capture_tests.rs` | NEW — 8 integration tests |
| `plans/kanban/legion-ga-backlog.toml` | P8.F3.T1/T2/T3 → done |
| `plans/product-readiness-ledger.md` | PR-REL-001 final M12 refresh |
| `plans/evidence/production/M12/PKT-CRASH-evidence.md` | NEW — this file |
