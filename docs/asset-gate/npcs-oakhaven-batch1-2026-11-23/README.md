# Asset Gate Pack — Town NPCs Batch 1: Oakhaven Core (2026-11-23)

Batch 1 replaces the green placeholder capsules for the **4 core named Oakhaven NPCs** (`torin`, `mira`, `vance`, `vex`) with portrait-matched miniature figures wired via `NPC_FIGURES` in `godot/Main.gd`.

## Figures in This Batch

| NPC ID | Name & Role (Oakhaven Tile) | Shipped GLB | Raw Tris → Shipped Tris | GLB Size | `FIGURE_HEIGHT` | `check_watertight.py` |
|---|---|---|---|---|---|---|
| `torin` | Torin Ironhand, Blacksmith `(7, 4)` | `godot/assets/models/figures/fig_npc_torin.glb` | 158,058 → 37,999 (24.0%, `--cut-z 0.050`) | 1.9 MB | `1.24` | FAIL (`boundary=6368 nonmanifold=206 degenerate=3`) |
| `mira` | Herbalist Mira, Apothecary `(22, 4)` | `godot/assets/models/figures/fig_npc_mira.glb` | 127,273 → 38,000 (29.9%, auto cut `0.066`) | 2.3 MB | `1.26` | FAIL (`boundary=14627 nonmanifold=568 degenerate=2`) |
| `vance` | Warden Vance, Outpost Captain `(14, 8)` | `godot/assets/models/figures/fig_npc_vance.glb` | 153,522 → 42,000 (27.4%, UV-preserving `--cut-z 0.068`) | 2.1 MB | `1.32` | FAIL (`boundary=11022 nonmanifold=530 degenerate=5`) |
| `vex` | Vex the Quiet, Shadow Broker `(6, 13)` | `godot/assets/models/figures/fig_npc_vex.glb` | 166,599 → 41,999 (25.2%, UV-preserving `--cut-z 0.036` + albedo lift `0.80/1.35`) | 2.0 MB | `1.28` | FAIL (`boundary=10021 nonmanifold=507 degenerate=0`) |

## Pipeline & Sources

- **Concepts:** DreamShaper XL Turbo v2.1 (`DreamShaperXL_Turbo_v2_1.safetensors`, `--steps 6 --cfg 2.0`, delighting off), matched to official portraits in `godot/assets/portraits/`:
  - `docs/concepts/figures/fig_npc_torin_v2_ds_s33.png`
  - `docs/concepts/figures/fig_npc_mira_v2_ds_s33.png`
  - `docs/concepts/figures/fig_npc_vance_ds_s11.png`
  - `docs/concepts/figures/fig_npc_vex_v2_ds_s33.png`
- **Cutouts:** BiRefNet (`rembg i -m birefnet-general`) in `docs/concepts/figures/cutouts/fig_npc_*.png`
- **Sculpts (TRELLIS.2 via Elf Works `/foundry` & `genq.py`):**
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_npc_torin-20261123-220747.glb` (460s)
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_npc_mira-20261123-221546.glb` (494s)
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_npc_vance-20261123-222422.glb` (519s)
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_npc_vex-20261123-223332.glb` (386s)
- **Conversion:**
  - `torin`: `blender_decimate_source.py --target-tris 38000 --rebake --tex-size 2048 --cut-base --cut-z 0.050` (`1.9 MB`)
  - `mira`: `blender_decimate_source.py --target-tris 38000 --rebake --tex-size 2048 --cut-base` (`2.3 MB`)
  - `vance`: `blender_decimate_source.py --target-tris 42000 --cut-base --cut-z 0.068` (UV-preserving, `2.1 MB`)
  - `vex`: `blender_decimate_source.py --target-tris 42000 --cut-base --cut-z 0.036` (UV-preserving + WebP albedo lift `0.80/1.35`, `2.0 MB`)

## Evidence Files

- **Torin:** `torin_source.png`, `torin_candidate.png`, `torin_ingame_overhead.png`, `torin_ingame_chase.png`
- **Mira:** `mira_source.png`, `mira_candidate.png`, `mira_ingame_overhead.png`, `mira_ingame_chase.png`
- **Vance:** `vance_source.png`, `vance_candidate.png`, `vance_ingame_overhead.png`, `vance_ingame_chase.png`
- **Vex:** `vex_source.png`, `vex_candidate.png`, `vex_ingame_overhead.png`, `vex_ingame_chase.png`

## Requested Exceptions

1. **Open-shell / non-watertight mesh** (`check_watertight.py` FAIL on all 4): standard for TRELLIS.2 decimated + `--cut-base` open-bottom miniature figures (same approved exception as Batches 1–8).
2. **GLB size > 600 KB** (`1.9–2.3 MB` at 38k–42k tris): preserves fine facial features, beards, clothing trim, and props without texture smearing (same approved exception as Batches 7–8).
3. **Vance close-zoom caveat:** UV-preserving decimation at `--cut-z 0.068` restored Vance's rear boot cleanly, leaving minor close-zoom beard seam speckling and a slightly blunted sword tip (reads cleanly at gameplay distance in Oakhaven).

## Independent Adversarial Review Summary (2026-11-23)

- **Mira (`fig_npc_mira.glb`) — `SHIP-WITH-EXCEPTION`** (Round 1; zero blockers).
- **Torin (`fig_npc_torin.glb`) — `SHIP-WITH-EXCEPTION`** (Round 2 after `--cut-z 0.050` restored his rounded right boot toe; zero blockers).
- **Vex (`fig_npc_vex.glb`) — Round-2 rebake blotch fixed via UV-preserving decimation + `0.80/1.35` lift** (front flap & cloak now 1:1 clean vs `vex_source.png`, reads clearly in West Gate shadow).
- **Vance (`fig_npc_vance.glb`) — Round-2 `REJECT` on close-zoom beard speckling & blunted sword tip** (rear boot fixed; reads cleanly in-game; surfaced for Garth's ship-with-waiver vs. hold/re-sculpt call).

## Verification

- `cd rust && cargo test`: **107/107 passed**
- `/Users/gerth/Projects/gamedev-pipeline/tools/godot_verify.sh godot`: **ALL GREEN** (`import`, `parse (6 scripts)`, `playtest_check.gd`, `run1_check.gd`)

