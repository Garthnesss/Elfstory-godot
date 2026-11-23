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

## Cinder Wisp — `fig_cinder_wisp.glb` (v2: 1.9 MB, 27,998 tris)

- **Concept:** `docs/concepts/figures/fig_cinder_wisp_ds_s22.png` (`DreamShaperXL_Turbo_v2_1.safetensors`, seed 22, 6 steps, CFG 2.0) — solo fire elemental with bright orange flame crest, glowing orange eyes in dark obsidian mask, bronze-obsidian armor, and glowing orange fireball on a clean flat stone disc base.
- **Cutout:** `docs/concepts/figures/cutouts/fig_cinder_wisp_v2.png` via BiRefNet (`rembg`).
- **Sculpt:** `fig_cinder_wisp_v2-20261123-101409` (417s) via `genq`.
- **Conversion (v2):** 27,998 tris (from 139,469), 2048px rebake, auto base cut (`z = -0.4156`, `0.084` of height; removed 418 debris faces). Base disc and stone slab completely removed; boots clean.
- **Watertight:** `boundary=6250 / nonmanifold=82 / degenerate=1` — **FAIL**, same TRELLIS open-shell exception class as batches 1–5 (cut in half from v1's `13050/78/2`).
- **In game (`fig_cinder_wisp_ingame_overhead.png`, `fig_cinder_wisp_ingame_chase.png`):** captured unoccluded in `dungeon_4` (`35,4`) front-and-center beside Vanguard, Silverwind Lynx, and Dread Wraith — flame crest, fireball, bronze-obsidian armor, and clean boots on the red plinth ring read immediately.
- **Wired:** `ENEMY_FIGURES["cinder_wisp"] = "fig_cinder_wisp"`, `FIGURE_HEIGHT = 1.32`.

## Independent adversarial review

- **Iron Golem (`fig_iron_golem.glb`):** **SHIP-WITH-EXCEPTION** — strong concept fidelity, crisp gunmetal + glowing cyan silhouette in `dungeon_4`, clean base removal (`z = -0.4082`, flush on red plinth), `1.5 MB` / `27,998` tris. Non-blocking exceptions: open-shell watertight (`3306/76/1`), slightly clipped boot soles, minor dark seam on left pauldron rim. (Shipped in `366c537`.)
- **Cinder Wisp (`fig_cinder_wisp.glb` v1):** **REJECT** (held) — mustard-yellow shift, dark eye cavity, grey rocky rubble mass beneath feet at `--cut-z 0.138`, fragmented fingers, right-forearm block artifact.
- **Cinder Wisp (`fig_cinder_wisp.glb` v2):** **SHIP-WITH-EXCEPTION** — all v1 blockers resolved: vibrant orange flame crest + glowing fireball + bronze-obsidian armor + readable glowing eyes, clean base cut (`z = -0.4156`, zero rubble, flush inside red plinth ring), intact left-hand fingers, and clean forearm bracer. Only exceptions: TRELLIS open-shell watertight (`6250/82/1`, `1.9 MB`) and faint dark line on left pauldron plate inherited from source sculpt.
