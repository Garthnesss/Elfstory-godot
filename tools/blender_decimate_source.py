"""UV-preserving decimation of a raw TRELLIS.2 .glb (the "source-first
textured" route in docs/asset-generation-pipeline.md).

Keeps the generator's own texture and UVs: a Collapse decimate delimited
at UV seams, smooth shading, origin moved to bottom-center at Z = 0,
WebP textures in the exported .glb. No retopo, no bake, so thin detail
(toes, claws, bills) survives — at the cost of strict watertightness
(TRELLIS shells have open edges; see tools/ASSET_GATE.md).

  Blender --background --python tools/blender_decimate_source.py -- \
    --input ~/GLocalLM/workspace/3d/<slug>-<stamp>.glb \
    --output godot/assets/models/<name>.glb --ratio 0.12 --name <Name>
"""
import argparse
import math
import sys

import bpy
from mathutils import Vector


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    p = argparse.ArgumentParser()
    p.add_argument("--input", required=True)
    p.add_argument("--output", required=True)
    p.add_argument("--ratio", type=float, default=0.0, help="kept share of faces, e.g. 0.12")
    p.add_argument("--target-tris", type=int, default=0, help="instead of --ratio: aim for this many triangles")
    p.add_argument("--name", default="")
    p.add_argument("--image-quality", type=int, default=82)
    p.add_argument("--rebake", action="store_true",
                   help="give the decimated mesh clean Smart UVs and bake the source colour onto them")
    p.add_argument("--tex-size", type=int, default=1024)
    p.add_argument("--cut-base", action="store_true",
                   help="slice off a sculpted display base (miniature figures stand on the game's own plinth)")
    p.add_argument("--cut-z", type=float, default=-1.0,
                   help="with --cut-base: cut height as a fraction of model height (default: auto-detect)")
    return p.parse_args(argv)


def main():
    a = parse_args()
    bpy.ops.wm.read_factory_settings(use_empty=True)
    # merge_vertices: glTF stores a split vertex at every UV seam. Without
    # welding, each UV island decimates on its own and the island borders
    # pull apart into cracks; welded, the seams stay as UV data on one
    # connected surface and delimit={"UV"} keeps them intact.
    bpy.ops.import_scene.gltf(filepath=a.input, merge_vertices=True)
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    bpy.ops.object.select_all(action="DESELECT")
    for o in meshes:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    if len(meshes) > 1:
        bpy.ops.object.join()
    obj = bpy.context.view_layer.objects.active
    # Bake parent transforms into the mesh, then drop the empties.
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    for o in list(bpy.context.scene.objects):
        if o is not obj:
            bpy.data.objects.remove(o, do_unlink=True)

    if a.cut_base:
        cut_base(obj, a.cut_z)
    before = sum(len(p.vertices) - 2 for p in obj.data.polygons)
    high = None
    if a.rebake:
        high = obj.copy()
        high.data = obj.data.copy()
        bpy.context.scene.collection.objects.link(high)
    mod = obj.modifiers.new("decimate", "DECIMATE")
    mod.decimate_type = "COLLAPSE"
    ratio = a.ratio
    if a.target_tris > 0:
        ratio = min(1.0, a.target_tris / max(before, 1))
    if ratio <= 0.0:
        raise SystemExit("give --ratio or --target-tris")
    a.ratio = ratio
    mod.ratio = ratio
    mod.delimit = {"UV"}
    bpy.ops.object.modifier_apply(modifier=mod.name)
    # Smooth only gentle angles: the stylized faceting is in the geometry,
    # and full smooth shading melts its ridges into soft creases.
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(35.0))

    # Bottom-center origin at Z = 0.
    ws = [obj.matrix_world @ v.co for v in obj.data.vertices]
    mn = Vector((min(v.x for v in ws), min(v.y for v in ws), min(v.z for v in ws)))
    mx = Vector((max(v.x for v in ws), max(v.y for v in ws), max(v.z for v in ws)))
    shift = Vector(((mn.x + mx.x) / 2, (mn.y + mx.y) / 2, mn.z))
    for v in obj.data.vertices:
        v.co -= shift
    if high is not None:
        for v in high.data.vertices:
            v.co -= shift
        rebake(obj, high, a.tex_size)
    if a.name:
        obj.name = a.name
        obj.data.name = a.name
    after = sum(len(p.vertices) - 2 for p in obj.data.polygons)

    bpy.ops.export_scene.gltf(
        filepath=a.output,
        export_format="GLB",
        export_image_format="WEBP",
        export_image_quality=a.image_quality,
        export_yup=True,
        export_apply=True,
        use_selection=True,
    )
    print(f"DECIMATED {a.input} -> {a.output} tris {before} -> {after} ratio {a.ratio}")


