#!/usr/bin/env python3
"""
ElfStory Reborn — V2 Track A: Modular Stylized Low-Poly Architectural Kitbasher (No TRELLIS.2).

Builds clean quad-topology buildings and hard-surface props directly in Blender 5.3+
with contiguous Box/Smart UVs, beveled edges, weighted normals, bottom-center origin
at Z = 0, and seamless tileable PBR materials (Albedo + Normal + Roughness from
`tools/comfy_runner.py --mode pbr-texture`).

Budget: 1,500–4,000 clean quads, < 500 KB .glb, zero xatlas UV seam bleed.

Usage (headless CLI):
  /Applications/Blender.app/Contents/MacOS/Blender --background --python \
    tools/blender_arch_kit.py -- \
    --preset apothecary \
    --output public/models/mira_apothecary_kit.glb \
    --blend docs/models/mira_apothecary_kit.blend \
    --texture-dir public/textures

Can also be imported in Blender MCP via:
  from blender_arch_kit import build_arch_asset
"""
import argparse
import math
import os
import sys
import bpy
import bmesh


def _parse_args():
    argv = sys.argv
    if "--" in argv:
        argv = argv[argv.index("--") + 1:]
    else:
        argv = []
    p = argparse.ArgumentParser(description="V2 Track A: Modular Stylized Low-Poly Architectural Kitbasher.")
    p.add_argument(
        "--preset",
        choices=["blacksmith", "apothecary", "cottage", "market_stall", "west_gate"],
        default="cottage",
        help="Architectural preset to build",
    )
    p.add_argument("--output", required=True, help="Output .glb path")
    p.add_argument("--blend", default="", help="Optional .blend output path")
    p.add_argument("--name", default="", help="Object name inside Blender")
    p.add_argument("--texture-dir", default="public/textures", help="Directory containing seamless PBR textures")
    p.add_argument("--target-width", type=float, default=3.2, help="Normalized horizontal footprint width in meters")
    return p.parse_args(argv)


def apply_pbr_texture_material(
    mat_name: str,
    base_color=(0.8, 0.75, 0.65, 1.0),
    roughness: float = 0.82,
    metallic: float = 0.0,
    emission_color=None,
    emission_strength: float = 0.0,
    albedo_path: str = "",
    normal_path: str = "",
    roughness_path: str = "",
    uv_scale: float = 1.0,
):
    """Create or update a clean Principled BSDF material with optional tileable PBR maps."""
    mat = bpy.data.materials.get(mat_name) or bpy.data.materials.new(name=mat_name)
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()

    out_node = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["Base Color"].default_value = base_color
    bsdf.inputs["Roughness"].default_value = roughness
    bsdf.inputs["Metallic"].default_value = metallic
    if emission_color is not None and emission_strength > 0:
        if "Emission Color" in bsdf.inputs:
            bsdf.inputs["Emission Color"].default_value = emission_color
        if "Emission Strength" in bsdf.inputs:
            bsdf.inputs["Emission Strength"].default_value = emission_strength
    nt.links.new(bsdf.outputs["BSDF"], out_node.inputs["Surface"])

    # Attach Mapping + Texture Coordinate if any texture path exists on disk
    has_alb = bool(albedo_path and os.path.isfile(albedo_path))
    has_nrm = bool(normal_path and os.path.isfile(normal_path))
    has_rgh = bool(roughness_path and os.path.isfile(roughness_path))
    if not (has_alb or has_nrm or has_rgh):
        return mat

    tex_coord = nt.nodes.new("ShaderNodeTexCoord")
    mapping = nt.nodes.new("ShaderNodeMapping")
    mapping.inputs["Scale"].default_value = (uv_scale, uv_scale, uv_scale)
    nt.links.new(tex_coord.outputs["UV"], mapping.inputs["Vector"])

    if has_alb:
        img_a = bpy.data.images.load(albedo_path, check_existing=True)
        img_a.colorspace_settings.name = 'sRGB'
        img_a.pack()
        node_a = nt.nodes.new("ShaderNodeTexImage")
        node_a.image = img_a
        nt.links.new(mapping.outputs["Vector"], node_a.inputs["Vector"])
        nt.links.new(node_a.outputs["Color"], bsdf.inputs["Base Color"])

    if has_rgh:
        img_r = bpy.data.images.load(roughness_path, check_existing=True)
        img_r.colorspace_settings.name = 'Non-Color'
        img_r.pack()
        node_r = nt.nodes.new("ShaderNodeTexImage")
        node_r.image = img_r
        nt.links.new(mapping.outputs["Vector"], node_r.inputs["Vector"])
        nt.links.new(node_r.outputs["Color"], bsdf.inputs["Roughness"])

    if has_nrm:
        img_n = bpy.data.images.load(normal_path, check_existing=True)
        img_n.colorspace_settings.name = 'Non-Color'
        img_n.pack()
        node_n = nt.nodes.new("ShaderNodeTexImage")
        node_n.image = img_n
        nrm_map = nt.nodes.new("ShaderNodeNormalMap")
        nrm_map.inputs["Strength"].default_value = 0.85
        nt.links.new(mapping.outputs["Vector"], node_n.inputs["Vector"])
        nt.links.new(node_n.outputs["Color"], nrm_map.inputs["Color"])
        nt.links.new(nrm_map.outputs["Normal"], bsdf.inputs["Normal"])

    return mat


