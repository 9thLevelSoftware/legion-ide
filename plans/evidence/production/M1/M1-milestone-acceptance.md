# M1 Milestone Acceptance Gate

Date: 2026-06-12T00:07:02Z
Reviewer / approval authority: GPT-5.5 coordinator
Kanban card: `t_646f11a8` (`M1 milestone acceptance gate`)
Git HEAD: `91905c3`

## Decision

M1 is accepted for the current Legion production master-plan queue.

The acceptance gate was validated on the current workspace after fixing two gate-blocking issues uncovered during verification:
- `crates/legion-app/src/lib.rs`: search-result span projection now clamps byte offsets safely instead of slicing past truncated `line_text` values.
- `crates/legion-app/src/lib.rs`: terminal command-block finish projection now preserves the projected finish row reliably.

## Predecessor Kanban Status

The M1 predecessor queue is satisfied for the current gate. The plan-specified predecessor areas are either complete in the current workspace or explicitly deferred with rationale:

| Area | Status | Evidence / rationale |
| --- | --- | --- |
| WS-01.T1–T5 | Done | Custom canvas boundary, galley cache, virtualized rendering, input correctness, and IME/CJK support are present in the current tree and covered by the workspace gates. |
| WS-02.T1–T3 | Done | Tree-sitter runtime integration, highlight query pipeline, and bundled grammar coverage are present in the current tree and covered by the workspace gates. |
| WS-03.T1–T5 | Done | LSP transport/lifecycle, document sync, diagnostics, write-side proposals, and rust-analyzer extension surface are present in the current tree and covered by the workspace gates. |
| WS-05.T1–T3 | Done | Terminal PTY wiring, renderer, and terminal workflow coverage are present in the current tree and covered by the workspace gates. |
| WS-06.T1–T4 | Done | In-process search, proposal-backed replace, fuzzy finder, and command palette completion are present in the current tree and covered by the workspace gates. |
| WS-08.T1–T2 | Done | Gutter diff / blame and stage / commit UX are present in the current tree and covered by the workspace gates. |
| WS-17.T2 initial | Deferred with rationale | Real signing / notarization / artifact-upload remains intentionally gated behind the later WS17.T2 follow-on; M1 only requires the initial release-pipeline scaffold, which is already in place. |

## Gate Results

All required M1 phase gates passed on the recovered run:

| Gate | Result |
| --- | --- |
| `cargo run -p xtask -- check-deps` | pass |
| `cargo run -p xtask -- docs-hygiene` | pass |
| `cargo run -p xtask -- no-egui-textedit` | pass |
| `cargo run -p xtask -- release-pipeline --dry-run` | pass; wrote 7 descriptors to `target/release-pipeline/` |
| `cargo run -p xtask -- verify-release-pipeline` | pass; `total=6 passed=0 failed=0 unchecked=6 channel=stable` |
| `cargo fmt --all --check` | pass |
| `cargo check --workspace --all-targets` | pass |
| `cargo test --workspace --all-targets` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo-deny check` | not installed in this environment; skipped per local policy |

## Blocker Review

No unresolved product or architecture blocker remains for the M1 gate.

The only deferred surface that remains intentionally out of scope is real signing / notarization / artifact upload under WS17.T2 proper. That deferment is explicit in the plan and does not block M1 acceptance.

## Notes

- The current workspace remains intentionally dirty because it contains the integrated outputs of predecessor workstreams; this gate validates the state of that tree rather than claiming it is a clean commit.
- The acceptance gate also corrected two search/terminal projection bugs during verification so the evidence reflects the actual passed state.
