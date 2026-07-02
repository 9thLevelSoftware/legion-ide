# Quaternius Calibration Adapter — Smallest-Change Proposal (no implementation)

Status: design only. No files modified. Survey of `off-the-rails-ai-infra` and `the-sargasso-of-stars` complete.

## 1. Context that drives the design

The user saw jumbled Quaternius renders and approved a calibration plan. The goal is a *data-driven* calibration scene for one room-module set that:

- Reuses the existing wrapper-scene + orchestrator + slop-detector stack as-is.
- Does NOT touch the M2 motif schema, the M3 zone schema, the M4 wrapper contract, the M5 scorer, or the Godot validator. Those are settled.
- Adds the smallest possible surface: a thin "kit adapter" config that says "for kit X, source Y, override Z", plus one new top-level entry point that emits a *calibration* scene/manifest variant and a calibration report.

The existing pipeline already has a working skeleton for the Quaternius subset: 23 wrapper bundles at `off-the-rails-ai-infra/artifacts/quaternius-megakit-spike/wrapper-bundles/` (input.json + manifest.json + tscn), all green against `validate_wrapper_scenes.gd` and `scene_slop_detector.py`. That is the natural starting point for the calibration adapter, not new ground.

## 2. Inventory of existing tools (what is reusable as-is)

All of these are already wired together; the adapter should sit on top, not replace them.

| Tool | Path | Role | Reuse verdict |
| --- | --- | --- | --- |
| `generate_godot_wrapper.py` | `tools/generate_godot_wrapper.py` (477 LOC) | Pure: input.json (asset_semantics) -> tscn + manifest. CLI: `python3 tools/generate_godot_wrapper.py <input.json> <out.tscn> --manifest-out <out.json>`. Categories enforced: structural, gameplay-prop, dressing, character. Anchors must start with `Anchor_`. | Reuse directly via the standard CLI. The adapter will call it; the adapter will not modify it. |
| `semantic_scene_orchestrator.py` | `tools/semantic_scene_orchestrator.py` (475 LOC) | Consumes `zone_doc + motifs.yaml + asset_catalog_dir` and writes a placement plan + per-bundle .tscn. Uses `placement_candidate_scorer` + `motif_expander`. | Reuse for the calibration scene composition. The adapter will provide a *calibration zone* (one zone_family/room_type for a single room module) and a *calibration asset catalog* (subset of Quaternius input.json files). |
| `scene_slop_detector.py` | `tools/scene_slop_detector.py` (646 LOC) | Walks `wrapper-bundles/`, audits every bundle's tscn + manifest, returns JSON/YAML. Rules live in `REQUIRED_ANCHORS_BY_CATEGORY`, `RUNTIME_WRAPPER_CATEGORIES`, `build_finding`. | Reuse on the calibration output. Extend nothing in the slop detector itself; the calibration report aggregates its output and adds *kit-level* metrics on top. |
| `placement_candidate_scorer.py` + `motif_expander.py` | `tools/` | Score and expand motifs into per-asset intents. | Reuse unchanged. Calibration is a single-zone, single-motif run. |
| Godot validator | `the-sargasso-of-stars/scripts/placement/validate_wrapper_scenes.gd` (396 LOC) | Validates tscn + manifest + input.json triplet per bundle. Same constants as the slop detector. | Reuse: `godot --headless --script res://scripts/placement/validate_wrapper_scenes.gd -- <calibration-bundles-dir>`. No changes. |
| `batch_asset_audit.py`, `run_procgen_regression.py` | `tools/` | Source-pack audit and end-to-end regression runner. | Reuse for upstream kit ingestion. Not part of the adapter's runtime path. |
| `asset_semantics.schema.json`, `zone_semantics.schema.json` | `schemas/` | Authoritative JSON Schemas with `additionalProperties: false`. | Adapters must not invent fields the schemas reject. Calibration adapter stays inside the existing schema shapes — the only additive change is an *optional* `kit` block on top-level input.json, not nested inside `asset`. |

## 3. Where the jumbled renders are actually coming from

Based on `SPIKE_REPORT.md` plus the manifests in `wrapper-bundles/`, the jumbling is structural, not tool-level:

1. The wrapper scenes are *skeletons* — they have a root Node3D, anchors, a CollisionRoot, and an empty `Visual` child. The real `.gltf` is referenced in `input.json`/`manifest.json` but is not instantiated as a child of `Visual`. A placer that follows `wrapper_scene` will get a marker, not a mesh. (Confirmed in SPIKE_REPORT §Caveats 1.)
2. `Prop_Chest.gltf` and `Prop_Vent_Big.gltf` have **zero** authored `_convcolonly` collision proxies; current manifests still emit `static-body-proxy` with `box` shape, but the box is `Vector3(1,1,1)` — that's almost never correct for a 2.0 x 2.0 chest. (See `build_shape_section` in `generate_godot_wrapper.py`.)
3. Bounds are inherited from raw `.gltf` scene bounds; `Column_Large_Straight` is 10 m tall, the `Decal_*` assets have 0 m Z thickness. Placer/visibility rules do not yet downscale tall columns or know that 0-thickness decals are wall-only.
4. `scene_slop_detector.py` audits *scene structure*, not *visual legibility*. Its rule list is rules: roots, anchors, collision. It will not catch "the box is 1x1x1 instead of 1.9x2.0x2.0".

So the calibration adapter's job is to make 1–4 visible and fixable in one run, per kit, without inventing a parallel pipeline.

## 4. Proposed design: a *kit adapter*, not a new tool

### 4.1 New data files (the smallest possible surface)

Add two files under `off-the-rails-ai-infra/data/kits/` — this directory does not exist yet, so the change is purely additive. Total new code: zero in the existing tool layer; a new ~200 LOC driver lives next to the data.

A. `data/kits/quaternius-megakit.yaml` — kit metadata (this is the only new author-facing file)

    schema_version: 1
    schema_kind: kit_adapter
    kit:
      id: quaternius-megakit
      display_name: "Quaternius Modular SciFi MegaKit"
      license: "CC0 1.0 Universal"
      source_pack: "/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]"
      flat_staging: artifacts/quaternius-megakit-spike/selected-source-flat
      wrapper_bundles: artifacts/quaternius-megakit-spike/wrapper-bundles
    calibration_room:
      zone_family: hub
      zone_kind: room
      room_type: maintenance-bay   # one of the existing schema-valid room types
      atiss_room_type: maintenance
      bounds: { local_min_m: [-6, -6, 0], local_max_m: [6, 6, 4] }
      style_tags: [ship, derelict, sci-fi, industrial]
      conditioning: { atiss_room_type: maintenance, ... }
      allowed_categories: [structural, gameplay-prop, dressing]
    calibration_asset_subset:
      include_globs:
        - "01-ast-qtr-wallband-straight"
        - "01-ast-qtr-wallband-corner-square-outer"
        - "01-ast-qtr-wallband-corner-square-inner"
        - "07-ast-qtr-platform-simple"
        - "11-ast-qtr-door-frame-square"
        - "15-ast-qtr-prop-computer"
        - "21-ast-qtr-decal-0"
      exclude_categories: [character]
    calibration_motif: maintenance-corridor-pressurized   # one of motifs.sample.yaml ids
    collision_overrides:        # kit-specific, not schema additions
      - asset_id: 16-ast-qtr-prop-chest
        proxy_shape: box
        size_m: [1.9, 2.0, 2.0]
      - asset_id: 20-ast-qtr-prop-vent-big
        proxy_shape: box
        size_m: [2.0, 1.0, 0.1]
      - asset_id: 14-ast-qtr-column-large-straight
        visibility_scale: 0.4   # only used in calibration report, not the scene
    visual_instantiation:
      strategy: "instance_gltf_under_visual"
      gltf_root_relative_to_visual: "."   # wrapper scene resolves gltf via res://

B. `data/kits/quaternius-megakit.collision_overrides.json` — *optional* split-out for ops editing; the driver accepts both layouts (kit YAML with embedded overrides, or a sibling JSON). Either way the schema is the same.

### 4.2 New driver (single file, ~200 LOC)

`tools/kit_calibration.py` — orchestrates one calibration run. It:

