extends Node3D
## ElfStory Reborn — Milestone 2 Godot front-end (vertical slice).
##
## Every rule lives in the Rust core (`elfstory-core`) behind the
## ElfStorySession GDExtension bridge. This script is a pure view:
## it renders the `state_json()` snapshot (tile grid + entities + HUD data)
## and forwards player intent (move / interact / town / merchant calls).
##
## Coordinate mapping: snapshot tile (x, y) -> world (x, 0, y), tile centres
## on integer coordinates, ground tops at y = 0. Placement x/z from
## `placements_json()` are already tile coords, so they drop straight in.

const WATER_SHADER: Shader = preload("res://water.gdshader")
const MOVE_REPEAT := 0.16 # held-key step interval, seconds
const LOTS := ["pondside", "market", "west_gate"]

# Tile kinds drawn as tall boxes on the second MultiMesh, with box heights.
const TALL_HEIGHTS := {
	"wall": 3.0,
	"tree": 2.4,
	"pillar": 2.6,
	"gate_city": 2.8,
	"market_stall": 1.7,
	"torch": 1.8,
}

var session: Object # ElfStorySession (GDExtension class)
var state: Dictionary = {}

var current_zone_id := ""
var current_build_key := "" # zone id + town JSON; a change rebuilds terrain/landmarks

var terrain_root: Node3D
var landmarks_root: Node3D
var forge_lights: Array = []
var _flicker_t := 0.0
var entities_root: Node3D

var player_node: Node3D
var entity_nodes: Dictionary = {} # key -> Node3D
var entity_targets: Dictionary = {} # Node3D -> Vector3 (lerp goal)

var cam_yaw := 0.0 # radians, snapped to 90-degree steps by [ and ]
var chase_mode := false # false = Overhead Tactical, true = Chase (V toggles)
var camera_snapped := false

var move_timer := 0.0

@onready var camera_rig: Node3D = $CameraRig
@onready var camera: Camera3D = $CameraRig/Camera3D
@onready var sun: DirectionalLight3D = $DirectionalLight3D

# UI (all built in code, see build_ui)
var hud_title: Label
var hud_body: Label
var log_label: RichTextLabel
var town_panel: PanelContainer
var town_box: VBoxContainer
var dialogue_panel: PanelContainer
var dialogue_label: Label
var dialogue_choices: VBoxContainer
var merchant_panel: PanelContainer
var merchant_title: Label
var merchant_rows: VBoxContainer
var current_merchant_id := ""


func _ready() -> void:
	sun.rotation_degrees = Vector3(-52.0, -35.0, 0.0) # warm afternoon angle

	terrain_root = Node3D.new()
	terrain_root.name = "Terrain"
	add_child(terrain_root)
	landmarks_root = Node3D.new()
	landmarks_root.name = "Landmarks"
	add_child(landmarks_root)
	entities_root = Node3D.new()
	entities_root.name = "Entities"
	add_child(entities_root)

	build_ui()

	session = ElfStorySession.new()
	session.new_game("Garth", "Vanguard", 20261120)
	refresh()


# ---------------------------------------------------------------------------
# Snapshot refresh + bridge events
# ---------------------------------------------------------------------------

## Re-fetch the snapshot and sync every view. Terrain/landmarks rebuild only
## when the zone or the town state changed (placements depend on both);
## entities and panels refresh every time.
func refresh() -> void:
	var parsed: Variant = JSON.parse_string(session.state_json())
	if parsed == null:
		push_warning("ElfStory: state_json() did not parse")
		return
	state = parsed

	update_hud()
	update_log()
	rebuild_town_panel()

	var zone: Dictionary = state["zone"]
	var zid := str(zone["id"])
	var build_key := zid + "|" + JSON.stringify(state.get("town", {}))
	if zid != current_zone_id or build_key != current_build_key:
		current_zone_id = zid
		current_build_key = build_key
		rebuild_world()
	refresh_entities()


## Bridge events are plain strings; panels are the only side effects here —
## the snapshot refresh that follows every action shows the consequences.
func handle_event(ev: String) -> void:
	if ev.begins_with("dialogue:"):
		open_dialogue(ev.trim_prefix("dialogue:"))
	elif ev.begins_with("merchant:"):
		open_merchant(ev.trim_prefix("merchant:"))
	# "zone_changed:<id>"  -> refresh() rebuilds on the zone-id change.
	# "player_died"        -> the core already woke the player in Oakhaven;
	#                         refresh() shows the new zone + the death log line.
	# "level_up:<n>"       -> HUD/log in the fresh snapshot tell the story.
	# "none"               -> nothing to do.


