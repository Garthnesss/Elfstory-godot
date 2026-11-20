# ElfStory — Project Truth
Status: The Combined Model was approved by Garth 2026-11-17 12:53 PST ("that combined model works — we will just have to try it"). This repo (Garthnesss/ElfStoryReborn) was loaded 2026-11-17 from frozen Elfstory3d at 6df25947ec73fce8a036829fa71659408ab30190 (branch frozen-v1), cloned whole and subtractive. Phase 0 is the prototype spike described at the end of this file.

## What ElfStory is now
A 3D roguelite dungeon crawler where you rebuild Oakhaven between runs, and the rebuilt town significantly changes the next runs. Title: ElfStory. Repo: Garthnesss/ElfStoryReborn. Base: frozen Elfstory3d (frozen-v1 @ 6df2594) cloned whole, subtractive.

## Kept from Elfstory3d (do not redesign)
Turn-based combat, skills/cooldowns, inventory/equipment, monsters/enemy behaviours, Overhead Tactical + Chase cameras, 3D rendering, Oakhaven village art, Wake in Oakhaven death return.

## Core loop (the truth everything else serves)
1. Choose a run zone — choice is driven by what the town needs (a piece, a cure, materials, a lair cleared).
2. Run the dungeon with the kept combat core. Find: pieces (specific), materials + rescued people (renewable), problem-cures (zone-specific).
3. Return alive to bank loot, or die and Wake in Oakhaven losing only unbanked carried loot. Town is never unbuilt.
4. Build beat (short, purposeful): place/complete majors, satisfy a wish, resolve a problem, growth ticks.
5. Town feeds the next run: discrete rewards per major + Population Tier bonuses + unlocked zones/districts.

## Town = two layers
- Majors (Dark Cloud): ~10 placeable major buildings/residents, pieces found in runs, player placement (lot/facing), resident wishes (optional, bonus-only, feedback before confirming, movable later for free, no permanent missables).
- Growth (ActRaiser): houses/roads/fields/population grow automatically around majors per return, directed by player priority, scaled by materials/people/problems. No real-time waiting.

## Town feedback (both kinds, on purpose)
- Lumpy: each major = one discrete reward (shop, storage, companion, +inventory, skill, key...).
- Smooth: Population Tier = a second, smaller progression bar (town tiers gate zones/districts and grant modest global bonuses). It does NOT replace XP/levels.

## Town measurement (3 bars, Georama-style)
Pieces % / Wishes % / Population Tier. Meta gates require Pieces + Population; Wishes are bonus only.

## Problems & expansion (ActRaiser)
Town stalls (plague/dispute/cold/stopped mill) freeze GROWTH only, cured by zone-specific discoveries. Clearing a zone boss/lair unlocks a new Oakhaven district/lots.

## Non-goals / deliberately not copied
Real-time town waiting; population-as-only-XP; hidden missable offerings; weapon-break-vanish; thirst taxes; order/time reward traps; a town-upgrades modal (cut Nov 17 — town must be the loop, not a menu).

## Prototype that proves or kills this truth (build FIRST, small)
One zone, one major (e.g. Blacksmith), 3 pieces for it in the zone, one resident wish, materials+2 rescued villagers, one stall (smithy needs a zone item), population tick on return, major reward = shop opens with one town-tier item.
Pass = a player can say after 2 runs: "my town changed my next run, and I chose my zone because of my town." If that sentence is false in play, the model is wrong no matter how good it looks on paper.

## Art Direction & V2 Hybrid 3D Asset Pipeline (Locked Nov 19, 2026)
- **Aesthetic**: **Crisp Stylized Low-Poly** (Tunic / Link's Awakening / For The King — clean quad topology, weighted normals, vibrant balanced albedos, warm afternoon sunlight + Three.js `RoomEnvironment` IBL).
- **Mac-Only 32 GB Memory Rule**: Never run ComfyUI (`~/ComfyUI`, port `8188`), TRELLIS.2 (`http://127.0.0.1:8765/foundry`), ACE-Step Music Studio, and Ollama simultaneously; `tools/comfy_runner.py` and `~/GLocalLM/ops/foundry.py` arbitrate `AGENT` ↔ `STUDIO` automatically.
- **Strict Two-Track Asset Routing** (see `.agents/skills/oakhaven-asset-pipeline/SKILL.md` and `elfstory-reborn-audit-2026-11-19.md`):
  - **Track A — Architecture, Buildings & Hard-Surface Props (No TRELLIS.2)**: Modular quad kitbashing in Blender 5.3.0 (`tools/blender_arch_kit.py`, `1,500–4,000` clean quads, `< 500 KB` `.glb`, contiguous Smart UVs, beveled edges + weighted normals) textured with 1024px seamless tileable PBR maps (`tools/comfy_runner.py --mode pbr-texture` generating albedo + normal + roughness).
  - **Track B — Organic Creatures, Trees, Rocks & Statues**: 1024px unlit/delighted flat-diffuse concept art (`tools/comfy_runner.py --mode concept`) → observable high-poly sculpt via Elf Works `/foundry` → Blender QuadriFlow Retopo (`~2,500` pure quads) + Shrinkwrap + Clean Smart UV Project + Cycles GPU Selected-to-Active Bakes (1024px Diffuse Color-only + Tangent-Space Normal Map + soft AO blend) + Smooth Shading & Weighted Normals (`tools/blender_retopo_bake.py`, `< 600 KB` `.glb`).

## Nov 19, 2026 Audit Guardrails (What Must Hold in Play)
1. **In-World Town Loop, Not a Detached Modal**: Rebuilt majors and their shops must be physically interactive in Oakhaven (talking to Torin or interacting with the rebuilt lot opens the rebuilt Blacksmith stock; lots are movable later for free with proper `hideTerrainFootprint` clearing).
2. **True Banking & Anti-Cheese Return Tick**: Town pieces/supplies found in a dungeon are carried unbanked until an alive return to Oakhaven (dropped back onto the dungeon floor on `"Wake in Oakhaven"` death, consumed from the pack on banking); empty Turn-0 stair-hops and death returns never tick population.
3. **Stalls Freeze Growth & Town Drives Zone Choice**: Stalls freeze population growth (`townReturnTick`), Population Tier grants modest global bonuses (`computeDerivedStats`) plus visible ActRaiser village growth in Oakhaven, and side-dungeon branches carry zone-specific town supplies so the player chooses their next zone because of what Oakhaven needs.
