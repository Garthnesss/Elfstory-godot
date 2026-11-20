#!/usr/bin/env python3
"""
ElfStory Reborn — V2 Track B: Organic High-Poly TRELLIS.2 -> QuadriFlow Retopo -> Cycles Bake Pipeline.

Replaces naive V1 DECIMATE + flat shading (which produced crumpled origami triangles
along fragmented xatlas UV seams) with a studio-grade game-art workflow in Blender 5.3+:
  1. Clean high-poly TRELLIS.2 sculpt (remove floating islands, trim underside spikes,
     normalize scale & bottom-center pivot at Z = 0, smooth high-poly normals).
  2. Duplicate to LowPoly, run a fine watertight Voxel manifold pre-pass, and run
     QuadriFlow Remesh (~2,500 evenly-spaced quads, 0 triangles, 0 non-manifold edges).
  3. Project LowPoly onto HighPoly via Shrinkwrap (NEAREST_SURFACEPOINT) to preserve
     crisp silhouettes (bills, eyes, webbed toes, tails, roots, foliage).
  4. Unwrap LowPoly with Clean Smart UV Project (angle_limit=66 deg, island_margin=0.02 ->
     ~15-30 contiguous UV islands instead of ~17,000 xatlas shards).
  5. Cycles Selected-to-Active GPU Bake (1024x1024, cage_extrusion=0.03):
     - Diffuse (Color-only unlit albedo, with HighPoly Metallic temporarily zeroed so
       Cycles never bakes metallic areas as black)
     - Tangent-Space Normal Map (recovers all 200K-tri surface detail onto the 2.5K-quad mesh)
     - Ambient Occlusion (softly blended into albedo with shadow-delighting lift)
  6. Apply Smooth Shading + Weighted Normals (FACE_AREA, keep_sharp=True) and stylized
     matte organic PBR material (Metallic <= 0.15, Roughness >= 0.65).
  7. Export WebP-compressed .glb (< 600 KB) and save .blend source.

Usage (headless CLI):
  /Applications/Blender.app/Contents/MacOS/Blender --background --python \
    tools/blender_retopo_bake.py -- \
    --input ~/GLocalLM/workspace/3d/oakhaven_platypus-20261119-225817.glb \
    --output public/models/oakhaven_platypus.glb \
    --blend docs/models/oakhaven_platypus.blend \
    --name Oakhaven_Platypus \
    --target-quads 2500 \
    --tex-size 1024 \
    --target-width 1.0
"""
import argparse
import math
import os
import sys
import bpy
import bmesh
import numpy as np


def _parse_args():
    argv = sys.argv
    if "--" in argv:
        argv = argv[argv.index("--") + 1:]
    else:
        argv = []
    p = argparse.ArgumentParser(description="V2 Track B: QuadriFlow Retopo + Cycles Bake for TRELLIS.2 assets.")
    p.add_argument("--input", required=True, help="Path to raw high-poly TRELLIS.2 .glb")
    p.add_argument("--output", required=True, help="Path to game-ready output .glb")
    p.add_argument("--blend", default="", help="Optional path to save .blend")
    p.add_argument("--name", default="", help="Object name inside Blender")
    p.add_argument("--target-quads", type=int, default=2500, help="Target quad count for QuadriFlow (default: 2500)")
    p.add_argument("--tex-size", type=int, default=1024, help="Baked texture resolution in px (default: 1024)")
    p.add_argument("--target-width", type=float, default=1.0, help="Normalized max horizontal span in meters (default: 1.0)")
    p.add_argument("--rotate-z", type=float, default=0.0, help="Optional Z rotation in degrees after import")
    p.add_argument("--cage-extrusion", type=float, default=0.03, help="Cycles bake cage extrusion (default: 0.03)")
    p.add_argument("--roughness", type=float, default=0.72, help="Clamped matte roughness >= 0.65 (default: 0.72)")
    p.add_argument("--metallic", type=float, default=0.05, help="Clamped metallic <= 0.15 (default: 0.05)")
    p.add_argument("--image-quality", type=int, default=80, help="WebP export image quality 1-100 (default: 80)")
    p.add_argument("--symmetry", action="store_true", help="Enable X-axis symmetry in QuadriFlow (only for symmetric T-poses)")
    return p.parse_args(argv)


