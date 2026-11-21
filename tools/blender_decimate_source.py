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
    p.add_argument("--ratio", type=float, required=True, help="kept share of faces, e.g. 0.12")
    p.add_argument("--name", default="")
    p.add_argument("--image-quality", type=int, default=82)
    p.add_argument("--rebake", action="store_true",
                   help="give the decimated mesh clean Smart UVs and bake the source colour onto them")
    p.add_argument("--tex-size", type=int, default=1024)
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

    before = sum(len(p.vertices) - 2 for p in obj.data.polygons)
    high = None
    if a.rebake:
        high = obj.copy()
        high.data = obj.data.copy()
        bpy.context.scene.collection.objects.link(high)
    mod = obj.modifiers.new("decimate", "DECIMATE")
    mod.decimate_type = "COLLAPSE"
    mod.ratio = a.ratio
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


def rebake(low, high, tex_size):
    """--rebake: the generator's xatlas UVs are thousands of tiny islands;
    decimating across them smears neighbouring atlas colour into flecks
    (orange triangles on the platypus's grey back, Nov 21). Instead give
    the decimated mesh clean Smart UVs and bake the source's colour onto
    it, Selected-to-Active, so the shape keeps the decimation and the
    texture keeps the source."""
    # Metallic bakes black in a DIFFUSE colour pass: zero it on the source.
    for m in high.data.materials:
        if m and m.use_nodes:
            for n in m.node_tree.nodes:
                if n.type == "BSDF_PRINCIPLED":
                    sock = n.inputs["Metallic"]
                    for l in list(sock.links):
                        m.node_tree.links.remove(l)
                    sock.default_value = 0.0

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

    img = bpy.data.images.new("Albedo", width=tex_size, height=tex_size, alpha=False)
    img.colorspace_settings.name = "sRGB"
    mat = bpy.data.materials.new("Baked")
    mat.use_nodes = True
    nt = mat.node_tree
    bsdf = next(n for n in nt.nodes if n.type == "BSDF_PRINCIPLED")
    bsdf.inputs["Roughness"].default_value = 0.75
    bsdf.inputs["Metallic"].default_value = 0.0
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    for n in nt.nodes:
        n.select = False
    tex.select = True
    nt.nodes.active = tex
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
    # The meshes nearly coincide (decimation moves verts ~mm), so a short
    # cage ray is enough and avoids picking up the far side of thin toes.
    bpy.ops.object.bake(type="DIFFUSE", pass_filter={"COLOR"}, use_selected_to_active=True,
                        cage_extrusion=0.008, max_ray_distance=0.016, margin=8)
    img.pack()
    bpy.data.objects.remove(high, do_unlink=True)
    bpy.ops.object.select_all(action="DESELECT")
    low.select_set(True)
    bpy.context.view_layer.objects.active = low


main()
