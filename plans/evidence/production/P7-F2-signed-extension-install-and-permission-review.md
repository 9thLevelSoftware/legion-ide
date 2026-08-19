# P7.F2 — Signed extension install UI, itemised permission review, tamper refusal

Covers `P7.F2.T1`, `P7.F2.T2`, `P7.F2.T3` in `plans/kanban/legion-ga-backlog.toml`.
Readiness row: `PR-VSC-001`.

## The defect found on the way in

`crates/legion-plugin/src/manifest.rs` existed, was 147 lines, and had a passing
test — except `crates/legion-plugin/src/lib.rs` declared only `pub mod host;`
and `pub mod registry;`. The file was never compiled. It was dead code and its
test had never run. It is now declared and compiled, and its contents were
rewritten into the typed review this feature needs.

The second thing worth stating plainly: the previous
`SignedExtensionRegistry::validate_installable` checked
`manifest.signature.is_none()`. That is a presence check, not a signature check.
An artifact carrying `Some(PluginSignatureMetadata { .. })` with arbitrary
strings passed. Nothing was ever verified.

## P7.F2.T1 — install / update / remove for signed artifacts

Acceptance: *User can install a safe bundled grammar/theme extension through
product UI.*
Stop condition: *Stop if an unsigned artifact is allowed to install by default.*

### Verification

`SignedExtensionRegistry` now takes a `SignedExtensionArtifact` — the manifest,
the bytes that would be executed, and a detached signature — and applies six
ordered, fail-closed guards in `verify`:

| # | Guard | Error |
|---|-------|-------|
| 1 | no signature at all | `UnsignedArtifact` |
| 2 | algorithm is not `ed25519` | `UnsupportedAlgorithm` |
| 3 | signer has no trust anchor | `UnknownSigner` |
| 4 | bytes do not hash to `manifest.module_hash` | `ArtifactChecksumMismatch` |
| 5 | signature does not verify over the canonical payload | `SignatureVerificationFailed` |
| 6 | local trust metadata forbids activation | `UntrustedArtifact` |

Guard 4 catches a swapped payload whose manifest was not updated. Guard 5
catches a swapped payload whose manifest *was* updated to match — the realistic
tamper, and the one only a signature can refuse.

An `ExtensionKeyring` with no anchors denies every signer. There is no
"unconfigured, therefore unrestricted" branch.

### One signing scheme, not two

Verification calls `legion_security::verify_ed25519_signature`, the workspace's
single ADR-0042 primitive. Signing needed the same discipline, so
`sign_ed25519_detached` and `ed25519_verifying_key` were factored out of
`sign_policy_bundle` and exported from `legion-security`. `legion-plugin` does
not depend on `ed25519-dalek`; it gained only `sha2` (to bind the manifest to
the bytes) and `base64` (to encode anchors and signatures the way
`legion-security` already encodes policy-bundle ones).

### The path a real user takes

The panel is not decoration. Every control carries a prebuilt action:

```
Settings > Extensions button
  -> DesktopAction::{SetExtensionPermission,InstallExtension,UpdateExtension,RemoveExtension}
  -> DesktopCommandBridge::translate  (validates against the projection)
  -> CommandDispatchIntent::{SetExtensionPermission,InstallExtension,...}
  -> CommandDispatcher::route_intent -> AppCommandRequest::ExtensionCatalog
  -> AppComposition::apply_extension_request
  -> extension_management::ExtensionCatalog::apply
  -> legion_plugin::SignedExtensionRegistry::{install,update,remove}
```

`crates/legion-desktop/tests/extensions_panel.rs::granting_each_permission_then_clicking_install_really_installs`
drives that whole chain through a real `DesktopRuntime`: it reads the actions
off the panel model, feeds them to `handle_action`, and asserts the projection
afterwards reports `ExtensionInstallState::Installed`. Nothing is stubbed.

The bundled extension is a first-party tree-sitter grammar
(`crates/legion-app/assets/extensions/legion-json-grammar/grammar.json`,
manifest id `legion.bundled.json-grammar`). It ships offered and *not*
pre-installed.

### Stop condition honoured

An unsigned artifact is refused at three independent layers, and the middle two
are what make it unreachable rather than merely refused:

* `legion-protocol`: `ExtensionSignatureState::is_installable()` is false for
  `Unsigned`, so `ExtensionCatalogEntry::can_install()` is false **regardless of
  how many permissions the user granted**.
* `legion-desktop`: the panel builds no install control at all for such an
  entry (`install_action: None`), and the bridge refuses a forged gesture with
  `ExtensionOperationUnavailable`.
