# Miniature Figures — Batch 7 Asset Gate (2026-11-23)

Targets: `obsidian_warden` (`dungeon_4` / `frost`) and `spore_hulk` (`mire_1` / `dungeon_6+`).

## Obsidian Warden — `fig_obsidian_warden.glb` (v2 fix: 2.5 MB, 38,000 tris)

- **Concept:** `docs/concepts/figures/fig_obsidian_warden_v2_ds_s55.png` (`DreamShaperXL_Turbo_v2_1.safetensors`, seed 55, 6 steps, CFG 2.0) — cape-less charcoal-black obsidian plate knight with coral-pink (`#FB7185`) horned helm visor, chest runes, pauldrons, tabard, axe-halberd, and kite shield on a smooth flat round black disc base. *(Replaced concept v1 `s44` whose thin flowing cape produced 20,578 raw non-manifold edges.)*
- **Cutout:** `docs/concepts/figures/cutouts/fig_obsidian_warden_v2.png` via BiRefNet (`rembg`).
- **Sculpt:** `fig_obsidian_warden_v2-20261123-120216` (476s) via `genq.py foundry`.
- **Conversion (v2 fix pass):** 38,000 tris (from 154,328), 2048px rebake (`island_margin=0.0015`, 32 Cycles samples — restored crisp axe-hub runes, left pauldron shading, chest X-rune, and belt/shield detail after 28k/0.01-margin v1 smeared), auto base cut (`z = -0.4158`, `0.084` of height; removed 1,042 debris faces), plus Mira albedo lift (`gamma=0.85, gain=1.28`) to prevent dark-armor crush under AgX.
- **Watertight:** `boundary=15228 / nonmanifold=395 / degenerate=6` — **FAIL**, same TRELLIS open-shell exception class as batches 1–6.
- **In game (`fig_obsidian_warden_ingame_overhead.png`, `fig_obsidian_warden_ingame_chase.png`):** captured unoccluded in `dungeon_4` (`21,6`) standing directly behind Vanguard (`1.30`) and beside Iron Golem (`1.48`) and Silverwind Lynx — lifted charcoal armor plates separate cleanly under AgX, coral-pink runes/helm/axe/shield pop immediately at `FIGURE_HEIGHT = 1.52`.
- **Wired:** `ENEMY_FIGURES["obsidian_warden"] = "fig_obsidian_warden"`, `FIGURE_HEIGHT = 1.52`.

## Spore Hulk — `fig_spore_hulk.glb` (v3 fix: 2.7 MB, 37,999 tris)

- **Concept:** `docs/concepts/figures/fig_spore_hulk_ds_s33.png` (`DreamShaperXL_Turbo_v2_1.safetensors`, seed 33, 6 steps, CFG 2.0) — siege-sized fungal bloom-mother golem with spiky lime-green (`#84CC16`) mushroom cap, glowing red-orange eyes, gnarled bark-and-moss body, and heavy clawed hands/feet.
- **Cutout:** `docs/concepts/figures/cutouts/fig_spore_hulk.png` via BiRefNet (`rembg`).
- **Sculpt:** `fig_spore_hulk-20261123-114537` (544s) via `genq.py foundry`.
- **Conversion (v3 fix pass):** 37,999 tris (from 122,601), 2048px rebake (`island_margin=0.0015`, 32 Cycles samples — crisp bark/muscle striations, belly lobes, cap spots, and jaw detail), base cut lowered to `--cut-z 0.109` (`z = -0.3908`; removed 1,854 debris faces while keeping all three front toe claws intact), plus a shadow-targeted albedo curve (`gain = 1 + 0.70*(1 - luma)^2.4`) that opens the under-cap face cavity and dark torso without bleaching the lime mushroom cap.
- **Watertight:** `boundary=11983 / nonmanifold=293 / degenerate=12` — **FAIL**, same TRELLIS open-shell exception class as batches 1–6.
- **In game (`fig_spore_hulk_ingame_overhead.png`, `fig_spore_hulk_ingame_chase.png`):** captured unoccluded in `mire_1` (`34,8`) in Fungal Hollow beside Vanguard and Silverwind Lynx (with a second Spore Hulk and Blight Weaver in the adjacent room) — rich mossy-lime cap matches source, `FIGURE_HEIGHT = 1.54` gives true siege-sized presence, claws sit cleanly on the red plinth, and musculature/mouth detail reads cleanly under AgX.
- **Wired:** `ENEMY_FIGURES["spore_hulk"] = "fig_spore_hulk"`, `FIGURE_HEIGHT = 1.54`.

## Independent adversarial review

- **Obsidian Warden (`fig_obsidian_warden.glb`):**
  - Round 1: **REJECT** — left-elbow/ankle tears, smeared axe-hub/pauldron bake, pinched front-left toe, dark armor crush under AgX.
  - Round 2 (v2 fix pass): **SHIP-WITH-EXCEPTION** — axe-hub runes, chest X-rune, and `dungeon_4` AgX plate separation resolved; non-blocking exceptions: TRELLIS open-shell watertight (`15228/395/6`, `2.5 MB`), minor left-elbow crease notch and right-boot arch sliver at close zoom, marbled highlight on left pauldron, pinched front-left toe inherited from source, and pastel shift on coral trim in neutral light (recovers under AgX).
- **Spore Hulk (`fig_spore_hulk.glb`):**
  - Round 1: **REJECT** — clipped front claws (`--cut-z 0.115`), uneven rear foot, smeared bark/mouth detail, pitch-black face cavity in `mire_1`.
  - Round 2 (v2 fix pass): **REJECT** — global `0.80/1.32` albedo lift bleached the mushroom cap to chalky mint-white in-game.
  - Round 3 (v3 fix pass): **SHIP-WITH-EXCEPTION** — shadow-targeted lift restored rich mossy-lime cap color in-game while keeping face/torso readable, all front claws intact at `--cut-z 0.109`, and `1.54` scale reads siege-sized; non-blocking exceptions: TRELLIS open-shell watertight (`11983/293/12`, `2.7 MB`), uneven rear-foot elevation and overhanging cap brow shading the eyes from elevated game cameras (inherited from source), and slight toe-tip flattening / wrist-moss clumps at close zoom.