func do_move(screen_dir: Vector2i) -> void:
	var world_dir := screen_dir_to_world(screen_dir)
	if world_dir == Vector2i.ZERO:
		return
	var ev: String = session.move_player(world_dir.x, world_dir.y)
	handle_event(ev)
	refresh()


func do_interact() -> void:
	var ev: String = session.interact()
	handle_event(ev)
	refresh()


## Convert a screen-space step (x = right, y = up/away from camera) into a
## world tile step, honouring the quarter-turn camera yaw. Yaw is always a
## multiple of 90 degrees, so the result is axis- or diagonal-aligned.
func screen_dir_to_world(sd: Vector2i) -> Vector2i:
	var offset := Vector3(0.0, 7.0, 9.0) if chase_mode else Vector3(0.0, 26.0, 14.0)
	var rotated := offset.rotated(Vector3.UP, cam_yaw)
	var fwd := Vector2(-rotated.x, -rotated.z).normalized() # away from camera, tile (x, y)
	var right := Vector2(-fwd.y, fwd.x) # screen right, tile (x, y)
	var w := right * float(sd.x) + fwd * float(sd.y)
	return Vector2i(int(round(w.x)), int(round(w.y)))


# ---------------------------------------------------------------------------
# World building: terrain MultiMeshes, water, landmark .glb placements
# ---------------------------------------------------------------------------

func rebuild_world() -> void:
	clear_children(terrain_root)
	clear_children(landmarks_root)

	var zone: Dictionary = state["zone"]
	var tiles: Array = zone["tiles"]
	var is_town := bool(zone.get("isTown", false))
	var placements := fetch_placements()

	# GUARDRAIL: tiles under a placed model's hideTerrainFootprint are never
	# drawn — the .glb owns that ground, and camera cutaways must not bring
	# the tiles back. Keyed "x,y" in tile coords (footprint z == tile y).
	var covered := {}
	if str(zone["id"]) == "oakhaven":
		for p in placements:
			if p.has("hideTerrainFootprint"):
				var f: Dictionary = p["hideTerrainFootprint"]
				for tx in range(int(f["minX"]), int(f["maxX"]) + 1):
					for tz in range(int(f["minZ"]), int(f["maxZ"]) + 1):
						covered["%d,%d" % [tx, tz]] = true

	var flat_items: Array = []
	var tall_items: Array = []
	for y in tiles.size():
		var row: Array = tiles[y]
		for x in row.size():
			if covered.has("%d,%d" % [x, y]):
				continue
			var kind := str(row[x])
			var item := {"x": x, "y": y, "kind": kind}
			if TALL_HEIGHTS.has(kind):
				tall_items.append(item)
			else:
				flat_items.append(item)

	var flat_mm := make_multimesh(flat_items, false, is_town)
	if flat_mm != null:
		terrain_root.add_child(flat_mm)
	var tall_mm := make_multimesh(tall_items, true, is_town)
	if tall_mm != null:
		terrain_root.add_child(tall_mm)

	build_water(tiles, is_town, covered)
	# Placements are the Oakhaven town-lot models; anywhere else they'd
	# draw the town's buildings at their Oakhaven tile coordinates.
	if str(zone["id"]) == "oakhaven":
		build_landmarks(placements)


func fetch_placements() -> Array:
	var parsed: Variant = JSON.parse_string(session.placements_json())
	if parsed is Array:
		return parsed
	return []


## One MultiMesh of unit boxes. Flat tiles top out at y = 0 (water sits
## lower, chests/fountains sit on top); tall kinds rise from y = -1.
func make_multimesh(items: Array, tall: bool, is_town: bool) -> MultiMeshInstance3D:
	if items.is_empty():
		return null
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	var box := BoxMesh.new()
	box.size = Vector3.ONE
	mm.mesh = box
	mm.instance_count = items.size()
	for i in items.size():
		var it: Dictionary = items[i]
		var kind := str(it["kind"])
		var h := 1.0
		var bottom := -1.0
		if tall:
			h = float(TALL_HEIGHTS[kind])
		else:
			var fb := flat_box(kind)
			h = fb.x
			bottom = fb.y
		var basis := Basis.from_scale(Vector3(0.97, h, 0.97))
		var origin := Vector3(float(it["x"]), bottom + h * 0.5, float(it["y"]))
		mm.set_instance_transform(i, Transform3D(basis, origin))
		mm.set_instance_color(i, tile_color(kind, is_town))
	var mmi := MultiMeshInstance3D.new()
	mmi.multimesh = mm
	var mat := StandardMaterial3D.new()
	mat.vertex_color_use_as_albedo = true # required for MultiMesh instance colours
	mat.roughness = 0.95
	mmi.material_override = mat
	return mmi


