# Quaternius Modular SciFi MegaKit — Spike Subset Recommendation

Audit target: `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/`
Project: Off The Rails (locked-isometric 3D Godot, hub ship + web-tracked derelicts, space horror)
Audit goal: pick a *minimal representative subset* for the first procgen ingestion spike that exercises the full structural / door / prop / dressing surface area without dragging the whole ~544-asset pack into the pipeline on day one.

---

## Pack Audit Summary

- License: CC0 1.0 (Public Domain). No attribution gate. Safe to ingest and redistribute inside the build.
- Format: glTF 2.0 with separate `.gltf` + `.bin` siblings; textures live in the same root directory (`T_*.png`).
- Authoring target: Godot 4.3+ (confirmed by License notes and KHR_materials_specular / KHR_materials_ior extensions on a few meshes).
- Axis / scale: 1 unit = 1 metre (Blender export default). Confirmed by reading the glTF — node names are clean, single root, no extra transforms.
- Inventory: 544 asset files across 5 categories:
  - Walls: 234 (largest — most variation)
  - Platforms + Doors: 106
  - Props: 86
  - Decals: 78
  - Columns: 30
  - Aliens: 10
  - Textures (root): 24 PBR sets
- Naming convention: every file follows `<Category>_<Variant>_<Topology>.gltf`. A few carry suffixes like `_Broken`, `_Divided`, `_Window`, `_Blocked`, `_TallBand`, `_Transition` — these are *topology hints* the procgen layer should parse (broken walls for derelicts, blocked doors for sealed corridors, windows for hub ship interior/exterior reads).

## Why a Spike Subset, Not the Whole Pack

A first ingestion spike only needs to prove the pipeline can:
1. Walk a glTF, resolve its external `.bin` and texture refs, hand it to Godot's GLTFDocumentImporter.
2. Extract the structural grid (wall snap points, platform footprint, door frame clearances).
3. Build a prop placement graph (where chairs, vents, pods, computers can sit on platforms).
4. Render a dressing pass (decals, lights, broken plates).
5. Round-trip a single room end-to-end.

Loading all 544 files into that pass on day one just hides bugs in the noise. Aim for ~5–6 per category, *biased toward the topology variants procgen will actually branch on*.

---

## Recommended Spike Subset

All paths are absolute, rooted at the glTF (Godot) folder. Godot resolves textures by relative path, so the subset will pull in `T_Trim_01_*`, `T_Trim_02_*`, `T_Trim_03_*`, `T_Decals.png`, and `T_PaddedWall_*` as a side effect — that is *desired*, it confirms texture reference resolution.

### 1. Structural Walls — 6 files

These cover the four wall families procgen needs to branch on: straight, inner corner, outer corner, plus the "broken derelict" state and the "window" variant for the hub ship.

- `Walls/WallBand_Straight.gltf` — **baseline straight wall** (band trim, simple topology)
- `Walls/WallBand_Corner_Square_Outer.gltf` — **outer corner** (room convex)
- `Walls/WallBand_Corner_Square_Inner.gltf` — **inner corner** (room concave)
- `Walls/WallBand_Straight_Broken.gltf` — **broken state** (essential for derelict dressing — derelicts *must* look damaged)
- `Walls/TopWindow_Straight.gltf` — **top window cap** (hub ship: lets corridor light into adjacent room; cheap signal that lighting / occlusion will need to handle transparent sections)
- `Walls/WallPipe_Straight.gltf` — **pipes-on-wall** (heaviest shader/material variant in the pack — proves the importer handles higher vertex counts and complex UVs)

Pipeline value: every wall in the 234-file folder is some combination of these six topology tokens. If the importer, snap-point extractor, and broken-state RNG all work on these six, scaling to the full set is a config change, not a code change.

### 2. Platforms + Doors — 6 files

Lock the floor vocabulary, plus the two door topology variants procgen will branch on.

- `Platforms/Platform_Simple.gltf` — **baseline 1×1 floor tile** (smallest, simplest; use it for grid-resolution smoke test)
- `Platforms/Platform_Simple_Curve.gltf` — **curved tile** (tests non-axis-aligned normals — critical for iso camera)
- `Platforms/Platform_Ramp_2.gltf` — **ramp** (height-change voxel — tests if the procgen heightmap can express Y deltas)
- `Platforms/Platform_Window_Thin.gltf` — **glass floor / window** (transparent surface — tests render-order assumptions)
- `Platforms/Door_Frame_Square.gltf` — **standard door frame** (the "open corridor" topology)
- `Platforms/Door_Frame_Square_Blocked.gltf` — **sealed door frame** (the "this room is locked" topology)

Pipeline value: the simple tile establishes the unit grid, the curve breaks the assumption that all surfaces are axis-aligned (matters for the locked-iso camera raycasts), the ramp proves height changes, the window proves transparency handling, and the two door frames are the binary state the derelict vs hub-ship branching hinges on.

### 3. Columns — 2 files

- `Columns/Column_BentSquare.gltf` — **angled column** (corner / transition piece)
- `Columns/Column_Large_Straight.gltf` — **large load-bearing column** (hub ship cargo bays / derelict structural supports)

Pipeline value: columns are the only "vertical filler" between rooms. We don't need all 30; we need one straight, one corner, and we'll learn whether the column snap system should be a separate grid layer from the wall snap system.

### 4. Props (Gameplay) — 6 files

Pick the props that exercise *different interaction shapes* — interactive surface, container, hazard, navigation blocker, salvage point, fast-travel node.

