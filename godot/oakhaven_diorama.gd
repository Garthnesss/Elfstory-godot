extends RefCounted
## Pitch C — "The Breached Sinkhole Waystation" diorama builder for Oakhaven.
##
## Owns multi-elevation tile heights for the central Reliquary breach and
## builds all handcrafted miniature architecture & props (oak treeline, log
## palisade, West Gate arch, terraced sinkhole retaining walls, north timber
## trestle bridge, south watch barricade & Vance's map table, stepped stone
## descent stairs, cool-lit Sunken Reliquary Archway, market stalls, Pell's
## supply cart, Vex's shaded smuggler alcove, Wren's candle shrine, Mira's
## herb planters, iron lantern posts, and natural pond shore + platypus).
##
## Strictly typed for Godot 4.7.2 strict GDScript checks.

const SINK_MIN_X := 9
const SINK_MAX_X := 17
const SINK_MIN_Y := 7
const SINK_MAX_Y := 11


static func is_in_sinkhole(x: int, y: int) -> bool:
	return x >= SINK_MIN_X and x <= SINK_MAX_X and y >= SINK_MIN_Y and y <= SINK_MAX_Y


## Elevation (world Y of walkable top surface) for each Oakhaven tile.
static func tile_elevation(zone_id: String, x: int, y: int) -> float:
	if zone_id != "oakhaven":
		return 0.0
	if x == 13 and y == 11:
		return -0.35
	if x == 13 and y == 10:
		return -0.75
	if x >= 12 and x <= 14 and y == 9:
		return -1.15
	if x >= 12 and x <= 14 and y == 8:
		return -1.25
	if is_in_sinkhole(x, y):
		return -2.4
	return 0.0


static func build(
	parent: Node3D,
	tiles: Array,
	oak_scene: PackedScene,
	platypus_scene: PackedScene,
	forge_lights: Array
) -> void:
	_build_woodland_and_palisade(parent, oak_scene, forge_lights)
	_build_sinkhole_breach(parent, forge_lights)
	_build_market_and_cart(parent)
	_build_vex_alcove(parent)
	_build_candle_shrine(parent)
	_build_herb_planters(parent)
	_build_lantern_posts(parent, tiles, forge_lights)
	_build_pond_shore_and_platypus(parent, platypus_scene)


static func _box(
	parent: Node3D,
	size: Vector3,
	pos: Vector3,
	color: Color,
	rot: Vector3 = Vector3.ZERO,
	emission: Color = Color.BLACK,
	emission_energy: float = 1.6,
	metallic: float = 0.0,
	roughness: float = 0.86
) -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	var mesh := BoxMesh.new()
	mesh.size = size
	mi.mesh = mesh
	mi.position = pos
	mi.rotation = rot
	var mat := StandardMaterial3D.new()
	mat.albedo_color = color
	mat.metallic = metallic
	mat.roughness = roughness
	if emission != Color.BLACK:
		mat.emission_enabled = true
		mat.emission = emission
		mat.emission_energy_multiplier = emission_energy
	mi.material_override = mat
	parent.add_child(mi)
	return mi


static func _cyl(
	parent: Node3D,
	r_top: float,
	r_bot: float,
	height: float,
	segs: int,
	pos: Vector3,
	color: Color,
	rot: Vector3 = Vector3.ZERO,
	emission: Color = Color.BLACK,
	emission_energy: float = 1.6,
	roughness: float = 0.85
) -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	var mesh := CylinderMesh.new()
	mesh.top_radius = r_top
	mesh.bottom_radius = r_bot
	mesh.height = height
	mesh.radial_segments = segs
	mi.mesh = mesh
	mi.position = pos
	mi.rotation = rot
	var mat := StandardMaterial3D.new()
	mat.albedo_color = color
	mat.roughness = roughness
	if emission != Color.BLACK:
		mat.emission_enabled = true
		mat.emission = emission
		mat.emission_energy_multiplier = emission_energy
	mi.material_override = mat
	parent.add_child(mi)
	return mi


