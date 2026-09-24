# P9.F2.T3 Evidence — Signed Org Policy Bundles

Branch: `p9-f2-t3-signed-policy-bundles`
Date: 2026-08-19
Readiness row: PR-ENT-002

## Task

> Add signed policy bundles: provider allowlists, MCP/tool allowlists, mode
> ceilings, budget caps, retention/export rules

Acceptance: *"Org policy pack enforces a restrictive enterprise profile end-to-end."*
Stop condition: *"Stop if a policy bundle is honored only for some surfaces."*

## What already existed, and what did not

`OrgPolicyBundle` already carried a mode ceiling and a full `SecurityPolicy`, and
`xtask/legion-policy.example.toml` already described a restrictive enterprise
profile. Three things were missing, and they are what this change adds:

1. **No signature.** `signature_label` was a `String` label. Nothing verified it,
   so the bundle was advisory: an operator could edit `mode_ceiling = "Assist"`
   to `"Automate"` and nothing would notice.
2. **No provider, MCP, budget, or retention-window rules.** `AiProviderPolicy`
   split local from remote by network target, which cannot express "Anthropic
   yes, everything else no". There was no MCP server/tool allowlist, no cost or
   token cap of any kind anywhere in the workspace, and no retention-window or
   export-destination rule.
3. **No structural guarantee of surface coverage.** Nothing made "the bundle
   reaches every surface" checkable.

## Signing: one scheme, not two

The Ed25519 verification primitive now lives in
`legion_security::verify_ed25519_signature` (`crates/legion-security/src/policy.rs`),
and `xtask::signing::verify_ed25519_signature` is a thin adapter over it. Release
manifests (ADR-0042) and policy bundles are therefore verified by the same code
with the same `verify_strict` semantics. `legion-security` is the right home
because `plans/dependency-policy.md` forbids any crate from depending on `xtask`,
so the primitive could not have gone the other way. The 17 pre-existing
`xtask/tests/manifest_sign.rs` tests pass unchanged through the shared primitive.

`SignedPolicyBundle` carries the exact TOML bytes that were signed, not a parsed
structure — re-serializing to verify would let a formatting difference break
verification, or let a semantically different re-serialization verify.

`VerifiedPolicyBundle` has **no constructor other than `SignedPolicyBundle::verify`**
and private fields. "An unsigned bundle honoured as if signed" is therefore not a
runtime mistake that can be made; it does not typecheck. `AppComposition::set_org_policy_bundle`
takes a `VerifiedPolicyBundle` for the same reason.

## Surface list

Every surface, how the bundle reaches it, and the test that proves it.

| # | Surface | How the bundle reaches it | Proving test |
|---|---------|---------------------------|--------------|
| 1 | **Mode ceiling** | `VerifiedPolicyBundle::decide` evaluates the ceiling first on every request; `AppComposition::set_product_mode` refuses an above-ceiling switch, and `set_org_policy_bundle` lowers a session already above it | `policy_bundle_surfaces.rs::every_surface_refuses_what_the_enterprise_bundle_forbids` (Mode case); `legion-app/tests/org_policy_mode_ceiling.rs` (5 tests) |
| 2 | **Provider allowlist** | `SecurityPolicy.bundle_enforcement.provider`, enforced inside `DenyByDefaultBroker::decide_with_context` before any per-family dispatch. `ProviderRouter::route_completion` now declares `ai_provider_id` in the request context | `policy_bundle_surfaces.rs` (Provider case, `broker_enforces_the_provider_allowlist_without_the_bundle_wrapper`); `legion-ai` `router_refuses_a_provider_outside_the_org_bundle_allowlist` / `router_permits_a_provider_on_the_org_bundle_allowlist` |
| 3 | **MCP / tool allowlist** | Same broker hook. `legion-agent`'s `check_broker_capability` now puts `mcp_server_id` / `mcp_tool_name` into the request context, so per-tool allowlisting is possible on the delegated-task path | `policy_bundle_surfaces.rs` (McpTool case, `broker_enforces_the_mcp_tool_allowlist_without_the_bundle_wrapper`, `allowlisted_server_does_not_admit_a_tool_that_is_not_allowlisted`); `legion-agent/tests/mcp_tool_allowlist_bridge.rs` (4 tests) |
| 4 | **Budget caps** | Same broker hook. Per-request cost, per-request tokens, and cumulative session spend. A `cloud.lane.submit` cost declared in the older `cloud_lane_estimated_cost_cents` field is capped too | `policy_bundle_surfaces.rs` (Budget case, `session_spend_accumulates_toward_the_session_cap`, `token_cap_refuses_independently_of_the_cost_cap`, `broker_caps_a_cloud_lane_cost_declared_in_the_legacy_field`) |
| 5 | **Retention window** | Same broker hook for `retention.*` capabilities, plus `FileBackedRawSourceVault::with_bundle_enforcement`, because the vault is not broker-mediated and decides capture locally | `policy_bundle_surfaces.rs` (Retention case); `legion-retention` `org_bundle_refuses_a_capture_window_longer_than_the_org_maximum` / `org_bundle_permits_a_capture_window_inside_the_org_maximum` |
| 6 | **Export rules** | Same broker hook for capabilities containing `.export`, plus the vault's `export_encrypted_bundle_hosted`, checked before consent validation | `policy_bundle_surfaces.rs` (Export case); `legion-retention` `org_bundle_refuses_a_hosted_export_the_retention_rules_forbid` / `org_bundle_refuses_an_export_destination_outside_the_allowlist` |
| 7 | **Base capability matrix** | `VerifiedPolicyBundle::check_capability` delegates to the bundle's own `DenyByDefaultBroker` | `policy_bundle_surfaces.rs` (Capability case) |

