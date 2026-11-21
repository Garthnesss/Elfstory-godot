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
	"vault_door": 2.6, # sealed doors stand in the wall line until opened
}

# Combat/loot feel (REEBOO Stylized VFX sample, MIT — addons/reeboo_vfx).
const VFX_HIT := preload("res://addons/reeboo_vfx/effects/hit_spark.tscn")
const VFX_SLASH := preload("res://addons/reeboo_vfx/effects/slash.tscn")
const VFX_SPARKLE := preload("res://addons/reeboo_vfx/effects/pickup_sparkle.tscn")

## Sound per core fx cue / UI action: the first path that exists plays.
## assets/audio/ui = Kenney Interface Sounds (CC0). assets/audio/rpg =
## Kenney RPG Audio (CC0) once it's in the project; until then combat cues
## with only rpg paths stay silent.
const SFX := {
	"hit": ["res://assets/audio/rpg/knifeSlice.ogg"],
	"crit": ["res://assets/audio/rpg/knifeSlice2.ogg", "res://assets/audio/rpg/knifeSlice.ogg"],
	"kill": ["res://assets/audio/rpg/chop.ogg"],
	"player_hit": ["res://assets/audio/rpg/cloth1.ogg"],
	"pickup": ["res://assets/audio/rpg/handleCoins.ogg", "res://assets/audio/ui/drop_002.wav"],
	"chest": ["res://assets/audio/rpg/metalLatch.ogg", "res://assets/audio/ui/confirmation_002.wav"],
	"brazier": ["res://assets/audio/ui/maximize_006.wav"],
	"waystone": ["res://assets/audio/ui/maximize_006.wav"],
	"unseal": ["res://assets/audio/rpg/doorOpen_1.ogg", "res://assets/audio/ui/bong_001.wav"],
	"level_up": ["res://assets/audio/ui/confirmation_004.wav"],
	"prompt": ["res://assets/audio/ui/question_001.wav"],
	"iron_fortress": ["res://assets/audio/rpg/metalPot1.ogg", "res://assets/audio/ui/maximize_006.wav"],
	"frost_nova": ["res://assets/audio/ui/maximize_006.wav"],
	"shadowstep": ["res://assets/audio/rpg/cloth1.ogg"],
	"ui_click": ["res://assets/audio/ui/click_001.wav"],
}
const STEP_SFX := [
	"res://assets/audio/rpg/footstep00.ogg", "res://assets/audio/rpg/footstep01.ogg",
	"res://assets/audio/rpg/footstep02.ogg", "res://assets/audio/rpg/footstep03.ogg",
]
const SFX_VOICES := 6
const SLASH_SCALE := 0.55 # slash.tscn spans ~3 units; tiles are 1
const SPARKLE_SECONDS := 0.8

var session: Object # ElfStorySession (GDExtension class)
var state: Dictionary = {}

var current_zone_id := ""
var current_build_key := "" # zone id + town JSON; a change rebuilds terrain/landmarks

var terrain_root: Node3D
var landmarks_root: Node3D
var forge_lights: Array = []
var _flicker_t := 0.0
var entities_root: Node3D
var fx_root: Node3D
var sfx_players: Array = []
var sfx_next := 0
var sfx_cache := {} # path -> AudioStream (or null when missing)

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
var hotbar_box: HBoxContainer
var skills_panel: PanelContainer
var skills_box: VBoxContainer
var aim_marker: MeshInstance3D
var telegraph_root: Node3D
var prompt_open := false
var title_panel: PanelContainer
var saving_enabled := true # off in --elfstory-capture smoke runs
var actions_since_save := 0
const AUTOSAVE_EVERY := 25 # forwarded actions between autosaves # the dialogue panel is showing a world-tile prompt


