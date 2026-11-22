---
name: oakhaven-asset-pipeline
description: >-
  Studio-Grade V2 Hybrid 3D Game Asset Pipeline for ElfStory Reborn: routes
  Architecture & Hard-Surface Props to Track A (Blender MCP Modular Quad Kit +
  ComfyUI 1024px Seamless PBR Textures) and Organic Creatures/Trees/Statues to
  Track B (1024px Delighted Concept Art -> TRELLIS.2 via Elf Works /foundry ->
  Blender QuadriFlow Retopo + Shrinkwrap + Smart UV + Cycles Diffuse/Normal/AO
  Bake + Weighted Normals).
---

# ElfStory Reborn — V2 Studio-Grade Hybrid 3D Asset Pipeline

Use this skill whenever Garth asks to create, generate, polish, and wire a new 3D building, prop, landmark, creature, or character into **ElfStory Reborn** (Oakhaven or dungeons).

## Art Direction & Routing Rule (Locked)
- **Style**: **Crisp Stylized Low-Poly** (Tunic / Link's Awakening / For The King aesthetic — clean quad topology, weighted normals, vibrant balanced colors, warm afternoon sunlight + Three.js `RoomEnvironment` IBL).
- **Routing Rule (NEVER run TRELLIS.2 on straight-edged buildings or hard-surface props)**:
  - **Track A — Architecture, Buildings & Hard-Surface Props** *(Blacksmith, Apothecary, Cottages, West Gate, Stalls, Bridges, Anvils, Chests)* → **Blender MCP Modular Kitbashing** (`tools/blender_arch_kit.py`) + **ComfyUI 1024px Seamless PBR Textures** (`tools/comfy_runner.py --mode pbr-texture`).
  - **Track B — Organic Creatures, Trees, Rocks & Statues** *(Platypus, Oakhaven Oak, Monsters, Statues, Hero Props)* → **1024px Delighted Concept Art** (`tools/comfy_runner.py --mode concept`) → **Observable High-Poly Sculpt via Elf Works `/foundry`** (`http://127.0.0.1:8765/foundry`) → **Blender QuadriFlow Retopo + Shrinkwrap + Smart UV + Cycles Selected-to-Active Bake** (`tools/blender_retopo_bake.py`).
- **32 GB Memory Rule**: Never run ComfyUI, TRELLIS.2, ACE-Step, and Ollama simultaneously. Both `tools/comfy_runner.py` and `~/GLocalLM/ops/foundry.py` automatically arbitrate `AGENT` ↔ `STUDIO` mode, stop ACE-Step, and call ComfyUI `POST /free`.

---

## Track A — Architecture, Buildings & Hard-Surface Props (No TRELLIS.2)

### Step A1 — Generate 1024px Seamless PBR Textures via Local ComfyUI
Generate 1024×1024 seamless tileable stylized albedo + derived tangent-space normal (`<slug>_normal.png`) + roughness (`<slug>_roughness.png`) maps:
```bash
python3 /Users/gerth/Projects/elfstoryreborn/tools/comfy_runner.py \
  --mode pbr-texture \
  --prompt "warm medieval terracotta roof shingles, stylized hand-painted RPG material" \
  --output /Users/gerth/Projects/elfstoryreborn/public/textures/oakhaven_shingle.png \
  --width 1024 --height 1024 --steps 4 --cfg 1.35
```
*(Supported canonical material slugs in `public/textures/`: `oakhaven_cobble`, `oakhaven_plaster`, `oakhaven_timber`, `oakhaven_shingle`.)*

### Step A2 — Build Clean Quad Architecture via Blender MCP (`tools/blender_arch_kit.py`)
- **Option 1 (Live Blender MCP — preferred)**:
  Call `execute_blender_code`:
  ```python
  import sys
  sys.path.insert(0, "/Users/gerth/Projects/elfstoryreborn/tools")
  from blender_arch_kit import build_arch_asset
  build_arch_asset(
      preset="apothecary",  # "blacksmith" | "apothecary" | "cottage" | "market_stall" | "west_gate"
      output_path="/Users/gerth/Projects/elfstoryreborn/public/models/<slug>.glb",
      blend_path="/Users/gerth/Projects/elfstoryreborn/docs/models/<slug>.blend",
      asset_name="Oakhaven_<Slug>",
      texture_dir="/Users/gerth/Projects/elfstoryreborn/public/textures",
      target_width=3.2,
  )
  ```
  Then inspect with Blender MCP `look` (`mode: "angles"`, `shading: "material"`, `views: ["three_quarter", "front", "right", "top"]`).
- **Option 2 (Headless Blender CLI)**:
  ```bash
  /Applications/Blender.app/Contents/MacOS/Blender --background --python \
    /Users/gerth/Projects/elfstoryreborn/tools/blender_arch_kit.py -- \
    --preset apothecary \
    --output /Users/gerth/Projects/elfstoryreborn/public/models/<slug>.glb \
    --blend /Users/gerth/Projects/elfstoryreborn/docs/models/<slug>.blend \
    --texture-dir /Users/gerth/Projects/elfstoryreborn/public/textures \
    --target-width 3.2
  ```
- **Track A Budget**: `1,500–4,000` clean quads, `< 500 KB` `.glb`, bottom-center origin at `Z = 0`, beveled edges + weighted normals.

---

## Track B — Organic Creatures, Trees, Rocks & Statues (TRELLIS.2 → Quad Retopo → Cycles Bake)

### Step B1 — 1024px Unlit / Delighted Concept Art
Generate a single isolated subject on a plain neutral mid-gray background with **flat diffuse overcast studio lighting** (automatic delighting lifts shadow troughs and removes specular glare):
```bash
python3 /Users/gerth/Projects/elfstoryreborn/tools/comfy_runner.py \
  --mode concept \
  --prompt "stylized low-poly fantasy platypus creature with smooth duck bill, warm golden chest, webbed feet, and flat beaver tail" \
  --output /Users/gerth/Projects/elfstoryreborn/docs/concepts/<slug>.png \
  --width 1024 --height 1024 --steps 4 --cfg 1.35
```
Visually inspect `docs/concepts/<slug>.png` with `read_file`, then stage a copy into `<Artifact Directory Path>/browser_sandbox/uploads/<slug>.png` for Foundry upload.

### Step B2 — Observable High-Poly Sculpt via Elf Works `/foundry`
**Queue rule (Nov 22, 2026)**: submit with `python3 tools/genq.py foundry --image <abs cutout> --name <slug>` so the job waits for any other session's generation to finish first; check `python3 tools/genq.py status` before using the `/foundry` page by hand.
**Rule**: Always run TRELLIS.2 through **Elf Works `/foundry`** (`http://127.0.0.1:8765/foundry`) so live progress and **RECENT 3D MODELS** are visible. Never invoke `generate.sh` directly in a hidden terminal.
- **In Antigravity**:
  1. Stage a copy of `docs/concepts/<slug>.png` into `<Artifact Directory Path>/browser_sandbox/uploads/<slug>.png` and use the `browser` subagent (`Model: "flash"`) to open `http://127.0.0.1:8765/foundry`, upload `<slug>.png`, set **Output name** = `<slug>`, **Pipeline** = `512`, **PBR Texture** = `1024` (or `512`), and click **Generate .glb**.
  2. Monitor `~/GLocalLM/workspace/3d/<slug>-*.glb` using `schedule` timers (`120–180s` intervals; never shell `sleep`).
- **In Muse (`com.meta.endo` / `muse` CLI) or Headless API submission**:
  1. Submit directly to the Elf Works `/foundry` HTTP endpoint so the job runs observably on the `/foundry` dashboard (`http://127.0.0.1:8765/foundry` shows live progress and adds the output to **RECENT 3D MODELS**):
     ```bash
     KEY=$(curl -s http://127.0.0.1:8765/foundry | sed -n "s/.*OPS_KEY = '\([^']*\)'.*/\1/p")
     curl -sS -X POST "http://127.0.0.1:8765/api/foundry?k=$KEY" \
       -H "Content-Type: application/json" \
       -d '{"op":"start","image_path":"/Users/gerth/Projects/elfstoryreborn/docs/concepts/<slug>.png","output_name":"<slug>","pipeline_type":"512","texture_size":1024}'
     ```
     *(Or drive the `/foundry` page with Computer Use `/cua` if preferred.)*
  2. Watch `~/GLocalLM/workspace/3d/<slug>-*.glb` (or poll `curl -s http://127.0.0.1:8765/api/foundry`) with Muse's `Monitor` tool until `job.state == "done"` and the `.glb` finishes writing.

### Step B3 — QuadriFlow Retopo + Shrinkwrap + Smart UV + Cycles Bake (`tools/blender_retopo_bake.py`)
- **Guardrail (TRELLIS `xatlas` UV rule)**: Never run raw 2D ComfyUI `img2img` (`--mode refine-texture`) directly on a TRELLIS.2 `xatlas` UV atlas — its ~17K fragmented UV islands bleed across seams. Always retopologize and bake onto clean Smart UV islands via `tools/blender_retopo_bake.py` (or adjust lighting/materials in Three.js / Blender).
- Never use naive `DECIMATE` + `flat_shade=True` on raw TRELLIS meshes. Run `tools/blender_retopo_bake.py` to retopologize into `~2,500` clean quads, shrinkwrap the silhouette, unwrap ~15–30 clean Smart UV islands, bake 1024×1024 Diffuse (Color-only) + Tangent-Space Normal Map + Ambient Occlusion in Cycles, and apply Smooth Shading + Weighted Normals:

- **Option 1 (Live Blender MCP — preferred)**:
  Call `execute_blender_code`:
  ```python
  import sys
  sys.path.insert(0, "/Users/gerth/Projects/elfstoryreborn/tools")
  from blender_retopo_bake import retopo_and_bake_asset
  retopo_and_bake_asset(
      input_path="/Users/gerth/GLocalLM/workspace/3d/<slug>-<stamp>.glb",
      output_path="/Users/gerth/Projects/elfstoryreborn/public/models/<slug>.glb",
      blend_path="/Users/gerth/Projects/elfstoryreborn/docs/models/<slug>.blend",
      asset_name="Oakhaven_<Slug>",
      target_quads=2500,
      tex_size=1024,
      target_width=1.0,
      cage_extrusion=0.03,
      roughness=0.72,
      metallic=0.05,
  )
  ```
  Then call Blender MCP `look` (`mode: "angles"`, `shading: "material"`, `views: ["three_quarter", "front", "right", "top"]`) and `get_scene_info` (`fields: ["size", "ground", "details", "topology"]`) to confirm `~2,500` quads, `0` non-manifold edges, intact silhouette, baked normal map, and `< 600 KB` `.glb`.
- **Option 2 (Headless Blender CLI)**:
  ```bash
  /Applications/Blender.app/Contents/MacOS/Blender --background --python \
    /Users/gerth/Projects/elfstoryreborn/tools/blender_retopo_bake.py -- \
    --input ~/GLocalLM/workspace/3d/<slug>-<stamp>.glb \
    --output /Users/gerth/Projects/elfstoryreborn/public/models/<slug>.glb \
    --blend /Users/gerth/Projects/elfstoryreborn/docs/models/<slug>.blend \
    --name Oakhaven_<Slug> \
    --target-quads 2500 --tex-size 1024 --target-width 1.0
  ```

---

## Stage 4 — Wire into Engine & Verify
1. Register placement in `src/game/modelLoader.ts` (`getOakhavenModelPlacements`), including `hideTerrainFootprint` when replacing procedural tiles. (Note: `src/components/PerspectiveCanvas.tsx` automatically provides `PMREMGenerator` + `RoomEnvironment` IBL so baked normal and albedo maps render with warm daylight cohesion.)
2. Update/add unit test coverage in `tests/modelLoader.test.ts`.
3. Run `npm run lint && npm test && npm run build`.
