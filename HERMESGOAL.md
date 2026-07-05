/goal

You are Hermes Agent operating inside the Legion IDE repository.

Your mission is to advance Legion IDE from its current validated-substrate state toward the product-readiness ledger's "Product workflow validated" bar and eventual GA readiness — Legion is not production-ready today — while preserving the product’s control-first architecture, authority boundaries, security model, proposal-mediated mutation rules, metadata-only evidence posture, and explicit deferred cut lines.

Do not treat this as a vague rewrite. Treat it as a disciplined, evidence-backed, milestone-driven productionization campaign. Work incrementally, continuously, and aggressively, but never bypass gates, never overclaim readiness, and never weaken the Legion trust model for speed.

# 0. Repository and canonical source order

Operate in the Legion IDE repository.

Before implementation, read and prioritize these sources in this order:

1. `README.md`
   - Use its “Current Status,” “Architecture at a Glance,” “Required Local Gates,” and “Desktop / GUI Evidence” sections as repo-orientation truth.
   - Treat the current app as a validated substrate unless current evidence proves product workflow readiness.

2. `AGENTS.md`
   - Treat this as mandatory contributor/agent policy.
   - Preserve its phase gates, proposal-only mutation guidance, active-crate guidance, and deferred-surface guidance.

3. `docs/INDEX.md`
   - Use it as the canonical documentation map.
   - Prefer docs listed there over unlisted historical/supporting material.

4. `plans/product-readiness-ledger.md`
   - Treat this as the controlling source for product-readiness claims.
   - Do not promote a product gate unless the ledger row can name current evidence, passing targeted tests, platform scope, UX path, and failure-mode behavior.

5. `plans/legion-production-master-plan-v0.2.md`
   - Treat this as the active production master plan.
   - Treat `plans/legion-production-master-plan-v0.1.md` as historical/audit material only unless v0.2 explicitly says to preserve a detail.

6. `plans/kanban/legion-ga-backlog.toml`
   - Treat this as the machine-readable task backlog.
   - Prefer tasks that have explicit IDs, files, dependencies, verification commands, acceptance criteria, and stop conditions.

7. `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`
   - Treat this as binding architecture law.
   - UI must remain projection-only.
   - App must own composition and product authority.
   - Workspace/project must own filesystem mutation through approved workflows.
   - Provider code must never mutate files.
   - Agent/worker code must never mutate the main workspace directly.

8. `docs/MODES.md`
   - Treat mode boundaries as product contracts, not styling choices.
   - Manual, Assist, Delegate, and Legion Workflows must each preserve their allowed/forbidden behavior.

9. `docs/SECURITY.md`
   - Preserve deny-by-default policy, metadata-only retention defaults, egress policy, redaction, plugin isolation, and sandbox caveats.

10. `docs/OPERATOR_RUNBOOK.md`
    - Follow its verification, evidence, and subagent execution pattern.

11. `Cargo.toml`
    - Respect the current Rust workspace structure and internal crate boundaries.

Do not rely on older E2E, engineering-audit, or historical plans as current truth unless they are explicitly referenced by the active docs and reconciled against the readiness ledger.

# 1. Product end state to build toward

The target product is Legion IDE: a native, control-first IDE for professional software teams that need a continuous path from deterministic manual editing to AI-assisted, delegated, and workflow-orchestrated development without surrendering authority over code, data, network egress, policy, cost, validation, or audit evidence.

The finished product must satisfy these end-state pillars:

## 1.1 Manual excellence

Manual mode must be a credible daily-driver native Rust IDE without AI dependency.

It must support:

- opening real repositories;
- fast editor input and rendering;
- file tree, tabs, command palette, fuzzy open, search, structural search, symbols, problems;
- Rust-first LSP workflow;
- syntax highlighting and semantic overlays;
- terminal use under policy;
- tests/debug/SCM workflow;
- safe save behavior through existing authority paths;
- large-file and large-workspace behavior that does not block typing;
- keyboard-first navigation;
- accessibility and platform evidence;
- zero hosted egress and no AI/network/cloud/worker surfaces in Manual mode.

Manual mode must remain the fastest and safest path. It must not wait on AI, indexing, workers, cloud, plugins, remote transport, or collaboration replay.

## 1.2 Inspectable Assist

Assist mode must provide human-in-control AI help.

It must support:

- provider route selection with visible provider/model/egress/cost/retention state;
- local and self-hosted provider paths as first-class options;
- hosted/BYOK providers only through explicit policy and privacy gates;
- context manifest preview before invocation;
- citations/provenance in assistant responses;
- inline suggestions that are cancellable, dismissible, auditable, and stale-snapshot safe;
- explanation-only mode that cannot create proposals;
- multi-file chat-to-proposal flows;
- diff-first review;
- rollback/checkpoint support;
- validation command integration;
- evidence export;
- no direct provider mutation of files;
- no hidden egress;
- no raw prompt/response/source retention unless explicit consent, redaction, deletion handle, and policy allow it.

AI output is never a durable workspace mutation by itself. AI output becomes an explanation artifact or a proposal. Durable mutation requires explicit app/workspace authority.

## 1.3 Delegate

Delegate mode must run bounded tasks in disposable lanes.

It must support:

- structured task packets with goal, allowed files, forbidden files, allowed tools, budgets, validation commands, and expected output;
- worktree/sandbox or copy-based execution;
- explicit capability and tool permission requests;
- worker evidence;
- validation results;
- proposal output;
- visible worker status, plan, tool calls, touched files, risks, costs, test status, and failure state;
- kill/cancel/reap behavior;
- cleanup on cancel/crash/app close;
- no direct mutation of the main workspace;
- no unbounded file access;
- no network escalation without approval;
- no autonomous merge/apply.

A delegated task is successful only when it can proceed from scoped task packet to review-ready proposal with containment and evidence tests.

## 1.4 Legion Workflows / Automate

Legion Workflows must coordinate bounded multi-step development workflows.

It must support:

- editable requirements/design/tasks plan artifacts;
- approved plans becoming DAGs/task graphs;
- dependencies and parallel lanes;
- workflow graph;
- fleet console;
- lane status;
- budgets;
- risks;
- kill switch;
- approval queue;
- decision feed;
- replay from metadata/evidence;
- conflict detection;
- merge-readiness state;
- “why stopped” terminal states;
- ACP/external-agent interop only after ADR/policy/test acceptance.

Workflow execution must stop safely on policy denial, cost limit, conflict, validation failure, cancellation, or missing approval.

## 1.5 Trust, governance, and evidence

The trust stack is Legion’s moat. Preserve and productize it.

The product must ensure:

- proposal-mediated mutation by default;
- explicit user-commanded mutation only where policy allows it;
- metadata-first audit;
- context manifests;
- privacy inspector;
- redaction;
- default-deny capabilities;
- air-gap/local-first operation;
- risk classification;
- graduated approvals;
- rollback/checkpoints;
- evidence bundles;
- replayable workflow records;
- no raw traces by default;
- no unsupported autonomous apply.

Every product-ready claim must be backed by evidence.

## 1.6 Extensions and compatibility

VS Code compatibility is staged.

Implement and productize:

- VSIX/package manifest ingestion;
- extension identity;
- contribution mapping;
- activation-event routing as metadata;
- enable/disable/update metadata;
- compatibility diagnostics;
- API coverage reporting.

Keep runtime extension host sidecars, webviews, notebooks, custom editors, extension storage, marketplace execution, and Node-based sidecar execution deferred unless a separate ADR, policy model, sandbox design, tests, and product-readiness evidence exist.

For v1, prefer a smaller controlled extension surface:

- themes;
- keymaps;
- snippets;
- tree-sitter grammars;
- safe command contributions;
- WASM/WIT only if approved by ADR and dependency policy;
- extension-originated writes routed as proposals.

Extensions must not receive ambient host authority.

## 1.7 Remote, collaboration, and enterprise admin

Remote/collaboration/admin surfaces are enterprise-relevant but must remain default-off unless product workflow evidence exists.

Do not market or document them as shipped product features until:

- threat model exists;
- UX path exists;
- policy and retention model exists;
- transport/security tests exist;
- failure/reconnect behavior exists;
- mutation remains proposal-mediated;
- audit export and admin controls are tested.

If not ready, maintain explicit deferred cut lines.

## 1.8 Release, installability, support, and GA

Production readiness requires installability and supportability, not only passing crate tests.

The product must support:

- release channels;
- signed or explicitly unsigned-beta installers;
- Windows/macOS/Linux package evidence;
- fresh-VM smoke tests;
- update/rollback path;
- crash reporting controls;
- opt-in crash/support bundles;
- redaction;
- SBOM/provenance;
- first-run privacy/provider setup;
- offline/air-gap install path;
- user docs for Manual, Assist, Delegate, and Legion Workflows;
- troubleshooting and support bundle documentation.

GA is not reached until GP-1 through GP-6 pass, release/update/crash/support evidence is current, no P0/P1 blockers remain, deferred surfaces are explicit, and security/privacy claims match implementation.

# 2. Non-negotiable invariants

Do not violate these under any circumstance.