func _ready() -> void:
	sun.rotation_degrees = Vector3(-26.0, -45.0, 0.0) # golden hour: low western sun

	terrain_root = Node3D.new()
	terrain_root.name = "Terrain"
	add_child(terrain_root)
	landmarks_root = Node3D.new()
	landmarks_root.name = "Landmarks"
	add_child(landmarks_root)
	entities_root = Node3D.new()
	entities_root.name = "Entities"
	add_child(entities_root)
	fx_root = Node3D.new()
	fx_root.name = "FX"
	add_child(fx_root)
	telegraph_root = Node3D.new()
	telegraph_root.name = "Telegraphs"
	add_child(telegraph_root)
	aim_marker = make_tile_marker(Color(1.0, 0.85, 0.25, 0.75), 0.06)
	aim_marker.visible = false
	add_child(aim_marker)
	for i in SFX_VOICES:
		var ap := AudioStreamPlayer.new()
		add_child(ap)
		sfx_players.append(ap)

	build_ui()

	session = ElfStorySession.new()
	session.new_game("Garth", "Vanguard", 20261120)
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--elfstory-capture="):
			saving_enabled = false # smoke runs never touch the player's save
	refresh()
	if saving_enabled:
		show_title_if_save()
	_maybe_smoke_capture()


## Smoke test for exported builds: `ElfStory -- --elfstory-capture=/abs/shot.png`
## renders ~2 s of Oakhaven, saves the frame and quits. No effect otherwise.
func _maybe_smoke_capture() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--elfstory-capture="):
			var path := arg.trim_prefix("--elfstory-capture=")
			await get_tree().create_timer(2.0).timeout
			await RenderingServer.frame_post_draw
			var err := get_viewport().get_texture().get_image().save_png(path)
			print("ELFSTORY_CAPTURE ", path, " err=", err)
			get_tree().quit(0 if err == OK else 1)
			return


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
	# tileRev bumps when a tile changes in place (vault unsealed, chest
	# looted, waystone attuned), so terrain rebuilds for those too.
	var build_key := "%s|%s|%s" % [zid, JSON.stringify(state.get("town", {})), str(zone.get("tileRev", 0))]
	if zid != current_zone_id or build_key != current_build_key:
		current_zone_id = zid
		current_build_key = build_key
		rebuild_world()
	refresh_entities()
	actions_since_save += 1
	if actions_since_save >= AUTOSAVE_EVERY:
		autosave()
	update_combat_overlays()
	update_hotbar()
	if skills_panel.visible:
		rebuild_skills_panel()
	play_fx()


