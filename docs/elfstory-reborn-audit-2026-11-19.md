# ElfStory Reborn — Technical, Design & 3D Asset Pipeline Audit (2026-11-19)

**Base commit audited:** `main` @ `ca2943d` (124/124 unit tests passing, `tsc --noEmit` and `vite build` clean)  
**Follow-up shipped in this session:** **V2 Studio-Grade Hybrid 3D Pipeline** (`tools/blender_retopo_bake.py`, `tools/blender_arch_kit.py`, `tools/comfy_runner.py`, `PerspectiveCanvas.tsx` `RoomEnvironment` IBL, `.agents/skills/oakhaven-asset-pipeline/SKILL.md`, and `public/models/oakhaven_platypus.glb` V2 acceptance verification).

---

## 1. Core Gameplay & Town-Loop Design (`PROJECT_TRUTH.md` vs. Reality)

### Phase 0 Pass/Fail Bar Verdict
> *"my town changed my next run, and I chose my zone because of my town."* — `PROJECT_TRUTH.md`

* **"My town changed my next run" — PARTIAL PASS (50%):** Completing Torin's Blacksmith unlocks `merchantId: 'blacksmith'` (`src/game/npcs.ts:1662-1679`), stocking the town-gated `Oakhaven-Forged Blade` (a Depth-3 Rare weapon available right after Floor 1) plus a one-time `+75g` wish payout (`src/game/town.ts:82`). Buying that blade meaningfully alters Floor 2–3 TTK. However, **Population Tier** does nothing in gameplay.
* **"I chose my zone because of my town" — HARD FAIL (0%):** Every single town item in the game (`blacksmith_anvil`, `blacksmith_bellows`, `blacksmith_sign`, and `heartcoal`) is unconditionally spawned together on `dungeon_1` (`src/game/world.ts:1406-1447`). Zero town pieces, materials, villagers, or stall cures exist in `overworld`, `barrow_1..2`, `mire_1..2`, `frost_1..3`, or `dungeon_2..5`. Because all 4 items are guaranteed on the mandatory starting floor, the town never drives zone selection.

### Concrete Design & Implementation Failures

1. **`TownPanel.tsx` Recreates the Cut Town-Upgrades Modal (Menu-Bound vs. Physical Loop)**
   * `PROJECT_TRUTH.md:32` explicitly cut `TownUpgradesModal` on Nov 17 because *"town must be the loop, not a menu."*
   * In reality, rebuilding and shopping at the rebuilt Blacksmith live entirely inside `src/components/TownPanel.tsx:29-159`, opened from a bottom-left HUD chip (`src/App.tsx:6159-6181`).
   * **Split Shop Identity:** Walking up to Torin Ironhand at `(7, 2)` in Oakhaven and talking to him opens his legacy `merchantId: 'torin'` shop (`src/game/npcs.ts:187-207`), which sells generic Depth 1–2 Uncommons and **never stocks the `Oakhaven-Forged Blade`**. The rebuilt Blacksmith shop (`merchantId: 'blacksmith'`) cannot be opened by interacting with Torin or with the rebuilt 3D annex on the map — it can **only** be opened by clicking the HUD chip → `TownPanel` → `"Open the Blacksmith Shop"` (`src/App.tsx:1216-1223`).