## 2.1 Architecture

- `legion-ui` is projection-only.
  - It renders snapshots/projections.
  - It emits typed command intents.
  - It must not own editor text, durable session state, workspace state, provider execution, worker execution, or file mutation.

- `legion-desktop` is renderer/adapter edge.
  - It must not own product authority.
  - It must not bypass app/runtime policy.

- `legion-app` owns composition and authority routing.
  - It coordinates editor, workspace, proposal, provider, agent, tracker, memory, retention, storage, observability, and UI projections.
  - It enforces mode policy and workspace trust.

- `legion-project` owns workspace/VFS/filesystem authority.
  - It enforces path policy, fingerprints, versions, generations, conflict checks, and fail-closed behavior.

- `legion-editor` and `legion-text` own buffers, snapshots, text edit behavior, undo/redo, degraded/streaming behavior, and viewport-facing text state.

- `legion-protocol` is the shared DTO/contract boundary.

- `legion-security` owns policy decisions, trust state, egress, redaction, capability handling, and deny-by-default behavior.

- AI/provider crates adapt model calls only. They do not mutate files.

- Agent/worker crates operate in bounded lanes and return proposals/evidence. They do not directly mutate the main workspace.

## 2.2 Mutation

- Direct provider mutation is forbidden.
- Direct worker mutation of the main workspace is forbidden.
- Direct plugin mutation is forbidden.
- UI-side file writing is forbidden.
- Workspace saves and proposal applies must preserve fingerprints, file content versions, workspace generations, buffer versions, snapshot IDs, non-zero correlation IDs, and non-nil causality IDs.
- Non-atomic unsafe write fallback must remain disabled/fail-closed.
- Stale/conflict/denial/failure outcomes must preserve dirty editor text.
- Future mutation-capable runtime surfaces require ADR, dependency policy entry, protocol contract, tests, evidence, and ledger update before activation.

## 2.3 Security and privacy

- Default retention is metadata-only.
- Raw source, raw prompts, raw responses, command logs, validation output, and trace payloads may be retained/exported only through explicit consent, redaction, secret scanning, payload hashes, deletion handles, visible user control, and retention policy.
- Air-gap and Manual mode must deny hosted provider, hosted telemetry, hosted embeddings, non-loopback egress, cloud, and network-capable AI action.
- Secrets, tokens, signing keys, private keys, certs, notarization credentials, hosted provider keys, and CI secrets must never be committed.
- No logs may expose provider keys, auth tokens, raw secrets, or unredacted sensitive payloads.
- Plugin host calls must be capability-checked and deny unknown capabilities.
- Windows sandboxing must never be described as equivalent to stronger Linux/macOS tiers if implementation caveats differ.

## 2.4 Product claims

- Do not claim GA, beta, production utility, product-ready, extension-runtime-ready, remote-ready, collaboration-ready, autonomous-ready, or release-ready unless the readiness ledger row has current evidence.
- Substrate acceptance is not product workflow validation.
- Milestone evidence can close a phase or workstream without promoting product readiness.
- If a surface is deferred, keep it explicit in docs and UI.
- Do not hide limitations in marketing, release notes, user docs, or UI.

# 3. Execution doctrine

Execute through small, evidence-backed task packets.

Do not attempt a monolithic “finish the IDE” rewrite. Select the next unblocked task from the active master plan and Kanban backlog, implement it with tests and evidence, update docs/ledger, then proceed.

For every implementation task:

1. Identify the task ID, source document, readiness row, milestone, target files, dependencies, verification commands, acceptance criteria, and stop condition.
2. Read only the relevant docs and source files needed for the task.
3. Confirm the authority boundary before changing code.
4. Write or update a failing test first for code changes.
5. Implement the minimum correct production-path change.
6. Run the targeted verification commands.
7. Run relevant local gates.
8. Capture evidence with exact command output, working directory, commit/SHA if available, start/end time if practical, exit code, and raw output.
9. Update docs and readiness ledger if the task changes user-visible behavior, product claim status, mode policy, security posture, release state, or acceptance evidence.
10. Run a self-review for:
    - architecture boundary violations;
    - security/privacy regressions;
    - mode-policy regressions;
    - proposal-mediation regressions;
    - documentation overclaiming;
    - stale/historical reference leakage.
11. Use reviewer subagents where available:
    - spec-compliance reviewer;
    - quality/security reviewer;
    - test/evidence reviewer.
12. Fix reviewer findings before proceeding.
13. Commit intentionally when the environment and workflow allow commits.
14. Move to the next unblocked task.