## Bridge events are plain strings; panels are the only side effects here —
## the snapshot refresh that follows every action shows the consequences.
func handle_event(ev: String) -> void:
	if ev.begins_with("zone_changed:") or ev == "player_died":
		autosave()
	if ev.begins_with("dialogue:"):
		open_dialogue(ev.trim_prefix("dialogue:"))
	elif ev.begins_with("prompt:"):
		play_sfx("prompt")
		render_prompt()
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
	var before: Dictionary = state.get("player", {})
	var ev: String = session.move_player(world_dir.x, world_dir.y)
	handle_event(ev)
	refresh()
	var after: Dictionary = state.get("player", {})
	if int(after.get("x", 0)) != int(before.get("x", 0)) or int(after.get("y", 0)) != int(before.get("y", 0)):
		play_step()


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
	forge_lights.clear() # the lights were children of landmarks_root (freed above)

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

	var used := {}
	for u in zone.get("usedTiles", []):
		used["%d,%d" % [int(u[0]), int(u[1])]] = true

	var flat_items: Array = []
	var tall_items: Array = []
	for y in tiles.size():
		var row: Array = tiles[y]
		for x in row.size():
			if covered.has("%d,%d" % [x, y]):
				continue
			var kind := str(row[x])
			var item := {"x": x, "y": y, "kind": kind, "used": used.has("%d,%d" % [x, y])}
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
		var c := tile_color(kind, is_town)
		if bool(it.get("used", false)):
			c = c.darkened(0.45) # looted chest / spent brazier / attuned waystone
		mm.set_instance_color(i, c)
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
		"vault_door":
			return Color(0.62, 0.46, 0.18)
		"waystone":
			return Color(0.30, 0.66, 0.85)
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
		if title_panel != null:
			return # Continue / New Game first
		match k.physical_keycode:
			KEY_V:
				chase_mode = not chase_mode
			KEY_BRACKETLEFT:
				cam_yaw += PI * 0.5
			KEY_BRACKETRIGHT:
				cam_yaw -= PI * 0.5
			KEY_F, KEY_ENTER, KEY_KP_ENTER:
				if is_aiming():
					do_session_action(func() -> String: return session.confirm_target())
				else:
					do_interact()
			KEY_K:
				toggle_skills_panel()
			KEY_ESCAPE:
				if is_aiming():
					session.cancel_aim()
					refresh()
				elif skills_panel.visible:
					skills_panel.hide()
			_:
				if merchant_panel.visible or dialogue_panel.visible or skills_panel.visible:
					return # modal is up; don't act behind it
				var code := k.physical_keycode
				if code >= KEY_1 and code <= KEY_5:
					var slot := int(code - KEY_1)
					do_session_action(func() -> String: return session.trigger_hotbar(slot))
					return
				if code == KEY_TAB:
					session.cycle_target()
					refresh()
					return
				if code == KEY_SPACE or code == KEY_PERIOD:
					do_session_action(func() -> String: return session.wait_turn())
					return
				var d := key_dir(code)
				if d != Vector2i.ZERO:
					if is_aiming():
						var w := screen_dir_to_world(d)
						session.move_aim(w.x, w.y)
						refresh()
					else:
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
	if move_timer > 0.0 or title_panel != null or merchant_panel.visible or dialogue_panel.visible or skills_panel.visible or is_aiming():
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

	# Skill hotbar, bottom-right: keys 1-5.
	var hotbar_panel := make_panel()
	hotbar_panel.anchor_left = 1.0
	hotbar_panel.anchor_right = 1.0
	hotbar_panel.anchor_top = 1.0
	hotbar_panel.anchor_bottom = 1.0
	hotbar_panel.offset_left = -590
	hotbar_panel.offset_right = -10
	hotbar_panel.offset_top = -78
	hotbar_panel.offset_bottom = -10
	layer.add_child(hotbar_panel)
	hotbar_box = HBoxContainer.new()
	hotbar_box.add_theme_constant_override("separation", 6)
	hotbar_panel.add_child(hotbar_box)

	# Skills panel (K), centred.
	skills_panel = make_panel()
	skills_panel.anchor_left = 0.5
	skills_panel.anchor_right = 0.5
	skills_panel.anchor_top = 0.5
	skills_panel.anchor_bottom = 0.5
	skills_panel.offset_left = -330
	skills_panel.offset_right = 330
	skills_panel.offset_top = -270
	skills_panel.offset_bottom = 270
	layer.add_child(skills_panel)
	var sk_scroll := ScrollContainer.new()
	sk_scroll.custom_minimum_size = Vector2(640, 520)
	skills_panel.add_child(sk_scroll)
	skills_box = VBoxContainer.new()
	skills_box.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	sk_scroll.add_child(skills_box)
	skills_panel.hide()

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
	b.pressed.connect(play_sfx.bind("ui_click"))
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
	var sts: PackedStringArray = []
	for st in pl.get("statuses", []):
		sts.append("%s %dt" % [str(st["kind"]).capitalize(), int(st["turns"])])
	if not sts.is_empty():
		hud_body.text += "\n" + " · ".join(sts)
	if int(pl.get("skillPoints", 0)) > 0:
		hud_body.text += "\n%d skill point(s) — press K" % int(pl["skillPoints"])


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
	session.close_prompt()
	prompt_open = false
	dialogue_panel.hide()
	refresh()


## World-tile prompt (vault door / seal door / waystone) from the core,
## shown in the dialogue panel: title, subtitle, body, one button per option
## (disabled options render disabled; the core also refuses them).
func render_prompt() -> void:
	var parsed: Variant = JSON.parse_string(session.prompt_json())
	if not (parsed is Dictionary):
		prompt_open = false
		dialogue_panel.hide()
		return
	var p: Dictionary = parsed
	prompt_open = true
	dialogue_label.text = "%s — %s\n%s" % [str(p.get("title", "")), str(p.get("subtitle", "")), str(p.get("body", ""))]
	clear_children(dialogue_choices)
	for o in p.get("options", []):
		var opt: Dictionary = o
		var b := make_button(str(opt.get("label", "...")))
		b.disabled = bool(opt.get("disabled", false))
		b.pressed.connect(_on_prompt_choice.bind(str(opt.get("id", ""))))
		dialogue_choices.add_child(b)
	dialogue_panel.show()