def _add_beveled_box(
    name: str,
    size: tuple[float, float, float],
    location: tuple[float, float, float],
    mat,
    bevel_width: float = 0.025,
    subdiv_cuts: tuple[int, int, int] = (1, 1, 1),
    rotation: tuple[float, float, float] = (0.0, 0.0, 0.0),
):
    """Create a clean quad-subdivided beveled box with contiguous UVs."""
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts:
        v.co.x *= size[0]
        v.co.y *= size[1]
        v.co.z *= size[2]

    if bevel_width > 0:
        bmesh.ops.bevel(
            bm,
            geom=list(bm.edges),
            offset=min(bevel_width, min(size) * 0.22),
            segments=1,
            profile=0.5,
            affect='EDGES',
        )

    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()

    obj = bpy.data.objects.new(name, mesh)
    obj.location = location
    obj.rotation_euler = rotation
    obj.data.materials.append(mat)
    bpy.context.collection.objects.link(obj)
    return obj


def build_stone_curb_base(width: float, depth: float, height: float, mat_stone, mat_curb):
    """Build a stylized cobblestone foundation plinth with individual quad border curb stones."""
    parts = []
    plinth = _add_beveled_box(
        "Kit_StonePlinth",
        (width, depth, height),
        (0.0, 0.0, height * 0.5),
        mat_stone,
        bevel_width=0.03,
        subdiv_cuts=(3, 3, 1),
    )
    parts.append(plinth)

    # Border curb stones around perimeter
    curb_w = 0.34
    curb_h = height + 0.04
    nx = max(3, int(round(width / curb_w)))
    ny = max(3, int(round(depth / curb_w)))
    step_x = width / nx
    step_y = depth / ny

    for i in range(nx):
        cx = -width * 0.5 + (i + 0.5) * step_x
        for sy in (-depth * 0.5, depth * 0.5):
            c = _add_beveled_box(
                f"Kit_CurbX_{i}_{sy:.1f}",
                (step_x * 0.94, 0.18, curb_h),
                (cx, sy, curb_h * 0.5),
                mat_curb,
                bevel_width=0.02,
            )
            parts.append(c)

    for j in range(1, ny - 1):
        cy = -depth * 0.5 + (j + 0.5) * step_y
        for sx in (-width * 0.5, width * 0.5):
            c = _add_beveled_box(
                f"Kit_CurbY_{j}_{sx:.1f}",
                (0.18, step_y * 0.94, curb_h),
                (sx, cy, curb_h * 0.5),
                mat_curb,
                bevel_width=0.02,
            )
            parts.append(c)
    return parts