- `Props/Prop_Computer.gltf` — **interactive terminal** (hub ship — quest / shop / lore interaction anchor)
- `Props/Prop_Chest.gltf` — **loot container** (loot roll surface; the test for "can procgen put an item-bearing prop on a platform and orient it to face the camera iso-angle")
- `Props/Prop_Barrel_Large.gltf` — **explosive / hazard** (web-trap lore; test for "can a prop be marked as a damage source")
- `Props/Prop_Pod.gltf` — **salvage / crew object** (crew recovery loop, salvage loop)
- `Props/Prop_Teleporter.gltf` — **fast-travel node** (the actual room-to-room hub mechanic — must be placeable, must be orientable)
- `Props/Prop_Vent_Big.gltf` — **navigation / sight blocker** (web-trap derelicts need crawlspaces and sight blockers for stealth horror — vents are the canonical one)

Pipeline value: this set covers the six interaction archetypes the room-design grammar needs (terminal, container, hazard, salvage, transit, blocker). If the prop placement layer can place and orient all six, every other prop in the 86-file folder is a swap-in.

### 5. Dressing — 3 files

Decals + lights are the *cheap visual variety* layer. Don't over-test this on the spike.

- `Decals/Decal_0.gltf` — **first decal** (any one of the 39 — pick this one to confirm the decal shader is wired)
- `Decals/Decal_5.gltf` — **middle decal** (proves there is mid-pack variation, not just a single decal mesh reused 39 times)
- `Props/Prop_Light_Corner.gltf` — **corner light fixture** (lighting placement + emissive material — the hub ship needs moody lights, derelicts need flickering ones)

Pipeline value: decals confirm the projection-decal shader path works; the corner light is the only lighting prop in the pack, so it has to be in the spike even though it's technically a prop.

### 6. Aliens — 1 file (deferred to spike-2)

- `Aliens/Alien_Cyclop.gltf`

**Recommendation: skip the Aliens folder for the first spike.** Reasons:
1. Aliens are a *behavior* concern (state machine, navmesh, anim tree) more than a *placement* concern. The procgen ingestion layer shouldn't care about them yet.
2. Including them on spike-1 risks the team optimising the pipeline for rigged skinned meshes when the structural ingestion is what needs validation.
3. Revisit on spike-2 once the structural + prop layer is round-tripping a room cleanly.

If spike-1 needs *one* alien to prove the rigged-mesh path, use `Aliens/Alien_Cyclop.gltf` (smallest file, no `_Large` suffix, single creature). But the default is *skip*.

---

## Subset Summary

| Category | Files | Why this count |
|---|---|---|
| Walls | 6 | one per topology token (straight / inner-corner / outer-corner / broken / window / heavy-material) |
| Platforms + Doors | 6 | baseline / curve / height-change / transparent / open-door / sealed-door |
| Columns | 2 | straight + corner |
| Props | 6 | one per interaction archetype |
| Dressing | 3 | 2 decals + 1 light fixture |
| Aliens | 0 (defer) | behavior concern, not ingestion concern |
| **Total** | **23 .gltf files** | covers the full ingestion surface in < 5% of the pack |

**Side-effect texture coverage** (loaded automatically because glTF refs them): `T_Trim_01_*.png`, `T_Trim_02_*.png`, `T_Trim_03_*.png`, `T_PaddedWall_*.png`, `T_Decals.png` — five PBR sets out of 24 in the pack. Spike-2 should add the remaining 19 if the texture streaming layer is being validated.

---

## Ingestion Spike Acceptance Criteria (suggested)

For the spike to be considered "passed" using this subset:
1. All 23 .gltf files import in Godot 4.3+ with zero warnings about missing textures or unsupported extensions.
2. Wall snap-point extractor produces a consistent grid from the three wall topologies.
3. At least one room can be assembled (4 walls + 1 floor + 1 door) and survive a save/load round-trip in a `.tscn`.
4. Each of the 6 prop archetypes can be placed on a platform, oriented toward the iso camera, and queried for its interaction type.
5. The two door topology variants (open / blocked) are distinguishable at the data layer (a tag, a property — procgen will branch on this).
6. The broken wall variant is visually distinguishable from the intact one (proves the RNG state for "derelict vs hub" can produce meaningful dressing).

When those 6 are green, the pipeline is ready to ingest the remaining ~520 files in batches by category.

---

## Files Referenced in This Report

Absolute paths (read-only, no modifications made):

- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Walls/WallBand_Straight.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Walls/WallBand_Corner_Square_Outer.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Walls/WallBand_Corner_Square_Inner.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Walls/WallBand_Straight_Broken.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Walls/TopWindow_Straight.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Walls/WallPipe_Straight.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Platforms/Platform_Simple.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Platforms/Platform_Simple_Curve.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Platforms/Platform_Ramp_2.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Platforms/Platform_Window_Thin.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Platforms/Door_Frame_Square.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Platforms/Door_Frame_Square_Blocked.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Columns/Column_BentSquare.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Columns/Column_Large_Straight.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Props/Prop_Computer.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Props/Prop_Chest.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Props/Prop_Barrel_Large.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Props/Prop_Pod.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Props/Prop_Teleporter.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Props/Prop_Vent_Big.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Decals/Decal_0.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Decals/Decal_5.gltf`
- `/Users/christopherwilloughby/Downloads/Modular SciFi MegaKit[Source]/glTF (Godot)/Props/Prop_Light_Corner.gltf`

No files in the MegaKit or in `/Users/christopherwilloughby/off-the-rails-ai-infra` were modified.
