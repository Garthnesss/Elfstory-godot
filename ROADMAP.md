# ElfStory Reborn (Godot 4 + Rust) — Roadmap

Current direction is in `PROJECT_TRUTH.md` (Combined Model: Dark Cloud majors + ActRaiser growth, modern stylized 3D indie RPG aesthetic, desktop standalone first).

**Scope is still forming (Garth, Nov 21, 2026).** The full shape of the game isn't decided yet and will change as we build, until Garth says it's settled. Treat this roadmap as the current plan, not a contract: when Garth makes a new call, update the affected docs with a dated note rather than defending the old plan.

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
- [x] Skills in combat (Nov 21–22): 8 actives + passives + prodigy hooks, aiming/hotbar/learning, TS enemy turn (statuses, DoTs, dodge, intents), Spirit Companions (Silverwind Lynx / Starlight Owl), Grave Husk Deathless & Spore Hulk burst, boss confrontation prompts (Vaelen / Serath / Malakor), and haste steps.
- [x] Native macOS `.app` export (Nov 21): `tools/build_app.sh` → `build/ElfStory.app` (universal, ad-hoc signed, not notarized).
- [ ] Remaining procedural boxes in Oakhaven (cottages/stalls) — style follows the Nov 21 textured-source direction in `tools/ASSET_GATE.md`, not flat Track A.

## Presentation pass — Godot Asset Library add-ons (picked Nov 21, 2026)
Full survey and reasoning: `docs/godot-asset-library-shortlist.md`. Every visual add-on goes through `tools/ASSET_GATE.md` and Garth's approval before commit.
- [ ] Combat feel (picked first): REEBOO Stylized VFX sample (hit spark / slash / pickup sparkle), Kenney Particle Pack, Kenney RPG Audio + Interface Sounds. Needs a presentation event feed from the Rust core (hits, kills, loot, doors) since the snapshot alone doesn't say what just happened.
- [ ] Later candidates: grass/foliage (Simple Grass Textured or Open Stylized 3D grass), Debug Draw 3D (dev), Phantom Camera (polish).
- Skipped for now: Sky3D (fights golden hour), dialogue add-ons (dialogue lives in Rust), Terrain3D (tile grid), flat low-poly model packs (style rejected).


## Next queue (Garth, Nov 21, 2026 — start next session)
Garth picked all four; take them in this order unless he says otherwise:
1. [x] **Grass & foliage** — done Nov 21: in-house meadow grass + wildflowers (`godot/grass.gd`, `grass.gdshader`) over Oakhaven + Silverwood Vale floor tiles; no add-on needed. Approved by Garth after two review rounds.
2. [x] **Platypus feet** — done Nov 21: rebuilt from the TRELLIS source (welded 25% decimation + Smart-UV re-bake, `tools/blender_decimate_source.py --rebake`); toes, webbing and claws restored. Garth shipped it with the watertight exception.
3. [x] **Title & save slots** — done Nov 22: full-screen title (`godot/title_screen.gd`) over crypt backdrop, 3 independent save slots with Continue / two-click Delete & overwrite confirmation, starting discipline + difficulty choice (Story / Standard / Hardened via `new_game_with_difficulty`), and app icon (`godot/assets/icon/elfstory_icon.png`).
4. [x] **Town loop feel** — done Nov 22: cold forge ruin at `(7, 2)` and marked build lots before completion, rebuilt `blacksmith.glb` + build/tier-up celebration VFX/SFX on chosen lot, visible Tier 1–3 ActRaiser growth structures (gardens, settler cottages, arbor, platypus, grove), regional supply shop refresh on banking, and upgraded `TownPanel`.
Also open: Garth's playtest notes from the app (all 4 combat gaps closed Nov 22).

## Retired / Dormant Systems (Nov 24, 2026 — Oakhaven Pitch C Redesign)
Town-rebuilding UI in Oakhaven is retired/hidden (`godot/Main.gd` `ENABLE_TOWN_REBUILD_UI := false`) while keeping the underlying Rust implementation intact so it can be restored or deleted cleanly later:
- `godot/Main.gd`: `ENABLE_TOWN_REBUILD_UI`, `rebuild_town_panel()`, `build_lot_markers()`
- `rust/elfstory-core/src/town.rs` & `rust/elfstory-core/src/town_layout.rs` (rebuild pieces, Heartcoal stall, 3 lots, ActRaiser growth placements, regional supplies)
- `rust/elfstory-core/src/session.rs`: `complete_blacksmith`, `relocate_blacksmith`, `return_to_oakhaven` banking/ticking