func _on_prompt_choice(option_id: String) -> void:
	var ev: String = session.choose_prompt(option_id)
	refresh()
	handle_event(ev)
	if str(JSON.parse_string(session.prompt_json())) == "<null>":
		prompt_open = false
		dialogue_panel.hide()


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


# ---------------------------------------------------------------------------
# Presentation cues: VFX + sound for what the core says just happened
# ---------------------------------------------------------------------------

## Drain the core's fx cues (hit, kill, pickup, chest, ...) and play them.
## Called from refresh(), i.e. after every forwarded action.
func play_fx() -> void:
	var parsed: Variant = JSON.parse_string(session.take_fx_json())
	if not (parsed is Array):
		return
	for c in parsed:
		var cue: Dictionary = c
		var kind := str(cue.get("kind", ""))
		var at := Vector3(float(cue.get("x", 0)), 0.6, float(cue.get("y", 0)))
		var dir := Vector3(float(cue.get("dx", 0)), 0.0, float(cue.get("dy", 0)))
		match kind:
			"hit", "crit":
				# The slash is authored at character scale, swinging along -Z
				# from the attacker's origin: spawn it on the attacker's tile,
				# facing the target, shrunk to the 1-unit tile grid.
				if dir.length() > 0.0:
					var from := Vector3(at.x - dir.x, 0.0, at.z - dir.z)
					var swing := VFXEffect.spawn(VFX_SLASH, fx_root, from, Basis.looking_at(dir.normalized(), Vector3.UP))
					swing.scale = Vector3.ONE * SLASH_SCALE
				spawn_vfx(VFX_HIT, at, 1.5 if kind == "crit" else 1.0)
			"kill":
				spawn_vfx(VFX_HIT, at, 1.8)
			"player_hit":
				spawn_vfx(VFX_HIT, at, 0.7)
			"unseal":
				spawn_vfx(VFX_HIT, at + Vector3(0, 0.6, 0), 2.0)
			"shield_bash":
				spawn_vfx(VFX_HIT, at, 1.4)
			"frost_nova":
				spawn_vfx(VFX_HIT, at, 3.0)
			"iron_fortress", "shadowstep":
				spawn_sparkle(at, 1.6)
			"ember_lance", "venom_tip", "executioners_mark":
				spawn_vfx(VFX_HIT, at, 1.2)
			"pickup", "chest", "brazier", "waystone":
				spawn_sparkle(at, 1.0)
			"level_up":
				spawn_sparkle(at, 1.6)
		play_sfx(kind)


func spawn_vfx(scene: PackedScene, at: Vector3, scale: float) -> void:
	var fx := VFXEffect.spawn(scene, fx_root, at)
	fx.scale = Vector3.ONE * scale


## pickup_sparkle loops until stopped; stop() lets live particles finish
## and then frees the node (auto_free).
func spawn_sparkle(at: Vector3, scale: float) -> void:
	var fx := VFXEffect.spawn(VFX_SPARKLE, fx_root, at)
	fx.scale = Vector3.ONE * scale
	get_tree().create_timer(SPARKLE_SECONDS).timeout.connect(func():
		if is_instance_valid(fx):
			fx.stop())


func play_sfx(kind: String) -> void:
	var stream := sfx_stream(kind)
	if stream == null:
		return
	var ap: AudioStreamPlayer = sfx_players[sfx_next]
	sfx_next = (sfx_next + 1) % sfx_players.size()
	ap.stream = stream
	ap.volume_db = 0.0
	ap.play()


## Footstep on a successful move (a random one of four, slightly quieter).
func play_step() -> void:
	var path: String = STEP_SFX[randi() % STEP_SFX.size()]
	if not sfx_cache.has(path):
		sfx_cache[path] = load(path) if ResourceLoader.exists(path) else null
	if sfx_cache[path] == null:
		return
	var ap: AudioStreamPlayer = sfx_players[sfx_next]
	sfx_next = (sfx_next + 1) % sfx_players.size()
	ap.stream = sfx_cache[path]
	ap.volume_db = -8.0
	ap.play()


