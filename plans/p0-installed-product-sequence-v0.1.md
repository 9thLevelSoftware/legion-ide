# P0 installed-product sequence v0.1

- Status: executable sequence (not a promotion)
- Date: 2026-08-31
- Scored head: `4ed8617142086c45f1b33b94503bf08ff314be27`
- Evidence: [`plans/evidence/production/WS-P0/2026-08-31-release-gap-full-pass.md`](evidence/production/WS-P0/2026-08-31-release-gap-full-pass.md)
- Does not supersede: [`plans/legion-production-master-plan-v0.2.md`](legion-production-master-plan-v0.2.md)
- Does not change: [`plans/product-readiness-ledger.md`](product-readiness-ledger.md) row statuses

This is the close-out order for the ten P0 blockers from the 2026-08-31 full pass. Task ids use the `GAP-*` prefix so they do not collide with kanban epic `P0` (truth/taxonomy import).

## 1. Why this order

The full pass found **no P0 at evidence level 4 or 5**. Code and crate tests exist; windowed GUI and signed 3-OS installers do not. The failure mode to prevent is more substrate work that gets described as product-ready.

Ordering rules:

1. **Honesty first.** Stop green gates from certifying stale claims, then lock `main`.
2. **Do not lose the user's work** before asking anyone to dogfood an install.
3. **Windowed proof before trust-chain theatre.** Unsigned native packages plus a hard-fail windowed E2E beat another journal FSM.
4. **Signing is a parallel procurement track.** `EXT-CERT-*` does not block Waves 0–2.
5. **P1–P3 wait.** Language packs, VSIX, remote, `legion-agentd`, and enterprise admin are not on this sequence.

## 2. Exit bars (honest)

| Bar | When it is true | What it is not |
| --- | --- | --- |
| Internal dogfood | Waves 0–2 accepted on current HEAD, with named windowed journals on at least one OS and crash-safe dirty restore | A signed installer, 3-OS accessibility, or Manual SKU |
| Invitation-only technical preview | Dogfood bar plus Wave 3 (signed packages, real update replacement, Manual SKU with OS-level no-egress) | Rust-first GA, universal IDE, autonomy, or enterprise |
| Public alpha / focused IDE GA | Out of scope for this sequence | Do not use this file to argue those claims |

The full-pass synthesizer marked even dogfood **Not yet** because it fail-closed at level 5. This sequence restores the August 31 audit's split: dogfood can be honest on unsigned local/native packages once Waves 0–2 land; preview still needs Wave 3.

## 3. Evidence ladder (non-negotiable)

A `GAP-*` task is accepted only when its evidence file names all five, or explicitly stops at a numbered level:

1. Unit
2. Subsystem
3. Desktop composition reachability (no test-only seams)
4. Windowed GUI (not headless kittest / `--beta-smoke` / AppComposition binaries)
5. Installed signed artifact on clean Windows, macOS, and Linux

Headless fixture tests cannot close a task whose acceptance names level 4 or 5.

## 4. Wave graph

```text
Wave 0  Honesty + governance     GAP-08, GAP-07
   |
Wave 1  Daily-driver safety      GAP-04, GAP-10
   |
Wave 2  Proof surface            GAP-01, GAP-09, GAP-05   (parallel after Wave 1)
   |
Wave 3  Trust chain              GAP-02 (procurement starts in Wave 0), GAP-03, GAP-06
```

`GAP-02.1` (certificate procurement) starts on day one and does not gate Waves 0–2.

---

## Wave 0 — Honesty and governance

Goal: a green `legion-gates.yml` run means the docs and the hosted workflows agree, and `main` cannot be force-pushed around the standing checks.

### GAP-08 — Documentation truth (P0-08)

