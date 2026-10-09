# Native package smoke prerequisite repair

Date: 2026-10-09. Branch: `codex/ide-2026-native-smoke-repair`.
Status: independently reviewed; Lagrange PASS on all five code files, reported by
the coordinator on 2026-10-09. Coordinator authorized committing these five files
and this evidence document, then fast-forward integration. This is a bounded prerequisite repair, not
ticket 04 native-input qualification or product acceptance.

## Failure evidence and scope

Coordinator evidence at `D:/legion-ide-2026-notes/pilot-candidate-87580fa.md`
records the real offline unsigned MSI: checksum, identity/version and extraction
passed, while headless beta smoke failed with Running/results=0 searches and
`offline.ai_feature_disabled` from default Assist. The coordinator also confirmed
the staging gate failed because `legion-desktop.exe.sha256` had no producer.
That baseline was not rerun unchanged. No signing, push or acceptance promotion.

Read the TDD skill, glossary and authority boundaries before editing. Public seams
were already approved: beta workflow report, desktop launch configuration and the
existing package-verifier script harness. Only beta.rs, narrow workflow.rs CLI
plumbing/frame-refresh visibility, beta_workflow.rs, the two owned verifier scripts
and this evidence document change. App/provider workers retain their other files.

## Repair

- `--beta-manual-local` explicitly selects the existing Manual workflow and
  requires `--beta-smoke`. Default beta still selects Assist; no implicit fallback.
  The Windows offline verifier passes this flag and exposes its actual argument
  array in `-PrintSmokePlan` for the existing harness.
- Beta waits at most five seconds per search, using the existing desktop frame
  projection refresh that drains app-owned results. It does not redispatch search
  or explorer intents. Timeout cancels the current query and remains a gate failure.
  Existing observable report assertions now require two active-file and five
  workspace matches from the known fixture.
- After installer checksum, metadata, version and unique extraction checks, the
  verifier rechecks the MSI hash and emits the extracted executable's SHA-256
  sidecar beside the MSI, logging the corresponding installer hash. Non-empty
  extraction directories fail closed: use a fresh WorkspaceRoot instead of
  binding stale files. No staging provenance gate is bypassed.
- Payload binding describes verified extraction, independently of smoke success.
  Smoke failure still writes `result = "failed"`. The focused regression mocks
  only OS extraction and uses real COM version/checksum checks; it does not qualify
  actual MSI installation. The stale `target/release-smoke` harness fixture now
  supplies the required payload sidecar before testing the directory policy.

## Exact local checks

All Cargo commands ran from this worker root with
`--target-dir D:/legion-ide-2026-tools/qualification-target`.

1. `cargo test -p legion-desktop --no-default-features --features offline --test beta_workflow manual_local_workflow_stays_in_manual_and_skips_ai_proposal --target-dir D:/legion-ide-2026-tools/qualification-target`
   passed before runtime changes, including strengthened fixture counts. This
   test did not reproduce the actual packaged async failure; no red claim is
   inferred from it and it was not repeated.
2. The same Cargo test command selecting
   `desktop_launch_config_selects_manual_beta_only_when_explicit` failed before
   implementation (`unsupported desktop argument: --beta-manual-local`), then
   passed on its one affected rerun. It also checks unchanged Assist default and
   rejection of the selector without beta smoke.
3. `pwsh -NoProfile -File scripts/test-native-package-verifiers.ps1`:
   21 passed, 2 failed, 0 skipped. Failures were missing observable Manual smoke
   arguments and the stale release-smoke fixture missing its payload sidecar.
   After supported fixes, only the affected tests ran:
   `-TestFilter 'ps verifier plans windows-x64-msi*'` and
   `-TestFilter 'stage script rejects a source under target/release'` each passed
   (1 passed, 0 failed, 0 skipped). Successful full-harness cases were not repeated.
4. `-TestFilter 'ps verifier binds extracted payload*'` first exposed a fixture
   problem: cmd.exe exited zero instead of failing beta smoke. Correcting the
   system-boundary fixture to where.exe produced the intended red: missing payload
   sidecar after extraction, with smoke exit 2/result failed. After producer repair
   the affected test passed (1/0/0), retaining the failed smoke result.
5. `cargo run -p legion-desktop --no-default-features --features offline --target-dir D:/legion-ide-2026-tools/qualification-target -- --beta-smoke --beta-manual-local --workspace . --beta-workspace target/native-smoke-repair/workspace --evidence target/native-smoke-repair/beta.md --session-state target/native-smoke-repair/session.json --diagnostics-export target/native-smoke-repair/diagnostics.md`
   exited 0. Actual executable, without test-helper search behavior: report passed,
   Manual, searches Completed with 2/5 results, saved fixture, AI proposal skipped.
   This is headless runtime evidence, not MSI/native-input evidence.
6. `cargo test -p legion-desktop --test beta_workflow beta_workflow_runs_through_desktop_runtime_and_writes_metadata_evidence --target-dir D:/legion-ide-2026-tools/qualification-target`
   passed (1 test): existing Assist proposal workflow retained. Existing vendor
   float fallback and offline unused-code warnings were not changed.
7. `-TestFilter 'ps verifier refuses stale extraction*'`: first selected zero
   tests due to accidental placement inside the extraction fixture here-string;
   that run is not credited. Placement was corrected and the affected check passed
   (1/0/0): stale payload refused before producing a binding.
8. `rustfmt --edition 2024 --check crates/legion-desktop/src/beta.rs crates/legion-desktop/src/workflow.rs crates/legion-desktop/tests/beta_workflow.rs`
   passed after formatting only those owned files. `git diff --check` found a
   trailing blank line in the PowerShell harness; removed that whitespace and
   reran only the affected diff check, which passed. No broader workspace gates ran.

## Integration and remaining gates

Merged integration cb817998 before starting, then fast-forwarded to 7b69a45
(ticket 02/235 documentation and coordinator execution changes) before handoff.
Original workspace and frozen MSI were not modified. Successful checks were not
repeated after review. A rebuilt immutable MSI must still pass real extraction/headless verification
and staging with this producer; native-input/AT, DAP/MSVC and actual pilot observation
remain separate downstream evidence. Payload hashes are integrity bindings, not
signatures or readiness decisions. Canonical acceptance remains unchanged.