## (height, bottom) for flat tile kinds.
func flat_box(kind: String) -> Vector2:
	match kind:
		"water":
			return Vector2(0.7, -1.0) # top at -0.3, under the water plane
		"chest":
			return Vector2(0.55, 0.0)
		"fountain":
			return Vector2(0.9, -0.1)
		_:
			return Vector2(1.0, -1.0) # top at 0


func tile_color(kind: String, is_town: bool) -> Color:
	match kind:
		"floor":
			return Color(0.33, 0.58, 0.27) if is_town else Color(0.34, 0.58, 0.28)
		"road":
			return Color(0.62, 0.48, 0.31)
		"wall":
			return Color(0.25, 0.24, 0.30)
		"water":
			return Color(0.10, 0.28, 0.55)
		"tree":
			return Color(0.16, 0.42, 0.18)
		"pillar", "gate_city":
			return Color(0.55, 0.53, 0.48)
		"chest":
			return Color(0.85, 0.65, 0.20)
		"fountain":
			return Color(0.45, 0.60, 0.72)
		"market_stall":
			return Color(0.60, 0.40, 0.25)
		"torch":
			return Color(0.80, 0.55, 0.25)
		"door":
			return Color(0.45, 0.30, 0.18)
		"stairs_down", "stairs_up":
			return Color(0.50, 0.42, 0.62)
		"shrine", "brazier":
			return Color(0.70, 0.62, 0.40)
		_:
			return Color(0.50, 0.50, 0.50)


## Animated water plane over the pond's bounding box (town zones only).
## Water tiles outside any model footprint still get the flat blue box
## underneath, so gaps in the plane never show through.
func build_water(tiles: Array, is_town: bool, covered: Dictionary) -> void:
	if not is_town:
		return
	var min_x := 1 << 30
	var min_y := 1 << 30
	var max_x := -(1 << 30)
	var max_y := -(1 << 30)
	for y in tiles.size():
		var row: Array = tiles[y]
		for x in row.size():
			if str(row[x]) == "water" and not covered.has("%d,%d" % [x, y]):
				min_x = mini(min_x, x)
				min_y = mini(min_y, y)
				max_x = maxi(max_x, x)
				max_y = maxi(max_y, y)
	if max_x < min_x:
		return # no open water in this zone
	var plane := PlaneMesh.new()
	plane.size = Vector2(float(max_x - min_x + 1), float(max_y - min_y + 1))
	var node := MeshInstance3D.new()
	node.mesh = plane
	var sm := ShaderMaterial.new()
	sm.shader = WATER_SHADER
	node.material_override = sm
	node.position = Vector3(
		(float(min_x) + float(max_x)) * 0.5,
		-0.06, # just above the sunken water-tile tops (-0.3), below ground (0)
		(float(min_y) + float(max_y)) * 0.5
	)
	terrain_root.add_child(node)


## Instance each .glb placement, scaled so its footprint spans roughly
## targetWidthTiles; fall back to a labelled placeholder box when the model
## file is not in the project.
func build_landmarks(placements: Array) -> void:
	forge_lights.clear()
	for p in placements:
		var fname := str(p.get("url", "")).get_file()
		var target := float(p.get("targetWidthTiles", 3.0))
		var y_off := float(p.get("yOffset", 0.0))
		var node: Node3D = null
		var path := "res://assets/models/" + fname
		if fname != "" and ResourceLoader.exists(path):
			var packed: Variant = load(path)
			if packed != null:
				node = packed.instantiate()
				var aabb := scene_aabb(node)
				var w := maxf(aabb.size.x, aabb.size.z)
				if w > 0.01:
					var s := target / w
					node.scale = Vector3.ONE * s
					y_off += -aabb.position.y * s # rest the model's base on y = 0
		if node == null:
			node = make_placeholder(str(p.get("id", fname)), target)
		node.position = Vector3(float(p["x"]), y_off, float(p["z"]))
		if p.has("rotationY"):
			node.rotation.y = float(p["rotationY"])
		landmarks_root.add_child(node)
		if p.get("light") != null:
			var ld: Dictionary = p["light"]
			var omni := OmniLight3D.new()
			var col := int(ld.get("color", 0xffffff))
			omni.light_color = Color(
				float((col >> 16) & 0xff) / 255.0,
				float((col >> 8) & 0xff) / 255.0,
				float(col & 0xff) / 255.0)
			omni.light_energy = float(ld.get("intensity", 2.0))
			omni.set_meta("base_energy", omni.light_energy)
			omni.omni_range = float(ld.get("distance", 6.0))
			omni.omni_attenuation = 1.2
			var off: Array = ld.get("offset", [0.0, 1.0, 0.0])
			# TS authored offsets in three.js handedness; Godot x runs the other way.
			var local := Vector3(-float(off[0]), float(off[1]), float(off[2]))
			omni.position = node.position + local.rotated(Vector3.UP, node.rotation.y)
			landmarks_root.add_child(omni)
			if col == 0xff8c2b:
				forge_lights.append(omni)