def build_timber_plaster_walls(width: float, depth: float, base_z: float, wall_h: float, mat_plaster, mat_timber, mat_stone):
    """Build quad-based half-timbered plaster walls with corner posts, beams, and stone wainscot."""
    parts = []
    # Stone wainscot lower course
    wain_h = wall_h * 0.24
    parts.append(
        _add_beveled_box(
            "Kit_Wainscot",
            (width + 0.04, depth + 0.04, wain_h),
            (0.0, 0.0, base_z + wain_h * 0.5),
            mat_stone,
            bevel_width=0.02,
            subdiv_cuts=(3, 3, 1),
        )
    )

    # Main plaster cabin volume
    parts.append(
        _add_beveled_box(
            "Kit_PlasterHull",
            (width, depth, wall_h),
            (0.0, 0.0, base_z + wall_h * 0.5),
            mat_plaster,
            bevel_width=0.02,
            subdiv_cuts=(4, 4, 2),
        )
    )

    # 4 corner timber posts + mid-wall posts
    post_t = 0.14
    for sx in (-width * 0.5, 0.0, width * 0.5):
        for sy in (-depth * 0.5, depth * 0.5):
            if sx == 0.0 and sy == -depth * 0.5:
                continue  # leave front center clear for door
            parts.append(
                _add_beveled_box(
                    f"Kit_Post_{sx:.1f}_{sy:.1f}",
                    (post_t, post_t, wall_h + 0.04),
                    (sx, sy, base_z + wall_h * 0.5),
                    mat_timber,
                    bevel_width=0.015,
                    subdiv_cuts=(1, 1, 2),
                )
            )

    # Horizontal timber ring beams (waist & top plate)
    for bz in (base_z + wain_h, base_z + wall_h - 0.06):
        for sy in (-depth * 0.5, depth * 0.5):
            parts.append(
                _add_beveled_box(
                    f"Kit_BeamX_{sy:.1f}_{bz:.2f}",
                    (width + 0.08, post_t * 0.85, 0.11),
                    (0.0, sy, bz),
                    mat_timber,
                    bevel_width=0.015,
                    subdiv_cuts=(3, 1, 1),
                )
            )
        for sx in (-width * 0.5, width * 0.5):
            parts.append(
                _add_beveled_box(
                    f"Kit_BeamY_{sx:.1f}_{bz:.2f}",
                    (post_t * 0.85, depth + 0.08, 0.11),
                    (sx, 0.0, bz),
                    mat_timber,
                    bevel_width=0.015,
                    subdiv_cuts=(1, 3, 1),
                )
            )
    return parts


def build_shingle_roof(width: float, depth: float, eave_z: float, pitch_h: float, mat_shingle, mat_shingle_alt, mat_timber):
    """Build a pitched gable roof with overlapping quad shingle rows and timber bargeboards."""
    parts = []
    overhang_x = 0.22
    overhang_y = 0.24
    span_w = width + overhang_x * 2
    half_d = depth * 0.5 + overhang_y
    slope_len = math.hypot(half_d, pitch_h)
    slope_angle = math.atan2(pitch_h, half_d)

    # Gable triangular plaster/timber infill wedges approximated with stepped quad courses
    courses = 5
    for c in range(courses):
        t = (c + 0.5) / courses
        cw = depth * (1.0 - t * 0.88)
        ch = pitch_h / courses
        cz = eave_z + (c + 0.5) * ch
        for sx in (-width * 0.5, width * 0.5):
            parts.append(
                _add_beveled_box(
                    f"Kit_GableStep_{c}_{sx:.1f}",
                    (0.12, cw, ch * 1.05),
                    (sx, 0.0, cz),
                    mat_timber,
                    bevel_width=0.015,
                )
            )

    # Ridge beam
    parts.append(
        _add_beveled_box(
            "Kit_RidgeBeam",
            (span_w + 0.08, 0.16, 0.16),
            (0.0, 0.0, eave_z + pitch_h + 0.04),
            mat_timber,
            bevel_width=0.02,
            subdiv_cuts=(4, 1, 1),
        )
    )

    # Overlapping quad shingle rows on front (-Y) and back (+Y) roof slopes
    rows = 7
    cols = 8
    shingle_w = span_w / cols
    shingle_l = (slope_len / rows) * 1.18

    for side in (-1, 1):
        rot_x = side * slope_angle
        for r in range(rows):
            frac = (r + 0.5) / rows
            y_pos = side * (half_d * (1.0 - frac))
            z_pos = eave_z + frac * pitch_h + 0.03
            for col in range(cols):
                x_pos = -span_w * 0.5 + (col + 0.5) * shingle_w
                use_alt = (r + col) % 2 == 0
                s_mat = mat_shingle_alt if use_alt else mat_shingle
                z_jitter = 0.008 if use_alt else 0.0
                parts.append(
                    _add_beveled_box(
                        f"Kit_Shingle_{side}_{r}_{col}",
                        (shingle_w * 0.96, shingle_l, 0.055),
                        (x_pos, y_pos, z_pos + z_jitter),
                        s_mat,
                        bevel_width=0.012,
                        rotation=(rot_x, 0.0, 0.0),
                    )
                )
    return parts


