# 2026-10 release-readiness audit

**Verdict: NO-GO.**

This tip is not general availability and not an invitation-only preview that implies a signed, zero-egress, accessible installed product. Substrate gates that do not require signing mostly pass on this Linux VM. `cargo deny` and `clippy -D warnings` are red on Rust 1.99.0. The candidate is missing `main`'s host-qualified `file://` remote gate and the current advisory pins. Signing, an HTTP updater, a Manual offline SKU, and macOS VoiceOver / Linux Orca proof remain open.

Audited tip: `autoresearch/review-the-technology-and-features-of-this-tool-20261002` at `0a27cebd939395f27b3a3d586ecb7f92b7622433` (2026-10-07). Toolchain: `rustc 1.99.0 (b940084d7 2026-09-28)`, `cargo-deny 0.20.2`, `cargo-packager 0.11.8`. Machine: Linux x86_64, 4 CPUs, about 15 GiB RAM, no swap. Compiles used `-j 2`.

This audit changes documentation only. No product behavior was edited. No ledger row was promoted. `REVIEW.md` is the Ponytail review process; it contains no product findings to revalidate. Prior IDs that still match the tree are folded into `findings.json` (`GAP-*`, `RCA-2026-08-13`).

Counts: **P0 6, P1 9, P2 10, P3 2** (27 findings, `RR-2026-001` through `RR-2026-027`).

## Follow-up as of 2026-10-08

The observations above are the 2026-10-07 tip. #226 through #231 have since merged to `main`. This section does not rewrite those observations and does not promote a ledger row. The verdict on the audited tip stays **NO-GO**. What remains after those merges is still NO-GO: signing, VoiceOver, Orca, Windows sandbox probes, MSI/DMG, a Linux perf baseline, a clean-VM capture, and an owner-provided production update feed.

| Finding | Follow-up |
| --- | --- |
| RR-2026-004, RR-2026-009 | #226 merged at `3344d1f`. It brings this tip into `main` and keeps #220. |
| RR-2026-003, RR-2026-007 | #227 merged into the #226 branch at `2471c90` and reached `main` with #226. wasmtime is 48.0.5 and clippy `-D warnings` is clear. |
| RR-2026-008, RR-2026-010, RR-2026-012, RR-2026-013, RR-2026-018, RR-2026-020, RR-2026-022, RR-2026-026 | #228 merged at `985d012`. Documentation only. No ledger promotion. |
| RR-2026-011, RR-2026-016, RR-2026-017 | #229 merged at `84ccb5c`. IDE commits do not inherit `commit.gpgsign`, git config is isolated for remote policy, and search walk errors are partial. |
| RR-2026-005 (provider stack), RR-2026-015, RR-2026-019, RR-2026-024, RR-2026-025, RR-2026-027 | #230 merged at `e5e3e35`. The offline desktop feature does not link `legion-ai-providers`, the deb metadata declares `libxkbcommon-x11-0`, a Manual local beta path stays in Manual, hosted telemetry is denied in every mode, the Maintainer is still a placeholder, and packager config is isolated. A rebuilt deb was not inspected. |
| RR-2026-002, RR-2026-005 (`reqwest`) | #231 merged at `19aca63`. Signed HTTP fetch, hash check, swap, launch, and N-1 rollback are in tree. Manual mode does not poll. The production URL and verifying key stay owner config. The offline package does not link `reqwest`. |
| RR-2026-001, RR-2026-006, RR-2026-014, RR-2026-021, RR-2026-023 | Still open. Signing certificates, VoiceOver, Orca, Windows sandbox, MSI/DMG, and perf baselines need a human and the other OS. |

## Branch merge assessment

`git rev-list --left-right --count origin/main...HEAD` is **6 behind, 10 ahead**. Merge-base `1a9264ebced087192f1ef980609181fcf942ad33`. The brief that said the branch was 2 commits behind `main` is stale.

The tip is **not fit to release by itself** (`RR-2026-009`, `RR-2026-004`). It is a reasonable merge **into `main`** after one documentation conflict, because product code does not revert `#220`.

