# Asset gate — miniature figures batch 2 (2026-11-22)

Second figure batch, same pipeline and rules as batch 1
(tools/ASSET_GATE.md): concept (DreamShaper XL Turbo via
tools/comfy_runner.py, delighting OFF — batch 1 finding) → BiRefNet
cutout → TRELLIS.2 via the /foundry queue (tools/genq.py) →
tools/blender_decimate_source.py --rebake (colour + metallic +
roughness) with the sculpted base cut → game plinth in godot/Main.gd.
**Nothing here is committed until Garth rules on this pack.**

## Goblin Skirmisher — fig_goblin.glb (1.8 MB, 28,000 tris) — FIXED after review REJECT

- Concept: seed 606 after three rounds. The generator kept doubling
  the figure (two-headed s101, twin figures) and piling on oversized
  weapon clusters; "no weapons" was ignored outright. Seed 606 is the
  cleanest single figure: bright lime-green crouched goblin, chunky
  spiked axe, brown leather scraps. (fig_goblin_concept_s606.png)
- Sculpt: fig_goblin-20261122-150026 (TRELLIS 512, texture 1024) via genq.
- Conversion took four bakes:
  - v1 (20k, auto cut): thin black base disc left under the feet, face melted.
  - v2 (28k, --cut-z 0.10): base clean, but the independent review
    REJECTED it — in-game it collapsed into a dark olive smudge next to
    the Vanguard, feet shredded at the cut plane, 2.6 MB.
  - v3 (28k, --cut-z 0.07, tex 1024): feet kept, but the base slab returned.
  - v4 FINAL (28k, --cut-z 0.085, tex 1024): base gone, feet intact,
    1.8 MB, plus the Mira brightness lift ((v^0.85)*1.28) applied to
    the embedded albedo with tools/glb_albedo_lift.py.
- Watertight (final): boundary 33,836 / non-manifold 39 / degenerate 3
  — FAIL, same TRELLIS open-shell exception class as batch 1.
- In game (fig_goblin_ingame_full/close.png, post-fix): bright
  lime-green, ears/axe/hunch read instantly beside the Vanguard. The
  murk the reviewer rejected is gone.
- Wired: ENEMY_FIGURES goblin_skirmisher, FIGURE_HEIGHT 0.95.

## Bone Acolyte — fig_bone_acolyte.glb (1.9 MB, 20,000 tris)

- Concept: seed 606 — single skeletal cultist, violet-purple hooded
  robe, gnarled staff with ornate gold head, bony casting hand.
  (fig_bone_acolyte_concept_s606.png)
- Sculpt: fig_bone_acolyte-20261122-150959 via genq. One bake (20k
  tris, auto base cut clean).
- Watertight: boundary 22,583 / non-manifold 36 / degenerate 0 — FAIL,
  same exception class.
- In game (fig_bone_acolyte_ingame_overhead/chase.png): the purple
  robe reads unmistakably at overhead distance; staff and hood
  survive. Reviewer flag — one corridor acolyte reading as a heap —
  checked on a zoom crop: it is a standing figure whose body is
  hidden by the corridor front wall (known player-centred cutaway
  behaviour), not a collapsed pose.
- Honest caveat from the review: it reads as a hooded cultist caster,
  not visibly skeletal — the hood interior is a black void and the
  only undead signal is the extended bony hand.
- Wired: ENEMY_FIGURES bone_acolyte, FIGURE_HEIGHT 1.35.

## Independent adversarial review (completed)

- Bone Acolyte: **SHIP-WITH-EXCEPTION** — "the strongest conversion
  in either batch"; fidelity high (hood, drape, staff head, hand);
  caveats = gate numbers, skeletal read, robe-hem shards, softened hand.
- Goblin v2: **REJECT** — in-game murk vs the Vanguard, shredded feet,
  worst gate numbers yet on the most common enemy. Fix list: value
  lift, foot repair, size down, re-shoot in-game. All four done in v4
  (see above); the re-shoot is fig_goblin_ingame_close.png.

## Process notes

- New GLBs need a Godot editor import pass before a game run can load
  them (--headless --editor --import); without it the figure loader
  silently falls back to capsules.
- The --elfstory-capture harness only works windowed; headless runs
  hang waiting for frame_post_draw.
- The Blender render_preview camera made the base-cut goblin look
  tilted; the model probe (rotation 0, sane AABB) and the in-game
  captures show it upright. Trust the game, not the preview, for pose.
- tools/glb_albedo_lift.py: applies the Mira lift to a GLB embedded
  albedo in place (used for the goblin; reusable for future dark bakes).

## Decision (Garth, Nov 22, 2026)

- **Bone Acolyte: SHIP with exceptions** (non-watertight, over 600 KB
  budget — same exception class as batch 1). Shipped in this commit.
- **Goblin: HOLD.** Garth rejected it from the presented captures.
  After his call, the captures turned out to be corrupted by a game
  bug: the wall-cutaway dither in godot/tile.gdshader capped its
  discard at 88%, leaving a dotted ghost of wall fragments printed
  over every figure near the player — the goblin was judged through
  that speckle. The shader is fixed in this commit (core of the
  cutaway now discards fully; feather stays on walls only), and the
  goblin was re-shot clean
  (fig_goblin_ingame_ditherfix_full/close.png). The goblin GLB and
  its Main.gd wiring stay OUT of this commit, uncommitted in the
  working tree, pending a fresh call from Garth on the clean captures.