static func _mount_glb_at(
	parent: Node3D,
	packed: PackedScene,
	pos: Vector3,
	target_width: float,
	yaw: float
) -> void:
	if packed == null:
		return
	var node := packed.instantiate() as Node3D
	if node == null:
		return
	var acc := _scene_aabb(node)
	var w := maxf(acc.size.x, acc.size.z)
	var y_off := 0.0
	if w > 0.01:
		var s := target_width / w
		node.scale = Vector3.ONE * s
		y_off = -acc.position.y * s
	node.position = Vector3(pos.x, pos.y + y_off, pos.z)
	node.rotation.y = yaw
	parent.add_child(node)


static func _scene_aabb(root: Node3D) -> AABB:
	var box := AABB()
	var first := true
	var stack: Array[Dictionary] = [{"node": root, "xf": Transform3D.IDENTITY}]
	while not stack.is_empty():
		var item: Dictionary = stack.pop_back() as Dictionary
		var n: Node = item["node"] as Node
		var xf: Transform3D = item["xf"] as Transform3D
		var local := xf
		if n is Node3D:
			local = xf * (n as Node3D).transform
		if n is MeshInstance3D:
			var mi := n as MeshInstance3D
			if mi.mesh != null:
				var ab := local * mi.get_aabb()
				if first:
					box = ab
					first = false
				else:
					box = box.merge(ab)
		for c: Node in n.get_children():
			stack.append({"node": c, "xf": local})
	return box


static func _add_omni(
	parent: Node3D,
	pos: Vector3,
	color: Color,
	energy: float,
	omni_range: float,
	flicker_list: Array = []
) -> OmniLight3D:
	var light := OmniLight3D.new()
	light.position = pos
	light.light_color = color
	light.light_energy = energy
	light.omni_range = omni_range
	light.omni_attenuation = 1.15
	light.set_meta("base_energy", energy)
	parent.add_child(light)
	if not flicker_list.is_empty() or color.r > 0.9:
		flicker_list.append(light)
	return light