func make_placeholder(label_text: String, target: float) -> Node3D:
	var root := Node3D.new()
	var mi := MeshInstance3D.new()
	var box := BoxMesh.new()
	box.size = Vector3(target, 2.0, target)
	mi.mesh = box
	mi.position = Vector3(0.0, 1.0, 0.0)
	var mat := StandardMaterial3D.new()
	mat.albedo_color = Color(0.55, 0.45, 0.35)
	mi.material_override = mat
	root.add_child(mi)
	var label := Label3D.new()
	label.text = label_text
	label.font_size = 64
	label.pixel_size = 0.008
	label.billboard = BaseMaterial3D.BILLBOARD_ENABLED
	label.position = Vector3(0.0, 2.5, 0.0)
	root.add_child(label)
	return root


# Recursive AABB of a scene's meshes, in the scene root's local space.
var _aabb_acc := AABB()
var _aabb_first := true

func scene_aabb(root: Node3D) -> AABB:
	_aabb_first = true
	_aabb_acc = AABB()
	_collect_aabb(root, Transform3D.IDENTITY)
	return _aabb_acc

func _collect_aabb(n: Node, xf: Transform3D) -> void:
	var local := xf
	if n is Node3D:
		local = xf * (n as Node3D).transform
	if n is MeshInstance3D:
		var mi := n as MeshInstance3D
		if mi.mesh != null:
			var ab := local * mi.get_aabb()
			if _aabb_first:
				_aabb_acc = ab
				_aabb_first = false
			else:
				_aabb_acc = _aabb_acc.merge(ab)
	for c in n.get_children():
		_collect_aabb(c, local)


# ---------------------------------------------------------------------------
# Entities: capsules + loot boxes, rebuilt per refresh, lerped per frame
# ---------------------------------------------------------------------------

func refresh_entities() -> void:
	# Remember where things stood so a rebuild doesn't teleport them.
	var prev := {}
	for child in entities_root.get_children():
		if child.has_meta("ekey"):
			prev[str(child.get_meta("ekey"))] = child.position
	clear_children(entities_root)
	entity_nodes.clear()
	entity_targets.clear()

	var zone: Dictionary = state["zone"]
	var pl: Dictionary = state["player"]

	player_node = make_capsule(Color(0.30, 0.55, 1.00), 1.0)
	add_entity("player", player_node, Vector3(float(pl["x"]), 0.0, float(pl["y"])), prev)

	for e in zone.get("enemies", []):
		var boss := bool(e.get("isBoss", false))
		var n := make_capsule(Color(0.90, 0.25, 0.20), 1.35 if boss else 1.0)
		add_entity("enemy:" + str(e["id"]), n, Vector3(float(e["x"]), 0.0, float(e["y"])), prev)

	for npc in zone.get("npcs", []):
		var n := make_capsule(Color(0.30, 0.85, 0.40), 1.0)
		add_label(n, str(npc["name"]))
		add_entity("npc:" + str(npc["id"]), n, Vector3(float(npc["x"]), 0.0, float(npc["y"])), prev)

	for l in zone.get("loot", []):
		var n := make_loot_box()
		var key := "loot:%d,%d" % [int(l["x"]), int(l["y"])]
		add_entity(key, n, Vector3(float(l["x"]), 0.0, float(l["y"])), prev)


