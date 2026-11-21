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
