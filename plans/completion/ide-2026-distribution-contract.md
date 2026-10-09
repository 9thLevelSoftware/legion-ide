# IDE 2026 distribution qualification contract

Date: 2026-10-08. Ticket: 235. Inspection baseline: `9a024a4`; integration
`dfcdd42` merged before editing. This bounded configuration contract supplies
M7 / XQ-01 and XQ-07 / A19 prerequisites, not release acceptance or provisioning.
Canonical implementation classifications and all acceptance values remain
unchanged (`unassessed`). Signer absence is preserved: supplementary descriptors
remain `dry-run/no-production-signer`; generated native package metadata remains
`unsigned-beta/no-os-code-signing`. A manifest signer is not an OS package signer.

## Authority and finite package cells

Sources: [approved specification](../../.scratch/ide-2026-planning/spec.md),
[execution authorization](../../.scratch/ide-2026-planning/execution.md),
[reconciliation](ide-2026-reconciliation.md), [distribution scope audit](distribution-scope-audit.md),
[matrix](matrix.json), [requirements](requirements.json),
[release descriptors](../../xtask/release-pipeline.example.toml),
[native workflow](../../.github/workflows/legion-release.yml),
[procurement/escrow policy](../release/procurement-and-key-escrow.md), and
[ADR-0042](../adrs/ADR-0042-auto-update-strategy-and-signed-manifest.md).
The current native workflow and scripts define the following declared package
formats; supported product delivery still requires qualification of each cell.

| Platform / architecture / target | Declared package | Structural verifier and trust qualification | Downstream owners |
| --- | --- | --- | --- |
| Windows x64 / `x86_64-pc-windows-msvc` | MSI (`wix` script format) | PowerShell native verifier: SHA-256, metadata, MSI ProductVersion, administrative extraction, headless smoke. Separately require Authenticode verification and clean-host SmartScreen/install evidence. | 236 Manual artifact; 237–238 installed update/recovery; 239 clean signed install; 240 signed update/offline |
| macOS Intel x64 / `x86_64-apple-darwin` | DMG containing app bundle | Shell native verifier: SHA-256, metadata, DMG verification/mount, bundle/version/architecture and headless smoke. Separately require codesign, notarization/stapling and fresh Gatekeeper evidence. | 241 Manual artifact; 242–243 update/recovery; 244 clean signed install; 245 signed update/offline |
| macOS Apple Silicon arm64 / `aarch64-apple-darwin` | Separate DMG containing arm64 app bundle | Same verifier/trust obligations, executed on the native arm64 host; Intel evidence does not qualify this cell. | 241–245, independently of Intel |
| Linux Ubuntu x64 / `x86_64-unknown-linux-gnu` | DEB | Shell native verifier: SHA-256, metadata, dpkg structure/version/extraction and headless smoke. Separately require approved detached signature/trust-key verification and clean native install evidence. | 246 Manual artifact; 247–248 update/recovery; 249 clean signed install; 250 signed update/offline |
| Linux Ubuntu x64 / `x86_64-unknown-linux-gnu` | AppImage | Shell native verifier: SHA-256, metadata, AppImage extraction/version and headless smoke. Separately require approved detached signature/trust-key verification and clean native launch/install lifecycle evidence. | 246–250, independently of DEB |

The script/verifier interfaces are [PowerShell packaging](../../scripts/package-native.ps1),
[shell packaging](../../scripts/package-native.sh), [PowerShell verification](../../scripts/verify-native-package.ps1)
and [shell verification](../../scripts/verify-native-package.sh). Current packaging
uses cargo-packager; historical cargo-dist/portable ZIP/tar.gz preview descriptions
are not additional current installer cells. RPM and Windows arm64/Linux arm64
are not declared by the current release workflow; this does not remove any
approved platform/architecture obligation. Preserve both native macOS architectures
and every existing canonical configuration; no matrix fields are ratified here.

For each applicable cell retain separate stable/full and preview/staged channel
identities, plus a separately labelled Manual/offline artifact. Normal Manual
mode is not proof of a separate offline package. Tickets 236/241/246 own its
construction/inventory and the downstream OS capture. Exact offline packaging
flavor and identifiers remain prerequisites until those owners record them.

## Clean-host contract

Each qualification run must pin native OS edition/release/build, architecture,
hardware, package format/flavor/channel, capture/verifier versions, host identifier
and reset image or fresh-user provenance. Existing Windows pilot selection
(Windows 11 Pro x64 build 26200, i9-14900HX / 32 GiB) is documented in the
[pilot configuration](ide-2026-pilot-configuration.md); it is not a clean-host
signoff. macOS 15 Intel and Apple Silicon and Ubuntu x64 retain their canonical
cells, with exact builds/hardware and approved host access still prerequisites.

Use fresh native machines/VMs or a documented reset environment; a fresh macOS
user is usable only with evidence that prior machine trust/install state does
not contaminate the run. No inherited checkout, Rust build/dependency cache,
development PATH, installed Legion, Gatekeeper approvals, publisher trust or
prior user settings may substitute for customer installation. Pin necessary OS
runtime dependencies and install them through the declared customer path.
Historical Mac mini access does not establish a current clean Intel/arm64 host.

Tickets 239/244/249 own clean install, real trust prompts/verifier outcomes,
first run, native project open/edit/save, uninstall/repair and interrupted or
failed-install recovery. Tickets 240/245/250 own signed installed update,
replacement/restart acknowledgement, interruption/rollback restoring executable
and workspace state, and offline capture. Bind external OS/disk/process/native
input observations to the tested bytes; extraction or headless smoke alone
cannot qualify these workflows. Preserve workspace proposal and save gates.

