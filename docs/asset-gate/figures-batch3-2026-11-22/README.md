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

## Grave Husk — `fig_grave_husk.glb` (1.7 MB, 19,999 tris)

- **Concept:** seed 606 after rejecting twin-figure/floating-blade seeds (s101, s202, s707). Seed 606 is a solo desiccated undead barrow-guard in amber-brown tattered hood and armor holding a chunky shield-staff (`fig_grave_husk_concept_s606.png`).
- **Sculpt:** `fig_grave_husk-20261122-174953` (434s) via `genq`.
- **Conversion:** 19,999 tris (from 151,272), 2048px rebake, auto base cut (`z = -0.4230`, `0.077` of height). Base slab removed cleanly; feet intact.
- **Watertight:** `boundary=15213 / nonmanifold=29 / degenerate=2` — **FAIL**, same TRELLIS open-shell exception class as batches 1–2.
- **In game (`fig_grave_husk_ingame_overhead.png`, `fig_grave_husk_ingame_chase.png`):** amber-brown silhouette, hood, and shield-staff read cleanly in Whispering Barrow (`barrow_1`) alongside the Vanguard and Crypt Sentinel.
- **Wired:** `ENEMY_FIGURES["grave_husk"] = "fig_grave_husk"`, `FIGURE_HEIGHT = 1.38`.

## Independent adversarial review

- **Ash Cultist (`fig_ash_cultist.glb`):** **SHIP-WITH-EXCEPTION** — faithful burnt-orange robe & magma orb, high in-game readability in `dungeon_3`; caveats: watertightness fail (`12046/86/0`), 1.7 MB, bare toes sliced at `z = -0.4006` (masked by robe hem in-game), minor robe-pleat pinholes & softer orb cracks.
- **Grave Husk (`fig_grave_husk.glb`):** **REJECT** — base slab removed cleanly and silhouette reads, but conversion degrades the source: muddy shield-staff starburst rebake, dark/mushy skull cavity, spiky shin-greave decimation artifacts, blunted front sandal toes at `z = -0.4230`, darker/low-contrast in `barrow_1`, and `2` degenerate tris (`15213/29/2`). Fix list: higher poly retention (`28k` tris), lower `--cut-z` to keep toes, and `glb_albedo_lift.py` brightness lift.