`main` commits absent from this tip:

| Commit | What it adds |
| --- | --- |
| `cc61c76` | E2E testing catalog (`#216`) |
| `84919ba` | Editor and language completion plan (`#217`) |
| `32b6c67` | Completion rounds r00–r07, evidence only (`#218`) |
| `e0a1277` | Host-qualified `file://` remotes are network targets (`#220`) |
| `b2a0140` | GAP-01 windowed report is not native input; wasmtime 48.0.3 (`#222`) |
| `99ab134` | Docs stop citing windowed E2E as native input (`#223`) |

`origin/main` pins wasmtime 48.0.3 and rustls 0.23.45. That clears `RUSTSEC-2026-0285` and `RUSTSEC-2026-0316`. `RUSTSEC-2026-0327` still needs wasmtime `>=48.0.4`. `main`'s `deny.toml` does not ignore 0327.

This tip's 10 commits are autoresearch harness setup plus score-neutral repairs: agent grep/glob fail closed on I/O errors and do not follow links, create-only-on-genuine-absence, snapshot guards across drafts, `replacement: null`, BudgetExhausted audit, and allowed-parent alias canonicalization.

`git merge-tree` shows one content conflict: `docs/superpowers/plans/2026-09-04-production-qualification.md`. `docs/INDEX.md` changed on both sides and auto-merges.

## What was run

Each check ran once, except the two cases called out below. A first combined `cargo test` / `clippy` / `check` died in about 70 seconds because `libdbus-sys` could not find `dbus-1.pc`. That is an environment miss, not a product result. After `.github/scripts/apt-install.sh --gui` plus `libdbus-1-dev`, the three commands were run separately and those exits are the ones recorded.

| Check | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all --check` | 0 | Silent success. `FMT_EXIT:0`. |
| `cargo deny check` | 1 | advisories FAILED; bans, licenses, and sources ok. |
| `cargo test --workspace --all-targets --no-fail-fast -j 2` | 101 | 4061 passed, 2 failed, 25 ignored, 312 ok binaries, 2 failed binaries. Ambient gitconfig. |
| Isolated rerun: `git_remote_policy_workflow` | 0 | 11 passed. `REMOTE_EXIT:0`. |
| Isolated rerun: `git_commit_validates_message_and_commits_staged_hunks` | 0 | 1 passed in 0.20s. `COMMIT_EXIT:0`. |
| `cargo clippy --workspace --all-targets -j 2 -- -D warnings` | 101 | `redundant_field_names` at `crates/legion-debug/src/live_session.rs:196`. |
| `cargo check --workspace --all-targets -j 2` | 0 | Finished in 1m 52s. |
| xtask `check-deps`, `docs-hygiene`, `claim-audit`, `no-egui-textedit`, `verify-kanban-backlog`, `verify-readiness-consistency` | 0 | Each recorded `*_EXIT:0`. |
| `release-pipeline --dry-run` and `verify-release-pipeline` | 0 | Report total 5, passed 0, unchecked 5. |
| `update-drill`, `hostile-evals`, `verify-hostile-evals` | 0 | Local fixture drill; hostile evals verified. |
| `golden-path-1` through `golden-path-4` | 0 | All four reports passed. |
| `perf-harness` and `verify-perf-harness` | 0 | 17 measured, 13 passed, 0 failed, 4 skipped. `baseline=missing_for_os`. |
| `rust-analyzer-smoke` | 0 | 1 LSP test plus 2 ignored app tests passed. |
| `windowed-gui-e2e` (first) | 1 | Panic: `libxkbcommon-x11.so` could not be loaded. `window_created=false`. |
| `windowed-gui-e2e` after `libxkbcommon-x11-0` | 0 | `window_created=true`, open/edit/save passed, `errors=[]`. |
| `python3 -m unittest evals.test_run_eval` | 0 | 2 tests. |
| `python3 -m unittest training.test_qlora_train` | 0 | 15 tests. No torch required. |
| CLI `legion-app` `:q` and `:w` | 0 | Save line recorded a sha256. Strace of `:q` showed no non-Unix `connect`/`sendto`/`sendmmsg`. |
| Untraced `--beta-smoke` | 0 (evidence) | Evidence file `status: passed`. The workflow sets Assist before browse/edit/save. |
| `strace -f` of `--beta-smoke` | 124 | Strace did not reap after the child exited. Not a packet capture. |
| `scripts/package-native.sh --version 0.0.1 --format deb --dry-run` | 0 | `Planned package: .../legion-desktop-linux-x64-deb.deb`. |
| Real `deb` build (first) | 1 | Invalid. A concurrent `--dry-run` rewrote `target/native-package/Packager.toml` while `cargo build` was still running. Wrapper: `expected exactly one .deb ... found 0`. `cargo-packager` itself had written the deb. |
| Real `deb` build (rerun, no concurrent dry-run) | 0 | `DEB_RERUN_EXIT:0`. Artifact recorded below. |

`scripts/run-phase-gates.sh` was not invoked as one script. Its constituent xtask gates and the Rust gates were run individually so a single failure would not hide the rest. The script header still says 20 gates while it runs 21 (`RR-2026-018`).

### Failure excerpts

`cargo deny check`:

```
RUSTSEC-2026-0285  rustls 0.23.40   fix >=0.23.45
RUSTSEC-2026-0316  wasmtime 46.0.3  fix >=48.0.3
RUSTSEC-2026-0327  wasmtime 46.0.3  fix >=48.0.4
advisories FAILED, bans ok, licenses ok, sources ok
DENY_EXIT:1
```

wasmtime is pulled by `legion-plugin` → `legion-app` → `legion-desktop`.

Workspace tests, ambient gitconfig (this VM sets `commit.gpgsign=true` and `url.*.insteadof` for GitHub):

```
test push_to_a_network_remote_is_denied_by_the_air_gapped_default_policy ... FAILED
assertion `left == right` failed
  left: "https://github.com"
  right: "ssh://github.com"