def build_chimney(x_pos: float, y_pos: float, base_z: float, top_z: float, mat_stone, mat_stone_alt, mat_ember):
    """Build a tapered masonry chimney stack with stone courses and glowing hearth box."""
    parts = []
    courses = 8
    total_h = top_z - base_z
    ch = total_h / courses
    for i in range(courses):
        t = i / max(1, courses - 1)
        w = 0.56 - t * 0.14
        cz = base_z + (i + 0.5) * ch
        m = mat_stone_alt if i % 2 == 0 else mat_stone
        parts.append(
            _add_beveled_box(
                f"Kit_ChimneyCourse_{i}",
                (w, w, ch * 0.98),
                (x_pos, y_pos, cz),
                m,
                bevel_width=0.02,
            )
        )
    # Chimney crown lip
    parts.append(
        _add_beveled_box(
            "Kit_ChimneyCrown",
            (0.48, 0.48, 0.1),
            (x_pos, y_pos, top_z + 0.04),
            mat_stone_alt,
            bevel_width=0.02,
        )
    )
    # Glowing hearth ember bed at base
    parts.append(
        _add_beveled_box(
            "Kit_ChimneyEmber",
            (0.32, 0.34, 0.18),
            (x_pos, y_pos - 0.12, base_z + 0.22),
            mat_ember,
            bevel_width=0.02,
        )
    )
    return parts


def build_door_and_windows(width: float, depth: float, base_z: float, mat_timber, mat_glow, mat_iron):
    """Build front timber door, warm window panes, and side casement windows."""
    parts = []
    front_y = -depth * 0.5 - 0.02
    # Door frame + planks
    door_x = -0.32
    door_h = 0.92
    parts.append(
        _add_beveled_box(
            "Kit_DoorFrame",
            (0.54, 0.1, door_h + 0.08),
            (door_x, front_y, base_z + door_h * 0.5),
            mat_timber,
            bevel_width=0.015,
        )
    )
    parts.append(
        _add_beveled_box(
            "Kit_DoorLeaf",
            (0.42, 0.08, door_h),
            (door_x, front_y - 0.01, base_z + door_h * 0.5),
            mat_timber,
            bevel_width=0.012,
            subdiv_cuts=(3, 1, 2),
        )
    )
    # Iron door straps
    for sz in (base_z + 0.24, base_z + 0.68):
        parts.append(
            _add_beveled_box(
                f"Kit_DoorStrap_{sz:.2f}",
                (0.36, 0.04, 0.05),
                (door_x, front_y - 0.04, sz),
                mat_iron,
                bevel_width=0.008,
            )
        )

    # Front glowing shop window
    win_x = 0.42
    win_z = base_z + 0.62
    parts.append(
        _add_beveled_box(
            "Kit_FrontWindowFrame",
            (0.62, 0.1, 0.48),
            (win_x, front_y, win_z),
            mat_timber,
            bevel_width=0.015,
        )
    )
    parts.append(
        _add_beveled_box(
            "Kit_FrontWindowGlow",
            (0.50, 0.06, 0.36),
            (win_x, front_y - 0.01, win_z),
            mat_glow,
            bevel_width=0.01,
        )
    )
    # Window sill
    parts.append(
        _add_beveled_box(
            "Kit_FrontWindowSill",
            (0.70, 0.18, 0.07),
            (win_x, front_y - 0.05, win_z - 0.24),
            mat_timber,
            bevel_width=0.012,
        )
    )

    # Side window (+X wall)
    parts.append(
        _add_beveled_box(
            "Kit_SideWindowFrame",
            (0.1, 0.52, 0.46),
            (width * 0.5 + 0.02, 0.0, win_z),
            mat_timber,
            bevel_width=0.015,
        )
    )
    parts.append(
        _add_beveled_box(
            "Kit_SideWindowGlow",
            (0.06, 0.40, 0.34),
            (width * 0.5 + 0.03, 0.0, win_z),
            mat_glow,
            bevel_width=0.01,
        )
    )
    return parts