static func _build_woodland_and_palisade(
	parent: Node3D,
	oak_scene: PackedScene,
	forge_lights: Array
) -> void:
	# Sculpted oak treeline around North, West, East, and far corners (south
	# center stays open so the camera sightline into the breach is clear).
	var oak_spots: Array[Vector4] = [
		Vector4(4.0, 0.2, 2.5, 0.7),
		Vector4(10.2, 0.1, 2.3, 2.1),
		Vector4(14.2, 0.1, 2.6, 3.8),
		Vector4(18.2, 0.2, 2.2, 1.2),
		Vector4(25.5, 0.2, 2.5, 4.4),
		Vector4(0.4, 2.0, 2.2, 1.5),
		Vector4(0.4, 5.2, 2.1, 2.9),
		Vector4(0.4, 10.2, 2.2, 0.3),
		Vector4(0.4, 13.5, 2.3, 4.1),
		Vector4(27.0, 2.2, 2.3, 2.4),
		Vector4(27.0, 6.2, 2.2, 5.1),
		Vector4(27.0, 12.6, 2.3, 1.1),
		Vector4(2.0, 15.6, 2.2, 3.3),
		Vector4(25.6, 15.5, 2.2, 0.8),
	]
	for sp: Vector4 in oak_spots:
		_mount_glb_at(
			parent,
			oak_scene,
			Vector3(sp.x, 0.0, sp.y),
			sp.z,
			sp.w
		)

	# Low mossy boulders + timber fence along the south rim (never blocks view).
	var wood := Color(0.32, 0.22, 0.14)
	var wood_dark := Color(0.22, 0.15, 0.10)
	var rock := Color(0.36, 0.38, 0.35)
	var moss := Color(0.24, 0.42, 0.20)
	for fx: int in [6, 8, 10, 12, 14, 16, 18, 20, 22]:
		_cyl(parent, 0.09, 0.10, 0.48, 8, Vector3(float(fx), 0.24, 15.7), wood_dark)
	_box(parent, Vector3(16.4, 0.08, 0.09), Vector3(14.0, 0.36, 15.7), wood)
	_box(parent, Vector3(16.4, 0.08, 0.09), Vector3(14.0, 0.18, 15.7), wood_dark)
	for bx: float in [5.4, 10.6, 16.4, 22.3]:
		_box(parent, Vector3(0.65, 0.38, 0.52), Vector3(bx, 0.19, 15.2), rock, Vector3(0.0, bx * 0.4, 0.0))
		_box(parent, Vector3(0.42, 0.16, 0.36), Vector3(bx + 0.08, 0.40, 15.18), moss)

	# Western log palisade segments flanking West Gate (0, 7).
	for z_seg: float in [3.2, 4.6, 9.6, 11.2, 12.6]:
		for i: int in 4:
			var pz := z_seg - 0.5 + float(i) * 0.34
			var h := 1.45 + float((i + int(z_seg)) % 3) * 0.16
			_cyl(parent, 0.13, 0.16, h, 8, Vector3(0.65, h * 0.5, pz), wood)
		_box(parent, Vector3(0.14, 0.14, 1.35), Vector3(0.65, 0.95, z_seg), wood_dark)

	# West Gate Frontier Archway at (0.3, 7.0).
	var stone := Color(0.45, 0.43, 0.40)
	var shingle := Color(0.48, 0.24, 0.16)
	_box(parent, Vector3(0.65, 2.2, 0.52), Vector3(0.35, 1.1, 6.05), stone)
	_box(parent, Vector3(0.65, 2.2, 0.52), Vector3(0.35, 1.1, 7.95), stone)
	_box(parent, Vector3(0.55, 0.32, 2.55), Vector3(0.35, 2.25, 7.0), wood_dark)
	_box(parent, Vector3(0.95, 0.16, 2.75), Vector3(0.35, 2.52, 7.0), shingle)
	# Warm hanging gate lantern + Vale signpost.
	_cyl(parent, 0.10, 0.12, 0.24, 8, Vector3(0.75, 1.92, 6.35), Color(1.0, 0.74, 0.30), Vector3.ZERO, Color(1.0, 0.65, 0.20), 2.2)
	_add_omni(parent, Vector3(0.95, 1.85, 6.35), Color(1.0, 0.68, 0.30), 2.3, 6.5, forge_lights)
	var sign_lbl := Label3D.new()
	sign_lbl.text = "← Silverwood Vale"
	sign_lbl.font_size = 38
	sign_lbl.pixel_size = 0.006
	sign_lbl.billboard = BaseMaterial3D.BILLBOARD_ENABLED
	sign_lbl.modulate = Color(0.96, 0.88, 0.68)
	sign_lbl.outline_size = 7
	sign_lbl.outline_modulate = Color(0.08, 0.06, 0.04)
	sign_lbl.position = Vector3(0.45, 2.95, 7.0)
	parent.add_child(sign_lbl)


