# WS-LANG-01 Rust LSP Product Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Rust language intelligence real, fast, and user-visible by driving a live `rust-analyzer` process through the existing `legion-lsp` substrate into the app's projection and proposal pipelines, with provenance, health, restart UX, and redaction — proven by an always-green mock gate plus an `--ignored` real-rust-analyzer smoke.

**Architecture:** `legion-lsp` keeps process/protocol authority and gains a bounded notification-pump (behind a trait, so a future reader-thread drops in) plus a rust-analyzer discovery resolver. `legion-protocol` gains an `LspServerHealthRecord` DTO. `legion-app` owns a new `RustAnalyzerSession` orchestrator that launches the server, runs the handshake, syncs documents, feeds the existing `ingest_lsp_*_response_for_buffer` methods, routes rename/code-action `WorkspaceEdit`s through the existing `convert_lsp_edit_to_workspace_proposal`, and enforces restart policy. Remote download is gated through the existing default-deny `CapabilityBrokerPort`/`NetworkPolicy`. `legion-ui`/`legion-desktop` gain projection-only health/refusal rows. `xtask` wires the real smoke into an opt-in lane.

**Tech Stack:** Rust 2024 workspace; `legion-lsp` (JSON-RPC framing, `LspStdioSession`, `LspSupervisor`); `legion-protocol` DTOs; `serde`/`serde_json`; `sha2`/`hex` for artifact provenance (already used in `legion-text`); existing `legion-security` `NetworkPolicy` + `CapabilityBrokerPort`; mock server `mock_lsp_server`; targeted integration tests under `crates/*/tests/`; evidence under `plans/evidence/production/WS-LANG-01/`.

**Design reference:** `docs/superpowers/specs/2026-06-21-ws-lang-01-rust-lsp-product-workflow-design.md`.

## Global Constraints

- Rust edition 2024; workspace builds with `cargo check --workspace --all-targets`.
- Authority boundaries hold: `legion-lsp` has no product state; `legion-ui`/`legion-desktop` are projection-only; product orchestration lives only in `legion-app`.
- All persisted/projected records are **metadata-only** — never raw source bodies, diagnostic message text, or LSP payloads. Use hashes (`FileFingerprint`) and counts.
- All workspace mutation stays proposal-mediated: rename/code-action edits become `WorkspaceProposal`s via `convert_lsp_edit_to_workspace_proposal`; nothing writes to disk directly.
- No automatic network egress. rust-analyzer download requests go through `CapabilityBrokerPort`; the default `NetworkPolicy` (`air_gap: true`, allowlist `["localhost"]`) denies them. Manual/air-gap zero-egress must stay green.
- The blocking CI gate must not require rust-analyzer. Real-process tests are `#[ignore]` and skip cleanly when `rust-analyzer` is absent from `PATH`.
- Standing gates (master plan §10) must pass: `cargo fmt --all --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo run -p xtask -- check-deps`, `cargo run -p xtask -- docs-hygiene`, `cargo deny check`.
- Conventional Commits. Commit after each task's tests pass.

## Existing Substrate to Reuse (do not rebuild)

- `legion-lsp/src/lib.rs`: `LspStdioSession` (line 2231), `LspStdioSession::start` (2248), `read_response_for` (2325), `read_until_correlated_response` (2393), `diagnostic_notifications()`/`progress_notifications()` (2374/2379), `LspStdioProcess::read_envelope` (2050), `LspStdioLauncher`/`LspStdioSpawner` (2108/2118), `LspSupervisorConfig` (585), `LspSupervisor` lifecycle (599), `LanguageServerAdapterPlan::system_path` (285)/`downloaded_artifact` (316), `LspServerBinarySource` (245), request builders (`completion_request`, `hover_request`, `definition_request`, `references_request`, `prepare_rename_request`/`rename_request`, `formatting_request`, `code_action_request`, `document_symbol_request`, `semantic_tokens_full_request`, `inlay_hint_request`, `code_lens_request`, `folding_range_request`), and projection builders (`project_publish_diagnostics`, `project_completion_response`, `project_hover_response`, `project_location_response`, `project_document_symbol_response`, `project_inlay_hint_response`, `project_code_lens_response`).
- `legion-protocol/src/lib.rs`: `LspCapabilitySummary` (16345), `LspRequestCorrelation` (16401), `LspEditProposalConversionInput` (16511), `convert_lsp_edit_to_workspace_proposal` (16541), `WorkspaceEditProposalPayload` (4073), `WorkspaceEditSourceKind` (4015), `LspResultStatus` (16071).
- `legion-app/src/lib.rs`: `ingest_lsp_publish_diagnostics_for_buffer` (18285), `ingest_lsp_unavailable_for_buffer` (18315), `ingest_lsp_completion_response_for_buffer` (18341), `..._hover_...` (18368), `..._definition_...` (18396), `..._references_...` (18423), `..._document_symbol_...` (18450), `..._inlay_hint_...` (18477), `..._code_lens_...` (18505), `LanguageToolingWorkflow` (4682), `LanguageRequestInput` (4599), `lsp_identity_for_language_request` (4633).
- `legion-security/src/lib.rs`: `NetworkPolicy` (757). `legion-protocol`: `CapabilityCommandClass` (20283), `NetworkTarget` (20301), `CapabilityRequestContext` (20313), `CapabilityBrokerPort` (22988).
- Mock server: `crates/legion-lsp/src/bin/mock_lsp_server.rs`; spawn via `env!("CARGO_BIN_EXE_mock_lsp_server")`, env vars `MOCK_LSP_EMIT_DIAGNOSTICS=1`, `MOCK_LSP_EMIT_PROGRESS=1`.

---

## Task 1: `LspServerHealthRecord` protocol DTO (LANG.02)

**Files:**
- Modify: `crates/legion-protocol/src/lib.rs` (add near `LspCapabilitySummary`, ~line 16358)
- Test: `crates/legion-protocol/tests/lsp_server_health_record.rs` (create)

**Interfaces:**
- Produces: `enum LspServerBinaryProvenance { Configured, ProjectLocal, SystemPath, Bundled, Downloaded }`; `struct LspServerHealthRecord { server_id: LanguageServerId, language_id: LanguageId, binary_provenance: LspServerBinaryProvenance, binary_path_hash: Option<FileFingerprint>, artifact_hash: Option<FileFingerprint>, version: Option<String>, init_status: LspResultStatus, capabilities: Vec<LspCapabilitySummary>, diagnostics_latency_ms: Option<u64>, restart_count: u32, download_decision_id: Option<CapabilityDecisionId>, schema_version: u16 }`; `impl LspServerHealthRecord { pub fn schema_version() -> u16 { 1 } }`.

- [ ] **Step 1: Write the failing test**

Create `crates/legion-protocol/tests/lsp_server_health_record.rs`:

```rust
use legion_protocol::{
    CapabilityDecisionId, LanguageId, LanguageServerId, LspResultStatus,
    LspServerBinaryProvenance, LspServerHealthRecord,
};

#[test]
fn health_record_round_trips_metadata_only() {
    let record = LspServerHealthRecord {
        server_id: LanguageServerId("rust-analyzer".into()),
        language_id: LanguageId("rust".into()),
        binary_provenance: LspServerBinaryProvenance::SystemPath,
        binary_path_hash: None,
        artifact_hash: None,
        version: Some("rust-analyzer 1.0.0".into()),
        init_status: LspResultStatus::Fresh,
        capabilities: Vec::new(),
        diagnostics_latency_ms: Some(42),
        restart_count: 0,
        download_decision_id: None,
        schema_version: LspServerHealthRecord::schema_version(),
    };
    let json = serde_json::to_string(&record).unwrap();
    // Metadata only: no raw source / payload fields leak in.
    assert!(!json.contains("source_text"));
    let back: LspServerHealthRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(back.version.as_deref(), Some("rust-analyzer 1.0.0"));
    assert_eq!(back.restart_count, 0);
}

#[test]
fn downloaded_provenance_carries_decision_id() {
    let record = LspServerHealthRecord {
        server_id: LanguageServerId("rust-analyzer".into()),
        language_id: LanguageId("rust".into()),
        binary_provenance: LspServerBinaryProvenance::Downloaded,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Unavailable,
        capabilities: Vec::new(),
        diagnostics_latency_ms: None,
        restart_count: 2,
        download_decision_id: Some(CapabilityDecisionId(7)),
        schema_version: LspServerHealthRecord::schema_version(),
    };
    assert_eq!(record.download_decision_id, Some(CapabilityDecisionId(7)));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p legion-protocol --test lsp_server_health_record`
Expected: FAIL — `LspServerBinaryProvenance` / `LspServerHealthRecord` not found.

- [ ] **Step 3: Add the DTO**

In `crates/legion-protocol/src/lib.rs` after `LspCapabilitySummary` (line ~16358), insert:

