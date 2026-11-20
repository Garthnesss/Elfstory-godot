# AGENTS.md

Muse Code, Claude Code, Codex, and Antigravity read this file as project rules when running in `/Users/gerth/Projects/elfstory-godot`.

## Stack & Commands

- **Engine**: Godot 4.7.2 (`/Applications/Godot.app/Contents/MacOS/Godot`, Forward+ / Metal on macOS)
- **Simulation / Core Logic**: Rust (`rust/elfstory-core`, pure Rust crate, zero Godot dependency) + thin GDExtension bridge (`rust/elfstory-gdext` via `godot` / `gdext`)
- **Reference Implementation**: `/Users/gerth/Projects/elfstoryreborn` (TypeScript + Three.js prototype, 133/133 unit tests passing, Milestones 1–3 landed Nov 20, 2026)
- **Rust Test**: `cd rust && cargo test`
- **Rust Build (GDExtension)**: `cd rust && cargo build` (or `--release`)
- **Godot Launch**: `/Applications/Godot.app/Contents/MacOS/Godot --path godot`
- **Godot Headless Check**: `/Applications/Godot.app/Contents/MacOS/Godot --headless --path godot --quit`

## Project Grounding (Read First)

1. `PROJECT_TRUTH.md` — Locked Combined Model roguelite + town loop (Dark Cloud majors + ActRaiser growth), art direction, and Nov 19–20 guardrails.
2. `HANDOFF.md` — Complete architectural brief, exact TS-to-Rust module port map, Vertical Slice 1 checklist, and lessons learned from the web version.
3. `ARCHITECTURE.md` — Crate layout (`elfstory-core` vs. `elfstory-gdext`), Godot scene/UI layout, and save format.
4. `ROADMAP.md` & `STATUS.md` — Sequenced milestones and current verified state.
5. `docs/elfstory-reborn-audit-2026-11-19.md` — Full 5-pillar design/tech/asset audit of the web prototype so we never re-introduce those bugs.

## Hardware & Local AI Stack Rules (Mandatory)

- **Mac-only** (Apple Silicon, 32 GB unified memory) — there is no Windows PC.
- **32 GB Memory Rule**: Never run ComfyUI (`~/ComfyUI`, port `8188`), TRELLIS.2 (`http://127.0.0.1:8765/foundry`), ACE-Step Music Studio, and Ollama simultaneously. Always let `tools/comfy_runner.py` and `~/GLocalLM/ops/foundry.py` arbitrate `AGENT` ↔ `STUDIO` mode automatically.
- **Observable 3D Generation**: Always run TRELLIS.2 jobs through **Elf Works `/foundry`** (`http://127.0.0.1:8765/foundry`) so live progress and **RECENT 3D MODELS** are visible — never invoke `generate.sh` directly in a hidden terminal.

## V2 Studio-Grade Hybrid 3D Game Asset Workflow (`oakhaven-asset-pipeline`)

Before creating or modifying any 3D building, prop, landmark, texture, creature, or character, read `.agents/skills/oakhaven-asset-pipeline/SKILL.md`:

1. **Track A — Architecture & Hard-Surface Props** *(No TRELLIS.2)*: `tools/comfy_runner.py --mode pbr-texture` (1024px seamless PBR) + `tools/blender_arch_kit.py` (`1,500–4,000` clean quads, `< 500 KB` `.glb`, bottom-center `Z = 0`, beveled edges + weighted normals) → export into `godot/assets/models/`.
2. **Track B — Organic Creatures, Trees, Rocks & Statues**: `tools/comfy_runner.py --mode concept` (1024px flat-diffuse delighted) → Elf Works `/foundry` → `tools/blender_retopo_bake.py` (QuadriFlow `~2,500` pure quads + Shrinkwrap + Smart UV + Cycles 1024px Diffuse/Normal/AO bake + Weighted Normals, `< 600 KB` `.glb`) → export into `godot/assets/models/`.

## Non-Negotiable Architecture Rules

- **All game rules live in `rust/elfstory-core`**, never in GDScript or `elfstory-gdext`. Every rule must be unit-tested via `cargo test` without booting Godot.
- **Use Rust's type system to prevent bugs**: exhaustive `match` on `TileKind`, `EnemyArchetype`, `TownSupplyTag`, `BlacksmithLot`, `DialogueAction`, `SkillId`; seeded PRNG (`elfstory-core::rng`) everywhere — never unseeded global RNG.
- **Godot owns presentation only**: 3D rendering (Forward+ / Metal, soft shadows, SSAO, bloom, animated water shader, `.glb` mounting), skeletal animations, Web/native audio, and native `Control` RPG UI (HUD, Pack, Merchant, Dialogue, `TownPanel`).
- **Keep the town loop physical, never a detached menu**: Rebuilt majors are interactive in-world in Oakhaven, lots are movable for free, town items bank only on alive return (dropping unbanked items on death), stalls freeze growth only, and side dungeons hold zone-specific town supplies.