static func _build_sinkhole_breach(parent: Node3D, forge_lights: Array) -> void:
	var slate := Color(0.24, 0.26, 0.31)
	var slate_dark := Color(0.16, 0.18, 0.23)
	var slate_cool := Color(0.26, 0.34, 0.44)
	var moss := Color(0.22, 0.38, 0.20)
	var wood := Color(0.34, 0.22, 0.13)
	var wood_dark := Color(0.23, 0.15, 0.09)
	var brass := Color(0.72, 0.56, 0.26)

	# 1. Terraced slate retaining walls lining the chasm rim (x 8.5..17.5, z 6.5..11.5).
	# West & East pit walls
	_box(parent, Vector3(0.45, 2.5, 5.2), Vector3(8.55, -1.22, 9.0), slate_dark)
	_box(parent, Vector3(0.45, 2.5, 5.2), Vector3(17.45, -1.22, 9.0), slate_dark)
	_box(parent, Vector3(0.28, 0.14, 5.2), Vector3(8.62, 0.04, 9.0), moss)
	_box(parent, Vector3(0.28, 0.14, 5.2), Vector3(17.38, 0.04, 9.0), moss)
	# North pit wall under the bridge
	_box(parent, Vector3(9.2, 2.5, 0.45), Vector3(13.0, -1.22, 6.55), slate_dark)
	# South pit walls flanking the central stair opening (x=13)
	_box(parent, Vector3(4.0, 2.5, 0.45), Vector3(10.5, -1.22, 11.45), slate_dark)
	_box(parent, Vector3(4.0, 2.5, 0.45), Vector3(15.5, -1.22, 11.45), slate_dark)
	# Subtle lower rock strata ledges for depth
	_box(parent, Vector3(0.65, 1.1, 4.6), Vector3(8.85, -1.85, 9.0), slate)
	_box(parent, Vector3(0.65, 1.1, 4.6), Vector3(17.15, -1.85, 9.0), slate)

	# 2. North Timber Trestle Bridge (x 9..17, z = 6) with pilings into the abyss.
	_box(parent, Vector3(9.2, 0.14, 1.12), Vector3(13.0, -0.02, 6.0), wood)
	for px: float in [9.2, 11.0, 13.0, 15.0, 16.8]:
		_cyl(parent, 0.14, 0.16, 2.48, 8, Vector3(px, -1.20, 6.42), wood_dark)
		_cyl(parent, 0.14, 0.16, 2.48, 8, Vector3(px, -1.20, 5.58), wood_dark)
		# Bridge railing posts on south edge
		_cyl(parent, 0.08, 0.09, 0.65, 8, Vector3(px, 0.34, 6.45), wood_dark)
	_box(parent, Vector3(9.0, 0.09, 0.10), Vector3(13.0, 0.62, 6.45), wood)

	# 3. South Watch Barricade & Corner Watch-Pylons with warm braziers.
	_box(parent, Vector3(3.6, 0.42, 0.22), Vector3(10.65, 0.21, 11.48), wood)
	_box(parent, Vector3(3.6, 0.42, 0.22), Vector3(15.35, 0.21, 11.48), wood)
	_box(parent, Vector3(3.6, 0.10, 0.12), Vector3(10.65, 0.64, 11.48), wood_dark)
	_box(parent, Vector3(3.6, 0.10, 0.12), Vector3(15.35, 0.64, 11.48), wood_dark)
	for px: float in [9.6, 10.8, 12.1, 13.9, 15.2, 16.4]:
		_cyl(parent, 0.08, 0.09, 0.68, 8, Vector3(px, 0.34, 11.48), wood_dark)

	# Four carved stone watch-pylons at the breach corners (replacing plain pillar boxes).
	var pylons: Array[Vector2] = [Vector2(9.0, 7.0), Vector2(17.0, 7.0), Vector2(9.0, 11.0), Vector2(17.0, 11.0)]
	for p: Vector2 in pylons:
		_box(parent, Vector3(0.72, 0.32, 0.72), Vector3(p.x, 0.16, p.y), slate_dark)
		_box(parent, Vector3(0.52, 1.35, 0.52), Vector3(p.x, 0.98, p.y), slate)
		_box(parent, Vector3(0.68, 0.18, 0.68), Vector3(p.x, 1.72, p.y), slate_dark)
		# Iron brazier bowl + warm ember flame
		_cyl(parent, 0.24, 0.14, 0.18, 10, Vector3(p.x, 1.90, p.y), Color(0.18, 0.18, 0.20), Vector3.ZERO, Color.BLACK, 1.0, 0.45)
		_cyl(parent, 0.16, 0.10, 0.14, 8, Vector3(p.x, 2.02, p.y), Color(1.0, 0.58, 0.16), Vector3.ZERO, Color(1.0, 0.50, 0.12), 2.4)
	_add_omni(parent, Vector3(9.0, 2.35, 11.0), Color(1.0, 0.64, 0.26), 2.2, 7.0, forge_lights)
	_add_omni(parent, Vector3(17.0, 2.35, 11.0), Color(1.0, 0.64, 0.26), 2.2, 7.0, forge_lights)

	# Warden Vance's Map Table at (15.1, 11.75) right behind Vance (14, 12).
	_box(parent, Vector3(1.10, 0.09, 0.68), Vector3(15.15, 0.62, 11.82), wood)
	for lx: float in [14.70, 15.60]:
		for lz: float in [11.55, 12.08]:
			_cyl(parent, 0.05, 0.06, 0.60, 6, Vector3(lx, 0.30, lz), wood_dark)
	_box(parent, Vector3(0.66, 0.02, 0.44), Vector3(15.10, 0.68, 11.82), Color(0.88, 0.80, 0.62), Vector3(0.0, 0.14, 0.0))
	_cyl(parent, 0.04, 0.05, 0.14, 8, Vector3(15.52, 0.74, 11.72), Color(0.95, 0.90, 0.74), Vector3.ZERO, Color(1.0, 0.72, 0.28), 1.8)

	# 4. Stepped Stone Stairway (x=13, z=11 -> 10 -> 9) descending into the pit.
	# Upper stair flight (z 11.4 -> 10.5, y 0.0 -> -0.35)
	for s: int in 4:
		var sy := -0.08 - float(s) * 0.10
		var sz := 11.35 - float(s) * 0.28
		_box(parent, Vector3(1.02, 0.26, 0.34), Vector3(13.0, sy, sz), slate_cool)
	# Mid stair flight (z 10.2 -> 9.4, y -0.45 -> -1.10)
	for s2: int in 4:
		var sy2 := -0.48 - float(s2) * 0.18
		var sz2 := 10.25 - float(s2) * 0.26
		_box(parent, Vector3(1.02, 0.30, 0.34), Vector3(13.0, sy2, sz2), slate_cool)
	# Beveled stone stair side-curbs
	_box(parent, Vector3(0.18, 0.55, 2.8), Vector3(12.42, -0.45, 10.2), slate_dark, Vector3(0.38, 0.0, 0.0))
	_box(parent, Vector3(0.18, 0.55, 2.8), Vector3(13.58, -0.45, 10.2), slate_dark, Vector3(0.38, 0.0, 0.0))
	# Sunken courtyard flagstone apron at y=-1.15 (x 12..14, z 9)
	_box(parent, Vector3(3.15, 1.30, 1.25), Vector3(13.0, -1.80, 9.0), slate_dark)

	# 5. Monumental Sunken Reliquary Archway at (13.0, -1.25, 7.65).
	_box(parent, Vector3(3.35, 1.25, 1.15), Vector3(13.0, -1.88, 8.0), slate_dark)
	_box(parent, Vector3(0.62, 2.25, 0.72), Vector3(11.65, -0.15, 7.62), slate)
	_box(parent, Vector3(0.62, 2.25, 0.72), Vector3(14.35, -0.15, 7.62), slate)
	# Heavy arched/stepped lintel + brass keystone
	_box(parent, Vector3(3.55, 0.48, 0.85), Vector3(13.0, 1.12, 7.62), slate_dark)
	_box(parent, Vector3(2.45, 0.28, 0.76), Vector3(13.0, 1.44, 7.62), slate)
	_box(parent, Vector3(0.46, 0.52, 0.90), Vector3(13.0, 1.18, 7.62), brass, Vector3.ZERO, Color.BLACK, 1.0, 0.7, 0.35)
	# Dark recessed portal void + glowing cool cyan threshold steps
	_box(parent, Vector3(2.05, 2.05, 0.35), Vector3(13.0, -0.22, 7.42), Color(0.04, 0.06, 0.12))
	_box(parent, Vector3(2.10, 0.08, 0.75), Vector3(13.0, -1.20, 7.82), Color(0.22, 0.52, 0.88), Vector3.ZERO, Color(0.24, 0.58, 1.0), 1.8)
	# Twin cool runic braziers flanking the Reliquary steps (warm haven vs cool threat!)
	for rx: float in [11.45, 14.55]:
		_cyl(parent, 0.20, 0.26, 0.72, 8, Vector3(rx, -0.88, 8.35), slate_dark)
		_cyl(parent, 0.16, 0.10, 0.18, 8, Vector3(rx, -0.45, 8.35), Color(0.35, 0.78, 1.0), Vector3.ZERO, Color(0.30, 0.72, 1.0), 3.0)
	_add_omni(parent, Vector3(13.0, -0.15, 8.35), Color(0.30, 0.64, 1.0), 3.8, 9.5)
	_add_omni(parent, Vector3(13.0, -1.75, 9.4), Color(0.22, 0.45, 0.90), 2.5, 8.5)

	# Rising cool arcane mist motes from the Reliquary pit.
	var mist := CPUParticles3D.new()
	mist.position = Vector3(13.0, -1.1, 8.2)
	mist.amount = 28
	mist.lifetime = 2.6
	mist.emission_shape = CPUParticles3D.EMISSION_SHAPE_BOX
	mist.emission_box_extents = Vector3(1.6, 0.2, 0.8)
	mist.direction = Vector3(0.0, 1.0, 0.0)
	mist.spread = 18.0
	mist.gravity = Vector3(0.0, 0.18, 0.0)
	mist.initial_velocity_min = 0.25
	mist.initial_velocity_max = 0.55
	mist.scale_amount_min = 0.08
	mist.scale_amount_max = 0.16
	var pmesh := SphereMesh.new()
	pmesh.radius = 0.5
	pmesh.height = 1.0
	var pmat := StandardMaterial3D.new()
	pmat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	pmat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	pmat.albedo_color = Color(0.45, 0.78, 1.0, 0.42)
	pmesh.material = pmat
	mist.mesh = pmesh
	parent.add_child(mist)

	var arch_lbl := Label3D.new()
	arch_lbl.text = "▼ Sunken Reliquary"
	arch_lbl.font_size = 42
	arch_lbl.pixel_size = 0.0065
	arch_lbl.billboard = BaseMaterial3D.BILLBOARD_ENABLED
	arch_lbl.modulate = Color(0.68, 0.88, 1.0)
	arch_lbl.outline_size = 8
	arch_lbl.outline_modulate = Color(0.04, 0.06, 0.12)
	arch_lbl.position = Vector3(13.0, 2.05, 7.75)
	parent.add_child(arch_lbl)


