extends RefCounted
## Meadow grass + wildflowers over "floor" tiles (presentation only).
##
## One procedural tuft mesh (a few tapered, leaning blades) is instanced
## many times per tile on a MultiMesh; a second sparse MultiMesh adds tiny
## flower heads. Placement is a pure function of tile coords (integer hash
## + fixed-seed noise), so the same zone always grows the same meadow and
## nothing touches the game's seeded RNG. Grass casts no shadows (cost,
## and blade shadows read as noise from the tactical camera).

const SHADER: Shader = preload("res://grass.gdshader")

const TUFTS_PER_TILE := 22
const BLADES_PER_TUFT := 11
const FLOWER_CHANCE := 0.035 # share of tufts that carry a flower head

# Meadow tint range (multiplies the shader's base->tip ramp).
const TINT_LUSH := Color(0.94, 1.0, 0.94)
const TINT_DRY := Color(1.04, 1.03, 0.90)
const FLOWER_COLORS := [
	Color(0.98, 0.96, 0.88), # white
	Color(1.0, 0.85, 0.30), # buttercup
	Color(0.78, 0.66, 0.95), # lavender
	Color(1.0, 0.62, 0.55), # soft coral
]


## Ground colour under the grass: darker than the bare-tile green so the
## gaps between tufts read as shade in the meadow, not tan dirt.
const GROUND := Color(0.17, 0.36, 0.13)


## Grass + flowers for the given tiles. `tiles` is the snapshot grid,
## `skip` is the "x,y" set of tiles a model owns. Returns a Node3D (or
## null when the zone has no floor tiles).
static func build(tiles: Array, skip: Dictionary) -> Node3D:
	var density := FastNoiseLite.new()
	density.seed = 7741
	density.frequency = 0.11
	var hue := FastNoiseLite.new()
	hue.seed = 1913
	hue.frequency = 0.06

	var tuft_xf: Array[Transform3D] = []
	var tuft_col: Array[Color] = []
	var flower_xf: Array[Transform3D] = []
	var flower_col: Array[Color] = []

	for y in tiles.size():
		var row: Array = tiles[y]
		for x in row.size():
			if str(row[x]) != "floor" or skip.has("%d,%d" % [x, y]):
				continue
			# Gentle clumping: 75-100% of the tuft slots grow, by low-freq noise.
			var keep := 0.88 + 0.25 * density.get_noise_2d(x, y)
			var dry := clampf(0.5 + hue.get_noise_2d(x, y) * 1.4, 0.0, 1.0)
			for i in TUFTS_PER_TILE:
				if h01(x, y, i * 4) > keep:
					continue
				var px := float(x) + (h01(x, y, i * 4 + 1) - 0.5) * 0.98
				var pz := float(y) + (h01(x, y, i * 4 + 2) - 0.5) * 0.98
				var r := h01(x, y, i * 4 + 3)
				var basis := Basis(Vector3.UP, r * TAU)
				var w := 0.85 + r * 0.35
				var tb := basis.scaled(Vector3(w, 0.75 + h01(x, y, i * 4 + 500) * 0.6, w))
				tuft_xf.append(Transform3D(tb, Vector3(px, 0.0, pz)))
				var c := TINT_LUSH.lerp(TINT_DRY, dry)
				var v := 0.96 + h01(x, y, i * 4 + 700) * 0.08
				tuft_col.append(Color(c.r * v, c.g * v, c.b * v))
				# A few tufts carry a flower head; the tuft hides its stem.
				if h01(x, y, i * 4 + 900) < FLOWER_CHANCE:
					var s := 0.8 + r * 0.5
					flower_xf.append(Transform3D(basis.scaled(Vector3(s, s, s)), Vector3(px, 0.0, pz)))
					var fc: Color = FLOWER_COLORS[int(r * 997.0) % FLOWER_COLORS.size()]
					flower_col.append(fc)

	if tuft_xf.is_empty():
		return null
	var root := Node3D.new()
	root.name = "Grass"
	root.add_child(_multimesh(tuft_mesh(), tuft_xf, tuft_col, 0.0))
	if not flower_xf.is_empty():
		root.add_child(_multimesh(flower_mesh(), flower_xf, flower_col, 1.0))
	return root


