# ElfStory Reborn (Godot 4 + Rust) — Architecture

## Big Picture

Standalone native 3D roguelite + town-builder built with **Godot 4.7.2** (Forward+ / Metal renderer + native `Control` RPG UI) and **Rust** (`elfstory-core` simulation crate + `elfstory-gdext` GDExtension bridge).

```
godot/ (Godot 4.7.2 — Forward+ / Metal)
  ├─ scenes/           — Main, Oakhaven3D, Dungeon3D, TitleScreen, TownPanel, HUD, Pack, Merchant, Dialogue
  ├─ scripts/          — Thin GDScript presentation controllers calling ElfStoryBridge
  ├─ shaders/          — Stylized water, fog, outline/cutaway shaders
  ├─ assets/
  │   ├─ models/       — V2 studio-grade .glb assets (blacksmith, mira_apothecary, oakhaven_oak, oakhaven_platypus)
  │   ├─ portraits/    — 13 NPC portraits + title backdrop (.jpg)
  │   └─ textures/     — Seamless 1024px PBR textures (albedo/normal/roughness)
  └─ elfstory.gdextension — Points Godot to rust/target/{debug,release}/libelfstory_gdext.dylib

rust/ (Cargo Workspace)
  ├─ elfstory-core/    — Pure Rust game simulation (zero Godot dependency, fast `cargo test`)
  │   ├─ src/types.rs      — Tile, ZoneMap, PlayerState, Enemy, Item, WorldFlags, QuestProgress
  │   ├─ src/rng.rs        — Deterministic seeded PRNG (mulberry32 / splitmix64)
  │   ├─ src/town.rs       — Combined Model town loop (Blacksmith major, 3 lots, free relocate, banking, stall freeze, regional supplies, Population Tier bonuses)
  │   ├─ src/world.rs      — Hand-authored Oakhaven + procedural dungeon & side-dungeon generators
  │   ├─ src/combat.rs     — Shadowcasting FOV, corner-safe A* pathfinding, telegraphed intents, hit math
  │   ├─ src/skills.rs     — Attributes, 3 skill trees, capstones, `compute_derived_stats` (incl. Town Tier)
  │   ├─ src/loot.rs       — Procedural gear, retuned Nov 16 rarity curve, named relics, quest/town items
  │   ├─ src/npcs.rs       — Dialogue trees, quests, merchant stocks (incl. supply-gated Blacksmith stock)
  │   ├─ src/difficulty.rs — Story / Standard / Hardened + NG+ cycle scalers
  │   └─ src/save.rs       — Versioned JSON/RON local save slots with corruption isolation
  └─ elfstory-gdext/   — Thin `godot` (gdext) wrapper exposing `ElfStorySession` Node/RefCounted to Godot
```

## Why This Split
- **Compile-time bug elimination:** Exhaustive Rust `match` prevents unhandled `TileKind`, `TownSupplyTag`, `EnemyArchetype`, or `DialogueAction` states; strict ownership eliminates React stale-closure bugs (which required ref-workarounds across 6,700 lines of `App.tsx` in the web prototype).
- **Native 3D Visual Ceiling:** Godot 4 Forward+ / Metal provides soft shadows, SSAO, glow/bloom, animated water shaders, skeletal character animation trees, and native `Control` RPG UI without fighting WebGL draw calls or React re-render overhead.
- **Instant Headless Verification:** Every gameplay rule, dungeon connectivity guarantee, loot curve, and town exploit guard runs under `cargo test` in milliseconds.

## Visual & Camera Design (Tunic / Link's Awakening Bar)
- **Cameras:** Overhead Tactical (default) + Chase 3rd-Person (`V` cycles), 4 quarter-turn yaws (`[` / `]`).
- **Environment:** `WorldEnvironment` with warm afternoon `DirectionalLight3D` (soft shadows), procedural sky + IBL ambient fill, subtle SSAO + Filmic/ACES tonemapping, gentle bloom on forge embers/waystones/spells.
- **Cutaway Rule (from Web Audit):** Every `.glb` building/tree placement owns a `hide_terrain_footprint` rect. Procedural terrain tiles inside that footprint are never un-hidden when camera cutaway toggles.
- **No Unseeded RNG:** Every dungeon floor and loot roll draws from `elfstory_core::rng::Rng` so runs are reproducible and shareable by seed.