def cut_base(obj, frac):
    """Miniature concepts come with a sculpted display base. Find its top
    (the lowest height where the cross-section narrows from base width to
    the feet/legs) and bisect everything below away, capping the cut."""
    ws = [obj.matrix_world @ v.co for v in obj.data.vertices]
    zmin = min(v.z for v in ws)
    zmax = max(v.z for v in ws)
    h = zmax - zmin
    if frac < 0.0:
        steps = 80
        band = h * 0.3 / steps
        prof = []
        for i in range(steps):
            z0 = zmin + i * band
            pts = [v for v in ws if z0 <= v.z < z0 + band]
            if len(pts) < 8:
                prof.append(0.0)
                continue
            prof.append(max(max(p.x for p in pts) - min(p.x for p in pts),
                            max(p.y for p in pts) - min(p.y for p in pts)))
        base_w = max(prof[:8]) if prof else 0.0
        cut_i = None
        for i in range(2, steps):
            if prof[i] and prof[i] < base_w * 0.8:
                cut_i = i
                break
        if cut_i is None:
            print("CUT_BASE no base detected; nothing cut")
            return
        cut = zmin + (cut_i + 1.5) * band
        print("CUT_BASE profile", [round(w / max(base_w, 1e-6), 2) for w in prof[:cut_i + 4]])
    else:
        cut = zmin + frac * h
    print(f"CUT_BASE at z={cut:.4f} ({(cut - zmin) / h:.3f} of height)")
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.bisect(plane_co=(0.0, 0.0, cut), plane_no=(0.0, 0.0, 1.0),
                        clear_inner=True, use_fill=True)
    bpy.ops.object.mode_set(mode="OBJECT")
    # Base debris above the cut (grass tufts, pebbles) survives as small
    # loose islands sitting right on the cut. Drop pieces that lie wholly in
    # the bottom 20% and are under 8% of the faces; small islands higher up
    # are real detail (eyes, buckles) and stay.
    import bmesh
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bm.faces.ensure_lookup_table()
    seen = set()
    islands = []
    for f in bm.faces:
        if f.index in seen:
            continue
        stack, comp = [f], []
        seen.add(f.index)
        while stack:
            cur = stack.pop()
            comp.append(cur)
            for e in cur.edges:
                for nf in e.link_faces:
                    if nf.index not in seen:
                        seen.add(nf.index)
                        stack.append(nf)
        islands.append(comp)
    total = len(bm.faces)
    low_band = cut + 0.2 * h
    mw = obj.matrix_world

    def is_debris(comp):
        if len(comp) >= total * 0.08:
            return False
        return max((mw @ v.co).z for f in comp for v in f.verts) < low_band
    doomed = [f for comp in islands if is_debris(comp) for f in comp]
    bmesh.ops.delete(bm, geom=doomed, context="FACES")
    bm.to_mesh(obj.data)
    bm.free()
    print(f"CUT_BASE removed {len(doomed)} faces of debris from {len(islands)} islands")


