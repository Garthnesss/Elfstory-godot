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

## Milestone 3 (part 2) — World content port (Nov 20, 2026)

- **world.rs grew to ~3,500 lines:** the three remaining towns
  (Highspire, Thornwick Fen, Cinderwatch) and the Silverwood Vale
  overworld (48×26: four city gates, four dungeon portals, waystone,
  standing stones, hunter camp, Harlan the brigand captain) are ported
  from the TS ASCII layouts with exact gate/portal coordinates.
- **Side dungeons:** Barrow / Mire / Frost generator with per-branch
  depth offsets, biome set pieces, floor-1 guaranteed town supply
  (Barrow Iron / Mire Timber / Frost Coal) and named terminal bosses —
  Arch-Wight Morvath (barrow_2), Spore-Matron Vexis (mire_2),
  Ignis-Warden Kaelthar (frost_3) — each guarding a boss relic chest.
- **Main-descent parity:** the sealed treasure vault (missing in M1),
  all four set pieces (Sepulcher, Spore-Heart, Rime Circle, Sealed
  Brazier with seal doors/braziers), the Abyss echo boss on even
  depths ≥ 6 and the Abyssal Harbinger on odd depths ≥ 7.
- **Zone access:** `generate_zone_by_id`, `create_initial_zones`,
  `get_zone_context` (full lore/tactical/recommended-level table).
- **Session travel wired:** gate cities route through the overworld
  with TS landing tiles; the Highspire causeway gate stays sealed
  without a Citadel Pass (verbatim log line); side-dungeon portals
  exit back to the overworld (no banking); arriving in Oakhaven with
  town items banks them (supplies included) via the return tick.
- **Front-end fix found by screenshot:** Oakhaven's .glb town
  landmarks were drawn at their Oakhaven coordinates in every zone;
  landmark building is now gated to Oakhaven.
- **Verified:** cargo test 77/77 on the Mac (incl. a session
  travel-graph test: gates, sealed Highspire, barrow portal round
  trip, supply banking), zero warnings, gdext builds, headless
  checks green, and real rendered screenshots of Oakhaven and the
  Silverwood Vale captured from the running game.
- Session-level *behaviors* for the new tiles (opening the vault
  door, lighting seal braziers, waystone attunement, boss-chest
  looting) remain future session work, as in the TS split — all
  world-side tiles, interactIds and data are in place.

## Visual pass — haze fix (Nov 20, 2026)

- **Diagnosis:** the washed-out look was stacked — warm tan fog
  (density 0.006) over everything, bloom lifting the bright scene,
  ACES tonemapping desaturating, full-energy warm sky ambient, and
  one palette bug: non-town "floor" tiles were gray-lavender, so the
  overworld ground rendered gray instead of green.
- **Fixes (Main.tscn):** fog density 0.006 → 0.0012 with a pale-blue
  light color; glow off; tonemapper ACES → Filmic; sky ambient
  energy 1.0 → 0.5; color adjustment on (saturation 1.18, contrast
  1.06); sky energy 1.15 → 1.0; sky ground colors changed from tan
  to sage so the backdrop around a map reads as distant meadow;
  sun energy 1.15 → 1.25 for crisper shadows.
- **Fixes (Main.gd palette):** wilderness floor is meadow green,
  roads warmer sand, trees and water deeper.
- **Verified** with before/after rendered screenshots of Oakhaven
  and the Silverwood Vale from the running game (dev capture script,
  removed after use); headless RUN1 check still green.

## Asset re-bake audit debt — verified closed (Nov 20, 2026)

The Nov 19 audit's remaining asset debt (V1 `mira_apothecary.glb`
4,364 KB / 39,615 tris and `oakhaven_oak.glb` 1,842 KB / 17,474 tris
with 512px muddy baked-AO textures) was already resolved in the
assets this project inherited — verified Nov 20 by Blender telemetry
and glTF inspection rather than assumed:

| Asset | Now | Was (audit) | Verdict |
| :--- | :--- | :--- | :--- |
| `mira_apothecary.glb` | 400 KB, 8,492 tris, 0 non-manifold, all smooth | 4,364 KB, 39,615 tris | Rebuilt Track A style: 9 arch-kit primitives (Arch_Stone / Plaster / Timber / Shingle / Iron / Glow / Herb), flat colors, no textures — under the < 500 KB architecture budget |
| `oakhaven_oak.glb` | 358 KB, 2,768 tris, 1024×1024 albedo + normal | 1,842 KB, 17,474 tris, 512px | V2 retopo/bake profile (platypus reference: 2,359 quads, 303 KB) — under the < 600 KB organic budget |

- Files are byte-identical (md5) to `elfstoryreborn/public/models/`.
- In-engine triangle load for Oakhaven's `.glb` set dropped from the
  audit's ~135,897 tris/frame to ~30K (oak ×5 placements + Mira +
  blacksmith + platypus).
- Both models read correctly in the post-visual-pass rendered
  screenshots. No re-bake run was needed; no asset files changed.

## Watertight pass + Mira roof fix (Nov 20, 2026)

- **Audit of all shipped models** (weld-first manifold check): the
  blacksmith, Mira's apothecary and the platypus were already
  watertight; the **Oakhaven oak was not** — 202 boundary edges, 22
  non-manifold edges, 174 degenerate faces, and no trunk (the canopy
  sat on the ground like a bush, in 5 placements).