test git_commit_validates_message_and_commits_staged_hunks ... FAILED
commit should succeed: CommandFailed { command: "-c core.hooksPath= commit --no-verify ...",
  stderr: "timed out after 5005ms" }
TEST_EXIT:101
```

The air-gap denial itself held. The assertion that failed was the scheme label after `insteadOf` rewrote `git@github.com:legion/example.git`. Userinfo from that rewrite did not appear in the test log. Both tests pass with `GIT_CONFIG_GLOBAL` pointed at an empty file and `GIT_CONFIG_NOSYSTEM=1`.

Clippy:

```
error: redundant field names in struct initialization
  --> crates/legion-debug/src/live_session.rs:196:11
   |
196 |         #[from]
197 |         source: DapFrameError,
CLIPPY_EXIT:101
```

`cargo check` later exited 0. This is a lint, and the same struct is on `origin/main`.

First windowed E2E:

```
Library libxkbcommon-x11.so could not be loaded.
windowed-gui-e2e_EXIT:1
```

`.github/scripts/apt-install.sh --gui` installs `libxkbcommon-dev` and does not install `libxkbcommon-x11-0`. `.github/workflows/legion-windowed-gui.yml` installs that runtime library separately. After `apt-get install libxkbcommon-x11-0`, the same xtask command exited 0. `libEGL` warned that DRI3 was unavailable under Xvfb; the window still came up. That warning is software rendering, not a GPU proof.

`verify-release-pipeline` wrote `target/release-pipeline/verify_report.toml` at `2026-10-07T23:14:27Z`: `total = 5`, `passed = 0`, `unchecked = 5`, every `signer_status = "dry-run/no-production-signer"`.

Perf (Linux only): `manual.renderer_input_to_paint` p50 3785 µs, p95 3892 µs, budget 32 ms, passed. `m9.large_file_100mb` p50 3309 µs, p95 3433 µs, budget 16 ms, passed. Repository search p50 168 ms. A generated 100k-file workspace search p50 2874 ms. Open-to-ready on this repository p50 297 ms. Verify skipped the regression comparison because `baseline=missing_for_os`.

CLI save (`target/verify-legion-ide/evidence/cli-1791413392/save.txt`):

```
Saved file_id=FileId(1) snapshot=2 hash=sha256:7dd375393629ca9e767a1c4496f6a1353aa790760272f3c1138cc6079c5c7254
```

Quit exited 0. Strace of the `:q` process recorded three `+++ exited with 0 +++` lines and no non-Unix `connect`, `sendto`, or `sendmmsg`.

## Linux packaging

`scripts/package-native.sh` accepts only version `0.0.N` with `N>=1`. The workspace package version is `0.1.0`. `packaging/Packager.toml` version is `0.0.0`. Dry-run with `--version 0.0.1 --format deb` exited 0 and planned `target/native-package/output/legion-desktop-linux-x64-deb.deb`.

The real build uses a separate `CARGO_TARGET_DIR` (`target/native-package/cargo-target`) so it does not share the perf-harness release directory. `cargo-packager` 0.11.8 installed successfully (`PACKAGER_INSTALL:0`). The release compile finished in 5m 28s.

The first wrapper exit was 1 because this audit started `--dry-run` while that compile was still running. `render_config` runs before the dry-run return and overwrites the single `target/native-package/Packager.toml`. `cargo-packager` then wrote the deb under the dry-run PID directory, and the live script looked in its own staging directory and found 0 files (`RR-2026-027`). That exit is an audit collision, not the package result.

The same command was rerun once with no concurrent dry-run. Incremental `cargo build --release` finished in 1.41s and the wrapper exited 0:

```
Wrote /workspace/target/native-package/output/legion-desktop-linux-x64-deb.deb
signer_status = unsigned-beta/no-os-code-signing
sha256 da2baecfc883dc52bb2dbb77b05517e4ae483621fa471d8167137eb70464f75a
size 19775668 bytes
Package: legion
Version: 0.0.1
Maintainer: Legion <dasblueeyeddevil@gmail.com>
```

`dpkg-deb -I` shows no `Depends` field. `libxkbcommon-x11-0` is not declared (`RR-2026-015`). `RELEASE-METADATA.toml` records `workspace_version = 0.1.0` beside `release_version = 0.0.1` (`RR-2026-022`).

`packaging/Packager.toml` copies `LICENSE`, `docs/PRIVACY.md`, and `THIRD_PARTY_NOTICES.md`. Authors is `Legion <dasblueeyeddevil@gmail.com>` (`RR-2026-025`). The August 2026 RCA asked for a human decision on the public Maintainer string. The field is non-empty, which lets the verifier see a Maintainer, and it is a personal address.

## Coverage map

| Feature | Exercised this run | Existing gate or tests | Drift or residual |
| --- | --- | --- | --- |
| Editor buffers, undo, save | CLI `:w` wrote a snapshot hash. Windowed open/edit/save passed. Golden-path 1 edit-save-commit passed. | `legion-editor` tests inside the 4061. | Palette and keyboard docs disagree with the keymap (`RR-2026-012`). |
| Workspace open and guarded writes | Golden paths opened temp workspaces and applied proposals through the app. | `workspace_vfs` and project tests in the suite. | Host-qualified `file://` remotes stay Local on this tip (`RR-2026-004`). |
| Workspace search | Golden-path 1 found `SMOKE_MARKER_ALPHA`. Perf search over this repo and a 100k-file tree passed. Beta smoke reported search Completed. | `daily_editing_search` in the suite. | Walker errors increment an omitted count and still return Ok (`RR-2026-017`). Ledger PR-17 still says search is on the UI thread (`RR-2026-010`). |
| Keyboard shortcuts | Code and `docs/KEYBOARD_REFERENCE.md` compared with `default_keymap`. | UI keymap tests in the suite. | Save Active is command+S and Save All is command+shift+S. Palette and one fixture row label them differently (`RR-2026-012`). |
| Manual / Assist / Delegate / Legion Workflows | Golden-path 1–4 passed in that order of modes. | Mode and policy tests in the suite. | Beta smoke sets Assist before browse/edit/save (`RR-2026-019`). |
| Assist, Delegate, Workflows opt-in | Default `AppProductMode` is Manual. `require_assist_mode` rejects AI dispatch outside Assist+. `ProposalAutoApprovalPolicy` defaults to enabled false. | `manual_zero_egress` is in the suite that passed aside from the two git tests. | Default desktop feature is `ai`, and `legion-ai-providers` is an unconditional desktop dependency (`RR-2026-005`). |
| Proposal review | Golden-path 2–4 applied proposals on the approval path. Beta evidence previewed proposal 2 and listed Legion Workflows apply as unsupported. | Proposal tests in the suite, including this branch's create-only and snapshot-guard repairs. | — |
| Plugins | Code review: `dispatch_host_call` is metadata. Wasmtime host sets fuel, epoch, and memory limits and does not enable WASI. Product composition does not execute plugin WASM. | Plugin crate tests in the suite. | wasmtime 46.0.3 is still linked (`RR-2026-003`). |
| Settings and session | Beta session JSON written. Crash-safe session path rejects raw-source markers (code review). | Storage and session tests in the suite. | User guide still says local history is in memory; `manifest.json` is persisted (`RR-2026-010`). |
| Diagnostics bundle | Beta diagnostics markdown written under `/tmp/audit` (metadata, counts, statuses). | — | — |
| Installer, updater, packaging | Release dry-run, update-drill, package dry-run, and a Linux deb rerun that exited 0. Updater source in tree is `LocalDir` only. No desktop caller of `check_for_update`. | `verify-release-pipeline` exits 0 while every descriptor is unchecked (`RR-2026-013`). | Signing and HTTP swap are open (`RR-2026-001`, `RR-2026-002`). Version strings disagree (`RR-2026-022`). The deb declares no `Depends` (`RR-2026-015`). A dry-run rewrites the shared packager config (`RR-2026-027`). |
| Projection-only UI | `no-egui-textedit` exited 0. | UI intent tests in the suite. | — |
| Provider and egress boundary | CLI `:q` strace had no non-Unix connect. Telemetry consent defaults all false. Reqwest export client defaults disabled and is not constructed from `legion-app`. | `manual_zero_egress` in the suite. | Not a clean-VM packet capture. Assist taxonomy allows `HostedTelemetry` while the exporter stays unwired (`RR-2026-024`). `CODEBASE.md` says provider traffic never includes raw source; Assist sends a buffer excerpt (`RR-2026-008`). |
| Path containment and symlinks | This branch's grep/glob commits fail closed and do not follow links. Workspace `WalkBuilder` is not `follow_links(true)`. Existing symlink-escape tests are in the suite. | Project containment tests. | Workspace search still returns Ok on walk errors (`RR-2026-017`). |
| Crash recovery and data loss | Code review: hot-exit sidecar and `.legion/local-history/manifest.json`. Stale save conflicts stay fail-closed in the documented save path. | Storage and save-conflict tests in the suite. | User-guide wording is stale (`RR-2026-010`). |
| Git commit and remotes | Full suite failed two tests under the agent gitconfig. Isolated reruns passed. | `git_workflow`, `git_remote_policy_workflow`. | 5s commit timeout inherits `commit.gpgsign` (`RR-2026-011`). `insteadOf` changes the recorded scheme (`RR-2026-016`). |
| Accessibility | Not run here. | Prior evidence: Windows Narrator 2026-09-02; macOS AX dump; Linux AT-SPI miss. | VoiceOver and Orca transcripts are absent (`RR-2026-006`). |
| Performance | Linux harness and verify exited 0. | Budgets above. | No Linux baseline and no macOS/Windows paint rows (`RR-2026-023`). |
| Licensing and notices | `cargo deny` licenses ok. Package script copies `THIRD_PARTY_NOTICES.md`. | — | Maintainer identity needs the owner (`RR-2026-025`). |
| Windows sandbox | Not run (Linux VM). | `docs/SECURITY.md` still says the job object does not enforce filesystem or network isolation. | `RR-2026-014`. |
| Supply chain | `cargo deny check` executed. | bans, licenses, sources ok. | Three advisories (`RR-2026-003`). |

