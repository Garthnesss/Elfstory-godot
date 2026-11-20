# ElfStory Reborn (Godot 4 + Rust) — Status

**2026-11-20 — Milestone 0 (Bootstrap & Handoff) Complete:**
- Created `Garthnesss/Elfstory-godot` at `/Users/gerth/Projects/elfstory-godot` as the standalone desktop successor to `/Users/gerth/Projects/elfstoryreborn`.
- Installed Rust toolchain (`rustup` / `rustc` / `cargo` on Apple Silicon `aarch64-apple-darwin`) alongside local Godot **4.7.2** (`/Applications/Godot.app`).
- Copied all canonical project truth, Nov 19 audit report, V2 Blender/ComfyUI pipeline tools (`tools/blender_arch_kit.py`, `tools/blender_retopo_bake.py`, `tools/comfy_runner.py`), `.agents/skills/oakhaven-asset-pipeline/SKILL.md`, all 4 V2-verified `.glb` models (`blacksmith.glb` `334 KB`, `mira_apothecary.glb` `391 KB`, `oakhaven_oak.glb` `350 KB`, `oakhaven_platypus.glb` `303 KB`), and all 14 portrait/backdrop `.jpg` assets.
- Authored `AGENTS.md`, `ARCHITECTURE.md`, `ROADMAP.md`, and `HANDOFF.md` for seamless pickup by Muse Code or any adjacent AI agent.

## Next Action
- Start **Milestone 1**: implement `rust/elfstory-core` (`rng`, `types`, `town`, `loot`, `skills`, `world`, `combat`, `save`) with `cargo test` green, then wire `rust/elfstory-gdext` to `godot/`.

## Milestone 1 — elfstory-core port (Nov 20, 2026, Muse + parallel agents)
- Ported from committed elfstoryreborn  (133/133 TS tests): types, run_rules, difficulty, town, town_layout, loot, skills, world (Oakhaven + Floor 1 slice), combat, save.
-  on Mac: **58/58 passed**. All 10 HANDOFF guardrails covered by named tests (Torin (7,4), lot footprints, banking/death drops, return-tick guards, tier bonuses, NG+ stock, loot curve, seeded determinism).
- Known M3 stubs in world.rs: other towns, side dungeons, vault + non-sepulcher set pieces, Abyss echo boss. Merchant stock builder lives in npcs.ts (M3) — loot.rs provides its item constructors.
- Next: Milestone 2 — gdext wiring (verify godot crate version against Godot 4.7.2 first) + playable Oakhaven/Floor 1 slice.

## Milestone 2 — Godot + gdext playable slice (Nov 20, 2026, Muse + agent)
- Bridge: godot crate **0.5 + api-4-7** (verified against Godot 4.7.2; HANDOFF api-4-3 suggestion superseded).  exposes  (JSON snapshot + intent methods);  builds clean.
- Core glue: new  — Run 1 turn loop (bump combat with TS damage formula, enemy turns/telegraphs, loot + first-gear guarantee, stairs transitions, alive-return banking + return tick, death Wake with unbanked drops, blacksmith complete/relocate, minimal rebuilt stock incl. the gated blade). Full gate flow proven by Rust test ; **cargo test 59/59 on Mac**.
- Godot front-end: Main.tscn/Main.gd — MultiMesh terrain with .glb footprint hiding, V2 landmarks (placeholder boxes when a model is absent), animated water plane over the pond, ACES + SSAO + glow environment, warm sun shadows, Overhead Tactical + Chase camera (V, [ ] yaw), HUD/log/TownPanel/Dialogue/Merchant UI.
- Verified headless on Mac: editor import clean,  -> RUN1_CHECK_OK (bridge live in-engine), Main scene runs 90 frames with no script errors. **NOT yet verified visually** — Garth opening the project in Godot is the visual gate.
- Slice simplifications (M3): skills/status-DoTs in combat, full merchant tables + NPC dialogue trees (npcs.ts), side-dungeon generators, gold death toll.

## Milestone 3 (part 1) — npcs.ts port: full merchant tables + dialogue trees (Nov 20, 2026)
- New `npcs.rs`: all **18 NPC defs** (13 quest/merchant + 5 ambient townsfolk), **60 dialogue nodes / 111 choices** ported verbatim (411 TS string literals + 122 colors audited byte-for-byte), 10 quest defs, 17 lore entries, `create_merchant_stock` (mira/vex/bram/halda/orin + supplies-gated blacksmith + Torin default), `is_quest_active`. Data-integrity test: every root node exists and every choice route resolves.
- New dialogue types in types.rs (DialogueChoice/Node/Action/Condition/SkillCheck, NpcDef).
- Session dialogue engine mirroring App.tsx: condition-gated choices (incl. the questStatus-undefined = not-started convention), attribute checks routing to fail nodes with no actions, all 14 action types (heal_full, start/advance/complete quest with the pay-once guard + relic/gold/XP rewards, set_flag against typed WorldFlags + overflow map, respec via compute_respec_points, companion swap with TS hp formulas, unlock_highspire, gold give/spend, consume quest item, grant buff).
- Merchant resolution now matches TS: Torin's trade resolves to the rebuilt 'blacksmith' stock once complete; stocks seed lazily from npcs.rs (the M2 hand-built minimal stock is gone).
- Bridge + Godot: `dialogue_json` / `choose_dialogue` / `close_dialogue`; DialoguePanel renders real nodes with choice buttons; movement gated while dialogue is open.
- Verified: **cargo test 69/69 on Mac**, gdext builds, headless RUN1_CHECK_OK (incl. new dialogue bridge smoke), Main scene 90 frames no errors. Visual/dialogue-by-eye check still Garth's gate in the editor.
