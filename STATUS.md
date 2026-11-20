# ElfStory Reborn (Godot 4 + Rust) — Status

**2026-11-20 — Milestone 0 (Bootstrap & Handoff) Complete:**
- Created `Garthnesss/Elfstory-godot` at `/Users/gerth/Projects/elfstory-godot` as the standalone desktop successor to `/Users/gerth/Projects/elfstoryreborn`.
- Installed Rust toolchain (`rustup` / `rustc` / `cargo` on Apple Silicon `aarch64-apple-darwin`) alongside local Godot **4.7.2** (`/Applications/Godot.app`).
- Copied all canonical project truth, Nov 19 audit report, V2 Blender/ComfyUI pipeline tools (`tools/blender_arch_kit.py`, `tools/blender_retopo_bake.py`, `tools/comfy_runner.py`), `.agents/skills/oakhaven-asset-pipeline/SKILL.md`, all 4 V2-verified `.glb` models (`blacksmith.glb` `334 KB`, `mira_apothecary.glb` `391 KB`, `oakhaven_oak.glb` `350 KB`, `oakhaven_platypus.glb` `303 KB`), and all 14 portrait/backdrop `.jpg` assets.
- Authored `AGENTS.md`, `ARCHITECTURE.md`, `ROADMAP.md`, and `HANDOFF.md` for seamless pickup by Muse Code or any adjacent AI agent.

## Next Action
- Start **Milestone 1**: implement `rust/elfstory-core` (`rng`, `types`, `town`, `loot`, `skills`, `world`, `combat`, `save`) with `cargo test` green, then wire `rust/elfstory-gdext` to `godot/`.