`CODEBASE.md` is a 2026-08-11 snapshot. Its zero-`todo!()` sentence is not what `claim-audit` enforces; that gate still exited 0 (`RR-2026-026`).

## What this VM could not verify

| Item | Why | What is needed |
| --- | --- | --- |
| macOS DMG and Windows MSI | This host is Linux. | A macOS runner and a Windows runner with the release workflow's package steps. |
| Authenticode, `codesign`, notarization, Gatekeeper, SmartScreen | No production signer. Issues `#211` (Mac), `#212` (Linux), `#213` (Windows) are still the sequence doc's open GAP-02 items. | Org-held certificates and a fresh-VM trust journal per OS. Ed25519 manifest signing does not close this. |
| VoiceOver | macOS only. | A Mac with VoiceOver notes plus the existing AX probe. |
| Orca | No real AT-SPI session. Prior xvfb AT-SPI dump missed AccessKit. | A Linux desktop session with Orca and a dump that sees the product window. |
| Windows sandbox escape probes | Linux VM. | `cargo test -p legion-sandbox --test escape_attempts` on Windows. |
| GPU / DRI3 | Xvfb warned that DRI3 was unavailable. The window still opened on software EGL. | A host with a real GPU and DRI3 if accelerated rendering is a release claim. |
| Clean-VM Manual packet capture (GAP-06.2) | CLI strace is one process on a dirty agent VM. Desktop default features include `ai`. `strace -f` of the desktop binary hung after the child exited. | A clean VM, an offline package, and a capture of open/edit/save/search with no DNS/TCP/UDP to non-loopback hosts. |
| Hosted update feed | Only `LocalDirManifestSource` exists. `update-drill` uses a local fixture and does not swap or roll back a packaged binary. | A signed manifest, installer swap, restart, and N-1 restore. Manual mode must not poll that feed. |
| Signing keys | Must stay out of the tree. | Secret store on the release runners. |

