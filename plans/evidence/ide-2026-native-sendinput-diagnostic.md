# Native SendInput diagnostic checkpoint

Date: 2026-10-09. Ticket 04 remains needs-info; no native acceptance is credited.

The owner explained they had been away during the prior foreground timeout and
explicitly requested another attempt. The coordinator ran the archived reviewed
driver `00ae938`, SHA-256
`400d068d7e87cd78d0c3e3101b50d2056d55027506bc6ee4e5e3d08cfba7fadb`,
with the same frozen product and reference checkout as the
[journey receipt](ide-2026-ticket004-native-journey.md).

Report:
`D:/legion-ide-2026-notes/ticket004-native-journey-attended-navigation-r2.toml`.
The process created HWND `0x140e38`, PID 56040. The attended invocation returned
after approximately 5.6 seconds with report code 3 (blocked), outer PowerShell
code 1, and `SendInput accepted 0 of 3 events`. The reference checkout's subsequent
`git status --short` was empty.

The source path establishes that the exact foreground guard and UIA bootstrap
had passed before this SendInput call, during the first Explorer drawer click.
Windows accepted no events from that batch. No file open/edit/save was observed.
The report does not contain GetLastError or process integrity levels, so neither
a particular desktop access right nor UIPI is established as the cause.

## Bounded diagnostic change

On base `26821c7`, the coordinator added immediate, cleared GetLastError capture
at the existing SendInput boundary and reports the INPUT structure byte size.
The error text no longer attributes every partial/zero send to the desktop.
The journey additionally records successful exact foreground observation and
UIA root acquisition before navigation. Existing per-input foreground guards,
atomic batches, desktop access flags, event contents and failure behavior remain
unchanged. No retries, elevation, ACL/security changes or alternate input route
were added.

[Microsoft's SendInput contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)
documents the count/error-code observations and warns that UIPI is not identified
by that return value or error code. A zero error code must not be read as success
or as proof that UIPI is absent.

Planned checks are changed-file rustfmt, driver build and diff check once. This
diagnostic-only patch does not warrant a synthetic test that merely repeats its
format string. Actual OS error capture and the original workflow remain pending
a separately attended run after review; no cause or repair is claimed.

Checks completed once:

- `cargo build --locked -p legion-input-driver --target-dir D:/legion-ide-2026-tools/native-input-target`: passed, 5.13 seconds.
- `rustfmt --check --edition 2024 crates/legion-input-driver/src/inject.rs crates/legion-input-driver/src/journey.rs`: passed after formatting those files.
- `git diff --check`: passed; Git emitted only line-ending conversion warnings.

Built debug driver SHA-256:
`f717cf675ef7f9aa03316e77f8af1f7c34aca139e50f1f7c28ad24a7798e51ff`.
At that checkpoint this binary had not been used for native input. The coordinator archived it at
`D:/legion-ide-2026-tools/legion-input-driver-diagnostic-f717cf67.exe` and verified
the same hash. `cargo run --locked -p xtask --target-dir D:/legion-ide-2026-tools/qualification-target -- docs-hygiene`
also passed once.

Pauli's independent review passed with no material findings: immediate same-thread
last-error capture, unchanged guards/input/security behavior, accurate checkpoint
ordering, and no unsupported cause or acceptance claim. The owner has been asked
about availability for the next attended run; this receipt does not imply one
has happened.

## Attended diagnostic result and desktop-access repair

The owner then replied "ready now". The coordinator launched the verified
`f717cf67` archive against the unchanged frozen product/reference checkout.
Report: `D:/legion-ide-2026-notes/ticket004-native-journey-attended-sendinput-diagnostic.toml`.
HWND `0x914ba`, PID 63764; outer exit 1, report exit 3. The report explicitly
records successful exact foreground observation and UIA root acquisition, then
`SendInput accepted 0 of 3 events; win32_last_error=5; input_struct_bytes=40`.
No event in that batch was accepted. The reference Git status remained empty and
README SHA-256 remained
`5da9ac0a7844b4f215829bc2a6523d120fdbf97e3e1759234aa12623be29bc9a`.
Error 5 is access denied; its presence still does not establish UIPI.

A separate read-only probe queried its own process and existing desktop objects,
without launching a product, switching desktops, injecting input or changing
security settings. Source/output:
`D:/legion-ide-2026-notes/ticket004-readonly-access-probe.rs` and `.txt`.
Observed process: session 1, integrity RID 8192 (medium), elevated 0, UIAccess 0.
Its station was WinSta0. Its inherited Default desktop received input and had
granted access `0x000f01ff`; a separately opened input desktop also named Default
received input but had the driver-requested mask `0x00000083`. These are that
probe's observations, not captured token metadata from the already-exited driver
or product process.

At the existing public session/OS boundary, a new opt-in test then reproduced
the actual probe behavior: `probe_input_desktop` reduced the calling thread's
desktop access from 983551 (`0xf01ff`) to 131 (`0x83`). This requires no product
or SendInput call. The repair on base `2392643` queries GetThreadDesktop/UOI_IO
and retains the borrowed current handle when Windows confirms it receives input.
Query errors remain blocked. If it does not receive input, the existing bounded
OpenInputDesktop/SetThreadDesktop path and its `0x83` request remain unchanged.
The driver adds only the existing windows crate's Threading API feature.

[SetThreadDesktop](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddesktop)
documents that subsequent operations use the assigned handle's access rights.
[GetThreadDesktop](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getthreaddesktop)
returns a handle that does not require closing, and
[UOI_IO](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getuserobjectinformationw)
tests whether that desktop receives input. No ACL, token, elevation, active
desktop, event content, foreground guard or retry policy was changed.

Planned focused checks, once each except the affected red-to-green rerun:

- `cargo test --locked -p legion-input-driver --test native_session already_attached_input_desktop_preserves_existing_access --target-dir D:/legion-ide-2026-tools/native-input-target -- --exact --ignored`: failed on the observed access reduction, then passed after repair. Logs `ticket004-preserve-desktop-access-red.log` and `ticket004-preserve-desktop-access-green.log` in the notes directory.
- `cargo test --locked -p legion-input-driver --test driver_contract attached_input_desktop_allows_native_sta_com_initialization --target-dir D:/legion-ide-2026-tools/native-input-target -- --exact --ignored`: passed once on the changed attachment path, 24 unrelated cases filtered; log `ticket004-preserved-access-com.log`. No input or product launch.
- `cargo build --locked -p legion-input-driver --target-dir D:/legion-ide-2026-tools/native-input-target`: passed.
- `rustfmt --check --edition 2024 crates/legion-input-driver/src/session.rs crates/legion-input-driver/tests/native_session.rs`: passed after formatting those files.

Compiled driver SHA-256:
`b8a44dd7c28920b9326317a494b79d06b156d55a0c649b832822aa1b51f7f7bc`.
The access-preservation regression is fixed. Whether it repairs SendInput in the
original native journey remains unverified until a reviewed, attended run.

`git diff --check` and the existing `cargo run --locked -p xtask --target-dir
D:/legion-ide-2026-tools/qualification-target -- docs-hygiene` both passed.
Pauli independently reviewed the three source/test files and then both evidence
updates, with no material findings. No successful checks were repeated after
review. The verified archive is
`D:/legion-ide-2026-tools/legion-input-driver-preserved-access-b8a44dd7.exe`.
The owner has been asked about availability for the repaired run; none has
occurred at this checkpoint.