def build_awning_and_lantern(front_y: float, base_z: float, mat_timber, mat_shingle, mat_iron, mat_glow):
    """Build a front shop awning and hanging wrought-iron lantern."""
    parts = []
    # Slanted awning over front window
    awn_x = 0.42
    awn_z = base_z + 0.96
    parts.append(
        _add_beveled_box(
            "Kit_AwningRoof",
            (0.78, 0.42, 0.06),
            (awn_x, front_y - 0.18, awn_z),
            mat_shingle,
            bevel_width=0.012,
            subdiv_cuts=(4, 2, 1),
            rotation=(0.35, 0.0, 0.0),
        )
    )
    # Hanging lantern beside door
    lx = -0.02
    ly = front_y - 0.12
    lz = base_z + 0.78
    parts.append(
        _add_beveled_box(
            "Kit_LanternBracket",
            (0.05, 0.18, 0.05),
            (lx, ly + 0.06, lz + 0.12),
            mat_iron,
            bevel_width=0.008,
        )
    )
    parts.append(
        _add_beveled_box(
            "Kit_LanternCore",
            (0.11, 0.11, 0.16),
            (lx, ly, lz),
            mat_glow,
            bevel_width=0.012,
        )
    )
    return parts


def build_role_props(preset: str, front_y: float, base_z: float, mat_iron, mat_timber, mat_stone, mat_herb, mat_glow):
    """Add role-specific low-poly quad props (anvil/quench trough for blacksmith, herb planters/phials for apothecary)."""
    parts = []
    if preset == "blacksmith":
        # Anvil on oak stump + quench trough
        ax, ay = 0.55, front_y - 0.36
        parts.append(_add_beveled_box("Kit_AnvilStump", (0.28, 0.28, 0.22), (ax, ay, base_z + 0.11), mat_timber, bevel_width=0.02))
        parts.append(_add_beveled_box("Kit_AnvilBody", (0.36, 0.18, 0.16), (ax, ay, base_z + 0.29), mat_iron, bevel_width=0.02))
        parts.append(_add_beveled_box("Kit_QuenchTrough", (0.48, 0.28, 0.22), (-0.75, front_y - 0.32, base_z + 0.11), mat_stone, bevel_width=0.02))
    elif preset == "apothecary":
        # Terracotta herb planters & glowing potion phials on window sill
        for idx, px in enumerate((-0.72, 0.18, 0.68)):
            parts.append(_add_beveled_box(f"Kit_Planter_{idx}", (0.22, 0.22, 0.20), (px, front_y - 0.24, base_z + 0.10), mat_timber, bevel_width=0.02))
            parts.append(_add_beveled_box(f"Kit_HerbBush_{idx}", (0.24, 0.24, 0.22), (px, front_y - 0.24, base_z + 0.28), mat_herb, bevel_width=0.03, subdiv_cuts=(2, 2, 2)))
    return parts


