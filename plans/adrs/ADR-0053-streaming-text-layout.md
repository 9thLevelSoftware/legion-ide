# ADR-0053: Same-version vendored epaint for streaming text layout

## Status

Accepted architecture and local source integration verified — 2026-09-05 by
the root coordinator under the authorized full-product plan. This decision
records the source and ownership contract for the S1-04h continuation and its
bounded local verification. It makes no production, full-wrapped, or product
qualification claim.

## Context

S1-04h supplies bounded shaped-row facts to editor-owned vertical caret
movement. The continuation needs an epaint API that is present in the local
vendor source but is not available through the public registry package at the
required continuation surface. The workspace already resolves `epaint
0.34.2` transitively through the `egui 0.34.2` renderer graph, with lock
checksum
`92b452e348c2758115288802ca25f86ee286ce2cfae6643711ce116662311310`.

The repository contains `vendor/epaint/` as the continuation source. The root
workspace excludes that nested manifest and applies the reviewed same-version
patch below. `vendor/epaint/LEGION_PROVENANCE.md` records that it began from the
crates.io `epaint-0.34.2.crate` archive and identifies the upstream egui
0.34.2 source. The local registry archive has the same SHA-256 as the lock
checksum. The source is not a workspace member; local activation verification
is complete as recorded below.

## Decision

1. Keep the existing public API route through `egui::epaint`; do not add a
   direct `epaint` dependency to any workspace crate. This keeps the editor,
   UI, protocol, and app dependency graph unchanged.
2. The root coordinator has authorized one exact same-version workspace patch:

   ```toml
   [patch.crates-io]
   epaint = { path = "vendor/epaint" }
   ```

   The patch is global because Cargo patches package resolution globally. It
   is permitted only for the existing `epaint 0.34.2` package and must not be
   used to add a new editor-to-renderer or UI-to-renderer edge. The patch must
   preserve the package name/version and all upstream dependency declarations.
3. Renderer ownership remains exclusively in `legion-desktop`. Layout facts
   cross the existing protocol/app route; editor state remains authoritative
   and does not import epaint. The patch changes package source provenance,
   not authority ownership.
4. `vendor/epaint` remains outside the workspace member list. If future Cargo
   discovery rules make the nested manifest discoverable, add an explicit
   `workspace.exclude = ["vendor/epaint"]`; never add it to `members`.
5. The vendor retains the canonical upstream license texts. The exact 0.34.2
   tag sources are:

   - `https://raw.githubusercontent.com/emilk/egui/0.34.2/LICENSE-MIT`
     (UTF-8, 1,092 bytes, SHA-256
     `95ca92f5f8ea5231f1580b3a2a799e8260af3114b900e1def5355a7f44bcf60c`).
   - `https://raw.githubusercontent.com/emilk/egui/0.34.2/LICENSE-APACHE`
     (UTF-8, 10,850 bytes, SHA-256
     `8173d5c29b4f956d532781d2b86e4e30f83e6b7878dce18c919451d6ba707c90`).

   These paths and bytes are provenance evidence for the vendor owner; this
   ADR does not edit `vendor/epaint`.

## Required verification checks

Local verification completed against the 0.34.2 archive: the aligned standalone
vendor test passed (40 passed, 0 failed); root desktop targeted tests passed;
dependency, documentation-hygiene, and claim-audit gates passed; and
`cargo deny check` passed advisories, bans, licenses, and sources. Evidence is
retained in the S1-04h plan scratch logs and the vendor provenance record.

The serialized verifier reviewed the vendor diff against the 0.34.2 archive,
verified the package and dependency versions, retained
the upstream license and attribution files, and record the resulting lockfile
source change. The existing `deny.toml` allow-list covers MIT, Apache-2.0,
OFL-1.1 and Ubuntu-font-1.0; no broad license or source exemption is
authorized.

`xtask check-deps` must continue to reject renderer dependencies outside
`legion-desktop`. A separate negative fixture or gate change may be proposed
if needed to make the external `epaint` ownership rule executable; this ADR
does not authorize weakening the current internal-edge or renderer-boundary
checks.

## 2026-09-06 implementation status note

The current renderer continuation adds bounded metric/replay progress and
preserves final-row/backpressure state across calls. Normal rendering owns a
persistent bounded UI source lease; the worker pool is integrated into that
workflow with checked lease/font keys and UI-only atlas replay. Terminal worker
failure does not spin, and queued typing remains preserved. Font layout identity
and atlas reset identity participate in cache reuse, including the unique-atlas
epoch regression coverage. Geometry computation remains renderer-owned
in-process CPU projection, with the existing 5 MiB snapshot and 96 KiB read
limits. The root desktop library run (`cargo test -p legion-desktop --lib`)
passed 233 tests, including the five focused owned-source cases. This is
implementation evidence only; it does not replace the ADR's wrapped, desktop,
or packaged/native acceptance gates.

## Consequences

The continuation can use the reviewed local source while retaining the
existing egui/epaint version identity and renderer ownership boundary. Cargo
source provenance remains a release concern, and the global patch must be
removed or revised if upstream epaint 0.34.2 becomes capable of providing the
required API. Local verification does not establish production, full-wrapped,
or product qualification.