2. **Schrödinger's Blacksmith & Broken Lot Placements in Oakhaven**
   * **Already Built on Turn 0:** `getOakhavenModelPlacements()` (`src/game/modelLoader.ts:79-93`) mounts `landmark:torin_forge` (`/models/blacksmith.glb`, `targetWidthTiles: 4.4`) at `(7.0, 2.0)` with a warm glowing forge `PointLight` (`0xff8c2b`, `intensity: 3.0`) on Turn 0 — while the HUD chip says `REBUILD THE BLACKSMITH · Pieces 0/3 · Forge stalled` and `TownPanel.tsx:76` says *"STALLED: Torin's forge is cold without it."*
   * **Duplicate Building on Completion:** Completing the Blacksmith spawns a **second**, smaller copy (`targetWidthTiles: 2.8`) of the exact same `/models/blacksmith.glb` at `OAKHAVEN_LOT_ANCHORS[lot]` (`src/game/modelLoader.ts:156-172`), leaving Oakhaven with two identical smithies while Torin remains inside the first one at `(7, 2)`.
   * **Lot Anchors Clip Through Unhidden Map Geometry & Have Zero Collision:** Unlike `landmark:torin_forge`, the dynamic placement `major:blacksmith:${anchor.lot}` (`src/game/modelLoader.ts:158-171`) omits `hideTerrainFootprint` and never updates `zone.tiles` walkability or `interactId`:
     * **`pondside` (`x: 17.5, z: 13.0`, 2.8 tiles wide → `x: 16.1..18.9, z: 11.6..14.4`):** Overlaps the eastern half of the unhidden 5×3 `#` cottage at `x: 14..18, z: 12..14` (`src/game/world.ts:175-177`), clipping directly into its half-timber walls and door `+` at `(18, 13)`.
     * **`market` (`x: 11.0, z: 5.0`, `x: 9.6..12.4, z: 3.6..6.4`):** Clips directly through the unhidden `Π` market stall at `(11, 4)` and `*` torch at `(12, 6)` (`src/game/world.ts:167-169`). Because `(10..12, 5)` stay `walkable: true`, Pell the Carter (`homeX: 10, homeY: 5`, `src/game/world.ts:192`) and the player walk straight through the solid 3D building.
     * **`west_gate` (`x: 4.5, z: 9.5`, `x: 3.1..5.9, z: 8.1..10.9`):** Encloses the unhidden procedural `♣` tree at `(4, 9)` (`src/game/world.ts:172`); its 3 green dodecahedron foliage balls poke straight out of the smithy roof, and walkable floor tiles `(4, 10)`, `(5, 9)`, `(5, 10)` allow walking through the mesh.

3. **Stall Mechanics Are Inverted Against `PROJECT_TRUTH.md`**
   * `PROJECT_TRUTH.md:29`: *"Town stalls (plague/dispute/cold/stopped mill) freeze GROWTH only, cured by zone-specific discoveries."*
   * In `src/game/town.ts:182-188`, `canCompleteBlacksmith` requires `state.stallItemFound` (`Heartcoal`), making the stall block **major completion** (acting as a 4th piece on Floor 1).
   * Conversely, `townReturnTick` (`src/game/town.ts:229-235`) **never checks `state.stallItemFound`** — population growth ticks `+2` even while the stall is active.

4. **Population Tier Is 100% Cosmetic & Farmable via Turn-0 Stair-Hopping**
   * `populationTier(town)` (`src/game/town.ts:238-240`) is only called in `src/components/TownPanel.tsx:33` to display `Town Tier ${tier}` in text. It is never read by `computeDerivedStats` (`src/game/skills.ts`), never unlocks districts/lots, and never spawns visible ActRaiser growth (houses, roads, fields, or villagers) in Oakhaven.
   * **Infinite Population Exploit:** The return watcher in `src/App.tsx:1174-1188` fires whenever `currentZoneId === 'oakhaven'` and `prevZone.depth > 0`. Stepping onto `>>` in Oakhaven `(25, 8)` to enter `dungeon_1` and immediately stepping back up `<` (`stairs_up`) ticks `+2` population per round-trip with zero combat or floor progress. Dying and clicking "Wake in Oakhaven" (`src/App.tsx:4799-4821`) also ticks `+2` population.

5. **Unbanked Loot Rule Violated & Permanent Quest-Item Pack Clutter**
   * `PROJECT_TRUTH.md:13`: *"Return alive to bank loot, or die and Wake in Oakhaven losing only unbanked carried loot."*
   * In `src/App.tsx:4402-4410`, `recordTownItem` is called **on dungeon floor pickup** rather than on alive return to Oakhaven — if you pick up all 4 items on Floor 1 and die, they are already permanently banked in `TownState`.
   * Simultaneously, those 4 items are pushed into `player.inventory` (`src/App.tsx:4399`) and **never removed** when `handleCompleteBlacksmith` runs (`src/App.tsx:1190-1214`), leaving 4 dead quest items in the player's Pack forever.

