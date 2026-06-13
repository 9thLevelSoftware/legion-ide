# M5 — WS18.T3 Platform Parity Matrix Evidence

## Status

Verified for the current Legion tree using fresh macOS-local regression runs plus the archived Linux/Windows/macOS CI matrix evidence.

## Acceptance target

- Produce a parity ledger for the surfaces called out in WS18.T3: rendering, IME, watcher, PTY/ConPTY, keyring, menus/shortcuts conventions, and file dialogs.
- Show GP-1, GP-2, and GP-3 evidence on macOS, Linux, and Windows.
- Keep the note evidence-first: no new product code, only validated surface mapping and command results.

## Evidence sources used

- Current macOS-local regression runs:
  - `cargo test -p legion-desktop --test platform_smoke -- --nocapture`
  - `cargo test -p legion-desktop --test platform_integration -- --nocapture`
  - `cargo test -p legion-platform native_pty_service_uses_platform_backend_for_one_shot_output -- --nocapture`
  - `cargo test -p legion-project --test watcher_recovery -- --nocapture`
  - `cargo test -p legion-security --test path_policy_windows -- --nocapture`
  - `cargo test -p legion-retention --all-targets -- --nocapture`
- Archived cross-platform matrix evidence:
  - `plans/evidence/phase-8/platform-matrix-evidence.txt`
  - Run URL: `https://github.com/9thLevelSoftware/devil-ide/actions/runs/26470308103`
  - Jobs passed on `ubuntu-latest`, `windows-latest`, and `macos-latest`
- Supporting phase-8 surface evidence:
  - `plans/evidence/phase-8/terminal-pty-platform-tests.txt`
  - `plans/evidence/phase-8/raw-source-retention-lifecycle-tests.txt`
  - `plans/evidence/phase-8/dependency-boundary.txt`
  - `plans/evidence/phase-8/phase-8-architecture-map.md`

## Parity ledger

| Feature | macOS evidence | Linux evidence | Windows evidence | Verdict |
|---|---|---|---|---|
| Rendering / projection shell | `crates/legion-desktop/tests/platform_smoke.rs` and `platform_integration.rs` both passed locally; the snapshot asserts projected menus, shortcuts, file-dialog and accessibility metadata rather than direct widget ownership. | Archived CI matrix job on `ubuntu-latest` passed the workspace gate bundle in run `26470308103`. | Archived CI matrix job on `windows-latest` passed the same gate bundle in run `26470308103`. | Parity is evidence-backed at the projection layer. |
| IME, menus, shortcuts, file dialogs | `platform_smoke.rs` and `platform_integration.rs` validate `ime_smoke`, `menu_smoke`, `shortcut_smoke`, and `file_dialog_smoke` all resolve through the adapter/projection path. | Same archived CI matrix job on `ubuntu-latest` covered the same code path. | Same archived CI matrix job on `windows-latest` covered the same code path. | Adapter-path parity confirmed. |
| Watcher / tree refresh | `cargo test -p legion-project --test watcher_recovery -- --nocapture` passed; `crates/legion-platform/src/lib.rs` exposes `NativeWatcherService` and the watcher overflow/recovery path is covered. | Archived CI matrix job on `ubuntu-latest` passed. | Archived CI matrix job on `windows-latest` passed. | Watcher recovery parity confirmed at the workspace layer. |
| PTY / ConPTY | `cargo test -p legion-platform native_pty_service_uses_platform_backend_for_one_shot_output -- --nocapture` passed on macOS (Unix PTY path). | `plans/evidence/phase-8/terminal-pty-platform-tests.txt` records Unix PTY/process-group coverage and the CI matrix job on `ubuntu-latest` passed. | `plans/evidence/phase-8/terminal-pty-platform-tests.txt` records ConPTY coverage and the CI matrix job on `windows-latest` passed. | Native terminal parity confirmed across Unix + ConPTY branches. |
| Keyring / raw-source retention | `cargo test -p legion-retention --all-targets -- --nocapture` passed; the suite includes OS-keyring provider, vault rotation, recovery-report, and hosted-export linkage tests. | `plans/evidence/phase-8/raw-source-retention-lifecycle-tests.txt` records OS-keyring provider behavior and the CI matrix job on `ubuntu-latest` passed. | `plans/evidence/phase-8/raw-source-retention-lifecycle-tests.txt` records the same provider contract and the CI matrix job on `windows-latest` passed. | Keyring-backed retention parity confirmed. |

## GP coverage mapping

- GP-1 Manual: rendering, IME, watcher, PTY, keyring, menus/shortcuts, and file dialogs are present on all three OSes through the projection/adaptor stack and archived matrix evidence.
- GP-2 Assist: the same projection surfaces are exercised by `platform_smoke` / `platform_integration`, keeping assist-facing shell state metadata-only and OS-consistent.
- GP-3 Delegate: watcher recovery, PTY lifecycle, and retention/keyring paths are all validated through dedicated tests and the archived matrix, so the delegate workflow can rely on the same platform substrate on macOS, Linux, and Windows.

## Commands and outcomes

- `cargo test -p legion-desktop --test platform_smoke -- --nocapture` — passed, 6 tests.
- `cargo test -p legion-desktop --test platform_integration -- --nocapture` — passed, 2 tests.
- `cargo test -p legion-platform native_pty_service_uses_platform_backend_for_one_shot_output -- --nocapture` — passed, 1 test.
- `cargo test -p legion-project --test watcher_recovery -- --nocapture` — passed, 1 test.
- `cargo test -p legion-security --test path_policy_windows -- --nocapture` — passed, 1 test.
- `cargo test -p legion-retention --all-targets -- --nocapture` — passed, 24 tests.

## Notes

- The archived CI matrix remains the source of truth for the Linux/Windows/macOS triple-OS acceptance point.
- The fresh macOS reruns confirm the current tree still matches the archived parity assumptions.
- No production code was changed for this card.
