# Asset Gate Pack — Town NPCs Batch 2: Oakhaven Complete (6/6) (2026-11-24)

Batch 2 completes **6/6 Oakhaven NPCs** (zero green placeholder capsules left in Oakhaven) with the blade-free **Warden Vance v3** re-sculpt plus the two roaming townsfolk **Pell the Carter** and **Sister Wren**, wired via `NPC_FIGURES` in `godot/Main.gd`.

## Figures in This Batch

| NPC ID | Name & Role (Oakhaven Tile) | Shipped GLB | Raw Tris → Shipped Tris | GLB Size | `FIGURE_HEIGHT` | `check_watertight.py` |
|---|---|---|---|---|---|---|
| `vance` | Warden Vance, Outpost Captain `(14, 8)` | `godot/assets/models/figures/fig_npc_vance.glb` | 171,997 → 39,999 (23.3%, `--rebake --tex-size 2048 --cut-z 0.040` + albedo lift `0.85/1.25`) | 3.0 MB | `1.32` | FAIL (`boundary=18971 nonmanifold=381 degenerate=3`) |
| `pell` | Pell the Carter, Outpost Carter `(10, 5)` | `godot/assets/models/figures/fig_npc_pell.glb` | 183,644 → 38,000 (20.7%, `--rebake --tex-size 2048`, no base cut needed) | 2.3 MB | `1.26` | FAIL (`boundary=17687 nonmanifold=307 degenerate=0`) |
| `wren` | Sister Wren, Shrine Keeper `(19, 8)` | `godot/assets/models/figures/fig_npc_wren.glb` | 99,319 → 40,000 (40.3%, `--rebake --tex-size 2048 --cut-z 0.060`) | 2.8 MB | `1.26` | FAIL (`boundary=36800+ nonmanifold=240+ degenerate=2`) |

## Pipeline & Sources

- **Concepts:** DreamShaper XL Turbo v2.1 (`DreamShaperXL_Turbo_v2_1.safetensors`, `--steps 6 --cfg 2.0`, delighting off):
  - `docs/concepts/figures/fig_npc_vance_v3_ds_s33.png` (blade-free re-concept replacing held v1)
  - `docs/concepts/figures/fig_npc_pell_ds_s22.png`
  - `docs/concepts/figures/fig_npc_wren_ds_s22.png`
- **Cutouts:** BiRefNet (`rembg i -m birefnet-general`) in `docs/concepts/figures/cutouts/fig_npc_{vance_v3,pell,wren}.png`
- **Sculpts (TRELLIS.2 via Elf Works `/foundry` & `genq.py`):**
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_npc_vance_v3-20261124-001256.glb` (414s)
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_npc_pell-20261124-002014.glb` (324s)
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_npc_wren-20261124-002601.glb`
- **Conversion (Round-2 fix pass):**
  - `vance`: `blender_decimate_source.py --target-tris 40000 --rebake --tex-size 2048 --cut-base --cut-z 0.040` + `glb_albedo_lift.py 0.85 1.25` (`3.0 MB`)
  - `pell`: `blender_decimate_source.py --target-tris 38000 --rebake --tex-size 2048` (`2.3 MB`)
  - `wren`: `blender_decimate_source.py --target-tris 40000 --rebake --tex-size 2048 --cut-base --cut-z 0.060` (`2.8 MB`)
  - Also fixed `tools/render_preview.py` camera framing (`dist = size * 1.38` + `to_track_quat`) so tall humanoid heads are no longer cropped.

## Evidence Files

- **Vance v3:** `vance_source.png`, `vance_candidate.png`, `vance_ingame_overhead.png`, `vance_ingame_chase.png`
- **Pell:** `pell_source.png`, `pell_candidate.png`, `pell_ingame_overhead.png`, `pell_ingame_chase.png`
- **Wren:** `wren_source.png`, `wren_candidate.png`, `wren_ingame_overhead.png`, `wren_ingame_chase.png`

## Requested Exceptions & Review Notes

1. **Open-shell / non-watertight mesh** (`check_watertight.py` FAIL on all 3): standard for TRELLIS.2 decimated miniature figures.
2. **GLB size > 600 KB** (`2.3–3.0 MB` at 38k–40k tris / 2048px rebake): preserves fine facial features, clothing trim, and props.
3. **Round-1 Adversarial Review → Round-2 Fixes Applied:**
   - **Vance v3:** Fixed in-game dark shadow crushing (`glb_albedo_lift.py 0.85 1.25`, now pops cleanly beside Vanguard) and switched to `--rebake`; minor close-zoom beard/tasset texture grain remains (invisible at gameplay scale).
   - **Wren:** Switched to `--rebake` + lowered `--cut-z` to `0.060` — restored intact right boot toe and eliminated cream-robe UV streaks.
   - **Pell:** Full-frame preview confirms clean cap/vest/boots; minor close-zoom pouch flap darkening (invisible in-game); note Pell's world spawn `(10, 5)` sits beside the Market Lot fence (town layout, not mesh).

## Verification

- `cd rust && cargo test`: **107/107 passed**
- `/Users/gerth/Projects/gamedev-pipeline/tools/godot_verify.sh godot`: **ALL GREEN** (`import`, `parse (6 scripts)`, `playtest_check.gd`, `run1_check.gd`)