---

## 2. Graphical Overhaul, Lighting & Visual Cohesion

### Four-Way Art Style Clash in Oakhaven

| Layer | Implementation | Shading & Material | Visual Mismatch |
| :--- | :--- | :--- | :--- |
| **1. Hand-Built Low-Poly** (`blacksmith.glb`) | 14 untextured primitives, `4,456` tris (`public/models/blacksmith.glb`) | `MeshStandardMaterial`, solid `baseColorFactor`, flat normals, `metallic: 0.82` on anvil | Clean, bright, crisp facets (`Tunic` aesthetic) — needed an IBL `envMap` so metallic surfaces don't reflect black void (fixed Nov 19 via `RoomEnvironment` IBL). |
| **2. V1 TRELLIS.2 PBR Meshes** (`mira_apothecary.glb`, `oakhaven_oak.glb`) | 1 mesh, `17.5K–39.6K` tris, 512px WebP `baseColorTexture` + `metallicRoughnessTexture` | `MeshStandardMaterial` with baked `xatlas` textures | Darker, muddier albedo because TRELLIS.2 bakes self-shadowing/AO into `baseColorTexture`, compounded by micro-faceted flat shading across 40K triangles. |
| **3. Procedural 2D Canvas Terrain** (`src/game/sceneMaterials.ts:208-214`) | 64×64 HTML5 Canvas textures per tile (`NearestFilter`, `generateMipmaps: false`, brightened `×2.2`) | `MeshLambertMaterial` | Aliasing/shimmering pixel-art texels on a harsh 1×1 grid right beneath smooth 3D `.glb` stone plinths. |
| **4. Legacy Box/Lollipop Primitives** (`src/game/sceneBuilder.ts:406-439`, `src/game/entityModels3d.ts`) | Untextured boxes, cylinders, and 3-dodecahedron trees | `MeshLambertMaterial` | 5 trees in Oakhaven are sculpted `.glb` oaks (`src/game/modelLoader.ts:110-154`), while the other **~85 border/interior `♣` tiles** remain 3-ball primitive lollipop trees (`treeFoliageA/B`). Vex's hut `(3..5, 12..14)` and the SE cottage `(14..18, 12..14)` remain flat-topped extruded `#` boxes. |

### Specific Rendering & Visual Bugs

1. **Missing `PMREMGenerator` / `RoomEnvironment` IBL *(FIXED Nov 19 in `PerspectiveCanvas.tsx`)*:**
   * Previously `scene.environment` was `null`, causing `GLTFLoader`'s `MeshStandardMaterial` meshes to receive zero indirect specular/diffuse IBL. Fixed by wiring `THREE.PMREMGenerator` + `RoomEnvironment` (`environmentIntensity = 0.55` outdoors, `0.32` in dungeons).
2. **`syncTileNode` Undoes `hideTerrainFootprint` on Camera Rotation / Movement:**
   * When a `.glb` finishes loading in `PerspectiveCanvas.tsx:795-811`, it hides `wallMesh`, `wallCapMesh`, `treeCrownGroup`, and `structureMeshes`.
   * However, `TileNode` (`src/game/sceneBuilder.ts:229-258`) stores no `hiddenByModel` flag. On every frame, `syncTileNode()` (`src/game/sceneBuilder.ts:1104-1120`) resets `node.wallCapMesh.visible = !isCutaway` and `node.treeCrownGroup.visible = !isCutaway` whenever `stateChanged || cutawayChanged` triggers, causing floating terracotta roof slabs and primitive tree crowns to pop back inside 3D `.glb` buildings/trees.
3. **Torin Is Buried Inside a Closed 3D Mesh with No Cutaway:**
   * In `src/game/world.ts:188`, Torin spawns at `(7, 2)` — the interior center of the `#` hut `(5..9, 1..3)`. Because `landmark:torin_forge` (`/models/blacksmith.glb` at `(7, 2)`) is a closed solid exterior 3D building with no roof-cutaway support, Torin is completely entombed inside the 3D mesh; only his 2D HUD nameplate is visible.

---