static func _build_market_and_cart(parent: Node3D) -> void:
	var wood := Color(0.36, 0.23, 0.13)
	var wood_dark := Color(0.24, 0.15, 0.09)
	var canvas_a := Color(0.76, 0.34, 0.20)
	var canvas_b := Color(0.88, 0.74, 0.46)
	var crate_col := Color(0.50, 0.35, 0.19)

	# Market Stalls at (9, 4) and (11, 4)
	for i: int in 2:
		var sx := 9.0 if i == 0 else 11.0
		var sz := 4.0
		# Timber counter & 4 corner posts
		_box(parent, Vector3(0.92, 0.46, 0.62), Vector3(sx, 0.23, sz + 0.08), wood)
		for px: float in [sx - 0.42, sx + 0.42]:
			for pz: float in [sz - 0.28, sz + 0.32]:
				_cyl(parent, 0.045, 0.05, 1.25, 6, Vector3(px, 0.625, pz), wood_dark)
		# Angled striped awning canopy
		_box(parent, Vector3(1.06, 0.08, 0.86), Vector3(sx, 1.22, sz + 0.02), canvas_a if i == 0 else canvas_b, Vector3(0.22, 0.0, 0.0))
		_box(parent, Vector3(0.36, 0.09, 0.86), Vector3(sx, 1.23, sz + 0.02), canvas_b if i == 0 else canvas_a, Vector3(0.22, 0.0, 0.0))
		# Goods on counter (barrel + crate)
		_cyl(parent, 0.15, 0.13, 0.32, 10, Vector3(sx - 0.22, 0.62, sz + 0.06), crate_col)
		_box(parent, Vector3(0.26, 0.22, 0.24), Vector3(sx + 0.20, 0.57, sz + 0.06), crate_col)

	# Pell's Supply Cart parked neatly beside the eastern stall at (12.15, 4.95)
	var cx := 12.15
	var cz := 4.95
	_box(parent, Vector3(0.95, 0.14, 0.64), Vector3(cx, 0.34, cz), wood)
	_box(parent, Vector3(0.95, 0.24, 0.07), Vector3(cx, 0.48, cz - 0.30), wood_dark)
	_box(parent, Vector3(0.95, 0.24, 0.07), Vector3(cx, 0.48, cz + 0.30), wood_dark)
	_cyl(parent, 0.24, 0.24, 0.08, 12, Vector3(cx - 0.22, 0.24, cz + 0.36), wood_dark, Vector3(PI * 0.5, 0.0, 0.0))
	_cyl(parent, 0.24, 0.24, 0.08, 12, Vector3(cx - 0.22, 0.24, cz - 0.36), wood_dark, Vector3(PI * 0.5, 0.0, 0.0))
	_box(parent, Vector3(0.34, 0.26, 0.32), Vector3(cx + 0.12, 0.54, cz), crate_col)
	_cyl(parent, 0.14, 0.12, 0.30, 10, Vector3(cx - 0.24, 0.54, cz - 0.08), Color(0.65, 0.55, 0.36))