func sfx_stream(kind: String) -> AudioStream:
	for path in SFX.get(kind, []):
		if not sfx_cache.has(path):
			sfx_cache[path] = load(path) if ResourceLoader.exists(path) else null
		if sfx_cache[path] != null:
			return sfx_cache[path]
	return null


# ---------------------------------------------------------------------------
# Skills: hotbar, aiming, telegraphs, skills panel (rules in combat_turn.rs)
# ---------------------------------------------------------------------------

func is_aiming() -> bool:
	return state.get("armedSkill") != null


## Forward one session call that returns a bridge event, then repaint.
func do_session_action(call: Callable) -> void:
	var ev: String = call.call()
	handle_event(ev)
	refresh()


func update_hotbar() -> void:
	clear_children(hotbar_box)
	var armed = state.get("armedSkill")
	var slots: Array = state.get("hotbar", [])
	for i in slots.size():
		var slot = slots[i]
		var b := make_button("")
		b.custom_minimum_size = Vector2(102, 52)
		b.add_theme_font_size_override("font_size", 12)
		if slot == null:
			b.text = "%d\n—" % (i + 1)
			b.disabled = true
		else:
			var sk: Dictionary = slot
			var cd := int(sk.get("cooldown", 0))
			var tail := "%d MP" % int(sk.get("manaCost", 0))
			if cd > 0:
				tail = "cd %dt" % cd
			b.text = "%d %s %s\n%s" % [i + 1, str(sk.get("glyph", "")), str(sk.get("name", "")), tail]
			b.tooltip_text = "%s · rank %d · range %d" % [str(sk.get("name", "")), int(sk.get("rank", 0)), int(sk.get("range", 0))]
			if not bool(sk.get("ready", false)):
				b.modulate = Color(0.6, 0.6, 0.65)
			if armed != null and str(armed) == str(sk.get("id", "")):
				b.modulate = Color(1.0, 0.9, 0.4)
			b.pressed.connect(func() -> void:
				do_session_action(func() -> String: return session.trigger_hotbar(i)))
		hotbar_box.add_child(b)


## Aim cursor on the aimed tile; red tiles where telegraphed enemy attacks
## will land next turn.
func update_combat_overlays() -> void:
	var aim = state.get("aimedTarget")
	aim_marker.visible = is_aiming() and aim != null
	if aim_marker.visible:
		aim_marker.position = Vector3(float(aim["x"]), 0.04, float(aim["y"]))
	clear_children(telegraph_root)
	var zone: Dictionary = state["zone"]
	for e in zone.get("enemies", []):
		var intent = e.get("intent")
		if intent == null:
			continue
		for t in intent.get("targetTiles", []):
			var m := make_tile_marker(Color(0.95, 0.15, 0.1, 0.45), 0.03)
			m.position = Vector3(float(t["x"]), 0.02, float(t["y"]))
			telegraph_root.add_child(m)


func make_tile_marker(color: Color, height: float) -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	var box := BoxMesh.new()
	box.size = Vector3(0.92, height, 0.92)
	mi.mesh = box
	var mat := StandardMaterial3D.new()
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	mat.albedo_color = color
	mi.material_override = mat
	return mi


func toggle_skills_panel() -> void:
	if skills_panel.visible:
		skills_panel.hide()
	else:
		rebuild_skills_panel()
		skills_panel.show()