def rebake(low, high, tex_size):
    """--rebake: the generator's xatlas UVs are thousands of tiny islands;
    decimating across them smears neighbouring atlas colour into flecks
    (orange triangles on the platypus's grey back, Nov 21). Instead give
    the decimated mesh clean Smart UVs and bake the source onto it,
    Selected-to-Active, so the shape keeps the decimation and the texture
    keeps the source. Bakes base colour plus metallic and roughness (the
    Nov 22 figure review: colour-only bakes turned polished pauldrons and
    blades into chalky plaster)."""
    bpy.ops.object.select_all(action="DESELECT")
    low.select_set(True)
    bpy.context.view_layer.objects.active = low
    while low.data.uv_layers:
        low.data.uv_layers.remove(low.data.uv_layers[0])
    low.data.uv_layers.new(name="UVMap")
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=math.radians(66.0), island_margin=0.01,
                             correct_aspect=True, scale_to_bounds=False)
    bpy.ops.object.mode_set(mode="OBJECT")

    def new_img(name, size, color):
        im = bpy.data.images.new(name, width=size, height=size, alpha=False)
        im.colorspace_settings.name = "sRGB" if color else "Non-Color"
        return im

    mr_size = max(256, tex_size // 2)
    img = new_img("Albedo", tex_size, True)
    img_r = new_img("Roughness", mr_size, False)
    img_m = new_img("Metallic", mr_size, False)
    mat = bpy.data.materials.new("Baked")
    mat.use_nodes = True
    nt = mat.node_tree
    bsdf = next(n for n in nt.nodes if n.type == "BSDF_PRINCIPLED")
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    tex_r = nt.nodes.new("ShaderNodeTexImage")
    tex_r.image = img_r
    tex_m = nt.nodes.new("ShaderNodeTexImage")
    tex_m.image = img_m
    nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    nt.links.new(tex_r.outputs["Color"], bsdf.inputs["Roughness"])
    nt.links.new(tex_m.outputs["Color"], bsdf.inputs["Metallic"])
    low.data.materials.clear()
    low.data.materials.append(mat)

    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.device = "GPU"
    scene.cycles.samples = 16
    scene.render.bake.margin = 8
    bpy.ops.object.select_all(action="DESELECT")
    high.select_set(True)
    low.select_set(True)
    bpy.context.view_layer.objects.active = low

    def bake_into(node, kind, **kw):
        for n in nt.nodes:
            n.select = False
        node.select = True
        nt.nodes.active = node
        # The meshes nearly coincide (decimation moves verts ~mm), so a short
        # cage ray is enough and avoids picking up the far side of thin toes.
        bpy.ops.object.bake(type=kind, use_selected_to_active=True,
                            cage_extrusion=0.008, max_ray_distance=0.016, margin=8, **kw)

    src_mats = [m for m in high.data.materials if m and m.use_nodes]
    bake_into(tex_r, "ROUGHNESS")

    # Metallic has no bake pass: route it through an emission shader.
    restore = []
    for m in src_mats:
        snt = m.node_tree
        p = next((n for n in snt.nodes if n.type == "BSDF_PRINCIPLED"), None)
        out = next((n for n in snt.nodes if n.type == "OUTPUT_MATERIAL"), None)
        if p is None or out is None:
            continue
        old = [l.from_socket for l in out.inputs["Surface"].links]
        em = snt.nodes.new("ShaderNodeEmission")
        sock = p.inputs["Metallic"]
        if sock.links:
            snt.links.new(sock.links[0].from_socket, em.inputs["Color"])
        else:
            v = sock.default_value
            em.inputs["Color"].default_value = (v, v, v, 1.0)
        snt.links.new(em.outputs["Emission"], out.inputs["Surface"])
        restore.append((snt, out, old, em))
    bake_into(tex_m, "EMIT")
    for snt, out, old, em in restore:
        snt.nodes.remove(em)
        if old:
            snt.links.new(old[0], out.inputs["Surface"])

    # Metallic bakes black in a DIFFUSE colour pass: zero it on the source.
    for m in src_mats:
        for n in m.node_tree.nodes:
            if n.type == "BSDF_PRINCIPLED":
                sock = n.inputs["Metallic"]
                for l in list(sock.links):
                    m.node_tree.links.remove(l)
                sock.default_value = 0.0
    bake_into(tex, "DIFFUSE", pass_filter={"COLOR"})
    for im in (img, img_r, img_m):
        im.pack()
    bpy.data.objects.remove(high, do_unlink=True)
    bpy.ops.object.select_all(action="DESELECT")
    low.select_set(True)
    bpy.context.view_layer.objects.active = low


main()
