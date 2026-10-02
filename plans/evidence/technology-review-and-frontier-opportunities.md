# Legion technology review and frontier opportunities

## Recommendation

Build **evidence-linked change intelligence**, not another agent chat interface: a developer should be able to ask what a proposed change affects, inspect the evidence supporting it, see exactly which assumptions were checked, and know when those checks became stale.

The strongest research extensions are counterexample-guided verification and incremental, semantics-preserving patch search. They fit Legion's existing proposal authority better than replacing its renderer, adding more agent roles, or building a proprietary model runtime.

**Novelty boundary:** this review cannot establish that a technology has never appeared in any IDE or private product. Formal verification IDEs, code canvases, semantic indexes, parallel agents, checkpoints, and local models already exist. The recommendations below are differentiated integration hypotheses, not first-ever or patentability claims. No reviewed mainstream competitor documentation established the complete proposed combination of snapshot-bound evidence, dependency-aware invalidation, counterexample-guided verification, and human-controlled application. Absence from those documents is not proof of absence from the products.

## 1. What Legion actually has

This assessment combines current source inspection with the controlling readiness ledger. A dependency declaration is not an active feature, a contract test is not an installed workflow, and historical acceptance is not current product readiness.

| Area | Current technology and capability | Important limit |
| --- | --- | --- |
| Native application | Rust 2024, MSRV 1.92; eframe/egui desktop; protocol DTOs, app composition, projection-only UI | UI/accessibility remains substrate-validated, not cross-platform daily-driver proof. Sources: `Cargo.toml`, `docs/ARCHITECTURE_AUTHORITY_BOUNDARIES.md`, `plans/product-readiness-ledger.md`. |
| Text/editor | Ropey-backed text, bounded snapshots/chunks, UTF coordinate conversion, buffer transactions and undo/redo | Text-model scale is not input-to-paint performance. Sources: `crates/legion-text/Cargo.toml`, `crates/legion-text/src/lib.rs`, `crates/legion-editor/src/lib.rs`, readiness PR-UI-002. |
| Language tooling | Tree-sitter grammars and LSP process/session plumbing, including rust-analyzer integration | In the reviewed semantic parser worker, Rust has actual Tree-sitter parsing; other paths include lexical fallbacks. Changed-range transport and exact-key tree caching do not establish edit-incremental parsing. Sources: `crates/legion-index/src/lib.rs`, `crates/legion-app/src/language/session.rs`. |
| Search/context | In-memory semantic records, hybrid deterministic ranking, provenance/freshness, background workspace search | Deterministic vectors are not learned embeddings. Tantivy is declared in the workspace but is not a dependency of `legion-index`; do not call that index Tantivy-backed. Sources: `Cargo.toml`, `crates/legion-index/Cargo.toml`, `crates/legion-index/src/lib.rs`, `crates/legion-app/src/search.rs`. |
| Models | Local HTTP adapters for Ollama/llama.cpp and hosted provider adapters; deterministic provider/embedding paths | Adapter support does not prove a model is installed. No embedded neural inference runtime was found in the reviewed manifests. Source: `crates/legion-ai-providers/src/lib.rs`. |
| AI control plane | Scoped agent loops, worktree lanes, proposal review/apply, context manifests, budgets, audit metadata and runtime-state replay | Metadata replay is not arbitrary process or filesystem replay. AI-safety substrate still needs broader product evidence. Sources: `crates/legion-agent/src/state.rs`, `crates/legion-agent/src/worktree.rs`, readiness PR-AI-001/002. |
| Graph/canvas | Semantic graph-record DTOs plus a spatial file-card canvas with human-drawn connections | Extracted call/import facts are not a dependable resolved cross-file impact graph. Canvas explicitly excludes derived edges rather than inventing them. Sources: `crates/legion-index/src/lib.rs`, `crates/legion-desktop/src/view/canvas_workspace.rs:8-22`. |
| Security/interop | Capability policies, Wasmtime dependency, platform sandbox implementations, LSP/DAP and agent/tool integration surfaces | Sandbox guarantees depend on platform and launched path. Runtime extension host, remote UX, and collaboration/admin product gates remain deferred. Sources: `Cargo.toml`, `crates/legion-sandbox/src/lib.rs`, `plans/product-readiness-ledger.md`. |

