# Asset Gate Pack — Town NPCs Highspire Batch 2: Highspire Complete (5/5) (2026-11-24)

Highspire Batch 2 completes **5/5 Highspire Citadel NPCs** (zero green placeholder capsules left in Highspire) with **Relic Appraiser Orin** and **Page Tomas**, wired via `NPC_FIGURES` in `godot/Main.gd`.

## Figures in This Batch

| NPC ID | Name & Role (Highspire Tile) | Shipped GLB | Raw Tris → Shipped Tris | GLB Size | `FIGURE_HEIGHT` | `check_watertight.py` |
|---|---|---|---|---|---|---|
| `orin` | Relic Appraiser Orin, Master Artificer `(21, 11)` | `godot/assets/models/figures/fig_npc_orin.glb` | 115,834 → 44,999 (38.8%, UV-preserving `--cut-z 0.111`) | 2.1 MB | `1.26` | FAIL (`boundary=7672 nonmanifold=855 degenerate=3`) |
| `tomas` | Page Tomas, Citadel Page `(10, 8)` | `godot/assets/models/figures/fig_npc_tomas.glb` | 184,330 → 44,999 (24.4%, UV-preserving, no base cut needed) | 2.0 MB | `1.18` | FAIL (`boundary=9525 nonmanifold=421 degenerate=1`) |

## Pipeline & Sources

- **Concepts:** DreamShaper XL Turbo v2.1 (`DreamShaperXL_Turbo_v2_1.safetensors`, `--steps 6 --cfg 2.0`, delighting off):
  - `docs/concepts/figures/fig_npc_orin_ds_s22.png`
  - `docs/concepts/figures/fig_npc_tomas_ds_s22.png`
- **Cutouts:** BiRefNet (`rembg i -m birefnet-general`) in `docs/concepts/figures/cutouts/fig_npc_{orin,tomas}.png`
- **Sculpts (TRELLIS.2 via Elf Works `/foundry` & `genq.py`):**
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_npc_orin-20261124-101539.glb`
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_npc_tomas-20261124-103350.glb` (400s)
- **Conversion:** `tools/blender_decimate_source.py --target-tris 38000 --rebake --tex-size 2048` (`--cut-base` on `orin`; no base cut needed on `tomas`).

## Evidence Files

- **Orin:** `orin_source.png`, `orin_candidate.png`, `orin_ingame_overhead.png`, `orin_ingame_chase.png`
- **Tomas:** `tomas_source.png`, `tomas_candidate.png`, `tomas_ingame_overhead.png`, `tomas_ingame_chase.png`

## Requested Exceptions & Round-2 Fix Summary

1. **Open-shell / non-watertight mesh** (`check_watertight.py` FAIL on both): standard for TRELLIS.2 decimated miniature figures.
2. **GLB size > 600 KB** (`2.0–2.1 MB` at 45k tris): preserves fine facial features, spectacles, beard, and backpack/pouch details.
3. **Round-1 Adversarial Review → Round-2 Fixes Applied:**
   - **Orin & Tomas:** Switched both from Cycles `--rebake` (`38k`) to **UV-preserving `45k` decimation**, eliminating Tomas's blue-sleeve slash and hip-pouch seam tear and cleaning Orin's beard/pauldron rebake speckles (minor close-zoom micro-speckling on Orin's beard and a tiny shadowed side-slit on Tomas's backpack remain invisible at gameplay distance).

## Verification

- `cd rust && cargo test`: **111/111 passed**
- `/Users/gerth/Projects/gamedev-pipeline/tools/godot_verify.sh godot`: **ALL GREEN** (`import`, `parse (7 scripts)`, `playtest_check.gd`, `run1_check.gd`)
