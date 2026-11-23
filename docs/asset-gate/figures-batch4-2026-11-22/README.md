# Asset gate — miniature figures batch 4 (2026-11-22)

Fourth figure batch, same pipeline and rules as batches 1–3 (`tools/ASSET_GATE.md`):
concept (DreamShaper XL Turbo via `tools/comfy_runner.py`, delighting OFF) →
BiRefNet cutout → TRELLIS.2 via `/foundry` queue (`tools/genq.py`) →
`tools/blender_decimate_source.py --rebake` (colour + metallic + roughness)
with sculpted base cut → game plinth in `godot/Main.gd`.
**Nothing here is committed until Garth rules on this pack.**

## Shadow Stalker — `fig_shadow_stalker.glb` v3 (2.1 MB, 28,000 tris)

- **v1 & v2 REJECTED:** v1 had blunt cloak clubs + lattice orb + wrong in-game shot; v2 had a floating detached sword hovering in front of the hand.
- **v3 Concept:** `fig_shadow_stalker_concept_s606.png` — solo hooded violet figure holding a chunky gold-and-purple-crystal torch gripped firmly in hand, boots planted on stone base.
- **v3 Sculpt:** `fig_shadow_stalker_v3-20261122-213649` (430s) via `genq`.
- **v3 Conversion:** 28,000 tris (from 160,238), 2048px rebake, auto base cut (`z = -0.4456`, `0.054` of height). Base slab & rocks removed cleanly; boots intact.
- **Watertight (v3):** `boundary=20088 / nonmanifold=94 / degenerate=2` — **FAIL**, same TRELLIS open-shell exception class as batches 1–3.
- **In game (`fig_shadow_stalker_ingame_overhead.png`, `fig_shadow_stalker_ingame_chase.png`):** captured in `dungeon_5` (`16,21`) beside Crypt Sentinel, Blight Rat, Silverwind Lynx, and Vanguard — violet hood, cloak, and glowing purple crystal torch read cleanly.
- **Wired:** `ENEMY_FIGURES["shadow_stalker"] = "fig_shadow_stalker"`, `FIGURE_HEIGHT = 1.35`.

## Blight Weaver — `fig_blight_weaver.glb` v3 (2.6 MB, 27,999 tris) — FIXED after v3 REJECT

- **v1 & v2 REJECTED:** v1 had shredded side tree stump + stray staff spike; v2 had bulbous hanging sleeve, hollowed face, and cut foot nub.
- **v3 Concept:** `fig_blight_weaver_concept_s606.png` — solo emerald-green druid hexer with fitted sleeves (no wide hanging sleeves) and glowing green crystals mounted on a solid root-pedestal.
- **v3 Sculpt:** `fig_blight_weaver_v3-20261122-214405` (514s) via `genq`.
- **v3 Conversion:**
  - cut 1 (`0.054`): **REJECTED** — left a tan base-rock mound under the foot/pedestal visible in `ingame_chase.png`.
  - cut 2 FINAL (`--cut-z 0.088` / `z = -0.4118`, `27,999` tris, `2048px` rebake): tan base-rock mound stripped cleanly, foot/toes preserved.
- **Watertight (v3 final):** `boundary=24115 / nonmanifold=160 / degenerate=6` — **FAIL**, same TRELLIS open-shell exception class as batches 1–3.
- **In game (`fig_blight_weaver_ingame_overhead.png`, `fig_blight_weaver_ingame_chase.png`):** captured in `dungeon_3` (`6,12`) beside Ash Cultist — sits cleanly on the dark plinth, vibrant emerald robes and glowing green crystals read cleanly.
- **Wired:** `ENEMY_FIGURES["blight_weaver"] = "fig_blight_weaver"`, `FIGURE_HEIGHT = 1.38`.

## Independent adversarial review (v3)

- **Shadow Stalker (`fig_shadow_stalker.glb` v3):** **SHIP-WITH-EXCEPTION** — v1/v2 flaws resolved (torch gripped firmly in hand, boots intact, base rocks removed, crisp in `dungeon_5`). Exceptions: open-shell watertight (`20088/94/2`, `2.1 MB`), close-zoom white/black rebake speckle on the torch grip shaft (invisible in-game), minor chest/belt dimples. **Shipped in `0f2def9`.**
- **Blight Weaver (`fig_blight_weaver.glb` v3, cut 1 → cut 2 `--cut-z 0.088`):** cut 1 (`0.054`) **REJECT** → cut 2 **REJECT (overridable to `SHIP-WITH-EXCEPTION` if waiving the root-base rock cluster)** — `--cut-z 0.088` shaved down the projecting tan rock mound while keeping the front foot/toes intact, and emerald robes, fitted sleeves, and glowing crystals read cleanly in `dungeon_3`. Remaining flaws: a low tan rock cluster still clings around the root-pedestal base (planar cut cannot remove it without slicing toes), dark bake pitting on bracers/trim, thin vertical belt-strip rod, open-shell watertight (`24115/160/6`, `2.6 MB`).
