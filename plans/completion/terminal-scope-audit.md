# S0-01e terminal scope audit

Baseline: committed `HEAD` `8d151d5fa3a4f42dbc0deda1301a458f3716f56d` (source classifications use `git show HEAD:`; current worktree edits are not evidence). This audit replaces only `COMP-SCOPE-FAMILY-07` with 15 atomic terminal product outcomes (stable TERM-003 removed as an exact duplicate) in `requirements.json`. Product acceptance remains `unassessed` for every row.

## Governing source and normalized coverage

The approved feature-family wording is preserved exactly: **“Real shell execution, interactive programs, control keys, resize, scrollback, output rendering, working directory and process cleanup”** (`docs/superpowers/specs/2026-09-04-product-completion-design.md:57`, family-07). The approved S1-06 deliverable is: **“A trusted-workspace terminal that starts in workspace root, executes user/UI commands rather than only displaying labels, supports control keys, resize, scrollback/search/output rendering, interactive TUI use, working-directory changes, cancellation and child cleanup. Its canonical acceptance evidence carries structured child PID, launch timestamp, command class, terminal exit status and independently observed effect; transcript error text is supporting diagnostic evidence only.”** (`docs/superpowers/plans/2026-09-04-manual-language-completion.md:220-224`). Interfaces retain `DesktopAction::{TerminalLaunch,TerminalInput,TerminalOutputPoll}` and require explicit input/approved execution for promised command execution (`...manual-language-completion.md:234`).

| Exact source promise (source location) | Normalized atomic outcome | Mapping |
|---|---|---|
| family-07 wording above (`product-completion-design.md:57`) | shell execution, interactive programs/control keys, resize, scrollback/output, cwd, cleanup | COMP-TERM-001, 004, 006, 007, 010; exact existing P2.F2.T2/T3/T4 mapped below |
| S1-06 trusted workspace/root/explicit command (`manual-language-completion.md:224,234`) | trusted root launch and explicit command route | COMP-TERM-001 |
| S1-06 control keys, resize, TUI, cancel/cleanup (`manual-language-completion.md:224,248-250`) | interactive child control and lifecycle | COMP-TERM-006, 010, 015 |
| S1-06 structured PID/time/class/exit/effect (`manual-language-completion.md:224,250`) | independent canonical process evidence | COMP-TERM-015 |
| WS-TERM-01 TERM.01 (`master-plan-v0.2.md:397`) and M8 shell precedence (`WS-TERM-01-evidence.md:17,173`) | workspace→user→platform shell profile selection | COMP-TERM-002 |
| WS-TERM-01 TERM.02/12 (`master-plan-v0.2.md:398,408`) | ConPTY/Unix PTY and PowerShell/cmd/bash/zsh platform parity | COMP-TERM-014; P2.F2.T5 exact existing mapping |
| TERM.03/04/07/09/10/11 (`master-plan-v0.2.md:399-407`) | trust gate (retained P2.F2.T2), redaction/env policy, cleanup, command proposal route, failure UX | retained `COMP-P2-F2-T2-1` for trust denial; COMP-TERM-009, 010, 012, 013, 014 |
| TERM.05/06 (`master-plan-v0.2.md:401-402`) | bounded scrollback/search and resize propagation | COMP-TERM-005; P2.F2.T3 exact existing mapping for resize |
| WS05.T1 (`master-plan-v0.1.md:305`) | real PTY interactive shell, input/resize/kill, all three OSes | COMP-TERM-001, 004, 006, 014 |
| WS05.T2 (`master-plan-v0.1.md:306`) | styled grid, cursor, selection/copy, scrollback, URL detection, TUI usability | COMP-TERM-004, 005, 006 |
| WS05.T3 (`master-plan-v0.1.md:307`) | OSC 133 boundaries, OSC 7 cwd, per-command exit/duration, navigable blocks | COMP-TERM-007, 009; exact existing P2.F2.T4 mapping |
| WS05.T4 (`master-plan-v0.1.md:308`) | dedicated task terminal tabs and structured exit/duration/output reference | COMP-TERM-008 |
| WS05.T5 (`master-plan-v0.1.md:309`) | policy-scoped agent terminal, bounded/redacted output, background polling | COMP-TERM-013 |
| WS05.T6 (`master-plan-v0.1.md:310`) | orphan reaping, crash cleanup, env hygiene, ConPTY parity | COMP-TERM-010, 012, 014 |
| terminal supervisor (`control-first-adaptive-ide-technical-design-v0.1.md:298-306`) | process/shell/env/cwd/timeout/kill/restart/scrollback/redaction mediation | COMP-TERM-009, 010 (timeout/deadline), 011, 012 |
| UI terminal projection (`foundational-core-ide-platform-roadmap-v0.1.md:203-206`) | bounded nonblocking terminal service plus tabs/output/scrollback/search and restart/kill/reconnect controls | COMP-TERM-005, 010, 011, 016 |
| ADR-0026 gates (`ADR-0026-standalone-local-terminal-runtime.md:15-24`) | lifecycle, cwd/env/shell, output bounds, kill-tree/orphans, metadata-only persistence, native parity | retained COMP-P2-F2-T2-1, COMP-TERM-009, 010, 012, 014 |
| ADR-0035 decision (`ADR-0035-terminal-stack.md:77-143`) | platform PTY ownership, VTE/grid, selection/scrollback/OSC, structured command blocks, metadata-only output | COMP-TERM-004, 005, 007, 009 |

