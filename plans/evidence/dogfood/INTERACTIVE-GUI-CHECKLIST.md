# Interactive GUI dogfood checklist (Phase 1 completion)

Use this for a **human-driven eframe** session so Phase 1 reaches ≥3 journals.

## Setup

```text
git checkout main && git pull
cargo run -p legion-desktop -- <path-to-legion-repo-or-fixture>
```

Record branch, SHA (`git rev-parse HEAD`), OS, and whether Ollama/Anthropic keys are present.

## Checklist (copy into journal)

| # | Action | Pass? | Notes |
|---|--------|-------|-------|
| 1 | Open this repo; expand nested dirs (crates/…) | | Watcher should not thrash |
| 2 | Edit a file; save; confirm dirty → clean; external overwrite conflict | | |
| 3 | Focus BYOK field; type; confirm key not inserted into buffer | | |
| 4 | Terminal: type command, see output, kill if needed | | |
| 5 | Assist: Deterministic proposal appears | | |
| 6 | Assist Auto with Ollama (if installed): streaming status then proposal | | |
| 7 | Delegate chat: Streaming… then reply | | |
| 8 | Git panel opens / status rows | | |
| 9 | Debug: **SIMULATED** banner still honest | | Until Phase 2 live |
| 10 | Sandbox panel: Windows caveats visible if Job Object-only | | |

## Journal destination

Copy template from `plans/dogfood/legion-on-legion-weekly-journal-template.md` to:

```text
plans/evidence/dogfood/YYYY-MM-DD-interactive-gui-journal.md
```

## Product-readiness impact

Mark floor bugs vs known cut lines (DAP simulated, Windows sandbox, no installers).
