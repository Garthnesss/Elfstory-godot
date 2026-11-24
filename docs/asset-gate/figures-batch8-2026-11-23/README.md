# Asset Gate Pack — Enemy Figures Batch 8 (Final Bosses: 17/17 Complete) (2026-11-23)

Batch 8 completes the **17/17 enemy figure roster** with the three dungeon bosses, paired with Garth's approved **larger gold-rimmed boss plinth** (`make_actor(..., is_boss)` in `godot/Main.gd`: wider/taller mahogany plinth scaled `1.35×` with dual metallic gold bezel rings).

## Figures in This Batch

| Archetype | Boss Name & Zone | Shipped GLB | Raw Tris → Shipped Tris | GLB Size | `FIGURE_HEIGHT` (Effective) | `check_watertight.py` |
|---|---|---|---|---|---|---|
| `boss_bone_cantor` | Vaelen, Bone Cantor (`dungeon_2`) | `godot/assets/models/figures/fig_boss_bone_cantor.glb` | 159,259 → 37,999 (23.9%) | 2.4 MB | `1.38` (`1.86` tiles) | FAIL (`boundary=11633 nonmanifold=393 degenerate=2`) |
| `boss_ashen_inquisitor` | Inquisitor Serath (`dungeon_4`) | `godot/assets/models/figures/fig_boss_ashen_inquisitor.glb` | 150,776 → 38,000 (25.2%) | 2.3 MB | `1.42` (`1.92` tiles) | FAIL (`boundary=10375 nonmanifold=317 degenerate=3`) |
| `boss_malakor` | Malakor, Sovereign of Cinder (`dungeon_5`) | `godot/assets/models/figures/fig_boss_malakor.glb` | 154,263 → 38,000 (24.6%) | 2.0 MB | `1.48` (`2.00` tiles) | FAIL (`boundary=6783 nonmanifold=235 degenerate=5`) |

## Pipeline & Sources

- **Concepts:** DreamShaper XL Turbo v2.1 (`DreamShaperXL_Turbo_v2_1.safetensors`, `--steps 6 --cfg 2.0`, delighting off)
  - `docs/concepts/figures/fig_boss_bone_cantor_ds_s22.png`
  - `docs/concepts/figures/fig_boss_ashen_inquisitor_ds_s11.png`
  - `docs/concepts/figures/fig_boss_malakor_ds_s22.png`
- **Cutouts:** BiRefNet (`rembg i -m birefnet-general`) in `docs/concepts/figures/cutouts/fig_boss_*.png`
- **Sculpts (TRELLIS.2 via Elf Works `/foundry` & `genq.py`):**
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_boss_bone_cantor-20261123-201913.glb` (436s)
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_boss_ashen_inquisitor-20261123-202648.glb` (393s)
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_boss_malakor-20261123-203331.glb` (449s)
- **Conversion:** `tools/blender_decimate_source.py --target-tris 38000 --rebake --tex-size 2048 --cut-base` (Smart UV `island_margin=0.0015`, Cycles 32 samples, JPEG-Q85 embedded PBR maps; no post-hoc albedo lift required — all three read cleanly under dungeon AgX lighting).

## Evidence Files

- **Bone Cantor:** `bone_cantor_source.png`, `bone_cantor_candidate.png`, `bone_cantor_ingame_overhead.png`, `bone_cantor_ingame_chase.png`
- **Ashen Inquisitor:** `ashen_inquisitor_source.png`, `ashen_inquisitor_candidate.png`, `ashen_inquisitor_ingame_overhead.png`, `ashen_inquisitor_ingame_chase.png`
- **Malakor:** `malakor_source.png`, `malakor_candidate.png`, `malakor_ingame_overhead.png`, `malakor_ingame_chase.png`

## Requested Exceptions

1. **Open-shell / non-watertight mesh** (`check_watertight.py` FAIL on all 3): standard for TRELLIS.2 decimated + `--cut-base` open-bottom miniature figures (same approved exception as Batches 1–7).
2. **GLB size > 1.5 MB** (`2.0–2.4 MB` at 38k tris / 2048px rebake): preserves fine boss armor trim, crown, skull, and glowing emissive-look accents without texture smearing (same approved exception as Batch 7).

## Independent Adversarial Review Verdict (2026-11-23)

- **`boss_bone_cantor` — `SHIP-WITH-EXCEPTION`** (no blockers; minor note: tiny ~2–3 px pink base sliver at bottom of staff tip on the plinth).
- **`boss_ashen_inquisitor` — `SHIP-WITH-EXCEPTION`** (no blockers; cleanest technical conversion in the batch).
- **`boss_malakor` — `SHIP-WITH-EXCEPTION`** (no blockers; minor notes: sweeping black cape extends slightly over adjacent tile's outer ring, dark cape blends into overhead dungeon shadow while armor/glows/gold plinth read clearly).

## Verification

- `cd rust && cargo test`: **107/107 passed**
- `/Users/gerth/Projects/gamedev-pipeline/tools/godot_verify.sh godot`: **ALL GREEN** (`import`, `parse (6 scripts)`, `playtest_check.gd`, `run1_check.gd`)

