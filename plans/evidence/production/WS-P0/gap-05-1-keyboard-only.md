# GAP-05.1 — Renderer-backed keyboard-only path

**Date:** 2026-09-02  
**Wave:** 2 proof surface  
**Task:** GAP-05.1

## What this is

The PR-15 packet now lists which keyboard routes are certified by renderer
key events and which remain residual typed-shell commands.

Certified through `DesktopEframeApp` egui key dispatch:

- F12 go-to-definition
- Ctrl/Cmd+Shift+F workspace search palette
- Ctrl/Cmd+Shift+G stage focused hunk
- Ctrl/Cmd+Shift+P then `git commit <message>` then Enter

Residual, named so they are not mistaken for certified routes:

- `:git-nav-*` hunk/file focus
- `:git-stage-hunk <hunk-id>` operand form
- `:term-launch <command>`
- test-run typed shell

AccessKit unit roles alone are not this certification. macOS and Linux OS-tree
probes and NVDA/VoiceOver/Orca transcripts are GAP-05.2–05.4.

## What this is not

- Not a live windowed keyboard walk
- Not a screen-reader session
- Not a ledger promotion of PR-UI-001
- Not GAP-05.2 Windows Narrator/NVDA

## Verification

```text
cargo test -p legion-desktop --test keyboard_nav
cargo test -p legion-desktop --test accessibility pr15_accessibility_evidence_keeps_unobserved_platforms_explicit
cargo test -p legion-desktop --lib keymap_dispatch
cargo test -p legion-desktop --test palette_coverage
```

Ledger row statuses are unchanged.
