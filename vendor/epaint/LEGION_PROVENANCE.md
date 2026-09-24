# Vendored epaint provenance

This directory is based on the crates.io package `epaint 0.34.2`, from the
local Cargo registry archive:

`C:\Users\dasbl\.cargo\registry\cache\index.crates.io-1949cf8c6b5b557f\epaint-0.34.2.crate`

Upstream repository: https://github.com/emilk/egui/tree/0.34.2/crates/epaint
Package license declaration: `MIT OR Apache-2.0`
Archive size: 109,515 bytes
Archive SHA-256: `92b452e348c2758115288802ca25f86ee286ce2cfae6643711ce116662311310`

The archive contains 42 regular members: `.cargo_vcs_info.json`, `Cargo.lock`,
`Cargo.toml`, `Cargo.toml.orig`, `README.md`, `benches/benchmark.rs`, and the
source tree under `src/` (`brush.rs`, `color.rs`, `corner_radius.rs`,
`corner_radius_f32.rs`, `direction.rs`, `image.rs`, `lib.rs`, `margin.rs`,
`margin_f32.rs`, `mesh.rs`, `mutex.rs`, `shadow.rs`, `shape_transform.rs`,
`shapes/{bezier_shape,circle_shape,ellipse_shape,mod,paint_callback,path_shape,rect_shape,shape,text_shape}.rs`,
`stats.rs`, `stroke.rs`, `tessellator.rs`, `text/{cursor,font,fonts,mod,text_layout,text_layout_types}.rs`,
`texture_atlas.rs`, `texture_handle.rs`, `textures.rs`, `util/mod.rs`, and
`viewport.rs`). No license file is present in the archive.

## Current patch and hashes

The active Legion patch changes exactly these source files relative to the
archive:

* `src/text/fonts.rs` — exposes bounded unwrapped chunk layout, atlas-independent
  metric chunks, separate layout/metric identities, and owned background metric
  snapshots/engines. SHA-256:
  `4b0465ff03bc5e174716d9c1eed6053e15ad6434b3e9f4736c36be81b3150c92`.
* `src/text/mod.rs` — re-exports the background metric snapshot and engine
  types. SHA-256:
  `7e15e03a6531f553dfdfb41435446eca84cd522dbf57d1ceddfd8125dc082dfa`.
* `src/text/text_layout.rs` — adds bounded continuation state, glyph-free
  checkpoint cloning, completed-pass precise summaries, atlas-independent
  metric and wrapped-row descriptors, shared shaping arithmetic, validation,
  and regression tests. SHA-256:
  `600bf948055946162b51561c57f4387900d4938078d7c264f20e4c7781679afc`.
* `src/text/font.rs` — factors atlas-independent glyph metrics and preserves
  resolved ID/advance with empty UVs when rasterization fails. SHA-256:
  `5ee64f7068cca4905b9740e63297f8170a7ab6c6fdf4139cff97dfef3bb06582`.

The upstream SHA-256 values for those same files are:

* `src/text/fonts.rs`: `154b2d3bacdcdbc582b3727a764bcfadfb12378095a0b73819fefc91f90a2313`
* `src/text/text_layout.rs`: `e3fc0cf86c98a68d2825020faa098224e7fc100d33cc91a80ed9c2003dd1b0f2`
* `src/text/font.rs`: `dafe7cd6217fba2b0f7ced070807f399136954c3870761e4e0df53152e9c7813`

`Cargo.lock` is retained as an aligned standalone lock for reproducible vendor
tests. It is not a source patch. Current SHA-256:
`e878979440c267e1ebf648e0fc8d3e390a85c7db2b26b6205922c63e0cc3bbfc`.

## Verification evidence

Using the bundled Python runtime at
`C:\Users\dasbl\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe`,
the archive member inventory and hashes above were compared against this
directory. The earlier standalone command passed 45 tests and remains
historical evidence. The current standalone wrapped-row run passed 61 tests
with zero failures; its raw output and exit code are retained in
`s1-04l-wrapped-root-r15.log`. The standalone vendor check passed with exit
code zero; its raw output is retained in `s1-04l-wrapped-check-root.log`.
A historical metric-scan standalone command
`cargo test --manifest-path vendor/epaint/Cargo.toml --lib` passed: 49 passed,
0 failed; its raw output and exit code are retained in
`s1-04k-metric-scan-final.log`. The source hash recorded with that historical
metric evidence was
`src/text/text_layout.rs` SHA-256
`0bfcd138132798332b59de0e4f5b25cc7248726e647e1f424e5c50650e5a8378`. The
current wrapped-row source has subsequent source-only changes after that
historical metric run. This is source-level vendor evidence only and does not
qualify full product readiness.

The earlier 40-test vendor run remains historical evidence in
`s1-04h-vendor-fix-tests.log`.

Root integration evidence is retained in scratch: desktop targeted tests passed
in `s1-04h-desktop-final-checks.log`, and `cargo deny check` passed with
advisories, bans, licenses, and sources all okay in
`s1-04h-integration-final-gates.log`. These are verification records only; this
file makes no full wrapped or production qualification claim.

Canonical upstream license texts are included beside this notice:

* [`LICENSE-MIT`](https://raw.githubusercontent.com/emilk/egui/0.34.2/LICENSE-MIT),
  SHA-256 `95ca92f5f8ea5231f1580b3a2a799e8260af3114b900e1def5355a7f44bcf60c`.
* [`LICENSE-APACHE`](https://raw.githubusercontent.com/emilk/egui/0.34.2/LICENSE-APACHE),
  SHA-256 `8173d5c29b4f956d532781d2b86e4e30f83e6b7878dce18c919451d6ba707c90`.
