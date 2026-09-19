# Full product resume evidence — 2026-09-09

Component and integration evidence gathered while resuming the full-product
completion work. Nothing here is product acceptance. All 419 requirement rows
remain `acceptance: unassessed`, and register ratification remains provisional
and agent-made.

## First native product acceptance attempt: blocked

`round-r09-coordinator-native-acceptance.log` and its PowerShell repeat
`round-r09-coordinator-native-acceptance-pwsh.log` record the first real
invocation of `xtask native-product-acceptance` against the staged unsigned
package, with the in-repo input driver. Both exit **3, blocked**:

> The driver reached the window but could not read it: CoInitializeEx failed:
> Access is denied. (0x80070005)

Blocked is not a pass, not a skip, and not a defect against the product. No
`run.json` was written, so the completion evidence loader ignores the attempt,
and no requirement moved.

The host is not the limitation. The invoking session is interactive: session id
1, `Console` window station, `UserInteractive` true, one attached screen. The
same failure occurs from `bash` and from `pwsh`. The COM initialisation failure
is therefore in the in-repo driver added in round r06, and repairing it is the
remaining barrier to any product-layer evidence on this host.

## Live language servers

`round-r08-test-s2-02a-live-language-servers.log`, EXIT=0: `python_app_startup`
4 passed 0 failed in 23.47s, `typescript_app_startup` 6 passed 0 failed in
17.76s, against the retained Pyright and TypeScript archives with the approved
Node runtime. This is the first live language-server execution in this effort.
It is component and integration evidence and it accepts nothing.

A rerun taken while the MSI build loaded the host failed all four Python tests,
each reporting `Node executable identity check timed out`.
`round-r08-post-python-idle.log` then passed 4 of 4 again, EXIT=0, on an idle
host. All three runs are recorded because a rerun does not erase an
intermittent failure. That behaviour is filed as DEF-0003.

## Unsigned package

The packaged product used by the acceptance attempt is
`target/native-package/output/legion-desktop-windows-x64-msi.msi`, 15,675,392
bytes, built by `scripts/package-native.ps1` with its SHA-256 recorded beside
it. It is **unsigned**. Every signing, notarisation, clean-machine and release
row stays blocked on owner-supplied credentials.
