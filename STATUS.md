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