static func _build_vex_alcove(parent: Node3D) -> void:
	# Replaces the old mauve bunker in SW corner with a shaded timber smuggler lean-to.
	var wood_dark := Color(0.20, 0.14, 0.11)
	var canvas := Color(0.28, 0.21, 0.34)
	var crate := Color(0.38, 0.27, 0.17)
	# Lean-to over Vex's contraband stall at (4, 14) extending toward (5.4, 14)
	_box(parent, Vector3(1.65, 0.52, 0.72), Vector3(4.15, 0.26, 14.15), wood_dark)
	for px: float in [3.4, 4.9]:
		for pz: float in [13.7, 14.6]:
			_cyl(parent, 0.055, 0.06, 1.38, 8, Vector3(px, 0.69, pz), wood_dark)
	_box(parent, Vector3(1.85, 0.09, 1.25), Vector3(4.15, 1.38, 14.15), canvas, Vector3(0.16, 0.0, 0.0))
	_box(parent, Vector3(0.36, 0.34, 0.34), Vector3(3.65, 0.68, 14.10), crate)
	_cyl(parent, 0.16, 0.14, 0.38, 10, Vector3(4.55, 0.70, 14.12), crate)
	_cyl(parent, 0.08, 0.10, 0.20, 8, Vector3(4.92, 1.15, 13.72), Color(0.76, 0.48, 0.96), Vector3.ZERO, Color(0.72, 0.40, 0.98), 2.0)
	_add_omni(parent, Vector3(5.25, 1.15, 13.75), Color(0.75, 0.50, 0.96), 1.6, 4.8)