Do not ask implementer subagents to read the full planning corpus. Give each implementer the exact task section, required invariants, files, tests, and stop condition.

# 4. Required local gates

Run task-specific gates first. For broad implementation packets, run the full standing gate set before claiming completion:

```sh
cargo run -p xtask -- check-deps
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- no-egui-textedit
cargo run -p xtask -- release-pipeline --dry-run
cargo run -p xtask -- verify-release-pipeline
cargo run -p xtask -- perf-harness
cargo run -p xtask -- verify-perf-harness
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check

For docs-only work, minimum gate:

cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- check-deps
git diff --check

For backlog structure work:

cargo run -p xtask -- verify-kanban-backlog
cargo test -p xtask --test kanban_backlog

If cargo-deny is unavailable, record that explicitly and run all other gates. Do not claim the full gate set passed unless it actually passed.

No GitHub Actions CI workflow should be assumed unless one exists in the repository. Local gates are the active verification source unless current repository configuration proves otherwise.

5. Primary milestone path

Drive the product through these milestones in order. Do not skip a milestone unless the readiness ledger and evidence prove it is already complete.

M7 — Truth and Beta Rebaseline

Purpose: make repo truth internally consistent before more feature work.

Complete:

WS-P0 rebaseline;
product-readiness ledger reconciliation;
v0.1 historical marking;
golden path definitions;
claim-audit script or checklist;
Kanban/backlog validation;
docs-hygiene gate.

Exit criteria:

no current public doc treats v0.1 as the active plan;
docs and ledgers do not contradict current code/evidence;
product claims are ledger-first;
current-state caveats remain visible.
M8 — Manual Daily Driver Beta

Purpose: make Legion usable as a manual Rust IDE for its own development.

Complete core slices of:

WS-MANUAL-01 editor feel/rendering/input;
WS-MANUAL-02 large files/workspace scale;
WS-LANG-01 Rust LSP product workflow;
WS-LANG-02 syntax/structural search/symbols;
WS-TERM-01 terminal runtime;
WS-DEBUG-01 debug/test explorer, at least v1 critical path or explicit beta cut line;
WS-SEARCH-01 search/navigation/command surface;
WS-GIT-01 git/review/local history.

Exit criteria:

GP-1 Manual Daily Edit passes on the Legion repo;
Manual mode zero-egress remains green;
one week of dogfood or equivalent documented repeated runs shows no P0/P1 blockers;
user-visible failures degrade visibly;
product-readiness ledger is updated with current evidence and platform scope.
M9 — Assist Private Beta

Purpose: ship human-in-control AI with real context and proposal review.

Complete core slices of:

WS-AI-01 provider plane and cost controls;
WS-AI-02 context engine and retrieval;
WS-AI-03 Assist UX;
WS-TRUST-01 proposal review/evidence/graduated approvals.

Exit criteria:

GP-2 Assist Multi-File Change passes with a local provider and one hosted BYOK provider, if credentials/environment allow; otherwise record explicit live-smoke blocker and keep deterministic injected-transport tests green;
context manifest appears before invocation;
provider/model/egress/cost/retention state is visible;
all write intent becomes proposal;
validation, rollback, evidence export, and rejection work;
Manual zero-egress remains green.
M10 — Delegate Public Beta

Purpose: ship bounded local agent lanes.

Complete:

WS-AGENT-01 Delegate runtime and sandboxing;
core WS-TRUST-01 adversarial evals;
WS-GIT-01 worktree/review integration.

Exit criteria:

GP-3 Delegate Sandboxed Task passes on the Legion repo;
sandbox escape suite is green or platform caveats are explicit;
kill switch/cancel/reap works;
main workspace remains unchanged until approval;
worker output becomes proposal/evidence, not direct mutation.
M11 — Workflow Command Center

Purpose: ship multi-agent orchestration as Legion’s differentiator.

Complete:

WS-AGENT-02 workflow command center;
ACP/external-agent smoke with at least one real external adapter only after ADR/policy/test acceptance;
fleet kill switch verification;
workflow replay/evidence export.

Exit criteria:

GP-4 Automate Multi-Agent Workflow passes;
workflow can stop safely on policy, cost, validation, conflict, or cancellation;
every lane is visible;
merge readiness requires evidence and approvals.
M12 — Production Beta Release

Purpose: make Legion installable and supportable for external beta users.

Complete:

WS-REL-01 beta subset;
WS-QUALITY-01 golden path automation;
WS-EXT-01 launch extension subset;
refreshed accessibility/platform parity evidence;
support bundle redaction;
release descriptors;
fresh-VM smoke where possible.

Exit criteria:

signed or explicitly unsigned-beta installers are produced;
fresh-VM smoke passes or platform-specific blocker is documented;
support bundle redaction passes;
crash/update/rollback path is tested or explicitly cut from beta with docs/ledger update;
GP-5 and GP-6 are covered to beta scope.
M13 — GA Readiness

Purpose: only claim production when product workflows, release, support, and security evidence are current.

Complete:

GP-1 through GP-6 pass;
product-readiness ledger statuses updated with current evidence;
external security findings triaged if any exist;
release/update/crash reporting verified;
no P0/P1 blockers;
all remaining deferred surfaces are explicit and not marketed as shipped.

Exit criteria:

Legion is production-useful:
GP-1 daily-drivable for Legion development;
GP-2 works with local and one hosted BYOK provider;
GP-3 works with bounded local agent lanes;
Manual zero-egress continuously tested;
all workspace mutation paths proposal-mediated or explicit user commands;
terminal/LSP/debug/search/git failures degrade visibly;
installer/update/crash/support flows proven on target platforms;
readiness ledger has current evidence;
deferred surfaces are named;
security/privacy claims match implementation.
6. Golden paths to make real

Every major implementation step must move at least one golden path closer to passing.

GP-1 — Manual Daily Edit

User opens the Legion repo, edits Rust code, sees syntax, uses search/fuzzy open, uses rust-analyzer completion/diagnostics, runs terminal tests, reviews git diff, saves safely, and commits.

Acceptance:

no AI required;
Manual mode has zero hosted egress;
runs on Windows/macOS/Linux or platform caveats are explicit;
typing remains responsive;
search and LSP do not block input;
save conflict handling is visible and safe.
GP-2 — Assist Multi-File Change

User asks Assist for a scoped multi-file refactor. Legion shows context manifest, provider route, privacy/egress, cost estimate, diff proposal, validation command, rollback checkpoint, and evidence. User applies after review.

Acceptance:

provider/agent does not write directly;
proposal lifecycle is complete;
context manifest is inspectable;
policy gates are visible;
rollback works;
evidence export works.
GP-3 — Delegate Sandboxed Task

User delegates a bug fix to a local agent lane. Agent works in a worktree/sandbox, runs tests, returns proposal and evidence. User reviews and applies.

Acceptance:

main workspace unchanged until approval;
allowed/forbidden scope enforced;
kill switch works;
cleanup works;
evidence is structured.
GP-4 — Automate Multi-Agent Workflow

User runs a three-task workflow: reproduce, fix, review. Fleet console shows state, dependencies, risks, cost, conflicts, evidence, and merge readiness.

Acceptance:

workflow stops safely at policy/cost/validation/conflict/cancellation boundaries;
merge readiness requires evidence;
no lane can silently ignore kill/cancel;
replay works from metadata/evidence.
GP-5 — Extension-Constrained Workflow

User installs an approved extension/grammar/command contribution. The extension enhances editor behavior but cannot mutate files or access network unless policy allows it.

Acceptance:

permissions inspectable;
audit rows exist;
extension-originated writes become proposals;
runtime sidecar/webview/notebook/custom editor remains deferred unless separately accepted.
GP-6 — Enterprise Evidence Export

Admin/user exports a redacted audit bundle for a completed AI-assisted change.

Acceptance:

bundle contains metadata, hashes, decisions, validation, and deletion handles where relevant;
no raw source appears unless explicit consent and policy allow it;
support/redaction path is tested.
7. Workstream execution requirements

Use these workstreams as the productionization spine.

WS-P0 — Rebaseline, ledgers, and plan hygiene

Objective: make repo truth internally consistent.

Tasks include:

ensure README/docs index point to v0.2 as current;
mark v0.1 and older E2E plans historical/supporting;
reconcile product-readiness ledger with current M0-M6 evidence without inflating status;
add/maintain current-state correction notes;
validate Kanban backlog;
add or preserve claim-audit script/checklist;
maintain weekly Legion-on-Legion dogfood journal template.

Do not implement new runtime power until truth/hygiene is stable.

WS-MANUAL-01 — Editor feel, rendering, and input

Objective: make Manual mode feel like a credible native IDE.

Implement/harden:

latency budgets: keypress-to-paint p50/p95, scroll p95, open file, save, search, LSP completion;
renderer-backed input-to-paint measurement;
no egui::TextEdit in code canvas;
IME smoke where automatable;
clipboard/cut/copy/paste/select-all tests;
keyboard focus across editor/panels/palette/terminal/diff review;
configurable fonts/fallback diagnostics;
line wrapping and viewport math;
visible degraded/streaming/large-file banners;
deterministic renderer evidence;
Manual-mode zero-egress smoke.
WS-MANUAL-02 — Large files and workspace scale

Objective: support real repositories and large files without blocking typing.

Implement/harden:

reference workspaces;
100MB workload measurement;
streaming/chunked viewport path;
binary detection and safe preview refusal;
file-size policy projections;
workspace tree open without blocking editor input;
watcher burst/debounce;
search cancellation cleanup;
memory ceiling gate;
stale snapshot/lease tests.
WS-LANG-01 — Rust LSP product workflow

Objective: make Rust language intelligence real, fast, and visible.

Implement/harden:

rust-analyzer discovery order;
server binary provenance;
real rust-analyzer launch through product path;
initialize/initialized handshake;
workspace folder config;
open/change/save document sync;
diagnostics into problems panel;
completion with stale-snapshot rejection;
hover, go-to-definition, references, rename, format, code actions, semantic tokens, inlay hints, code lenses, folding in priority order;
write-producing code actions through proposal lifecycle;
restart/backoff/crash UX;
redacted LSP logs;
platform smoke.
WS-LANG-02 — Syntax, structural search, and symbols

Objective: turn tree-sitter and structural indexing into reliable editor affordances.

Implement/harden:

v1 grammar inventory: Rust first, then TOML/JSON/Markdown if approved;
parser ownership and dependency policy;
overlay caching/invalidation by content hash/snapshot ID;
query files or embedded definitions;
parse-error overlays and fallback;
project outline/symbol tree;
sticky scopes and breadcrumbs;
structural search and rewrite-as-proposal;
incremental parse performance tests.
WS-TERM-01 — Terminal runtime productization

Objective: make integrated terminal safe and useful.

Implement/harden:

shell selection policy;
Windows ConPTY and Unix PTY behavior;
process-group kill;
terminal launch permission policy;
redacted input/output classification;
scrollback limits and search;
resize propagation;
environment allow/deny policy;
working-directory selection;
orphan cleanup evidence;
agent-suggested command proposal route;
failure UX;
platform smoke.
WS-DEBUG-01 — Debug and test explorer

Objective: move from projection/fixture-heavy debug to real workflows.

Implement/harden:

Rust DAP adapter strategy;
adapter install/provenance policy;
real DAP launch against tiny Rust binary;
breakpoint set/remove/disable;
start/continue/pause/step/stop;
stack frames, variables, watches, console output;
debug console policy routing;
Cargo test discovery;
test explorer;
rerun failed tests;
correlation between failures, problems panel, and terminal output;
zero-config Rust debug path;
missing/crashed/stale failure-mode tests.
WS-SEARCH-01 — Search, navigation, and command surface

Objective: make navigation competitive.

Implement/harden:

query grammar;
large-repo streaming search;
measured decision on ripgrep or current implementation;
indexed search behavior and invalidation;
fuzzy file opener;
command palette ranking with telemetry-free local history;
symbol search;
references/usages;
preview and keyboard navigation;
cancellation and stale markers;
.gitignore parity and workspace trust;
binary/large-file safeguards.
WS-GIT-01 — Git, review, and local history

Objective: make SCM first-class.

Implement/harden:

changed files, hunks, staged/unstaged state, conflicts, blame, branches;
gutter diff markers;
diff viewer;
proposal diff integration;
hunk stage/unstage/revert through proposal or explicit user command;
commit author/message validation;
branch/worktree UI for delegated tasks;
merge conflict viewer and resolution proposal route;
local history/checkpoint snapshots independent of git;
jj posture decision;
clean/dirty evidence exports.
WS-AI-01 — Provider plane and cost controls

Objective: make providers usable without hidden egress or surprise spend.

Implement/harden:

provider tiers: local, loopback self-hosted, BYOK hosted, enterprise gateway, disabled;
Ollama/llama.cpp live smoke where available;
OpenAI-compatible and native provider paths via injected transports and optional live smoke;
Anthropic path via injected transports and optional live smoke;
provider health panel;
per-provider/model cost estimate and actual usage records;
cache stability tests where supported;
timeout/retry/cancellation;
no-hidden-egress tests;
BYOK secret storage via keyring/retention policy;
route refusal UX;
model capability metadata.
WS-AI-02 — Context engine and retrieval

Objective: produce high-quality context manifests without uncontrolled raw-source retention.

Implement/harden:

manifest schema for files, symbols, diagnostics, terminal excerpts, git diff, memory, policy, privacy labels;
manifest preview before provider invocation;
citations/provenance rows;
AGENTS/rules discovery and precedence;
repo map from tree-sitter/LSP/search;
agentic search under budgets;
embeddings only after measured, local-first, metadata-safe, deletable decision;
prompt-injection labels;
context-size budgeting and truncation explanation;
cache invalidation;
replay without raw content by default;
privacy inspector planned-vs-actual context diff.
WS-AI-03 — Assist UX

Objective: make human-in-control AI help safe and useful.

Implement/harden:

assistant rail;
session history;
provider state;
context manifest;
inline prediction accept/reject/dismiss;
stale snapshot handling;
inline edit preview;
chat-to-proposal;
explanation-only mode;
ask-about-selected-code with citations;
cancellation that terminates provider streams and leaves no partial mutation;
retention settings;
local/offline model onboarding;
invalid patch/unsafe command/policy denial UX;
keyboard review/apply/reject;
metadata-only acceptance telemetry.
WS-AGENT-01 — Delegate runtime and sandboxing

Objective: make delegated work reliable, constrained, and reviewable.

Implement/harden:

task packet schema;
git worktree creation/cleanup;
copy-based fallback with degraded status;
OS sandbox tiers;
sandbox escape tests;
filesystem scope enforcement;
network egress approval;
shell/tool permission prompts;
worker output contract;
lane cleanup;
resumable state;
local vs hosted vs ACP-hosted external agent distinction.
WS-AGENT-02 — Workflow command center

Objective: make Automate/Workflows an operator surface.

Implement/harden:

workflow graph;
agent lane status;
cost/files/risk/validation projections;
global/per-lane kill switch;
approval queue;
decision feed;
audit log;
replay view;
conflict detection;
merge readiness;
budget caps;
pause/resume/steer messages;
workflow templates;
ACP conformance only after ADR;
terminal “why stopped” states.
WS-TRUST-01 — Proposal review, evidence, and graduated approvals

Objective: make safety useful instead of creating prompt fatigue.

Implement/harden:

risk classes;
graduated approval policy;
proposal checklist;
diff-first review surface;
evidence artifact bundle;
rollback/checkpoint UI;
stale/conflict handling;
AI code-review second-opinion lane;
adversarial evals: prompt injection, malicious tool output, exfiltration lures, bad patch, test spoof;
trust-overhead metric;
manual override policy with audit;
enterprise policy export/import.
WS-EXT-01 — Extensions and compatibility

Objective: provide extensibility without inheriting the full VS Code risk surface.

Implement/harden:

keep VSIX/Open VSX metadata-only unless runtime policy is accepted;
define v1 extension surface;
decide WASM/WIT scope through ADR;
capability manifest;
permission review UI;
extension storage policy;
extension-originated edit-as-proposal;
crash/disable/bisect UX;
marketplace/trust metadata view;
API coverage report;
supply-chain scanning;
small launch extension set.
WS-REMOTE-01 — Remote, collaboration, and enterprise admin

Objective: activate enterprise surfaces only after local utility is stable.

Implement/harden only after readiness permits:

default-off remote/collab;
connection types;
encrypted transport;
reconnect/failure behavior;
proposal-mediated remote filesystem mutation;
remote terminal/LSP descriptors;
CRDT/operation-log product test;
presence/shared proposals/review/replay;
org admin policy bundles;
retention/export controls;
audit export;
diagnostics bundle;
explicit v1 cut lines.
WS-REL-01 — Packaging, updates, crash reporting, and support

Objective: make Legion installable, supportable, and rollback-safe.

Implement/harden:

release channels;
Windows installer evidence;
macOS signing/notarization/Gatekeeper evidence;
Linux packaging evidence;
auto-update/staged rollout/rollback;
crash reporting opt-in and local crash bundles;
first-run privacy/provider setup;
offline/air-gap install;
fresh-VM smoke;
descriptor verification;
SBOM/provenance;
user docs;
support bundle redaction;
troubleshooting docs.
WS-QUALITY-01 — Evals, benchmarks, and dogfooding

Objective: make quality measurable and regressions hard to hide.

Implement/harden:

GP-1 through GP-6 definitions and automation;
Legion-Bench v0 local fixtures;
adversarial safety evals as blocking tests;
benchmark posture doc;
weekly dogfood journal;
performance dashboard artifacts;
crash-free metric once crash reporter exists;
provider cost per completed task;
acceptance/rejection metadata loop;
release-blocker taxonomy;
claim-audit script checking docs against ledgers.
8. Task packet template

For every selected task, produce and follow a packet with this shape:

Task ID:
Source:
Milestone:
Readiness row:
User-visible goal:
Non-goals:
Dependencies:
Files likely touched:
Authority-boundary check:
Mode-policy check:
Security/privacy check:
Mutation-path check:
Test-first plan:
Implementation plan:
Targeted verification:
Evidence path:
Docs/ledger updates:
Rollback/failure behavior:
Stop conditions:
Commands run:
Results:

Do not start code changes until this packet is clear.

9. Evidence requirements

Durable evidence belongs under the existing evidence structure, preferably:

plans/evidence/production/...
plans/evidence/gui-productization/...
plans/evidence/legion-e2e/...
other current repo-approved evidence paths.

Every evidence file should contain:

command;
working directory;
commit/SHA if available;
start/end time when practical;
exit code;
raw output;
platform/OS;
product path/golden path;
readiness row affected;
failure-mode result;
whether evidence promotes a gate or merely informs it.

Do not paste secrets, raw prompts, raw traces, private source, tokens, keys, or unredacted sensitive output into evidence.

10. Definition of done for any product-readiness promotion

A readiness row may move forward only when the same change set provides:

named golden path;
current test or smoke evidence;
user-visible UX path;
platform scope;
failure-mode behavior;
security/privacy review;
docs update;
readiness ledger update;
commands and results;
no contradiction with deferred cut lines.

If any item is missing, keep the row at its prior status and record the gap.

11. Stop conditions

Stop the current task and record a blocker rather than improvising if:

the task would require committing secrets, signing material, credentials, or provider keys;
implementation would require direct UI/provider/worker/plugin mutation of workspace files;
Manual mode would gain AI/network/cloud/worker surfaces;
a hosted/network path would activate without explicit policy and visibility;
raw source or prompt retention would become default-on;
a deferred product surface would be marketed or documented as shipped without evidence;
a new dependency violates dependency policy;
a runtime surface lacks ADR/policy/test/evidence;
a platform-specific claim cannot be tested or caveated;
a gate fails and the cause is unknown after focused debugging;
historical docs conflict with v0.2/readiness ledger and cannot be reconciled safely.

When blocked, produce:

blocker summary;
affected task/readiness row;
evidence;
safest next action;
whether an ADR, policy decision, secret provisioning, platform runner, or product cut-line decision is required.
12. High-priority starting sequence

Begin with the smallest sequence that improves truth, then Manual daily-driver utility.

Validate current docs and Kanban state:
cargo run -p xtask -- docs-hygiene
cargo run -p xtask -- check-deps
cargo run -p xtask -- verify-kanban-backlog
cargo test -p xtask --test kanban_backlog
Inspect plans/product-readiness-ledger.md, plans/legion-production-master-plan-v0.2.md, and plans/kanban/legion-ga-backlog.toml.
Confirm v0.2 is active.
Confirm v0.1 is historical.
Confirm readiness rows do not overclaim.
Confirm task dependencies resolve.
Start or continue M7/P0 if any truth/hygiene gaps remain.
Fix stale references.
Fix inconsistent product claims.
Preserve deferred cut lines.
Update docs/ledger with evidence only.
Move to M8/Manual daily-driver tasks.
Prefer tasks that directly advance GP-1:
renderer-backed input harness;
Manual mode surface filtering;
code canvas/productized editor;
large-file streaming/degraded mode;
Rust LSP launch/product workflow;
terminal runtime;
search/fuzzy open;
Git diff/stage/commit;
platform/accessibility smoke.
Only after GP-1 is stable, move to Assist.
provider policy;
context manifest;
proposal review;
validation;
rollback;
evidence export.
Only after GP-2 is stable, move to Delegate.
Only after GP-3 is stable, move to Workflows.
Only after GP-4 is stable, move to extensions/release/enterprise evidence for Production Beta and GA.
13. Final success condition

The mission is complete only when:

all relevant local gates pass or every unavailable gate is explicitly documented with reason;
GP-1 through GP-6 pass or deferred platform/provider/live-service caveats are explicit and accepted in the ledger;
product-readiness ledger contains current evidence for every promoted row;
Manual mode is daily-drivable and zero-egress;
Assist is inspectable, proposal-mediated, and useful;
Delegate produces scoped proposals/evidence without main workspace mutation;
Legion Workflows provide visible, stoppable, replayable multi-agent orchestration;
extensions are constrained and auditable;
remote/collaboration/admin surfaces are either product-evidenced or clearly deferred;
release/install/update/crash/support paths are evidenced;
docs match implementation;
no security/privacy claim exceeds implementation;
no P0/P1 blockers remain;
remaining deferred surfaces are named and not marketed as shipped.

Operate until the next unblocked task is complete, verified, evidenced, documented, and reviewed. Then select the next unblocked task and continue.