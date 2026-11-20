"""Asset gate: every shipped .glb must be watertight.

Welds coincident vertices first (glTF splits verts at UV/normal seams,
which fakes boundary edges), then fails the file if any mesh has
boundary edges, non-manifold edges, or degenerate faces. A watertight
model is a set of closed solids — no holes for light leaks, broken
bakes, bad boolean/retopo results, or torn renders downstream.

Usage:
  Blender --background --python tools/check_watertight.py -- \
      godot/assets/models/*.glb
Exit code 0 = all watertight, 1 = at least one failure.
Run this on every new or re-baked model BEFORE committing it.
"""
import bpy
import sys
from collections import defaultdict

argv = sys.argv[sys.argv.index("--") + 1:]
if not argv:
    print("usage: check_watertight.py -- file.glb [file.glb ...]")
    sys.exit(2)

failures = 0
for path in argv:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=path)
    file_ok = True
    for ob in bpy.context.scene.objects:
        if ob.type != "MESH":
            continue
        me = ob.data
        key_of = {}
        remap = []
        for v in me.vertices:
            k = (round(v.co.x, 4), round(v.co.y, 4), round(v.co.z, 4))
            if k not in key_of:
                key_of[k] = len(key_of)
            remap.append(key_of[k])
        edge_use = defaultdict(int)
        degenerate = 0
        for poly in me.polygons:
            idx = [remap[i] for i in poly.vertices]
            if len(set(idx)) < 3:
                degenerate += 1
                continue
            for i in range(len(idx)):
                e = tuple(sorted((idx[i], idx[(i + 1) % len(idx)])))
                edge_use[e] += 1
        boundary = sum(1 for c in edge_use.values() if c == 1)
        nonmanifold = sum(1 for c in edge_use.values() if c > 2)
        ok = boundary == 0 and nonmanifold == 0 and degenerate == 0
        file_ok = file_ok and ok
        print(f"{'PASS' if ok else 'FAIL'} {path} :: {ob.name} "
              f"boundary={boundary} nonmanifold={nonmanifold} "
              f"degenerate={degenerate}")
    if not file_ok:
        failures += 1

print(f"WATERTIGHT GATE: {'PASS' if failures == 0 else f'FAIL ({failures} file(s))'}")
sys.exit(1 if failures else 0)