## Findings

Full records are in `findings.json`. The table is the review surface.

| ID | Sev | Area | Feature | Evidence / repro | Acceptance | Wave |
| --- | --- | --- | --- | --- | --- | --- |
| RR-2026-001 | P0 | release | OS code signing | Dry-run descriptors are `dry-run/no-production-signer`. Release workflow is unsigned-beta. `#211` `#212` `#213` open. | A real OS signer and a fresh-VM trust journal per OS. | W5 |
| RR-2026-002 | P0 | release | Update swap and rollback | `updater.rs` is LocalDir only. No desktop `check_for_update`. Drill does not install. | Fetch, swap, restart, restore N-1. Manual does not poll. | W5 |
| RR-2026-003 | P0 | supply-chain | cargo deny | 0285 rustls 0.23.40, 0316 and 0327 wasmtime 46.0.3. `DENY_EXIT:1`. | `cargo deny check` exits 0 at wasmtime `>=48.0.4` and rustls `>=0.23.45`, with no new ignore. | W1 |
| RR-2026-004 | P0 | security | Host-qualified file remotes | `classify_git_remote_url` treats every `file://` as Local. `e0a1277` is only on `main`. | `file://attacker.example/share/repo.git` is Host and the air-gap denies it. | W0 |
| RR-2026-005 | P0 | privacy | Manual zero-egress SKU | CLI strace was clean. Desktop `default = ["ai"]` and providers stay linked. No clean-VM capture. | `--features offline` drops the provider HTTP stack, and a clean-VM capture stays on loopback. | W4 |
| RR-2026-006 | P0 | accessibility | VoiceOver and Orca | Narrator transcript exists. macOS evidence is an AX dump. Linux AT-SPI missed the window. | Committed VoiceOver notes and an Orca transcript whose AT-SPI dump sees the window. | W5 |
| RR-2026-007 | P1 | tooling | clippy on 1.99 | `live_session.rs:196` `redundant_field_names`. `CLIPPY_EXIT:101`. Same code on `main`. | `cargo clippy --workspace --all-targets -- -D warnings` exits 0 on CI's stable toolchain. | W1 |
| RR-2026-008 | P1 | docs | Provider payload | `CODEBASE.md` says traffic never includes raw source. Assist sends `assist_buffer_excerpt`. | The snapshot says opted-in Assist/Delegate sends a bounded excerpt. | W2 |
| RR-2026-009 | P1 | integration | Merge fitness | 6 behind, 10 ahead. One doc conflict. Missing `#220` and the advisory pins. | Merge into `main` keeps `#220`, the pins, and the native-input wording. Do not release this tip. | W0 |
| RR-2026-010 | P1 | docs | Ledger and user guide | PR-17 still says search is on the UI thread. User guide says local history is in memory and that P3.F1.T2 still gates apply. | Ledger, guide, and code agree. No readiness row is promoted. | W2 |
| RR-2026-011 | P1 | scm | Commit vs gpgsign | 5s timeout on `commit --no-verify` when `commit.gpgsign=true`. Isolated rerun passed in 0.20s. | The IDE commit disables signing for that action, or surfaces the signer and uses a budget that survives a normal sign. | W3 |
| RR-2026-012 | P1 | ui | Shortcut labels | Keymap: command+S Save Active, command+shift+S Save All. Palette and one keyboard-doc row disagree. | Labels name the platform command modifier plus S, and that modifier plus Shift+S. | W2 |
| RR-2026-013 | P1 | release | Dry-run verifier | `verify-release-pipeline` exits 0 with passed 0 and unchecked 5. | Stable channel fails on unchecked descriptors, or the gate name stops implying artifact verification. | W2 |
| RR-2026-014 | P1 | security | Windows sandbox | `SECURITY.md`: job object kills the tree and does not enforce filesystem or network isolation. Not re-run here. | Windows enforcement holds under escape probes, or the UI keeps the caveat and the release claim stays honest. | W5 |
| RR-2026-015 | P1 | packaging | libxkbcommon-x11 | First windowed E2E panicked. CI GUI apt script omits the runtime library. Rerun after install passed. | The deb depends on `libxkbcommon-x11-0` or bundles it, and windowed E2E passes on a clean Ubuntu image. | W4 |
| RR-2026-016 | P2 | scm | insteadOf scheme | Ambient rewrite made the expected `ssh://` assertion see `https://`. Denial held. Isolated 11 passed. | The test uses a private `GIT_CONFIG_GLOBAL`. The audit row records the URL git will contact. | W3 |
| RR-2026-017 | P2 | search | Walk errors | Agent grep/glob on this branch fail closed. Workspace search still returns Ok and bumps `omitted_file_count`. | A walk error is a failed or partial status. | W3 |
| RR-2026-018 | P2 | tooling | Gate count | `scripts/run-phase-gates.sh` says 20 and runs 21. README lists 21. | Header, progress denominator, and README match. | W2 |
| RR-2026-019 | P2 | privacy | Beta smoke mode | `run_beta_workflow_inner` sets Assist. Evidence `status: passed`. `strace -f` hung. | Manual egress evidence is a Manual-mode process. | W4 |
| RR-2026-020 | P2 | testing | Ignored tests | 25 ignored in the default suite. `rust-analyzer-smoke` later passed its ignored tests. | The runbook lists which ignored tests are required gates. | W2 |
| RR-2026-021 | P2 | release | August E2E RCA | Linux deb rerun exited 0 with Maintainer set and `signer_status` unsigned-beta. Control file has no Depends. MSI and DMG were not re-run. | A current three-OS verifier run exits 0, including Maintainer on the deb. | W5 |
| RR-2026-022 | P2 | packaging | Version and changelog | Crate `0.1.0`, Packager.toml `0.0.0`, script accepts `0.0.N` only. Changelog Unreleased has one note. | One version is used by the workspace, the package script, and the changelog section that ships. | W2 |
| RR-2026-023 | P2 | performance | Linux baseline | Harness passed. Verify skipped regression: `baseline=missing_for_os`. One machine. | A linux block in `plans/evidence/perf-harness-trend/baseline.toml`, plus macOS and Windows rows. | W5 |
| RR-2026-024 | P2 | privacy | Hosted telemetry bit | Assist and above allow `HostedTelemetry`. The HTTP client defaults off and is unwired. | The taxonomy matches the wired path, or the privacy docs name both facts together. | W4 |
| RR-2026-025 | P3 | packaging | Debian maintainer | Authors is a personal Gmail address. | The owner approves the public Maintainer string. | W5 |
| RR-2026-026 | P3 | docs | CODEBASE snapshot | Header says 2026-08-11 and zero `todo!()`. `claim-audit` does not enforce that sentence. | The header is dated as a snapshot, or the zero-todo sentence is removed. | W2 |
| RR-2026-027 | P2 | packaging | Shared packager config | `--dry-run` writes `target/native-package/Packager.toml` before returning. A dry-run during the live compile made the wrapper look in an empty staging dir (`DEB_EXIT:1`). The isolated rerun exited 0. | A dry-run does not rewrite the config a live invocation passes to `cargo-packager`. | W4 |

