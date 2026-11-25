class_name WorldProps
extends RefCounted
## Small built-in-code props and the outdoor trees. One mesh per tree part for the
## whole zone (MultiMesh), so a forest costs two draw calls.

const TREE_SHADER: Shader = preload("res://systems/prop_tree.gdshader")


static func _h(x: int, y: int, n: int) -> float:
	var v := sin(float(x) * 127.1 + float(y) * 311.7 + float(n) * 74.7) * 43758.5453
	return v - floorf(v)


## Trunks and canopies for every tree tile. Empty when there are none.
static func trees(at: Array[Vector2i]) -> Array[MultiMeshInstance3D]:
	var out: Array[MultiMeshInstance3D] = []
	if at.is_empty():
		return out
	var trunk := CylinderMesh.new()
	trunk.top_radius = 0.09
	trunk.bottom_radius = 0.17
	trunk.height = 1.0
	trunk.radial_segments = 7
	trunk.rings = 1
	var leaf := SphereMesh.new()
	leaf.radius = 0.5
	leaf.height = 0.92
	leaf.radial_segments = 9
	leaf.rings = 5
	var trunk_xf: Array[Transform3D] = []
	var leaf_xf: Array[Transform3D] = []
	var leaf_col: Array[Color] = []
	for p: Vector2i in at:
		var r := _h(p.x, p.y, 0)
		var base := Vector3(float(p.x) + (_h(p.x, p.y, 1) - 0.5) * 0.3, 0.0, float(p.y) + (_h(p.x, p.y, 2) - 0.5) * 0.3)
		var tall := 0.85 + r * 0.5
		var turn := Basis(Vector3.UP, r * TAU)
		trunk_xf.append(Transform3D(turn.scaled(Vector3(1.0, tall, 1.0)), base + Vector3(0.0, tall * 0.5, 0.0)))
		# Three overlapping puffs, lowest widest, offset so no two trees match.
		for i: int in 3:
			var a := r * TAU + float(i) * 2.2
			var spread := 0.22 - 0.07 * float(i)
			var s := 1.05 - 0.2 * float(i) + (_h(p.x, p.y, 3 + i) - 0.5) * 0.15
			leaf_xf.append(Transform3D(turn.scaled(Vector3(s, s * 0.92, s)), base + Vector3(cos(a) * spread, tall + 0.08 + 0.34 * float(i), sin(a) * spread)))
			var g := _h(p.x, p.y, 9 + i) * 0.10
			leaf_col.append(Color(0.17 + g, 0.36 + g * 1.4 + 0.05 * float(i), 0.13 + g * 0.5))
	var brown: Array[Color] = []
	brown.resize(trunk_xf.size())
	brown.fill(Color(0.30, 0.20, 0.12))
	out.append(_mm(trunk, trunk_xf, brown, 0.0))
	out.append(_mm(leaf, leaf_xf, leaf_col, 0.03))
	return out


static func _mm(mesh: Mesh, xfs: Array[Transform3D], cols: Array[Color], sway: float) -> MultiMeshInstance3D:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	mm.mesh = mesh
	mm.instance_count = xfs.size()
	for i: int in xfs.size():
		mm.set_instance_transform(i, xfs[i])
		mm.set_instance_color(i, cols[i])
	var mat := ShaderMaterial.new()
	mat.shader = TREE_SHADER
	mat.set_shader_parameter("sway", sway)
	var node := MultiMeshInstance3D.new()
	node.multimesh = mm
	node.material_override = mat
	return node


## A chest, brazier or waystone; `used` shows it spent.
static func make(kind: String, used: bool) -> Node3D:
	var root := Node3D.new()
	match kind:
		"chest":
			var wood := Color(0.36, 0.22, 0.11) if not used else Color(0.22, 0.15, 0.09)
			_box(root, Vector3(0.62, 0.30, 0.42), Vector3(0.0, 0.15, 0.0), wood)
			var lid := _box(root, Vector3(0.64, 0.14, 0.44), Vector3(0.0, 0.37, 0.0), wood.lightened(0.08))
			if used:
				lid.rotation.x = -1.0
				lid.position = Vector3(0.0, 0.46, -0.2)
			else:
				_box(root, Vector3(0.10, 0.16, 0.03), Vector3(0.0, 0.30, 0.22), Color(0.95, 0.74, 0.26), 0.25, 0.8)
			for x: float in [-0.24, 0.24]:
				_box(root, Vector3(0.06, 0.32, 0.45), Vector3(x, 0.16, 0.0), Color(0.62, 0.46, 0.18), 0.0, 0.7)
		"brazier":
			_cyl(root, 0.10, 0.18, 0.42, Vector3(0.0, 0.21, 0.0), Color(0.24, 0.22, 0.22))
			_cyl(root, 0.32, 0.20, 0.20, Vector3(0.0, 0.50, 0.0), Color(0.30, 0.26, 0.22))
			if not used:
				_cyl(root, 0.02, 0.22, 0.34, Vector3(0.0, 0.74, 0.0), Color(1.0, 0.55, 0.14), 2.2)
				_light(root, Color(1.0, 0.62, 0.28), 1.6, 4.0, 0.95)
		"waystone":
			_cyl(root, 0.34, 0.40, 0.14, Vector3(0.0, 0.07, 0.0), Color(0.38, 0.40, 0.46))
			var stone := _box(root, Vector3(0.30, 1.15, 0.22), Vector3(0.0, 0.70, 0.0), Color(0.34, 0.40, 0.52), 0.0 if used else 0.35)
			stone.rotation.y = 0.5
			if not used:
				_light(root, Color(0.45, 0.78, 1.0), 1.2, 3.4, 1.1)
	return root


static func _mat(color: Color, glow: float, metal: float) -> StandardMaterial3D:
	var m := StandardMaterial3D.new()
	m.albedo_color = color
	m.roughness = 0.75
	m.metallic = metal
	if glow > 0.0:
		m.emission_enabled = true
		m.emission = color
		m.emission_energy_multiplier = glow
	return m


static func _box(root: Node3D, size: Vector3, at: Vector3, color: Color, glow := 0.0, metal := 0.0) -> MeshInstance3D:
	var mesh := BoxMesh.new()
	mesh.size = size
	return _add(root, mesh, at, _mat(color, glow, metal))


static func _cyl(root: Node3D, top: float, bottom: float, height: float, at: Vector3, color: Color, glow := 0.0) -> MeshInstance3D:
	var mesh := CylinderMesh.new()
	mesh.top_radius = top
	mesh.bottom_radius = bottom
	mesh.height = height
	mesh.radial_segments = 10
	mesh.rings = 1
	return _add(root, mesh, at, _mat(color, glow, 0.0))


static func _add(root: Node3D, mesh: Mesh, at: Vector3, mat: StandardMaterial3D) -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	mi.mesh = mesh
	mi.position = at
	mi.material_override = mat
	root.add_child(mi)
	return mi


static func _light(root: Node3D, color: Color, energy: float, reach: float, height: float) -> void:
	var l := OmniLight3D.new()
	l.light_color = color
	l.light_energy = energy
	l.omni_range = reach
	l.position.y = height
	l.distance_fade_enabled = true
	l.distance_fade_begin = 22.0
	root.add_child(l)