func add_entity(key: String, node: Node3D, target: Vector3, prev: Dictionary) -> void:
	node.set_meta("ekey", key)
	entities_root.add_child(node)
	node.position = prev.get(key, target) # start where it was (or at target if new)
	entity_nodes[key] = node
	entity_targets[node] = target


func make_capsule(color: Color, scale_factor: float) -> Node3D:
	var root := Node3D.new()
	var mi := MeshInstance3D.new()
	var cap := CapsuleMesh.new()
	cap.radius = 0.32
	cap.height = 1.15
	mi.mesh = cap
	mi.position = Vector3(0.0, 0.6, 0.0)
	var mat := StandardMaterial3D.new()
	mat.albedo_color = color
	mat.roughness = 0.7
	mi.material_override = mat
	root.add_child(mi)
	root.scale = Vector3.ONE * scale_factor
	return root


func make_loot_box() -> Node3D:
	var root := Node3D.new()
	var mi := MeshInstance3D.new()
	var box := BoxMesh.new()
	box.size = Vector3(0.45, 0.30, 0.45)
	mi.mesh = box
	mi.position = Vector3(0.0, 0.15, 0.0)
	var mat := StandardMaterial3D.new()
	mat.albedo_color = Color(0.95, 0.75, 0.25)
	mat.metallic = 0.4
	mat.roughness = 0.35
	mi.material_override = mat
	root.add_child(mi)
	return root


func add_label(parent: Node3D, text: String) -> void:
	var label := Label3D.new()
	label.text = text
	label.font_size = 64
	label.pixel_size = 0.008
	label.billboard = BaseMaterial3D.BILLBOARD_ENABLED
	label.modulate = Color(0.95, 0.97, 1.0)
	label.outline_size = 8
	label.outline_modulate = Color(0.05, 0.05, 0.08)
	label.position = Vector3(0.0, 1.55, 0.0)
	parent.add_child(label)


# ---------------------------------------------------------------------------
# Camera, input, per-frame updates
# ---------------------------------------------------------------------------

func _process(delta: float) -> void:
	# Smooth entity movement toward their tile positions.
	var w := minf(1.0, delta * 10.0)
	for node in entity_targets.keys():
		if is_instance_valid(node):
			node.position = node.position.lerp(entity_targets[node], w)
	update_camera(delta)
	update_held_movement(delta)


func update_camera(delta: float) -> void:
	_flicker_forge(delta)
	if player_node == null:
		return
	var target := player_node.position # entities_root sits at the origin
	if not camera_snapped:
		camera_rig.position = target
		camera_snapped = true
	else:
		camera_rig.position = camera_rig.position.lerp(target, minf(1.0, delta * 6.0))
	var offset := Vector3(0.0, 7.0, 9.0) if chase_mode else Vector3(0.0, 26.0, 14.0)
	camera.position = offset.rotated(Vector3.UP, cam_yaw)
	var look := camera_rig.position + Vector3(0.0, 0.8, 0.0)
	if camera.global_position.distance_squared_to(look) > 0.01:
		camera.look_at(look, Vector3.UP)


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey:
		var k := event as InputEventKey
		if not k.pressed or k.echo:
			return
		match k.physical_keycode:
			KEY_V:
				chase_mode = not chase_mode
			KEY_BRACKETLEFT:
				cam_yaw += PI * 0.5
			KEY_BRACKETRIGHT:
				cam_yaw -= PI * 0.5
			KEY_F, KEY_ENTER, KEY_KP_ENTER:
				do_interact()
			_:
				if merchant_panel.visible or dialogue_panel.visible:
					return # modal is up; don't walk behind it
				var d := key_dir(k.physical_keycode)
				if d != Vector2i.ZERO:
					do_move(d)
					move_timer = MOVE_REPEAT


## Screen-space step for a key: (x = right, y = away from camera).
func key_dir(k: int) -> Vector2i:
	match k:
		KEY_UP, KEY_W:
			return Vector2i(0, 1)
		KEY_DOWN, KEY_S:
			return Vector2i(0, -1)
		KEY_LEFT, KEY_A:
			return Vector2i(-1, 0)
		KEY_RIGHT, KEY_D:
			return Vector2i(1, 0)
		KEY_Q:
			return Vector2i(-1, 1)
		KEY_E:
			return Vector2i(1, 1)
		KEY_Z:
			return Vector2i(-1, -1)
		KEY_C:
			return Vector2i(1, -1)
	return Vector2i.ZERO


