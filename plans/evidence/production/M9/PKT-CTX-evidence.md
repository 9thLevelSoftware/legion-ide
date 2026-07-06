# PKT-CTX Evidence — Context Manifest and Privacy Inspector

Branch: `m9/context-manifest`

## Task Commits

| SHA | Task | Conventional commit |
|-----|------|---------------------|
| 015dbc4 | T1 | feat: assemble context manifest from real sources (P4.F2.T1) |
| ae7816d | T2 | feat: pre-invocation manifest UI with per-item exclusion (P4.F2.T2) |
| febdc4b | T3 | test: broaden egress-equality tests across providers and categories (P4.F2.T3) |
| 4732aa2 | T4 | feat: privacy inspector deletion wired to retention vault (P4.F2.T4) |

---

## T1 — assemble_context_manifest_from_sources + 7 collector functions

**New production symbols** (all in `crates/legion-ai/src/manifest.rs`):
- `ManifestMetadata` — assembly metadata DTO (workspace_id, proposal_id, purpose, trust state, privacy/risk/egress, permissions, timestamp, schema_version)
- Source descriptor DTOs: `ManifestFileSource`, `ManifestSelectionSource`, `ManifestSymbolSource`, `ManifestTerminalExcerpt`, `ManifestMemoryRecordSource`, `ManifestRuleRecordSource`
- `collect_file_context(paths, workspace_id) -> Vec<ContextManifestItem>`
- `collect_selection_context(selections) -> Vec<ContextManifestItem>`
- `collect_symbol_context(symbols) -> Vec<ContextManifestItem>`
- `collect_diagnostic_context(diagnostics) -> Vec<ContextManifestItem>`
- `collect_terminal_context(excerpts) -> Vec<ContextManifestItem>`
- `collect_memory_context(memory_items) -> Vec<ContextManifestItem>`
- `collect_rules_context(rules) -> Vec<ContextManifestItem>`
- `assemble_context_manifest_from_sources(sources, metadata) -> ContextManifestRecord`
- `compute_manifest_id(items) -> String` — FNV-1a 64-bit over sorted item IDs

**manifest_id determinism**: FNV-1a 64-bit (not `DefaultHasher`) ensures stable IDs across invocations when hash randomization is enabled.

**Test file**: `crates/legion-ai/tests/context_manifest.rs`  
**Tests added**: 5 (assemble_from_file_sources, omitted_count, stale_detection, manifest_id_determinism, empty_sources)  
**Total passing**: 7

---

## T2 — Pre-invocation manifest UI with per-item exclusion

**New production symbols** (all in `crates/legion-desktop/src/view/manifest_panel.rs`):
- `DesktopManifestItemToggleViewModel { item_id, kind, current_inclusion, can_exclude }`
- `manifest_item_toggle_view_models(manifest) -> Vec<DesktopManifestItemToggleViewModel>`
- `toggle_manifest_item_inclusion(manifest, item_id) -> bool` — flips Included↔Excluded; mandatory items (label `"mandatory"`) are refused; recomputes `omitted_item_count`
- `preview_rows(snapshot) -> Vec<String>` — enhanced: shows `[LEAVES_MACHINE]` for `ExternalEgressMetadata`/`RemoteApprovalRequired` egress, `can_exclude=` per item, mandatory-item warning row, "before invocation" header

**Test file**: `crates/legion-desktop/tests/manifest_panel.rs`  
**Tests**: 5 (toggle_flips_state, omitted_count, preview_rows_show_items, egress_marked, mandatory_cannot_exclude)  
**Total passing**: 5

---

## T3 — Broaden egress-equality tests

**Test file**: `crates/legion-ai/tests/egress_equality.rs`  
**Tests added**: 4

| Test | Property verified |
|------|-------------------|
| `egress_equality_all_seven_source_categories` | All 7 kinds (File, UserSelection, SemanticRecord, LspDiagnosticSummary, TerminalSummary, MemoryRecord, Rule) survive Included egress pass |
| `egress_equality_with_mixed_inclusion_states` | Excluded/Redacted/Denied/Omitted items stripped; only Included remains |
| `excluded_items_never_appear_in_egress_bytes` | Serialised JSON contains no excluded item IDs |
| `egress_is_deterministic_across_assembly_paths` | `assemble_context_manifest` and `assemble_context_manifest_from_sources` produce byte-equal egress item lists for same input items |

**Total passing**: 5

---

## T4 — Privacy inspector deletion wired to retention vault

**New production symbols**:

`crates/legion-retention/src/lib.rs`:
- `pub trait RawSourceVault` — `vault_delete_bundle`, `vault_read_bundle_descriptor`
- `impl RawSourceVault for RetentionFixtureVault` — converts `RetentionFixtureError` → `RawSourceVaultError`
- `impl<K, C> RawSourceVault for FileBackedRawSourceVault<K, C>` — delegates to existing methods
- `RetentionFixtureVault::lookup_bundle(bundle_id) -> Option<&RawSourceRetentionBundleDescriptor>`
- `pub mod training;` and `pub mod privacy;` declarations

`crates/legion-retention/src/privacy.rs` (new):
- `lookup_retention_bundle<V: RawSourceVault>(vault, bundle_id) -> Result<Descriptor, Error>`
- `delete_retention_record<V: RawSourceVault>(vault, tombstone) -> Result<Tombstone, Error>`
- `format_deletion_handle(tombstone) -> String` — metadata-only handle, no raw source content
- `execute_privacy_deletion<V: RawSourceVault>(vault, bundle_id, reason, timestamps...) -> Result<String, Error>` — full end-to-end: verify existence → build tombstone → delete → return handle

**Test file**: `crates/legion-retention/tests/privacy_deletion.rs`  
**Tests**: 4 (removes_bundle, missing_bundle_error, lookup_returns_descriptor, handle_is_metadata_only)  
**Total passing**: 4

---

## Standing gate

Full workspace: `cargo test --workspace` — 0 failures across all crates.

## Dependency policy compliance

- `legion-ai` depends only on `legion-protocol` (and `legion-security`). No new dependencies added.
- `legion-retention` depends only on `legion-protocol` (and existing crypto/storage deps). No `legion-desktop` dependency.
- `legion-desktop` depends on `legion-ui` and `legion-protocol` for T2. No cross-boundary violations.