### Why the coverage is structural, not a promise

`PolicySurface` enumerates the seven surfaces. `VerifiedPolicyBundle::SURFACE_CHECKS`
holds one evaluator per variant, and `decide` **iterates that table** rather than
running a hand-written sequence of `if` blocks. Two tests hold the two halves
together:

- `surface_check_table_covers_every_declared_surface` — the runtime table must
  equal `PolicySurface::ALL` exactly.
- `every_surface_in_the_enumeration_is_exercised` — the refusal-case table in the
  test file must equal `PolicySurface::ALL` exactly.

Plus `policy_surface_all_lists_every_variant_exactly_once` in `policy.rs`, whose
exhaustive `match` will not compile when a variant is added without a slot.

Adding a surface without an evaluator, or without a refusal test, turns these red.

### Bypass resistance

Two bypasses were closed deliberately, and both are tested:

- **Omitting the operand.** A capability matching a surface's prefix but
  declaring no provider / no MCP tool / no cost / no retention window is
  *refused*, not skipped (`omitting_the_operand_a_surface_matches_on_is_a_refusal`).
- **Renaming the capability.** A request that names a provider or an MCP tool is
  checked whatever capability id it uses, so relabelling does not escape the
  allowlist (`a_renamed_capability_cannot_route_around_an_allowlist`).

The broker hook is placed *before* the per-capability-family dispatch in
`decide_with_context`, so a capability family that returns early further down
cannot escape it, and a family added later inherits the bundle for free.

## Negative tests

### Signature (`crates/legion-security/tests/signed_policy_bundle.rs`, 16 tests)

| Test | Proves |
|---|---|
| `tampered_payload_is_rejected` | appended bytes break verification |
| `relaxing_the_mode_ceiling_after_signing_is_rejected` | the headline attack: `Assist` rewritten to `Automate` is refused |
| `widening_the_provider_allowlist_after_signing_is_rejected` | adding `openai` to the allowlist post-signing is refused |
| `tampered_signature_is_rejected` | a flipped signature byte is refused |
| `signature_from_a_different_key_is_rejected` | an impostor who knows the key id but not the key is refused |
| `empty_keyring_honours_nothing` | no trust anchors means no bundle applies, never every bundle |
| `unknown_key_id_is_rejected` | an unconfigured signer is refused |
| `unsigned_bundle_cannot_be_honoured_as_signed` | `algorithm = "none"`, blank algorithm, and empty signature are all refused |
| `algorithm_match_is_exact_not_case_folded` | `ED25519` is a downgrade, not a spelling |
| `undecodable_signature_is_rejected` | non-base64 signature |
| `malformed_trust_anchor_is_rejected_rather_than_skipped` | a bad anchor fails closed instead of being skipped |
| `wrong_length_trust_anchor_is_rejected` | a 16-byte "key" fails as a bad key |
| `unparseable_payload_is_rejected_even_with_a_valid_signature` | a correctly signed non-bundle does not become a bundle |
| `unsupported_schema_version_is_rejected_even_when_signed` | schema 99 is refused despite a valid signature |

### Per-surface refusals

Every row of the surface table above has a refusal case that asserts three
things: that the request was denied, that **the intended surface** denied it, and
that the denial reason contains the expected fragment. Asserting the surface is
what stops a case passing because something unrelated refused it.