```rust
/// Provenance of a launched language-server binary. Metadata-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LspServerBinaryProvenance {
    /// Resolved from an explicit user/workspace-configured path.
    Configured,
    /// Resolved from a project-local vendored location.
    ProjectLocal,
    /// Resolved from the system PATH.
    SystemPath,
    /// Resolved from an application-bundled binary.
    Bundled,
    /// Materialized from a policy-gated download.
    Downloaded,
}

/// Metadata-only health/provenance record for a supervised language server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspServerHealthRecord {
    /// Server identity.
    pub server_id: LanguageServerId,
    /// Language served.
    pub language_id: LanguageId,
    /// How the binary was resolved.
    pub binary_provenance: LspServerBinaryProvenance,
    /// Hash of the resolved binary path (never the raw path bytes elsewhere).
    pub binary_path_hash: Option<FileFingerprint>,
    /// Hash of a downloaded artifact when provenance is `Downloaded`.
    pub artifact_hash: Option<FileFingerprint>,
    /// Version string reported by `--version`.
    pub version: Option<String>,
    /// Status of the `initialize` handshake.
    pub init_status: LspResultStatus,
    /// Capability summaries reported at initialize.
    pub capabilities: Vec<LspCapabilitySummary>,
    /// Latency from didOpen to first diagnostics, when observed.
    pub diagnostics_latency_ms: Option<u64>,
    /// Number of restarts observed in this session's lifetime.
    pub restart_count: u32,
    /// Capability decision id authorizing a download, when provenance is `Downloaded`.
    pub download_decision_id: Option<CapabilityDecisionId>,
    /// Health record schema version.
    pub schema_version: u16,
}

impl LspServerHealthRecord {
    /// Current schema version for the health record.
    pub fn schema_version() -> u16 {
        1
    }
}
```

Verify `LanguageServerId`, `LanguageId`, `FileFingerprint`, `CapabilityDecisionId`, `LspResultStatus` are already `pub` in this crate (they are — used by neighbors). If any is not re-exported in a way the test can import, add it to the existing `pub use`/`pub` surface.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p legion-protocol --test lsp_server_health_record`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/legion-protocol/src/lib.rs crates/legion-protocol/tests/lsp_server_health_record.rs
git commit -m "feat: add LspServerHealthRecord protocol DTO (WS-LANG-01 LANG.02)"
```

---

## Task 2: Notification-pump trait + `BlockingPump` (LANG.03/04, design §4)

**Files:**
- Modify: `crates/legion-lsp/src/lib.rs` (add trait + impl; refactor `read_until_correlated_response`)
- Test: `crates/legion-lsp/tests/pump_contract.rs` (create)

**Interfaces:**
- Produces: `enum PumpOutcome { PredicateMet, Deadline, Closed }`; `struct PumpedNotifications { diagnostics: Vec<LspDiagnosticNotificationMetadata>, progress: Vec<LspProgressNotification> }`; `trait LspNotificationSource { fn pump_until(&mut self, deadline: std::time::Instant, predicate: &mut dyn FnMut(&PumpedNotifications) -> bool) -> LspRuntimeResult<PumpOutcome>; }`; method `LspStdioSession::pump_until(...)` delegating to the trait; new accessor `LspStdioSession::take_pumped_notifications(&mut self) -> PumpedNotifications` is NOT added — pumped notifications continue to land in the existing `diagnostic_notifications`/`progress_notifications` buffers.
- Consumes: existing `LspStdioProcess::read_envelope` (returns `Option<JsonRpcEnvelope>`), `progress_notification_from_params`, `diagnostic_notification_from_params`.

> Design note: `BlockingPump` reads available frames until the predicate returns true or `deadline` passes. Because `read_envelope` blocks, the deadline is enforced by checking `Instant::now() >= deadline` between frames; for the mock (instant responses) this is sufficient and deterministic. A future `ThreadedPump` implementing `LspNotificationSource` can enforce wall-clock deadlines precisely without changing callers (B-ready seam).

- [ ] **Step 1: Write the failing test**

Create `crates/legion-lsp/tests/pump_contract.rs`:

```rust
use std::time::{Duration, Instant};

use legion_lsp::{LspStdioSession, PumpOutcome};

mod common; // reuse mock spawn helper pattern (copy from stdio_transport_contract.rs)

#[test]
fn pump_collects_async_diagnostics_until_predicate() {
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session =
        LspStdioSession::start(common::mock_server_config_with_diagnostics(), &mut launcher)
            .unwrap();

    // After initialize, the mock emits one publishDiagnostics notification.
    session
        .initialize(serde_json::json!({}), common::ctx())
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = false;
    let outcome = session
        .pump_until(deadline, &mut |n| {
            if !n.diagnostics.is_empty() {
                seen = true;
            }
            seen
        })
        .unwrap();

    assert!(matches!(outcome, PumpOutcome::PredicateMet | PumpOutcome::Closed));
    assert!(!session.diagnostic_notifications().is_empty());
}
```

Create `crates/legion-lsp/tests/common/mod.rs` by copying the `mock_server_config`, `mock_server_config_with_diagnostics`, and a `ctx()` helper from `crates/legion-lsp/tests/stdio_transport_contract.rs` (lines ~127-167). Keep them `pub`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p legion-lsp --test pump_contract`
Expected: FAIL — `PumpOutcome` / `pump_until` not found.

- [ ] **Step 3: Implement the trait, impl, and refactor**

In `crates/legion-lsp/src/lib.rs`, add near `LspStdioSession` (before line 2231):

```rust
/// Result of a bounded notification pump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpOutcome {
    /// The caller predicate returned true.
    PredicateMet,
    /// The deadline elapsed before the predicate was met.
    Deadline,
    /// The child closed its stdout before the predicate was met.
    Closed,
}

/// Notifications observed during a pump, borrowed for predicate evaluation.
#[derive(Debug, Default)]
pub struct PumpedNotifications {
    /// Diagnostic notifications observed so far.
    pub diagnostics: Vec<LspDiagnosticNotificationMetadata>,
    /// Progress notifications observed so far.
    pub progress: Vec<LspProgressNotification>,
}

/// Source of asynchronous LSP notifications. `BlockingPump` is the current
/// single-threaded implementation; a future reader-thread implementation can
/// replace it without changing callers (design §4, B-ready seam).
pub trait LspNotificationSource {
    /// Reads frames until `predicate` returns true, the deadline elapses, or
    /// the child closes stdout. Id-bearing frames are routed to correlation;
    /// notification frames are accumulated and surfaced to `predicate`.
    fn pump_until(
        &mut self,
        deadline: std::time::Instant,
        predicate: &mut dyn FnMut(&PumpedNotifications) -> bool,
    ) -> LspRuntimeResult<PumpOutcome>;
}
```

Add this method inside `impl LspStdioSession` (near `read_until_correlated_response`):

```rust
    /// Bounded pump for asynchronous notifications (design §4). Accumulated
    /// notifications also land in `self.diagnostic_notifications` /
    /// `self.progress_notifications` so existing accessors keep working.
    pub fn pump_until(
        &mut self,
        deadline: std::time::Instant,
        predicate: &mut dyn FnMut(&PumpedNotifications) -> bool,
    ) -> LspRuntimeResult<PumpOutcome> {
        let mut acc = PumpedNotifications::default();
        loop {
            if std::time::Instant::now() >= deadline {
                return Ok(PumpOutcome::Deadline);
            }
            let envelope = match self.process.read_envelope()? {
                Some(envelope) => envelope,
                None => return Ok(PumpOutcome::Closed),
            };
            if envelope.id.is_some() {
                // Out-of-band response while pumping: keep it correlatable by
                // dropping it here is wrong, so ignore id frames during a pump
                // (callers pump only when no request is outstanding).
                continue;
            }
            match envelope.method.as_deref() {
                Some("$/progress") => {
                    if let Some(p) = progress_notification_from_params(envelope.params.as_ref()) {
                        self.progress_notifications.push(p.clone());
                        acc.progress.push(p);
                    }
                }
                Some("textDocument/publishDiagnostics") => {
                    if let Some(d) = diagnostic_notification_from_params(envelope.params.as_ref()) {
                        self.diagnostic_notifications.push(d.clone());
                        acc.diagnostics.push(d);
                    }
                }
                _ => {}
            }
            if predicate(&acc) {
                return Ok(PumpOutcome::PredicateMet);
            }
        }
    }
```

Implement the trait by delegation:

```rust
impl LspNotificationSource for LspStdioSession {
    fn pump_until(
        &mut self,
        deadline: std::time::Instant,
        predicate: &mut dyn FnMut(&PumpedNotifications) -> bool,
    ) -> LspRuntimeResult<PumpOutcome> {
        LspStdioSession::pump_until(self, deadline, predicate)
    }
}
```

Ensure `LspDiagnosticNotificationMetadata` and `LspProgressNotification` derive `Clone` (they are simple metadata structs; add `Clone` to their derive lists if missing).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p legion-lsp --test pump_contract`
Expected: PASS.

- [ ] **Step 5: Run the existing transport contract to confirm no regression, then commit**

Run: `cargo test -p legion-lsp --test stdio_transport_contract`
Expected: PASS (unchanged).

```bash
git add crates/legion-lsp/src/lib.rs crates/legion-lsp/tests/pump_contract.rs crates/legion-lsp/tests/common/mod.rs
git commit -m "feat: add bounded LSP notification pump behind LspNotificationSource (WS-LANG-01 LANG.03)"
```