* `legion-plugin`: `verify` returns `UnsignedArtifact` before any capability is
  granted.

Tests: `an_unsigned_entry_can_never_be_installed` (protocol),
`an_unsigned_entry_is_given_no_install_control` (desktop),
`an_unsigned_candidate_is_never_installable_however_many_permissions_are_granted`
(app), `signed_extension_registry_rejects_unsigned_artifacts_by_default`
(plugin).

## P7.F2.T2 — manifest permission review

Acceptance: *Each install prompts the user with a structured permission list.*
Stop condition: *Stop if permissions are summarized in a single 'trust this
extension' toggle.*

The stop condition is honoured structurally, not by convention. There is no API,
intent, action, or DTO field anywhere in the chain that can grant more than one
capability:

* `ExtensionPermissionReview` holds one `ExtensionPermissionDecision` per
  requested capability and starts fully `Undecided`. Its only mutators are
  `decide(capability, ..)` and `decide_at(index, ..)` — both single-row.
* `approval()` refuses on manifest mismatch, an unreviewed capability, any
  undecided row, and any denied row.
* `ExtensionInstallApproval` is constructible **only** through `approval()`, and
  `SignedExtensionRegistry::install` demands one. An install therefore cannot
  occur without a completed per-capability review.
* `CommandDispatchIntent::SetExtensionPermission` and
  `DesktopAction::SetExtensionPermission` each carry exactly one `CapabilityId`.
* The panel emits one Allow/Deny pair per capability row.

`permission_review_is_itemised_not_a_single_trust_toggle` asserts directly that
granting one capability leaves the others undecided.
`each_permission_control_carries_exactly_one_capability` asserts the same at the
renderer/bridge boundary.

Rows are typed (`ExtensionPermissionRow` with ordinal, capability, title,
reason derived from declared contributions, and a risk classification) rather
than the pre-existing `Vec<String>`.

## P7.F2.T3 — tampered artifact rejection

Acceptance: *A tampered artifact is refused before any code runs.*
Stop condition: *Stop if a tampered artifact is allowed to load with a warning
instead of a refusal.*

The pre-existing single test built a manifest that set its own
`trust.decision = ChecksumMismatch` and then asserted it was rejected. A real
attacker does not label their own artifact as tampered, so that test proved
almost nothing.

`crates/legion-plugin/tests/tampered.rs` is now seven tests over **genuinely
valid, instantiable wasm modules**:

* `honest_artifact_actually_loads_in_the_host` — the control. The untampered
  artifact installs *and* `WasmPluginHost::load_fixture` really loads it. Without
  this, every refusal below could be failing for an unrelated reason.
* `swapped_payload_with_stale_manifest_is_refused` — trips the digest guard only.
* `swapped_payload_with_recomputed_hash_is_refused_by_the_signature` — the
  attacker recomputes the checksum so it is internally consistent; only the
  signature can refuse it.
* `artifact_resigned_by_a_different_key_is_refused` — internally perfect,
  wrong key.
* `artifact_from_a_signer_with_no_trust_anchor_is_refused` — a signer the
  keyring has never heard of, refused as `UnknownSigner` rather than as a
  signature mismatch.
* `capability_escalation_after_signing_is_refused` — the capability list is
  inside the signed payload.
* `a_tampered_artifact_never_reaches_execution_in_either_layer` — the malicious
  module is written to a real file on disk, and both the registry and the host
  refuse it.

"Before any code runs" is structural: `verify` performs byte inspection only.
It opens no file, compiles no module, and instantiates nothing. Every outcome is
`Err`; there is no variant that returns `Ok` with a warning attached.

## Mutation testing

Every guard was removed or inverted, the matching test run, and the source
restored. `git status` was clean afterwards and the full suite re-run green.

| # | Mutation | Result |
|---|----------|--------|
| M1 | registry: unsigned artifacts fall through to a synthesized signature | KILLED |
| M2 | registry: accept any signature algorithm | KILLED |
| M3 | registry: unknown signer verifies instead of being refused | KILLED |
| M4 | registry: artifact digest not compared to the manifest | KILLED |
| M5 | registry: Ed25519 verification result discarded | KILLED (see note) |
| M6 | registry: local trust posture no longer gates a valid signature | KILLED |
| M7 | manifest: an undecided permission counts as granted | KILLED |
| M8 | manifest: a denied permission counts as granted | KILLED |
| M9 | protocol: every signature posture counts as installable | KILLED |
| M10 | protocol: permissions count as granted regardless of state | KILLED |
| M11 | bridge: lifecycle gesture translated without checking the projection | KILLED |
| M12 | panel: an install control is built for every entry | KILLED |