## 3. 3D Model Generation & Asset Pipeline Efficiency (V1 Diagnosis & V2 Solution)

### Binary Telemetry of Shipped `.glb` Assets (`public/models/`)

| Asset | Pipeline | File Size | Geometry Buffer | Image Buffer | Topology (Faces / Tris) | Exported Vertices | Verts : Tris |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `blacksmith.glb` | Hand-built Blender | **334.3 KB** | 320.8 KB (96.0%) | 0.0 KB (0%) | 2,491 polys / **4,456 tris** | 9,438 | 2.12 : 1 |
| `blacksmith_trellis.glb` | V1 (`blender_polish_asset.py`) | **4,954.7 KB** | **4,819.0 KB (97.3%)** | 134.0 KB (2.7%) | **45,693 tris** | **137,073** | **3.00 : 1** |
| `mira_apothecary.glb` | V1 (`blender_polish_asset.py`) | **4,364.0 KB** | **4,177.8 KB (95.7%)** | 184.5 KB (4.2%) | **39,615 tris** | **118,834** | **3.00 : 1** |
| `oakhaven_oak.glb` | V1 (`blender_polish_asset.py`) | **1,841.7 KB** | **1,740.6 KB (94.5%)** | 99.5 KB (5.4%) | **17,474 tris** | **52,422** | **3.00 : 1** |
| `oakhaven_platypus.glb` *(Before)* | V1 (`blender_polish_asset.py`) | **577.0 KB** | 476.5 KB | 100.5 KB | 0 quads / **4,915 jagged tris** (1,304 non-manifold) | 14,745 | 3.00 : 1 |
| **`oakhaven_platypus.glb` *(After)*** | **V2 (`blender_retopo_bake.py`)** | **302.8 KB** | **131.7 KB (43.5%)** | **169.4 KB (55.9%)** | **2,359 pure quads** (`4,718` glTF tris, **0 non-manifold**) | **3,330** | **0.71 : 1** |

### Why V1 Bloated Files to 4.3–4.9 MB & Crumpled Organic Silhouettes
1. **`xatlas` UV Seam Lock Defeated `DECIMATE`:** TRELLIS.2 exports ~17,000 fragmented `xatlas` UV islands. Blender's `DECIMATE (COLLAPSE)` refuses to collapse across UV/topological boundary seams once quadric error hits the seam threshold, stalling at `17K–45K` triangles (or collapsing interior faces into jagged origami spikes when forced lower).
2. **`flat_shade=True` Tripled Vertex Count (`verts = 3 × tris`):** In `tools/blender_polish_asset.py:216-217`, setting `p.use_smooth = False` forced the glTF exporter to split every triangle corner into 3 independent vertices (`137,073` vertices for `45,693` triangles = `4.82 MB` of uncompressed float32 buffers!).
3. **512px SDXL Turbo Concepts Baked Shadows & Multi-Object Framing Leaks:** Running `sd_xl_turbo_1.0_fp16.safetensors` at `512×512` (`steps=6, cfg=1.8`) produced cropped multi-object scenes (e.g. `docs/concepts/oakhaven_cottage_comfy.png` with a background tree and grass island) and dark drop shadows that TRELLIS.2 baked directly into the 3D albedo.

### The V2 Hybrid 3D Pipeline (Shipped Nov 19, 2026)
* **Track A — Architecture, Buildings & Hard-Surface Props (`tools/blender_arch_kit.py` + `tools/comfy_runner.py --mode pbr-texture`, No TRELLIS.2):**
  * Modular quad kitbasher in Blender 5.3.0 (`1,500–4,000` clean quads, `< 500 KB` `.glb`, contiguous Smart UVs, smooth shading + `WeightedNormal`, bottom-center `Z = 0`).
  * ComfyUI `--mode pbr-texture` generates 1024×1024 seamless tileable albedo textures (toroidal edge blending + low-frequency illumination delighting) and automatically derives `<slug>_normal.png` (periodic Sobel tangent-space normal map) and `<slug>_roughness.png` (`[0.64, 0.92]`).
