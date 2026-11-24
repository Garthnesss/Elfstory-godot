"""Render an isolated 3/4 preview of a .glb (sun + neutral world) so
surface/texture problems are visible without scene clutter.
  Blender --background --python render_preview.py -- in.glb out.png"""
import bpy
import sys
import math
from mathutils import Vector

argv = sys.argv[sys.argv.index("--") + 1:]
src, out = argv[0], argv[1]

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src)

meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
mn = Vector((1e9, 1e9, 1e9))
mx = Vector((-1e9, -1e9, -1e9))
for o in meshes:
    for c in o.bound_box:
        w = o.matrix_world @ Vector(c)
        mn = Vector(map(min, mn, w))
        mx = Vector(map(max, mx, w))
center = (mn + mx) / 2
size = (mx - mn).length

cam_data = bpy.data.cameras.new("cam")
cam = bpy.data.objects.new("cam", cam_data)
bpy.context.scene.collection.objects.link(cam)
dist = size * 1.38
cam.location = center + Vector((dist * 0.72, -dist * 0.72, dist * 0.42))
cam.rotation_euler = (center - cam.location).to_track_quat("-Z", "Y").to_euler()
bpy.context.scene.camera = cam

sun_data = bpy.data.lights.new("sun", "SUN")
sun_data.energy = 3.0
sun = bpy.data.objects.new("sun", sun_data)
bpy.context.scene.collection.objects.link(sun)
sun.rotation_euler = (math.radians(50), math.radians(15), math.radians(-35))

world = bpy.data.worlds.new("w")
world.use_nodes = True
bg = world.node_tree.nodes["Background"]
bg.inputs[0].default_value = (0.55, 0.58, 0.52, 1)
bg.inputs[1].default_value = 0.7
bpy.context.scene.world = world

scene = bpy.context.scene
try:
    scene.render.engine = "BLENDER_EEVEE_NEXT"
except Exception:
    scene.render.engine = "BLENDER_EEVEE"
scene.render.resolution_x = 640
scene.render.resolution_y = 480
scene.render.filepath = out
bpy.ops.render.render(write_still=True)
print("PREVIEW_SAVED", out)
