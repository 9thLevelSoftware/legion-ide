# The three things called GP-5, and which one is in CI

Date: 2026-08-23
Branch: `roadmap-audit`

## Why this file exists

`plans/legion-production-roadmap-v1.0.md` gives Phase 6 the exit gate **"GP-5
promoted to a standing gate and green"**. The repository contains a command
called `golden-path-5`. Reading the second as satisfying the first would be
wrong, and wrong in the direction the standing gates exist to prevent: a record
naming something other than what it is.

Three distinct things carry the name.

| Name | Defined in | What it proves |
| --- | --- | --- |
| GP-5 (v0.1) | `plans/legion-production-master-plan-v0.1.md` §M5 | Fresh machine → signed installer → first-run consent → auto-update → minidump. Superseded; `plans/evidence/production/M12/PKT-UPDATER-evidence.md` records it as having mapped to the update drill. |
| GP-5 (v0.2, and the roadmap's) | `plans/legion-production-master-plan-v0.2.md` §"GP-5 Extension-Constrained Workflow" | A user installs an approved extension/grammar/command contribution; it enhances editor behaviour but **cannot mutate files or reach the network unless policy allows**; permissions are inspectable and auditable. |
| `golden-path-5` (the binary) | `crates/legion-app/src/bin/golden_path_5.rs` | The manual editing loop: copy fixture → open workspace → open file → edit and save → tree-sitter highlight → terminal echo → git stage and commit → evidence TOML. |

## What this change does

Adds `smoke-manual-loop` to `.github/workflows/legion-smoke.yml`, running
`cargo run -p xtask -- golden-path-5` on ubuntu/windows/macos and uploading
`gp5_report.toml`.

The job is named for the loop it exercises rather than for the number the
binary carries, and the workflow header spells out the collision, so nobody
reads a green run here as Phase 6 evidence.

## Local result

`cargo run -p xtask -- golden-path-5` on Windows, 7/7 steps:

```
s1 passed (367ms): fixture copied and workspace opened
s2 passed (2ms):   main.rs opened and buffer loaded
s3 passed (23ms):  edit+save verified on disk
s4 passed (87ms):  TreeSitterParser highlight captures non-empty
s5 passed (251ms): echo marker received via product terminal gate
s6 passed (2115ms): stage-commit cycle verified
s7 passed (0ms):   evidence TOML written
```

Hosted 3-OS result: pending the first run of the added job.

## What this does *not* do

Phase 6's exit gate is **not** advanced by this change. The roadmap's GP-5 is
the extension-constrained workflow, and no harness for it exists in the tree:
there is no smoke that installs an approved extension, exercises a contribution
point, and then proves the extension could not write a file or open a socket
that policy had not allowed.

The Phase 6 gate stays open. This file exists so the next person reading
"golden-path-5 is in CI" beside "GP-5 promoted to a standing gate" does not
conclude they are the same sentence.
