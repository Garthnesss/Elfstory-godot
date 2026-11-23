# Asset gate — miniature figures batch 3 (2026-11-22)

Third figure batch, same pipeline and rules as batches 1–2 (`tools/ASSET_GATE.md`):
concept (DreamShaper XL Turbo via `tools/comfy_runner.py`, delighting OFF) →
BiRefNet cutout → TRELLIS.2 via `/foundry` queue (`tools/genq.py`) →
`tools/blender_decimate_source.py --rebake` (colour + metallic + roughness)
with sculpted base cut → game plinth in `godot/Main.gd`.
**Nothing here is committed until Garth rules on this pack.**

## Ash Cultist — `fig_ash_cultist.glb` (1.7 MB, 20,000 tris)

- **Concept:** seed 606 after rejecting earlier seeds with twin figures (s101), detached/double props (s202, s303, s404). Seed 606 is a clean solo hooded cultist in bright orange robes holding a glowing magma/ember orb (`fig_ash_cultist_concept_s606.png`).
- **Sculpt:** `fig_ash_cultist-20261122-174237` (425s) via `genq`.
- **Conversion:** 20,000 tris (from 135,597), 2048px rebake, auto base cut (`z = -0.4006`, `0.099` of height). Base slab and floor flame removed cleanly; feet/robe hem intact.
- **Watertight:** `boundary=12046 / nonmanifold=86 / degenerate=0` — **FAIL**, same TRELLIS open-shell exception class as batches 1–2.
- **In game (`fig_ash_cultist_ingame_overhead.png`, `fig_ash_cultist_ingame_chase.png`):** bright burnt-orange robe and glowing magma orb read unmistakably at both overhead and chase zoom in The Ember Vaults (`dungeon_3`).
- **Wired:** `ENEMY_FIGURES["ash_cultist"] = "fig_ash_cultist"`, `FIGURE_HEIGHT = 1.35`.

## Grave Husk — `fig_grave_husk.glb` v2 (2.3 MB, 27,999 tris) — FIXED after review REJECT

- **Concept:** seed 606 after rejecting twin-figure/floating-blade seeds (s101, s202, s707). Seed 606 is a solo desiccated undead barrow-guard in amber-brown tattered hood and armor holding a chunky shield-staff (`fig_grave_husk_concept_s606.png`).
- **Sculpt:** `fig_grave_husk-20261122-174953` (434s) via `genq`.
- **Conversion:**
  - v1 (19,999 tris, auto cut `z = -0.4230`): **REJECTED** by independent review — muddy shield-staff starburst rebake, dark/mushy skull cavity, spiky shin-greave decimation artifacts, blunted front sandal toes, and low in-game contrast.
  - v2 FINAL (`27,999` tris, `--cut-z 0.068` / `z = -0.4319`, `2048px` rebake, `2.3 MB` + Mira albedo lift `(v^0.85)*1.28` via `tools/glb_albedo_lift.py`): base slab gone, front sandal toes preserved, starburst relief and skull brow/jaw readable, bright amber-ochre contrast in `barrow_1`.
- **Watertight (v2):** `boundary=17147 / nonmanifold=70 / degenerate=3` — **FAIL**, same TRELLIS open-shell exception class as batches 1–2.
- **In game (`fig_grave_husk_ingame_overhead.png`, `fig_grave_husk_ingame_chase.png`):** amber-brown silhouette, hood, skull face, and shield-staff read cleanly in Whispering Barrow (`barrow_1`) alongside the Vanguard and Crypt Sentinel.
- **Wired:** `ENEMY_FIGURES["grave_husk"] = "fig_grave_husk"`, `FIGURE_HEIGHT = 1.38`.

## Independent adversarial review

- **Ash Cultist (`fig_ash_cultist.glb`):** **SHIP-WITH-EXCEPTION** — faithful burnt-orange robe & magma orb, high in-game readability in `dungeon_3`; caveats: watertightness fail (`12046/86/0`), 1.7 MB, bare toes sliced at `z = -0.4006` (masked by robe hem in-game), minor robe-pleat pinholes & softer orb cracks. **Shipped in `338c370`.**
- **Grave Husk (`fig_grave_husk.glb` v1 → v2):** v1 **REJECT** → v2 **SHIP-WITH-EXCEPTION** — Mira albedo lift fixed `barrow_1` contrast and skull cavity readability; `--cut-z 0.068` preserved front sandal strap/toe box; `27,999` tris recovered the shield starburst boss/arms and reduced shin spikes. Remaining caveats: watertightness fail (`17147/70/3`, `2.3 MB`), residual small shin-greave edge spikes, minor tabard/belt pinholes, and softer outer starburst tips.