Refusals proven: a provider not on the allowlist; an MCP tool not on the
allowlist; an MCP tool on an allowlisted server that is not itself allowlisted; a
mode above the ceiling; a request over the per-request cost cap, over the token
cap, and over the session cap; a retention window longer than the org maximum; an
export the rules forbid; and an export to a destination outside the allowlist.

## Mutation results

Each mutation was applied, the named test run, and the file restored with
`git checkout --`. `git status` was verified clean after the sequence.

| # | Mutation | Test run | Result |
|---|---|---|---|
| M1 | `SignedPolicyBundle::verify` — drop the `verify_ed25519_signature` call | `signed_policy_bundle` | **KILLED** — 7 failures |
| M2 | `verify` — drop the `keyring.is_empty()` guard | `signed_policy_bundle` | **KILLED** — 1 failure |
| M3 | `verify` — drop the algorithm check | `signed_policy_bundle` | **KILLED** — 2 failures |
| M4 | `ProviderAllowlistPolicy::refusal` — return `None` unconditionally | `policy_bundle_surfaces`, `legion-ai --lib` | **KILLED** — 4 + 1 failures |
| M5 | `McpToolAllowlistPolicy::refusal` — return `None` unconditionally | `policy_bundle_surfaces`, `mcp_tool_allowlist_bridge` | **KILLED** — 5 + 2 failures |
| M6 | `BudgetCapPolicy::refusal` — return `None` unconditionally | `policy_bundle_surfaces` | **KILLED** — 6 failures |
| M7 | `RetentionExportPolicy::retention_refusal` — return `None` unconditionally | `policy_bundle_surfaces`, `legion-retention --lib` | **KILLED** — 3 + 1 failures |
| M8 | `RetentionExportPolicy::export_refusal` — return `None` unconditionally | `policy_bundle_surfaces`, `legion-retention --lib` | **KILLED** — 2 + 2 failures |
| M9 | `SURFACE_CHECKS` — replace the `Mode` evaluator with a no-op | `policy_bundle_surfaces`, `org_policy_mode_ceiling` | **KILLED** by surfaces — 1 failure; app tests **survived** (see note) |
| M10 | Broker hook — remove `bundle_enforcement.refusal` from `decide_with_context` | `policy_bundle_surfaces` | **KILLED** — exactly 5 failures, all `broker_*`; bundle-level tests survived (see note) |
| M11 | `legion-agent` — revert `check_broker_capability` to `CapabilityRequestContext::default()` | `mcp_tool_allowlist_bridge` | **KILLED** — 2 failures |
| M12 | `legion-ai` — set `ai_provider_id: None` in the route context | `legion-ai --lib` | **KILLED** — 1 failure |
| M13a | `legion-retention` — change the capture check's capability string | `legion-retention --lib` | **KILLED NOTHING** — see finding below |
| M13b | `legion-retention` — remove the capture-window guard from `capture_bundle` | `legion-retention --lib` | **KILLED** — 1 failure |
| M14 | `legion-retention` — remove the export guard from `export_encrypted_bundle_hosted` | `legion-retention --lib` | **KILLED** — 2 failures |
| M15 | `legion-app` — remove the ceiling early-return from `set_product_mode` | `org_policy_mode_ceiling` | **KILLED** — 1 failure |
| M16 | `allowlist_contains` — return `true` for an empty allowlist | `legion-security --lib` | **KILLED** — 3 failures |

Working tree verified clean (`git status --short` empty) after restoring, and the
four affected crates re-tested green.

### The mutation that killed nothing (M13a)

M13a changed the capability string passed to `retention_refusal` in
`capture_bundle` from `"retention.raw_source.capture"` to a string matching no
prefix. Every test still passed.

Investigated: this is not a coverage gap, it is the operand trigger working as
designed. `retention_refusal` fires on **either** a matching capability prefix
**or** a declared retention window, precisely so that renaming a capability
cannot switch a rule off. `capture_bundle` always declares the window, so the
rule still fired and the guard still refused. The mutation removed the prefix
half of a deliberately two-sided trigger and the other half held.

Confirmed by M13b, which removes the guard outright and does kill the test. And
the property M13a accidentally demonstrated has its own test at the bundle level:
`a_renamed_capability_cannot_route_around_an_allowlist`. Recorded rather than
discarded because "the mutation killed nothing" was the honest first observation,
and the reason it killed nothing is a property worth naming.

### Notes on masking