static func _build_candle_shrine(parent: Node3D) -> void:
	# Sister Wren's Vigil Candle Shrine at (19, 13).
	var stone := Color(0.56, 0.54, 0.50)
	var stone_dark := Color(0.38, 0.36, 0.34)
	_box(parent, Vector3(1.06, 0.16, 1.06), Vector3(19.0, 0.08, 13.0), stone_dark)
	_box(parent, Vector3(0.72, 0.52, 0.58), Vector3(19.0, 0.42, 13.0), stone)
	_box(parent, Vector3(0.58, 0.72, 0.20), Vector3(19.0, 0.92, 12.82), stone_dark)
	for cx: float in [18.78, 18.92, 19.08, 19.22]:
		_cyl(parent, 0.035, 0.04, 0.18, 8, Vector3(cx, 0.77, 13.10), Color(0.95, 0.90, 0.76))
		_cyl(parent, 0.025, 0.01, 0.07, 6, Vector3(cx, 0.89, 13.10), Color(1.0, 0.78, 0.28), Vector3.ZERO, Color(1.0, 0.72, 0.20), 2.4)
	_add_omni(parent, Vector3(19.0, 1.25, 13.15), Color(1.0, 0.82, 0.45), 2.0, 5.2)


static func _build_herb_planters(parent: Node3D) -> void:
	var wood := Color(0.36, 0.24, 0.14)
	var soil := Color(0.22, 0.15, 0.10)
	var herb := Color(0.22, 0.54, 0.26)
	var bloom := Color(0.45, 0.88, 0.72)
	for pos: Vector2 in [Vector2(17.0, 4.0), Vector2(24.0, 4.0), Vector2(24.0, 5.0)]:
		_box(parent, Vector3(0.86, 0.18, 0.86), Vector3(pos.x, 0.09, pos.y), wood)
		_box(parent, Vector3(0.72, 0.20, 0.72), Vector3(pos.x, 0.10, pos.y), soil)
		for ox: float in [-0.18, 0.18]:
			for oz: float in [-0.18, 0.18]:
				_cyl(parent, 0.09, 0.14, 0.20, 7, Vector3(pos.x + ox, 0.26, pos.y + oz), herb)
		_cyl(parent, 0.05, 0.05, 0.10, 6, Vector3(pos.x, 0.34, pos.y), bloom, Vector3.ZERO, bloom, 1.4)


