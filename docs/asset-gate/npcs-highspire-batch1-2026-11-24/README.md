# Asset Gate Pack — Town NPCs Highspire Batch 1 (`lyra`, `corvus`, `kaelen`) (2026-11-24)

Highspire Batch 1 replaces 3 of 5 Highspire Citadel NPCs (**Archivist Lyra**, **Magistrate Corvus**, **Guildmaster Kaelen**) with portrait-matched miniature figures wired via `NPC_FIGURES` in `godot/Main.gd`.

## Figures in This Batch

| NPC ID | Name & Role (Highspire Tile) | Shipped GLB | Raw Tris → Shipped Tris | GLB Size | `FIGURE_HEIGHT` | `check_watertight.py` |
|---|---|---|---|---|---|---|
| `lyra` | Archivist Lyra, Keeper of Glyphs `(14, 2)` | `godot/assets/models/figures/fig_npc_lyra.glb` | 191,632 → 42,000 (21.9%, UV-preserving + WebP albedo lift `0.85/1.25`) | 2.6 MB | `1.26` | FAIL (`boundary=35009 nonmanifold=326 degenerate=0`) |
| `corvus` | Magistrate Corvus, Citadel Arbiter `(5, 6)` | `godot/assets/models/figures/fig_npc_corvus.glb` | 131,207 → 37,999 (29.0%, `--rebake --tex-size 2048 --cut-z 0.068` + albedo lift `0.85/1.25`) | 1.8 MB | `1.30` | FAIL (`boundary=3478 nonmanifold=340 degenerate=2`) |
| `kaelen` | Guildmaster Kaelen, Warden of the Descent `(23, 6)` | `godot/assets/models/figures/fig_npc_kaelen.glb` | 79,054 → 42,000 (53.1%, UV-preserving `--cut-z 0.072` + WebP albedo lift `0.85/1.25`) | 2.8 MB | `1.34` | FAIL (`boundary=27724 nonmanifold=496 degenerate=2`) |

## Pipeline & Sources

- **Concepts:** DreamShaper XL Turbo v2.1 (`DreamShaperXL_Turbo_v2_1.safetensors`, `--steps 6 --cfg 2.0`, delighting off), matched to official portraits in `godot/assets/portraits/`:
  - `docs/concepts/figures/fig_npc_lyra_ds_s22.png`
  - `docs/concepts/figures/fig_npc_corvus_ds_s22.png`
  - `docs/concepts/figures/fig_npc_kaelen_ds_s22.png`
- **Cutouts:** BiRefNet (`rembg i -m birefnet-general`) in `docs/concepts/figures/cutouts/fig_npc_{lyra,corvus,kaelen}.png`
- **Sculpts (TRELLIS.2 via Elf Works `/foundry` & `genq.py`):**
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_npc_lyra-20261124-090920.glb` (406s)
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_npc_corvus-20261124-091645.glb` (437s)
  - `/Users/gerth/Projects/GLocalLLM/workspace/3d/fig_npc_kaelen-20261124-092435.glb` (459s)
- **Conversion:** `tools/blender_decimate_source.py --target-tris 38000 --rebake --tex-size 2048` (`--cut-base` on `corvus` & `kaelen`, no base cut needed on `lyra`; gentle `glb_albedo_lift.py 0.85 1.25` on `lyra` & `corvus` for Highspire stone alcove readability).

## Evidence Files

- **Lyra:** `lyra_source.png`, `lyra_candidate.png`, `lyra_ingame_overhead.png`, `lyra_ingame_chase.png`
- **Corvus:** `corvus_source.png`, `corvus_candidate.png`, `corvus_ingame_overhead.png`, `corvus_ingame_chase.png`
- **Kaelen:** `kaelen_source.png`, `kaelen_candidate.png`, `kaelen_ingame_overhead.png`, `kaelen_ingame_chase.png`

## Requested Exceptions & Round-2 Fix Summary

1. **Open-shell / non-watertight mesh** (`check_watertight.py` FAIL on all 3): standard for TRELLIS.2 decimated miniature figures.
2. **GLB size > 600 KB** (`1.8–2.8 MB` at 38k–42k tris): preserves fine facial features, embroidery, gold collars, and weapon edges.
3. **Round-1 Adversarial Review → Round-2 Fixes Applied:**
   - **Lyra:** Switched to UV-preserving decimation (`42k` tris) + WebP albedo lift `0.85/1.25` — cleared blackened satchel and outer cape smudge cleanly.
   - **Corvus:** Lowered `--cut-z` from `0.088` → `0.068` — restored full right boot cleanly with zero base debris.
   - **Kaelen:** Switched to UV-preserving decimation (`42k` tris) + `--cut-z 0.072` + WebP albedo lift `0.85/1.25` — restored both boots, cleared armor/pouch/cape rebake smears, and lifted silver/crimson readability in Highspire.

## Verification

- `cd rust && cargo test`: **107/107 passed**
- `/Users/gerth/Projects/gamedev-pipeline/tools/godot_verify.sh godot`: **ALL GREEN** (`import`, `parse (6 scripts)`, `playtest_check.gd`, `run1_check.gd`)