## Exact deduplication to retained rows

These source promises are already represented by exact legacy acceptance outcomes and are mapped without altering those rows: P2.F2.T1 (`COMP-P2-F2-T1-1`, “Manual mode can run `cargo test` in a terminal tab and capture exit status metadata”); P2.F2.T2 (`COMP-P2-F2-T2-1`, denied/untrusted paths fail closed); P2.F2.T3 (`COMP-P2-F2-T3-1`, panel renders/scrolls/accepts input/respects resize); P2.F2.T4 (`COMP-P2-F2-T4-1`, OSC parsed with cwd/exit queryable); and P2.F2.T5 (`COMP-P2-F2-T5-1`, trusted Windows ConPTY equivalence). They are not duplicated. Narrower promises such as shell profiles, task result references, restart/reconnect, explicit evidence fields, and TUI control remain separate because those legacy outcomes do not prove them.

## Implementation trace and limits at committed HEAD

| ID | HEAD implementation evidence (literal path/symbol or test) | Classification and limit |
|---|---|---|
| COMP-TERM-001 | `crates/legion-app/src/lib.rs:7615-7792` `TerminalWorkflow::launch`; `crates/legion-terminal/src/lib.rs:621-757` `TerminalRuntime::launch`; `crates/legion-platform/src/lib.rs:1903` `NativePtyService` | partial: route exists; native product process oracle and full S1 scenario remain unassessed |
| COMP-TERM-002 | `crates/legion-app/src/terminal_policy.rs:15` `TerminalShellSelection`; `crates/legion-app/src/lib.rs:7748` launch request | implemented: selection function/path exists; cross-OS/profile qualification remains unassessed |
| COMP-TERM-004 | `crates/legion-terminal/src/lib.rs:782-840` input/resize; `crates/legion-terminal/src/grid.rs:23` `TerminalGrid`; `crates/legion-desktop/src/view/terminal_panel.rs:39` `TerminalPanelRenderModel` | implemented: lower-layer path exists; TUI/native rendering qualification unassessed |
| COMP-TERM-005 | `crates/legion-protocol/src/lib.rs:16872-16887` scrollback/search DTOs; `crates/legion-terminal/src/grid.rs`; `crates/legion-desktop/src/view/terminal_panel.rs` | partial: projection structures exist; URL/search/TUI product journey unassessed |
| COMP-TERM-006 | `crates/legion-terminal/src/lib.rs:782` input and `:1010` kill; `crates/legion-platform/src/lib.rs:1903` PTY | partial: lifecycle primitives exist; declared interactive TUI/control-key scenario is absent/unassessed |
| COMP-TERM-007 | `crates/legion-terminal/src/osc.rs:parse_terminal_shell_output`, `TerminalShellBoundary`; `crates/legion-terminal/src/session.rs:TerminalSessionMetadata` | implemented: parser/session metadata exists; external shell truth and native acceptance unassessed |
| COMP-TERM-008 | `crates/legion-protocol/src/lib.rs:4745` `TerminalCommandProposal`; terminal launch/runtime APIs | partial: proposal DTO/runtime exist; dedicated task/test-explorer structured completion route not evidenced here |
| COMP-TERM-009 | `crates/legion-terminal/src/lib.rs:854` output polling; `crates/legion-protocol/src/lib.rs:3021` output chunk and `:3103` audit record | partial: bounded/redacted DTO path exists; navigable links and external output oracle unassessed |
| COMP-TERM-010 | `crates/legion-terminal/src/lib.rs:979-1099` close/kill/cleanup; `crates/legion-app/src/lib.rs:26120` `cleanup_terminal_orphans` | partial: lifecycle cleanup functions exist, but timeout/deadline enforcement and native crash/restart drill remain unassessed |
| COMP-TERM-011 | `crates/legion-protocol/src/lib.rs:2981` runtime states; `crates/legion-app/src/lib.rs:7511` failure projection | partial: failure states project; restart/reconnect control and stale-session recovery are unassessed |
| COMP-TERM-012 | `crates/legion-app/src/terminal_policy.rs:105` `TerminalEnvPolicy`; `crates/legion-platform/src/lib.rs:1903` PTY spawn | implemented: env filtering path exists; per-OS external secret oracle unassessed |
| COMP-TERM-013 | `crates/legion-protocol/src/lib.rs:4745` proposal DTO; `crates/legion-terminal/src/lib.rs:854` polling | partial: contracts exist; policy-scoped agent/background execution route not evidenced as product behavior |
| COMP-TERM-014 | `crates/legion-platform/src/lib.rs:783,1903` native PTY; `crates/legion-terminal/tests/platform_shell_smoke.rs` | partial: platform primitives/smoke path exists; all declared OS/shell parity remains unassessed |
| COMP-TERM-015 | `crates/legion-protocol/src/lib.rs:3103` `TerminalAuditRecord`; `crates/legion-app/src/lib.rs:26120` cleanup evidence | partial: metadata records exist; independent PID/time/class/effect canonical scenario is unassessed |