**Note on M5, and why it is worth writing down.** The first attempt at M5 wrote
`let _mutant = verify_ed25519_signature(..).map_err(..)?;` and the test still
passed. That looked like a masked guard. It was not — the mutation was
ineffective: the trailing `?` survived the edit, so the error still propagated
and the code was behaviourally unchanged. Replacing the whole statement with
`let _mutant_ignored = verify_ed25519_signature(..);` killed four tampered tests
and one registry test. The lesson is that a surviving mutant needs the mutation
itself checked before the guard is blamed.

M1 is a genuine partial-masking case and is reported as such: with guard 1
removed, guard 5 also rejects the artifact, because a synthesized signature does
not verify. The test still fails, because it asserts the exact error variant
`UnsignedArtifact` rather than merely "an error". The kill is real but it is a
variant-level kill, not a behaviour-level one.

## Verification commands

```
cargo test -p legion-plugin -p legion-desktop -j 6
cargo test -p legion-app --lib extension_management -j 6
cargo test -p legion-protocol --lib extensions -j 6
cargo test -p legion-desktop --test extensions_panel -j 6
cargo test -p legion-plugin --test tampered -j 6
cargo clippy --workspace --all-targets -j 6 -- -D warnings
cargo fmt --all
cargo run -q -p xtask -- check-deps
cargo run -q -p xtask -- docs-hygiene
cargo run -q -p xtask -- claim-audit
cargo run -q -p xtask -- extract-before-modify
cargo run -q -p xtask -- intent-reachability
cargo run -q -p xtask -- verify-kanban-backlog
cargo run -q -p xtask -- verify-readiness-consistency
```

All pass. `legion-plugin` went from 14 to 29 unit tests and from 1 to 7 tampered
tests.

## Snapshot impact

None. `cargo test -p legion-desktop --test shell_snapshots` passes against the
committed baselines untouched, and no baseline was regenerated. The four
snapshotted states (empty shell, explorer, open file, unsaved-changes prompt) do
not open the settings overlay, so the new Settings > Extensions section paints
no pixels in any of them.

`crates/legion-desktop/src/view.rs` grew 15 lines against its 120-line
`extract-before-modify` slack; the panel itself is a new module.

## Corrections made under review

Three of the eight review findings on the PR were defects rather than tidying,
and the claims this document made before them were wrong:

**The signing payload was not length-prefixed.** It was newline-terminated while
claiming otherwise, which is forgeable: a field value containing a newline can
reproduce a different manifest's byte sequence and reuse its signature.
`extension_signing_payload` now emits `<name-len>:<name><value-len>:<value>`
per field, so no value can be mistaken for a field boundary.

**The permission review mis-handled a repeated capability.** Rows were emitted
one per requested-capability *entry*, and `index_of` resolves a capability to
the first matching row. A manifest listing a capability twice therefore produced
a review that could not be completed — the second row was undecidable — while
`approval()` succeeded anyway, and a denial recorded on the duplicate row was
discarded. Rows and grants are deduplicated at the source now.

The test written for that took two attempts, which is worth recording: the first
draft denied *by capability* and passed with the dedup mutated out, because
`decide` reached the first row either way. It documented the outcome without
guarding it. The version that ships denies by row index — the way a UI decides —
and dies when the dedup is removed.

**One tamper test was named for a guard it did not exercise.**
`artifact_resigned_by_an_untrusted_key_is_refused` kept
`signer = "legion-first-party"`, which *is* anchored, so it tested the signature
check with the wrong key. The unknown-signer guard had no test at all. Both now
exist under accurate names.

Also from that round, with no claim in this document to correct: the extensions
panel projected no accessibility node, so the one surface that asks a user to
grant capabilities to third-party code was silent to a screen reader; it now
announces its entry count and pending-review count.

## Known limitation

`DEVELOPMENT_SIGNING_SEED` in `crates/legion-app/src/extension_management.rs` is
a committed development signing seed, so the bundled artifact's signature is
reproducible from source and reviewable in a diff. A developer build's
first-party trust anchor can therefore be forged locally.

This does not weaken any refusal above — unsigned and tampered artifacts are
refused identically either way, and every mutation above was killed with the seed
in place. But GA must replace it with a committed detached signature produced by
the release signing infrastructure (`xtask::signing`, ADR-0042) and delete the
seed. The change is confined to `BundledExtension::signature_b64`, which returns
a `String`: feed it from a constant instead of from `sign_extension_artifact`.
No other code moves.
