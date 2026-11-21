# ElfStory Reborn (Godot 4 + Rust) — Roadmap

Direction is locked in `PROJECT_TRUTH.md` (Combined Model: Dark Cloud majors + ActRaiser growth, modern stylized 3D indie RPG aesthetic, desktop standalone first).

## Milestone 0 — Repo Bootstrap & Toolchain (DONE 2026-11-20)
- [x] Install Rust toolchain (`rustup` / `rustc` / `cargo`) on Apple Silicon Mac alongside Godot 4.7.2 (`/Applications/Godot.app`).
- [x] Create `Garthnesss/Elfstory-godot` with canonical docs (`PROJECT_TRUTH.md`, `AGENTS.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `STATUS.md`, `HANDOFF.md`), Nov 19 audit archive, V2 Blender/ComfyUI pipeline tools, V2 `.glb` models, and portraits.
- [x] Scaffold Cargo workspace (`rust/elfstory-core`, `rust/elfstory-gdext`) and minimal `godot/project.godot` + `elfstory.gdextension`.

## Milestone 1 — Vertical Slice Core in Rust (`elfstory-core`)
Port the vertical slice from `/Users/gerth/Projects/elfstoryreborn/src/game/` into pure, exhaustively-tested Rust:
- [x] `rng.rs` — Seeded PRNG (`mulberry32`) with deterministic tests.
- [x] `types.rs` — Core data types (`Tile`, `ZoneMap`, `PlayerState`, `Enemy`, `Item`, `WorldFlags`, `TownState`).
- [x] `town.rs` — Full Milestone 1 Reborn town loop: Blacksmith pieces (`anvil`, `bellows`, `sign`), `heartcoal` stall freeze, 3 lots (`pondside` / `market` / `west_gate`), free `relocate_blacksmith`, alive-return `bank_carried_town_items`, death `split_unbanked_town_items`, Turn-0/death/stall return-tick guards, `population_tier` bonuses, and regional side-dungeon supplies (`barrow_iron`, `mire_timber`, `frost_coal`).
- [x] `loot.rs` & `skills.rs` — Retuned Nov 16 rarity curve, starter/quest/town items, `compute_derived_stats` (including Town Tier bonuses & 2-piece relic sets).
- [x] `world.rs` — Hand-authored `create_oakhaven_map` (with Torin at `(7, 4)`) + procedural `generate_dungeon_floor(1)` (with guaranteed 4 town items and connectivity check).
- [x] `combat.rs` — Shadowcasting FOV, corner-safe A* pathfinding, telegraphed enemy intents, hit math.
- [x] `save.rs` — Versioned JSON save slots (`SAVE_VERSION = 1`) with per-slot corruption isolation.
- **Gate:** `cd rust && cargo test` 100% green covering all town exploit guards, reachability, and combat math.

## Milestone 2 — Godot 4 + `gdext` Vertical Slice (Playable 3D Oakhaven + Floor 1)
- [x] Wire `elfstory-gdext` (`ElfStorySession`) exposing map tiles, entities, player actions, dialogue, merchant, and town commands to Godot.
- [x] Build `godot/scenes/Oakhaven3D.tscn` & `Dungeon3D.tscn`:
  - `WorldEnvironment` (ACES tonemapping, warm afternoon sun, soft shadows, SSAO, subtle bloom, sky IBL).
  - `MultiMeshInstance3D` batched terrain + animated stylized water shader for Oakhaven Pond (eliminating web grid seams).
  - Mount V2 `.glb` landmarks (`blacksmith.glb`, `mira_apothecary.glb`, `oakhaven_oak.glb`, `oakhaven_platypus.glb`) with `hide_terrain_footprint` + Tier 1–3 ActRaiser growth structures.
  - Overhead Tactical + Chase 3rd-Person camera rig (`V` cycles, `[` / `]` quarter-turn yaw, smooth follow).
- [x] Native Godot `Control` UI: Top HUD, Action Dock, `TownPanel` (with free lot relocation & Regional Town Supplies), `DialogueModal`, `MerchantModal` (routing Torin & rebuilt lot directly to rebuilt Blacksmith stock), and `InventoryModal`.
- **Gate:** Playable end-to-end Run 1 in Godot: Oakhaven → Floor 1 Reliquary → collect 3 pieces + Heartcoal → return alive to bank → rebuild Blacksmith on chosen lot → buy `Oakhaven-Forged Blade` → verify Torin & lot open rebuilt shop.

## Milestone 3 — Full Content Port (Overworld, Side Dungeons, Bosses & Polish)
- [x] Port `overworld`, `highspire`, `thornwick`, `cinderwatch`, `dungeon_2..5`, `barrow_1..2`, `mire_1..2`, `frost_1..3`, and Endless Abyss (`dungeon_6+`) with regional town supplies on `barrow_1` / `mire_1` / `frost_1`.
- [x] Port all 18 NPCs, comedy-tone dialogue/barks (`garthVoice`), 10 quests, set-piece rooms, 17 enemy archetypes, companions, difficulty tiers, Hardcore, NG+ ascension (preserving `blacksmith` stock), and gamepad navigation.
- [x] World session behaviors: vault door, Seal Brazier hall, waystone attunement, chests (ordinary / vault / boss hoard / satchel), restorative braziers (Nov 21).
- [ ] Skills in combat (skill behaviors on top of the ported combat core).
- [ ] Native macOS `.app` export — no export preset exists yet; the game only runs inside Godot.
- [ ] Remaining procedural boxes in Oakhaven (cottages/stalls) — style follows the Nov 21 textured-source direction in `tools/ASSET_GATE.md`, not flat Track A.