1. Loads the kit adapter YAML.
2. Writes a *transient* zone document: `data/kits/_runtime/quaternius-megakit.zone.yaml` — exactly the `zone_semantics.schema.json` shape, no schema edits.
3. Writes a *transient* asset catalog directory: `data/kits/_runtime/quaternius-megakit-assets/` — copies the selected `*.input.json` files from `wrapper-bundles/` (no edits to them, the schema forbids structural changes). Applies `collision_overrides` in-memory by mutating the `collision_policy.proxy_shape` and (when supported) `size_m` on a *copy* of the dict at runtime; the original `input.json` files on disk are not touched.
4. Calls the existing `generate_godot_wrapper.py` only for assets where overrides changed the collision policy — and only writes a *new* tscn into `data/kits/_runtime/quaternius-megakit-bundles/<id>.tscn` plus a *new* manifest. The originals in `wrapper-bundles/` remain untouched. (Read-only respect for the spike's already-green output.)
5. Calls the existing `semantic_scene_orchestrator.py` once with the transient zone + the sample motif library + the transient asset catalog, writing to `data/kits/_runtime/quaternius-megakit-plan/` and `--plan-out data/kits/_runtime/quaternius-megakit.plan.json`.
6. Calls the existing `scene_slop_detector.py` on the orchestrator's output dir; saves the JSON.
7. Calls the Godot validator on the same output dir; captures the result.
8. Writes `artifacts/kits/quaternius-megakit.calibration.json` containing:
   - `meta`: kit id, schema versions, seed, timestamps, tool versions.
   - `placement_plan`: path to the orchestrator's plan.json (reused, not duplicated).
   - `bundles`: list of `{input, tscn, manifest, role}` paths. `role` is one of: `original` (untouched from `wrapper-bundles/`) or `override-applied` (newly generated with kit overrides).
   - `slop_audit`: the slop-detector's own meta + findings count, plus a *kit-level* rollup: `assets_with_findings`, `max_severity`, `anchors_missing`, `collision_warnings`.
   - `godot_validation`: stdout/stderr summary line + non-zero exit if any.
   - `calibration_metrics`: derived from the plan + slop audit. Concrete metrics to compute:
     - `placements_per_zone` (should be 1 for a calibration scene)
     - `category_breakdown` (counts by category actually placed)
     - `tall_assets`: list of placed assets with `max_corner.z > 5` and their visibility scale recommendation
     - `zero_thickness_dressing`: list of dressing assets with `local_max_m.z - local_min_m.z < 0.05`
     - `missing_collision_proxy`: list of `gameplay-prop` placements whose input manifest has `collision_proxy_objects == 0` and which were *not* in `collision_overrides` (i.e., a real gap)
     - `untouched_from_kit_assets`: how many of the kit's `include_globs` matched an existing bundle (target: 100%)
   - `next_actions`: a deterministic string list, e.g.
     - "Add 16-ast-qtr-prop-chest to collision_overrides.proxy_shape box size_m [1.9,2.0,2.0]"
     - "Mark 21-ast-qtr-decal-0 as wall-only via supported_surfaces=['wall'] in input.json"
     - "Run with seed 14, 15 to confirm determinism"
9. Optionally writes `artifacts/kits/quaternius-megakit.preview.tscn` — a *re-execution* of the orchestrator's plan, with one extra pass that copies the wrapper tscn content under the Visual node, replacing the empty placeholder. This is the calibration render target. (For the *first* pass, the driver should print a clear "skipped: visual_instantiation not yet implemented" line and exit 0. The MVP is the report, not the rendered .tscn.)

### 4.3 What the driver does NOT do

- Does not modify `generate_godot_wrapper.py`, `semantic_scene_orchestrator.py`, `scene_slop_detector.py`, `placement_candidate_scorer.py`, `motif_expander.py`, the GDScript validator, or any schema file.
- Does not modify existing `wrapper-bundles/*` input.json/manifest.json/tscn files. The calibration produces a *new* directory tree under `data/kits/_runtime/` and `artifacts/kits/`.
- Does not invent a new schema field. The kit YAML is a *driver config*, not a schema-validated doc.
- Does not auto-fix any visual jumbling in the spike output. It surfaces the issues; humans curate the kit adapter YAML.

### 4.4 Where the calibration scene lives in the Godot project

`the-sargasso-of-stars/scenes/validation/` already has `locked_iso_readability_harness.tscn` and `m7_web_breached_encounter_proof.tscn` as a pattern. The kit adapter emits, on `--godot-output` flag:

    scenes/validation/quaternius-megakit-calibration.tscn
    scripts/validation/quaternius_megakit_calibration.gd
    artifacts/validation-previews/quaternius-megakit-calibration.png
    artifacts/validation-previews/quaternius-megakit-calibration.wav

The .gd is a SceneTree harness modeled on `m7_web_breached_encounter_proof.gd`: set up the camera, instance the orchestrator's bundles, capture a frame. No new validation logic.

## 5. File/data change manifest (smallest possible)

New files only:

  off-the-rails-ai-infra/data/kits/quaternius-megakit.yaml
  off-the-rails-ai-infra/data/kits/README.md                 (one paragraph: how to add a kit)
  off-the-rails-ai-infra/tools/kit_calibration.py            (~200 LOC driver)
  off-the-rails-ai-infra/tests/test_kit_calibration.py       (one test: round-trip one kit YAML to calibration.json with stub gltf root)
  off-the-rails-ai-infra/scripts/validate_kit_adapter.py     (~80 LOC: checks required fields, zone schema, room_type in motif host_room_types, asset ids exist in wrapper-bundles/)

Optional, if user wants the rendered .tscn in this pass:

  the-sargasso-of-stars/scenes/validation/quaternius-megakit-calibration.tscn
  the-sargasso-of-stars/scripts/validation/quaternius_megakit_calibration.gd

Files NOT touched:

  tools/generate_godot_wrapper.py
  tools/semantic_scene_orchestrator.py
  tools/scene_slop_detector.py
  tools/placement_candidate_scorer.py
  tools/motif_expander.py
  scripts/validate_*.py
  tests/test_*.py
  schemas/*.json
  the-sargasso-of-stars/scripts/placement/validate_wrapper_scenes.gd
  artifacts/quaternius-megakit-spike/**    (read-only seed for the calibration run)

## 6. Verification commands

These are the exact commands to run after the driver lands. They are all *existing* commands plus the new driver. None require schema or tool changes.

A. Static: kit YAML validates, all referenced asset ids exist in `wrapper-bundles/`, room_type is one the motif library actually covers:

    python3 scripts/validate_kit_adapter.py \
      data/kits/quaternius-megakit.yaml \
      --bundles artifacts/quaternius-megakit-spike/wrapper-bundles \
      --motifs data/procgen/motifs.sample.yaml

B. Dry-run: produce the calibration report only, no Godot output:

    python3 tools/kit_calibration.py \
      data/kits/quaternius-megakit.yaml \
      --out artifacts/kits/quaternius-megakit.calibration.json \
      --no-godot-output

C. End-to-end: same, but also write a calibration .tscn + .gd to the Godot project, and capture the preview:

    python3 tools/kit_calibration.py \
      data/kits/quaternius-megakit.yaml \
      --out artifacts/kits/quaternius-megakit.calibration.json \
      --godot-project /Users/christopherwilloughby/the-sargasso-of-stars \
      --godot-output /Users/christopherwilloughby/.local/bin/godot-4.6.2

D. Godot-side validation of the calibration bundles (the canonical gate the rest of the stack already trusts):

    /Users/christopherwilloughby/.local/bin/godot-4.6.2 --headless \
      --path /Users/christopherwilloughby/the-sargasso-of-stars \
      --script res://scripts/placement/validate_wrapper_scenes.gd -- \
      /Users/christopherwilloughby/off-the-rails-ai-infra/data/kits/_runtime/quaternius-megakit-bundles

Expected: `Validated N wrapper scene bundle(s).` with N == number of override-applied assets.

E. Slop detector on the same dir (the structural / collision / anchor audit, which the report aggregates):

    python3 tools/scene_slop_detector.py \
      data/kits/_runtime/quaternius-megakit-bundles \
      --report-out artifacts/kits/quaternius-megakit.slop.json \
      --format json

Expected: `Audited N scene bundle(s); findings=K (high=H, medium=M, low=L)`. The calibration report's `slop_audit` block must echo these numbers exactly; mismatches = bug in the driver.

F. Regression: prove the existing spike bundles are still green (the adapter must not have touched them):

    /Users/christopherwilloughby/.local/bin/godot-4.6.2 --headless \
      --path /Users/christopherwilloughby/the-sargasso-of-stars \
      --script res://scripts/placement/validate_wrapper_scenes.gd -- \
      /Users/christopherwilloughby/off-the-rails-ai-infra/artifacts/quaternius-megakit-spike/wrapper-bundles

    python3 tools/scene_slop_detector.py \
      artifacts/quaternius-megakit-spike/wrapper-bundles \
      --report-out /tmp/spike-slop-after.json --format json

    diff /Users/christopherwilloughby/off-the-rails-ai-infra/artifacts/quaternius-megakit-spike/selected-slop-report.json /tmp/spike-slop-after.json

Expected: zero diff on the slop report. Bundles file mtimes unchanged (`find wrapper-bundles -name '*.tscn' -newer kit_calibration.py` returns empty).

G. Determinism: re-run the calibration with the same seed; the plan.json must be byte-identical:

    python3 tools/kit_calibration.py data/kits/quaternius-megakit.yaml --out /tmp/cal-1.json --no-godot-output
    python3 tools/kit_calibration.py data/kits/quaternius-megakit.yaml --out /tmp/cal-2.json --no-godot-output
    diff <(jq -S '.placement_plan' /tmp/cal-1.json) <(jq -S '.placement_plan' /tmp/cal-2.json)

Expected: empty diff. (If not, the orchestrator is already non-deterministic; that's a separate, pre-existing issue, but the calibration report should still note the seed and the run id so humans can spot it.)

H. Unit test:

    python3 -m pytest tests/test_kit_calibration.py -q

Expected: 1 test that loads a stubbed kit YAML pointing at the 23 spike bundles, runs the driver with `--no-godot-output`, asserts: report file exists, `meta.kit_id == "quaternius-megakit"`, every entry in `bundles` has both `original` and `override-applied` roles covered, and `calibration_metrics.missing_collision_proxy` is exactly `["16-ast-qtr-prop-chest", "20-ast-qtr-prop-vent-big"]` (because the kit adapter *also* lists them in `collision_overrides`, the report should classify them as `override-applied` and *not* flag them as missing — a second test should remove them from overrides and assert they *are* flagged).

I. Quick visual sanity (after the optional .tscn path lands): open the .tscn in Godot 4.6.2 editor, press F6, eyeball whether the calibration scene shows a recognizable Quaternius maintenance-bay layout. The render is the deliverable; everything before it is machinery.

## 7. Open questions for the user before implementation

1. Is the MVP *the calibration report JSON* (no rendered .tscn), or do you want the rendered calibration .tscn in this pass? The report alone is ~200 LOC and reuses the existing pipeline end-to-end; the .tscn adds another ~150 LOC of Godot harness and depends on the visual-instantiation strategy being acceptable.
2. The kit YAML's `collision_overrides` mutates the `collision_policy` and injects a kit-only `size_m` field that the existing schema does not know about. The driver applies these *in-memory* and re-feeds the *modified* dict back through `generate_godot_wrapper.py`, but `generate_godot_wrapper.py` currently ignores `size_m` (it always emits `Vector3(1,1,1)` for boxes). To honor overrides we either (a) extend `generate_godot_wrapper.py` to read an optional `size_m` from `collision_policy` (one tiny if-block, 6 lines, no schema change), or (b) post-process the generated tscn string in the driver. Option (a) is the right move; flag for approval.
3. Should the kit adapter also cover Motif calibration (e.g., adjust `host_zone_tags` or `style_tags` on a per-kit basis), or stay strictly to the asset/collision/scale layer for this pass? My recommendation: stay out of motif editing. The motif library is curated content; kits consume it. If a kit needs a new motif, that's a separate M2 task.
4. Where do kit adapter outputs live long-term? `data/kits/_runtime/` and `artifacts/kits/` are my proposal, mirroring the existing `data/procgen/` and `artifacts/quaternius-megakit-spike/` layout. Confirm or redirect.
5. The driver will shell out to `generate_godot_wrapper.py`, `semantic_scene_orchestrator.py`, and `scene_slop_detector.py` via `subprocess.run` rather than importing them, to keep coupling at "same as the spike's `run_spike.py`". Confirm or switch to import.

## 8. Bottom line

The smallest viable change is: one new YAML, one new driver, one new validator, one new test, zero modifications to the existing tool layer. The driver is a thin coordinator that reuses `generate_godot_wrapper.py`, `semantic_scene_orchestrator.py`, `scene_slop_detector.py`, the GDScript validator, and the existing `wrapper-bundles/` spike output. Its only new behavior is kit-specific collision overrides and a kit-level rollup report. The one-line code change to `generate_godot_wrapper.py` (to honor an optional `size_m` in `collision_policy`) is the only "edit" the rest of the stack would need, and it is optional — without it, the calibration report still works, it just reports `collision_warnings: 2` for the prop_chest and prop_vent_big boxes being 1x1x1.