- **Oak rebuilt:** the torn shell was solidified, voxel-remeshed into
  closed volumes (all canopy blobs kept, floating scraps removed),
  and the original texture re-baked onto it (Cycles diffuse + normal,
  with a delight pass to lift TRELLIS's baked-in shadow). A tapered
  trunk with root flares was added. Result: **watertight**
  (0 boundary / 0 non-manifold / 0 degenerate), 18.4K tris, 784 KB —
  over the 600 KB organic guideline; accepted exception (finer voxel
  needed to hold the canopy; one shared file for 5 placements).
- **Mira's "weird texture" was her roof:** individual shingle tiles
  over open gaps, reading as broken stripes in-game. Two
  shingle-colored deck panels were fitted under the tile fields
  (plane-fit to the tile centroids on both slopes); the tiles now
  read as relief on a solid roof. Still watertight, 402 KB.
- **Gate added so this can't recur:** `tools/check_watertight.py`
  (batch manifold gate, non-zero exit on failure) and
  `tools/render_preview.py` (isolated eyeball render), with the rules
  written up in `tools/ASSET_GATE.md`. All 4 shipped models pass the
  gate; verified in-game with a rendered screenshot.


## 2026-11-21 — Oak replaced under the new asset gate

- The voxel-remeshed oak from the watertight pass was rejected on
  sight by Garth (canopy detail destroyed vs the raw TRELLIS
  source). It shipped in dc01038 and is now replaced.
- Process that produced this fix is the new standard, codified in
  tools/ASSET_GATE.md: render the raw source first, adversarial
  subagent review at close zoom, side-by-side pack to Garth,
  install only on his pick.
- Candidates reviewed at chase-camera zoom: raw 194,770 tris
  (full fidelity), 50% 97,385 (minor loss), 25% 48,692 (minor
  loss), 12% 23,371 (major loss: shards, black speckle).
- Garth picked the 25% cut: 48,692 tris, 2.9 MB, one shared file
  across 5 Oakhaven placements (~243k tris of oak town-wide vs
  ~975k raw). Foliage exception invoked per ASSET_GATE.md: the
  decimated TRELLIS canopy is not strictly watertight (shell
  intersections, invisible in renders) - accepted knowingly.
- Verified in-game with a rendered screenshot before commit.

## Nov 21, 2026 - style direction: textured buildings (Garth)
- The flat-color Track A rebuilds read bland next to the TRELLIS
  concepts (Garth, Nov 21). Blacksmith and Mira now ship the textured
  TRELLIS originals at 25% decimation (~48k tris each, 3.1 / 3.3 MB).
  The blacksmith stone forge with embers - missing from the flat
  rebuild - is back by construction.
- Mira albedo got a brightness lift ((v^0.85)*1.28): TRELLIS bakes
  its own shading and read near-black under the game Filmic tonemap.
  Verified in-game with a rendered screenshot before commit.
- Direction recorded in tools/ASSET_GATE.md: hero buildings follow
  the textured source-first path now.

## Accent lighting (Nov 21, 2026)

- Main.gd: build_landmarks now honors the placement light data ported from TS (PlacementLight in the Rust data): an OmniLight3D per lit building, TS x-offset flipped for Godot handedness. Forge lights (0xff8c2b) flicker in update_camera via _flicker_forge; Mira gets her green apothecary glow (0x7ef29a). The built blacksmith (post-quest lot) carries its own light in the data and will light up the same way.
- Main.tscn: soft bloom on (glow 0.4 / 0.9 / threshold 0.1).
- Verified in-game (tactical + close-ups): warm pool sits on the forge/door, green glow at Mira front. Lights rebuild cleanly with refresh() (children of landmarks_root).

## Golden hour (Nov 21, 2026, Garths call)

- Base light changed from midday to golden hour (accent lights from 06aee4b stay and read stronger at dusk). Sun: rotation (-26, -45) in Main.gd, color (1, 0.68, 0.42), energy 1.2. Sky: dusky top (0.22, 0.30, 0.52), peach horizon (0.97, 0.66, 0.42), sun_curve 0.06, warm-gray ground colors so the tactical backdrop stays calm. Fog warm peach (0.94, 0.72, 0.55) at 0.0012, sky_affect 0.1; ambient 0.48; saturation 1.22.
- Tuned over three in-game captures: v1 was an orange-out (saturated sun + fog sky_affect 0.35 flattened the backdrop); v3 keeps terrain readable with long shadows and warm buildings.

## World session behaviors (Nov 21, 2026)

- New `world_interact.rs` ports the App.tsx `interactWithTile` branches for the M3 world tiles:
  - **Vault door:** bumping it opens a prompt. "Break the Seal" turns it into an open door and alerts every enemy within 8 tiles.
  - **Seal Brazier hall:** the seal door's prompt reports lit/total. Each brazier heals fully, and lighting the last one opens the door ("Unsealed Reliquary Door").
  - **Waystones:** stepping on one opens a prompt. Attune restores 35% HP/MP and gives Shielded +4 DEF for 4 turns. The Starlight Surge option is gated on Arcana or Resolve 3; it clears cooldowns, fills mana and gives +15 gold. Either way it increments `waystonesAttuned`, adds `lore_waystone`, and moves Outrider Oath to stage 2/3.
  - **Chests:** ordinary, vault reliquary (min Epic), side-dungeon boss hoard (90 + depth×22 + 10 gold plus a random set relic), and the overworld satchel (advances Silverwood Road).
  - **Braziers:** restorative braziers, including the hunter camp.
  - Every TS `Math.random()` in these branches now draws from the session RNG.
- **Session:** chests, braziers and waystones trigger when stepped on, with no enemy turn when handled, as in TS. The interact key checks the tile underfoot and then the 8 around it. Moves and interacts are ignored while a prompt is open. Consumables stack on pickup (TS `addItemToPlayer`).
- **Bridge:** `prompt_json`, `choose_prompt` and `close_prompt`, plus a `prompt:<kind>` event. `debug_warp(zone, x, y)` exists in debug builds only, for captures.
- **Godot:** prompts render in the dialogue panel, with disabled options shown disabled. A `tileRev` counter in the snapshot rebuilds the terrain when a tile changes in place. Used chests, braziers and waystones render darkened. Vault doors stand wall-height until opened.
- **Bug fixed:** since `06aee4b`, leaving Oakhaven left freed forge lights in `forge_lights`, so `_flicker_forge` threw a script error every frame in every other zone.
- **TS quirk kept verbatim:** the surge option's label says "+8 Dust", but its effect and log line give +15 gold.
- **Verified:** cargo test 83/83 (6 new tests). The gdext debug build and release check are clean. Headless RUN1_CHECK_OK now includes a prompt bridge smoke test. Main.tscn runs 90 frames with no errors. In-game captures on Floor 2 show the vault prompt, the unsealed door with guardians closing in, and a chase-camera close-up. The capture script was deleted.
- **Not yet checked in-game:** seal hall, waystone, and chest darkening (each is test-covered in Rust only).


## Combat feel pass — approved by Garth (Nov 21, 2026)

- **Core:** a presentation cue feed. `GameSession::take_fx()` drains `{kind, x, y, dx, dy}` events: hit, crit, kill, player_hit, windup, pickup, chest, brazier, waystone, unseal, level_up. They aren't game state and are never saved; the feed is capped at 64. Bridge: `take_fx_json()`. cargo test 84/84.
- **Godot:** `refresh()` drains the feed and plays effects from `addons/reeboo_vfx` (REEBOO Stylized VFX Free Sample, MIT, reviewed: particles and lights only):
  - slash from the attacker's tile toward the target, scaled 0.55 to fit the 1-unit tiles (the scene is authored at character scale)
  - hit spark on the target (1.5× on a crit, 1.8× on a kill, 0.7× when the player is hit)
  - pickup sparkle on loot, chests, braziers, waystones and level-ups; it loops by design, so it's stopped after 0.8 s and frees itself (verified: 0 nodes left)
- **Sound:** an `SFX` table plays the first available file for each cue, plus a click on every UI button. In the project now: 8 Kenney Interface Sounds (CC0) in `assets/audio/ui`. The Asset Library's Kenney RPG Audio port was deleted from GitHub (404), so combat hit sounds point at `assets/audio/rpg/*.ogg` and stay silent until that pack is added from kenney.nl.
- **Kenney Particle Pack** (CC0) is downloaded but not in the project; it's held for the brazier, forge and waystone pass.
- **Verified:** headless checks green. In-game captures on Floor 2: the chase camera shows the slash and impact rings, and the tactical view shows the chest sparkle as a warm glow, subtle at that zoom. Sound is untested by ear.

## Skills in combat (Nov 21, 2026)

- **New `combat_turn.rs`** ports the combat half of App.tsx:
  - **Melee:** lifesteal, bleed-on-crit / chill-on-hit affixes, Soul Siphon r3 crit mana, Flexible Combat follow-up. Kills still give enemies their turn, as in TS.
  - **All 8 active skills** with exact TS numbers: Sundering Cleave, Bastion Bash, Iron Fortress, Shadowstep, Viper Edge, Executioner's Mark, Ember Lance, Rime Nova. Arcane Echo, Carry the World and Arcane Conduit are included. Invalid targets log the TS hint and cost nothing.
  - **Kill passives:** Crimson Vigor r3, Soul Siphon, mana_on_kill.
  - **Aiming:** hotbar trigger, auto-aim at the nearest valid enemy, cycle, confirm, cancel, plus a keyboard aim cursor for Shadowstep. Learning skills and assigning the hotbar work as in TS.
- **The enemy turn is now the TS `runEnemyTurns` combat core**, replacing the M2 slice:
  - The player's statuses, cooldowns and prodigy cooldowns tick, with 1 MP every 2 turns (+2 per turn in town), regen, and DoTs on the player.
  - The Draconic Will cleanse and resist, Cauterize and Spine of the World prodigies work.
  - Enemies: skill cooldowns, DoT ticks, status countdown, stunned enemies skip, Shielding Rune.
  - Telegraphed intents resolve with dodge, 0.7 DEF mitigation, minimum 2, and skill ailments. Enemy Shadowstep blinks.
  - Fresh intents prime every turn, including ranged ones; the slice only primed adjacent windups. Adjacent strikes use dodge, Phantom Reflex mana and counter, chill (×0.65 ATK) and thorns.
  - Visibility is computed (sight radius 8): only enemies that see the player get alerted, and Shadowstep needs a visible tile. Diagonal moves can't cut wall corners (TS rule).
  - Wake in Oakhaven now clears statuses and cooldowns (TS respawn).
- **Bridge:** `trigger_hotbar`, `confirm_target`, `cycle_target`, `move_aim`, `cancel_aim`, `wait_turn`, `skills_json`, `learn_skill`. The snapshot adds the hotbar, the armed skill and aimed target, and enemy statuses and intents. String enums gained `parse()` (TS ids).
- **Godot:**
  - Hotbar (bottom-right; keys 1–5, click works too); while aiming, the movement keys move the cursor, Tab cycles, F/Enter casts, Esc cancels.
  - Yellow aim tile, red telegraph tiles where windups will land, skills panel (K) with Learn buttons, Space or `.` to wait, statuses and skill points in the HUD, VFX for skill cues.
- **TS quirks kept as-is, for Garth to decide:**
  1. ~~1-turn stuns expired before the enemy acted.~~ **Fixed (Garth, Nov 21):** the stun is now read before the status countdown, so an N-turn stun skips N enemy turns, and the log says "<enemy> is stunned." Test: `a_one_turn_bash_stun_skips_the_enemys_next_turn`.
  2. ~~Ember Lance r2's "pierces 1 tile behind" wasn't implemented.~~ **Added (Garth, Nov 21):** at rank 2 the lance carries one tile past the target, away from the caster, hitting any enemy there for 60% of the lance plus Burn 5/t for 2 turns (the rank-3 splash strength). Rank 3's blast already covers that tile, so pierce is rank 2 only and nothing is hit twice. Test: `ember_lance_rank_2_pierces_one_tile_behind_the_target`. cargo test 99/99.
- **Found, not changed:** serde writes the `string_enum!` types as Rust variant names (`"ShieldBash"`), not TS ids (`"shield_bash"`). This affects save JSON (`SAVE_VERSION` 1) and `state_json`'s player block. It's internally consistent, but not TS-save-compatible.
- **Verified:** cargo test 97/97 (13 new: each active skill, stun skip, DoT ticks both ways, mana regen, kill passives, learning, targeting order and cycling, id parsing). The gdext build is clean, and the headless RUN1 check and Main scene run with no script errors. In-game captures on Floor 2 show the aim tile, the cleave slash, the bash flash with a "cd 2t" hotbar, and a telegraph tile; Oakhaven's textured buildings still render.
- **Local noise:** the editor import cache logs UID warnings for `*_Image_N.webp` (the files are absent and `*.import` is gitignored). Textures still render.

## macOS app + combat sounds (Nov 21, 2026)

- **`ElfStory.app` builds:** run `tools/build_app.sh` (add `--smoke` to launch it, save `build/smoke.png` and quit). Output is `build/ElfStory.app`: 188 MB, gitignored. The script does a release Rust build, then exports the `macOS` preset in `godot/export_presets.cfg`.
  - The binary is universal, because the official template ships only universal; it runs natively on Apple Silicon. The bridge `.dylib` is arm64 and bundled in `Contents/Frameworks`.
  - Ad-hoc signed, not notarized: it runs on this Mac. On another Mac it needs right-click → Open the first time.
  - Needs Godot 4.7.2 export templates, installed Nov 21 (macOS + version.txt) in `~/Library/Application Support/Godot/export_templates/4.7.2.stable/`.
  - Project setting `textures/vram_compression/import_etc2_astc=true` is required for arm64/universal exports.
  - `elfstory.gdextension` gained `macos.release`.
- **Bug found by the export and fixed:** the extracted textures of the textured blacksmith, Mira and oak (`*_Image_0/1.webp`) were never committed and had vanished from disk. The editor hid this by drawing from its import cache, but the exported game failed to load all three models. A forced re-import restored them from the `.glb` files (Mira's albedo lift is baked into the `.glb`). They're now committed. Verified: pixel comparison against the pre-fix editor render shows mean difference 0.0 / 0.0 / 0.02 on blacksmith, Mira and oak. This also clears the import-cache UID warnings noted earlier.
- **Smoke hook:** `Main.gd` honours `-- --elfstory-capture=<png>`: it renders about 2 s of Oakhaven, saves the frame and quits. It does nothing otherwise.
- **Combat sounds:** Kenney RPG Audio (CC0, downloaded by Garth from kenney.nl). Eight files fill the existing combat cues (slash, crit, kill, hurt, coins, chest latch, vault door, Iron Fortress), and four footsteps play on each successful step at −8 dB. Sound is still not checked by ear.
- **Verified:** the exported app launches with no errors and renders textured Oakhaven, the HUD and the hotbar (smoke capture reviewed). cargo test 99/99, headless checks green.

## Save & Continue (Nov 21, 2026)

- **`session_save.rs`** turns the live game into the web version's save-bundle shape and back: `player`, `zones` (every visited zone, including looted chests, opened vaults and enemies), `currentZoneId`, `worldFlags`, `quests`, `merchantStocks`, `logs`, `town`, `difficulty` and `ngPlusCycle`. A `rust` block holds the RNG state (a loaded game continues the same seeded sequence), the item-id counter, extra dialogue flags, the delve counters the return-tick guards read, and the tile revision.
  - Presentation state (aim, open prompt or dialogue, fx cues) is not saved.
  - Loading goes through save.rs's checks: corrupt, newer-version and fallen-hardcore slots are refused with their messages.
- **`DirStorage`** (save.rs) keeps one JSON file per slot and writes via temp file + rename, so a crash mid-save keeps the previous save. Saves live in `~/Library/Application Support/Godot/app_userdata/ElfStory Reborn/saves/` (slot 1, about 70 KB per Oakhaven-only game), shared by the editor and the app.
- **Bridge:** `save_game`, `load_game`, `save_summary_json`.
- **Godot:**
  - Autosave on every zone change and death-wake, every 25 actions, and on window close.
  - At launch, if a save exists, a panel offers **Continue — <zone>, level N** or **New Game**. Unreadable saves show their message and are never silently replaced.
  - `--elfstory-capture` smoke runs never read or write saves.
- **Verified:** cargo test 101/101. The new round-trip test walks into the overworld and changes player, flags, town, quests and a tile, then saves, loads and compares everything, RNG continuation included; corrupt and empty slots are also covered. A two-launch editor check (move, learn a skill, autosave, relaunch, see the title, Continue) came back at the same tile with the skill on the hotbar; the test save was deleted afterwards. The rebuilt app passes the smoke test.
- **Not yet:** multiple slots in the UI (the core supports 3), difficulty choice at New Game, and a proper title screen with art.

## Meadow grass (Nov 21, 2026)

- **Grass and wildflowers** on every `floor` tile in Oakhaven and the Silverwood Vale (`GRASS_ZONES` in `Main.gd`). Written in-house instead of an Asset Library add-on: `grass.gd` builds one procedural tuft (11 tapered blades) and instances it about 22 times per tile on a MultiMesh, with a sparse second MultiMesh of flower heads (white, buttercup, lavender, coral). `grass.gdshader` ramps root-to-tip colour, sways with wind (rolling gusts + flutter, roots stay planted) and lets the low sun through the tips.
- Placement is a pure function of tile coordinates (integer hash + fixed-seed noise), so each zone always grows the same meadow and the game's seeded RNG is untouched. Tiles under a model footprint get no grass. Grass casts no shadows.
- **Ground under the grass** is a darker green (`Grass.GROUND`), and meadow floor boxes close their 3% gap so the tile grid no longer shows there; roads and other tiles keep the grid.
- **Review:** two rounds of in-game captures at tactical and chase zoom. Garth's round-1 notes (too busy, not lush enough, too yellow, hide the grid) were all addressed; he approved round 2.
- **Capture hooks** (dev only): `--elfstory-warp=<zone>[,x,y]` (debug builds) and `--elfstory-chase` alongside `--elfstory-capture=<png>`.
- **Verified:** cargo test 101/101, headless RUN1 check OK, all captures render with no script errors. Frame rate not measured (roughly a quarter-million extra triangles per zone).

## Platypus feet (Nov 21, 2026)

- **`oakhaven_platypus.glb` rebuilt from its TRELLIS source** (`~/GLocalLM/workspace/3d/oakhaven_platypus-20261119-225817.glb`). The Nov 19 watertight retopo (2,359 quads) had turned the webbed toes and claws into smooth paddles. The new model is 49,017 tris and 2.26 MB with a 2048px baked albedo, and the toes, webbing and claws match the source.
- **New tool `tools/blender_decimate_source.py`:** welded-vertex import → Collapse decimate (UV-delimited) → smooth-by-angle 35° → bottom-center origin → optional `--rebake` onto fresh Smart UVs. Route notes are in `docs/asset-generation-pipeline.md`.
- **Gate:** routes compared side by side against the source (decimation at 8/15/25/35%, welded vs unwelded, 12k-quad retopo, 1024 vs 2048 bake). Two adversarial review rounds. Garth approved shipping with the watertight exception and the accepted close-zoom flaws recorded in `tools/ASSET_GATE.md`. The platypus appears at Population Tier 2. It was checked in-game with a temporary placement and camera, and nothing from that patch was committed.
- **Verified:** clean Godot re-import, and the headless RUN1 check is OK.

## Title screen, 3 save slots, difficulty & app icon (Nov 22, 2026)

- **Core (`session.rs`, `session_save.rs`):** `GameSession::new_game_with_difficulty` creates runs at Story / Standard / Hardened (`DifficultyTier`); `GameSession::delete_slot` removes a slot's save file cleanly (no-op on empty slots); `SaveSummary` includes `starterClass`, `difficulty`, `kills`, and `ngPlusCycle`.
- **Bridge (`elfstory-gdext/src/lib.rs`):** added `new_game_with_difficulty` and `delete_slot`; made `save_game`, `load_game`, and `save_summary_json` slot-aware (`1..=3` via `slot_id`).
- **Godot (`title_screen.gd`, `Main.gd`):** full-screen title overlay over `title_crypt_backdrop_1794536484639.jpg` with discipline picker (Vanguard / Shadowcraft / Runecaster / Wanderer), difficulty selector (Story / Standard / Hardened), 3 independent save slot cards (`Continue`, two-click `Delete` confirmation, and two-click `Replace Slot N?` confirmation before overwriting an occupied slot), plus dev capture flags `--elfstory-show-title` and `--elfstory-save-dir=`.
- **App icon:** added `godot/assets/icon/elfstory_icon.png` wired into `godot/project.godot` (`config/icon`) and `godot/export_presets.cfg` (`application/icon`).
- **Verified:** `cargo test` **102/102** (including `slots_are_independent_and_carry_difficulty`), `cargo build`, Godot headless `run1_check.gd` (`RUN1_CHECK_OK`, extended with slot & difficulty bridge checks), and `tools/build_app.sh --smoke` (`ELFSTORY_CAPTURE .../build/smoke.png err=0`).

## Town loop feel — cold forge ruin, lot rebuild moment & ActRaiser growth (Nov 22, 2026)

- **Core (`town_layout.rs`, `session.rs`):**
  - Before `blacksmith_complete`, `landmark:torin_forge` renders as `procedural://cold_forge_ruin` at `(7, 2)` instead of a pre-built glowing forge, so rebuilding on a chosen lot is a real visual transformation.
  - Added visible ActRaiser village growth structures per Population Tier (`20/35/55`): Tier 1 (`tier1_west_arbor` + `tier1_garden_field`), Tier 2 (`tier2_pond_totem` platypus + `tier2_settler_cottage`), and Tier 3 (`tier3_east_grove` + `tier3_east_cottage` + `tier3_harvest_field`), placed on verified conflict-free meadow tiles.
  - `complete_blacksmith`, `relocate_blacksmith`, and `return_to_oakhaven` now emit `town_build`, `town_relocate`, and `town_tier_up` FX cues, log TS-matched stall/supply/tier-up messages, refresh player derived HP/MP when a tier rises, and refresh cached `blacksmith` merchant stock when regional supplies are banked post-completion. `state_json()` now exposes `townTierBonuses`.
- **Godot (`Main.gd`, `lib.rs`):**
  - Added procedural 3D builders (`make_cold_forge_ruin`, `make_village_cottage`, `make_village_field`), pre-rebuild candidate lot markers (`build_lot_markers`), grass-skip logic under procedural structures, celebration VFX/SFX for `town_build` / `town_relocate` / `town_tier_up`, and an upgraded `TownPanel` showing tier bonuses, piece/Heartcoal checklists, wish hint, lot build/relocate controls, `Open Blacksmith Shop`, and regional supply tracking.
  - Added debug-only `debug_set_town` bridge + `--elfstory-demo-town=<tier>,<lot>` capture flag.
- **Verified:** `cargo test` **103/103** (new `town_loop_rebuild_fx_and_supply_stock_refresh` + updated `tier_growth_placements_follow_population_tier`), `cargo build`, Godot headless `run1_check.gd` (`RUN1_CHECK_OK`) + 30-frame `Main.tscn`, Tier 0 and Tier 3 Oakhaven captures reviewed via `read_file`, and `tools/build_app.sh --smoke` (`ELFSTORY_CAPTURE .../build/smoke.png err=0`).

## Remaining combat gaps closed (Nov 22, 2026)

- **Spirit Companions (`combat_turn.rs`, `session.rs`, `Main.gd`):**
  - `new_game` bonds `Starlight Owl` for Runecaster (`+1` sight radius to 9, passive mana trickle, +1 MP on hit) or `Silverwind Lynx` for other classes (20% crit pounce, periodic `+2` heal).
  - Companion HP scales on level-up; heals and reforms when entering town or resting 8 turns after collapse; pursues visible/alerted foes within leash 6, attacks adjacent enemies, absorbs enemy strikes when adjacent, and swaps tiles cleanly when stepped onto outside town.
  - `Main.gd` renders the companion capsule + label and shows companion HP / reform timer in the HUD.
- **Enemy Death Quirks (`session.rs`):**
  - `GraveHusk` triggers "Deathless" once per enemy, reviving in place at 30% HP with no kill credit or loot (`deathless` FX cue).
  - `SporeHulk` detonates on death (`spore_burst` FX cue), dealing `6 + depth*2` damage and Poison (3t) to point-blank killers (`check_player_fatal` handles Cauterize/death).
- **Boss Confrontation Prompts & Haste (`world_interact.rs`, `combat_turn.rs`, `session.rs`):**
  - `PromptKind::BossConfrontation` triggers pre-fight confrontation modals for Vaelen, Serath, and Malakor (each with attribute-gated options and one-time sighting per archetype) plus Mini-Boss Warden sightings (`boss_roar` FX cue).
  - `after_step_on_tile` consumes `haste_steps` for free movement without advancing enemy turns (`haste` FX cue), cleared on `wake_in_oakhaven`.
- **Verified:** `cargo test` **107/107** (4 new tests: `companion_fights_heals_swaps_and_reforms`, `grave_husk_revives_once_and_spore_hulk_bursts_adjacent`, `boss_confrontation_prompts_trigger_once_and_resolve`, `haste_steps_grant_free_moves_without_enemy_turn`), `cargo build`, Godot headless `run1_check.gd` (`RUN1_CHECK_OK`), 30-frame `Main.tscn`, and `tools/build_app.sh --smoke`.

## Diorama look & combat feel — report Option A, Days 1–3 code half (Nov 22, 2026, branch `presentation/diorama-feel`)

- **Lighting (`Main.tscn`):** Filmic → **AgX** (exposure 1.12, contrast 1.32), cool ambient fill instead of the orange sky, stronger low sun with soft shadows, SSAO retuned + SSIL on, saturation boost cut from 1.22 to 1.08.
- **Per-zone moods (`Main.gd` `ZONE_MOODS`):** golden hour on the surface; cool crypt (main descent), dusty barrow, teal mire, icy frost, each with its own sun, ambient, fog, floor and wall stone.
- **Terrain shader (`tile.gdshader`):** per-tile tone jitter, painted mottling, darkened block rims, stone courses + dark foot on tall sides. Dungeon rock fully enclosed by rock (MultiMesh custom data) renders as one dark earth mass, so rooms read as carved out instead of a grid of boxes.
- **Wall cutaway:** tall tiles between the player and the camera dissolve with a screen-door dither (radius 1.8, 2.4 in chase view).
- **Ground skirts:** procedural dirt `Decal` under every Oakhaven building; actors sit on render layer 2 so decals skip them.
- **Camera:** overhead pulled in to a ~45° diorama tilt (`0, 13.5, 12`); mouse wheel or `-`/`=` zooms 0.65–1.7×.
- **Combat feel (`juice.gd`):** actors persist between refreshes (keyed by entity id). Attack lunge (0.05 s pull → 0.07 s strike), 0.12 s hit-flash + squash, death flatten-pop, arcing damage numbers taken from snapshot HP diffs, hit-stop only on crits/kills/hits taken, camera trauma shake. Blows in one turn are staggered 0.11 s apart, and any key press flushes queued cues and finishes every tween, so input is never blocked.
- **UI skin:** walnut-timber panels with brass edges, cream serif (Palatino) text, matching buttons; smaller NPC name labels.
- **Verified:** `cargo test` 107/107, headless `run1_check.gd` (`RUN1_CHECK_OK`), 30-frame headless `Main`, plus captures of Oakhaven Tier 2, dungeon_1/barrow_1/mire_1/frost_1, and a scripted 3-swing combat sequence at 40/100/250/500 ms. **Not yet checked by hand in the editor.**
- **Not done (needs assets):** miniature actors to replace the capsules, and the mauve town wall blocks.

## Miniature figures + generation queue (Nov 22, 2026 — APPROVED by Garth, shipped and merged to main)

- **Generation queue (`tools/genq.py`, Garth's rule):** a machine-wide FIFO in `~/.asset-gen-queue/`. One ComfyUI/TRELLIS/Blender-bake job runs at a time across every AI session, and it also waits when the foundry or ComfyUI is busy with a job started outside the queue. `tools/comfy_runner.py` queues itself. The rule is written into AGENTS.md, docs/asset-generation-pipeline.md, the asset skill and the global CLAUDE.md.
- **Figures:** vanguard, Silverwind Lynx, Blight Rat and Crypt Sentinel. Concept → BiRefNet → queued `/foundry` → `blender_decimate_source.py` with the new `--cut-base` / `--cut-z` / `--target-tris` options and a colour + metallic + roughness re-bake. Gate pack: `docs/asset-gate/figures-2026-11-22/`.
- **Godot:** actors are figures on walnut plinths with faction rims (blue player, teal companion, red enemy, gold boss, green NPC). They fall back to a capsule when there's no model yet, and turn to face their move or attack direction. The hit-flash is now an additive overlay, so it works on any model.
- **Core:** `state_json` enemies now include `archetype`. That also fixed the Starlight Owl colour, which never applied because the companion kind serializes as `StarlightOwl`.
- **Verified:** `cargo test` 107/107, `RUN1_CHECK_OK`, 30-frame headless run, close-zoom combat captures. Adversarial review round 1 was done and its main finding (colour-only bake) is fixed. **Garth's call Nov 22: ship-with-exceptions for all four (non-watertight, over budget), sentinel keeps its teal cloak, rat v2 accepted.** Rat v2 (20k tris, 1.90 MB, clean no-delight concept, no base cut) replaced the v1 rat after this entry's table was written; post-ship the same day its gate was completed: in-game captures taken, watertight 16,764/34/2 (fails, exception class), second adversarial review verdict SHIP-WITH-EXCEPTION (faithful bake; pose/tail/moss-read exceptions inherited from the source). Delighting is now off by default in `tools/comfy_runner.py` (`--delight` to opt in).

## Playtest + audit (Nov 22, 2026, Muse — Garth's ask: launch it, playtest it, audit it, merge the updates)

- **Launched:** Godot project running on the Mac at current main (`6cd2d19`); window left open for Garth.
- **New harness:** `godot/playtest_check.gd` — a headless, deterministic (seed 20261120) bridge-level playtest that plays a first-hour run and prints `PLAYTEST_OK` / `PLAYTEST_FAIL`: new game (Vanguard + Silverwind Lynx bonded) → overworld Blight Rat fight at (9,17) (killed; player 61→58 HP) → dungeon_1 (11 enemies: crypt_skeleton / plague_rat / goblin_skirmisher) → Crypt Sentinel fight (player 58→27 HP) → Sundering Cleave cast through the hotbar (sentinel 37→3 HP, mana 18→14 = 5 cost with +1 alternating-turn regen) → Torin dialogue opens → completed-blacksmith stock (Oakhaven-Forged Blade + regional gear) → save/load round trip. **Result: PLAYTEST_OK, 2 kills, 0 notes.** Hotbar slots are 0-based in the bridge and targeted skills arm the aim returning a None event by design — the harness documents both so nobody re-files them as bugs.
- **Visual audit:** captures of title, Oakhaven, dungeon_1 (Moss-Sunken Reliquary), mire_1 (Fungal Hollow) and frost_1 (Frostpeak Cavern) at current main. Title and per-zone moods read clean; the new figures (vanguard, lynx, rat, sentinel) render correctly on their plinths in town, overworld and dungeon frames; no errors in the game log; no TODO/FIXME markers in core, Godot scripts or tools.
- **Findings (no code defects found; nothing here was changed unilaterally):**
  1. *Balance spike, both directions:* one depth-1 Crypt Sentinel fight cost a fresh level-1 Vanguard 31 HP (58→27), while one Sundering Cleave dealt 34 damage — nearly one-shotting a 37 HP sentinel. Fights are short and swingy at level 1. Any retune is Garth's call.
  2. *Figure coverage:* only 4 archetypes have figures (vanguard, lynx, blight rat, crypt sentinel). Goblins, the other 13 enemy archetypes and all NPCs still render as capsules on plinths. A second figure batch is the obvious next asset run.
  3. *Occlusion:* enemies standing behind tall wall tiles are partly hidden by the wall (the cutaway dither is centred on the player only) — readable in play, noted for the presentation pass.
- **Verified at merge:** `cargo test` 107/107, `RUN1_CHECK_OK`, `PLAYTEST_OK`, 30-frame headless Main clean.

## Miniature figures batch 2 + cutaway dither fix (Nov 22, 2026)
- **Bone Acolyte figure SHIPPED** (Garth ship-with-exceptions call): fig_bone_acolyte.glb (1.9 MB, 20,000 tris, violet skeletal cultist) wired for bone_acolyte in Main.gd at FIGURE_HEIGHT 1.35. Gate pack: docs/asset-gate/figures-batch2-2026-11-22/ — independent review verdict SHIP-WITH-EXCEPTION ("strongest conversion in either batch"); caveat: reads as a hooded caster, not visibly skeletal (hood interior is a black void).
- **Goblin Skirmisher HELD by Garth** after a review REJECT + fix pass (Mira albedo lift via new tools/glb_albedo_lift.py, re-bake cut-z 0.085, tex 1024, 1.8 MB). The presented captures were corrupted by the cutaway bug below, so the goblin was re-shot clean post-fix and awaits a fresh call; its GLB + wiring remain uncommitted.
- **Cutaway dither fix (godot/tile.gdshader):** the player cutaway capped fragment discard at 88%, leaving a dotted ghost of wall printed over figures near the player. Core of the cutaway now discards fully (inside > bayer), feathered rim unchanged; verified by before/after captures — figures render clean, walls still dissolve.
- Playtest harness still PLAYTEST_OK after the wiring + shader changes.

## Goblin Skirmisher SHIPPED (Nov 22, 2026, follow-up)
- On the clean post-dither-fix captures, Garth reversed the hold: fig_goblin.glb (1.8 MB, 28,000 tris, lifted albedo) is wired for goblin_skirmisher in Main.gd at FIGURE_HEIGHT 0.95 and committed. Figure coverage is now 6 of 17 archetypes (Vanguard, Lynx, Blight Rat, Crypt Sentinel, Goblin, Bone Acolyte). Playtest still PLAYTEST_OK.

## Miniature figures batch 3 — Ash Cultist & Grave Husk SHIPPED (Nov 22, 2026)
- **Ash Cultist SHIPPED** (Garth ship-with-exceptions call): `fig_ash_cultist.glb` (1.7 MB, 20,000 tris, burnt-orange robe + magma orb) wired for `ash_cultist` in `Main.gd` at `FIGURE_HEIGHT` 1.35. Gate pack: `docs/asset-gate/figures-batch3-2026-11-22/` — independent review verdict `SHIP-WITH-EXCEPTION` (watertight `12046/86/0`, 1.7 MB, toes sliced at `z=-0.4006` masked by robe hem in-game).
- **Grave Husk SHIPPED** (Garth ship-with-exceptions call after v2 fix pass): v1 `REJECT` → v2 (`fig_grave_husk.glb`, 2.3 MB, 27,999 tris, `--cut-z 0.068`, Mira albedo lift) wired for `grave_husk` in `Main.gd` at `FIGURE_HEIGHT` 1.38. Independent re-review verdict `SHIP-WITH-EXCEPTION` (watertight `17147/70/3`, residual shin-greave edge spikes & minor close-up pinholes). Figure coverage is now 8 of 17 archetypes. Playtest still PLAYTEST_OK.

## Miniature figures batch 4 — Shadow Stalker SHIPPED, Blight Weaver HELD (Nov 22, 2026)
- **Shadow Stalker SHIPPED** (Garth ship-with-exceptions call on v3): v1/v2 `REJECT` → v3 (`fig_shadow_stalker.glb`, 2.1 MB, 28,000 tris, violet hood + purple crystal torch) wired for `shadow_stalker` in `Main.gd` at `FIGURE_HEIGHT` 1.35. Gate pack: `docs/asset-gate/figures-batch4-2026-11-22/` — independent review verdict `SHIP-WITH-EXCEPTION` (watertight `20088/94/2`, close-zoom speckle on torch grip shaft). Figure coverage is now 9 of 17 archetypes.
- **Blight Weaver HELD** after v3 `REJECT` (tan base rock mound left under foot/pedestal at `0.054` cut, plus bracer bake pitting); base-rock cleanup pass in progress.
