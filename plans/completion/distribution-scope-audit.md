# S0-01y distribution/operations scope audit

Date: 2026-09-05. Baseline: `4452c97`. This inventory replaces only the
aggregate family-22 row. Product/native/external acceptance remains
`unassessed`; current packaging, dry-run, unsigned-beta, and historical release
records are implementation traces only.

The approved family promise covers signed native packages, a separate
Manual/offline artifact with OS-level no-egress verification, clean
installation, real update replacement/restart/rollback, diagnostics, crash
controls, documentation, and support. The finite Stage-0 matrix must name
supported OS/package/signer/verifier/channel/update-feed/clean-machine cells
and owner-held prerequisites before dependent acceptance. XQ-01 supplies
canonical completion rows; XQ-04 update replacement; XQ-05 crash/privacy and
deletion; XQ-06 Manual/offline capture; and XQ-07 signed installers and clean
machines.

## Atomic inventory

| ID | Atomic outcome | Package | Current trace and limit |
|---|---|---|---|
| `COMP-DIST-001` | Stage 0 publishes the finite OS/package/signer/verifier/channel/feed/offline/clean-machine matrix and owner-held prerequisites. | S0-02 | `absent`: the approved plans define the prerequisite, but no current canonical matrix was found. |
| `COMP-DIST-002` | Release pipeline reproducibly builds declared artifacts and records version/channel/target metadata, hashes, manifest provenance, SBOM/dependency provenance, and verifier results without promoting dry-run or missing artifacts. | XQ-01 | `partial`: descriptor/hash and verification substrate exists; current production artifact/SBOM evidence is not established. |
| `COMP-DIST-003` | Production artifacts carry real Windows/macOS/Linux signer and OS-verifier evidence with protected secret handling, notarization/trust checks, and explicit unsigned-beta or blocked status when prerequisites are unavailable. | XQ-07 | `partial`: signing/status code and operator configuration exist; no production signer credentials or current three-OS signed candidate are evidenced. |
| `COMP-DIST-004` | A separately labeled Manual/offline artifact is externally verified to run local workflows with zero external DNS/TCP/UDP/telemetry/provider/update/crash egress. | XQ-06 | `partial`: packaging and Manual policy substrate exist; OS-level capture of the current packaged artifact is not established. |
| `COMP-DIST-005` | Stable and preview channels publish separate signed or explicitly unsigned descriptors/manifests with visible rollout policy, hashes, feed provenance, cancellation, and no cross-channel leakage. | XQ-04 | `partial`: channel descriptors, manifests, and updater journal substrate exist; production feed and candidate channel acceptance are unassessed. |
| `COMP-DIST-006` | Installed update replacement uses a separately packaged helper with path/hash/signature checks, locking, atomic swap, restart acknowledgement, interruption handling, and rollback restoring executable/workspace state. | XQ-04 | `partial`: updater journal and deterministic drill exist; the packaged handoff/replacement/restart path is not complete. |
| `COMP-DIST-007` | Each supported OS passes clean-machine install/trust/first-run/project/uninstall-repair and interruption or failed-install recovery without inherited checkout/cache state. | XQ-07 | `partial`: historical fresh-VM and verifier records exist; current signed clean-machine qualification is absent. |
| `COMP-DIST-008` | Crash recovery and support diagnostics are opt-in, restart-safe, metadata-only by default, redacted, exportable, and deletable with truthful consent, retention, and deletion receipt. | XQ-05 | `partial`: crash capture, diagnostics, metadata export, and deletion substrate exist; packaged current-candidate qualification is unassessed. |
| `COMP-DIST-009` | User/operator support material matches observed distribution behavior and documents Manual/offline, consent, install/repair/uninstall, update/rollback, crash/support export, diagnostics, recovery, platform caveats, and escalation. | XQ-05 | `partial`: runbook, privacy, troubleshooting, and historical support records exist; current candidate alignment is unqualified. |
| `COMP-DIST-010` | Final distribution qualification exercises packaged install, first-run, project, update, interruption, rollback, crash/privacy, support, uninstall/repair, and Manual/offline journeys with immutable hashes, external OS oracles, reviewer coverage, and blocked prerequisite outcomes. | S6-01 | `absent`: no current complete release EvidenceRun set establishes the full operations workflow. |

## Ownership and rewiring

Distribution rows do not duplicate platform quality. `COMP-PLAT-009` is a
dependency for Manual/offline, clean-machine, and final release qualification;
platform input/accessibility/performance acceptance remains family21. Existing
update contracts remain `COMP-P8-F2-T1-1..T3-1`; crash/privacy contracts remain
`COMP-P8-F3-T1-1..T3-1`. The existing `COMP-SCOPE-GAP-02` protection is retained
on every affected internal row.

Removing family22 required authorized protection rewiring on exactly seven
retained rows. The exact mapping is:

| Retained row | Replacement protection | Rationale |
|---|---|---|
| `COMP-P0-F4-T3-1` | `COMP-DIST-002` | Build/no-default-feature gate protects reproducible artifact construction. |
| `COMP-P0-F4-T4-1` | `COMP-SCOPE-GAP-02`, `COMP-DIST-002` | Per-format validation and publish parsing protect artifact verification. |
| `COMP-P8-F1-T1-1` | `COMP-SCOPE-GAP-02`, `COMP-DIST-003` | Signer configuration and secret exclusion protect production signing. |
| `COMP-P8-F1-T2-1` | `COMP-SCOPE-GAP-02`, `COMP-DIST-002`, `COMP-DIST-003` | Descriptor provenance/hash and signer/verifier status cover artifact and trust outcomes. |
| `COMP-P8-F1-T3-1` | `COMP-SCOPE-GAP-02`, `COMP-DIST-007` | Fresh-VM evidence is owned by clean-machine qualification. |
| `COMP-P8-F1-T4-1` | `COMP-SCOPE-GAP-02`, `COMP-DIST-003`, `COMP-DIST-007` | Unsigned-beta policy is tied to signer status and the machine trust/install boundary. |
| `COMP-P8-F1-T5-1` | `COMP-SCOPE-GAP-02`, `COMP-DIST-003`, `COMP-DIST-005`, `COMP-DIST-007` | External signing/feed/cloud-machine prerequisites span signer, update channel, and clean-machine outcomes. |

No other retained fields changed. All ten new rows use `owner_role:
luna_worker`, `acceptance: unassessed`, literal existing source paths, and
truthful `partial`/`absent` classifications. No product code, Cargo file, GUI
artifact, network action, commit, or unrelated plan file was changed.

## Validation report

The exact UTF-8 Git blob for `plans/completion/requirements.json` at `4452c97`
was decoded and compared object-by-object. Baseline had 409 requirement
objects. Removing family22 and retaining 408 non-family objects, with only the
seven explicitly authorized protection-field exceptions, then adding ten rows
produces 418 total rows. Validation requires unique IDs, zero dangling
`depends_on`/`protected_product_ids` edges, an acyclic graph, and existing
literal source paths for every new row.

Dry-run and unsigned-beta descriptors remain explicitly partial evidence. No
production signer, live credentials, or current three-OS native release proof
is inferred.