| Task | Outcome | Primary files | Verification | Acceptance | Stop condition |
| --- | --- | --- | --- | --- | --- |
| GAP-08.1 | `claim-audit` and `verify-readiness-consistency` fail when AGENTS.md, ledger evidence cells, Appendix A, or workflow headers contradict each other | `xtask/src/claim_audit.rs`, `xtask/src/readiness_consistency.rs`, `xtask/tests/` | `cargo run -p xtask -- claim-audit`; `cargo run -p xtask -- verify-readiness-consistency`; `cargo test -p xtask` | New failing fixtures for the drift rows named in the full pass (hosted release workflow exists; perf-harness is hosted; GP-1 3-OS is independent; PR-AI-001 is not a live-model GUI) | Do not silence drift with allowlists |
| GAP-08.2 | Rewrite the stale sentences so the widened gates pass honestly | `AGENTS.md`, `plans/product-readiness-ledger.md`, `plans/legion-production-master-plan-v0.2.md` Appendix A, `docs/USER_GUIDE.md` | same as GAP-08.1 plus `docs-hygiene` | Each rewritten sentence names the current workflow or test; no row status change unless that row's own evidence file already supports it | Do not promote PR-AI-001, PR-UI-001, or PR-REL-001 |

Depends on: none. Do GAP-08.1 first so GAP-08.2 is forced by a red gate.

### GAP-07 — Governance (P0-07)

| Task | Outcome | Primary files | Verification | Acceptance | Stop condition |
| --- | --- | --- | --- | --- | --- |
| GAP-07.1 | `CODEOWNERS` covers workflows, `xtask`, protocol, app, desktop, and `plans/` | `.github/CODEOWNERS` | file exists; `docs-hygiene` | Required-review owners are named people or teams that exist in the org | Do not add owners that GitHub will ignore |
| GAP-07.2 | Release-blocker issue template and label (`qual-11` or equivalent) | `.github/ISSUE_TEMPLATE/`, `.github/labeler` or org labels | template renders; id `QUAL.11` is defined in-repo | Every P0 remaining gap can be filed with severity, owner, and linked ledger row | Do not use the generic `bug` template as the queue |
| GAP-07.3 | GitHub ruleset: `main` requires PRs, the Standing gates job, cargo-deny, and recorded bench; no force-push | GitHub rulesets (operator); note the SHA in evidence | `gh api` (or UI export) of the ruleset committed under `plans/evidence/production/WS-P0/` | Evidence file names the ruleset id and required checks | Do not claim protection from workflow YAML alone |

Depends on: none. Parallel with GAP-08.

**Landed 2026-08-31:** GAP-07.2 taxonomy + issue form + GitHub labels; GAP-07.3 ruleset `protect-main` id `21950476` (`plans/evidence/production/WS-P0/gap-07-3-main-ruleset.md`). Independent review is still off (single owner). Direct pushes to `origin/main` fail.

---

## Wave 1 — Daily-driver safety

Goal: killing the desktop does not drop dirty buffers, and Help/About can emit a metadata-only support bundle plus real legal files.

Depends on: Wave 0 (so new product claims cannot land as overstated).

### GAP-04 — Data safety (P0-04)

| Task | Outcome | Primary files | Verification | Acceptance | Stop condition |
| --- | --- | --- | --- | --- | --- |
| GAP-04.1 | Crash-safe unsaved-buffer snapshots, restored on relaunch | `crates/legion-app/src/lib.rs`, `crates/legion-storage/`, `crates/legion-desktop/src/session.rs` | `cargo test -p legion-app`; `cargo test -p legion-desktop --test session_restore` | A killed dirty desktop session restores the dirty body, not only disk text; session JSON still rejects raw-secret markers in the durable metadata file | Do not persist secrets in plaintext session.json; keep proposal-mediated save |
| GAP-04.2 | Local-history metadata reloads from `.legion/local-history` | `crates/legion-storage/src/local_history.rs` | `cargo test -p legion-app --test local_history_workflow` | Restart shows the same history entries without an in-memory-only store | Do not block typing on history I/O |
| GAP-04.3 | Windowed or desktop-integration proof of GAP-04.1 | `crates/legion-desktop/tests/` | new desktop test plus a named journal | Evidence path in `plans/evidence/production/WS-P0/` | Headless-only restore is level 3, not dogfood-bar close |

### GAP-10 — Support / legal (P0-10)

