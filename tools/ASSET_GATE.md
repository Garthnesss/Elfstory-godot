# Asset Gate — read before adding or re-baking any model

Every `.glb` shipped in `godot/assets/models/` must pass two checks.
Both exist because of real failures (Nov 20, 2026): the original
Oakhaven oak was a torn TRELLIS shell — 202 boundary edges, 22
non-manifold edges, 174 degenerate faces, no trunk — and Mira's
apothecary roof had gaps that read as a broken texture in-game.

## 1. Watertight (hard gate)

A model is watertight when, after welding coincident vertices, every
edge is shared by exactly 2 faces: no boundary edges, no non-manifold
edges, no degenerate faces. Anything less leaks light, breaks bakes
and booleans, and renders torn at close range.

```
/Applications/Blender.app/Contents/MacOS/Blender --background \
  --python tools/check_watertight.py -- godot/assets/models/*.glb
```

Exit 0 = pass. **Do not commit a model that fails.** To repair:
weld (`remove_doubles`) → dissolve degenerate → fill boundary loops
(`holes_fill`) → delete remaining non-manifold verts, in that order.
If the source is a thin crumpled shell (raw TRELLIS output), first
Solidify it, then voxel-remesh — decimate AFTER a voxel pass tears
edges back open, so trim size with the voxel size, never decimate.
Decimate/collapse is also what created the original oak's origami
shards via the old V1 polish script — see the Nov 19 audit.

## 2. Visual gate — source comparison + adversarial review

Topology numbers are the floor, not the verdict. A model ships only
after this process (set by Garth, Nov 20, 2026, after a watertight
but visually ruined oak rebuild was committed on self-grading):

1. **Source first.** Locate the raw generator output (foundry:
   `~/GLocalLM/workspace/3d/`) and render it with
   `tools/render_preview.py`. Never repair or judge a derivative
   without the source render in hand.
2. **Side-by-side.** Render the candidate with the same tool (same
   framing) and compare against the source: silhouette, proportions,
   ground connection (trees have trunks), surface detail, color.
   A flaw you can describe is a fail, not a style note — "reads fine
   at game distance" requires the in-game screenshot, not a sentence.
3. **Adversarial review.** An independent reviewer (subagent that did
   not build the asset) gets both renders plus the gate numbers and
   is briefed to find differences and defects, not to approve.
4. **Garth sees the pack before commit:** side-by-side renders, the
   reviewer's findings, and the numbers (tris, file size, watertight
   counts). Nothing commits on the builder's say-so.

"Verified" is banned for visuals unless this process ran — say
"checks passed" when only checks ran.

## Budgets (V2 pipeline)

- Track A architecture: < 500 KB per `.glb`
- Track B organic: < 600 KB per `.glb`
- Textures: 1024×1024 albedo + normal, WebP in the `.glb`
- Foliage exception class: raw TRELLIS foliage is intersecting thin
  shells and cannot be both strictly watertight and faithful to the
  source — voxel/retopo wraps destroy the leaf detail (proven twice,
  Nov 20). For foliage, the visual gate above decides; state the
  watertight counts and the trade-off in the pack for Garth's call.

## Direction change - Nov 21, 2026 (Garth)
- Hero buildings no longer use the Track A flat-color rebuild: it
  read bland against the TRELLIS concepts. Buildings follow the
  source-first textured path instead: TRELLIS original, scaled to
  the shipped footprint, UV-preserving decimation to ~25% (~48k
  tris, ~3 MB), plus an albedo brightness lift when the bake reads
  dark in-game. The < 500 KB Track A budget above now applies only
  to remaining flat arch-kit props.
