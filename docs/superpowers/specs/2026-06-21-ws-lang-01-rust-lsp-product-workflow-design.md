# WS-LANG-01 Rust LSP Product Workflow — Design

- Status: Approved for implementation planning
- Date: 2026-06-21
- Workstream: WS-LANG-01 in `plans/legion-production-master-plan-v0.2.md` (lines 338-368)
- Milestone: M8 — Manual Daily Driver Beta
- Product gate: `PR-LANG-001 Rust language workflow`
- Predecessors complete: WS-P0 (rebaseline), WS-MANUAL-01 (editor feel), WS-MANUAL-02 (large files/scale)

## 1. Problem and Goal

The LSP substrate in `legion-lsp` is rich — JSON-RPC framing, an adapter
registry, `LspSupervisor`, request builders for every LSP method, diagnostic
projection, and a blocking `LspStdioSession` with `initialize`. But **every
response in the product path is deterministic**: the app drives the mock LSP
server (`legion-lsp/src/bin/mock_lsp_server.rs`) and `project_*_response`
builders. **No real `rust-analyzer` process is ever launched anywhere in the
repository, and no `LspServerHealthRecord` type exists.** This is the exact
"substrate validated, product workflow not validated" gap `PR-LANG-001` names.

Goal: make Rust language intelligence real, fast, and user-visible. After this
workstream a user can open Legion itself, see real rust-analyzer
diagnostics/completion/hover/definition/references, perform a rename as a
reviewable proposal, apply a write-producing code action as a proposal, format,
and recover from a server restart — with provenance, health, and failure states
visible and metadata-only.

## 2. Shaping Decisions (locked)

1. **Evidence strategy — Hybrid.** The always-green CI gate runs against the
   deterministic mock server. A separate real-rust-analyzer smoke is marked
   `#[ignore]` (opt-in / release-gate), matching the WS-MANUAL-02 100MB pattern.
   The blocking gate never depends on rust-analyzer being present.
2. **Scope — Full workstream, all real-process.** All 12 LANG tasks
   (LANG.01–LANG.12) are closed, and every LSP method is exercised against live
   rust-analyzer in the `--ignored` smoke, not only the acceptance-named core.
3. **Session model — Bounded blocking pump with B-ready seams.** A
   single-threaded read-with-timeout pump handles rust-analyzer's asynchronous
   notifications, placed behind a trait so a future reader-thread implementation
   drops in without churning callers.
4. **rust-analyzer acquisition — Discovery + gated download.** Local discovery
   first; remote download only through the existing default-deny capability
   broker with pinned-hash/version provenance. No automatic egress.

## 3. Authority Boundaries

No boundary changes; this fills in the existing spine.

| Crate | Responsibility added |
| --- | --- |
| `legion-lsp` | Notification-pump trait + `BlockingPump`, async diagnostics draining, rust-analyzer discovery/provenance helpers, `rust-analyzer --version` probe. Pure process/protocol authority; no product state. |
| `legion-protocol` | `LspServerHealthRecord` DTO + binary-source/provenance enums; reuse existing `LspCapabilitySummary`. Contracts only. |
| `legion-app` | Orchestration: discovery → (gated download) → launch → handshake → doc-sync → projection → restart policy. Owns real product state and the capability-broker request for download. |
| `legion-ui` / `legion-desktop` | Projection-only health/diagnostics/restart/refusal status rows. No new authority. |
| `xtask` | Wire the `--ignored` real smoke into an opt-in / release-gate lane (LANG.12). |
| evidence | `plans/evidence/production/WS-LANG-01/`. |

## 4. Notification-Pump Seam (architectural core)

Real rust-analyzer pushes `textDocument/publishDiagnostics` and
`$/progress` **asynchronously**, after indexing, not in reply to any request.
The current `read_until_correlated_response` only collects notifications while
blocking on some other request, so it cannot observe real diagnostics that
arrive between requests.

Design:

- Introduce a trait, e.g. `LspNotificationSource`, exposing a bounded
  `pump_until(deadline, predicate) -> PumpOutcome`. The predicate lets callers
  drain "until diagnostics for URI X arrive, or the deadline elapses."