| Task | Outcome | Primary files | Verification | Acceptance | Stop condition |
| --- | --- | --- | --- | --- | --- |
| GAP-10.1 | `LICENSE` and a user-facing privacy policy that match Manual zero-egress and opt-in AI | `LICENSE`, `docs/` privacy page, `docs/INDEX.md`, `README.md` | `docs-hygiene`, `claim-audit` | Canonical docs are linked from INDEX; no "generally available" / "production-ready" claims | Do not imply OSI or public distribution |
| GAP-10.2 | `SupportBundleAssembler` is called from AppComposition; Help/About exports metadata-only | `crates/legion-app/src/diagnostics.rs`, `crates/legion-desktop/src/view.rs` | `cargo test -p legion-desktop --test diagnostics_export`; projection/GUI path | Desktop action exists outside `--diagnostics-export` | No raw source or secrets in the bundle |
| GAP-10.3 | Native packages include LICENSE, privacy, and generated third-party notices | `scripts/package-native.*`, `crates/legion-desktop/src/package.rs` | packaging tests; layout smoke | Extracted artifact contains the three files | Unsigned-beta may ship these before Authenticode exists |

---

## Wave 2 — Proof surface

Goal: a merge-blocking (or promotion-clocked) 3-OS path drives a **window**, not `--beta-smoke`, and renderer/a11y evidence is current.

Depends on: Wave 1. GAP-01, GAP-09, and GAP-05 may run in parallel.

### GAP-01 — Installed-product truth (P0-01)

| Task | Outcome | Primary files | Verification | Acceptance | Stop condition |
| --- | --- | --- | --- | --- | --- |
| GAP-01.1 | Windowed GUI E2E on an extracted native package (unsigned allowed) | `crates/legion-desktop/src/smoke.rs`, `scripts/verify-native-package.*`, new xtask | local 1-OS run that launches `eframe::run_native` (not `--beta-smoke`) and completes open/edit/save | Report names the binary path, OS, and that a window was created | Do not accept AppComposition `golden-path-5` as this task. **Landed 2026-09-01** on #196 |
| GAP-01.2 | 3-OS CI job, hard-fail, independent first | `.github/workflows/` (new or extend preview/release verify) | hosted run URLs | Evidence file with three OS artifacts; `continue-on-error` forbidden on the GUI step | Do not fold into PR gates until the T0-D four-green-run clock plus owner sign-off. **Landed 2026-09-02** on #205; clock 4/4 + owner sign-off in [`gap-01-2-windowed-gui-clock-signoff.md`](evidence/production/WS-P0/gap-01-2-windowed-gui-clock-signoff.md). Still independent; not a required check. |
| GAP-01.3 | Filled installed-preview journal | `plans/evidence/dogfood/` | journal template fields complete | `YYYY-MM-DD-installed-preview-journal.md` names SHA, OS, result | Checklist-only files are not evidence. **Landed 2026-09-01** on #196 (`plans/evidence/dogfood/2026-09-01-installed-preview-journal.md`) |

Follow the existing smoke-promotion clock in [`plans/evidence/production/WS-P0/T0-D-smoke-promotion-criteria.md`](evidence/production/WS-P0/T0-D-smoke-promotion-criteria.md): four consecutive green 3-OS runs and owner sign-off before this job becomes merge-blocking.

### GAP-09 — Performance (P0-09)

| Task | Outcome | Primary files | Verification | Acceptance | Stop condition |
| --- | --- | --- | --- | --- | --- |
| GAP-09.1 | Commit measured renderer reports, including `large_file_manual_renderer_perf.toml` | `xtask/src/perf_harness.rs`, `plans/evidence/perf-harness-trend/` | `cargo run -p xtask -- perf-harness`; `verify-perf-harness` | Reports exist for the current SHA on at least one OS; 100MB row is paint, not text-model | Do not treat `large_file_perf` (EditorEngine) as paint. **Landed 2026-09-01** on #198 |
| GAP-09.2 | Fail-closed armed budgets for the renderer rows that have reports | `.github/workflows/legion-gates.yml` | gates job without `LEGION_PERF_FAIL_ON_BUDGET_MS=0` on those rows | Red build on budget miss | Keep skeleton rows report-only if still synthetic. **Landed 2026-09-01** on #200 |
| GAP-09.3 | Lexical indexer off the file-open path | `crates/legion-app/src/lib.rs` `bind_opened_file` | `product_perf` p8.startup; typing tests | Open no longer waits on `LexicalIndexer` | Do not block save or first paint on index. **Landed 2026-09-01** on #201 |