**M9 is a partial-mask finding and is recorded as one.** Replacing the `Mode`
evaluator in the surface table kills the bundle-level Mode case, but the
`legion-app` mode-ceiling tests keep passing, because
`AppComposition::set_product_mode` calls `bundle().allows_mode(..)` directly
rather than going through `decide`. That is two enforcement points for one rule,
which is deliberate — the app gate stops the user stranding themselves in a mode
that cannot act, the per-request gate is the security boundary — but it does mean
neither test alone covers both. Both exist, and M9 and M15 show each is killed by
its own test.

**M10 is the masking-detection pair for M4–M8.** M4–M8 mutate the shared rule
functions and kill both the bundle-level and broker-level tests. M10 mutates only
the broker's *call* to those rules, and kills exactly the five `broker_*` tests
while every bundle-level test survives. That asymmetry is the evidence that the
two enforcement points are separately covered rather than one masking the other.

### A wrong prefix the tests caught during development

An earlier draft used `delegate.tool.mcp_passthrough` as the MCP capability
prefix. The `legion-agent` refusal tests passed anyway — the *operand* trigger
was catching the request, not the prefix. The real capability id is
`delegate.tool.mcp-passthrough` (hyphen: `LegionToolKind::tool_name()` returns
`"mcp-passthrough"`), so the prefix matched nothing. The isolation test
`the_broker_receives_the_mcp_server_and_tool_identity` surfaced it. Left unfixed,
an MCP passthrough declaring *no* server or tool would have slipped past the
allowlist instead of being refused for failing to declare one. Fixed in the
source, the example bundle, and every test.

## A surface finding worth recording

`DenyByDefaultBroker` has **no `delegate.tool.*` arm**, so its base capability
matrix refuses every delegated tool call outright, independent of any bundle.
This matters for test design: a `DenyByDefaultBroker`-based test of the MCP
allowlist on that path would pass whether or not the allowlist did anything. The
`legion-agent` tests therefore use a broker that applies *only* the allowlist, so
the allowlist is the only thing under test, and
`the_enterprise_bundle_still_permits_what_it_allows` documents the base-matrix
denial explicitly rather than working around it silently. This is a pre-existing
property of the broker, not something this change introduced, and it is not
something a policy bundle can fix.

## Backward compatibility

Every new rule set defaults to `enforced = false`, and
`broker_without_a_bundle_is_unaffected_by_the_new_rules` /
`a_vault_without_an_org_bundle_is_unaffected` /
`default_bundle_enforcement_refuses_nothing` hold that default in place. An
installation with no org bundle behaves exactly as before.

## Commands run

```
cargo fmt --all
cargo clippy --workspace --all-targets -j 6 -- -D warnings     # clean
cargo test -p legion-security -j 6                             # pass
cargo test --workspace -j 6                                    # pass
cargo run -p xtask -- check-deps                               # dependency policy checks passed
cargo run -p xtask -- docs-hygiene                             # pass
cargo run -p xtask -- claim-audit                              # pass
cargo run -p xtask -- extract-before-modify                    # pass
cargo run -p xtask -- verify-kanban-backlog                    # pass
cargo run -p xtask -- verify-readiness-consistency             # pass
```

## Files changed

- `crates/legion-security/src/policy.rs` — signing primitive, `SignedPolicyBundle`,
  `VerifiedPolicyBundle`, `PolicySurface`, the five per-surface policies
- `crates/legion-security/src/lib.rs` — `SecurityPolicy.bundle_enforcement`, broker hook, re-exports
- `crates/legion-security/Cargo.toml` — `ed25519-dalek`, `base64`, `toml`
- `crates/legion-protocol/src/lib.rs` — eight `CapabilityRequestContext` operand fields
- `crates/legion-protocol/tests/dto_contracts.rs` — golden updated with the new operands
- `crates/legion-ai/src/lib.rs` — provider id in the route context; 2 tests
- `crates/legion-agent/src/agent_loop.rs` — MCP operands in the broker context
- `crates/legion-retention/src/lib.rs` — vault bundle enforcement; 5 tests
- `crates/legion-app/src/lib.rs` — org bundle field, mode ceiling gate
- `xtask/src/signing.rs` — delegates to the shared primitive
- `xtask/legion-policy.example.toml` — the five enforcement sections
- New tests: `crates/legion-security/tests/signed_policy_bundle.rs`,
  `crates/legion-security/tests/policy_bundle_surfaces.rs`,
  `crates/legion-agent/tests/mcp_tool_allowlist_bridge.rs`,
  `crates/legion-app/tests/org_policy_mode_ceiling.rs`
