# Ticket 004 candidate r3: native full-document observation

Date: 2026-10-09. Ticket 04 remains incomplete. The read-only Windows document
prerequisite passed; native typing/save, recovery and full input conformance
remain unqualified. No fresh attendance is inferred from the earlier ready reply.

## Reviewed source and package

Pauli independently reviewed the 19-file bounded editor accessibility repair,
identified three P2 issues, and passed the final fixes: retained-button focus
provenance, contradictory warmed-cache previews, and logical-line continuity
across text-run fragments. The reviewed patch was committed and fast-forwarded
into integration as `caf1821f4499d6af858aed9992acbedfc4cb7087`, with both worktrees
clean. The [implementation receipt](ide-2026-ticket004-editor-accessibility-prerequisite.md)
records 23 unique focused passing tests and the compile/dependency/canvas/docs/
format checks. Its pending-review wording describes the worker handoff; this
receipt records the subsequent final PASS. No full-workspace or clippy pass is
claimed.

A new clean detached checkout at `D:/legion-ide-2026-candidate-r3` freezes that
exact source. Candidate-r2 and its prior evidence were preserved.

- Offline unsigned MSI version: `0.0.3`.
- Package: `target/native-package/output/legion-desktop-windows-x64-msi.msi`.
- MSI size: 14,323,712 bytes.
- MSI SHA-256: `23a063dd26f95374fb5b9c42e856ee6fc50d3947924149c69edb9015181d7d42`.
- Extracted/staged executable SHA-256: `01629d85d9be7456d7d6cb7aadd6fdb522f718391405fd106bad82377c20b6a4`.
- Staged executable: `D:/legion-ide-2026-candidate-r3/target/native-input-acceptance/package/legion-desktop.exe`.

The package build ran once with `scripts/package-native.ps1 -Version 0.0.3
-Format wix`; release compilation finished in 2m14s and packaging exited 0.
The existing epaint warning and 42 offline app warnings remain recorded; they
were not reclassified as warning-free compilation.

`scripts/verify-native-package.ps1` ran once with the exact package directory,
version, source SHA and frozen workspace root. Its summary passed checksum,
metadata, MSI version, extraction structure and headless Manual smoke
(`smoke_exit = 0`). The temporary PowerShell default for Start-Process window
style was Hidden and was restored afterward. No frozen script was edited.

`scripts/stage-native-acceptance-package.ps1` then staged the four-file payload
from the verifier's fresh `target/release-smoke/windows-x64-msi/staging`
extraction. MSI and executable digests matched their verified sidecars.
`STAGING-EVIDENCE.toml` records unsigned status. Frozen Git status remained clean.

These are local packaging and smoke checks, not signing, clean-machine,
cross-platform, release or native editing acceptance.

## Independent read-only Windows observation

The reviewed helper at
`D:/legion-ide-2026-notes/ticket004-tab-observer-r3/` copied the final driver
observation/session sources from `6eae87d`. Pauli's helper review found one P2:
an unavailable optional selection query could block valid document observation.
After making that query diagnostic only, its affected rebuild and final review
passed. Exact document, clean tab and visible/enabled positive-area geometry
remained mandatory.

- Helper SHA-256: `597533a6fcf2d2d84a0dab97d1187ba7712e52eae292d9ec2e1c20e0cf91d686`.
- Main source SHA-256: `178623bcea1ab927c5535e3535d95bccfa672d683f99029c11ebe838f8750b98`.
- Copied observe source SHA-256: `a9bcbed41c31a53322c909f9d06b57d7e597ffa2adea1c66fbf8f17eaf941446`.
- Copied session source SHA-256: `6cc70461fb5f05d62eaf72401dd8b7c1c852e07a0d5c2f0f1f04d27ee9810362`.

The helper ran once with `--observe-clean-tab` after its and the product's hashes
were checked. It launched the packaged product with explicit `--workspace` and
`--file` diagnostic setup, queried UIA and terminated only its owned child. It
sent no input, made no explicit activation call and saved nothing. Direct file
setup is not Explorer journey evidence.

The first sample passed: selected tab `Clean`, one exact complete document,
control type 50004, enabled, on screen, and rectangle `(161,328)-(1117,656)`.
UIA returned all 8,461 bytes / 8,413 Unicode scalar values, including the CRLF
terminators, with exactly the baseline SHA-256. There were 148 UIA elements.
The optional selection query returned an empty selection; this does not qualify
native selection mutation. Observer and outer command exited 0.

Reference checkout: `D:/legion-ide-2026-tools/ticket004-reference-c2a6578`, HEAD
`c2a65786862e759e67ebac28c32c1cbc618047f0`. It remained Git-clean with README
SHA-256 `5da9ac0a7844b4f215829bc2a6523d120fdbf97e3e1759234aa12623be29bc9a`.

Raw logs, preserved outside the checkout:

- `D:/legion-ide-2026-notes/ticket004-candidate-r3-package.log`
- `D:/legion-ide-2026-notes/ticket004-candidate-r3-verification.log`
- `D:/legion-ide-2026-notes/ticket004-candidate-r3-readonly-text-patterns.log`

## Remaining attended gate

Fresh owner availability was requested only after this read-only success. No
attended run has yet used candidate r3. The prepared driver is the reviewed
`legion-input-driver-tab-oracle-e4168202.exe`, SHA-256
`e416820272207a88f872978d99b7188a51f4a81aa460394fac1beea966052f74`.
It retains exact foreground guards, complete document comparison, exact disk
bytes and Git oracles. Its bounded journey does not cover scenario steps 7-8,
all recovery cases, CJK/full input conformance or human pilot durations.

The product repair deliberately supports bounded small documents. Complete
large/degraded-document accessibility and native accessibility actions remain
separate requirements; no full-document claim is made for constrained coverage.
