# S0-01w training/telemetry scope audit

Date: 2026-09-05. Baseline: `48ec816`. This inventory replaces only the
aggregate family-20 row. Product/native/external acceptance remains
`unassessed`; current substrate is recorded as implementation evidence only.

The approved family promise is explicit opt-in capture, redaction, export,
deletion, retention, and planned feedback mechanisms while Manual privacy
remains intact. The finite Stage-0 matrix rule requires supported consent,
retention/export, model/runtime, reproducibility, and privacy configurations to
be selected before dependent acceptance. The S5-12 plan adds separate crash,
product, and raw-training consent, preview/redaction, local spool, upload,
revocation, export, deletion, retention, provenance, candidate feedback, and
Manual/offline no-egress. S5-13 adds a consented corpus, hash-bound real
adapter run, held-out evaluation, Legion-Bench comparison, repeatability, and
independent review. Production qualification adds EvidenceRun records and
external network/deletion oracles.

## Atomic inventory

| ID | Atomic outcome | Package | Current trace and limit |
|---|---|---|---|
| `COMP-TRAIN-001` | Stage 0 publishes the finite telemetry/training matrix for consent, retention/export, model/runtime/hardware, reproducibility, and Manual/offline privacy. | S0-02 | `absent`: the approved plans define the prerequisite, but no current canonical matrix was found. |
| `COMP-TRAIN-002` | Separate crash, product, and raw-training consent is default-off, purpose/retention/destination-visible, enterprise-ceiling-aware, and revocable before future capture/upload. | S5-12 | `partial`: telemetry and first-run primitives exist; the planned app-owned training consent service and native controls are absent. |
| `COMP-TRAIN-003` | Consent preview binds exact scope, purpose, retention, destination, provenance, and redaction; metadata remains the default and raw training requires its own active consent. | S5-12 | `partial`: `legion-observability::training`, retention training, and authority rules enforce metadata/consent boundaries; complete product preview/upload equality is unassessed. |
| `COMP-TRAIN-004` | Consent-gated data uses a recoverable local spool and explicit upload route with retry, revocation/expiry, corrupt-spool recovery, and no pre-consent upload. | S5-12 | `partial`: telemetry spool/exporter and retention primitives exist; the planned end-to-end service and native recovery journey are not evidenced. |
| `COMP-TRAIN-005` | Authorized export and deletion enforce retention, remove ciphertext/keys, expose handles, emit tombstones/receipts, and recover truthfully after restart. | S5-12 | `partial`: retention training and privacy-deletion tests provide substrate; packaged consented telemetry/training deletion is unassessed. |
| `COMP-TRAIN-006` | Packaged Manual/offline operation proves no telemetry/provider/update/crash/training egress regardless of stored consent through OS-level external capture; Manual never uploads or falls back. | S5-12 | `partial`: Manual policy tests exist; the production-qualification plan still requires signed packaged artifact and OS-level capture. Stored consent cannot relax Manual's zero-egress rule. |
| `COMP-TRAIN-007` | Current corpus export filters non-consented traces, enforces metadata-only redaction and secret scanning, binds fingerprints, and rejects tampering at the trainer boundary. | S5-13 | `partial`: `xtask::training_corpus` and observability training re-check consent and redaction, but no dedicated secret-scanner gate was found and current fixtures are not a current product corpus. |
| `COMP-TRAIN-008` | A real consent-gated adapter run records pinned code/corpus/consent/model/runtime/seed/hyperparameters/resources/artifact hash, held-out comparison, Legion-Bench comparison, repeatability, and review. | S5-13 | `partial`: Python trainer/evaluator and historical evidence exist; the current real run remains unexecuted, and existing manifests do not yet establish the full S5-13 candidate SHA, runtime/GPU, complete hyperparameter/resource, adapter-hash, paired-evaluation, and reviewer provenance bundle. Dry-run or fixture output cannot satisfy it. |
| `COMP-TRAIN-009` | Packaged Stage-5 qualification exercises consent, preview, redaction, upload, revocation, export, deletion, retention, Manual/offline, corpus, training, evaluation, restart, and failure recovery across the ratified matrix with external oracles. | S5-15 | `absent`: no current family-specific packaged EvidenceRun set establishes the complete workflow. |
| `COMP-TRAIN-010` | S5-12 candidate feedback lets an authorized user review, accept, reject, correct, withdraw, or delete a training candidate while preserving consent, provenance, redaction, retention, and downstream eligibility. | S5-12 | `absent`: candidate DTOs and corpus filtering exist, but no product feedback workflow or native controls were found. |

## Ownership and deduplication

The retained `COMP-P9-F4-T1-1`, `COMP-P9-F4-T2-1`, and `COMP-P9-F4-T3-1`
rows remain the narrow historical training-flywheel owners. The new rows depend
on them where applicable and expand current-candidate consent/export and
qualification scope without copying their titles. Enterprise policy and
retention remain owned by `COMP-ENT-003` and `COMP-ENT-004`; context provenance
by `COMP-CTX-005`; audit and egress by `COMP-TRUST-004` and `COMP-TRUST-005`;
metadata telemetry by `COMP-P4-F3-T4-1`; and metadata-only export by
`COMP-P8-F3-T3-1`. No existing `depends_on` or `protected_product_ids` entry
referenced `COMP-SCOPE-FAMILY-20`, so no authorized rewiring was needed.

All ten new rows use `owner_role: luna_worker`, `acceptance: unassessed`,
literal existing source paths, and truthful `partial`/`absent` classifications.
No product code, Cargo file, GUI artifact, network action, commit, or unrelated
plan file was changed.

## Validation report

The exact UTF-8 Git blob for `plans/completion/requirements.json` at `48ec816`
was decoded and compared object-by-object. Baseline had 392 objects; the
family replacement retained 391 objects byte-decoded equal to baseline and
added ten rows, producing 401 total rows. Validation results:

- unique requirement IDs: pass;
- `depends_on` and `protected_product_ids` edges: pass, zero dangling targets;
- dependency/protection graph: pass, acyclic;
- source existence for all new rows: pass;
- all new rows: product, required, `acceptance: unassessed`, `owner_role: luna_worker`.

The exact machine-readable run report is
`.superpowers/sdd/2026-09-04-full-product-completion/s0-01w-report.md`.