## Held-key walking: poll the same key set every MOVE_REPEAT seconds.
func update_held_movement(delta: float) -> void:
	move_timer -= delta
	if move_timer > 0.0 or merchant_panel.visible or dialogue_panel.visible:
		return
	var held := Vector2i.ZERO
	if Input.is_physical_key_pressed(KEY_W) or Input.is_physical_key_pressed(KEY_UP):
		held.y += 1
	if Input.is_physical_key_pressed(KEY_S) or Input.is_physical_key_pressed(KEY_DOWN):
		held.y -= 1
	if Input.is_physical_key_pressed(KEY_A) or Input.is_physical_key_pressed(KEY_LEFT):
		held.x -= 1
	if Input.is_physical_key_pressed(KEY_D) or Input.is_physical_key_pressed(KEY_RIGHT):
		held.x += 1
	if Input.is_physical_key_pressed(KEY_Q):
		held += Vector2i(-1, 1)
	if Input.is_physical_key_pressed(KEY_E):
		held += Vector2i(1, 1)
	if Input.is_physical_key_pressed(KEY_Z):
		held += Vector2i(-1, -1)
	if Input.is_physical_key_pressed(KEY_C):
		held += Vector2i(1, -1)
	held.x = clampi(held.x, -1, 1)
	held.y = clampi(held.y, -1, 1)
	if held != Vector2i.ZERO:
		do_move(held)
		move_timer = MOVE_REPEAT


# ---------------------------------------------------------------------------
# UI: HUD, log, town panel, dialogue, merchant — all built in code
# ---------------------------------------------------------------------------

func build_ui() -> void:
	var layer := CanvasLayer.new()
	layer.name = "UI"
	add_child(layer)

	# HUD, top-left.
	var hud_panel := make_panel()
	hud_panel.position = Vector2(10, 10)
	layer.add_child(hud_panel)
	var hud_box := VBoxContainer.new()
	hud_panel.add_child(hud_box)
	hud_title = make_label()
	hud_title.add_theme_font_size_override("font_size", 17)
	hud_box.add_child(hud_title)
	hud_body = make_label()
	hud_box.add_child(hud_body)

	# Log, bottom-left: the last 8 lines from the snapshot.
	var log_panel := make_panel()
	log_panel.anchor_top = 1.0
	log_panel.anchor_bottom = 1.0
	log_panel.offset_left = 10
	log_panel.offset_top = -148
	log_panel.offset_right = 550
	log_panel.offset_bottom = -10
	layer.add_child(log_panel)
	log_label = RichTextLabel.new()
	log_label.bbcode_enabled = false
	log_label.scroll_following = true
	log_label.custom_minimum_size = Vector2(520, 118)
	log_label.add_theme_font_size_override("normal_font_size", 13)
	log_label.add_theme_color_override("default_color", Color(0.88, 0.90, 0.94))
	log_panel.add_child(log_label)

	# Town panel, right side (visible in town zones only).
	town_panel = make_panel()
	town_panel.anchor_left = 1.0
	town_panel.anchor_right = 1.0
	town_panel.offset_left = -272
	town_panel.offset_right = -10
	town_panel.offset_top = 10
	town_panel.offset_bottom = 340
	layer.add_child(town_panel)
	town_box = VBoxContainer.new()
	town_panel.add_child(town_box)

	# Dialogue panel, bottom-centre.
	dialogue_panel = make_panel()
	dialogue_panel.anchor_left = 0.5
	dialogue_panel.anchor_right = 0.5
	dialogue_panel.anchor_top = 1.0
	dialogue_panel.anchor_bottom = 1.0
	dialogue_panel.offset_left = -300
	dialogue_panel.offset_right = 300
	dialogue_panel.offset_top = -196
	dialogue_panel.offset_bottom = -86
	layer.add_child(dialogue_panel)
	var dlg_box := VBoxContainer.new()
	dialogue_panel.add_child(dlg_box)
	dialogue_label = make_label()
	dialogue_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	dialogue_label.custom_minimum_size = Vector2(560, 0)
	dlg_box.add_child(dialogue_label)
	dialogue_choices = VBoxContainer.new()
	dlg_box.add_child(dialogue_choices)
	var dlg_close := make_button("Close")
	dlg_close.pressed.connect(_on_dialogue_close)
	dlg_box.add_child(dlg_close)
	dialogue_panel.hide()

	# Merchant modal, centred.
	merchant_panel = make_panel()
	merchant_panel.anchor_left = 0.5
	merchant_panel.anchor_right = 0.5
	merchant_panel.anchor_top = 0.5
	merchant_panel.anchor_bottom = 0.5
	merchant_panel.offset_left = -250
	merchant_panel.offset_right = 250
	merchant_panel.offset_top = -210
	merchant_panel.offset_bottom = 230
	layer.add_child(merchant_panel)
	var mer_box := VBoxContainer.new()
	merchant_panel.add_child(mer_box)
	merchant_title = make_label()
	merchant_title.add_theme_font_size_override("font_size", 17)
	mer_box.add_child(merchant_title)
	merchant_rows = VBoxContainer.new()
	mer_box.add_child(merchant_rows)
	var mer_close := make_button("Close")
	mer_close.pressed.connect(func() -> void: merchant_panel.hide())
	mer_box.add_child(mer_close)
	merchant_panel.hide()