Manual/offline capture begins before launch, covers the whole process tree and
local workflows, and checks DNS/TCP/UDP, telemetry, providers, update and crash
egress, including loopback inference. Capture tool/version, privileges and
independent observation are unresolved prerequisites for each host. Unavailable
tools, signers, hosts or observations block that cell rather than produce a pass.

## Signer, feed and approval responsibilities

These are downstream responsibility assignments, not fabricated named operators,
signer identities, services or approval records. Real external support requires
owner authorization before credentials, provisioning, publication or signing.

| Responsibility / canonical anchor | Accountable downstream owner | Explicit prerequisite |
| --- | --- | --- |
| Windows OS signer; `EXT-CERT-WIN`, COMP-DIST-003 | Product/release owner selects and authorizes signer; ticket239 records identity/trust evidence, ticket240 consumes it | Provider choice and eligibility, real publisher identity/certificate or managed signing reference, protected CI access and verifier versions. Azure Trusted Signing is a historical option, not an approved available signer. |
| macOS signer/notarizer; `EXT-CERT-MAC`, COMP-DIST-003 | Product/release owner controls Apple account/certificates; ticket244 records per-architecture trust evidence, ticket245 consumes it | Real Developer ID identity/certificate references, notary credentials, submission/stapling evidence and approved CI access. Historical program membership is not certificate issuance or current notarization. |
| Linux artifact signer; `EXT-CERT-LIN`, COMP-DIST-003 | Product/release owner authorizes key custody/trust distribution; ticket249 records both formats, ticket250 consumes them | Real minisign/Ed25519 key/public identity and approved detached-signature verifier/trust policy for DEB/AppImage. No key is created here. |
| Update manifest signer; COMP-DIST-005, ADR-0042 | Release owner controls signing/rotation/escrow; tickets237/242/247 implement installed consumers and 240/245/250 qualify them | Real Ed25519 public trust anchor, signer reference and protected secret access. Example LEGION_SIGNING_KEY and identity strings are configuration references, not evidence of a production signer. |
| HTTPS feed/channel operation; `EXT-FEED`, COMP-DIST-005 | Product/release owner authorizes operator, host and publication; tickets237/242/247 and 240/245/250 consume/qualify the feed | Approved operator, domain/bucket/URL, access/recovery controls, stable/preview isolation, rollout/cancellation and previous accepted rollback target. Domain/R2 and staging sketches are not provisioned hosts. |
| Clean hosts and immutable evidence; COMP-DIST-007/010 | 239/244/249 install owners and 240/245/250 update/offline owners; 251 coverage, 252 independent audit, 254 final acceptance | Approved fresh native hosts, OS tools, retained observation, independent reviewers and canonical configuration/scenario joins. Final acceptance is owner-controlled. |

Keep private keys, certificates, tokens and notarization credentials outside the
repository; reference approved secret stores only. Retain the existing offline
escrow, recovery and rotation policy. An ephemeral test signature or a signed
manifest alone never satisfies OS signing. Stable/preview feeds use
`release-manifest.v1.toml` plus detached Ed25519 `.sig`, verify signature before
trusting contents, and bind channel/version/rollout, previous accepted version,
artifact identifiers/URLs, SHA-256, timestamp and signer reference. Offline
artifacts must not contact the feed. Missing signatures cannot silently activate
production updates; development/unsigned exercises remain labelled accordingly.

## Immutable candidate and evidence rules

Freeze the exact source commit, lockfile/build tool identities, build options,
OS/architecture/format/flavor/channel/version, artifact inventory and SHA-256 of
the final distributed bytes. Retain release metadata, descriptor/manifest and
signature digests, dependency/SBOM provenance, verifier logs and trust outcomes.
Signing/stapling/repackaging changes bytes: retain the unsigned input and nominate
a distinct final digest; qualification applies to the final signed bytes.

Bind every run to that immutable subject and host/configuration/scenario, with
start/end, commands, exit codes, observations and limitations. Never overwrite a
failed candidate or reuse its identifier for repaired bytes. A repair creates a
new source/artifact/run and retains the failure; unchanged historical substrate
tests are not current release evidence. Channel/rollback pointers may change
only through authorized signed manifests; accepted artifact bytes stay immutable.

The existing internal offline MSI at source
`87580fae51293ca619a71535a184ef1a96a00b9c`, SHA-256
`f73b50214c81bdb1498672268385d9439443bc420e4fb6099901fd4602d07976`,
remains an unsigned pilot candidate with a failed headless smoke recorded in the
execution/pilot documents. Its raw evidence is retained outside the repository
at `D:/legion-ide-2026-tools/pilot-candidate-87580fa/`. Packaging/checksum success
does not establish production signing, clean installation, native-input or release
acceptance. No new candidate is nominated by this contract.

COMP-DIST-001..010 and existing update/crash/privacy/platform anchors are retained.
Downstream owners must reconcile missing configuration/scenario/evidence joins
through the existing canonical registers before qualification; ticket251 checks
coverage and ticket254 owns the final join. This document changes neither those
registers nor the map, execution record or canonical status.

## Planned documentation verification

Run existing xtask `docs-hygiene` and `verify-completion-register --root .`, then
staged `git diff --cached --check`, once each. Use the shared existing xtask binary
with this worktree as root; record actual results in ticket235. These establish
documentation/register structure only. Independent read-only review precedes
commit; runtime, signing, provisioning and platform acceptance are downstream.

Independent review: PASS, no findings, reported by the coordinator on 2026-10-09.
Ticket235 is resolved for this bounded documentation contract only. The planned
documentation/register/whitespace checks passed once as recorded in ticket235;
no checks were repeated. External signing, hosts, feed and release acceptance
remain downstream prerequisites, with canonical acceptance unchanged.
