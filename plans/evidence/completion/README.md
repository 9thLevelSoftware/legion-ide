# Completion evidence layout

The completion evidence checkout is quiescent while validation runs. The
canonical root is `plans/evidence/completion/`. Each evidence run is a
single portable directory whose name exactly equals the run identifier and
which contains `run.json`. Directories without `run.json` are support or
incomplete directories and are ignored as evidence. Run records are parsed
strictly before candidate selection, so malformed historical records cannot be
hidden by selecting a newer candidate.

`run.json` is an `EvidenceRun` record. Its `artifact_hashes` map declares every
attachment that the run owns, including attachments not referenced by an oracle
or recovery check. Paths use forward slashes and remain beneath the canonical
evidence root. The validator canonicalizes paths before reading and rejects
direct symlinked run/receipt paths, traversal, escapes, missing files,
directories, and hash mismatches.

The selected candidate is the explicit candidate argument and must match the
candidate manifest. Runs for older candidates remain historical records and do
not get compared with the selected manifest or included in selected-candidate
metadata coverage. Evidence-only commits therefore preserve the candidate
identity while retaining their own repository provenance in build receipts.

Passed product runs require `identity.json` containing a `RunningIdentityReceipt`.
The receipt identifies the component and matrix configuration. That pair must
resolve to exactly one nominated candidate artifact. The artifact's
`build_provenance_path` points to a contained `BuildProvenanceReceipt`, which
is read only after its declared attachment has passed path and hash validation.
The existing identity validator compares candidate, run, build, and observed
artifact identity fields. A conventional `identity.json` that is present or
declared on another run is also parsed and validated rather than ignored.

Build provenance receipts describe the build observer, verifier revision,
tool versions, build command, candidate artifact hash, and a separate build
capture attachment. Runtime receipts describe the independently observed
process or service identity and a separate runtime capture attachment. These
receipts are metadata consistency records; they do not authenticate the human
observer or prove that an external installation was honestly observed.

`CandidateArtifact.path` may describe an installed runtime artifact outside the
repository. The evidence loader never reads or hashes that external path.
Runtime artifact identity is represented by the contained receipt and its
hashed capture files. Installer packages, runtime binaries, build receipts,
and observer captures are separate artifacts with separate declared hashes.

The loader's integration fixtures are synthetic `TESTDATA` only. They are
never nominated evidence and cannot establish product acceptance. A missing
evidence root is treated as an empty development runset; it does not satisfy
release completeness. The loader is composed by the `verify-completion` CLI;
missing canonical registers still fail closed, and an empty evidence root
cannot satisfy release completeness.

Run completion verification with an explicit nomination:

```text
cargo run -p xtask -- verify-completion --candidate <40-lowercase-hex-SHA>
cargo run -p xtask -- verify-completion --candidate <40-lowercase-hex-SHA> --release
```

Development mode permits incomplete, unaccepted requirements while retaining
structural, candidate, evidence, and false-acceptance checks. Release mode
adds required product/configuration coverage, accepted internal safety
evidence, candidate-pinned defect verification, and release-blocking defect
checks. The command prints implementation and acceptance status counts
separately and does not calculate a blended percentage.

For each declared external prerequisite, release verification requires a
selected-candidate passed run for an applicable required package/configuration
pair with an `EvidenceRun.dependencies` observation whose name exactly equals
the prerequisite, whose execution is `real`, whose `required_for_outcome` is
true, and whose observed version is nonblank. Product-linked pairs use
product/native-input eligibility; internal-linked pairs use reviewed
component/integrated safety evidence. This is a typed consistency join and
does not authenticate an observer or an external installation.
Canonical containment is checked before reads, and direct symlinked run
directories, run records, and declared receipt attachments are rejected where
the host can observe them; this remains a path-integrity check rather than an
authenticity claim.