* **Track B — Organic Creatures, Trees, Rocks & Statues (`tools/comfy_runner.py --mode concept` → Elf Works `/foundry` → `tools/blender_retopo_bake.py`):**
  * 1024×1024 flat diffuse overcast studio lighting concepts with front-loaded subject prompts and automatic `delight_concept_image()` shadow-trough lift.
  * `tools/blender_retopo_bake.py` keeps `HighPoly` custom normals/UVs untouched, runs a watertight Voxel manifold pre-pass + `10×` scale area guard + `quadriflow_remesh(target_faces=2500)` + `Shrinkwrap` (`NEAREST_SURFACEPOINT`) + Clean Smart UV Project (`66°`, `0.02` margin) + Cycles GPU Selected-to-Active Bakes (1024×1024 `DIFFUSE` Color-only with `Metallic=0`, `NORMAL`, and `AO` softly blended at 22% into delighted albedo) + Smooth Shading + `WeightedNormal` (`Metallic=0.05`, `Roughness=0.72`).

---

## 4. Renderer Performance & Codebase Health

### Renderer Performance (`PerspectiveCanvas.tsx` & `sceneBuilder.ts`)
1. **1,437–1,535+ Draw Calls & the Hidden 6× Wall Multiplier:**
   * `buildZoneTerrain` (`src/game/sceneBuilder.ts:299-1055`) creates one `THREE.Group` per tile (`476` in Oakhaven, `1,472` in dungeons).
   * `SceneMaterialPool.getWallArray` (`src/game/sceneBuilder.ts:177-215`) creates an array of **6 separate `MeshLambertMaterial` instances** for every `wallBox`, causing Three.js to issue **6 draw calls per wall tile in the main pass + 6 in the shadow pass = 12 draw calls per wall tile**.
2. **271,794 Extra `.glb` Triangles Per Frame (Main + 2048px Shadow Pass):**
   * Until `mira_apothecary.glb` (`39,615` tris) and `oakhaven_oak.glb` (`17,474` tris × 5 placements = `87,370` tris) are re-processed through the new V2 pipeline (`tools/blender_arch_kit.py` / `tools/blender_retopo_bake.py`), Oakhaven renders `135,897` `.glb` triangles twice per frame (`main` + `2048×2048 PCFSoftShadowMap` = `271,794` tris/frame at 120 fps).
3. **10–11 Simultaneous Active `PointLight`s & Async Shader Recompilations:**
   * `PerspectiveCanvas.tsx:489-496` allocates `1` `playerLight` + `7` `envLights`, and `instantiatePlacedModel` (`src/game/modelLoader.ts:249-257`) attaches 2–3 additional `THREE.PointLight`s inside `modelsGroup`. Every async `.glb` load mutates `NUM_POINT_LIGHTS` (`8 → 9 → 10 → 11`) and triggers full GLSL shader program recompilation.
4. **Always-On 120 FPS Loop & GPU Memory Leaks:**
   * `requestAnimationFrame(render)` (`PerspectiveCanvas.tsx:1766`) never idles or throttles when full-screen modals cover the canvas.
   * `SceneMaterialPool.dispose()` (`src/game/sceneBuilder.ts:217-226`) disposes `Material` objects without disposing their `CanvasTexture` maps in `canvasTextureCache` (`src/game/sceneMaterials.ts:183`), and FX pool materials (`shadowPool`, `ringPool`, `beamPool`, `sparkPool`, `projPool`) are not disposed on unmount.

### Codebase Health, Save Compatibility & Test Gaps
1. **`src/App.tsx` Monolith:** `6,581` lines and `152` React hooks (`53 useCallback`, `45 useState`, `26 useEffect`, `24 useRef`, `4 useMemo`).
2. **NG+ Wipes Rebuilt Blacksmith Stock:** In `handleAscendNgPlus` (`src/App.tsx:1465`), `setMerchantStocks(freshMerchants)` resets `merchantStocks` via `createInitialMerchants()` (which omits `blacksmith`), while `town.blacksmithComplete` remains `true`.
3. **Unseeded `Math.random()`:** `82` direct calls (`36` in `world.ts`, `25` in `App.tsx`, `7` in `loot.ts`, `7` in `ambient.ts`, `5` in `combat.ts`, `2` in `audio.ts`).
4. **Bundle Size:** Single `1.35 MB` JS bundle (`380 KB` gzip) because `vite.config.ts` defines no code-splitting for `three`.
5. **Test Coverage Gaps:** `tests/modelLoader.test.ts` tests placement array returns but does not test map footprint collisions against `createOakhavenMap()`, `instantiatePlacedModel` pivot/scale math, or `syncTileNode` cutaway persistence.