### GAP-05 — Accessibility (P0-05)

| Task | Outcome | Primary files | Verification | Acceptance | Stop condition |
| --- | --- | --- | --- | --- | --- |
| GAP-05.1 | Finish the renderer-backed keyboard-only path in the PR-15 packet | `crates/legion-desktop/tests/keyboard_nav.rs`, `plans/evidence/accessibility/PR-15-manual-keyboard-path.md` | desktop keyboard tests; named remaining keymap routes closed or explicitly cut | Evidence lists which routes are certified vs residual | AccessKit unit roles alone are not certification. **Landed 2026-09-02** on #202 |
| GAP-05.2 | Windows NVDA or Narrator transcript of a live window | `plans/evidence/accessibility/`, `scripts/a11y-uia-walk.ps1` | committed transcript + SHA + OS | Names the AT used | UIA tree dump is not a screen-reader session. **Landed 2026-09-02** on #203 |
| GAP-05.3 | macOS AX probe + VoiceOver notes | `scripts/` plus evidence | committed AX dump and VoiceOver notes | macOS is no longer "unobserved" | One-off unreproducible dumps stay insufficient |
| GAP-05.4 | Linux AT-SPI probe + Orca notes | `scripts/` plus evidence | committed AT-SPI dump and Orca notes | Linux is no longer "unobserved" | Same as GAP-05.3 |

Dogfood bar can close with GAP-05.1 plus GAP-05.2 (Windows). Preview still needs GAP-05.3 and GAP-05.4.

---

## Wave 3 — Trust chain

Goal: invitation-only technical preview is defensible.

Depends on: Wave 2 for the thing being signed; `GAP-02.1` already in flight.

### GAP-02 — Release signing (P0-02)