def _remove_tiny_islands(obj, min_face_ratio=0.005, keep_largest_only=False):
    """Remove disconnected floating artifacts or interior voxel cavities."""
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bm.faces.ensure_lookup_table()
    total_faces = len(bm.faces)
    if total_faces < 20:
        bm.free()
        return 0

    visited = set()
    islands = []
    for f in bm.faces:
        if f.index in visited:
            continue
        stack = [f]
        comp = []
        visited.add(f.index)
        while stack:
            cur = stack.pop()
            comp.append(cur)
            for e in cur.edges:
                for lf in e.link_faces:
                    if lf.index not in visited:
                        visited.add(lf.index)
                        stack.append(lf)
        islands.append(comp)

    if len(islands) <= 1:
        bm.free()
        return 0

    islands.sort(key=len, reverse=True)
    threshold = max(8, int(total_faces * min_face_ratio))
    to_delete = []
    removed_islands = 0
    for comp in islands[1:]:
        if keep_largest_only or len(comp) < threshold:
            to_delete.extend(comp)
            removed_islands += 1

    if to_delete:
        bmesh.ops.delete(bm, geom=to_delete, context='FACES')
        loose_v = [v for v in bm.verts if not v.link_faces]
        if loose_v:
            bmesh.ops.delete(bm, geom=loose_v, context='VERTS')
        bm.to_mesh(obj.data)
        obj.data.update()
    bm.free()
    return removed_islands


def _normalize_bounds(obj, target_width: float, rotate_z_deg: float = 0.0):
    """Normalize horizontal width to target_width and place bottom-center at (0, 0, 0)."""
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj

    if abs(rotate_z_deg) > 1e-4:
        obj.rotation_euler.z += math.radians(rotate_z_deg)
        bpy.ops.object.transform_apply(location=False, rotation=True, scale=False)

    coords = [v.co for v in obj.data.vertices]
    min_x = min(c.x for c in coords)
    max_x = max(c.x for c in coords)
    min_y = min(c.y for c in coords)
    max_y = max(c.y for c in coords)
    min_z = min(c.z for c in coords)

    width_x = max_x - min_x
    depth_y = max_y - min_y
    cx = (min_x + max_x) * 0.5
    cy = (min_y + max_y) * 0.5
    max_horiz = max(width_x, depth_y, 1e-4)
    scale = target_width / max_horiz

    for v in obj.data.vertices:
        v.co.x = (v.co.x - cx) * scale
        v.co.y = (v.co.y - cy) * scale
        v.co.z = (v.co.z - min_z) * scale
    obj.data.update()


def _make_watertight_manifold(obj):
    """Ensure mesh is 100% manifold with consistent normals for QuadriFlow."""
    _remove_tiny_islands(obj, keep_largest_only=True)
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=0.0002)
    bmesh.ops.dissolve_degenerate(bm, dist=0.0001, edges=bm.edges)

    for _ in range(4):
        bm.verts.ensure_lookup_table()
        bm.edges.ensure_lookup_table()
        nm_verts = [v for v in bm.verts if not v.is_manifold]
        if not nm_verts:
            break
        bmesh.ops.delete(bm, geom=nm_verts, context='VERTS')
        bndr = [e for e in bm.edges if e.is_boundary]
        if bndr:
            bmesh.ops.holes_fill(bm, edges=bndr, sides=64)

    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(obj.data)
    bm.free()
    obj.data.validate(clean_customdata=False)
    obj.data.update()


