# Full product resume evidence — 2026-09-08

## What this directory is

This directory holds one raw log: the workspace test suite run for round r00 of
the resumed full-product completion work.

**Classification: workspace suite result. Not product acceptance.**

A green workspace suite says the repository's own tests compile and pass on this
host. It does not assess any acceptance row, does not qualify a packaged
product, and does not establish native GUI, cross-OS, release, dependency, or
phase readiness. No `acceptance` value in `plans/completion/requirements.json`
may be changed on the strength of this run. The 59-package /
419-requirement scope remains unchanged and unassessed as a complete product
matrix.

## The run

- Packet: `round-r00-full-suite`
- Kind: `cargo-full-suite`
- Command: `cargo test --workspace --all-targets -j 1 --no-fail-fast`
- Worktree: `D:/legion-ide-completion`, branch `codex/full-product-resume`
- Environment: `LEGION_TEST_NODE_RUNTIME` exported
- Log: [round-r00-full-suite.log](round-r00-full-suite.log) (6983 lines, ends `EXIT=0` at line 6983)
- Source of the copy: `.superpowers/sdd/2026-09-04-full-product-completion/round-r00-full-suite-full-suite.log`
  (local gitignored ledger); copied byte-identical,
  SHA-256 `06a09d89d2ef7db434eb64ff2120f5fe9306621adc265a85bdf73d2e7e09d3ed`.

## Outcome exactly as logged

Build finished (``Finished `test` profile [unoptimized + debuginfo] target(s) in
3m 30s``, log line 37) with zero compiler errors.

356 test binaries were launched and 356 `test result:` lines were emitted; every
started target reported a result and none was cut short. Aggregate across all
356 targets:

| | passed | failed | ignored | measured | filtered out |
|---|---|---|---|---|---|
| 49 unittest binaries | 2140 | 0 | 2 | 0 | 0 |
| 307 integration-test binaries | 2521 | 0 | 35 | 0 | 0 |
| **total** | **4661** | **0** | **37** | **0** | **0** |

Every `test result:` line is `ok.`; there are zero non-ok result lines. The log
contains 0 lines matching `^error[` or `^error:`, 0 lines containing `FAILED`, 0
`failures:` sections, and 0 `panicked at` occurrences. The list of failing test
names is empty. Doc-tests were not run, which is expected under `--all-targets`.

All 30 workspace members named in `Cargo.toml` produced a unittest binary
(`legion_app`, `legion_desktop`, `xtask`, `legion_ui`, `legion_editor`,
`legion_text`, `legion_project`, `legion_index`, `legion_lsp`, `legion_ai`,
`legion_ai_providers`, `legion_agent`, `legion_tracker`, `legion_memory`,
`legion_security`, `legion_platform`, `legion_protocol`, `legion_storage`,
`legion_observability`, `legion_plugin`, `legion_vscode_compat`,
`legion_collaboration`, `legion_remote`, `legion_remote_transport`,
`legion_terminal`, `legion_debug`, `legion_telemetry`, `legion_retention`,
`legion_sandbox`, `legion_cli`). `legion_app`, `legion_desktop` and `xtask` each
contributed two unittest binaries (lib + bin). The remaining unittest binaries
are test fixtures and examples.

A second ``Finished `test` profile ... in 0.18s`` at log line 1235 follows
`Compiling bench-test-fixture v0.1.0` in a temp directory. That is a
test-spawned cargo build of a temp fixture crate from inside the suite
(`legion_bench_live`), not a second workspace build. Its interleaved output is
the only ordering anomaly in the Running-to-result alternation (lines 1236 and
1245); the counts still pair 1:1 and the parent target reports 13 passed at line
1240.