| Task | Outcome | Primary files | Verification | Acceptance | Stop condition |
| --- | --- | --- | --- | --- | --- |
| GAP-02.1 | Issue `EXT-CERT-WIN/MAC/LIN` | `plans/release/procurement-and-key-escrow.md` | escrow checklist complete | Certs exist in the org secret store, not the repo | Do not commit private keys. QUAL.11 issues filed 2026-09-02: [#213](https://github.com/9thLevelSoftware/legion-ide/issues/213) WIN, [#211](https://github.com/9thLevelSoftware/legion-ide/issues/211) MAC, [#212](https://github.com/9thLevelSoftware/legion-ide/issues/212) LIN. **Not closed** — certs are not in the org secret store. |
| GAP-02.2 | `legion-release.yml` signs with signtool / codesign+notarytool / Linux package signature; `signer_status` is no longer forced `unsigned-beta` | `.github/workflows/legion-release.yml`, `xtask/src/signing.rs`, `scripts/package-native.*` | dry-run then a dispatch publish | Descriptor `signer_status` reflects a real OS signer | Ed25519 manifest-only signing is not Authenticode |
| GAP-02.3 | Fresh-VM Gatekeeper / SmartScreen / Linux trust evidence | `plans/evidence/release/` | three OS journals | Replaces the descriptor-only `P8-F1-T3` checkpoint | Operator "click through SmartScreen" is a fail |

### GAP-03 — Update safety (P0-03)

Depends on: GAP-02.2 (signed metadata and artifacts).

| Task | Outcome | Primary files | Verification | Acceptance | Stop condition |
| --- | --- | --- | --- | --- | --- |
| GAP-03.1 | `HttpManifestSource` + desktop check-for-update path | `crates/legion-app/src/updater.rs`, `crates/legion-desktop/` | app updater tests; desktop reachability | AppComposition can fetch a signed manifest without the drill binary | No silent egress in Manual |
| GAP-03.2 | Installer swap, process restart, N−1 restore | `crates/legion-app/src/updater.rs`, ADR-0042 D5 | extended `update-drill` | Drill steps cover replace, restart, interrupt, rollback, N−1 | Journal-only apply/rollback is not this task |
| GAP-03.3 | Hosted update feed for preview channel | `.github/workflows/legion-release.yml` or a feed workflow | fetch from a clean VM | Feed URL + signature documented in OPERATOR_RUNBOOK | Unsigned zip layout smoke is not a feed |

### GAP-06 — Manual / no-AI proof (P0-06)

Depends on: GAP-02.2 for a signed Manual channel; GAP-01.1 for the install/launch loop.

| Task | Outcome | Primary files | Verification | Acceptance | Stop condition |
| --- | --- | --- | --- | --- | --- |
| GAP-06.1 | Separate `--no-default-features --features offline` native package channel, labeled Manual in installer and About | `crates/legion-desktop/Cargo.toml`, `scripts/package-native.*`, `xtask` release descriptors | `cargo check -p legion-desktop --no-default-features --features offline`; package layout | Artifact SBOM/deps do not include provider HTTP stacks as product features | Default `ai` desktop build is not the Manual SKU |
| GAP-06.2 | OS-level no-egress on a clean VM (packet capture) for the Manual artifact | evidence under `plans/evidence/production/WS-MANUAL-01/` | capture logs committed or summarized with SHA | Open/edit/save/search/build/test/git produce zero DNS/TCP/UDP to providers | AppComposition `manual_zero_egress` is not this task |

---

## 5. Parking lot (not this sequence)

Do not pull these into a P0 wave. They are the full-pass P1–P3 rows:

- P1 workbench splits / merge editor / settings profiles
- Language Pack SDK and non-Rust servers
- DAP condition/hit/log + windowed real-adapter journal
- Generic test controller
- Forge PR APIs
- Provider catalog / local-model sidecar
- Native minidump process
- VSIX / Node / web extension hosts
- SSH remote server and Dev Containers
- `legion-agentd`, Automate desktop execute, AppContainer/VM isolation
- MCP 2026-07-28 / ACP v1
- SSO/SCIM and durable collaboration

## 6. Suggested first week

1. GAP-08.1 (widen claim-audit) and GAP-07.1 (`CODEOWNERS`) in one PR. **Landed 2026-08-31** with GAP-08.2 in the same change so the new gate stays green: `.github/CODEOWNERS`; `xtask` cross-doc claim-audit + Appendix A consistency; AGENTS.md / ledger / Appendix A / USER_GUIDE wording corrected.
2. ~~GAP-08.2 (rewrite stale sentences) immediately after, forced by the new reds.~~ Done with item 1.
3. GAP-07.2 / GAP-07.3. **Landed 2026-08-31:** QUAL.11 taxonomy + release-blocker issue form; GitHub labels `qual-11` / `release-blocker` / `severity-p0`…`p3`; ruleset `protect-main` id `21950476` (see `plans/evidence/production/WS-P0/gap-07-3-main-ruleset.md`). Independent review is still off (single owner). Direct pushes to `origin/main` now fail.
4. Open GAP-02.1 procurement in parallel (human, not a code PR). QUAL.11 issues [#211](https://github.com/9thLevelSoftware/legion-ide/issues/211)/[#212](https://github.com/9thLevelSoftware/legion-ide/issues/212)/[#213](https://github.com/9thLevelSoftware/legion-ide/issues/213) filed 2026-09-02; certs still not in the org secret store.
5. GAP-04.1 crash-safe dirty restore. **Landed 2026-08-31** on this PR: sidecar `.legion/unsaved/` (not session JSON); killed dirty session restores buffer text; disk is unchanged until a proposal-mediated save.
6. GAP-04.2 local-history metadata reload. **Landed** on #192: `.legion/local-history/manifest.json` round-trips identity/hash/timestamp; blobs without metadata rows are not offered after restart. Typing is unchanged — persist runs on the save path.
7. GAP-04.3 desktop-integration proof of GAP-04.1. **This change:** `session_restore_killed_dirty_session_restores_sidecar_without_writing_disk` plus `plans/evidence/production/WS-P0/gap-04-3-desktop-hot-exit.md`. Not a windowed GAP-01 GUI run.

## 7. Standing gates

Every implementation PR still runs the 21 local standing gates in `AGENTS.md`. GAP-01.2's promotion clock is complete (owner sign-off 2026-09-02); windowed-gui remains an independent workflow and is not a 22nd local gate or a `protect-main` required check.