## Wave plan

Waves are independently reviewable. Later waves assume the earlier dependency.

| Wave | Title | Findings | Depends on |
| --- | --- | --- | --- |
| W0 | Merge this tip into `main`. Do not release the tip. | RR-2026-009, RR-2026-004 | — |
| W1 | Make deny and clippy green on stable. | RR-2026-003, RR-2026-007 | W0 |
| W2 | Documentation truth. No behavior change and no ledger promotion. | RR-2026-008, 010, 012, 013, 018, 020, 022, 026 | W0 |
| W3 | Git hermeticity and search error status. | RR-2026-011, 016, 017 | W0 |
| W4 | Manual offline SKU, deb runtime library, telemetry taxonomy, packager config isolation. | RR-2026-005, 015, 019, 024, 027 | W1 |
| W5 | Human trust chain and other-OS proof. | RR-2026-001, 002, 006, 014, 021, 023, 025 | W4 |

W0 resolves `docs/superpowers/plans/2026-09-04-production-qualification.md` and keeps `main`'s file-remote gate, rustls 0.23.45, wasmtime bump, and native-input wording. W1 then moves wasmtime to `>=48.0.4` and fixes the one clippy lint. Re-run only `cargo deny check` and `cargo clippy`.

## Harness and environment

No repository test or CI file was edited. The suites that can run on Linux ran after environment packages:

- Rustup stable 1.99.0 (the image started on 1.83, which cannot compile edition 2024).
- `cargo-deny` 0.20.2 and `cargo-packager` 0.11.8.
- `.github/scripts/apt-install.sh --gui` plus `libdbus-1-dev` so `libdbus-sys` can find `dbus-1.pc`.
- `libxkbcommon-x11-0` so the windowed binary can load `libxkbcommon-x11.so`.
- `strace` for the CLI probe.

The two git failures were rerun once each under an empty gitconfig. They passed. They stay findings because a developer gitconfig can time out a commit and rewrite a remote scheme. The perf-harness trend entry written under `plans/evidence/perf-harness-trend/entries/` is generated and is not part of this audit commit.

## Human-only setup

- Signing certificates for `#211` (macOS), `#212` (Linux), and `#213` (Windows), stored in the org secret store.
- A Mac for DMG, `codesign`, notarization, Gatekeeper, and VoiceOver.
- A Windows machine for MSI, Authenticode, SmartScreen, Narrator regression, and sandbox escape probes.
- A Linux desktop session with Orca for the AT-SPI transcript.
- A clean VM and a packet capture for Manual open/edit/save/search (GAP-06.2).
- Owner approval of the public Debian Maintainer string.
- A committed Linux perf baseline, then macOS and Windows paint rows, before the regression gate can fail a Linux regression.
