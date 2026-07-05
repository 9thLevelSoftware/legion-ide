# P8.F5.T3 Platform Parity Report

Date: 2026-06-14  
Git HEAD: `91ce6cc`  
Task: `t_3b3bb62e`  
Scope: IME/CJK, clipboard, file dialogs, keyring, PTY/ConPTY, watcher behavior

## Status

Parity is recorded for all supported OSes. macOS is backed by fresh local test runs in this workspace; Linux and Windows are backed by the archived CI matrix evidence already cited in the M5 parity corpus.

## macOS parity record

Fresh local verification in this workspace:

- `cargo test -p legion-desktop --test platform_smoke platform_smoke_adapter_paths_route_without_metrics_payloads -- --nocapture` — passed
- `cargo test -p legion-desktop --test platform_integration -- --nocapture` — passed
- `cargo test -p legion-desktop --test input_conformance clipboard_input_routes_to_the_buffer_through_the_egui_context -- --nocapture` — passed
- `cargo test -p legion-desktop --test input_conformance ime_input_routes_commit_text_and_preserves_the_commit_payload -- --nocapture` — passed
- `cargo test -p legion-project --test watcher_recovery -- --nocapture` — passed
- `cargo test -p legion-platform native_pty_service_uses_platform_backend_for_one_shot_output -- --nocapture` — passed
- `cargo test -p legion-retention --all-targets -- --nocapture` — passed, 24 tests

What these checks cover:

- IME/CJK: `input_conformance` preserves IME commit payloads, including non-ASCII input.
- Clipboard: `input_conformance` routes paste text through the egui context and keeps clipboard payloads intact.
- File dialogs: `platform_integration` keeps `file_dialog_smoke` on the adapter path.
- Keyring: `legion-retention` includes `os_keyring_provider_exposes_metadata_reference_without_inline_key` and the broader retention suite passes.
- PTY: `legion-platform` validates the native PTY backend path for one-shot output.
- Watcher: `watcher_recovery` proves overflow/rescan recovery and the recovery event path.

## Linux parity record

Linux is recorded in the archived CI matrix evidence already cited by the M5 parity corpus:

- `plans/evidence/phase-8/platform-matrix-evidence.txt`
- `plans/evidence/production/M5/WS18-T3-platform-parity-matrix.md`

Those records show the `ubuntu-latest` matrix leg passed for the parity bundle that covers rendering, IME, watcher, PTY/ConPTY, keyring, menus/shortcuts, and file dialogs.

## Windows parity record

Windows is recorded in the same archived CI matrix evidence corpus:

- `plans/evidence/phase-8/platform-matrix-evidence.txt`
- `plans/evidence/production/M5/WS18-T3-platform-parity-matrix.md`

Those records show the `windows-latest` matrix leg passed for the parity bundle.

## Verdict

Recorded parity exists for macOS, Linux, and Windows. No platform was left at an implicit `untested` fallback.

## Evidence handling

- Archive command labels, exit status, platform, run id, and artifact references only.
- Do not record raw source, secrets, provider payloads, transport frames, or terminal bodies.
- Treat missing platform coverage as blocked rather than implying parity.