- Ship one implementation now: `BlockingPump`, using a read-timeout on child
  stdout. It routes id-bearing frames to the correlation map and
  notification-shaped frames to typed buffers (diagnostics, progress).
- Refactor `read_until_correlated_response` to delegate to the pump so there is
  **one** read path, not two.
- **B-ready seam:** a future `ThreadedPump` (reader thread + channel) implements
  the same trait and replaces `BlockingPump` without changing `LspStdioSession`
  callers. Out of scope for this packet.

Platform note: the read-timeout must be correct on Windows and Unix. Prefer a
bounded poll loop over per-OS non-blocking handles unless measurement shows it is
needed; the smoke test controls timing, so the deadline is the safety net.

## 5. Discovery, Gated Download, and Provenance (LANG.01–LANG.02)

**Discovery order:** configured path → project-local → system `PATH` →
bundled-if-present.

**Gated download** (only when no binary is discovered, never automatic):

1. App composes a `CapabilityRequest` with `command_class = Network`,
   `network_target = { https, <pinned release host>, 443 }`,
   `command_binary = "rust-analyzer"`, `lsp_server_binary = "rust-analyzer"`.
2. The `CapabilityBrokerPort` evaluates against `NetworkPolicy`. The default
   policy (`air_gap: true`, `local_provider_only: true`, allowlist
   `["localhost"]`) **denies** the request. Download therefore requires the user
   to explicitly permit the release host (allowlist / disable air-gap) *and*
   grant consent. **Manual/air-gap zero-egress is preserved by construction** —
   no egress path bypasses policy.
3. On grant: download → verify against a **pinned SHA-256 + expected version
   string** → record provenance. A hash or version mismatch **fails closed** and
   discards the artifact.
4. On deny: the refusal is visible in the projection; discovery falls back to
   the local sources. Manual mode never silently fetches.

**`LspServerHealthRecord`** (metadata-only) records:

- `binary_source ∈ {Configured, ProjectLocal, SystemPath, Bundled, Downloaded}`
- resolved path hash / artifact hash
- version string (from `rust-analyzer --version`)
- init status and `LspCapabilitySummary` reference
- diagnostics latency
- restart count
- `CapabilityDecisionId` when `Downloaded`

No raw source content is ever stored in the record.

## 6. Document Sync and Read Projections (LANG.05–LANG.08)

Flow: editor snapshot → app builds `didOpen` / `didChange` from the buffer's
UTF-16 ranges (builders already exist) → session sends → pump drains
`publishDiagnostics` → existing diagnostic projection feeds the problems panel.

Read methods — completion, hover, definition, references, document/workspace
symbols, semantic tokens, inlay hints, code lenses, folding ranges, formatting —
go request → pump → projection through the existing `project_*_response` shapes,
now fed by real responses in the smoke and mock responses in the gate. Semantic
tokens are projected for correctness (token ranges/types); rendering polish
beyond correct projection is out of scope (§13).

**Stale-snapshot rejection (LANG.07):** every request carries the snapshot id it
was issued against. If the buffer's snapshot advanced by the time the response
lands, the projection is dropped as stale. The `LspRequestCorrelation` /
snapshot plumbing is partly present and is hardened here so a late response can
never overwrite a newer buffer state.

## 7. Write-Producing Actions → Proposals (LANG.09)

Rename and edit-producing code actions return LSP `WorkspaceEdit`s. These route
through the **existing** conversion path
(`ProposalPayload::WorkspaceEdit` / `WorkspaceEditProposalPayload`, protocol
~line 16509) into the proposal lifecycle — diff-first review, never direct disk
mutation. No new mutation path is created. The edit source is labelled so audit
distinguishes LSP-originated edits from assist/agent edits.

## 8. Failure and Restart UX (LANG.10–LANG.11)

- **Restart / backoff / crash:** `LspSupervisor` already models lifecycle. The
  app adds a restart policy (bounded retries with backoff, `restart_count` on the
  health record) and a visible "server restarting / unavailable" projection.
  In-flight requests are abandoned on restart and never mis-correlated to the new
  process.