Warnings: 22 warning lines. Only 2 are compiler warnings, both from the excluded
vendored crate — ``warning: falling back to `f32` as the trait bound `f32:
From<f64>` is not satisfied`` at `vendor/epaint/src/tessellator.rs:2326:34`
(lint `float_literal_f32_fallback`, future-incompatible, rust-lang issue
#154024) and its `epaint (lib) generated 1 warning` roll-up. The other 20 are
`LF will be replaced by CRLF` notices from git subprocesses that tests spawn
against temp fixtures. No warning came from any `legion-*` crate.

Sum of per-target reported runtimes is 114.5s; wall clock was dominated by the
3m30s compile plus serialized `-j 1` execution.

## What this run does NOT cover — 37 ignored tests

37 tests were reported ignored (37 occurrences, 37 unique names). **These tests
did not run.** `#[ignore]` is a compile-time attribute, so exporting
`LEGION_TEST_NODE_RUNTIME` opted none of them in under a plain `cargo test`
invocation — several print an opt-in Node/Pyright/TypeScript-fixture reason
string and were skipped all the same. Running them requires a separate
invocation with `-- --ignored` plus the named fixtures and servers, which was
not part of this step.

Live-model hostile eval (needs a local model server; `cargo run -p xtask --
hostile-eval-live`):

1. `a_live_model_cannot_exfiltrate_a_secret`
2. `a_live_model_reading_injected_text_still_cannot_act_on_it`
3. `tests::anthropic_messages_client_live_smoke_round_trip`

Native LSP / language runtime (opt-in Node, retained Pyright and TypeScript
fixtures, rust-analyzer):

4. `explicit_python_startup_is_lazy_live_and_restart_preserves_dirty_text`
5. `explicit_typescript_startup_is_lazy_live_and_restart_preserves_dirty_text`
6. `native_node_runtime_probe_uses_selected_executable_and_revalidates`
7. `native_python_diagnostic_clears_after_editor_replace_and_save`
8. `native_python_rename_external_overwrite_rejects_without_partial_mutation`
9. `native_python_rename_is_reviewable_cancelable_and_saves_cross_file_edit`
10. `native_typescript_formatting_is_reviewable_before_disk_mutation`
11. `native_typescript_missing_import_code_action_is_reviewable_and_applies_after_approval`
12. `native_typescript_organize_imports_is_reviewable_and_removes_only_unused_imports`
13. `native_typescript_rename_conflict_does_not_partially_apply`
14. `native_typescript_rename_is_reviewable_and_applies_only_after_approval`
15. `retained_pyright_fixture_materializes_with_catalog_identity`
16. `rust_analyzer_full_workflow`
17. `rust_analyzer_initializes_against_legion_repo_when_opted_in`
18. `rust_analyzer_initializes_and_emits_diagnostics`
19. `rust_analyzer_product_composition_smoke`
20. `two_ra_same_workspace_didchange_stress`

100MB / scale budgets:

21. `large_file_100mb_degraded_mode_measurement` (report-only)
22. `scale_100mb_buffer_creation_under_budget`
23. `scale_100mb_memory_ceiling`
24. `scale_100mb_single_keystroke_edit_under_budget`
25. `scale_100mb_snapshot_creation_and_chunk_iteration`
26. `scale_100mb_streaming_from_reader`
27. `scale_100mb_viewport_slice_under_budget`

Perf / timing diagnostics and other opt-ins:

28. `completion_request_cost_against_position_depth` (does not assert)
29. `indexed_workspace_search_benchmark_large_fixture`
30. `keystroke_cost_amortized_versus_compaction`
31. `keystroke_cost_by_position_in_the_file`
32. `keystroke_cost_tracks_line_count_not_byte_count`
33. `snapshot_retention_and_release`
34. `training::tests::regenerate_training_candidate_fixtures`
35. `undo_redo_latency_under_edit_burst`
36. `viewport_projection_cost_against_scroll_depth`
37. `workspace_open_1000_files_completes_within_budget`

Any acceptance row that depends on native LSP behaviour (rust-analyzer, Pyright,
TypeScript), on the live-model hostile eval, on the 100MB scale budgets, or on
the keystroke/viewport perf budgets has **no evidence from this run** and must
not be assessed from it.

## Unverifiable from the log alone

The log has no invocation header; it begins directly with compiler output. The
branch name `codex/full-product-resume` and the `LEGION_TEST_NODE_RUNTIME`
export are therefore recorded from the invoking context, not readable from the
log bytes. Neither affects the pass/fail outcome above.

## Standing position

The full-product goal remains unfinished, not blocked and not complete. Native
GUI automation is resumed on this Windows host by owner instruction; macOS and
Linux hosts remain unavailable and their rows stay blocked with their recorded
prerequisite strings.

---

# Round r01 — packet round evidence (appended 2026-09-08)

## What round r01 was

One bounded packet round on branch `codex/full-product-resume`. One packet
passed review: `s2-03b-language-extract`, a pure-move extraction of the language
toolchain settings and the proposal-kind dispatch out of
`crates/legion-app/src/lib.rs` into `crates/legion-app/src/language/`. Two
packets were rejected at review and reverted before any test ran
(`s2-03a-bounded-stdin`, `s0-03e-register-verify`); no packet failed its tests.

**Classification for every log below: component or integrated evidence.
Not packaged evidence, not native GUI evidence, not product acceptance.**

No `acceptance` value in `plans/completion/requirements.json` was changed on the
strength of any of these runs, and none may be. These are crate-level compile
checks, crate-level unit and integration tests, a static lint pass, and
repository hygiene gates. They exercise no packaged artifact, no real window, no
real language-server process, and no cross-OS host.

## Commands

The raw logs carry no invocation header — each begins directly with tool output
and ends with a trailing `EXIT=<code>` line written by the runner. The command
column below is the command the round driver
(`.superpowers/sdd/2026-09-04-full-product-completion/legion-completion-round.js`,
steps Check and Test) issues for that step, and it is consistent with the tool
output present in each log. Every cargo command in this round took `-j 1` under
the single-cargo-lane rule.

## Logs

| log | command | exit | classification |
|---|---|---|---|
| `round-r01-check0-legion-platform.log` | `cargo check -p legion-platform --all-targets -j 1` | 0 | component evidence — compile check only, no test executed |
| `round-r01-check0-legion-app.log` | `cargo check -p legion-app --all-targets -j 1` | 0 | component evidence — compile check only, no test executed |
| `round-r01-check0-xtask.log` | `cargo check -p xtask --all-targets -j 1` | 0 | component evidence — compile check only, no test executed |
| `round-r01-test-step1.log` | `git checkout --` of the rejected packets' tracked paths (no cargo) | 0 | process record, not product evidence — records the revert of the two rejected packets |
| `round-r01-test-step2a.log` | `cargo test -p legion-app --test typescript_toolchain_settings -j 1 --no-fail-fast` | 0 | integrated evidence, crate level — in-crate integration target, no real language server, no packaged product |
| `round-r01-test-step2b.log` | `cargo test -p legion-app --lib -j 1 --no-fail-fast language::formatting_dispatch_tests` | 0 | component evidence — in-crate unit tests |
| `round-r01-test-step2c.log` | `cargo test -p legion-app --lib -j 1 --no-fail-fast language::toolchain_settings::toolchain_approval_tests` | 0 | component evidence — in-crate unit tests |
| `round-r01-test-step4a.log` | `cargo test -p legion-app --lib -j 1` | 0 | component evidence — crate unit suite, regression guard |
| `round-r01-test-step4b.log` | `cargo test -p legion-desktop --lib -j 1` | 0 | component evidence — crate unit suite, regression guard |
| `round-r01-test-step5.log` | `cargo clippy --workspace --all-targets -j 1 -- -D warnings` | 0 | component evidence — static lint, no test executed |
| `round-r01-test-step6a.log` | `cargo fmt --all --check` | 0 | component evidence — repository hygiene gate |
| `round-r01-test-step6b.log` | `cargo run -p xtask -- check-deps` | 0 | component evidence — repository hygiene gate |
| `round-r01-test-step6c.log` | `cargo run -p xtask -- docs-hygiene` | 0 | component evidence — repository hygiene gate |
| `round-r01-test-step6d.log` | `cargo run -p xtask -- claim-audit` | 0 | component evidence — repository hygiene gate |
| `round-r01-test-step6e.log` | `cargo run -p xtask -- extract-before-modify` | 0 | component evidence — chokepoint growth gate |

Step 3 of the driver (the `--ignored` real-server startup suites) produced no log
because no packet in this round was marked `real_server`; the passed packet is a
pure move. That absence is not a skipped pass — the real-server rows simply have
no evidence from round r01.

## Copy integrity

Each log was copied byte-identical from the gitignored ledger
`.superpowers/sdd/2026-09-04-full-product-completion/` into this directory and
re-hashed after the copy; every pair matched.

| log | lines | SHA-256 |
|---|---|---|
| `round-r01-check0-legion-platform.log` | 4 | `2c546ca14139c3828f449558cd0c747e81cba2db751fa19780726be463b66fb3` |
| `round-r01-check0-legion-app.log` | 25 | `60e6339b68d365ad8a95fb0a6f346b482f7f20f845a6fb96db241fae11ba1a46` |
| `round-r01-check0-xtask.log` | 10 | `007f9de0f447ab3e323138f1aff8de69b79fb3864847f313eb08e739e1b366d8` |
| `round-r01-test-step1.log` | 25 | `4b0c8cdfe2567d25c642d813b0a58ec6f930e2126df5b882eb71c5e2c1f328bd` |
| `round-r01-test-step2a.log` | 18 | `ea15b1708a508c82af9d4febd0dacda27be2249c01be4de73c136618767ab55d` |
| `round-r01-test-step2b.log` | 12 | `e0e1a55589c3f2f370f7c82c3356d8db0e727c503e8f3450548e3492ba3170d2` |
| `round-r01-test-step2c.log` | 11 | `372db2a3864684d4fa3de24b82b23d07a199c69c6b8c3a3159a8edf341954bce` |
| `round-r01-test-step4a.log` | 447 | `755d52241e6765c9cbcb8810058246c83238e0b74beaa5ece4e106922cf7b8ea` |
| `round-r01-test-step4b.log` | 270 | `768700fde9254d8c8e5a3cfb679610ba70046c65f89cac5aa7c78b32dbbdff91` |
| `round-r01-test-step5.log` | 22 | `b344d85a276c4d281245ccea62a83dc58ee476279bc5dd6391b48492646b8446` |
| `round-r01-test-step6a.log` | 1 | `418a5c17f33c70e99b0cc0a07fce69191489cfedc94164bfa903785777c5bd4b` |
| `round-r01-test-step6b.log` | 7 | `718f6d2e2a47a0de3efd52d3acee85a51746f0d18e3bd1e183c538e7648ff88f` |
| `round-r01-test-step6c.log` | 4 | `31a4c7b8571b88142024770317c27cec193fa0cb884af5c9dacd78909066505b` |
| `round-r01-test-step6d.log` | 4 | `61a4c8e60570d2bb6d4c97cc28156db6791917256960e01635a7d985b500b0a3` |
| `round-r01-test-step6e.log` | 4 | `e56ade19083b2c5e960a68ff8dd507c8befa2e633b08647a8da3795dce4f6767` |

The hashes above are of the LF bytes as the runner wrote them, which is
also what git stores: this repository sets `core.autocrlf=true` and
`.gitattributes` carries no rule for `plans/evidence/**`, so a Windows checkout
expands these logs to CRLF and re-hashing them there will not reproduce these
values. Compare against the stored blob (`git show <rev>:<path> | sha256sum`),
not against a Windows working-tree copy.

## Outcomes exactly as logged

- `check0` (three logs): all three crates checked with `--all-targets`; the `dev`
  profile finished in 17.73s, 52.56s and 29.57s respectively; zero compiler
  errors; `EXIT=0`.
- Step 1: the two rejected packets' tracked files were restored with
  `git checkout --` (`checkout rc=0`), and the one untracked file the rejected
  bounded-stdin packet would have added
  (`crates/legion-platform/src/bounded_stdin.rs`) was recorded `ABSENT (never
  created; nothing to delete)`. The pre-revert diffstat it discarded was 5 files,
  1021 insertions, 38 deletions. Post-revert status for those paths is empty; the
  only remaining dirty paths are the four owned by the passed packet.
- Step 2a: `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0
  filtered out; finished in 0.02s`.
- Step 2b: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 436
  filtered out; finished in 0.01s`.
- Step 2c: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 436
  filtered out; finished in 0.01s`.
- Step 4a regression guard: `test result: ok. 439 passed; 0 failed; 0 ignored; 0
  measured; 0 filtered out; finished in 0.65s`. The driver expects 439 or more;
  439 observed, no regression.
- Step 4b regression guard: `test result: ok. 243 passed; 0 failed; 0 ignored; 0
  measured; 0 filtered out; finished in 11.09s`. The driver expects 243 or more;
  243 observed, no regression.
- Step 5: clippy finished in 1m 15s with `EXIT=0`. The only warning in the log is
  the vendored-crate `float_literal_f32_fallback` future-incompatibility at
  `vendor/epaint/src/tessellator.rs:2326:34` and its `epaint (lib) generated 1
  warning` roll-up; `epaint` is excluded from the workspace lint failure. No
  `legion-*` crate warned.
- Step 6a: no output at all apart from the trailing `EXIT=0` — the formatting
  gate found nothing to report.
- Step 6b: `dependency policy checks passed`.
- Step 6c: `documentation hygiene checks passed`.
- Step 6d: `claim audit passed`.
- Step 6e: `extract-before-modify: no chokepoint file grew past its slack`.

Every one of the fifteen logs ends `EXIT=0`. No log contains a `FAILED` marker, a
`failures:` section, a `panicked at` line, or a non-`ok.` `test result:` line.

## Register effect of round r01: none

The reviewer-proposed implementation transition for the passed packet was
`COMP-LANG-007` to `partial`. That row was already `partial`, so applying the
transition changed nothing: `plans/completion/requirements.json` still holds 419
rows, the implementation tally is unchanged at 143 implemented / 233 partial / 43
absent, and all 419 `acceptance` values remain `unassessed`. The file is
byte-identical to its state before the round. A pure move is expected to be
register-neutral; it relocates code without adding product behaviour.

## Standing position

The full-product goal remains unfinished, not blocked and not complete. Native
GUI automation is resumed on this Windows host by owner instruction; macOS and
Linux hosts remain unavailable and their rows stay blocked with the exact
prerequisite strings recorded in `plans/completion/decisions.md` and in the local
`blockers.json`.

## Round r02 — bounded stdin, register validator, canonical registers

Three packets passed independent review with no blocking findings. This is
component evidence. It does not qualify packaged completion, native GUI
behaviour, or any of the 419 requirements; every row remains `unassessed`.

Packet checks:

- `round-r02-test-packet-tests-s2-03a.log` and
  `round-r02-test-packet-tests-s2-03a-rerun.log` record the first bounded-stdin
  attempt, in which one test failed (`test result: FAILED. 0 passed; 1 failed`,
  `EXIT=101`). They are retained because a failed attempt is part of the record.
- `round-r02-retest-packet-retests-s2-03a-test.log` records the repaired run:
  `cargo test -p legion-platform --test bounded_process -j 1 --no-fail-fast`,
  18 passed, 0 failed, `EXIT=0`.
- `round-r02-test-packet-tests-s0-03e.log` records the register-validator tests,
  33 passed and 18 passed, 0 failed, `EXIT=0`.

Round gates, re-run by the coordinator with full command provenance after the
first attempt was refuted for producing logs that did not show a command had
run:

| Log | Result |
| --- | --- |
| `round-r02-gates-fmt.log` | `cargo fmt --all --check`, EXIT=0 |
| `round-r02-gates-check-deps.log` | dependency policy, EXIT=0 |
| `round-r02-gates-docs-hygiene.log` | documentation hygiene, EXIT=0 |
| `round-r02-gates-claim-audit.log` | claim audit, EXIT=0 |
| `round-r02-gates-extract-before-modify.log` | no chokepoint growth, EXIT=0 |
| `round-r02-gates-regress-app.log` | `legion-app` lib, 439 passed, 0 failed |
| `round-r02-gates-regress-desktop.log` | `legion-desktop` lib, 243 passed, 0 failed |
| `round-r02-gates-clippy.log` | workspace all-targets `-D warnings`, EXIT=0 |
| `round-r02-gates-register-verify.log` | `verify-completion-register`, **EXIT=1**, 1579 structural issues |

The register validator exiting 1 is the truthful current state, not a
regression. It now runs for the first time, and it reports that the 419
requirement rows still carry empty `scenario_ids` and `configuration_ids`, so
required rows have no scenario or configuration coverage. Binding those rows is
the next register task. No row was accepted, and no exemption was added to make
the validator pass.

The first gate attempt of this round was refuted and superseded. Its
narration-only logs are retained in the local ledger and were deliberately not
copied here, because they are not evidence of execution.

## Round r03 — register binding, Python toolchain settings, formatter approval

Three packets passed independent review. Component evidence only; every one of
the 419 rows remains `acceptance: unassessed`.

| Log | Result |
| --- | --- |
| `round-r03-gates-fmt.log` | `cargo fmt --all --check`, EXIT=0 |
| `round-r03-gates-check-deps.log` | dependency policy, EXIT=0 |
| `round-r03-gates-docs-hygiene.log` | documentation hygiene, EXIT=0 |
| `round-r03-gates-claim-audit.log` | claim audit, EXIT=0 |
| `round-r03-gates-extract-before-modify.log` | no chokepoint growth, EXIT=0 |
| `round-r03-gates-regress-app.log` | `legion-app` lib, 446 passed, 0 failed |
| `round-r03-gates-regress-desktop.log` | `legion-desktop` lib, 243 passed, 0 failed |
| `round-r03-gates-clippy.log` | workspace all-targets `-D warnings`, EXIT=0 |
| `round-r03-gates-register-verify.log` | `verify-completion-register`, **EXIT=1**, 144 structural issues |

The register validator improves from 1579 issues to 144 and still exits 1,
which is the honest state. The remainder is 77 rows the binding generator
deliberately left unbound, each named with a reason code in its packet report,
plus 67 pre-existing register defects that the first real validator run has now
surfaced: product rows carrying `protected_product_ids`, `source_refs` pointing
at directories rather than files, a product requirement owned by S6, and two
non-bidirectional links. No row was accepted and no exemption was added.

The round's first formatting gate failed with 13 mechanical rustfmt hunks in
two files of the formatter-approval packet. `round-r03-fix-fmt.log` records the
`cargo fmt --all` that repaired it, and every gate above is the re-run after
that fix. The failed gate attempt is part of the record.

## Round r04 — register defect repair, Python formatter route, native harness

Three packets passed independent review. Component evidence only; all 419 rows
remain `acceptance: unassessed`.

| Log | Result |
| --- | --- |
| `round-r04-retest-packet-retests-s0-01d-register-defect-repair.log` | xtask completion tests, 33 and 18 passed, 0 failed |
| `round-r04-retest-packet-retests-s2-03d-python-formatter-proposal.log` | 12 passed, 0 failed |
| `round-r04-retest-packet-retests-s1-02a-native-acceptance-harness.log` | 8 passed, 0 failed |
| `round-r04-gates-fmt.log` | `cargo fmt --all --check`, EXIT=0 |
| `round-r04-gates-check-deps.log` | dependency policy, EXIT=0 |
| `round-r04-gates-docs-hygiene.log` | documentation hygiene, EXIT=0 |
| `round-r04-gates-claim-audit.log` | claim audit, EXIT=0 |
| `round-r04-gates-extract-before-modify.log` | no chokepoint growth, EXIT=0 |
| `round-r04-gates-regress-app.log` | `legion-app` lib, 447 passed, 0 failed |
| `round-r04-gates-regress-desktop.log` | `legion-desktop` lib, 243 passed, 0 failed |
| `round-r04-gates-clippy.log` | workspace all-targets `-D warnings`, EXIT=0 |
| `round-r04-gates-register-verify.log` | `verify-completion-register`, **EXIT=1**, 78 structural issues |

The clippy log records two successive failures before the recorded pass: an
`ok_or_else` holding a constant, then a constant assertion clippy wants as a
`const` block. Both were repaired and the compile-time check that the Python
formatter document bound stays inside the transport limit is preserved.

`verify-completion-register` improves from 144 issues to 78 and still exits 1.
The remainder is the 77 rows that cannot be bound to a scenario honestly and one
product requirement owned by S6, which is a question about that row's `kind` and
is left as an owner decision. No row was accepted and no exemption was added.

The round's gate lane initially reported blocked, not failed, because an
unrelated `cargo test --workspace --locked` from another project was running on
this host. The gates above were re-run by the coordinator at `-j 1` with 10.7 GB
free, which is the mitigation the serialized-Cargo ruling prescribes.

## Round r05 — TypeScript registry pin; residue coverage rejected

One packet passed independent review and was committed
(`s2-01b-typescript-registry-pin`). One packet, `s0-05c-residue-coverage`, was
rejected at review and reverted; its logs are retained below because they were
produced, not because they support anything. A third packet,
`s1-02b-native-acceptance-run`, produced no cargo log at all: it was blocked
before any command could run.

**Classification for every log in this section: component evidence, with four
crate-level integrated test targets. Not packaged evidence, not native GUI
evidence, not product acceptance.** No `acceptance` value in
`plans/completion/requirements.json` was changed from any of these runs; all 419
rows remain `acceptance: unassessed`.

All nineteen logs were copied byte-identical from the gitignored ledger
`.superpowers/sdd/2026-09-04-full-product-completion/`, SHA-256 verified after
each copy.

| Log | Command | Exit | Classification |
| --- | --- | --- | --- |
| `round-r05-check0-legion-lsp.log` | `cargo check -p legion-lsp --all-targets -j 1` | 0 | component evidence — compile check, no test executed |
| `round-r05-test-s2-01b-packet-tests.log` | `cargo test -p legion-lsp --test registry_contract -j 1 --no-fail-fast` | 0 | integrated evidence — crate-level integration target, 15 passed / 0 failed |
| `round-r05-test-s2-01b-clippy.log` | `cargo clippy -p legion-lsp --all-targets -j 1 -- -D warnings` | 0 | component evidence — static lint only |
| `round-r05-test-s2-01b-blast-radius-lsp.log` | `cargo test -p legion-lsp --test rust_analyzer_launch -j 1 --no-fail-fast` | 0 | integrated evidence — crate-level integration target, 1 passed / 0 failed |
| `round-r05-test-s2-01b-blast-radius-app.log` | `cargo test -p legion-app --test javascript_adapter_selection --test typescript_app_startup --test python_app_startup -j 1 --no-fail-fast` | 0 | integrated evidence — 1 passed / 0 failed and **10 ignored, which did not run** (see below) |
| `round-r05-test-round-fmt.log` | `cargo fmt --all --check` | 0 | component evidence — fast gate, silent on success |
| `round-r05-test-s0-05c-packet-tests.log` | `cargo test -p xtask --test completion_command -j 1 --no-fail-fast` | 0 | integrated evidence for a **rejected and reverted** packet, 33 passed / 0 failed; supports nothing |
| `round-r05-test-s0-05c-clippy.log` | `cargo clippy -p xtask --all-targets -j 1 -- -D warnings` | 0 | component evidence for a **rejected and reverted** packet |
| `round-r05-test-s0-05c-verify-completion-register.log` | `cargo run -p xtask -j 1 -- verify-completion-register --root .` | **1** | register validator on the **reverted** tree, 35 structural issues; does not describe the committed tree (see below) |
| `round-r05-fixtest1-fix1-tests-precondition.log` | `powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='cargo.exe' or Name='rustc.exe'\" \| Select-Object ProcessId,Name,CreationDate,CommandLine \| Format-List; Get-Date"` | 0 | cargo-lane precondition observation — host state, not product evidence |
| `round-r05-gates-01-revert-rejected.log` | `git status --porcelain -- plans/completion/scenarios.json plans/completion/requirements.json && ls -la .../s0-05c-residue-coverage-rejected.patch .../s0-05c-residue-coverage-rejected-newfiles/` | 0 | revert verification — housekeeping, not product evidence |
| `round-r05-gates-02a-test-legion-app.log` | `cargo test -p legion-app --lib -j 1` | 0 | component evidence — regression guard, 447 passed / 0 failed |
| `round-r05-gates-02b-test-legion-desktop.log` | `cargo test -p legion-desktop --lib -j 1` | 0 | component evidence — regression guard, 243 passed / 0 failed |
| `round-r05-gates-03-clippy-workspace.log` | `cargo clippy --workspace --all-targets -j 1 -- -D warnings` | 0 | component evidence — workspace lint gate |
| `round-r05-gates-04a-fmt-check.log` | `cargo fmt --all --check` | 0 | component evidence — fast gate, silent on success |
| `round-r05-gates-04b-xtask-check-deps.log` | `cargo run -p xtask -- check-deps` | 0 | component evidence — fast gate, "dependency policy checks passed" |
| `round-r05-gates-04c-xtask-docs-hygiene.log` | `cargo run -p xtask -- docs-hygiene` | 0 | component evidence — fast gate, "documentation hygiene checks passed" |
| `round-r05-gates-04d-xtask-claim-audit.log` | `cargo run -p xtask -- claim-audit` | 0 | component evidence — fast gate, "claim audit passed" |
| `round-r05-gates-04e-xtask-extract-before-modify.log` | `cargo run -p xtask -- extract-before-modify` | 0 | component evidence — fast gate, "no chokepoint file grew past its slack" |

Every fast gate step exited 0. `round-r05-test-round-fmt.log` and
`round-r05-gates-04a-fmt-check.log` are byte-identical
(SHA-256 `8d1e140cf6ec861d9e2b3ba8410ce7d29b7abbe7cc16abc59a153f4597030a2a`)
because `cargo fmt --all --check` prints nothing when it succeeds; both are kept
so each lane's frame stands on its own.

### The ten ignored tests in the blast-radius log did not run

`round-r05-test-s2-01b-blast-radius-app.log` reports `0 passed; 0 failed; 6
ignored` for `typescript_app_startup` and `0 passed; 0 failed; 4 ignored` for
`python_app_startup`, each with the reason `opt-in native Node + retained
TypeScript fixtures` / `opt-in native Node + retained Pyright fixture`. `#[ignore]`
is a compile-time attribute; these targets compiled and executed nothing. The
pinned TypeScript descriptor added this round has therefore launched no language
server, and the TypeScript/JavaScript live workflow has no execution evidence
here. This is an absence of evidence, not a pass and not a skip. Lifting it is
recorded as an owner-blocked prerequisite in `plans/completion/decisions.md`.

### The 35-issue register result is not the committed tree

`round-r05-test-s0-05c-verify-completion-register.log` was produced while the
rejected packet `s0-05c-residue-coverage` was still applied to the working tree.
That work was reverted before any commit, so the 35 figure describes a tree that
no longer exists. The committed tree's last measured value remains the 78
structural issues recorded for round r04. No `verify-completion-register` run in
this round measured the post-revert tree, and no register value was changed on
the strength of the 35.

## Round r06 — in-repo native input driver; register kind repair; pinned-archive extract

Three packets passed independent review and their tests were green:
`s1-02c-native-input-driver`, `s0-05d-register-kind-repair` and
`s2-01c-pinned-archive-extract`. A fourth packet, `ledger-r02-r04-backfill`,
touched only the gitignored local ledger and produced no cargo log and no
committable file.

**Classification for every log in this section: component evidence, with five
crate-level integrated test targets. Not packaged evidence, not native GUI
evidence, not product acceptance.** No `acceptance` value in
`plans/completion/requirements.json` was changed from any of these runs; all 419
rows remain `acceptance: unassessed`.

The five integrated targets are `xtask --test completion_command`,
`xtask --test native_product_acceptance`,
`legion-input-driver --test driver_contract`,
`legion-lsp --test registry_contract` and
`legion-app --test typescript_bundle_drift`. Every one of them runs entirely
in-process against fixtures and temp directories. In particular
`native_product_acceptance` and `driver_contract` exercise the *harness and the
driver's own contract*; **no product window was opened, no OS-level input was
injected into any process, and `xtask native-product-acceptance` was not run in
this round at all.**

All thirty logs were copied byte-identical from the gitignored ledger
`.superpowers/sdd/2026-09-04-full-product-completion/`, SHA-256 verified after
each copy.

| Log | Command | Exit | Classification |
| --- | --- | --- | --- |
| `round-r06-check0-legion-input-driver.log` | `cargo check -p legion-input-driver --all-targets -j 1` | 0 | component evidence — compile check, no test executed |
| `round-r06-check0-legion-app.log` | `cargo check -p legion-app --all-targets -j 1` | 0 | component evidence — compile check, no test executed |
| `round-r06-check0-legion-lsp.log` | `cargo check -p legion-lsp --all-targets -j 1` | 0 | component evidence — compile check, no test executed |
| `round-r06-check0-xtask.log` | `cargo check -p xtask --all-targets -j 1` | 0 | component evidence — compile check, no test executed |
| `round-r06-test-s1-02c-driver-contract.log` | `cargo test -p legion-input-driver --test driver_contract -j 1 --no-fail-fast` | 0 | integrated evidence — crate-level integration target, 8 passed / 0 failed; asserts the driver's own exit and report contract, injects nothing |
| `round-r06-test-s1-02c-xtask-native-product-acceptance.log` | `cargo test -p xtask --test native_product_acceptance -j 1 --no-fail-fast` | 0 | integrated evidence — crate-level integration target, 10 passed / 0 failed; harness behaviour against fixtures, no product launched |
| `round-r06-test-s1-02c-clippy-legion-input-driver.log` | `cargo clippy -p legion-input-driver --all-targets -j 1 -- -D warnings` | **101** | component evidence — **failed**: `unused import: windows_uia::*` in the `driver_contract` target. Repaired in the fix pass; superseded by `round-r06-retest-s1-02c-clippy-legion-input-driver.log` |
| `round-r06-test-s1-02c-s0-05d-clippy-xtask.log` | `cargo clippy -p xtask --all-targets -j 1 -- -D warnings` | **101** | component evidence — **failed**: `assertions_on_constants` in the `native_product_acceptance` target. Repaired in the fix pass; superseded by `round-r06-retest-clippy-xtask.log` |
| `round-r06-test-s0-05d-completion-command.log` | `cargo test -p xtask --test completion_command -j 1 --no-fail-fast` | 0 | integrated evidence — crate-level integration target, 33 passed / 0 failed; synthetic registers in temp directories, not the repository register |
| `round-r06-test-s0-05d-named-filter-check.log` | `cargo test -p xtask --test completion_command -j 1 --no-fail-fast -- verify_completion_register` | 0 | filter probe — 0 passed / 0 failed / **33 filtered out**; this exit 0 means *no test matched the name*, not that anything ran |
| `round-r06-test-s2-01c-lsp-registry-contract.log` | `cargo test -p legion-lsp --test registry_contract -j 1 --no-fail-fast` | 0 | integrated evidence — crate-level integration target, 17 passed / 0 failed |
| `round-r06-test-s2-01c-app-typescript-bundle-drift.log` | `cargo test -p legion-app --test typescript_bundle_drift -j 1 --no-fail-fast` | 0 | integrated evidence — crate-level integration target, 2 passed / 0 failed; new drift guard, green-on-agreement only (its red path was never exercised) |
| `round-r06-test-s2-01c-clippy-legion-lsp.log` | `cargo clippy -p legion-lsp --all-targets -j 1 -- -D warnings` | 0 | component evidence — static lint only |
| `round-r06-test-s2-01c-clippy-legion-app.log` | `cargo clippy -p legion-app --all-targets -j 1 -- -D warnings` | 0 | component evidence — static lint only |
| `round-r06-test-round-fmt-check.log` | `cargo fmt --all --check` | 0 | component evidence — fast gate, silent on success |
| `round-r06-retest-s1-02c-clippy-legion-input-driver.log` | `cargo clippy -p legion-input-driver --all-targets -j 1 -- -D warnings` | 0 | component evidence — fix-pass retest of the 101 above |
| `round-r06-retest-s1-02c-named-tests.log` | `cargo test -p legion-input-driver -p xtask --test driver_contract --test native_product_acceptance -j 1 --no-fail-fast` | 0 | integrated evidence — 8 passed / 0 failed and 10 passed / 0 failed after the fix pass |
| `round-r06-retest-clippy-xtask.log` | `cargo clippy -p xtask --all-targets -j 1 -- -D warnings` | 0 | component evidence — fix-pass retest of the 101 above |
| `round-r06-retest-s0-05d-completion-command.log` | `cargo test -p xtask --test completion_command -j 1 --no-fail-fast` | 0 | integrated evidence — 33 passed / 0 failed after the fix pass |
| `round-r06-retest-s0-05d-named-filter-check.log` | `cargo test -p xtask --test completion_command -j 1 --no-fail-fast -- verify_completion_register` | 0 | filter probe — 0 passed / 0 failed / 33 filtered out; again, nothing ran |
| `round-r06-retest-round-fmt-check.log` | `cargo fmt --all --check` | 0 | component evidence — fast gate, silent on success |
| `round-r06-fixtest1-fmt.log` | `cargo fmt --all --check` | 0 | component evidence — fast gate, silent on success |
| `round-r06-gates-02a-test-legion-app.log` | `cargo test -p legion-app --lib -j 1` | 0 | component evidence — regression guard, 447 passed / 0 failed |
| `round-r06-gates-02b-test-legion-desktop.log` | `cargo test -p legion-desktop --lib -j 1` | 0 | component evidence — regression guard, 243 passed / 0 failed |
| `round-r06-gates-03-clippy-workspace.log` | `cargo clippy --workspace --all-targets -j 1 -- -D warnings` | 0 | component evidence — workspace lint gate |
| `round-r06-gates-04a-fmt-check.log` | `cargo fmt --all --check` | 0 | component evidence — fast gate, silent on success |
| `round-r06-gates-04b-xtask-check-deps.log` | `cargo run -p xtask -- check-deps` | 0 | component evidence — fast gate, "dependency policy checks passed" |
| `round-r06-gates-04c-xtask-docs-hygiene.log` | `cargo run -p xtask -- docs-hygiene` | 0 | component evidence — fast gate, "documentation hygiene checks passed" |
| `round-r06-gates-04d-xtask-claim-audit.log` | `cargo run -p xtask -- claim-audit` | 0 | component evidence — fast gate, "claim audit passed" |
| `round-r06-gates-04e-xtask-extract-before-modify.log` | `cargo run -p xtask -- extract-before-modify` | 0 | component evidence — fast gate, "no chokepoint file grew past its slack" |

Every fast gate step exited 0, read from the logs' own `EXIT=` frames:
`cargo fmt --all --check` silent, `check-deps`, `docs-hygiene`, `claim-audit`
and `extract-before-modify` each printing their pass line, workspace `clippy`
under `-D warnings` at 0, and the two regression guards at 447 and 243 passed
with 0 failed. The workspace clippy log's only warning is the vendored `epaint`
`falling back to f32` future-incompatibility, which is outside the workspace
lint failure and unchanged from earlier rounds.

Four logs share SHA-256
`8d1e140cf6ec861d9e2b3ba8410ce7d29b7abbe7cc16abc59a153f4597030a2a`
(`round-r06-test-round-fmt-check.log`, `round-r06-fixtest1-fmt.log`,
`round-r06-retest-round-fmt-check.log`, `round-r06-gates-04a-fmt-check.log`)
because `cargo fmt --all --check` prints nothing when it succeeds. Each is kept
so that every lane's frame stands on its own, and each carries the same hash the
r05 fmt logs carry for the same reason.

### The two exit-101 logs are retained failures, not hidden ones

`round-r06-test-s1-02c-clippy-legion-input-driver.log` and
`round-r06-test-s1-02c-s0-05d-clippy-xtask.log` both end `EXIT=101`. They are
the first packet-lane clippy runs of the round; both failures are mechanical
lints in *test* targets (`unused import: windows_uia::*`, and
`assertions_on_constants`), both were repaired in the fix pass, and both retests
exit 0. The failing logs are kept because they happened.

### No `verify-completion-register` run measured this tree

No `round-r06-*verify-completion-register*` log exists. The register validator
was not run against the repository register in this round, so the last figure
measured on a tree that exists is still the **78 structural issues of round
r04**. The `s0-05d-register-kind-repair` report predicts 3 remaining issues
after its edits, and an independently written port of
`validate_register_structure` reproduced 78-at-HEAD and 3-on-tree during review,
but neither is a `verify-completion-register` exit code and neither is recorded
here as a measurement. Closing it needs exactly:
`cargo run -p xtask -- verify-completion-register --root .` run from
`D:/legion-ide-completion` by the cargo lane, logged with its own frame.

### Four verification commands named by a brief have no r06 log

The `s2-01c-pinned-archive-extract` brief named eight verification commands;
four produced no log this round:
`cargo test -p legion-lsp --test rust_analyzer_launch`,
`cargo test -p legion-app --test javascript_adapter_selection`,
`cargo test -p legion-app --lib` as that packet's named blast-radius guard, and
`cargo run -p xtask -- extract-before-modify` attributed to that packet. The
`--all-targets` check and clippy runs prove those targets *compile*; they do not
prove they *pass*. `cargo test -p legion-app --lib` and `extract-before-modify`
did both run at round-gate level (`round-r06-gates-02a-test-legion-app.log`,
`round-r06-gates-04e-xtask-extract-before-modify.log`, both exit 0) on the tree
containing all three packets, which covers the regression question but is not a
per-packet attribution. The two `--test` targets ran in neither place.

### The native input driver was built but never used to drive anything

`crates/legion-input-driver` is new in this round and its contract tests pass.
That is a driver *in the workspace*, not a driver *run*. On this host
`target/debug/legion-input-driver.exe` exists only as a side effect of the cargo
lane's own test builds; no `target/release/legion-input-driver.exe` exists, no
`target/native-input-acceptance/package/` directory exists, and
`xtask native-product-acceptance` was not invoked. `BLK-2026-09-08-04` (a
packaged native Legion product staged into the package directory) still blocks
every run. `COMP-PLAT-002` stays `implementation: partial`,
`acceptance: unassessed`.

Before any future `native-product-acceptance` artifact is transcribed anywhere:
the harness currently writes `status = "conformance-failed"`, `exit_code = 1`
for **any** post-driver outcome that is not `driver_code == 0 && window_created
&& 6/6 conforms` — including the driver's own exit `3`/blocked, which is what
this host is expected to produce, because `observe_ime_cjk` blocks whenever the
product window's keyboard layout is not CJK. Until a follow-up propagates the
driver's blocked exit into a blocked harness status, **no `conformance-failed`
artifact from this command may be entered into `plans/completion/defects.json`
or into any requirement row**, because it would be blaming the product for a
missing host prerequisite.

## Round r07 — blocked-outcome propagation; final register closure; package staging instrument

Three packets passed independent review with green tests:
`s1-02d-blocked-outcome-propagation`, `s0-05e-final-register-closure` and
`s1-02e-package-the-product` (the last after one fix pass). No packet was
rejected and no packet's tests failed.

**Classification for every log in this section: component evidence, with three
crate-level integrated test targets and one PowerShell contract suite. Not
packaged evidence, not native GUI evidence, not product acceptance.** No
`acceptance` value in `plans/completion/requirements.json` was changed from any
of these runs; all 419 rows remain `acceptance: unassessed`.

The three integrated targets are `xtask --test native_product_acceptance`,
`legion-input-driver --test driver_contract` and
`xtask --test completion_command`. Each runs entirely in-process against
fixtures and temp directories. The PowerShell suite
`scripts/test-native-package-verifiers.ps1` runs against synthetic fixtures with
no real installer, no `msiexec` and no product build. **No product window was
opened, no OS-level input was injected into any process, `xtask
native-product-acceptance` was not invoked, no MSI was built, and nothing was
staged into `target/native-input-acceptance/package/`.**

All nineteen logs were copied byte-identical from the gitignored ledger
`.superpowers/sdd/2026-09-04-full-product-completion/`, SHA-256 verified after
each copy.

| Log | Command | Exit | Classification |
| --- | --- | --- | --- |
| `round-r07-check0-legion-input-driver.log` | `cargo check -p legion-input-driver --all-targets -j 1` | 0 | component evidence — compile check, no test executed |
| `round-r07-check0-xtask.log` | `cargo check -p xtask --all-targets -j 1` | 0 | component evidence — compile check, no test executed |
| `round-r07-test-s1-02d-xtask-native-product-acceptance.log` | `cargo test -p xtask --test native_product_acceptance -j 1 --no-fail-fast` | 0 | integrated evidence — crate-level integration target, 19 passed / 0 failed (10 in r06); harness outcome mapping against fixture driver results, no product launched |
| `round-r07-test-s1-02d-legion-input-driver-driver-contract.log` | `cargo test -p legion-input-driver --test driver_contract -j 1 --no-fail-fast` | 0 | integrated evidence — crate-level integration target, 11 passed / 0 failed (8 in r06); asserts the driver's own exit and report contract, injects nothing |
| `round-r07-test-s1-02d-clippy-legion-input-driver.log` | `cargo clippy -p legion-input-driver --all-targets -j 1 -- -D warnings` | 0 | component evidence — static lint only |
| `round-r07-test-s1-02d-clippy-xtask.log` | `cargo clippy -p xtask --all-targets -j 1 -- -D warnings` | 0 | component evidence — static lint only |
| `round-r07-test-s0-05e-xtask-completion-command.log` | `cargo test -p xtask --test completion_command -j 1 --no-fail-fast` | 0 | integrated evidence — crate-level integration target, 33 passed / 0 failed; synthetic registers in temp directories, **not** the repository register |
| `round-r07-test-s1-02e-native-package-verifier-tests.log` | `pwsh -NoProfile -File scripts/test-native-package-verifiers.ps1` | 0 | integrated evidence — PowerShell contract suite, `passed=21 failed=0 skipped=0`; synthetic fixtures only, no installer, no `msiexec`, no product build |
| `round-r07-test-round-fmt-check.log` | `cargo fmt --all --check` | 0 | component evidence — fast gate, silent on success |
| `round-r07-fixtest1-stage-script-contract-tests.log` | `pwsh -NoProfile -File scripts/test-native-package-verifiers.ps1` | 0 | integrated evidence — fix-pass retest, `passed=21 failed=0 skipped=0` |
| `round-r07-fixtest1-cargo-fmt-check.log` | `cargo fmt --all --check` | 0 | component evidence — fix-pass retest, silent on success |
| `round-r07-gates-02a-test-legion-app.log` | `cargo test -p legion-app --lib -j 1` | 0 | component evidence — regression guard, 447 passed / 0 failed |
| `round-r07-gates-02b-test-legion-desktop.log` | `cargo test -p legion-desktop --lib -j 1` | 0 | component evidence — regression guard, 243 passed / 0 failed |
| `round-r07-gates-03-clippy-workspace.log` | `cargo clippy --workspace --all-targets -j 1 -- -D warnings` | 0 | component evidence — workspace lint gate |
| `round-r07-gates-04a-fmt-check.log` | `cargo fmt --all --check` | 0 | component evidence — fast gate, silent on success |
| `round-r07-gates-04b-xtask-check-deps.log` | `cargo run -p xtask -- check-deps` | 0 | component evidence — fast gate, "dependency policy checks passed" |
| `round-r07-gates-04c-xtask-docs-hygiene.log` | `cargo run -p xtask -- docs-hygiene` | 0 | component evidence — fast gate, "documentation hygiene checks passed" |
| `round-r07-gates-04d-xtask-claim-audit.log` | `cargo run -p xtask -- claim-audit` | 0 | component evidence — fast gate, "claim audit passed" |
| `round-r07-gates-04e-xtask-extract-before-modify.log` | `cargo run -p xtask -- extract-before-modify` | 0 | component evidence — fast gate, "no chokepoint file grew past its slack" |

**Every one of the nineteen logs ends `EXIT=0`.** Every fast gate step exited 0,
read from the logs' own `EXIT=` frames: `cargo fmt --all --check` silent,
`check-deps`, `docs-hygiene`, `claim-audit` and `extract-before-modify` each
printing their pass line, and workspace `clippy` under `-D warnings` at 0. The
workspace clippy log's only warning is the vendored `epaint` "falling back to
f32" future-incompatibility, which is outside the workspace lint failure and
unchanged from earlier rounds. Regression guards hold at 447 and 243 passed with
0 failed, the same figures as r05 and r06.

Three logs share SHA-256
`8d1e140cf6ec861d9e2b3ba8410ce7d29b7abbe7cc16abc59a153f4597030a2a`
(`round-r07-test-round-fmt-check.log`, `round-r07-fixtest1-cargo-fmt-check.log`,
`round-r07-gates-04a-fmt-check.log`) because `cargo fmt --all --check` prints
nothing when it succeeds. Each is kept so that every lane's frame stands on its
own, and each carries the same hash the r05 and r06 fmt logs carry for the same
reason.

### The two stage-script suite logs are indistinguishable apart from a temp path

`round-r07-test-s1-02e-native-package-verifier-tests.log` and
`round-r07-fixtest1-stage-script-contract-tests.log` differ on exactly one line:
the `fixtures:` temp directory name. Same twenty-one test names, same
`passed=21 failed=0 skipped=0`. The fix pass strengthened assertions **inside**
three existing tests — making their "no destination was created" oracles real by
dropping `-DryRun` — and added no test and renamed none, so the retest log is
not by itself evidence of what the fix changed. It is evidence that the suite
still passes after it. Five of the eight staging tests still assert
`-not (Test-Path $destination)` after a `-DryRun` run, where a successful run
would not have created the destination either; those particular oracles remain
vacuous and are carried forward as coverage debt.

### No package was built and nothing was staged

`s1-02e-package-the-product` is titled for a package build, and no package was
built. `scripts/package-native.ps1` runs `cargo build --release -p legion-desktop`
and `cargo packager`, so it is a cargo command, and the implementer role may not
run one; the lane was occupied when the packet ran. What the packet produced is
the **staging instrument** — `scripts/stage-native-acceptance-package.ps1`, its
eight contract tests, and a runbook paragraph — plus
`plans/evidence/native-input-acceptance/2026-09-08-unsigned-local-package.md`,
which states in its own opening that no MSI was built and nothing was staged.

`BLK-2026-09-08-04` is therefore **not retired**:
`target/native-input-acceptance/package/legion-desktop.exe` is still absent and
still blocks every harness run. `COMP-PLAT-002` stays
`implementation: partial`, `acceptance: unassessed`.

Two guarantees the staging script does *not* provide, both carried forward: it
never reads `VALIDATION-SUMMARY.toml`, so "nothing may be staged from an
unverified MSI" remains procedural text in the runbook rather than a mechanical
precondition; and `STAGING-EVIDENCE.toml` records `source_msi` and
`source_msi_sha256` beside a payload the script never proves came from that MSI,
because the MSI-to-payload link is operator-asserted through `-StagingSource`.
The `msiexec /a` extraction layout is itself unexercised — whether it yields
exactly one `legion-desktop.exe` is unknown until an MSI exists.

### The harness still has not been run

`s1-02d-blocked-outcome-propagation` changes what
`xtask native-product-acceptance` would publish for a driver that exits blocked:
`status = "blocked"`, `exit_code = 3`, carrying the driver's own composed
prerequisite, instead of `status = "conformance-failed"`, `exit_code = 1`. It
also adds `input_classes_blocked` and `input_classes_deviating` to the artifact.
Nineteen harness tests and eleven driver tests assert that mapping against
fixture driver results. **None of them ran the harness against a product.** The
standing constraint recorded in round r06 is what this packet was built to
retire, and it is retired *in the code*, not by any run: no
`native-product-acceptance` artifact of any status exists on this host.

Two asymmetries in that mapping survive the packet and are carried forward. The
pass arm does not consult the two new fields, so a driver result emitting both
`conforms` and `blocked` for one class would still satisfy
`every_class_observed` and publish `passed` beside a non-empty
`input_classes_blocked`. And the conformance-failed arm has no `stated_status`
guard to match the pass arm's `stated_status_denies_pass`, so a driver exiting 1
while its own result says `status = "blocked"` would be published as a product
deviation. Both are unreachable from today's driver, which pushes one
observation per class and renders `conformance_status` and
`conformance_exit_code` from the same report.

### `verify-completion-register` was not measured on this tree

There is no `round-r07-*verify-completion-register*` log. `s0-05e`'s success
criterion — exit 0 with zero structural issues — is therefore **predicted, not
measured**. The last measured figure is **3 structural issues, exit 1**, from
`round-r06-post-register-verify.log`, which lives only in the gitignored ledger
and was never committed; its three issues are exactly the three `s0-05e`
addresses (`COMP-DIST-010` product row owned by S6, `COMP-P1-F1-T3-1` and
`COMP-TRAIN-009` empty coverage). Two independent Node ports of
`validate_register_structure`, written separately by the implementer and the
reviewer, both calibrate to 3 at that commit and report 0 on this tree. Neither
is an exit code. Exact prerequisite to close it: the cargo lane runs
`cargo run -p xtask -- verify-completion-register --root .` from
`D:/legion-ide-completion` with
`.superpowers/sdd/2026-09-04-full-product-completion/cargo.lock-owner` absent and
no `cargo.exe`/`rustc.exe` running, logs it with a CMD/CWD/BEGIN/END/EXIT frame,
and a later record cites the measured issue list and exit code in place of this
prediction.

### Register effect of round r07: none in `implementation` or `acceptance`

All eight reviewer-proposed implementation transitions were applied as written
and **all eight were verified no-ops** — each row already held the proposed
value. Measured against `HEAD` (`21dfc0b`), `plans/completion/requirements.json`
changed on 11 fields across 6 rows, none of them `implementation` and none of
them `acceptance`: `kind`, `scenario_ids`, `configuration_ids` and
`protected_product_ids` on `COMP-P1-F1-T3-1`; `protected_product_ids` on
`COMP-P0-F3-T1-1`, `-T2-1` and `-T3-1`; `scenario_ids` and `configuration_ids`
on `COMP-TRAIN-009`; and `stage`/`package_id` on `COMP-DIST-010`. 419 rows
before and after, 143 implemented / 233 partial / 43 absent, all 419
`acceptance: unassessed`. The product-row count moves 357 to 356 because
`COMP-P1-F1-T3-1` is re-kinded to `internal`.

## Round r10 — three hosted-CI failure repairs (2026-09-09)

Round r10 ran three packets, all of which passed implementation, review and the
serialized cargo lane. All three exist to repair `cargo test` failures observed
in the hosted `Legion Gates` run `34322199039` (measured below, not quoted from a brief); none of them changes product
behaviour, and all three are test-side or test-fixture repairs.

| Packet | Change | Owned code path |
| --- | --- | --- |
| `s2-03e-typescript-organize-path-shape` | Fix the TypeScript organize-imports test expectation that compared the product's normalised path against a non-canonical fixture path | `crates/legion-app/src/language/typescript_organize_tests.rs` |
| `s1-08a-streamed-worker-mailbox-flake` | Make the streamed-layout worker admission test wait on the condition instead of a 5ms sleep before a 64-deep bounded mailbox | `crates/legion-desktop/src/view/streamed_layout/worker.rs` |
| `s1-04m-snapshot-lease-expiry` | Give the retention test's hand-built snapshot lease a real TTL so the lease-pinned descriptor survives the sweep on every host | `crates/legion-editor/src/lib.rs` |

### Logs

Every log below carries a `CMD:` / `CWD:` / `BEGIN` / `END` / `EXIT=` frame
written by the round runner, and was copied byte-identical from
`.superpowers/sdd/2026-09-04-full-product-completion/` (the gitignored ledger)
into this directory; `cmp` reports zero differences on all 22 files. Two further
logs, `round-r10-record-gh-*.log`, were written by the record role in this step
and carry the same frame.

**Every one of the 24 logs exited 0.** No step in this round is red, blocked or
skipped. Twenty-two are cargo-lane logs; two are `gh` observations taken by the
record role.

| Log | Command | Exit | Classification |
| --- | --- | --- | --- |
| `round-r10-check0-legion-app.log` | `cargo check -p legion-app --all-targets -j 1` | 0 | component evidence — pre-implementation compile baseline |
| `round-r10-check0-legion-desktop.log` | `cargo check -p legion-desktop --all-targets -j 1` | 0 | component evidence — pre-implementation compile baseline |
| `round-r10-check0-legion-editor.log` | `cargo check -p legion-editor --all-targets -j 1` | 0 | component evidence — pre-implementation compile baseline |
| `round-r10-test-packet-tests-s2-03e.log` | `cargo test -p legion-app --lib language::typescript_organize_tests -j 1 --no-fail-fast` | 0 | component evidence — packet-focused unit tests, 6 passed / 0 failed / 441 filtered out |
| `round-r10-test-packet-tests-s2-03e-full-lib.log` | `cargo test -p legion-app --lib -j 1 --no-fail-fast` | 0 | component evidence — crate regression guard, 447 passed / 0 failed / 0 ignored |
| `round-r10-test-packet-tests-s2-03e-clippy.log` | `cargo clippy -p legion-app --all-targets -j 1 -- -D warnings` | 0 | component evidence — crate lint gate |
| `round-r10-test-packet-tests-s1-08a.log` | `cargo test -p legion-desktop --lib view::streamed_layout::worker -j 1 --no-fail-fast` | 0 | component evidence — packet-focused unit tests, 4 passed / 0 failed / 239 filtered out |
| `round-r10-test-packet-tests-s1-08a-soak.log` | `sh .superpowers/sdd/2026-09-04-full-product-completion/soak-s1-08a.sh` | 0 | component evidence — flake soak: 20 idle then 10 loaded (12 busy shells) repetitions of the packet-focused unit tests, `TOTAL: idle_pass=20 idle_fail=0 loaded_pass=10 loaded_fail=0` |
| `round-r10-test-packet-tests-s1-08a-full-lib.log` | `cargo test -p legion-desktop --lib -j 1 --no-fail-fast` | 0 | component evidence — crate regression guard, 243 passed / 0 failed / 0 ignored |
| `round-r10-test-packet-tests-s1-08a-clippy.log` | `cargo clippy -p legion-desktop --all-targets -j 1 -- -D warnings` | 0 | component evidence — crate lint gate |
| `round-r10-test-packet-tests-s1-04m.log` | `cargo test -p legion-editor --lib -j 1 --no-fail-fast` | 0 | component evidence — crate unit tests, 59 passed / 0 failed / 0 ignored |
| `round-r10-test-packet-tests-s1-04m-atomicity.log` | `cargo test -p legion-editor --test atomicity_and_retention -j 1 --no-fail-fast` | 0 | component evidence — single-crate integration test binary against the crate public API, 8 passed / 0 failed |
| `round-r10-test-packet-tests-s1-04m-clippy.log` | `cargo clippy -p legion-editor --all-targets -j 1 -- -D warnings` | 0 | component evidence — crate lint gate |
| `round-r10-test-packet-tests-round-fmt.log` | `cargo fmt --all --check` | 0 | component evidence — formatting gate, silent on success |
| `round-r10-gates-step2a-legion-app-lib-tests.log` | `cargo test -p legion-app --lib -j 1` | 0 | component evidence — post-merge regression guard, 447 passed / 0 failed / 0 ignored |
| `round-r10-gates-step2b-legion-desktop-lib-tests.log` | `cargo test -p legion-desktop --lib -j 1` | 0 | component evidence — post-merge regression guard, 243 passed / 0 failed / 0 ignored |
| `round-r10-gates-step3-workspace-clippy.log` | `cargo clippy --workspace --all-targets -j 1 -- -D warnings` | 0 | component evidence — workspace lint gate |
| `round-r10-gates-step4a-cargo-fmt-check.log` | `cargo fmt --all --check` | 0 | component evidence — fast gate, silent on success |
| `round-r10-gates-step4b-xtask-check-deps.log` | `cargo run -p xtask -- check-deps` | 0 | component evidence — fast gate, "dependency policy checks passed" |
| `round-r10-gates-step4c-xtask-docs-hygiene.log` | `cargo run -p xtask -- docs-hygiene` | 0 | component evidence — fast gate, "documentation hygiene checks passed" |
| `round-r10-gates-step4d-xtask-claim-audit.log` | `cargo run -p xtask -- claim-audit` | 0 | component evidence — fast gate, "claim audit passed" |
| `round-r10-gates-step4e-xtask-extract-before-modify.log` | `cargo run -p xtask -- extract-before-modify` | 0 | component evidence — fast gate, "no chokepoint file grew past its slack" |
| `round-r10-record-gh-run-list.log` | `gh run list --branch codex/full-product-resume --workflow "Legion Gates" --limit 30` | 0 | hosted-CI observation — nine runs listed, all nine `completed / failure` |
| `round-r10-record-gh-run-34322199039-failed-tests.log` | `gh run view 34322199039 --json jobs` then `gh run view 34322199039 --job <id> --log-failed` per failing job | 0 | hosted-CI observation — per-job failing test names for the run the three packets target |

### Classification, stated plainly

All 22 cargo-lane r10 logs are **component evidence**. Not one of them is
integrated evidence, packaged-product evidence, native-GUI evidence, or product
acceptance. The two `round-r10-record-gh-*.log` files are neither: they are
**hosted-CI observations** — a record of what GitHub-hosted runners reported,
which the standing rule already classifies as a component-layer workspace gate
and never as packaged-product acceptance:

- Every command is a `cargo` workspace command run against source in
  `D:/legion-ide-completion`. Nothing was packaged, installed, signed or
  launched.
- No window was opened and no OS input was synthesised, so nothing here bears on
  the native-GUI rows.
- `round-r10-test-packet-tests-s1-04m-atomicity.log` runs a `tests/` binary, but
  it links one crate (`legion-editor`) and drives its public API in-process; it
  is a crate-level test, not an integration of the product parts.
- The soak log is 30 repetitions of the same in-process unit tests on this
  Windows host. It measures this host flake rate, not the hosted runners.
- No `acceptance` field in `plans/completion/requirements.json` moves on this
  evidence, and none was touched.

### Register effect of round r10: none

All four reviewer-proposed implementation transitions were applied as written
and **all four were verified no-ops** — each row already held the proposed
value: `COMP-LANG-007` (`partial`), `COMP-P0-F4-T5-1` (`partial`),
`COMP-P1-F4-T2-1` (`implemented`), `COMP-PRES-007` (`partial`).
`plans/completion/requirements.json` is byte-identical before and after: 419
rows before, 419 rows after; 143 implemented / 233 partial / 43 absent, both
before and after; all 419 `acceptance: unassessed`, both before and after. This
is expected — the three packets repair tests, they do not add product
capability, so no row earns a promotion from them.

### What round r10 actually targets, measured this round

The record role ran `gh` and read run `34322199039` directly rather than
restating the packet briefs. Measured:

- **Nine `Legion Gates` runs on this branch, all nine `completed / failure`**,
  from `34235634538` (2026-09-08T14:01:38Z) to `34322199039`
  (2026-09-09T07:06:27Z). The nine-failed-runs figure the briefs quote is
  confirmed.
- Run `34322199039` has four jobs: `cargo-deny` success, and **all three**
  `Standing gates` jobs failed — `windows-latest` (`102371151604`),
  `ubuntu-latest` (`102371151608`) and `macos-latest` (`102371151801`).
- The failures map to the packets like this, and **the map is not the one the
  round brief describes**:

| Job | Failing test | Packet |
| --- | --- | --- |
| `windows-latest` | `language::typescript_organize_tests::typescript_organize_uses_file_and_all_mode_without_candidates`, panic at `typescript_organize_tests.rs:112:5`; 446 passed / 1 failed | `s2-03e` |
| `macos-latest` | the same test, same panic site; 450 passed / 1 failed | `s2-03e` |
| `ubuntu-latest` | `view::streamed_layout::worker::tests::worker_admission_is_bounded_and_cancel_rejects_late_output`, panic at `worker.rs:659:18`; 241 passed / 1 failed | `s1-08a` |
| `ubuntu-latest` | `tests::retention_drained_prefix_releases_each_unpinned_descriptor_and_preserves_lease`, panic at `legion-editor/src/lib.rs:4573:9`; 56 passed / 1 failed | `s1-04m` |

Two corrections follow from that, and they are recorded rather than smoothed
over:

1. The round brief frames all three packets as repairs to `macos-latest` and
   `ubuntu-latest` failures. **The TypeScript failure is on `windows-latest`
   and `macos-latest`, not on `ubuntu-latest`** — `ubuntu-latest` ran
   `legion-app --lib` at 451 passed / 0 failed. The branch is red on all three
   operating systems, not two.
2. Because the `s2-03e` failure reproduces on a hosted `windows-latest` runner,
   the standing claim that these repairs "cannot be falsified on this Windows
   host" is **true only of `s1-08a` and `s1-04m`**. For `s2-03e` this host
   simply does not reproduce a failure another Windows host does: 447 tests
   here, 447 on `windows-latest` (446 passed + 1 failed), same count, different
   outcome. That difference is unexplained by anything measured this round and
   is the most important open question the round leaves behind.

### What round r10 does not establish

- **None of the three repairs is observed to fix anything.** All were validated
  only on this Windows host, where none of the three target failures reproduces.
  Exact prerequisite: a completed `Legion Gates` run on branch
  `codex/full-product-resume` after these commits land, with `windows-latest`,
  `ubuntu-latest` and `macos-latest` all green.
- Why this Windows host passes
  `typescript_organize_uses_file_and_all_mode_without_candidates` while the
  hosted `windows-latest` runner fails it at the same panic site was not
  determined. Until it is, `s2-03e` is a plausible repair, not a diagnosed one.
- The strongest local falsification for `s2-03e` — re-running the single test
  with `TMP`/`TEMP` pointed at the 8.3 short-name alias of the temp root — was
  offered as optional in the brief and was **not attempted**. It is recorded as
  not attempted, not as passed. Given the `windows-latest` finding above it
  should no longer be treated as optional.
- The gates sequence has no `legion-editor` step; that crate's coverage this
  round comes from the three `s1-04m` packet logs above, which ran on the same
  merged tree.
- These logs were placed in this `2026-09-08` directory by explicit round
  instruction. Rounds r08 and r09 filed their logs under
  `plans/evidence/full-product-resume-2026-09-09/`, so r10 evidence is not
  co-located with the immediately preceding rounds.
