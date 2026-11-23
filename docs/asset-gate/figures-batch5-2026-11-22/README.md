# Asset gate — miniature figures batch 5 (2026-11-22)

Fifth figure batch (`dread_wraith` + `rime_revenant`, Frostpeak pair),
same pipeline and rules as batches 1–4 (`tools/ASSET_GATE.md`):
concept (DreamShaper XL Turbo via `tools/comfy_runner.py`, delighting OFF) →
BiRefNet cutout → TRELLIS.2 via `/foundry` queue (`tools/genq.py`) →
`tools/blender_decimate_source.py --rebake` (colour + metallic + roughness)
with sculpted base cut → game plinth in `godot/Main.gd`.
**Nothing here is committed until Garth rules on this pack.**

## Dread Wraith — `fig_dread_wraith.glb` (3.0 MB, 28,000 tris)

- **Concept:** `fig_dread_wraith_concept_s42.png` — solo sky-blue hooded phantom in tattered icy-blue robes holding a glowing blue faceted gem, bare feet planted on stone base.
- **Sculpt:** `fig_dread_wraith-20261122-232420` (530s) via `genq`.
- **Conversion:** 28,000 tris (from 154,175), 2048px rebake, auto base cut (`z = -0.4205`, `0.081` of height). Base slab & rocks removed cleanly; feet intact.
- **Watertight:** `boundary=30901 / nonmanifold=114 / degenerate=3` — **FAIL**, same TRELLIS open-shell exception class as batches 1–4.
- **In game (`fig_dread_wraith_ingame_overhead.png`, `fig_dread_wraith_ingame_chase.png`):** captured in `frost_2` (`9,10`) beside Vanguard, Silverwind Lynx, and Rime Revenant — sky-blue hood, robes, and glowing gem read cleanly.
- **Wired:** `ENEMY_FIGURES["dread_wraith"] = "fig_dread_wraith"`, `FIGURE_HEIGHT = 1.35`.

## Rime Revenant — `fig_rime_revenant.glb` (2.3 MB, 28,000 tris)

- **Concept:** `fig_rime_revenant_concept_s99.png` — solo pale-blue armored skeletal lich in icy-blue robes with gold trim, holding glowing blue orbs in both hands + belt tabard.
- **Sculpt:** `fig_rime_revenant-20261122-233316` (558s) via `genq`.
- **Conversion:** 28,000 tris (from 171,740), 2048px rebake, auto base cut (`z = -0.4227`, `0.066` of height). Round base slab removed cleanly; skeletal feet intact (small pale cloth/rock scraps around toes).
- **Watertight:** `boundary=20830 / nonmanifold=97 / degenerate=4` — **FAIL**, same TRELLIS open-shell exception class as batches 1–4.
- **In game (`fig_rime_revenant_ingame_overhead.png`, `fig_rime_revenant_ingame_chase.png`):** captured in `frost_2` (`9,10`) beside Vanguard, Silverwind Lynx, and Dread Wraith — pale-blue skeletal silhouette and three glowing blue orbs read clearly.
- **Wired:** `ENEMY_FIGURES["rime_revenant"] = "fig_rime_revenant"`, `FIGURE_HEIGHT = 1.40`.

## Independent adversarial review

- **Dread Wraith (`fig_dread_wraith.glb`):** **SHIP-WITH-EXCEPTION** — clean base cut (feet intact, base rocks gone) and clear sky-blue silhouette in `frost_2`. Exceptions: open-shell watertight (`30901/114/3`, `3.0 MB`), bake pitting on gold tabard trim & jagged hood rim, and `frost_2,9,10` chase shot shows it partially occluded behind Rime Revenant.
- **Rime Revenant (`fig_rime_revenant.glb`):** **REJECT** (overridable to `SHIP-WITH-EXCEPTION` if waiving base-rubble scraps) — pale-blue skeletal silhouette & 3 blue orbs read well in `frost_2`, but `z = -0.4227` (`0.066`) left jagged pale-grey base-rubble/cloth chunks fused around both feet spilling onto the plinth in `ingame_chase`, plus darkened skull cavity, right hood-fold polygon spike, and watertight `20830/97/4` (`2.3 MB`). Fix list: raise `--cut-z` (e.g. `0.082`) or clean foot rubble, apply albedo lift for skull/gold contrast, and re-capture unoccluded.
