# AGENTS.md

Muse Code, Claude Code, Codex, and Antigravity read this file as project rules when running in `/Users/gerth/Projects/elfstory-godot`.

## Stack & Commands

- **Engine**: Godot 4.7.2 (`/Applications/Godot.app/Contents/MacOS/Godot`, Forward+ / Metal on macOS)
- **Simulation / Core Logic**: Rust (`rust/elfstory-core`, pure Rust crate, zero Godot dependency) + thin GDExtension bridge (`rust/elfstory-gdext` via `godot` / `gdext`)
- **Reference Implementation**: `/Users/gerth/Projects/elfstoryreborn` (TypeScript + Three.js prototype, 133/133 unit tests passing, Milestones 1–3 landed Nov 20, 2026)
- **Rust Test**: `cd rust && cargo test`
- **Rust Build (GDExtension)**: `cd rust && cargo build` (or `--release`)
- **Verify Gate**: `tools/pipeline/godot_verify.sh godot` (import, parse, `run1_check`, `playtest_check`, `ui_check`, asset audit, `cargo test`). Needs `cargo build` first.
- **Release Gate**: `tools/build_app.sh --smoke` before any merge that touches `Main.gd` or the bridge. Debug-only bridge calls must go through `session.call("debug_...")`: a direct call breaks the whole script in release builds and no debug check can see it (this shipped broken Nov 23 to 25).
- **Godot Launch**: `/Applications/Godot.app/Contents/MacOS/Godot --path godot`
- **Godot Headless Check**: `/Applications/Godot.app/Contents/MacOS/Godot --headless --path godot --quit`

## Project Grounding (Read First)

1. `PROJECT_TRUTH.md` — Locked Combined Model roguelite + town loop (Dark Cloud majors + ActRaiser growth), art direction, and Nov 19–20 guardrails.
2. `HANDOFF.md` — Complete architectural brief, exact TS-to-Rust module port map, Vertical Slice 1 checklist, and lessons learned from the web version.
3. `ARCHITECTURE.md` — Crate layout (`elfstory-core` vs. `elfstory-gdext`), Godot scene/UI layout, and save format.
4. `ROADMAP.md` & `STATUS.md` — Sequenced milestones and current verified state.
5. `docs/elfstory-reborn-audit-2026-11-19.md` — Full 5-pillar design/tech/asset audit of the web prototype so we never re-introduce those bugs.
6. **Shared game-dev pipeline (all of Garth's games):** read `tools/pipeline/AGENT_RULES.md` (vendored; never edit it here) before any Godot work, and start a session with `gdp next godot`. At the unplayed limit, no new features. This project's docs win on project-specific decisions (Rust core, asset overrides in `tools/ASSET_GATE.md`). Verify gate above.

## Hardware & Local AI Stack Rules (Mandatory)

- **Mac-only** (Apple Silicon, 32 GB unified memory) — there is no Windows PC.
- **32 GB Memory Rule**: Never run ComfyUI (`~/ComfyUI`, port `8188`), TRELLIS.2 (`http://127.0.0.1:8765/foundry`), ACE-Step Music Studio, and Ollama simultaneously. Always let `tools/comfy_runner.py` and `~/GLocalLM/ops/foundry.py` arbitrate `AGENT` ↔ `STUDIO` mode automatically.
- **Observable 3D Generation**: Always run TRELLIS.2 jobs through **Elf Works `/foundry`** (`http://127.0.0.1:8765/foundry`) so live progress and **RECENT 3D MODELS** are visible — never invoke `generate.sh` directly in a hidden terminal.
- **One generation job at a time, queued (Garth, Nov 22, 2026)**: before any model generation (ComfyUI concepts/textures, TRELLIS.2 sculpts), make sure no other AI or chat session is already generating, and queue behind it instead of starting in parallel. Use the machine-wide queue `/Users/gerth/Projects/elfstory-godot/tools/genq.py`: `genq.py status` to see what is running/queued, `genq.py foundry --image <abs cutout> --name <slug>` for TRELLIS.2 (submits to `/foundry` and holds the slot until the job finishes), and `genq.py run --label <what> -- <cmd>` for anything else. `tools/comfy_runner.py` queues itself automatically. The queue also waits while the foundry or ComfyUI is busy with a job started outside it, so never bypass it (no direct `/api/foundry` start, no clicking Generate on `/foundry` while `genq.py status` shows a job).

## V2 Studio-Grade Hybrid 3D Game Asset Workflow (`oakhaven-asset-pipeline`)

> **Current reference (Nov 21, 2026): `docs/asset-generation-pipeline.md`.** It adds the BiRefNet cutout step (`~/asset-tools/venv/bin/rembg`), the DreamShaper XL Turbo concept checkpoint, the UV-preserving decimation route, and the UniRig status (parked). Where it and the older text below disagree, it wins. ElfStory's buildings ship as textured TRELLIS originals at ~25% decimation (Garth, Nov 21), not the flat Track A kit.

Before creating or modifying any 3D building, prop, landmark, texture, creature, or character, read `.agents/skills/oakhaven-asset-pipeline/SKILL.md`:

1. **Track A — Architecture & Hard-Surface Props** *(No TRELLIS.2)*: `tools/comfy_runner.py --mode pbr-texture` (1024px seamless PBR) + `tools/blender_arch_kit.py` (`1,500–4,000` clean quads, `< 500 KB` `.glb`, bottom-center `Z = 0`, beveled edges + weighted normals) → export into `godot/assets/models/`.
2. **Track B — Organic Creatures, Figures & Landmarks**: `tools/comfy_runner.py --mode concept` (DreamShaper XL Turbo `DreamShaperXL_Turbo_v2_1.safetensors`, `--steps 6 --cfg 2.0`, delighting off) → BiRefNet cutout (`~/asset-tools/venv/bin/rembg i -m birefnet-general`) → `python3 tools/genq.py foundry` → `tools/blender_decimate_source.py --rebake --tex-size 2048` (add `--cut-base` for miniatures) + `tools/check_watertight.py` → export into `godot/assets/models/`.
3. **Asset Gate & Provenance**: Every shipped model requires source-vs-candidate + in-game captures + independent adversarial review in `docs/asset-gate/` (`tools/ASSET_GATE.md`), Garth's explicit approval (including open-shell/size exceptions), and a row in `docs/asset_provenance.csv`.

## Non-Negotiable Architecture Rules

- **All game rules live in `rust/elfstory-core`**, never in GDScript or `elfstory-gdext`. Every rule must be unit-tested via `cargo test` without booting Godot.
- **Use Rust's type system to prevent bugs**: exhaustive `match` on `TileKind`, `EnemyArchetype`, `TownSupplyTag`, `BlacksmithLot`, `DialogueAction`, `SkillId`; seeded PRNG (`elfstory-core::rng`) everywhere — never unseeded global RNG.
- **Godot owns presentation only**: 3D rendering (Forward+ / Metal, soft shadows, SSAO, bloom, animated water shader, `.glb` mounting), skeletal animations, Web/native audio, and native `Control` RPG UI (HUD, Pack, Merchant, Dialogue, `TownPanel`).
- **Keep the town loop physical, never a detached menu**: Rebuilt majors are interactive in-world in Oakhaven, lots are movable for free, town items bank only on alive return (dropping unbanked items on death), stalls freeze growth only, and side dungeons hold zone-specific town supplies.