---

## 5. Overall Evolution Scorecard & Sequenced Milestones

| Pillar | Score (Nov 19 Baseline) | Post-V2 Pipeline Status |
| :--- | :---: | :--- |
| **1. Core Gameplay & Town-Loop Design** | **4.5 / 10** | Next up (Milestone 1 below) |
| **2. Graphical Overhaul, Lighting & Visual Cohesion** | **5.0 / 10** | Upgraded to **6.5 / 10** (`RoomEnvironment` IBL shipped; Oakhaven asset re-bakes & cutaway flag next) |
| **3. 3D Model Generation & Asset Pipeline Efficiency** | **5.5 / 10** | Upgraded to **9.0 / 10** (V2 Track A + Track B shipped & proven on `oakhaven_platypus.glb`: `2,359` pure quads, `302.8 KB`) |
| **4. Renderer Performance & Codebase Health** | **5.5 / 10** | Next up (Milestone 3 below) |
| **Overall Project Score** | **5.1 / 10** | **6.1 / 10** after V2 Pipeline & IBL landing |

### Next 3 Milestones (Ordered by Impact-to-Effort Ratio)
1. **Milestone 1 — Physicalize the Phase 0/1 Town Loop & Close All Town Exploits:**
   * Move Torin to `(7, 4)` outside the 3D forge mesh; add `hideTerrainFootprint` to all 3 `OAKHAVEN_LOT_ANCHORS`; route Torin's shop & in-world lot interaction to `merchantId: 'blacksmith'` when rebuilt; add free lot relocation (`relocateBlacksmith`); bank carried town items only on alive return (dropping unbanked items on death) and remove them from `player.inventory`; block Turn-0 stair-hop and death population ticks; wire `populationTier(town)` into `computeDerivedStats` and visible ActRaiser growth placements; add side-dungeon town supplies (`barrow_1`, `mire_1`, `frost_1`) so the town drives Run-2 zone selection; preserve `merchantStocks.blacksmith` across NG+.
2. **Milestone 2 — Re-Bake Existing Oakhaven Assets with V2 Pipeline & Fix `syncTileNode` Cutaway:**
   * Re-build `mira_apothecary.glb` (`4.36 MB → < 400 KB`) via Track A (`tools/blender_arch_kit.py`) and re-bake `oakhaven_oak.glb` (`1.84 MB → < 350 KB`) via Track B (`tools/blender_retopo_bake.py`).
   * Add `hiddenByModel?: boolean` to `TileNode` in `src/game/sceneBuilder.ts` so `syncTileNode` never pops procedural roofs or tree crowns back inside `.glb` models on camera cutaway.
   * Upgrade Oakhaven ground/road `CanvasTexture` filtering in `src/game/sceneMaterials.ts` from `64×64 NearestFilter` to `256×256 LinearMipmapLinearFilter` (`anisotropy = 4`).
3. **Milestone 3 — Terrain Instancing, Light-Pool Consolidation & `App.tsx` Extraction:**
   * Collapse the 6-material wall box array in `src/game/sceneBuilder.ts` and batch static terrain into `THREE.InstancedMesh` buckets (target: `< 150` draw calls in Oakhaven).
   * Fold `.glb` building lights into `PerspectiveCanvas`'s existing `candidateLights` pool to eliminate mid-run GLSL shader recompiles, and throttle `requestAnimationFrame` when modals cover the map.
   * Split `three` into a vendor chunk in `vite.config.ts`, thread a seeded PRNG (`src/game/rng.ts`) through `world.ts`/`loot.ts`, and extract town/dialogue actions out of `src/App.tsx`.