static func _multimesh(mesh: Mesh, xfs: Array[Transform3D], cols: Array[Color], tint_only: float) -> MultiMeshInstance3D:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	mm.mesh = mesh
	mm.instance_count = xfs.size()
	for i in xfs.size():
		mm.set_instance_transform(i, xfs[i])
		mm.set_instance_color(i, cols[i])
	var mat := ShaderMaterial.new()
	mat.shader = SHADER
	mat.set_shader_parameter("tint_only", tint_only)
	var mmi := MultiMeshInstance3D.new()
	mmi.multimesh = mm
	mmi.material_override = mat
	mmi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	return mmi


## A tuft: BLADES_PER_TUFT tapered blades (5 verts / 3 tris each) fanned
## around a small circle, each leaning outward. Fixed seed, so every
## build makes the same tuft.
static func tuft_mesh() -> ArrayMesh:
	var rng := RandomNumberGenerator.new()
	rng.seed = 20261121
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	for b in BLADES_PER_TUFT:
		var ang := TAU * float(b) / float(BLADES_PER_TUFT) + rng.randf_range(-0.4, 0.4)
		var out := Vector3(cos(ang), 0.0, sin(ang))
		var root := out * rng.randf_range(0.0, 0.09)
		var h := rng.randf_range(0.16, 0.26)
		var half_w := rng.randf_range(0.04, 0.056)
		var lean := out * h * rng.randf_range(0.12, 0.32)
		var side := Vector3(-out.z, 0.0, out.x).rotated(Vector3.UP, rng.randf_range(-0.6, 0.6)) * half_w
		var mid := root + Vector3(0.0, h * 0.55, 0.0) + lean * 0.35
		var tip := root + Vector3(0.0, h, 0.0) + lean
		var v: Array[Vector3] = [root - side, root + side, mid - side * 0.65, mid + side * 0.65, tip]
		var uvy: Array[float] = [0.0, 0.0, 0.55, 0.55, 1.0]
		var tris: Array[PackedInt32Array] = [
			PackedInt32Array([0, 1, 2]),
			PackedInt32Array([2, 1, 3]),
			PackedInt32Array([2, 3, 4]),
		]
		for tri: PackedInt32Array in tris:
			for k: int in tri:
				st.set_uv(Vector2(0.5, uvy[k]))
				st.set_normal(Vector3.UP)
				st.add_vertex(v[k])
	return st.commit()


## A flower head: a small 5-petal star facing up at tuft height (its
## tuft hides the missing stem). All UV.y = 1 so it sways with the tips.
static func flower_mesh() -> ArrayMesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var c := Vector3(0.0, 0.24, 0.0)
	var petals := 5
	for p in petals:
		var a0 := TAU * float(p) / float(petals)
		var tip := c + Vector3(cos(a0), 0.15, sin(a0)) * 0.08
		var l := c + Vector3(cos(a0 - 0.45), 0.05, sin(a0 - 0.45)) * 0.045
		var r := c + Vector3(cos(a0 + 0.45), 0.05, sin(a0 + 0.45)) * 0.045
		var verts: Array[Vector3] = [c, l, tip, c, tip, r]
		for vtx: Vector3 in verts:
			st.set_uv(Vector2(0.5, 1.0))
			st.set_normal(Vector3.UP)
			st.add_vertex(vtx)
	return st.commit()


## Deterministic 0..1 hash of three ints (stable across runs and builds).
static func h01(a: int, b: int, c: int) -> float:
	var n := (a * 73856093) ^ (b * 19349663) ^ (c * 83492791)
	n = (n ^ (n >> 13)) * 1274126177
	n = n ^ (n >> 16)
	return float(n & 0xFFFFFF) / float(0xFFFFFF)