static func _build_lantern_posts(parent: Node3D, tiles: Array, forge_lights: Array) -> void:
	var iron := Color(0.16, 0.17, 0.19)
	var amber := Color(1.0, 0.74, 0.30)
	for y: int in tiles.size():
		var row: Array = tiles[y] as Array
		for x: int in row.size():
			if str(row[x]) != "torch":
				continue
			var fx := float(x)
			var fz := float(y)
			_cyl(parent, 0.06, 0.09, 1.45, 8, Vector3(fx, 0.725, fz), iron, Vector3.ZERO, Color.BLACK, 1.0, 0.45)
			_box(parent, Vector3(0.32, 0.07, 0.07), Vector3(fx + 0.12, 1.42, fz), iron)
			_cyl(parent, 0.09, 0.07, 0.20, 8, Vector3(fx + 0.24, 1.28, fz), amber, Vector3.ZERO, Color(1.0, 0.65, 0.22), 2.2)
			_add_omni(parent, Vector3(fx + 0.24, 1.32, fz), Color(1.0, 0.68, 0.30), 2.1, 6.8, forge_lights)


static func _build_pond_shore_and_platypus(parent: Node3D, platypus_scene: PackedScene) -> void:
	var rock := Color(0.36, 0.38, 0.36)
	var reed := Color(0.28, 0.48, 0.22)
	var wood := Color(0.36, 0.25, 0.15)
	# Small timber pier on the west shore of the pond (21.65, 9.6)
	_box(parent, Vector3(0.95, 0.09, 0.78), Vector3(21.75, 0.03, 9.65), wood)
	_cyl(parent, 0.07, 0.08, 0.65, 8, Vector3(22.15, -0.22, 9.32), wood)
	_cyl(parent, 0.07, 0.08, 0.65, 8, Vector3(22.15, -0.22, 9.98), wood)
	# Natural shoreline boulders & reed tufts
	var rocks: Array[Vector4] = [
		Vector4(22.15, 8.15, 0.65, 0.28),
		Vector4(25.3, 8.4, 0.54, 0.30),
		Vector4(25.5, 11.4, 0.50, 0.28),
		Vector4(22.5, 12.15, 0.58, 0.26),
		Vector4(24.9, 12.2, 0.62, 0.28),
	]
	for r: Vector4 in rocks:
		_box(parent, Vector3(r.z, r.w, r.z * 0.85), Vector3(r.x, 0.05, r.y), rock, Vector3(0.0, r.x * 0.5, 0.0))
	for r_pos: Vector2 in [Vector2(22.6, 8.5), Vector2(25.1, 9.2), Vector2(25.2, 11.1), Vector2(22.9, 11.9)]:
		for k: int in 3:
			_cyl(parent, 0.015, 0.03, 0.46 + float(k) * 0.08, 5, Vector3(r_pos.x + float(k) * 0.08, 0.16, r_pos.y), reed, Vector3(0.08 * float(k - 1), 0.0, 0.06))

	# Basking Oakhaven Platypus on the SE mossy bank
	_mount_glb_at(parent, platypus_scene, Vector3(24.15, 0.02, 12.2), 1.15, -PI * 0.35)
