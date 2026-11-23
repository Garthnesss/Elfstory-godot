# Miniature Figures — Batch 6 Asset Gate (2026-11-23)

Targets: `iron_golem` and `cinder_wisp` (`dungeon_4`, Inquisitor Crucible).

## Iron Golem — `fig_iron_golem.glb` (1.5 MB, 27,998 tris)

- **Concept:** `docs/concepts/figures/fig_iron_golem_ds_s11.png` (`DreamShaperXL_Turbo_v2_1.safetensors`, seed 11, 6 steps, CFG 2.0) — solo heavy iron golem in dark gunmetal plate armor with glowing cyan chest core, pauldrons, bracers, tabard, and horned visor, standing on a clean flat round base with zero rubble. (Replaced earlier `sd_xl_turbo` concepts `s505`/`s606`/`s707`/`s808`/`s909`/`s101`/`s202` which had extra limbs/floating rocks/conjoined bodies.)
- **Cutout:** `docs/concepts/figures/cutouts/fig_iron_golem.png` via BiRefNet (`rembg`).
- **Sculpt:** `fig_iron_golem-20261123-091622` (2657s) via `genq`.
- **Conversion:** 27,998 tris (from 155,472), 2048px rebake, auto base cut (`z = -0.4082`, `0.092` of height; removed 155 debris faces). Both boots cleanly freed from the round base.
- **Watertight:** `boundary=3306 / nonmanifold=76 / degenerate=1` — **FAIL**, same TRELLIS open-shell exception class as batches 1–5 (cleanest boundary count of any TRELLIS figure so far).
- **In game (`batch6_ingame_overhead.png`, `batch6_ingame_chase.png`):** captured unoccluded in `dungeon_4` (`39,15`) beside Vanguard, Silverwind Lynx, Dread Wraith, and Cinder Wisp — chunky gunmetal silhouette and cyan armor accents read immediately.
- **Wired:** `ENEMY_FIGURES["iron_golem"] = "fig_iron_golem"`, `FIGURE_HEIGHT = 1.48`.

## Cinder Wisp — `fig_cinder_wisp.glb` (2.3 MB, 28,000 tris)

- **Concept & Cutout:** `docs/concepts/figures/cutouts/fig_cinder_wisp.png` — golden-flame horned elemental with fiery orange orb and smoky grey tendrils.
- **Sculpt:** `fig_cinder_wisp-20261123-090656` (563s) via `genq`.
- **Conversion:** 28,000 tris (from 158,357), 2048px rebake, raised base cut (`--cut-z 0.138`, `z = -0.3616`; removed 2,201 debris faces). Flat base disc and front fire spikes removed; golden feet intact, with a small grey smoky/rocky mass remaining under the back heel.
- **Watertight:** `boundary=13050 / nonmanifold=78 / degenerate=2` — **FAIL**, same TRELLIS open-shell exception class as batches 1–5.
- **In game (`batch6_ingame_overhead.png`, `batch6_ingame_chase.png`):** captured unoccluded in `dungeon_4` (`39,15`) — golden horns, fiery orb, and smoky grey tendrils sit cleanly inside the red enemy ring.
- **Wired:** `ENEMY_FIGURES["cinder_wisp"] = "fig_cinder_wisp"`, `FIGURE_HEIGHT = 1.32`.

## Independent adversarial review

- **Iron Golem (`fig_iron_golem.glb`):** **SHIP-WITH-EXCEPTION** — strong concept fidelity, crisp gunmetal + glowing cyan silhouette in `dungeon_4`, clean base removal (`z = -0.4082`, flush on red plinth), `1.5 MB` / `27,998` tris. Non-blocking exceptions: open-shell watertight (`3306/76/1`), slightly clipped boot soles, minor dark seam on left pauldron rim.
- **Cinder Wisp (`fig_cinder_wisp.glb`):** **REJECT** — golden armor shifted to mustard-yellow with dark eye cavity, grey rocky rubble mass still attached beneath the figure at `--cut-z 0.138` (`z = -0.3616`), fragmented left-hand fingers, protruding block artifact on right forearm, pauldron/fireball bake spotting, and `boundary=13050 / nonmanifold=78 / degenerate=2`. Fix options: regenerate clean no-rubble wisp concept with DreamShaperXL Turbo (closer to a floating/ember wisp rather than rocky humanoid) + re-sculpt, or clean rubble/apply albedo lift on current sculpt.
