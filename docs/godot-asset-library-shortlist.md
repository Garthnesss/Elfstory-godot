# Godot Asset Library — shortlist (Nov 21, 2026)

Survey of https://godotengine.org/asset-library for what fits ElfStory. The library is mostly free code add-ons (tools, shaders, VFX, editor plugins); it has very little 3D art. Versions, licenses and update dates are as listed on the library pages on Nov 21, 2026.

Any add-on that changes the look still goes through `tools/ASSET_GATE.md`: side-by-side review and Garth's approval before it's committed. Sound and VFX are checked in-game.

## Picked (Garth, Nov 21): combat first

| Add-on | License | Fit |
|---|---|---|
| REEBOO Stylized VFX Free Sample (Godot 4.4+, Oct 2026) | MIT | Hit spark, sword slash, pickup sparkle. Fits bump combat, chest loot and the skills-in-combat work. Needs WorldEnvironment glow (already on). The author says the code was written with an AI assistant. |
| Kenney Particle Pack | CC0 | Particle textures for forge embers, brazier fire, waystone sparkle. |
| Kenney RPG Audio + Kenney Interface Sounds | CC0 | The Godot port has no audio at all yet (the web game had door, loot, step and spell sounds). Cheapest large improvement. |

## Worth trying later

| Add-on | License | Fit |
|---|---|---|
| Simple Grass Textured, or Open Stylized 3D (grass only) | MIT | Grass and foliage over meadow tiles. Oakhaven and Silverwood Vale ground is flat colored boxes today. Open Stylized 3D also builds trees, but the approved oak stays. |
| Debug Draw 3D | MIT | Dev tool: draw paths, FOV and building footprints in the scene to check them by eye. |
| Phantom Camera (updated Nov 17, 2026) | MIT | Smoother camera framing and transitions. Polish; the current tactical and chase cameras work. |
| Kiln Shader Kit (tested on 4.7, Nov 16, 2026) | MIT | 20 shaders including toon and water. Toon shading would likely fight the textured TRELLIS look. |

## Not a fit (as of Nov 21)

- **Sky3D**: a day/night cycle would override the golden-hour lighting decision.
- **Dialogue Manager 3** and other dialogue add-ons: all dialogue lives in Rust `npcs.rs`; a GDScript system would be a second source of truth.
- **Terrain3D**: heightmap terrain doesn't match the tile grid.
- **Kenney and Quaternius 3D model packs**: flat-color low-poly, the style Garth rejected as bland on Nov 21.

These are current calls, not final ones. The project's direction is still forming (see `PROJECT_TRUTH.md`).
