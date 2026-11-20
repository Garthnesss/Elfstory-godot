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

## 2. Eyeball render (visual gate)

```
Blender --background --python tools/render_preview.py -- in.glb out.png
```

Look at the render. Check: solid surfaces where solid is expected
(no see-through roofs, no floating scraps), a ground connection
(trees have trunks), colors that read at a glance. Then confirm
in-game with a real screenshot — a model can pass both checks and
still be placed wrong.

## Budgets (V2 pipeline)

- Track A architecture: < 500 KB per `.glb`
- Track B organic: < 600 KB per `.glb` (the oak is 784 KB — accepted
  exception, Nov 20: its canopy needed a finer voxel pass to hold
  together; it is one shared file for 5 placements)
- Textures: 1024×1024 albedo + normal, WebP in the `.glb`