func make_panel() -> PanelContainer:
	var p := PanelContainer.new()
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.08, 0.09, 0.12, 0.84)
	sb.set_corner_radius_all(6)
	sb.set_content_margin_all(10.0)
	p.add_theme_stylebox_override("panel", sb)
	return p


func make_label() -> Label:
	var l := Label.new()
	l.add_theme_font_size_override("font_size", 14)
	l.add_theme_color_override("font_color", Color(0.92, 0.93, 0.95))
	return l


func make_button(text: String) -> Button:
	var b := Button.new()
	b.text = text
	b.add_theme_font_size_override("font_size", 14)
	return b


func update_hud() -> void:
	var zone: Dictionary = state["zone"]
	var pl: Dictionary = state["player"]
	var derived: Dictionary = state.get("derived", {})
	hud_title.text = "%s — %s" % [str(zone["name"]), str(zone.get("subtitle", ""))]
	var max_hp := int(derived.get("maxHp", pl.get("maxHp", 1)))
	var max_mp := int(derived.get("maxMana", pl.get("maxMana", 0)))
	hud_body.text = "HP %s %d/%d\nMP %s %d/%d\nLv %d · %d g · ATK %d · DEF %d" % [
		text_bar(int(pl["hp"]), max_hp), int(pl["hp"]), max_hp,
		text_bar(int(pl["mana"]), max_mp), int(pl["mana"]), max_mp,
		int(pl["level"]), int(pl["gold"]),
		int(derived.get("atk", 0)), int(derived.get("def", 0)),
	]


func text_bar(cur: int, maxv: int, width: int = 14) -> String:
	var filled := 0
	if maxv > 0:
		filled = clampi(int(round(float(cur) / float(maxv) * float(width))), 0, width)
	return "█".repeat(filled) + "░".repeat(width - filled)


func update_log() -> void:
	var lines: PackedStringArray = []
	for l in state.get("log", []):
		lines.append(str(l))
	log_label.text = "\n".join(lines)


func rebuild_town_panel() -> void:
	clear_children(town_box)
	var zone: Dictionary = state["zone"]
	if not bool(zone.get("isTown", false)):
		town_panel.hide()
		return
	town_panel.show()

	var town: Dictionary = state.get("town", {})
	var pieces: Array = town.get("piecesFound", [])
	var supplies: Array = town.get("suppliesFound", [])

	var head := make_label()
	head.add_theme_font_size_override("font_size", 16)
	head.text = "— Town —"
	town_box.add_child(head)

	var lines := make_label()
	var wish := "done" if bool(town.get("wishSatisfied", false)) else "pending"
	lines.text = "Pieces: %d/3\nWish: %s\nPopulation: %d (Tier %d)" % [
		pieces.size(), wish,
		int(town.get("population", 0)), int(state.get("populationTier", 0)),
	]
	if not supplies.is_empty():
		lines.text += "\nSupplies: " + join_strs(supplies)
	town_box.add_child(lines)

	var carried: Array = state.get("carriedTownTags", [])
	if not carried.is_empty():
		var c := make_label()
		c.text = "Carrying: " + join_strs(carried)
		town_box.add_child(c)

	if bool(state.get("canCompleteBlacksmith", false)):
		var hint := make_label()
		hint.text = "Torin's forge can be rebuilt:"
		town_box.add_child(hint)
		for lot in LOTS:
			var b := make_button("Build: " + lot)
			b.pressed.connect(_on_complete_blacksmith.bind(lot))
			town_box.add_child(b)
	elif bool(town.get("blacksmithComplete", false)):
		var cur := str(town.get("blacksmithLot", ""))
		var at := make_label()
		at.text = "Forge standing: " + cur
		town_box.add_child(at)
		for lot in LOTS:
			if lot == cur:
				continue
			var b := make_button("Relocate: " + lot)
			b.pressed.connect(_on_relocate_blacksmith.bind(lot))
			town_box.add_child(b)