**Keep the stack.** Rust, ropes, egui, Tree-sitter, LSP, and proposal-mediated workspace authority are suitable foundations. This research found no evidence that a framework replacement would deliver more value than completing reliable editing, language workflows, accessibility, containment, and installation. The readiness gaps are independently important; frontier features cannot compensate for data-loss or daily-driver failures.

## 2. Features that are not a unique edge

- **Parallel agents and worktree tournaments:** Cursor 3 documents parallel local/cloud/SSH agents, `/worktree`, and `/best-of-n`. Merely adding agent fleets or branch comparison would not differentiate Legion. [Cursor changelog](https://cursor.com/changelog/3-0).
- **Local models, checkpoints, permissions, and OS sandboxing:** Zed documents local-provider options, multiple agent threads, worktree isolation and checkpoints; its sandbox docs explicitly describe enforcement and platform caveats. These are baseline capabilities, not exclusive moats. [Agent panel](https://zed.dev/docs/ai/agent-panel.md), [sandboxing](https://zed.dev/docs/ai/sandboxing).
- **Semantic retrieval and incremental content indexing:** Augment documents embedding-backed search, content hashes, incremental updates, and MCP access. Graph/context marketing alone is insufficient differentiation. [Context Connectors architecture](https://docs.augmentcode.com/context-services/context-connectors/how-it-works).
- **Specifications and plans:** Kiro already generates requirements, design and implementation tasks through a reviewed workflow. [Requirements-first workflow](https://kiro.dev/docs/specs/feature-specs/requirements-first.md).
- **A canvas editor:** Haystack's product README explicitly describes a canvas UI, code navigation and debugging. [Haystack primary source](https://github.com/haystackeditor/haystack-editor).
- **Formal verification in an IDE:** Dafny already has a static verifier, IDE plugins and an LSP-based language server. Novelty must lie in the workflow and assurance boundaries, not displaying proof results. [Dafny](https://dafny.org/).

These are documentary comparisons, not hands-on competitor benchmarks. They do not establish relative latency, security, model quality, or completeness.

## 3. Ranked opportunities

### Priority 1 — Evidence-linked change graph

**User outcome:** clicking a proposed symbol change shows resolved consumers, candidate affected tests, the source of each relationship, and which verification results still apply. An agent explanation is visibly distinct from a compiler-resolved fact or a human-drawn connection.

**Technology:** reuse Legion's semantic/proposal/audit contracts. Evaluate compiler/LSP-derived relationships or SCIP imports for stable symbol identities; use an explicit dependency graph and revision-keyed invalidation. Salsa is a mature reference for deterministic incremental recomputation, not a mandatory replacement for the existing scheduler. SCIP supports occurrences, references and implementation relationships; it does not itself prove runtime causality or capture every dynamic dependency. [SCIP indexer specification](https://sourcegraph.com/docs/code-navigation/writing-an-indexer), [Salsa overview](https://salsa-rs.github.io/salsa/overview.html).

**Actual increment over Legion:** resolved cross-file relationships and dependency-aware evidence invalidation, rather than merely adding graph DTOs, citations or a canvas that already exist. Distinguish a code dependency graph from causal IDs in an execution ledger.

**Assurance boundary:** bind each verification envelope to the exact proposal content, source snapshot, relevant dependencies, tool version/configuration, command, exit status, checked properties and explicit limits. A content hash detects mismatches; it does not make a test truthful or exhaustive. For changes without complete dependency coverage, invalidate conservatively and show unknowns rather than declaring unaffectedness.

**Proposed acceptance:** fixed cross-file/rename/delete scenarios; compare impact precision and recall against an independently maintained oracle; demonstrate that source/dependency/tool-config changes invalidate the right evidence; no old result appears current after a snapshot change. Measure incremental update latency, memory and unnecessary rechecks separately. Begin with one well-supported language, then prove each additional language.

**Risk:** incomplete call resolution, macros, dynamic dispatch, generated code and external dependencies. Never derive authoritative impact claims solely from the current lexical graph. Productize the existing proposal review path before introducing another authority or storage system.

### Priority 2 — Counterexample-guided verification lane

**User outcome:** for an eligible Rust change, the IDE presents a concrete failing input and a checked property, not just an agent saying that tests passed. A subsequent repair remains a proposal requiring review.

**Newer research:** ExVerus (ICML 2026) uses validated counterexamples to guide Verus proof repair and invariant generation. Its reported evaluation concerns proof generation/repair; it is not evidence that arbitrary application bugs can be solved or that Legion can reproduce its gains. [Microsoft Research publication](https://www.microsoft.com/en-us/research/publication/exverus-verus-proof-repair-via-counterexample-reasoning/).

**Practical entry:** evaluate Kani proof harnesses for small pure Rust components and user-approved assertions; investigate Verus separately for explicitly specified, supported code. Kani can prove/disprove properties or exhaust resources, and does not support every Rust feature, including concurrency. Its documented easy-install platforms are Linux and macOS, not native Windows. An approved Linux worker or separately validated toolchain would therefore be a prerequisite for a Windows product path, not a hidden fallback. [Kani guarantees](https://model-checking.github.io/kani/), [limitations](https://model-checking.github.io/kani/limitations.html), [installation](https://model-checking.github.io/kani/install-guide.html).

**Proposed acceptance:** on eligible functions, replay validated counterexamples; record checked assertions, bounds, solver/tool version and assumptions; separately label proved, disproved, unsupported and resource-exhausted results. Contract edits require human review. Never let an agent weaken a property to make verification green. Track counterexample detection and eligible-contract verification rates; retain ordinary tests and explicit unsupported cases.

**Risk:** specification quality and verifier support. A proof applies to stated properties under its model/assumptions, not to all behavior of the application. Testing cannot be relabeled proof. This is a specialized lane, not a requirement that every project adopt formal methods.

### Priority 3 — Incremental verified refactoring search

**User outcome:** the IDE offers several equivalent implementations for a small, well-defined expression, with a checkable rewrite explanation and a transparent cost model, instead of asking a model to guess an optimization.

**Technology frontier:** egglog combines equality-saturation applications with a Rust API; 2025 incremental equality-saturation research reuses a versioned e-graph across successive inputs. The paper reports gains on an algebraic simplifier, notes that some individual terms favor standard saturation, and assumes a constant ruleset with important cost-model restrictions. These results do not establish gains on an IDE codebase. [egglog documentation](https://docs.rs/egglog/latest/egglog/), [2025 research paper](https://rupanshusoi.github.io/pdfs/egraphs-25.pdf).

**Pilot boundary:** a typed, side-effect-free IR with an audited rewrite set, fixed node/iteration budgets, versioned rules and an independent semantic checker. Preserve integer width, overflow/panic semantics and evaluation behavior; exclude floating-point reassociation, I/O, mutation and concurrency until separately justified. Do not confuse equality under supplied rules with correctness of the rules for real source-language semantics.

**Proposed acceptance:** held-out eligible expressions; no accepted inequivalent rewrite against the independent checker; measure search cost, incremental reuse, emitted-code cost and actual workload performance separately. Reject the pilot if compile overhead, graph growth or complexity exceeds demonstrated user benefit.

**Risk:** research-engine maturity, rewrite soundness and scope. This is the most experimental option and should follow evidence/workflow productization, not delay Manual-mode reliability.

### Enabler, not headline — Capability-scoped structured local output

XGrammar uses token masks to constrain generated output to a grammar. It can improve proposal/tool payload structure, but structural validity is not authorization, semantic correctness, or prompt-injection immunity. [Constrained decoding](https://xgrammar.mlc.ai/docs/latest/start/constrained_decoding.html).

Evaluate grammar support through an already supported local inference endpoint before adopting another runtime. Keep schemas aligned with protocol DTOs and enforce capabilities at execution time. Measure malformed payload rate, correct task completion, cancellation and decoding overhead on fixed inputs. Do not claim this technology is unique: structured generation and local inference are enabling infrastructure.

## 4. Adoption decision

1. Preserve current authority/privacy boundaries and complete the ordinary IDE workflows needed to use and review changes safely.
2. Prioritize the evidence-linked graph as the product differentiator; start with conservative freshness and real relationships, not attractive guessed edges.
3. Evaluate the specialized counterexample lane on supported Rust functions and an explicitly provisioned environment.
4. Run equality-saturation research only after a narrow domain, semantic oracle and cost target exist.
5. Keep local structured decoding and learned retrieval as separately measured enablers. Do not train a proprietary model, add a vector database, or replace the UI framework merely to appear newer.

Any new product surface still needs the repository's ADR, policy, dependency and acceptance process. Nothing in this report promotes deferred readiness rows or authorizes cloud egress/autonomous apply.

## 5. Evaluation baseline and limits

The existing recorded Legion-Bench is the appropriate initial offline baseline: it copies fixture repositories, executes the actual delegated-agent/tool/proposal/apply paths, and runs each task's verification command while replaying only provider replies. There are 25 task definitions, 20 executable governed cassettes, and five excluded holdouts. [Repository contract](../../evals/legion-bench/recorded/README.md); source: `xtask/src/legion_bench_live.rs`, `xtask/src/legion_bench.rs`, `crates/legion-app/src/bin/legion_bench_live.rs`.

The autoresearch harness emits the existing integer average task score, with passed tasks, verification passes and cassette drift as secondary observations. It freezes 109 task/fixture/cassette/scorer/lockfile inputs, remains offline, and never rewrites the reference baseline. Failed reference tasks are legitimate measurements; infrastructure failure is not success.

**This is a baseline for existing workflow behavior, not a score for novelty or for the proposed graph/proof features.** Fixed tapes limit generalization: request drift can make a tape no longer representative, and public fixture contents invite overfitting. Future feature experiments need their own independent impact, freshness, proof and semantic-equivalence workloads before comparative claims are justified. A higher replay score alone cannot select a frontier technology or establish a competitive moat.

The frontier recommendations above remain research findings and proposed acceptance criteria, not implemented features. Harness setup changed no product behavior; subsequent correctness experiments are recorded below.

## 6. Exercised baseline evidence

Entry point: `bash autoresearch.sh`. Supporting files are `scripts/autoresearch_bench.py` and `scripts/autoresearch_bench_inputs.json`. The helper executes `cargo run --offline --locked -p xtask -- legion-bench --mode recorded --corpus evals/legion-bench/tasks --cassettes evals/legion-bench/recorded --out target/autoresearch-bench` and verifies fresh runner/report data before emitting metrics.

Two consecutive completed runs exited successfully and produced identical metrics:

```text
METRIC recorded_task_score=59
METRIC passed_tasks=4
METRIC verification_passes=5
METRIC cassette_drift=5
```

Both executed 20 tasks and excluded five holdouts. Sixteen reference tasks failed their scored acceptance; that is not a green product-task suite. Five task verification commands passed, but only four tasks met their full acceptance/budget criteria. The five drifted exchanges are visible baseline limitations, not zero-drift evidence.

Observed toolchain: Windows Cargo 1.98.1, Git 2.55.0.windows.3, Python 3.12.10 and Node 24.19.0. On this workstation `bash` starts WSL; the launcher bridges to native Windows Python and paths so the workload uses the installed Windows tools. Ordinary Bash environments retain the normal Python path. Harness prerequisites are Bash, Python >=3.11, Git, Cargo with cached dependencies, and Node for existing JavaScript fixtures. The child runner currently explicitly builds with four jobs, overriding the inherited one-job Cargo setting; no frozen runner code was changed.

Passing an argument to the fixed-workload launcher was exercised and rejected with exit 2 and no metric output. The initial WSL/Linux-Python prerequisite failure was fixed in the launcher; no tooling installation was needed. Benchmark outputs stay under ignored `target/autoresearch-bench/`; temporary fixture checkouts are managed by the existing runner. Documentation hygiene passed.

## 7. Autoresearch correctness iterations

The recorded score remains a workflow proxy. Score-neutral changes are retained only when an independent behavioral reproduction demonstrates a real correctness or containment defect and every previously passing task/verification remains successful with no increased per-task cassette drift. They are not reported as benchmark optimization or frontier-feature delivery.

| Run | Candidate | Recorded score / passed / verification / drift | Exercised proof and disposition |
| --- | --- | --- | --- |
| 1 | Unchanged tracked baseline | 59 / 4 / 5 / 5 | Reproduced setup metrics; baseline recorded. |
| 2 | Bind fragment drafts to their original source fingerprint and length; stage only accepted proposals | 59 / 4 / 5 / 5 | Deterministic external-write regression failed before the fix and passed afterward. Three snapshot/accepted-draft/expected-absence regressions and five existing fragment integration tests passed. Individual recorded acceptance and drift did not regress. Kept as score-neutral authority repair. |
| 3 | Honor nullable optional replacement when an exact fragment is supplied | 59 / 4 / 5 / 5 | Schema-valid anchored input failed before the fix and passed afterward without writing disk or losing the original guard. Four draft regressions and five fragment integrations passed; individual recorded acceptance/drift unchanged. Kept as score-neutral schema-contract repair. |
| 4 | Reject simultaneous full-file and fragment fields | 59 / 4 / 5 / 5 | Declined: the current schema does not prohibit both fields and full replacement has explicit whole-content semantics. A speculative refusal assertion failed, but adding rejection would invent contract narrowing rather than establish a defect. Discarded test; no product policy change. |
| 5 | Do not follow discovered directory/file links in recursive grep/glob | 59 / 4 / 5 / 5 | Real Windows junctions reproduced external canary leakage and recursive cycle duplicates before the fix. Both regressions and existing forbidden-descendant behavior passed afterward. Individual recorded acceptance/drift unchanged. Kept as score-neutral containment repair. |
| 6 | Report grep filesystem failures instead of successful incomplete observations | 59 / 4 / 5 / 5 | A removed validated search directory and a Windows exclusively locked text file both falsely returned success before the fix. Both now return runtime failure; releasing the lock restores the intended match. Five search/containment regressions passed; individual recorded acceptance/drift unchanged. Kept as score-neutral observation repair. |
| 7 | Report glob traversal failures instead of successful incomplete observations | 59 / 4 / 5 / 5 | Removing a validated selected directory reproduced false successful emptiness before the fix and runtime failure afterward. Six search/containment regressions passed; individual recorded acceptance/drift unchanged. Kept as score-neutral observation repair. |
| 8 | Fail closed on nonregular/unreadable proposal bases; stream original guard bytes | 59 / 4 / 5 / 5 | Directory and exclusively locked file targets incorrectly produced proposals before the fix and were rejected afterward. Seventeen proposal-related tests passed, including normal absence/existing guards and a 16,385-byte independent fingerprint reference crossing read boundaries. Individual recorded acceptance/drift unchanged. Kept as score-neutral proposal-authority repair. |

Run 2 uses a compact numeric original-base guard with the existing FNV content-version convention. It refuses a draft if proposal generation captures a different on-disk base, preserves previously accepted draft content after refusal, and detects external creation of a previously absent draft target. It adds no direct workspace writes, source inference, new provider request shape, dependency, or public API.

Run 8 permits absence only after an initial `NotFound`; errors after observing an existing target do not become unguarded creation. Nonregular targets are rejected before opening; one regular-file handle supplies metadata and bounded-buffer hashing. Existing content-version/fingerprint values, including the historical truncated 128-bit FNV convention and its existing algorithm label, are unchanged. This is a correctness and allocation-bound repair, not a measured latency improvement.
