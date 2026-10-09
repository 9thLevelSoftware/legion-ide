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
This binary has not been used for native input. The coordinator archived it at
`D:/legion-ide-2026-tools/legion-input-driver-diagnostic-f717cf67.exe` and verified
the same hash. `cargo run --locked -p xtask --target-dir D:/legion-ide-2026-tools/qualification-target -- docs-hygiene`
also passed once.

Pauli's independent review passed with no material findings: immediate same-thread
last-error capture, unchanged guards/input/security behavior, accurate checkpoint
ordering, and no unsupported cause or acceptance claim. The owner has been asked
about availability for the next attended run; this receipt does not imply one
has happened.