| COMP-TERM-016 | `crates/legion-protocol/src/lib.rs:16902` `TerminalPanelProjection`; `crates/legion-app/src/lib.rs:14312` terminal workflow ownership | partial: projection/session ownership exists; independent multi-session/tab switching and restart controls remain unassessed |

## Additional-source inspection record

`docs/superpowers/plans/2026-09-04-completion-traceability.md:25-31` was inspected; its additional-requirements list was followed. The listed `plans/legion-production-roadmap-v1.0.md`, `plans/product-readiness-ledger.md`, `plans/phase-status-ledger.md`, `plans/p0-installed-product-sequence-v0.1.md`, `plans/control-first-adaptive-ide-granular-implementation-plan-v0.1.md`, `plans/control-first-adaptive-ide-technical-design-v0.1.md`, `plans/foundational-core-ide-platform-roadmap-v0.1.md`, `plans/remaining-implementation-tasks-plan-v0.1.md`, and `docs/ui/canvas-workspace-direction.md` were inspected for terminal-specific commitments. The foundational implementation plan also promises terminal panel tabs and per-session kill/restart state (`plans/foundational-core-ide-platform-implementation-plan-v0.1.md:178`), mapped to COMP-TERM-016. No inspected source promises terminal split panes or multiplexing beyond independently identified tabs/sessions; no split requirement was invented. Only the roadmap/technical-design/foundational-terminal projection commitments recorded above added terminal outcomes; the remaining listed documents contained no additional terminal product promise beyond the approved sources. The unavailable `.hermes` generator and cleaned engineering status/audit/plan sources are recorded as unavailable by traceability and were not inferred. Historical v0.1/master and retained ADR/evidence were treated as supporting requirements where they cross the approved terminal family; historical status labels never changed the required scope.

Terminal-specific retained evidence was inspected at `plans/evidence/production/M8/WS-TERM-01-evidence.md:12-32,119-142,238-296`, `plans/evidence/phase-8/terminal-runtime-policy-tests.txt:1-15`, and `plans/evidence/phase-8/terminal-pty-platform-tests.txt:1-15`. These are implementation signals only; all product acceptance stays `unassessed`. This increment does not claim S0-01 completion or acceptance.