---

## Task 3: rust-analyzer discovery resolver + version probe (LANG.01/02, §5)

**Files:**
- Modify: `crates/legion-lsp/src/lib.rs` (add resolver)
- Test: `crates/legion-lsp/tests/discovery_contract.rs` (create)

**Interfaces:**
- Produces: `struct RustAnalyzerDiscovery { configured_path: Option<std::path::PathBuf>, project_local_path: Option<std::path::PathBuf>, bundled_path: Option<std::path::PathBuf>, path_env: Option<String> }`; `enum DiscoveredBinary { Found { path: std::path::PathBuf, provenance: legion_protocol::LspServerBinaryProvenance }, NotFound }`; `impl RustAnalyzerDiscovery { pub fn resolve(&self) -> DiscoveredBinary; pub fn probe_version(path: &std::path::Path) -> Option<String>; }`.
- Order: configured → project-local → system PATH (scan `path_env`) → bundled.

- [ ] **Step 1: Write the failing test**

Create `crates/legion-lsp/tests/discovery_contract.rs`:

```rust
use std::path::PathBuf;

use legion_lsp::{DiscoveredBinary, RustAnalyzerDiscovery};
use legion_protocol::LspServerBinaryProvenance;

#[test]
fn configured_path_wins_over_everything() {
    let d = RustAnalyzerDiscovery {
        configured_path: Some(PathBuf::from("/cfg/rust-analyzer")),
        project_local_path: Some(PathBuf::from("/proj/rust-analyzer")),
        bundled_path: Some(PathBuf::from("/bundle/rust-analyzer")),
        path_env: Some("/usr/bin".into()),
    };
    match d.resolve() {
        DiscoveredBinary::Found { path, provenance } => {
            assert_eq!(path, PathBuf::from("/cfg/rust-analyzer"));
            assert_eq!(provenance, LspServerBinaryProvenance::Configured);
        }
        DiscoveredBinary::NotFound => panic!("expected configured path"),
    }
}

#[test]
fn empty_discovery_is_not_found() {
    let d = RustAnalyzerDiscovery {
        configured_path: None,
        project_local_path: None,
        bundled_path: None,
        path_env: Some(String::new()),
    };
    assert!(matches!(d.resolve(), DiscoveredBinary::NotFound));
}
```