func _on_complete_blacksmith(lot: String) -> void:
	session.complete_blacksmith(lot)
	refresh()


func _on_relocate_blacksmith(lot: String) -> void:
	session.relocate_blacksmith(lot)
	refresh()


# ---------------------------------------------------------------------------
# Dialogue + merchant panels
# ---------------------------------------------------------------------------

func open_dialogue(_npc_id: String) -> void:
	render_dialogue()


## Render the session's current dialogue node (real npcs.rs trees via the
## bridge): speaker, text, and one button per condition-visible choice.
func render_dialogue() -> void:
	var parsed: Variant = JSON.parse_string(session.dialogue_json())
	if not (parsed is Dictionary):
		dialogue_panel.hide()
		return
	var node: Dictionary = parsed
	var speaker := str(node.get("speakerName", ""))
	var head := str(node.get("name", ""))
	if speaker != "":
		head = speaker
	var title := str(node.get("title", ""))
	if title != "":
		head += " — " + title
	dialogue_label.text = "%s\n%s" % [head, str(node.get("text", ""))]
	clear_children(dialogue_choices)
	for c in node.get("choices", []):
		var choice: Dictionary = c
		var b := make_button(str(choice.get("label", "...")))
		b.pressed.connect(_on_dialogue_choice.bind(str(choice.get("id", ""))))
		dialogue_choices.add_child(b)
	dialogue_panel.show()


func _on_dialogue_choice(choice_id: String) -> void:
	var ev: String = session.choose_dialogue(choice_id)
	if ev.begins_with("merchant:"):
		dialogue_panel.hide()
		refresh()
		open_merchant(ev.trim_prefix("merchant:"))
		return
	refresh()
	# Still talking (advanced node / failed check) or done?
	if str(JSON.parse_string(session.dialogue_json())) == "<null>":
		dialogue_panel.hide()
	else:
		render_dialogue()


func _on_dialogue_close() -> void:
	session.close_dialogue()
	dialogue_panel.hide()
	refresh()


func open_merchant(merchant_id: String) -> void:
	current_merchant_id = merchant_id
	merchant_title.text = "Merchant — " + merchant_id
	rebuild_merchant_rows()
	merchant_panel.show()


func rebuild_merchant_rows() -> void:
	clear_children(merchant_rows)
	var parsed: Variant = JSON.parse_string(session.merchant_stock_json(current_merchant_id))
	if not (parsed is Array):
		var none := make_label()
		none.text = "(no stock)"
		merchant_rows.add_child(none)
		return
	var stock: Array = parsed
	for i in stock.size():
		var item: Dictionary = stock[i]
		var row := HBoxContainer.new()
		var l := make_label()
		l.text = "%s (%s) — %d g" % [str(item.get("name", "?")), str(item.get("rarity", "")), int(item.get("value", 0))]
		l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		row.add_child(l)
		var b := make_button("Buy")
		b.pressed.connect(_on_buy_item.bind(i))
		row.add_child(b)
		merchant_rows.add_child(row)


func _on_buy_item(index: int) -> void:
	if session.buy_item(current_merchant_id, index):
		refresh() # gold/inventory changed; snapshot shows it
		rebuild_merchant_rows() # stock may have changed too


# ---------------------------------------------------------------------------
# Small helpers
# ---------------------------------------------------------------------------

func join_strs(arr: Array) -> String:
	var parts: PackedStringArray = []
	for v in arr:
		parts.append(str(v))
	return ", ".join(parts)


func clear_children(node: Node) -> void:
	for child in node.get_children():
		node.remove_child(child)
		child.free()


func _flicker_forge(delta: float) -> void:
	if forge_lights.is_empty():
		return
	_flicker_t += delta
	for i in forge_lights.size():
		var l: OmniLight3D = forge_lights[i]
		if is_instance_valid(l):
			var base := float(l.get_meta("base_energy", 3.0))
			var n := sin(_flicker_t * 11.0 + float(i) * 1.7) * 0.6 + sin(_flicker_t * 23.7 + float(i) * 3.1) * 0.4
			l.light_energy = base * (0.9 + 0.1 * n)
