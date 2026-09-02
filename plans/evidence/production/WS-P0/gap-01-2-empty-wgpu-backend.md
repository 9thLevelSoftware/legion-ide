# GAP-01.2 — Empty `WGPU_BACKEND` disabled Windows/macOS surfaces

**Date:** 2026-09-02  
**Wave:** 2 proof surface  
**Task:** GAP-01.2 follow-up

## What this is

`.github/workflows/legion-windowed-gui.yml` no longer exports `WGPU_BACKEND=`
on Windows and macOS. wgpu parses a set-but-empty `WGPU_BACKEND` as an empty
backend mask (`Backends::from_comma_list("")`), then
`create_surface` fails with `Failed to create surface for any enabled backend: {}`.

Per OS the step now sets a real backend only:

| OS | `WGPU_BACKEND` |
| --- | --- |
| Linux | `gl` (plus `LIBGL_ALWAYS_SOFTWARE=1` and xvfb) |
| Windows | `dx12` |
| macOS | `metal` |

`WGPU_ADAPTER_NAME=Microsoft Basic Render Driver` is removed so an adapter-name
filter cannot hide WARP/Metal when the instance actually has backends.

## What this is not

- Not a green T0-D four-run clock start (needs a hosted 3-OS dispatch after merge)
- Not folding windowed-gui into PR gates
- Not GAP-05.3 VoiceOver / GAP-05.4 Orca

Ledger row statuses are unchanged.