- **LSP log redaction (LANG.11):** stderr is summarized to metadata (line counts,
  severity tallies), never raw-source-bearing payloads, consistent with the
  metadata-first rule.

## 9. Testing Strategy

**Always-green gate (mock server, deterministic):**

- initialize/initialized handshake and capability summary
- doc-sync didOpen/didChange and diagnostics projection
- every read projection (completion, hover, definition, references, symbols,
  semantic tokens, inlay, code lens, folding, format)
- stale-snapshot rejection
- restart policy / backoff / abandoned in-flight requests
- log redaction (no raw source in summaries)
- proposal conversion for rename / code action
- discovery order resolution
- capability-broker download decisions: air-gap-denies, explicit-grant-allows,
  hash-mismatch-fails-closed, refusal-is-visible

**Real `--ignored` smoke (live rust-analyzer):**

- `cargo test -p legion-lsp --test rust_analyzer_smoke -- --ignored` and an
  app-level equivalent, skipping cleanly when `rust-analyzer` is absent from
  `PATH`.
- Drives the full language slice against the Legion repo: diagnostics,
  completion, hover, definition, references, rename-proposal, format,
  code-action-proposal, then forced restart recovery.
- A separate `--ignored` download smoke exercises the live gated fetch when a
  release host is explicitly permitted.

**3-OS (LANG.12):** the `--ignored` smoke is wired into a release-gate / opt-in
CI lane, not the blocking gate, so the absence of rust-analyzer never reds the
build.

## 10. Targeted Code-Quality Improvement (in-scope, minimal)

`legion-app/src/lib.rs` is ~25,546 lines, which makes every edit in this
workstream harder and riskier. As part of this work — improving the code being
edited, not a speculative refactor — the language-tooling code touched here is
extracted into a focused module (`language/` or `language_tooling.rs`). Bounded
strictly to what WS-LANG-01 touches; no unrelated reorganization.

## 11. Task → Design Mapping

| Task | Description | Design section |
| --- | --- | --- |
| LANG.01 | rust-analyzer discovery order | §5 |
| LANG.02 | server binary provenance / version in health record | §5 |
| LANG.03 | launch rust-analyzer for a real fixture | §4, §9 |
| LANG.04 | initialize/initialized handshake + workspace folders | §6, §9 |
| LANG.05 | doc open/change/save sync from editor snapshots | §6 |
| LANG.06 | publishDiagnostics → problems panel, redacted | §6, §8 |
| LANG.07 | completion with stale-snapshot rejection | §6 |
| LANG.08 | hover/def/refs/rename/format/code actions/semantic tokens/inlay/code lens/folding | §6 |
| LANG.09 | write-producing code actions → proposal lifecycle | §7 |
| LANG.10 | server restart/backoff/crash UX | §8 |
| LANG.11 | LSP log redaction | §8 |
| LANG.12 | 3-OS rust-analyzer smoke in CI / release gate | §9 |

## 12. Acceptance

- `PR-LANG-001` can move toward product-workflow validated: a user opens Legion,
  sees real rust-analyzer diagnostics/completion/hover/definition/references,
  performs a rename proposal, applies a code-action proposal, formats, and
  recovers from a server restart.
- Mock gate is green and blocking; real rust-analyzer smoke passes when the
  binary is available and skips cleanly otherwise.
- Manual / air-gap zero-egress remains green; no download path bypasses the
  default-deny capability broker.
- All standing gates in `plans/legion-production-master-plan-v0.2.md` §10 pass.
- Evidence captured under `plans/evidence/production/WS-LANG-01/` and the
  product-readiness ledger updated in the same change.

## 13. Out of Scope

- Threaded/async reader-thread pump implementation (B) — seam only.
- Non-Rust language servers (Rust first; TOML/JSON/Markdown are WS-LANG-02).
- Semantic-token rendering polish beyond projection correctness.
- Automatic / silent rust-analyzer download — always capability-gated.
- DAP/debug/test (WS-DEBUG-01) and structural search (WS-LANG-02).