def build_arch_asset(
    preset: str = "cottage",
    output_path: str = "public/models/oakhaven_cottage_kit.glb",
    blend_path: str = "",
    asset_name: str = "",
    texture_dir: str = "public/textures",
    target_width: float = 3.2,
    clear_scene: bool = True,
):
    output_path = os.path.abspath(os.path.expanduser(output_path))
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    if blend_path:
        blend_path = os.path.abspath(os.path.expanduser(blend_path))
        os.makedirs(os.path.dirname(blend_path), exist_ok=True)
    tex_dir = os.path.abspath(os.path.expanduser(texture_dir))

    if clear_scene:
        for o in list(bpy.data.objects):
            bpy.data.objects.remove(o, do_unlink=True)

    def _tex(slug, suffix=""):
        p = os.path.join(tex_dir, f"{slug}{suffix}.png")
        return p if os.path.isfile(p) else ""

    # Stylized palette + optional ComfyUI seamless PBR textures
    roof_color = (0.22, 0.48, 0.36, 1.0) if preset == "apothecary" else (0.56, 0.20, 0.14, 1.0)
    roof_alt_color = (0.28, 0.56, 0.42, 1.0) if preset == "apothecary" else (0.46, 0.15, 0.11, 1.0)
    glow_color = (0.45, 0.95, 0.58, 1.0) if preset == "apothecary" else (1.0, 0.58, 0.16, 1.0)

    mat_stone = apply_pbr_texture_material(
        "Arch_Stone",
        base_color=(0.46, 0.43, 0.39, 1.0),
        roughness=0.88,
        albedo_path=_tex("oakhaven_cobble"),
        normal_path=_tex("oakhaven_cobble", "_normal"),
        roughness_path=_tex("oakhaven_cobble", "_roughness"),
    )
    mat_stone_alt = apply_pbr_texture_material(
        "Arch_StoneAlt",
        base_color=(0.54, 0.50, 0.45, 1.0),
        roughness=0.86,
    )
    mat_plaster = apply_pbr_texture_material(
        "Arch_Plaster",
        base_color=(0.88, 0.80, 0.64, 1.0),
        roughness=0.88,
        albedo_path=_tex("oakhaven_plaster"),
        normal_path=_tex("oakhaven_plaster", "_normal"),
        roughness_path=_tex("oakhaven_plaster", "_roughness"),
    )
    mat_timber = apply_pbr_texture_material(
        "Arch_Timber",
        base_color=(0.26, 0.16, 0.09, 1.0),
        roughness=0.82,
        albedo_path=_tex("oakhaven_timber"),
        normal_path=_tex("oakhaven_timber", "_normal"),
        roughness_path=_tex("oakhaven_timber", "_roughness"),
    )
    mat_shingle = apply_pbr_texture_material(
        "Arch_Shingle",
        base_color=roof_color,
        roughness=0.78,
        albedo_path=_tex("oakhaven_shingle"),
        normal_path=_tex("oakhaven_shingle", "_normal"),
        roughness_path=_tex("oakhaven_shingle", "_roughness"),
    )
    mat_shingle_alt = apply_pbr_texture_material(
        "Arch_ShingleAlt",
        base_color=roof_alt_color,
        roughness=0.80,
    )
    mat_iron = apply_pbr_texture_material(
        "Arch_Iron",
        base_color=(0.16, 0.18, 0.20, 1.0),
        roughness=0.45,
        metallic=0.80,
    )
    mat_herb = apply_pbr_texture_material(
        "Arch_Herb",
        base_color=(0.24, 0.64, 0.34, 1.0),
        roughness=0.82,
    )
    mat_glow = apply_pbr_texture_material(
        "Arch_Glow",
        base_color=glow_color,
        roughness=0.35,
        emission_color=glow_color,
        emission_strength=2.2,
    )

    parts = []
    base_h = 0.16
    wall_w = 2.1
    wall_d = 1.7
    wall_h = 1.24
    pitch_h = 0.86

    parts.extend(build_stone_curb_base(2.8, 2.5, base_h, mat_stone, mat_stone_alt))
    parts.extend(build_timber_plaster_walls(wall_w, wall_d, base_h, wall_h, mat_plaster, mat_timber, mat_stone_alt))
    parts.extend(build_shingle_roof(wall_w, wall_d, base_h + wall_h, pitch_h, mat_shingle, mat_shingle_alt, mat_timber))
    parts.extend(build_door_and_windows(wall_w, wall_d, base_h, mat_timber, mat_glow, mat_iron))
    parts.extend(build_awning_and_lantern(-wall_d * 0.5, base_h, mat_timber, mat_shingle, mat_iron, mat_glow))
    if preset in ("blacksmith", "apothecary", "cottage"):
        chim_x = -wall_w * 0.5 - 0.18 if preset == "blacksmith" else 0.35
        chim_y = 0.0 if preset == "blacksmith" else wall_d * 0.25
        parts.extend(build_chimney(chim_x, chim_y, base_h, base_h + wall_h + pitch_h + 0.32, mat_stone, mat_stone_alt, mat_glow))
    parts.extend(build_role_props(preset, -wall_d * 0.5, base_h, mat_iron, mat_timber, mat_stone_alt, mat_herb, mat_glow))

    # Join all modular parts into one clean mesh
    bpy.ops.object.select_all(action='DESELECT')
    for p in parts:
        p.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]
    bpy.ops.object.join()

    obj = bpy.context.active_object
    stem = asset_name or os.path.splitext(os.path.basename(output_path))[0]
    obj.name = stem
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)

    # Unwrap contiguous Smart UVs
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.uv.smart_project(angle_limit=math.radians(66.0), island_margin=0.015, correct_aspect=True)
    bpy.ops.object.mode_set(mode='OBJECT')

    # Normalize scale to target_width and place bottom-center at Z = 0
    coords = [v.co for v in obj.data.vertices]
    min_x, max_x = min(c.x for c in coords), max(c.x for c in coords)
    min_y, max_y = min(c.y for c in coords), max(c.y for c in coords)
    min_z = min(c.z for c in coords)
    cx = (min_x + max_x) * 0.5
    cy = (min_y + max_y) * 0.5
    scale = target_width / max(max_x - min_x, max_y - min_y, 1e-4)
    for v in obj.data.vertices:
        v.co.x = (v.co.x - cx) * scale
        v.co.y = (v.co.y - cy) * scale
        v.co.z = (v.co.z - min_z) * scale
    obj.data.update()

    # Smooth shading + Weighted Normals for crisp beveled low-poly look
    for poly in obj.data.polygons:
        poly.use_smooth = True
    obj.data.update()
    wn = obj.modifiers.new("WeightedNormal", 'WEIGHTED_NORMAL')
    wn.mode = 'FACE_AREA'
    wn.weight = 50
    wn.keep_sharp = True
    bpy.ops.object.modifier_apply(modifier=wn.name)

    quads = sum(1 for p in obj.data.polygons if len(p.vertices) == 4)
    tris = sum(1 for p in obj.data.polygons if len(p.vertices) == 3)

    if blend_path:
        bpy.ops.wm.save_as_mainfile(filepath=blend_path)

    bpy.ops.export_scene.gltf(
        filepath=output_path,
        export_format='GLB',
        use_selection=True,
        export_apply=True,
        export_materials='EXPORT',
        export_image_format='WEBP',
        export_image_quality=85,
        export_yup=True,
    )

    out_kb = os.path.getsize(output_path) / 1024.0
    summary = f"V2 Arch Kit '{stem}' ({preset}): {quads:,} quads ({tris} tris) | {out_kb:.1f} KB | saved {output_path}"
    print(summary)
    return summary


if __name__ == "__main__":
    args = _parse_args()
    build_arch_asset(
        preset=args.preset,
        output_path=args.output,
        blend_path=args.blend,
        asset_name=args.name,
        texture_dir=args.texture_dir,
        target_width=args.target_width,
        clear_scene=True,
    )
