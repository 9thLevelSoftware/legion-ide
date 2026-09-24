# ADR-0055: Verified language artifact materializer

- Status: Proposed
- Date: 2026-09-05
- Owners: app composition / language tooling
- Depends on: ADR-0010 (air gap), ADR-0018 and ADR-0034 (LSP lifecycle),
  existing capability-broker and `legion-lsp` downloaded-artifact contracts
- Supersedes: none

## Context

The language registry already describes downloaded packages (currently the
Node-backed Pyright `.tgz` entry) with a URI, pinned SHA-256, archive format,
package root, entrypoint, Node minimum, and policy-gate label. The app currently
has only a capability decision plus in-memory SHA helper in
`crates/legion-app/src/language/download.rs`; no fetch, archive extraction,
cache publication, cancellation, or runtime probe exists. The LSP crate's
resolver explicitly requires an app-owned materializer and approved Node path.

The product contract is strict: Manual mode is zero-egress. A user gesture
does not create a network exception. A local archive supplied by the user can
be imported offline after the same identity and safety checks. A network source
is a separate, visible capability-gated operation and is unavailable in Manual.

## Decision

`legion-app` owns a `LanguageArtifactMaterializer` composed behind the existing
language workflow. `legion-lsp` remains responsible for descriptor and final
process-config resolution; it does not read the network, extract archives, or
own cache state. UI/desktop emits intents and projects status only.

The materializer accepts an immutable descriptor plus an explicit source:

```text
ArtifactDescriptor {
  artifact_id, version, archive_format, expected_sha256,
  package_root, entrypoint, runtime, policy_gate
}
ArtifactSource = LocalArchive { path } | Network { uri, release_host }
MaterializeRequest { descriptor, source, operation_id, correlation_id }
MaterializeEvent = Queued | Fetching { bytes } | Verifying | Extracting { entries }
                 | ProbingRuntime | Publishing | Ready { cache_root }
                 | Denied | Cancelled | Failed { bounded_code }
```

The concrete Rust names may vary, but the authority and serialized fields are
stable. Events contain bounded metadata only: artifact ID/version, byte and
entry counters, state, error code, correlation/operation IDs, and decision ID;
never archive contents, source text, credentials, HTTP bodies, or raw process
output.

### Source and policy

* `LocalArchive` opens only a caller-selected local file. It requests no
  network capability and is the supported Manual/offline import route.
* `Network` first asks the capability broker for `network.fetch` with the
  descriptor host and HTTPS target. Any broker denial, malformed target,
  missing decision, zero correlation ID, or Manual-mode policy result fails
  closed. The app must not fall back from a denied network source to another
  host or a local cache with mismatched identity.
* Network fetching is behind an explicit optional `tool-downloads` Cargo
  feature for dependency/packaging control, but runtime policy checks remain
  mandatory in every build that contains it. The `offline` feature must never
  silently widen policy or hide a network call.

### Verification and extraction pipeline

The operation is asynchronous, bounded, and cancellable. It streams the source
to an operation-private staging file, hashes as it reads, and compares the
lowercase normalized SHA-256 before opening the archive. It rejects a source
that exceeds any limit or deadline and checks cancellation between reads,
archive entries, file copies, and runtime probing.

The initial archive contract is `tar.gz` only. Limits are descriptor/policy
configuration with conservative defaults and hard upper bounds:

* compressed input: 256 MiB;
* total uncompressed bytes: 1 GiB;
* one extracted regular file: 256 MiB;
* entries: 100,000;
* path depth: 64 components; each component 255 bytes; full relative path
  4,096 bytes;
* wall-clock operation deadline: 10 minutes, with cancellation checked at
  every bounded I/O/extraction step.

### Tar implementation contract and metadata preflight

The cached official crates are `tar 0.4.46` (MIT OR Apache-2.0) and
`flate2 1.1.9` (MIT OR Apache-2.0); both have the expected upstream license
files in the local Cargo registry. `tar 0.4.46` exposes the required second
pass APIs: `tar::Archive::new(reader)`, `Archive::entries()`,
`tar::Entry::{header,path_bytes,path,unpack_in}`, and `Header::entry_type()`.
`flate2::read::GzDecoder<File>` supplies the streaming gzip reader. `tar 0.4.46`
is preferred over cached `binstall-tar 0.4.42`: the former is the official
`tar` package and normal ecosystem API, while the latter is a forked package
name.

The normal `tar::Archive::entries()` iterator is not sufficient as the first
security gate. In `tar 0.4.46`, the iterator internally reads GNU long
name/link and PAX extension bodies with `read_all()`, which allocates a
`Vec` with initial capacity `min(size, 128 KiB)` and then `read_to_end` can
grow it to the entire metadata entry before the caller receives the logical
entry. The implementation must therefore perform a first, raw
streaming preflight over a fresh `GzDecoder` before invoking normal `tar`
iteration:

1. Read exactly 512-byte tar headers through a bounded `Read` wrapper. Verify
   the header checksum and parse the octal/base-256 size with checked
   arithmetic. Reject a header size above the configured entry/file limit or
   an archive whose padded skip would overflow or exceed the total
   uncompressed limit.
2. For extension types GNU `L`/`K` and PAX local `x`, stream their payload
   into a fixed 64 KiB scratch buffer only. Reject metadata payloads larger
   than 64 KiB before allocation/copying; parse PAX records with checked
   record lengths and reject malformed lengths. PAX global `g` is rejected
   explicitly because the pinned `tar 0.4.46` parser does not apply global
   records to subsequent entries; accepting it would create a raw/parser
   interpretation mismatch. This preserves legitimate long names/local PAX
   records while bounding the library's later `read_all()`.
   The effective path limit remains 4,096 bytes, so a valid long-name payload
   is accepted when it fits that limit.
3. Inspect all ordinary headers before skipping their padded payload: reject
   GNU sparse type `S`, contiguous type `7`, link/symlink, character/block
   device, FIFO, socket, and unknown entry types. Also reject PAX keys
   `GNU.sparse.*`, `SCHILY.xattr.*`/other filesystem-materializing metadata,
   and linkpath values; PAX `path` is accepted only after the same UTF-8,
   separator, and Windows containment checks. Track duplicate names and
   conflicting directory/file kinds in a bounded set.
4. Enforce the same cancellation/deadline and compressed/uncompressed,
   entry-count, path-depth, component-length, and per-file limits while the
   preflight skips bytes. When local PAX supplies `size`, use that checked
   effective size for aggregate/per-file limits and padded payload skipping,
   matching `Entry::size()` in the maintained parser. Require two consecutive zero blocks and reject any
   non-zero trailing bytes according to the chosen tar end-of-archive policy.
5. Rewind/reopen the immutable SHA-verified staged archive and run the second
   pass with `tar::Archive::entries()`. Before each `Entry::unpack_in`, repeat
   the effective path/type/size checks from the preflight, use a destination
   path opened/created only beneath the private root, and copy regular file
   bytes through a counting/cancellation reader. Do not call
   `Archive::unpack` or `Entry::unpack` wholesale because those APIs apply
   filesystem behavior before the app's per-entry policy can complete.

This two-pass contract is required even for trusted catalog archives: the
first pass bounds GNU/PAX metadata allocation, and the second pass delegates
format details to the maintained parser without rejecting valid long-name
metadata. The preflight scanner is a small app-owned tar-header reader, not a
second archive extractor; it never creates filesystem entries.

### Pyright archive evidence

The retained fixture is
`.superpowers/sdd/2026-09-04-full-product-completion/pyright-1.1.400.tgz`.
Its SHA-256 recomputes to
`2ccba7af9c8b14bb81c8fa9bb558d8b5181b586ec4dfc448b78eb4209e7a429a`, matching
the registry descriptor. The sibling metadata records npm's `fileCount: 4626`
and `unpackedSize: 16302649`; the raw gzip/tar inventory independently found
4,626 entries and 16,302,649 total regular-file bytes.

The raw 512-byte header scan found typeflag `0` for all 4,626 members, no GNU
long-name/link or PAX `x`/`g` extension headers, no sparse/link/device/FIFO
entries, and no trailing extension metadata. The longest effective member path
from the tar listing is 121 UTF-8 bytes; the longest raw header name is 99
bytes (the ordinary ustar name field), so this fixture does not exercise PAX
long-path handling. The materialized sibling cache contains the expected
`package` root and `package/langserver.index.js` entrypoint. These facts ground
the current limits without weakening the preflight: future catalog archives
may use valid GNU/PAX long names and will be accepted when their metadata is
within the 64 KiB cap and effective path policy.

Before creating each destination, the extractor validates the tar path as a
relative path with no root/prefix, `.` or `..` component, empty name, or
platform separator ambiguity. The same checks apply on Windows, including
drive prefixes, UNC/device prefixes, alternate separators, reserved device
names, and paths whose normalized form escapes the staging root. Each resolved
destination must remain beneath the canonical staging root.

Only regular files and directories are accepted. Symlinks, hard links, device
nodes, FIFOs, sockets, PAX link targets, duplicate/conflicting entries, and
metadata that requests any other filesystem kind are rejected. File and
directory creation uses restrictive permissions where supported. No archive
entry may overwrite an earlier entry.

After extraction, the materializer verifies that `package_root` is a directory
and `entrypoint` is a regular file below it, both using canonical containment
checks. It rechecks entry count/bytes and the descriptor identity before
publication.

### Node runtime and future Python runtime

For a Node descriptor, app composition resolves an approved local Node
executable, invokes it as a child process with exactly `--version`, and
captures only a bounded, redacted version line. The process has a short probe
deadline and is cancellable/killable. The observed version is passed to
`LanguageServerAdapterPlan::resolve_downloaded_process`, which performs the
existing exact parse, minimum-version, absolute-path, canonical containment,
and shell-free process-config checks.