func rebuild_skills_panel() -> void:
	clear_children(skills_box)
	var parsed: Variant = JSON.parse_string(session.skills_json())
	if not (parsed is Dictionary):
		return
	var data: Dictionary = parsed
	var head := make_label()
	head.add_theme_font_size_override("font_size", 17)
	head.text = "Skills — %d point(s) to spend   (K / Esc to close)" % int(data.get("skillPoints", 0))
	skills_box.add_child(head)
	var branch := ""
	for sk_v in data.get("skills", []):
		var sk: Dictionary = sk_v
		if str(sk["branch"]) != branch:
			branch = str(sk["branch"])
			var bl := make_label()
			bl.add_theme_font_size_override("font_size", 15)
			bl.add_theme_color_override("font_color", Color(1.0, 0.82, 0.45))
			bl.text = branch.capitalize()
			skills_box.add_child(bl)
		var row := HBoxContainer.new()
		var l := make_label()
		l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		l.custom_minimum_size = Vector2(500, 0)
		var kind := "passive" if str(sk["kind"]) == "passive" else "%d MP · cd %d" % [int(sk["manaCost"]), int(sk["cooldown"])]
		var desc := str(sk["summary"])
		if sk.get("next") != null:
			desc += "\nNext: " + str(sk["next"])
		l.text = "%s %s  (tier %d, rank %d/3, %s)\n%s" % [str(sk["glyph"]), str(sk["name"]), int(sk["tier"]), int(sk["rank"]), kind, desc]
		row.add_child(l)
		var b := make_button("Learn")
		b.disabled = not bool(sk.get("canLearn", false))
		if sk.get("reason") != null:
			b.tooltip_text = str(sk["reason"])
		var sid := str(sk["id"])
		b.pressed.connect(func() -> void:
			if session.learn_skill(sid):
				refresh()
				rebuild_skills_panel())
		row.add_child(b)
		skills_box.add_child(row)


# ---------------------------------------------------------------------------
# Save / continue (single slot; rules + format in session_save.rs)
# ---------------------------------------------------------------------------

func save_dir() -> String:
	return OS.get_user_data_dir().path_join("saves")


func autosave() -> void:
	if not saving_enabled or title_panel != null:
		return # nothing to save before the player picks Continue / New Game
	actions_since_save = 0
	if not session.save_game(save_dir()):
		push_warning("ElfStory: autosave failed (%s)" % save_dir())


func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		autosave()


## On launch: if slot 1 holds a game, offer Continue / New Game over the
## fresh town; unreadable saves are explained, never silently replaced.
func show_title_if_save() -> void:
	var parsed: Variant = JSON.parse_string(session.save_summary_json(save_dir()))
	if not (parsed is Dictionary):
		return
	var sum: Dictionary = parsed
	var status := str(sum.get("status", "empty"))
	if status == "empty":
		return
	title_panel = make_panel()
	title_panel.anchor_left = 0.5
	title_panel.anchor_right = 0.5
	title_panel.anchor_top = 0.5
	title_panel.anchor_bottom = 0.5
	title_panel.offset_left = -220
	title_panel.offset_right = 220
	title_panel.offset_top = -85
	title_panel.offset_bottom = 70
	hud_title.get_parent().get_parent().get_parent().add_child(title_panel) # the UI CanvasLayer
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 10)
	title_panel.add_child(box)
	var head := make_label()
	head.add_theme_font_size_override("font_size", 22)
	head.text = "ElfStory"
	head.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	box.add_child(head)
	if status == "ok":
		var cont := make_button("Continue — %s, level %d" % [str(sum.get("zone", "?")), int(sum.get("level", 1))])
		cont.pressed.connect(_on_title_continue)
		box.add_child(cont)
		var note := make_label()
		note.text = "New Game replaces this save at the next autosave."
		note.add_theme_font_size_override("font_size", 12)
		box.add_child(note)
	else:
		var msg := make_label()
		msg.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		msg.custom_minimum_size = Vector2(400, 0)
		msg.text = str(sum.get("message", "This save can't be loaded."))
		box.add_child(msg)
	var fresh := make_button("New Game")
	fresh.pressed.connect(_on_title_new_game)
	box.add_child(fresh)


func _on_title_continue() -> void:
	var res: String = session.load_game(save_dir())
	close_title()
	if res != "ok":
		push_warning("ElfStory: load failed: " + res)
	current_zone_id = "" # force a full rebuild for the loaded zone
	current_build_key = ""
	refresh()


func _on_title_new_game() -> void:
	close_title()
	autosave()
	refresh()


func close_title() -> void:
	if title_panel != null:
		title_panel.queue_free()
		title_panel = null