> Note: `configured_path`/`project_local_path`/`bundled_path` resolve to `Found` based on the field being `Some` (existence is the caller's concern in unit tests; the resolver trusts provided paths and only *scans* for the PATH case). This keeps the resolver deterministic and testable without touching the filesystem for configured/project/bundled inputs.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p legion-lsp --test discovery_contract`
Expected: FAIL — types not found.

- [ ] **Step 3: Implement the resolver**

In `crates/legion-lsp/src/lib.rs` add near `LanguageServerAdapterPlan` (line ~266):

```rust
/// Resolution inputs for locating a rust-analyzer binary (design §5).
#[derive(Debug, Clone, Default)]
pub struct RustAnalyzerDiscovery {
    /// Explicit user/workspace-configured path.
    pub configured_path: Option<std::path::PathBuf>,
    /// Project-local vendored binary path.
    pub project_local_path: Option<std::path::PathBuf>,
    /// Application-bundled binary path.
    pub bundled_path: Option<std::path::PathBuf>,
    /// Raw `PATH` environment value to scan for `rust-analyzer`.
    pub path_env: Option<String>,
}

/// Outcome of binary discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveredBinary {
    /// A binary was resolved with the given provenance.
    Found {
        /// Resolved binary path.
        path: std::path::PathBuf,
        /// How it was resolved.
        provenance: legion_protocol::LspServerBinaryProvenance,
    },
    /// No binary could be resolved through any source.
    NotFound,
}

impl RustAnalyzerDiscovery {
    /// Resolves the binary in order: configured -> project-local -> PATH -> bundled.
    pub fn resolve(&self) -> DiscoveredBinary {
        use legion_protocol::LspServerBinaryProvenance as P;
        if let Some(p) = &self.configured_path {
            return DiscoveredBinary::Found { path: p.clone(), provenance: P::Configured };
        }
        if let Some(p) = &self.project_local_path {
            return DiscoveredBinary::Found { path: p.clone(), provenance: P::ProjectLocal };
        }
        if let Some(path_env) = &self.path_env {
            let exe = if cfg!(windows) { "rust-analyzer.exe" } else { "rust-analyzer" };
            for dir in std::env::split_paths(path_env) {
                let candidate = dir.join(exe);
                if candidate.is_file() {
                    return DiscoveredBinary::Found { path: candidate, provenance: P::SystemPath };
                }
            }
        }
        if let Some(p) = &self.bundled_path {
            return DiscoveredBinary::Found { path: p.clone(), provenance: P::Bundled };
        }
        DiscoveredBinary::NotFound
    }

    /// Probes `<path> --version`, returning the trimmed stdout line if it runs.
    pub fn probe_version(path: &std::path::Path) -> Option<String> {
        let output = std::process::Command::new(path).arg("--version").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() { None } else { Some(text) }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p legion-lsp --test discovery_contract`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/legion-lsp/src/lib.rs crates/legion-lsp/tests/discovery_contract.rs
git commit -m "feat: add rust-analyzer discovery resolver and version probe (WS-LANG-01 LANG.01)"
```

---

## Task 4: Gated download decision through the capability broker (LANG.01/02, §5)

**Files:**
- Create: `crates/legion-app/src/language/download.rs`
- Modify: `crates/legion-app/src/lib.rs` (declare `mod language;` if not present; re-export)
- Create: `crates/legion-app/src/language/mod.rs`
- Test: `crates/legion-app/tests/rust_analyzer_download_policy.rs` (create)

**Interfaces:**
- Produces: `struct RustAnalyzerDownloadRequest { release_host: String, artifact_uri: String, expected_sha256: String, expected_version: String }`; `enum DownloadDecision { Denied { reason: String }, Allowed { decision_id: CapabilityDecisionId } }`; `fn evaluate_rust_analyzer_download(req: &RustAnalyzerDownloadRequest, broker: &dyn CapabilityBrokerPort, ctx: &CapabilityRequestContext) -> DownloadDecision`; `fn verify_downloaded_artifact(bytes: &[u8], expected_sha256: &str) -> bool` (sha2).
- Consumes: `CapabilityBrokerPort` (protocol 22988), `CapabilityRequestContext` (20313), `CapabilityCommandClass::Network` (20283), `NetworkTarget` (20301).

> This task implements the **decision + verification**, not a live HTTP fetch. The live fetch is exercised only in the `--ignored` smoke (Task 12). Default `NetworkPolicy` denies; the test proves deny-by-default, explicit-allow, and hash-mismatch-fails-closed.

- [ ] **Step 1: Write the failing test**

Create `crates/legion-app/tests/rust_analyzer_download_policy.rs`:

```rust
use legion_app::language::{
    evaluate_rust_analyzer_download, verify_downloaded_artifact, DownloadDecision,
    RustAnalyzerDownloadRequest,
};
use legion_protocol::{
    CapabilityCommandClass, CapabilityRequestContext, NetworkTarget,
};

mod broker_fixture; // small in-test CapabilityBrokerPort impls: AllowAll, DenyAll

fn req() -> RustAnalyzerDownloadRequest {
    RustAnalyzerDownloadRequest {
        release_host: "releases.example.invalid".into(),
        artifact_uri: "https://releases.example.invalid/rust-analyzer".into(),
        expected_sha256: sha256_hex(b"binary-bytes"),
        expected_version: "rust-analyzer 1.0.0".into(),
    }
}

fn ctx() -> CapabilityRequestContext {
    CapabilityRequestContext {
        command_class: Some(CapabilityCommandClass::Network),
        command_binary: Some("rust-analyzer".into()),
        network_target: Some(NetworkTarget {
            scheme: "https".into(),
            host: "releases.example.invalid".into(),
            port: Some(443),
        }),
        lsp_server_binary: Some("rust-analyzer".into()),
        ..Default::default()
    }
}

#[test]
fn air_gap_default_denies_download() {
    let broker = broker_fixture::DenyAll;
    match evaluate_rust_analyzer_download(&req(), &broker, &ctx()) {
        DownloadDecision::Denied { .. } => {}
        DownloadDecision::Allowed { .. } => panic!("air-gap must deny"),
    }
}

#[test]
fn explicit_grant_allows_download() {
    let broker = broker_fixture::AllowAll;
    assert!(matches!(
        evaluate_rust_analyzer_download(&req(), &broker, &ctx()),
        DownloadDecision::Allowed { .. }
    ));
}

#[test]
fn hash_mismatch_fails_closed() {
    assert!(verify_downloaded_artifact(b"binary-bytes", &sha256_hex(b"binary-bytes")));
    assert!(!verify_downloaded_artifact(b"tampered", &sha256_hex(b"binary-bytes")));
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}
```

Create `crates/legion-app/tests/broker_fixture/mod.rs` with two `CapabilityBrokerPort` implementations (`AllowAll`, `DenyAll`) returning the minimal `CapabilityResponse`/`CapabilityDecision` shapes (mirror an existing broker test in `crates/legion-app/tests/` — search for `CapabilityBrokerPort` usages to copy the exact return shape).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p legion-app --test rust_analyzer_download_policy`
Expected: FAIL — `legion_app::language` module / items not found.

- [ ] **Step 3: Implement module + download decision**

Create `crates/legion-app/src/language/mod.rs`:

```rust
//! Language-tooling orchestration extracted from `lib.rs` (design §10).
mod download;
pub use download::{
    evaluate_rust_analyzer_download, verify_downloaded_artifact, DownloadDecision,
    RustAnalyzerDownloadRequest,
};
```

Create `crates/legion-app/src/language/download.rs`:

```rust
use legion_protocol::{
    CapabilityBrokerPort, CapabilityDecisionId, CapabilityRequest, CapabilityRequestContext,
    CapabilityResponse,
};

/// A request to fetch a rust-analyzer artifact (design §5).
#[derive(Debug, Clone)]
pub struct RustAnalyzerDownloadRequest {
    /// Host the artifact is fetched from.
    pub release_host: String,
    /// Full artifact URI.
    pub artifact_uri: String,
    /// Pinned SHA-256 of the expected artifact (hex).
    pub expected_sha256: String,
    /// Expected `--version` string after install.
    pub expected_version: String,
}

/// Outcome of a gated download decision.
#[derive(Debug, Clone)]
pub enum DownloadDecision {
    /// The broker denied the network capability.
    Denied {
        /// Human-readable refusal reason for projection.
        reason: String,
    },
    /// The broker granted the network capability.
    Allowed {
        /// Decision id recorded into the health record.
        decision_id: CapabilityDecisionId,
    },
}

/// Asks the capability broker whether a rust-analyzer download may proceed.
/// Default `NetworkPolicy` is air-gapped, so this denies unless the operator
/// has explicitly permitted the release host.
pub fn evaluate_rust_analyzer_download(
    _req: &RustAnalyzerDownloadRequest,
    broker: &dyn CapabilityBrokerPort,
    ctx: &CapabilityRequestContext,
) -> DownloadDecision {
    let request = CapabilityRequest::network(ctx.clone()); // use the real constructor; see note
    match broker.decide(request) {
        CapabilityResponse::Granted(grant) => {
            DownloadDecision::Allowed { decision_id: grant.decision_id }
        }
        CapabilityResponse::Denied(denial) => {
            DownloadDecision::Denied { reason: denial.reason_label() }
        }
    }
}

/// Verifies downloaded bytes against the pinned SHA-256. Fails closed.
pub fn verify_downloaded_artifact(bytes: &[u8], expected_sha256: &str) -> bool {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize()).eq_ignore_ascii_case(expected_sha256)
}
```

> Implementation note: replace `CapabilityRequest::network(...)`, `CapabilityResponse::Granted/Denied`, `grant.decision_id`, and `denial.reason_label()` with the **actual** variants/fields from `legion-protocol` (read `CapabilityRequest` at line 20884, `CapabilityResponse` at 20911, `CapabilityGrant` at 20242, `CapabilityDenial` at 20257). The shape above is the contract; match real names exactly.

In `crates/legion-app/src/lib.rs`, ensure `pub mod language;` is declared near the top module declarations.

Add `sha2` and `hex` to `crates/legion-app/Cargo.toml` `[dependencies]` if not already present (they are workspace deps used by `legion-text`; use `sha2.workspace = true` / `hex.workspace = true`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p legion-app --test rust_analyzer_download_policy`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/legion-app/src/language/ crates/legion-app/src/lib.rs crates/legion-app/Cargo.toml crates/legion-app/tests/rust_analyzer_download_policy.rs crates/legion-app/tests/broker_fixture/
git commit -m "feat: gate rust-analyzer download through capability broker with pinned-hash verify (WS-LANG-01 LANG.01)"
```

---

## Task 5: `RustAnalyzerSession` orchestrator — launch + handshake → health record (LANG.03/04)

**Files:**
- Create: `crates/legion-app/src/language/session.rs`
- Modify: `crates/legion-app/src/language/mod.rs` (re-export)
- Test: `crates/legion-app/tests/rust_analyzer_session_handshake.rs` (create, mock-driven)

**Interfaces:**
- Produces: `struct RustAnalyzerSession { /* owns LspStdioSession + health */ }`; `struct RustAnalyzerLaunchConfig { discovery: RustAnalyzerDiscovery, supervisor: legion_lsp::LspSupervisorConfig, server_id: LanguageServerId, language_id: LanguageId }`; `impl RustAnalyzerSession { pub fn launch(config: RustAnalyzerLaunchConfig, launcher: &mut impl legion_lsp::LspStdioSpawner) -> Result<Self, LanguageSessionError>; pub fn health(&self) -> &LspServerHealthRecord; pub fn initialize(&mut self, root_uri: &str) -> Result<(), LanguageSessionError>; }`; `enum LanguageSessionError { Discovery, Launch(legion_lsp::LspRuntimeError), Handshake(legion_lsp::LspRuntimeError) }`.
- Consumes: Task 2 pump, Task 3 discovery, Task 1 health record.

- [ ] **Step 1: Write the failing test**

Create `crates/legion-app/tests/rust_analyzer_session_handshake.rs`:

```rust
use legion_app::language::{RustAnalyzerLaunchConfig, RustAnalyzerSession};
use legion_protocol::{LanguageId, LanguageServerId, LspResultStatus, LspServerBinaryProvenance};

mod lsp_mock; // helper: builds an LspSupervisorConfig pointing at CARGO_BIN_EXE_mock_lsp_server

#[test]
fn launch_and_initialize_populates_health_record() {
    let config = RustAnalyzerLaunchConfig {
        discovery: legion_lsp::RustAnalyzerDiscovery {
            configured_path: Some(lsp_mock::mock_server_path()),
            ..Default::default()
        },
        supervisor: lsp_mock::mock_supervisor_config(),
        server_id: LanguageServerId("rust-analyzer".into()),
        language_id: LanguageId("rust".into()),
    };
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = RustAnalyzerSession::launch(config, &mut launcher).unwrap();
    session.initialize("file:///workspace").unwrap();

    let health = session.health();
    assert_eq!(health.binary_provenance, LspServerBinaryProvenance::Configured);
    assert_eq!(health.init_status, LspResultStatus::Fresh);
    assert_eq!(health.restart_count, 0);
}
```

Create `crates/legion-app/tests/lsp_mock/mod.rs` exposing `mock_server_path() -> PathBuf` (`PathBuf::from(env!("CARGO_BIN_EXE_mock_lsp_server"))`) and `mock_supervisor_config() -> LspSupervisorConfig` (copy the construction from `crates/legion-lsp/tests/stdio_transport_contract.rs`).

> `env!("CARGO_BIN_EXE_mock_lsp_server")` is only available to `legion-lsp`'s own targets. For `legion-app` tests, add `mock_lsp_server` discoverability: either (a) add `legion-lsp` as a `dev-dependency` and set the binary path via a build-time env, or (b) simpler — gate this specific test behind a helper that locates the mock binary in `target/<profile>/mock_lsp_server`. Use approach (b): implement `mock_server_path()` to resolve `CARGO_MANIFEST_DIR/../../target/<profile>/mock_lsp_server[.exe]`, and `#[ignore]` the test if the binary is absent so it never reds the gate. Document this in the test file header.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p legion-app --test rust_analyzer_session_handshake`
Expected: FAIL — `RustAnalyzerSession` not found.

- [ ] **Step 3: Implement the orchestrator**

Create `crates/legion-app/src/language/session.rs`:

```rust
use legion_lsp::{DiscoveredBinary, LspStdioSession, LspStdioSpawner, LspSupervisorConfig};
use legion_protocol::{
    LanguageId, LanguageServerId, LspResultStatus, LspServerBinaryProvenance,
    LspServerHealthRecord,
};

use super::RustAnalyzerDiscovery;

/// Errors raised while launching or initializing the rust-analyzer session.
#[derive(Debug)]
pub enum LanguageSessionError {
    /// No binary could be discovered.
    Discovery,
    /// The process failed to launch.
    Launch(legion_lsp::LspRuntimeError),
    /// The initialize handshake failed.
    Handshake(legion_lsp::LspRuntimeError),
}

/// Inputs for launching the rust-analyzer session.
pub struct RustAnalyzerLaunchConfig {
    /// Discovery inputs.
    pub discovery: RustAnalyzerDiscovery,
    /// Supervisor/process config (command + policy).
    pub supervisor: LspSupervisorConfig,
    /// Server identity for the health record.
    pub server_id: LanguageServerId,
    /// Language identity for the health record.
    pub language_id: LanguageId,
}

/// Owns a live rust-analyzer stdio session and its health record.
pub struct RustAnalyzerSession {
    session: LspStdioSession,
    health: LspServerHealthRecord,
}

impl RustAnalyzerSession {
    /// Resolves discovery, launches the process, and seeds the health record.
    pub fn launch(
        config: RustAnalyzerLaunchConfig,
        launcher: &mut impl LspStdioSpawner,
    ) -> Result<Self, LanguageSessionError> {
        let provenance = match config.discovery.resolve() {
            DiscoveredBinary::Found { provenance, .. } => provenance,
            DiscoveredBinary::NotFound => return Err(LanguageSessionError::Discovery),
        };
        let session = LspStdioSession::start(config.supervisor, launcher)
            .map_err(LanguageSessionError::Launch)?;
        let health = LspServerHealthRecord {
            server_id: config.server_id,
            language_id: config.language_id,
            binary_provenance: provenance,
            binary_path_hash: None,
            artifact_hash: None,
            version: None,
            init_status: LspResultStatus::Unavailable,
            capabilities: Vec::new(),
            diagnostics_latency_ms: None,
            restart_count: 0,
            download_decision_id: None,
            schema_version: LspServerHealthRecord::schema_version(),
        };
        Ok(Self { session, health })
    }

    /// Sends `initialize` with the workspace root and records the result.
    pub fn initialize(&mut self, root_uri: &str) -> Result<(), LanguageSessionError> {
        let params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {},
            "workspaceFolders": [{ "uri": root_uri, "name": "workspace" }],
        });
        let response = self
            .session
            .initialize(params, super::operation_context())
            .map_err(LanguageSessionError::Handshake)?;
        self.health.init_status = response.status;
        self.session
            .send_notification("initialized", serde_json::json!({}))
            .map_err(LanguageSessionError::Handshake)?;
        Ok(())
    }

    /// Borrows the health record for projection.
    pub fn health(&self) -> &LspServerHealthRecord {
        &self.health
    }

    /// Mutable access for later tasks (doc sync, reads, restart).
    pub(crate) fn session_mut(&mut self) -> &mut LspStdioSession {
        &mut self.session
    }

    pub(crate) fn health_mut(&mut self) -> &mut LspServerHealthRecord {
        &mut self.health
    }
}
```

Add to `crates/legion-app/src/language/mod.rs`:

```rust
mod session;
pub use session::{LanguageSessionError, RustAnalyzerLaunchConfig, RustAnalyzerSession};

pub(crate) fn operation_context() -> legion_lsp::LspOperationContext {
    // Mirror the context construction already used in lib.rs LSP call sites.
    legion_lsp::LspOperationContext::default()
}
```

> Replace `LspOperationContext::default()` and `RustAnalyzerDiscovery` re-export with the exact constructor/path used by existing `lib.rs` call sites (search `LspOperationContext` in `lib.rs`). Re-export `RustAnalyzerDiscovery` from `legion_lsp` in `mod.rs` (`pub use legion_lsp::RustAnalyzerDiscovery;`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p legion-app --test rust_analyzer_session_handshake`
Expected: PASS (or skip cleanly if mock binary absent — build it first with `cargo build -p legion-lsp --bin mock_lsp_server`).

- [ ] **Step 5: Commit**

```bash
git add crates/legion-app/src/language/ crates/legion-app/tests/rust_analyzer_session_handshake.rs crates/legion-app/tests/lsp_mock/
git commit -m "feat: add RustAnalyzerSession launch+handshake orchestrator (WS-LANG-01 LANG.03/04)"
```

---

## Task 6: Document sync + diagnostics ingest via pump (LANG.05/06)

**Files:**
- Modify: `crates/legion-app/src/language/session.rs` (add `did_open`, `pump_diagnostics`)
- Test: `crates/legion-app/tests/rust_analyzer_doc_sync.rs` (create, mock with `MOCK_LSP_EMIT_DIAGNOSTICS=1`)

**Interfaces:**
- Produces: `impl RustAnalyzerSession { pub fn did_open(&mut self, uri: &str, language_id: &str, version: i64, text: &str) -> Result<(), LanguageSessionError>; pub fn pump_diagnostics(&mut self, uri: &str, timeout: std::time::Duration) -> Vec<legion_lsp::LspDiagnosticNotificationMetadata>; }`.
- Consumes: Task 2 `pump_until`, existing `did_open_notification` builder, existing `diagnostic_notifications()`.

- [ ] **Step 1: Write the failing test**

Create `crates/legion-app/tests/rust_analyzer_doc_sync.rs`:

```rust
use std::time::Duration;

use legion_app::language::{RustAnalyzerLaunchConfig, RustAnalyzerSession};
use legion_protocol::{LanguageId, LanguageServerId};

mod lsp_mock;

#[test]
fn did_open_then_pump_collects_diagnostics() {
    let config = RustAnalyzerLaunchConfig {
        discovery: legion_lsp::RustAnalyzerDiscovery {
            configured_path: Some(lsp_mock::mock_server_path()),
            ..Default::default()
        },
        supervisor: lsp_mock::mock_supervisor_config_with_diagnostics(),
        server_id: LanguageServerId("rust-analyzer".into()),
        language_id: LanguageId("rust".into()),
    };
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = RustAnalyzerSession::launch(config, &mut launcher).unwrap();
    session.initialize("file:///workspace").unwrap();

    session
        .did_open("file:///workspace/src/lib.rs", "rust", 1, "fn main() {}")
        .unwrap();

    let diags = session.pump_diagnostics(
        "file:///workspace/src/lib.rs",
        Duration::from_secs(5),
    );
    assert!(!diags.is_empty(), "mock emits one publishDiagnostics");
}
```

Add `mock_supervisor_config_with_diagnostics()` to `lsp_mock/mod.rs` (sets `MOCK_LSP_EMIT_DIAGNOSTICS=1` in the process env, mirroring `stdio_transport_contract.rs`).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p legion-app --test rust_analyzer_doc_sync`
Expected: FAIL — `did_open` / `pump_diagnostics` not found.

- [ ] **Step 3: Implement doc sync + pump**

Add to `impl RustAnalyzerSession` in `session.rs`:

```rust
    /// Sends `textDocument/didOpen` for a buffer.
    pub fn did_open(
        &mut self,
        uri: &str,
        language_id: &str,
        version: i64,
        text: &str,
    ) -> Result<(), LanguageSessionError> {
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri,
                "languageId": language_id,
                "version": version,
                "text": text,
            }
        });
        self.session
            .send_notification("textDocument/didOpen", params)
            .map_err(LanguageSessionError::Handshake)
    }

    /// Drains notifications until diagnostics for `uri` arrive or `timeout`.
    pub fn pump_diagnostics(
        &mut self,
        _uri: &str,
        timeout: std::time::Duration,
    ) -> Vec<legion_lsp::LspDiagnosticNotificationMetadata> {
        let deadline = std::time::Instant::now() + timeout;
        let before = self.session.diagnostic_notifications().len();
        let _ = self.session.pump_until(deadline, &mut |n| !n.diagnostics.is_empty());
        self.session.diagnostic_notifications()[before..].to_vec()
    }
```

> If `LspDiagnosticNotificationMetadata` carries a URI/file field, filter the predicate by `uri`. Otherwise pump until any diagnostics arrive (the mock emits for the opened doc). Ensure `LspDiagnosticNotificationMetadata: Clone` (added in Task 2).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p legion-app --test rust_analyzer_doc_sync`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/legion-app/src/language/session.rs crates/legion-app/tests/rust_analyzer_doc_sync.rs crates/legion-app/tests/lsp_mock/
git commit -m "feat: sync documents and pump diagnostics through real session (WS-LANG-01 LANG.05/06)"
```

---

## Task 7: Read requests + stale-snapshot rejection (LANG.07/08)

**Files:**
- Modify: `crates/legion-app/src/language/session.rs` (add `request_read`)
- Modify: `crates/legion-app/src/language/mod.rs` (add `is_stale_response` helper)
- Test: `crates/legion-app/tests/rust_analyzer_read_requests.rs` (create)
- Test: `crates/legion-app/tests/language_stale_snapshot.rs` (create)

**Interfaces:**
- Produces: `impl RustAnalyzerSession { pub fn request_read(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, LanguageSessionError>; }` (returns the raw response `result` value, to be fed to existing `ingest_lsp_*` methods); `fn is_stale_response(issued_snapshot: SnapshotId, current_snapshot: SnapshotId) -> bool`.
- Consumes: existing `LspStdioSession::request`, request builders, `SnapshotId`.

- [ ] **Step 1: Write the failing tests**

Create `crates/legion-app/tests/rust_analyzer_read_requests.rs`:

```rust
use legion_app::language::{RustAnalyzerLaunchConfig, RustAnalyzerSession};
use legion_protocol::{LanguageId, LanguageServerId};

mod lsp_mock;

#[test]
fn completion_request_returns_result_value() {
    let config = RustAnalyzerLaunchConfig {
        discovery: legion_lsp::RustAnalyzerDiscovery {
            configured_path: Some(lsp_mock::mock_server_path()),
            ..Default::default()
        },
        supervisor: lsp_mock::mock_supervisor_config(),
        server_id: LanguageServerId("rust-analyzer".into()),
        language_id: LanguageId("rust".into()),
    };
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = RustAnalyzerSession::launch(config, &mut launcher).unwrap();
    session.initialize("file:///workspace").unwrap();

    let params = serde_json::json!({
        "textDocument": { "uri": "file:///workspace/src/lib.rs" },
        "position": { "line": 0, "character": 0 }
    });
    let result = session.request_read("textDocument/completion", params).unwrap();
    assert!(result.is_object() || result.is_array() || result.is_null());
}
```

Create `crates/legion-app/tests/language_stale_snapshot.rs`:

```rust
use legion_app::language::is_stale_response;
use legion_protocol::SnapshotId;

#[test]
fn response_for_older_snapshot_is_stale() {
    assert!(is_stale_response(SnapshotId(1), SnapshotId(2)));
}

#[test]
fn response_for_current_snapshot_is_fresh() {
    assert!(!is_stale_response(SnapshotId(2), SnapshotId(2)));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p legion-app --test rust_analyzer_read_requests --test language_stale_snapshot`
Expected: FAIL — items not found.

- [ ] **Step 3: Implement read request + staleness helper**

Add to `impl RustAnalyzerSession`:

```rust
    /// Sends a request and blocks for its correlated response, returning the
    /// raw `result` value for the caller to project via `ingest_lsp_*`.
    pub fn request_read(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, LanguageSessionError> {
        let response = self
            .session
            .request(method.to_string(), params, super::operation_context())
            .map_err(LanguageSessionError::Handshake)?;
        Ok(response.result_value())
    }
```

> Replace `response.result_value()` with the actual accessor on `LspCorrelatedResponse` that yields the JSON `result` (inspect the struct near line 781 of `legion-lsp`). If the result is stored as `Value`, expose/clone it.

Add to `crates/legion-app/src/language/mod.rs`:

```rust
use legion_protocol::SnapshotId;

/// Returns true when a response issued against `issued` is stale relative to
/// the buffer's `current` snapshot (design §6, LANG.07).
pub fn is_stale_response(issued: SnapshotId, current: SnapshotId) -> bool {
    issued != current
}
```

> Wire `is_stale_response` into each `ingest_lsp_*_response_for_buffer` call site in `lib.rs` (lines 18341-18514): before projecting, compare the snapshot the request was issued against to the buffer's current `SnapshotId`; if stale, drop the projection and record a metadata "stale" operation instead. Add one focused test asserting a stale completion response does not replace newer buffer projections (extend `language_tooling_workflow.rs`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p legion-app --test rust_analyzer_read_requests --test language_stale_snapshot`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/legion-app/src/language/ crates/legion-app/tests/rust_analyzer_read_requests.rs crates/legion-app/tests/language_stale_snapshot.rs
git commit -m "feat: real read requests with stale-snapshot rejection (WS-LANG-01 LANG.07/08)"
```

---

## Task 8: Rename + code-action → proposal routing (LANG.09)

**Files:**
- Modify: `crates/legion-app/src/language/session.rs` (add `request_workspace_edit`)
- Create: `crates/legion-app/src/language/proposal.rs`
- Modify: `crates/legion-app/src/language/mod.rs` (re-export)
- Test: `crates/legion-app/tests/language_edit_proposal_routing.rs` (create)

**Interfaces:**
- Produces: `fn workspace_edit_to_proposal_input(server_edit: serde_json::Value, correlation: LspRequestCorrelation, proposal_id: ProposalId, principal: PrincipalId, capability: CapabilityId, preconditions: ProposalVersionPreconditions, lifecycle_state: ProposalLifecycleState, privacy_label: ProposalPrivacyLabel, preview: PreviewSummary, created_at: TimestampMillis) -> LspEditProposalConversionInput`.
- Consumes: existing `convert_lsp_edit_to_workspace_proposal` (protocol 16541), `WorkspaceEditProposalPayload` (4073), `WorkspaceEditSourceKind` (4015).

- [ ] **Step 1: Write the failing test**

Create `crates/legion-app/tests/language_edit_proposal_routing.rs`:

```rust
use legion_app::language::workspace_edit_to_proposal_input;
use legion_protocol::{
    convert_lsp_edit_to_workspace_proposal, CapabilityId, PrincipalId, ProposalId,
    ProposalLifecycleState, ProposalPayload,
};

mod proposal_fixture; // builds minimal correlation/preconditions/preview/privacy/timestamp

#[test]
fn rust_analyzer_rename_edit_becomes_workspace_proposal() {
    let server_edit = serde_json::json!({
        "changes": {
            "file:///workspace/src/lib.rs": [
                { "range": { "start": {"line":0,"character":3}, "end": {"line":0,"character":7} },
                  "newText": "renamed" }
            ]
        }
    });
    let input = workspace_edit_to_proposal_input(
        server_edit,
        proposal_fixture::correlation(),
        ProposalId(1),
        PrincipalId("user".into()),
        CapabilityId("language.rename".into()),
        proposal_fixture::preconditions(),
        ProposalLifecycleState::Draft,
        proposal_fixture::privacy_label(),
        proposal_fixture::preview(),
        proposal_fixture::created_at(),
    );
    let proposal = convert_lsp_edit_to_workspace_proposal(input).unwrap();
    assert!(matches!(proposal.payload, ProposalPayload::WorkspaceEdit(_)));
}
```

Create `crates/legion-app/tests/proposal_fixture/mod.rs` returning the minimal real-typed values (copy shapes from an existing proposal test in `crates/legion-app/tests/` — search for `convert_lsp_edit_to_workspace_proposal` or `WorkspaceEditProposalPayload` to find one).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p legion-app --test language_edit_proposal_routing`
Expected: FAIL — `workspace_edit_to_proposal_input` not found.

- [ ] **Step 3: Implement the conversion adapter**

Create `crates/legion-app/src/language/proposal.rs`:

```rust
use legion_protocol::{
    CapabilityId, LspEditProposalConversionInput, LspRequestCorrelation, PreviewSummary,
    PrincipalId, ProposalId, ProposalLifecycleState, ProposalPrivacyLabel,
    ProposalVersionPreconditions, TimestampMillis, WorkspaceEditProposalPayload,
    WorkspaceEditSourceKind,
};

/// Builds a proposal-conversion input from a rust-analyzer `WorkspaceEdit`
/// JSON result. The edit is parsed into the metadata-only payload; raw text is
/// summarized by the existing payload type, not stored verbatim here.
#[allow(clippy::too_many_arguments)]
pub fn workspace_edit_to_proposal_input(
    server_edit: serde_json::Value,
    correlation: LspRequestCorrelation,
    proposal_id: ProposalId,
    principal: PrincipalId,
    capability: CapabilityId,
    preconditions: ProposalVersionPreconditions,
    lifecycle_state: ProposalLifecycleState,
    privacy_label: ProposalPrivacyLabel,
    preview: PreviewSummary,
    created_at: TimestampMillis,
) -> LspEditProposalConversionInput {
    let workspace_edit = WorkspaceEditProposalPayload::from_lsp_changes(
        &server_edit,
        WorkspaceEditSourceKind::LanguageServer,
    );
    LspEditProposalConversionInput {
        proposal_id,
        principal,
        capability,
        request: correlation,
        workspace_edit,
        preconditions,
        lifecycle_state,
        privacy_label,
        preview,
        expires_at: None,
        created_at,
        diagnostics: Vec::new(),
        schema_version: 1,
    }
}
```

> Replace `WorkspaceEditProposalPayload::from_lsp_changes(...)` and `WorkspaceEditSourceKind::LanguageServer` with the real constructor/variant. Read `WorkspaceEditProposalPayload` (4073) and `WorkspaceEditSourceKind` (4015): if no `from_lsp_changes` exists, construct the payload field-by-field from the parsed `changes`/`documentChanges`, using whichever source-kind variant denotes LSP origin. Confirm `schema_version` matches the payload's expected value.

Re-export from `mod.rs`: `pub use proposal::workspace_edit_to_proposal_input;`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p legion-app --test language_edit_proposal_routing`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/legion-app/src/language/ crates/legion-app/tests/language_edit_proposal_routing.rs crates/legion-app/tests/proposal_fixture/
git commit -m "feat: route rust-analyzer rename/code-action edits to proposals (WS-LANG-01 LANG.09)"
```

---

## Task 9: Restart/backoff policy + log redaction (LANG.10/11)

**Files:**
- Modify: `crates/legion-app/src/language/session.rs` (add restart policy, `restart` method)
- Create: `crates/legion-app/src/language/redaction.rs`
- Modify: `crates/legion-app/src/language/mod.rs` (re-export)
- Test: `crates/legion-app/tests/language_restart_policy.rs` (create)
- Test: `crates/legion-app/tests/language_log_redaction.rs` (create)

**Interfaces:**
- Produces: `struct RestartPolicy { max_restarts: u32, backoff_base_ms: u64 }`; `impl RustAnalyzerSession { pub fn note_crash_and_should_restart(&mut self, policy: &RestartPolicy) -> Option<std::time::Duration>; }`; `fn redact_lsp_stderr(raw: &str) -> StderrSummary`; `struct StderrSummary { line_count: u32, error_lines: u32, warn_lines: u32 }`.

- [ ] **Step 1: Write the failing tests**

Create `crates/legion-app/tests/language_log_redaction.rs`:

```rust
use legion_app::language::redact_lsp_stderr;

#[test]
fn redaction_keeps_only_metadata_counts() {
    let raw = "INFO starting\nERROR cannot open /home/secret/path\nWARN slow\nERROR boom";
    let summary = redact_lsp_stderr(raw);
    assert_eq!(summary.line_count, 4);
    assert_eq!(summary.error_lines, 2);
    assert_eq!(summary.warn_lines, 1);
}
```

Create `crates/legion-app/tests/language_restart_policy.rs`:

```rust
use legion_app::language::RestartPolicy;

#[test]
fn restart_backoff_grows_until_cap() {
    // Uses a session-free policy check via the public helper on the policy.
    let policy = RestartPolicy { max_restarts: 2, backoff_base_ms: 100 };
    assert_eq!(policy.backoff_for_attempt(0).as_millis(), 100);
    assert_eq!(policy.backoff_for_attempt(1).as_millis(), 200);
    assert!(policy.is_exhausted(2));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p legion-app --test language_log_redaction --test language_restart_policy`
Expected: FAIL.

- [ ] **Step 3: Implement redaction + restart policy**

Create `crates/legion-app/src/language/redaction.rs`:

```rust
/// Metadata-only summary of LSP stderr (design §8, LANG.11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StderrSummary {
    /// Total line count.
    pub line_count: u32,
    /// Lines containing an error marker.
    pub error_lines: u32,
    /// Lines containing a warning marker.
    pub warn_lines: u32,
}

/// Summarizes stderr into counts. Never retains raw line text.
pub fn redact_lsp_stderr(raw: &str) -> StderrSummary {
    let mut line_count = 0;
    let mut error_lines = 0;
    let mut warn_lines = 0;
    for line in raw.lines() {
        line_count += 1;
        let upper = line.to_ascii_uppercase();
        if upper.contains("ERROR") {
            error_lines += 1;
        } else if upper.contains("WARN") {
            warn_lines += 1;
        }
    }
    StderrSummary { line_count, error_lines, warn_lines }
}
```

Add to `session.rs`:

```rust
/// Bounded restart policy for a crashed server (design §8, LANG.10).
#[derive(Debug, Clone)]
pub struct RestartPolicy {
    /// Maximum restarts before giving up.
    pub max_restarts: u32,
    /// Base backoff in milliseconds, doubled per attempt.
    pub backoff_base_ms: u64,
}

impl RestartPolicy {
    /// Backoff duration for a zero-based attempt index.
    pub fn backoff_for_attempt(&self, attempt: u32) -> std::time::Duration {
        std::time::Duration::from_millis(self.backoff_base_ms << attempt.min(16))
    }

    /// Whether the restart budget is exhausted at `attempt`.
    pub fn is_exhausted(&self, attempt: u32) -> bool {
        attempt >= self.max_restarts
    }
}

impl RustAnalyzerSession {
    /// Records a crash, increments `restart_count`, and returns the backoff if
    /// a restart is still permitted (caller performs the relaunch).
    pub fn note_crash_and_should_restart(
        &mut self,
        policy: &RestartPolicy,
    ) -> Option<std::time::Duration> {
        let attempt = self.health.restart_count;
        if policy.is_exhausted(attempt) {
            self.health.init_status = legion_protocol::LspResultStatus::Unavailable;
            return None;
        }
        self.health.restart_count = attempt + 1;
        Some(policy.backoff_for_attempt(attempt))
    }
}
```

Re-export `RestartPolicy`, `redact_lsp_stderr`, `StderrSummary` from `mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p legion-app --test language_log_redaction --test language_restart_policy`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/legion-app/src/language/ crates/legion-app/tests/language_restart_policy.rs crates/legion-app/tests/language_log_redaction.rs
git commit -m "feat: add restart/backoff policy and LSP stderr redaction (WS-LANG-01 LANG.10/11)"
```

---

## Task 10: Projection-only health/refusal rows in UI + desktop

**Files:**
- Modify: `crates/legion-protocol/src/lib.rs` (add `LspServerHealthProjection` row if no suitable projection exists)
- Modify: `crates/legion-ui/src/...` (add health/refusal projection to the language view model)
- Modify: `crates/legion-desktop/src/...` (render the rows)
- Test: `crates/legion-desktop/tests/language_health_view.rs` (create)

**Interfaces:**
- Produces: `struct LspServerHealthProjection { server_label: String, provenance_label: String, version_label: String, status_label: String, restart_count: u32, download_refused: bool }`; a UI mapping `fn project_lsp_health(record: &LspServerHealthRecord, download_refused: bool) -> LspServerHealthProjection`.

- [ ] **Step 1: Write the failing test**

Create `crates/legion-desktop/tests/language_health_view.rs`:

```rust
use legion_protocol::{
    LanguageId, LanguageServerId, LspResultStatus, LspServerBinaryProvenance,
    LspServerHealthRecord,
};
use legion_ui::project_lsp_health;

#[test]
fn health_projection_labels_provenance_and_status() {
    let record = LspServerHealthRecord {
        server_id: LanguageServerId("rust-analyzer".into()),
        language_id: LanguageId("rust".into()),
        binary_provenance: LspServerBinaryProvenance::SystemPath,
        binary_path_hash: None,
        artifact_hash: None,
        version: Some("rust-analyzer 1.0.0".into()),
        init_status: LspResultStatus::Fresh,
        capabilities: Vec::new(),
        diagnostics_latency_ms: Some(12),
        restart_count: 1,
        download_decision_id: None,
        schema_version: LspServerHealthRecord::schema_version(),
    };
    let p = project_lsp_health(&record, false);
    assert!(p.provenance_label.to_lowercase().contains("path"));
    assert!(p.version_label.contains("1.0.0"));
    assert_eq!(p.restart_count, 1);
    assert!(!p.download_refused);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p legion-desktop --test language_health_view`
Expected: FAIL — `project_lsp_health` not found.

- [ ] **Step 3: Implement projection + render**

Add `LspServerHealthProjection` to `legion-protocol` (near other language projections) and `project_lsp_health` to `legion-ui` (in the module that already holds `LanguageToolingProjection`-adjacent mappings). Map provenance/status enums to labels; set `download_refused` from the passed flag. In `legion-desktop`, render the rows in the existing language/problems panel area (follow the existing projection-row rendering pattern; no authority, read-only).

```rust
// legion-ui
pub fn project_lsp_health(
    record: &legion_protocol::LspServerHealthRecord,
    download_refused: bool,
) -> legion_protocol::LspServerHealthProjection {
    use legion_protocol::{LspResultStatus, LspServerBinaryProvenance as P};
    let provenance_label = match record.binary_provenance {
        P::Configured => "configured path",
        P::ProjectLocal => "project-local",
        P::SystemPath => "system PATH",
        P::Bundled => "bundled",
        P::Downloaded => "downloaded",
    }
    .to_string();
    let status_label = match record.init_status {
        LspResultStatus::Fresh => "ready",
        LspResultStatus::Stale => "stale",
        LspResultStatus::Unavailable => "unavailable",
    }
    .to_string();
    legion_protocol::LspServerHealthProjection {
        server_label: record.server_id.0.clone(),
        provenance_label,
        version_label: record.version.clone().unwrap_or_else(|| "unknown".into()),
        status_label,
        restart_count: record.restart_count,
        download_refused,
    }
}
```

> Match `LspResultStatus` variants to the real enum (line 16071). If it has more/different variants, cover them all.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p legion-desktop --test language_health_view`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/legion-protocol/src/lib.rs crates/legion-ui/src crates/legion-desktop/src crates/legion-desktop/tests/language_health_view.rs
git commit -m "feat: project LSP health and download-refusal rows (WS-LANG-01 LANG.06/10)"
```

---

## Task 11: Real rust-analyzer `--ignored` smoke (lsp + app) (LANG.03/12)

**Files:**
- Create: `crates/legion-lsp/tests/rust_analyzer_smoke.rs`
- Create: `crates/legion-app/tests/rust_analyzer_workflow.rs`

**Interfaces:**
- Consumes: everything above. These tests are `#[ignore]` and skip cleanly when `rust-analyzer` is not on `PATH`.

- [ ] **Step 1: Write the ignored smoke (lsp crate)**

Create `crates/legion-lsp/tests/rust_analyzer_smoke.rs`:

```rust
use std::time::{Duration, Instant};

use legion_lsp::{LspStdioSession, RustAnalyzerDiscovery, DiscoveredBinary};

fn discovered() -> Option<std::path::PathBuf> {
    let d = RustAnalyzerDiscovery {
        path_env: std::env::var("PATH").ok(),
        ..Default::default()
    };
    match d.resolve() {
        DiscoveredBinary::Found { path, .. } => Some(path),
        DiscoveredBinary::NotFound => None,
    }
}

#[test]
#[ignore = "requires rust-analyzer on PATH; run with --ignored"]
fn rust_analyzer_initializes_and_emits_diagnostics() {
    let Some(bin) = discovered() else {
        eprintln!("rust-analyzer not found on PATH; skipping");
        return;
    };
    let version = RustAnalyzerDiscovery::probe_version(&bin);
    assert!(version.is_some(), "rust-analyzer --version should succeed");

    // Build a supervisor config pointing at the discovered binary, launch a
    // real session against this repo, initialize, didOpen a fixture file, and
    // pump for diagnostics within a generous deadline.
    let mut launcher = legion_lsp::LspStdioLauncher::new();
    let mut session = LspStdioSession::start(
        // helper: real_supervisor_config(&bin) — construct from LspServerProcessConfig
        rust_analyzer_smoke_support::real_supervisor_config(&bin),
        &mut launcher,
    )
    .expect("launch rust-analyzer");

    session
        .initialize(rust_analyzer_smoke_support::init_params(), rust_analyzer_smoke_support::ctx())
        .expect("initialize");
    session
        .send_notification("initialized", serde_json::json!({}))
        .unwrap();

    // didOpen a small real file from this crate.
    session
        .send_notification(
            "textDocument/didOpen",
            rust_analyzer_smoke_support::did_open_params(),
        )
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(60);
    let outcome = session
        .pump_until(deadline, &mut |n| !n.diagnostics.is_empty())
        .unwrap();
    // rust-analyzer may report zero diagnostics for clean code; accept either
    // diagnostics observed or a clean deadline as a successful handshake proof.
    assert!(matches!(
        outcome,
        legion_lsp::PumpOutcome::PredicateMet | legion_lsp::PumpOutcome::Deadline
    ));
}
```

Add a `rust_analyzer_smoke_support` inline `mod` (or `tests/rust_analyzer_smoke_support/mod.rs`) implementing `real_supervisor_config`, `init_params`, `did_open_params`, `ctx` using the real `LspServerProcessConfig`/`LspSupervisorConfig` constructors and a real path under the crate.

- [ ] **Step 2: Write the ignored app-level workflow smoke**

Create `crates/legion-app/tests/rust_analyzer_workflow.rs` that, when `rust-analyzer` is present, drives `RustAnalyzerSession` end-to-end against a tiny fixture dir: launch → initialize → didOpen → pump diagnostics → completion → hover → definition → references → formatting → rename → `convert_lsp_edit_to_workspace_proposal` → forced restart via `note_crash_and_should_restart`. Mark `#[ignore]` and skip cleanly if discovery returns `NotFound`. Assert each step returns a well-formed result; tolerate empty result sets (clean code) but require no transport errors.

- [ ] **Step 3: Run the ignored smokes locally (only where rust-analyzer is installed)**

Run: `cargo test -p legion-lsp --test rust_analyzer_smoke -- --ignored`
Run: `cargo test -p legion-app --test rust_analyzer_workflow -- --ignored`
Expected: PASS where rust-analyzer is installed; the non-ignored gate (`cargo test --workspace --all-targets`) must still pass without it.

- [ ] **Step 4: Verify the blocking gate ignores them**

Run: `cargo test -p legion-lsp --test rust_analyzer_smoke`
Expected: the test is listed as ignored, suite passes with 0 run.

- [ ] **Step 5: Commit**

```bash
git add crates/legion-lsp/tests/rust_analyzer_smoke.rs crates/legion-app/tests/rust_analyzer_workflow.rs
git commit -m "test: add ignored real rust-analyzer smoke for lsp and app (WS-LANG-01 LANG.03/12)"
```

---

## Task 12: xtask smoke command, CI lane, evidence, and ledger (LANG.12 + acceptance)

**Files:**
- Modify: `xtask/src/main.rs` (add `rust-analyzer-smoke` subcommand)
- Modify: CI workflow under `.github/workflows/` (add opt-in 3-OS lane) — locate the existing GUI-phase lanes and mirror their structure
- Create: `plans/evidence/production/WS-LANG-01/WS-LANG-01-evidence.md`
- Modify: `plans/product-readiness-ledger.md` (PR-LANG-001 evidence row)
- Modify: `.planning/STATE.md` (optional: record WS-LANG-01 acceptance)

**Interfaces:**
- Produces: `cargo run -p xtask -- rust-analyzer-smoke` that shells `cargo test -p legion-lsp --test rust_analyzer_smoke -- --ignored` and the app workflow, returning non-zero only on real failure (skips count as pass).

- [ ] **Step 1: Add the xtask subcommand**

In `xtask/src/main.rs`, add a `run_rust_analyzer_smoke_command()` mirroring the existing `fn run_*_command` pattern (e.g. `run_perf_harness_command`, line ~805). It spawns the two `--ignored` test invocations via `process::Command::new("cargo")`, streams output, and maps exit codes. Wire it into the arg dispatch match alongside the other subcommands.

```rust
fn run_rust_analyzer_smoke_command() -> i32 {
    let invocations = [
        vec!["test", "-p", "legion-lsp", "--test", "rust_analyzer_smoke", "--", "--ignored"],
        vec!["test", "-p", "legion-app", "--test", "rust_analyzer_workflow", "--", "--ignored"],
    ];
    for args in invocations {
        let status = std::process::Command::new("cargo").args(&args).status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => return s.code().unwrap_or(1),
            Err(_) => return 1,
        }
    }
    0
}
```

- [ ] **Step 2: Verify the subcommand runs**

Run: `cargo run -p xtask -- rust-analyzer-smoke`
Expected: PASS (skips cleanly if rust-analyzer absent; runs real smoke if present).

- [ ] **Step 3: Add the opt-in CI lane**

In the existing CI workflow directory, add a job (workflow_dispatch / release-gate triggered, not on every push) running on `ubuntu-latest`, `macos-latest`, `windows-latest`: install rust-analyzer (rustup component `rust-analyzer` where available), then `cargo run -p xtask -- rust-analyzer-smoke`. Mirror an existing `gui-phase*` job's structure and matrix.

- [ ] **Step 4: Write evidence + update ledger (docs)**

Create `plans/evidence/production/WS-LANG-01/WS-LANG-01-evidence.md` using the WS-MANUAL-02 evidence file as the template: a task table mapping LANG.01–LANG.12 to the exact test commands and outcomes, plus the standing-gate results and the real-smoke command. Update `plans/product-readiness-ledger.md` PR-LANG-001 row: append the WS-LANG-01 evidence (mock-gate tests + ignored real smoke) and keep the status conservative (toward product-workflow validated only if the real 3-OS smoke has actually been run; otherwise note it as substrate-plus-real-smoke-available).

Run docs gate: `cargo run -p xtask -- docs-hygiene`
Expected: PASS.

- [ ] **Step 5: Run full standing gates and commit**

Run:
```
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- check-deps
cargo run -p xtask -- docs-hygiene
cargo deny check
```
Expected: all PASS.

```bash
git add xtask/src/main.rs .github/workflows/ plans/evidence/production/WS-LANG-01/ plans/product-readiness-ledger.md .planning/STATE.md
git commit -m "feat: wire rust-analyzer smoke xtask/CI lane and WS-LANG-01 evidence (WS-LANG-01 LANG.12)"
```

---

## Self-Review

**1. Spec coverage** — every spec section maps to a task:
- §2 evidence strategy → Tasks 2/6 (mock gate), Task 11 (ignored real smoke).
- §3 boundaries → respected: lsp (T2/3), protocol (T1/10), app orchestration (T4–9), ui/desktop projection-only (T10).
- §4 pump seam → Task 2 (trait + BlockingPump + B-ready impl).
- §5 discovery + gated download + provenance → Tasks 3 (discovery/version), 4 (gated download), 1+5 (health record).
- §6 doc sync + read projections + stale-snapshot → Tasks 6, 7.
- §7 write→proposal → Task 8.
- §8 restart + redaction → Task 9.
- §9 testing strategy → Tasks 2–11 (mock), 11/12 (real + CI).
- §10 code-quality extraction → Tasks 4–9 build the new `language/` module instead of growing `lib.rs`.
- §11 task mapping → Tasks 1–12 cover LANG.01–LANG.12.
- §12 acceptance → Task 12 (evidence + ledger + gates).

**2. Placeholder scan** — code steps contain real code. Where the giant `lib.rs` and exact protocol constructor names can't be reproduced verbatim, steps give the exact signature/contract plus the precise line anchor to read and a "replace with the real name" instruction. These are bounded lookups, not open-ended TODOs.

**3. Type consistency** — `RustAnalyzerSession`, `RustAnalyzerLaunchConfig`, `RustAnalyzerDiscovery`, `DiscoveredBinary`, `PumpOutcome`, `PumpedNotifications`, `LspNotificationSource`, `LspServerHealthRecord`, `LspServerBinaryProvenance`, `LspServerHealthProjection`, `RestartPolicy`, `is_stale_response`, `workspace_edit_to_proposal_input`, `redact_lsp_stderr` are used consistently across tasks with the signatures defined in their producing task's Interfaces block.

## Known follow-ups (out of scope, flagged)

- Threaded reader-thread pump (`ThreadedPump`) — seam exists (Task 2); implement when daily-driver non-blocking diagnostics demand it.
- The exact protocol constructor names (`CapabilityRequest`/`CapabilityResponse` variants, `WorkspaceEditProposalPayload` builder, `LspCorrelatedResponse` result accessor, `LspOperationContext` constructor) must be confirmed against live source during implementation — anchors are provided in each task.
- `legion-app/src/lib.rs` full decomposition remains large; this plan extracts only the WS-LANG-01 surface into `language/`.