def retopo_and_bake_asset(
    input_path: str,
    output_path: str,
    blend_path: str = "",
    asset_name: str = "",
    target_quads: int = 2500,
    tex_size: int = 1024,
    target_width: float = 1.0,
    rotate_z_deg: float = 0.0,
    cage_extrusion: float = 0.03,
    roughness: float = 0.72,
    metallic: float = 0.05,
    image_quality: int = 80,
    use_symmetry: bool = False,
    clear_scene: bool = True,
):
    input_path = os.path.abspath(os.path.expanduser(input_path))
    output_path = os.path.abspath(os.path.expanduser(output_path))
    if not os.path.isfile(input_path):
        raise FileNotFoundError(f"Input GLB not found: {input_path}")

    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    if blend_path:
        blend_path = os.path.abspath(os.path.expanduser(blend_path))
        os.makedirs(os.path.dirname(blend_path), exist_ok=True)

    # Clamp stylized organic PBR parameters per V2 spec
    roughness = max(0.65, min(1.0, float(roughness)))
    metallic = max(0.0, min(0.15, float(metallic)))

    if clear_scene:
        for o in list(bpy.data.objects):
            bpy.data.objects.remove(o, do_unlink=True)

    # 1. Import raw high-poly TRELLIS.2 GLB
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=input_path)
    imported = [o for o in bpy.data.objects if o not in before]
    mesh_objs = [o for o in imported if o.type == 'MESH']
    if not mesh_objs:
        raise RuntimeError(f"No mesh objects found in {input_path}")

    bpy.ops.object.select_all(action='DESELECT')
    for o in mesh_objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = mesh_objs[0]
    if len(mesh_objs) > 1:
        bpy.ops.object.join()

    high_obj = bpy.context.active_object
    high_obj.parent = None
    for o in imported:
        if o != high_obj and o.name in bpy.data.objects:
            bpy.data.objects.remove(o, do_unlink=True)

    stem = asset_name or os.path.splitext(os.path.basename(output_path))[0]
    high_obj.name = f"{stem}_HighPoly"
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    if abs(rotate_z_deg) > 1e-4:
        high_obj.rotation_euler.z += math.radians(rotate_z_deg)
        bpy.ops.object.transform_apply(location=False, rotation=True, scale=False)

    raw_tris = sum(max(1, len(p.vertices) - 2) for p in high_obj.data.polygons)

    # 2. Duplicate to LowPoly first, weld UV seams on LowPoly, and strip floating islands
    #    so HighPoly's imported glTF custom normals and xatlas UV islands remain 100% untouched!
    low_obj = high_obj.copy()
    low_obj.data = high_obj.data.copy()
    low_obj.name = stem
    bpy.context.collection.objects.link(low_obj)

    bpy.ops.object.select_all(action='DESELECT')
    low_obj.select_set(True)
    bpy.context.view_layer.objects.active = low_obj

    bm = bmesh.new()
    bm.from_mesh(low_obj.data)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=0.0002)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(low_obj.data)
    bm.free()
    low_obj.data.update()

    removed_islands = _remove_tiny_islands(low_obj, min_face_ratio=0.005, keep_largest_only=True)

    # Compute normalized bounds from cleaned LowPoly and apply identically to both HighPoly & LowPoly
    coords = [v.co for v in low_obj.data.vertices]
    min_x, max_x = min(c.x for c in coords), max(c.x for c in coords)
    min_y, max_y = min(c.y for c in coords), max(c.y for c in coords)
    zs = sorted(c.z for c in coords)
    floor_z = zs[max(0, int(len(zs) * 0.003))] if len(zs) > 100 else zs[0]
    cx = (min_x + max_x) * 0.5
    cy = (min_y + max_y) * 0.5
    max_horiz = max(max_x - min_x, max_y - min_y, 1e-4)
    scale = target_width / max_horiz

    for target_mesh in (high_obj.data, low_obj.data):
        for v in target_mesh.vertices:
            z_clamped = max(v.co.z, floor_z)
            v.co.x = (v.co.x - cx) * scale
            v.co.y = (v.co.y - cy) * scale
            v.co.z = (z_clamped - floor_z) * scale
        target_mesh.update()

    for p in high_obj.data.polygons:
        p.use_smooth = True
    high_obj.data.update()

    max_dim = max(low_obj.dimensions.x, low_obj.dimensions.y, low_obj.dimensions.z, 0.1)
    rm = low_obj.modifiers.new("ManifoldVoxelPrepass", 'REMESH')
    rm.mode = 'VOXEL'
    rm.voxel_size = max_dim * 0.0075
    rm.adaptivity = 0.0
    bpy.ops.object.modifier_apply(modifier=rm.name)

    _make_watertight_manifold(low_obj)

    # Scale temporarily by 10x so all voxel face areas exceed QuadriFlow's C++ 1e-7 cross-product threshold
    low_obj.scale = (10.0, 10.0, 10.0)
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)

    qf_res = bpy.ops.object.quadriflow_remesh(
        use_mesh_symmetry=use_symmetry,
        use_preserve_sharp=True,
        use_preserve_boundary=False,
        preserve_attributes=False,
        smooth_normals=True,
        mode='FACES',
        target_faces=target_quads,
        seed=0,
    )
    if 'FINISHED' not in qf_res:
        # Retry with preserve_sharp=False if a sharp feature corner resisted
        qf_res = bpy.ops.object.quadriflow_remesh(
            use_mesh_symmetry=False,
            use_preserve_sharp=False,
            use_preserve_boundary=False,
            preserve_attributes=False,
            smooth_normals=True,
            mode='FACES',
            target_faces=target_quads,
            seed=0,
        )

    low_obj.scale = (0.1, 0.1, 0.1)
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)

    # 3. Shrinkwrap LowPoly onto HighPoly to snap silhouette details crisply
    sw = low_obj.modifiers.new("ShrinkwrapToHigh", 'SHRINKWRAP')
    sw.target = high_obj
    sw.wrap_method = 'NEAREST_SURFACEPOINT'
    sw.wrap_mode = 'ON_SURFACE'
    bpy.ops.object.modifier_apply(modifier=sw.name)

    # Re-pin bottom-most vertices cleanly at Z = 0 after shrinkwrap
    min_z = min(v.co.z for v in low_obj.data.vertices)
    if abs(min_z) > 1e-5:
        for v in low_obj.data.vertices:
            v.co.z -= min_z
        low_obj.data.update()
        for v in high_obj.data.vertices:
            v.co.z -= min_z
        high_obj.data.update()

    # 4. Clean Smart UV Project (angle_limit=66 deg, island_margin=0.02)
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.uv.smart_project(
        angle_limit=math.radians(66.0),
        island_margin=0.02,
        correct_aspect=True,
        scale_to_bounds=False,
    )
    bpy.ops.object.mode_set(mode='OBJECT')

    for p in low_obj.data.polygons:
        p.use_smooth = True
    low_obj.data.update()

    # 5. Cycles Selected-to-Active Bake (Diffuse Color-only + Tangent Normal + AO)
    saved_metal_links = []
    for m in high_obj.data.materials:
        if m and m.use_nodes:
            nt_h = m.node_tree
            bsdf_h = next((n for n in nt_h.nodes if n.type == 'BSDF_PRINCIPLED'), None)
            if bsdf_h:
                metal_sock = bsdf_h.inputs.get("Metallic")
                if metal_sock:
                    for l in list(metal_sock.links):
                        saved_metal_links.append((nt_h, l.from_socket, metal_sock))
                        nt_h.links.remove(l)
                    metal_sock.default_value = 0.0

    alb_name = f"{stem}_Albedo"
    nrm_name = f"{stem}_Normal"
    ao_name = f"{stem}_AO_Tmp"
    for img_n in (alb_name, nrm_name, ao_name):
        if img_n in bpy.data.images:
            bpy.data.images.remove(bpy.data.images[img_n])

    img_albedo = bpy.data.images.new(alb_name, width=tex_size, height=tex_size, alpha=False, float_buffer=False)
    img_albedo.colorspace_settings.name = 'sRGB'

    img_normal = bpy.data.images.new(nrm_name, width=tex_size, height=tex_size, alpha=False, float_buffer=True)
    img_normal.colorspace_settings.name = 'Non-Color'

    img_ao = bpy.data.images.new(ao_name, width=tex_size, height=tex_size, alpha=False, float_buffer=False)
    img_ao.colorspace_settings.name = 'Non-Color'

    mat = bpy.data.materials.new(name=f"{stem}_Mat")
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()

    out_node = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf_node = nt.nodes.new("ShaderNodeBsdfPrincipled")
    bsdf_node.inputs["Roughness"].default_value = roughness
    bsdf_node.inputs["Metallic"].default_value = metallic
    nt.links.new(bsdf_node.outputs["BSDF"], out_node.inputs["Surface"])

    node_alb = nt.nodes.new("ShaderNodeTexImage")
    node_alb.image = img_albedo

    node_nrm = nt.nodes.new("ShaderNodeTexImage")
    node_nrm.image = img_normal

    node_ao = nt.nodes.new("ShaderNodeTexImage")
    node_ao.image = img_ao

    low_obj.data.materials.clear()
    low_obj.data.materials.append(mat)

    scene = bpy.context.scene
    prev_engine = scene.render.engine
    scene.render.engine = 'CYCLES'
    scene.cycles.device = 'GPU'
    scene.cycles.samples = 16
    scene.cycles.use_denoising = False

    bake = scene.render.bake
    bake.use_selected_to_active = True
    bake.use_pass_direct = False
    bake.use_pass_indirect = False
    bake.use_pass_color = True
    bake.cage_extrusion = cage_extrusion
    bake.max_ray_distance = cage_extrusion * 2.0
    bake.margin = 8
    bake.margin_type = 'EXTEND'

    def _run_bake(target_node, bake_type, pass_filter=None):
        bpy.ops.object.select_all(action='DESELECT')
        high_obj.select_set(True)
        low_obj.select_set(True)
        bpy.context.view_layer.objects.active = low_obj
        for n in nt.nodes:
            n.select = False
        target_node.select = True
        nt.nodes.active = target_node
        if pass_filter:
            bpy.ops.object.bake(
                type=bake_type,
                pass_filter=pass_filter,
                use_selected_to_active=True,
                cage_extrusion=cage_extrusion,
                max_ray_distance=cage_extrusion * 2.0,
                margin=8,
            )
        else:
            bpy.ops.object.bake(
                type=bake_type,
                use_selected_to_active=True,
                cage_extrusion=cage_extrusion,
                max_ray_distance=cage_extrusion * 2.0,
                margin=8,
            )

    _run_bake(node_alb, 'DIFFUSE', {'COLOR'})
    _run_bake(node_nrm, 'NORMAL')
    scene.cycles.samples = 24
    _run_bake(node_ao, 'AO')

    for link_nt, from_s, to_s in saved_metal_links:
        link_nt.links.new(from_s, to_s)

    # Delight deep shadow troughs & softly blend baked AO into Albedo
    alb_px = np.array(img_albedo.pixels[:], dtype=np.float32).reshape((-1, 4))
    ao_px = np.array(img_ao.pixels[:], dtype=np.float32).reshape((-1, 4))
    rgb = alb_px[:, :3]
    ao_val = np.clip(ao_px[:, 0:1], 0.0, 1.0)
    luma = 0.2126 * rgb[:, 0:1] + 0.7152 * rgb[:, 1:2] + 0.0722 * rgb[:, 2:3]
    shadow_lift = np.where(luma > 1e-4, np.power(np.clip(luma, 0.0, 1.0), 0.85) / np.maximum(luma, 1e-4), 1.0)
    rgb = np.clip(rgb * shadow_lift, 0.0, 1.0)
    ao_soft = 0.78 + 0.22 * ao_val
    rgb = np.clip(rgb * ao_soft, 0.0, 1.0)
    alb_px[:, :3] = rgb
    img_albedo.pixels[:] = alb_px.ravel().tolist()
    img_albedo.update()
    img_albedo.pack()

    img_normal.pack()
    nt.nodes.remove(node_ao)
    bpy.data.images.remove(img_ao)

    nrm_map_node = nt.nodes.new("ShaderNodeNormalMap")
    nrm_map_node.inputs["Strength"].default_value = 1.0
    nt.links.new(node_alb.outputs["Color"], bsdf_node.inputs["Base Color"])
    nt.links.new(node_nrm.outputs["Color"], nrm_map_node.inputs["Color"])
    nt.links.new(nrm_map_node.outputs["Normal"], bsdf_node.inputs["Normal"])

    scene.render.engine = prev_engine

    # 6. Delete HighPoly & purge orphan images/meshes so .blend and .glb stay ultra-compact
    bpy.data.objects.remove(high_obj, do_unlink=True)
    for img in list(bpy.data.images):
        if img not in (img_albedo, img_normal) and img.users == 0:
            bpy.data.images.remove(img)

    # Apply Smooth Shading + Weighted Normals on LowPoly
    bpy.ops.object.select_all(action='DESELECT')
    low_obj.select_set(True)
    bpy.context.view_layer.objects.active = low_obj
    wn = low_obj.modifiers.new("WeightedNormal", 'WEIGHTED_NORMAL')
    wn.mode = 'FACE_AREA'
    wn.weight = 50
    wn.keep_sharp = True
    bpy.ops.object.modifier_apply(modifier=wn.name)

    final_quads = sum(1 for p in low_obj.data.polygons if len(p.vertices) == 4)
    final_tris = sum(1 for p in low_obj.data.polygons if len(p.vertices) == 3)
    final_verts = len(low_obj.data.vertices)

    # 7. Save .blend and export WebP-textured .glb
    if blend_path:
        bpy.ops.wm.save_as_mainfile(filepath=blend_path)

    bpy.ops.export_scene.gltf(
        filepath=output_path,
        export_format='GLB',
        use_selection=True,
        export_apply=True,
        export_materials='EXPORT',
        export_image_format='WEBP',
        export_image_quality=int(image_quality),
        export_yup=True,
    )

    raw_kb = os.path.getsize(input_path) / 1024.0
    out_kb = os.path.getsize(output_path) / 1024.0
    summary = (
        f"V2 Retopo+Bake '{stem}': {raw_tris:,} raw tris -> {final_quads:,} quads ({final_tris} tris, {final_verts:,} verts) | "
        f"{raw_kb:.0f} KB -> {out_kb:.1f} KB | "
        f"removed {removed_islands} floating islands | baked {tex_size}px Albedo+AO & Normal | "
        f"saved {output_path}"
    )
    print(summary)
    return summary


if __name__ == "__main__":
    args = _parse_args()
    retopo_and_bake_asset(
        input_path=args.input,
        output_path=args.output,
        blend_path=args.blend,
        asset_name=args.name,
        target_quads=args.target_quads,
        tex_size=args.tex_size,
        target_width=args.target_width,
        rotate_z_deg=args.rotate_z,
        cage_extrusion=args.cage_extrusion,
        roughness=args.roughness,
        metallic=args.metallic,
        image_quality=args.image_quality,
        use_symmetry=args.symmetry,
        clear_scene=True,
    )
