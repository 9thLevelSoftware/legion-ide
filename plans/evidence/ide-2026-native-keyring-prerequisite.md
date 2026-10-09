# Native credential-store prerequisite repair

Date: 2026-10-09. Bounded prerequisite for tickets 40 and 63, based on `00ae938`.
This does not close either ticket or promote native profile/live-provider acceptance.

## Defect and authority boundary

`OsKeyringSecretStore::load` classified missing entries by matching the display
text `not found`. The resolved keyring 3.6.3 library's `Error::NoEntry` instead
displays `No matching entry found in secure storage`. Empty credential slots
therefore returned storage errors. Conversely, a real storage failure whose
message included `not found` could incorrectly become absence.

The production change matches the typed `keyring::Error::NoEntry` variant only.
All other errors remain failures. No credential reference, backend, permission,
fallback, dependency or transport changed.

The deterministic regression uses the public `OsKeyringSecretStore` interface
with the keyring library's public backend seam. Its process-global mock builder
is confined to a dedicated one-test integration binary; it never accesses the
OS keyring. Native qualification uses a separate binary without that override.

## Planned checks and observed results

Each check was tied to this boundary: reproduce and repair missing-key behavior;
exercise actual Windows storage across processes; run existing secret contracts,
targeted clippy, formatting and whitespace verification. Successful checks were
not repeated after they passed.

All Cargo checks below used
`--target-dir D:/legion-ide-2026-tools/qualification-target` from the isolated
`D:/legion-ide-2026-workers/credential-missing` checkout.

| Command | Observed result |
| --- | --- |
| `cargo test --locked -p legion-storage --test credential_missing` | Red: `NoEntry` was incorrectly returned as `KeyringFailure`. After the one-line typed fix, the same affected check passed once. Also verifies that a platform error mentioning `not found` remains an error. |
| `cargo test --locked -p legion-storage --test native_keyring_qualification native_store_survives_process_restart_replacement_and_revocation -- --exact --ignored --nocapture` | Passed once on Windows with process-local `LEGION_NATIVE_KEYRING_QUALIFICATION=1`. One parent and three fresh child invocations exercised the actual native backend. |
| `cargo test --locked -p legion-storage --lib secrets::tests` | 3 existing secret-store contracts passed; 79 unrelated tests filtered out. |
| `cargo clippy --locked -p legion-storage --all-targets -- -D warnings` | Passed, exit 0. Compiled targets; did not execute ignored native checks again. |
| `cargo fmt -p legion-storage -- --check` and `git diff --check` | No formatting or whitespace issues reported. |
| `cargo run -p xtask -- docs-hygiene` | New prerequisite evidence passed documentation hygiene, exit 0. |

## Native qualification scope

The opt-in test generated a fresh UUID account under
`legion-ide-synthetic-keyring-qualification`. It first verified absence, then
stored a fixed synthetic value, reopened it in a child process, replaced it,
reopened the replacement in another child, and revoked it in a third child.
The parent verified absence after revocation. Cleanup also ran and succeeded.

Only that generated account was queried or modified. No existing user/provider
credential was enumerated or read. Values were not logged or sent to a provider;
no network call, product launch, elevation or desktop-permission change occurred.
The test is Windows-only, ignored by default and requires explicit environment
opt-in. It runs in a different test binary from the deterministic mock backend.

This establishes the observed native store/load/replacement/revocation primitives
and cross-process visibility in this host context. It does not establish native
UI entry, route-bound profile end-to-end restart, macOS/Linux keyrings, power-loss
behavior or live MiMo/MCP authentication. Those qualification gates remain open.
Independent review precedes integration of this patch.
