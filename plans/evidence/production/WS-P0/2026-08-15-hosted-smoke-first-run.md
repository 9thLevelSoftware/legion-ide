# First dispatched hosted 3-OS smoke run — 2026-08-15 (P0.F4.T5)

Run: https://github.com/9thLevelSoftware/legion-ide/actions/runs/31853712883
(workflow_dispatch of `legion-smoke.yml` against `main` @ `8cc415b`, dispatched
2026-08-15 00:28 UTC as roadmap Phase 0.8. The two prior scheduled runs,
2026-08-03 and 2026-08-10, failed in ~7 minutes for the same update-drill
cause diagnosed below.)

## Results

| Job | ubuntu | windows | macos |
| --- | --- | --- | --- |
| GP-1 smoke | **FAIL (s3)** | **FAIL (s3)** | **FAIL (s3)** |
| GP-2 smoke | pass | pass | pass |
| GP-3 smoke | pass | pass | pass |
| GP-4 smoke | pass | pass | pass |
| Update drill | **FAIL (compile)** | **FAIL (compile)** | **FAIL (compile)** |

Nine of fifteen jobs green — GP-2/GP-3/GP-4 pass on all three OSes, which is
the first hosted 3-OS evidence for those paths (previously Windows-local
only).

## Failure 1 — Update drill: `--no-default-features` build break (diagnosed, fixed)

The drill spawns `cargo run -p legion-app --bin upd-drill --no-default-features`.
That configuration failed to compile on `main` (13 errors: ungated
`ProductChatCompletion` / `product_stream_from_completion` uses and ungated
`legion_sandbox` references), so the drill died during compilation, produced
no `target/update-drill/update_drill_report.toml`, and the job failed. This is
the same defect fixed by Phase 0 truth-repair commit `25a1ec0`
(branch `phase-0-truth-repair`), which also adds the configuration to
`legion-gates.yml` so it cannot silently regress again. Expected to pass on
the next run that includes that commit.

## Failure 2 — GP-1 s3: no error diagnostic from hosted rust-analyzer (diagnosed, fixed)

Identical on all three OSes:

- s1 passes; s2 initializes rust-analyzer and reports health `Fresh`
  (ubuntu: 58ms). Hosted rust-analyzer is **1.97.1** (via
  `rustup component add rust-analyzer`); the local pinned smoke uses
  **1.95.0**.
- s3's initial pump receives 1 diagnostics notification for the target uri
  (ubuntu: 2.2s) — the pipe works.
- After introducing a type mismatch via the app edit path, **zero further
  notifications arrive within the 120s deadline**, despite the silent-stall
  workaround nudges (`did_change` v3/v4/v5 at 30s intervals).
  Post-mortem: `buffered_notifications=0`, reader
  `frames_forwarded=6`, child still running, no crash.

**Root cause (confirmed by local reproduction, 2026-08-15):** the failure
reproduced identically on local Windows with rust-analyzer 1.97.1 on PATH —
so the variable is the rust-analyzer version, not the environment. Modern
rust-analyzer serves NATIVE diagnostics (type errors) via LSP 3.17 **pull**
diagnostics (`textDocument/diagnostic`); push (`publishDiagnostics`) carries
flycheck/cargo output, which refreshes on save — and GP-1 s3 deliberately
never saves (documented FS-watcher race). Legion's client advertised empty
capabilities and only listened for push, so on 1.97.1 the type mismatch never
arrived on any channel. The 1.95.0-era passes relied on push behavior that no
longer exists. This also explains the previously intermittent
PKT-S3-WEDGE-R3 "silent stall" signature becoming deterministic.

**Fix (P0.F4.T6):** the session now advertises `textDocument.diagnostic` by
default, records the server's `diagnosticProvider` capability, and exposes
`pull_diagnostics()` whose full reports are converted to
publishDiagnostics-shaped params and routed through the same ingestion path
(`ingest_lsp_publish_diagnostics_for_buffer`). GP-1 s3 pulls when the server
supports it (push remains the fallback for older servers and the mock), with
pull-derived params taking precedence over the push buffer, which may hold
only an empty clearing ack. Contract test:
`cargo test -p legion-app --test rust_analyzer_read_requests`
(`pull_diagnostics_full_report_parses_and_synthesizes_publish_params`); mock
server now advertises `diagnosticProvider` and answers pulls.

**Result:** local GP-1 passes against rust-analyzer 1.97.1 with s3 at
**6.3s** (previous 1.95.0-era passes spent ~28s in the initial push wait
alone; 1.97.1 without the fix timed out at 120s). Hosted 3-OS confirmation
pending the next dispatched run containing this fix.

## Confirmation run — 15/15 green on 3 OSes

Run: https://github.com/9thLevelSoftware/legion-ide/actions/runs/31893365466
(workflow_dispatch of `legion-smoke.yml` against `phase-0-truth-repair`,
2026-08-15, containing both fixes).

| Job | ubuntu | windows | macos |
| --- | --- | --- | --- |
| GP-1 smoke | pass | pass | pass |
| GP-2 smoke | pass | pass | pass |
| GP-3 smoke | pass | pass | pass |
| GP-4 smoke | pass | pass | pass |
| Update drill | pass | pass | pass |

Both hosted failures are resolved on the runners that produced them, not
only locally: the update drill passes now that
`legion-app --no-default-features` compiles, and GP-1 s3 passes on hosted
rust-analyzer 1.97.1 via the pull-diagnostics path. **This is the first
all-green hosted 3-OS smoke run in the project's history** and the first
cross-platform evidence for GP-1 (previously Windows-local only).

## Promotion-clock status

The promotion criterion in
`plans/evidence/production/WS-P0/T0-D-smoke-promotion-criteria.md` requires
**four consecutive green _scheduled_ 3-OS runs**. This run was
workflow_dispatch on a branch, so it does not itself advance that count — it
establishes that the scheduled runs can now pass. The clock starts on the
next scheduled Monday 06:00 UTC run after these fixes reach `main`.