Python startup is a later descriptor/runtime increment. This ADR does not mark
Python complete, does not infer a Python interpreter from Node metadata, and
does not permit arbitrary package install or execution. A future Python
descriptor must add an explicit interpreter approval/version probe and retain
the same source, extraction, cache, and policy contracts.

### Cache publication and tamper handling

The cache is app-owned and content-addressed by artifact identity, at minimum
`sha256/<expected_sha256>/`. Each operation extracts into a unique temporary
directory below the cache staging root. Publication is atomic: write and flush
the metadata sidecar only after all files validate, then rename the complete
directory into its final identity. A ready marker/sidecar is the final write;
partially staged directories are never cache entries.

The sidecar records schema version, artifact ID/version, archive format,
expected SHA-256, package root, entrypoint, runtime descriptor, byte/entry
counts, and materializer implementation version. It contains no raw archive or
process output. On a repeat request, the sidecar and expected descriptor must
match exactly; required files are rechecked for regular kind and containment,
and the materialized identity is revalidated before reuse. A missing, malformed,
mismatched, incomplete, or tampered entry is quarantined/deleted within the
cache namespace and rematerialized. A collision at an occupied identity is
never overwritten: identical valid identity is reused, while differing bytes
or metadata fail closed and receive a bounded collision result.

Cancellation or failure removes only the current operation's temporary files.
The previous valid cache entry remains available and is never replaced by a
partial result. Cache cleanup is bounded and cannot traverse outside the
app-owned cache root.

### Lifecycle and observability

The materializer runs on an app background worker with bounded channels. The
frame path only drains events. Cancellation is idempotent; the terminal event
is emitted once, and child processes are killed before staging cleanup. A
materializer operation may not launch an LSP process until publication and
runtime approval have succeeded. The resulting LSP health record carries the
download provenance, artifact hash, observed version, and capability decision
ID, while diagnostics remain metadata-only.

## Dependency decision

The workspace lock already contains `reqwest 0.13.1` and `flate2 1.1.9`.
`tar` is not currently locked. Later network support uses the workspace
`reqwest` declaration with rustls-only features and an optional app
`tool-downloads` feature. The optional feature must be absent from the
`--no-default-features --features offline` Manual package, preserving a build
without the HTTP dependency. Runtime policy remains enforced in builds that do
include it.

Bounded `tar.gz` extraction requires an explicit `tar` dependency plus the
locked `flate2` path (or an equivalent reviewed parser). This is a new direct
dependency edge and requires the dependency-policy update, cargo-deny/license
review, and contract tests in the same implementation change. No existing
provider, remote, updater, or terminal HTTP client is reused: those clients
own unrelated authorities and would hide the source/policy distinction.

## Consequences

This keeps Manual zero-egress mathematically visible: only `LocalArchive` can
materialize in Manual, and no app feature can turn a denied network request
into a fetch. It makes cache reuse safe across restarts and hostile archives
fail closed before any LSP process launch. The cost is a small app-owned
workflow, archive-parser dependencies, explicit feature/dependency review,
and later work for network fetching, Node probing, and Python startup.

## Rejected alternatives

* Reusing `legion-ai-providers`, `legion-remote`, or updater HTTP clients would
  couple language provisioning to another authority and obscure audit scope.
* Shelling out to platform `tar` would make path/device/link limits and
  cancellation platform-dependent and untestable.
* Treating a human click as Manual network consent contradicts the existing
  mode and protocol contracts.
* Extracting directly into the final cache or launching before revalidation
  permits partial/tampered state and is forbidden.

## Acceptance gates for implementation

The implementation may proceed in ordered increments: (1) local import,
bounded extraction, atomic cache, and tamper/cancellation tests; (2) the
optional policy-gated network source and HTTP streaming; (3) approved Node
probe and LSP health provenance; (4) a separately reviewed Python runtime
descriptor/startup path. Each increment needs app/LSP contract tests and must
preserve the Manual denial tests. No increment may claim Python completion
before its own native workflow evidence exists.

## 2026-09-06 implementation status note

The materializer contract remains the canonical home for any TypeScript or
JavaScript bundle identity, pinned digest, approved Node runtime, bounded
extraction, cancellation, and Manual zero-egress decision. Current native
TypeScript startup evidence is the
`explicit_typescript_startup_is_lazy_live_and_restart_preserves_dirty_text`
test (`LEGION_TEST_NODE_RUNTIME=<approved-local-node> cargo test -p legion-app
--test typescript_app_startup -- --ignored --nocapture`, 1 passed in 16.23s).
The run requires the retained pinned TypeScript archives and an explicit local
Node runtime. No registry, materializer, or unit-test result is sufficient to
claim packaged language completion; the S2 native EvidenceRun requirements
remain in force.
