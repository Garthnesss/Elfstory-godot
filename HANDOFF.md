# ElfStory Reborn (Godot 4 + Rust) — Agent Handoff Brief

**Date:** 2026-11-20  
**Audience:** Muse Code / Claude Code / Codex / Antigravity picking up `Garthnesss/Elfstory-godot`  
**Reference Web Prototype:** `/Users/gerth/Projects/elfstoryreborn` (133/133 unit tests passing, Milestones 1–3 landed and browser-playtested Nov 20, 2026)

---

## 1. Why We Are Moving to Godot 4 + Rust (`gdext`)
Garth wants ElfStory Reborn to be a **standalone desktop 3D indie RPG** (macOS `.app` first, Windows/Linux/Steam Deck later) with a **modern stylized 3D aesthetic** (*Tunic* / *Link's Awakening*: soft shadows, SSAO, bloom, animated water shader, skeletal character animations) and **Rust's compile-time bug prevention** for game state and simulation rules.

Our agreed architecture:
- **`rust/elfstory-core`** — Pure Rust crate (zero Godot dependency). Owns 100% of game rules, turn loop, town loop, procedural generation, loot, combat math, seeded RNG, and saves. Tested headlessly via `cargo test`.
- **`rust/elfstory-gdext`** — Thin GDExtension bridge (`godot` crate / `gdext`) exposing `ElfStorySession` to Godot 4.7.2.
- **`godot/`** — Godot 4.7.2 (Forward+ / Metal) handling 3D rendering, lighting, shaders, animations, audio, and native `Control` RPG UI.

---

## 2. Local Environment (Mac-Only Apple Silicon, 32 GB Unified Memory)
- **Godot:** `/Applications/Godot.app/Contents/MacOS/Godot` (**4.7.2.stable.official**)
- **Rust:** `~/.cargo/bin/rustc` & `~/.cargo/bin/cargo` (`aarch64-apple-darwin`)
- **Blender:** `/Applications/Blender.app/Contents/MacOS/Blender` (**5.3.0**)
- **Local AI Stack (32 GB Memory Rule):** Never run ComfyUI (`~/ComfyUI`, port `8188`), TRELLIS.2 (always via Elf Works `/foundry` at `http://127.0.0.1:8765/foundry`), ACE-Step, and Ollama simultaneously.

---

## 3. Already Copied Into This Repo (Ready to Use)
- **Docs:** `PROJECT_TRUTH.md`, `AGENTS.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `STATUS.md`, `docs/elfstory-reborn-audit-2026-11-19.md`
- **V2 3D Asset Pipeline:** `.agents/skills/oakhaven-asset-pipeline/SKILL.md`, `tools/blender_arch_kit.py`, `tools/blender_retopo_bake.py`, `tools/comfy_runner.py`
- **V2 Game-Ready 3D Models (`godot/assets/models/`):**
  - `blacksmith.glb` (`334 KB`, hand-built stylized low-poly)
  - `mira_apothecary.glb` (`391 KB`, V2 Track A, `3,474` quads — note: roof albedo is slightly dark green, can brighten if desired)
  - `oakhaven_oak.glb` (`350 KB`, V2 Track B QuadriFlow + Cycles bake, `1,384` pure quads, 1024px WebP Albedo+AO + Normal)
  - `oakhaven_platypus.glb` (`303 KB`, V2 Track B, `2,359` pure quads, 1024px WebP Albedo+AO + Normal)
- **Portraits & Title Backdrop (`godot/assets/portraits/`):** All 13 NPC portraits + `title_crypt_backdrop`.

---

## 4. Critical Design & Bug Guardrails (From Nov 19–20 Audit — Do Not Regress)
Every single one of these was fixed in Milestone 1–3 of `/Users/gerth/Projects/elfstoryreborn` and MUST be preserved in the Rust/Godot rewrite:
1. **Torin Position:** Torin spawns at `(7, 4)` outside his forge door in Oakhaven (never `(7, 2)` inside `blacksmith.glb`).
2. **Lot Footprints:** All 3 Blacksmith lots (`pondside`, `market`, `west_gate`) carry explicit `hide_terrain_footprint` rects so the rebuilt Blacksmith never clips cottages, stalls, or trees.
3. **In-World Physical Shop & Free Relocation:** Talking to Torin or interacting adjacent to the rebuilt lot opens the rebuilt `blacksmith` shop directly once `blacksmith_complete == true`. Relocating lots later is free (`relocate_blacksmith`) and pays the one-time `+75g` pondside wish bonus if moved to `pondside` for the first time.
4. **True Banking & Death Drops:** Town items picked up in dungeons stay unbanked in `player.inventory` until an **alive return to Oakhaven** (`bank_carried_town_items`), which records them into `TownState` and removes them from the Pack. On `"Wake in Oakhaven"` death, unbanked carried town items drop back onto the dungeon floor where the player fell (`split_unbanked_town_items`).
5. **Anti-Cheese Return Tick & Stall Freeze:** `town_return_tick` refuses empty Turn-0 stair-hops (`delve_progress <= 0`), death returns (`is_death_return == true`), and active stalls (`!stall_item_found`).
6. **Population Tier Payoff & Zone-Choice Incentives:** `population_tier(town)` (`[20, 35, 55]`) grants `+4 Max HP, +1 ATK, +1 Spell Power` per tier (`+1/+2 DEF` at Tiers 2/3) in `compute_derived_stats`, mounts visible ActRaiser growth structures at Tiers 1–3, and Floor 1 of the 3 side-dungeon branches spawns regional supplies (`barrow_1 → barrow_iron`, `mire_1 → mire_timber`, `frost_1 → frost_coal`, `+4 Pop` each + unlocks `Barrow-Iron Hauberk`, `Mire-Timber Greatbow`, `Frost-Quenched Greathelm` in Torin's rebuilt Blacksmith).
7. **NG+ Stock Persistence:** Ascending to NG+ preserves `merchant_stocks.blacksmith` when `town.blacksmith_complete` is true.
8. **Cutaway Never Un-Hides `.glb` Footprints:** Tiles under a `.glb` footprint stay hidden across camera cutaway toggles.
9. **Retuned Loot Curve (Nov 16):** Depth term `× 5` (cap `+50`), Rare `> 75`, Epic `> 104`, Relic `> 125` (premium-only; ordinary drops cap at Epic), ordinary kill drop chance `30%`, first regular kill for Lv `<= 3` guarantees equipment.
10. **Seeded PRNG Everywhere:** Thread `elfstory_core::rng::Rng` through `world` and `loot` from day one.

---

## 5. Exact Reference Module Map (TS → Rust)
When porting from `/Users/gerth/Projects/elfstoryreborn/src/game/`:
- `rng.ts` → `rust/elfstory-core/src/rng.rs`
- `types.ts` → `rust/elfstory-core/src/types.rs`
- `town.ts` → `rust/elfstory-core/src/town.rs`
- `skills.ts` → `rust/elfstory-core/src/skills.rs`
- `loot.ts` → `rust/elfstory-core/src/loot.rs`
- `world.ts` → `rust/elfstory-core/src/world.rs`
- `combat.ts` → `rust/elfstory-core/src/combat.rs`
- `npcs.ts` → `rust/elfstory-core/src/npcs.rs`
- `difficulty.ts` → `rust/elfstory-core/src/difficulty.rs`
- `runRules.ts` → `rust/elfstory-core/src/run_rules.rs`
- `saveSlots.ts` → `rust/elfstory-core/src/save.rs`
- `modelLoader.ts` (anchors & placement derivation) → `rust/elfstory-core/src/town_layout.rs`

---

## 6. First Steps for Next Agent (Vertical Slice 1)
1. Verify Rust & Godot locally:
   ```bash
   export PATH="$HOME/.cargo/bin:$PATH"
   rustc --version && cargo --version
   /Applications/Godot.app/Contents/MacOS/Godot --version
   ```
2. Implement `rust/elfstory-core` Vertical Slice (`rng`, `types`, `town`, `loot`, `skills`, `world` Oakhaven + Floor 1, `combat`, `save`) and run `cd rust && cargo test`.
3. Wire `rust/elfstory-gdext` (using `godot = { version = "0.2", features = ["api-4-3"] }` or latest `godot` crate compatible with Godot 4.7.2) and verify `cargo build` produces `target/debug/libelfstory_gdext.dylib`.
4. Build the playable Godot 4 vertical slice (`Oakhaven3D` + `Dungeon3D` + `TownPanel` + `HUD` + `MerchantModal`) using the V2 `.glb` models in `godot/assets/models/`, a smooth animated water shader for Oakhaven Pond, and `WorldEnvironment` (ACES + SSAO + bloom).
