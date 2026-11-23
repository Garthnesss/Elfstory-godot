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
const TILE_SHADER: Shader = preload("res://tile.gdshader")
const Grass := preload("res://grass.gd")
## Zones whose "floor" tiles grow meadow grass + wildflowers (grass.gd).
const GRASS_ZONES := ["oakhaven", "overworld"]
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
	"town_build": ["res://assets/audio/ui/confirmation_004.wav", "res://assets/audio/rpg/metalPot1.ogg"],
	"town_relocate": ["res://assets/audio/ui/confirmation_002.wav"],
	"town_tier_up": ["res://assets/audio/ui/confirmation_004.wav"],
	"companion": ["res://assets/audio/ui/maximize_006.wav"],
	"companion_hit": ["res://assets/audio/rpg/knifeSlice.ogg"],
	"deathless": ["res://assets/audio/ui/bong_001.wav"],
	"spore_burst": ["res://assets/audio/rpg/cloth1.ogg"],
	"boss_roar": ["res://assets/audio/ui/bong_001.wav"],
	"haste": ["res://assets/audio/ui/maximize_006.wav"],
	"ui_click": ["res://assets/audio/ui/click_001.wav"],
}
const STEP_SFX := [
	"res://assets/audio/rpg/footstep00.ogg", "res://assets/audio/rpg/footstep01.ogg",
	"res://assets/audio/rpg/footstep02.ogg", "res://assets/audio/rpg/footstep03.ogg",
]
const SFX_VOICES := 6
const SLASH_SCALE := 0.55 # slash.tscn spans ~3 units; tiles are 1
const SPARKLE_SECONDS := 0.8

var session: ElfStorySession
var state: Dictionary = {}

var current_zone_id := ""
var current_build_key := ""
var grass_zone := false # floor tiles take Grass.GROUND under the meadow # zone id + town JSON; a change rebuilds terrain/landmarks

var terrain_root: Node3D
var cutaway_mat: ShaderMaterial # tall-tile material; player/camera fed each frame
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
var entity_hp: Dictionary = {} # key -> hp in the last snapshot
var entities_zone := ""
var dying: Dictionary = {} # "x,y" -> Node3D removed from the snapshot, awaiting its kill cue
var pending_numbers: Dictionary = {} # "x,y" -> damage taken this action
var cue_queue: Array = [] # [{"t": seconds until play, "cue": Dictionary}]
const CUE_STAGGER := 0.11 # gap between consecutive blows in one turn
const Juice := preload("res://juice.gd")
var juice: Juice

var cam_yaw := 0.0 # radians, snapped to 90-degree steps by [ and ]
var chase_mode := false # false = Overhead Tactical, true = Chase (V toggles)
var cam_zoom := 1.0 # mouse wheel / - = ; scales the overhead offset
const CAM_ZOOM_MIN := 0.65
const CAM_ZOOM_MAX := 1.7
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
const TitleScreen := preload("res://title_screen.gd")
var title_screen: TitleScreen = null # full-screen title (title_screen.gd); null once a game is picked
var active_slot := 1 # save slot (1-3) autosave writes to
var save_dir_override := "" # dev: --elfstory-save-dir=<abs path>
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
	juice = Juice.new()
	juice.name = "Juice"
	add_child(juice)
	fx_root = Node3D.new()
	fx_root.name = "FX"
	add_child(fx_root)
	telegraph_root = Node3D.new()
	telegraph_root.name = "Telegraphs"
	add_child(telegraph_root)
	aim_marker = make_tile_marker(Color(1.0, 0.85, 0.25, 0.75), 0.06)
	aim_marker.visible = false
	add_child(aim_marker)
	for i: int in SFX_VOICES:
		var ap := AudioStreamPlayer.new()
		add_child(ap)
		sfx_players.append(ap)

	build_ui()

	session = ElfStorySession.new()
	session.new_game("Garth", "Vanguard", 20261120)
	var force_title := false
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--elfstory-capture="):
			saving_enabled = false # smoke runs never touch the player's save
		elif arg == "--elfstory-show-title":
			force_title = true # dev: capture the title screen
		elif arg.begins_with("--elfstory-save-dir="):
			save_dir_override = arg.trim_prefix("--elfstory-save-dir=")
	refresh()
	if saving_enabled or force_title:
		show_title()
	_maybe_smoke_capture()


## Smoke test for exported builds: `ElfStory -- --elfstory-capture=/abs/shot.png`
## renders ~2 s of Oakhaven, saves the frame and quits. No effect otherwise.
func _maybe_smoke_capture() -> void:
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--elfstory-capture="):
			var path := arg.trim_prefix("--elfstory-capture=")
			# Dev-only framing for captures: --elfstory-warp=<zone>[,x,y]
			# (debug builds) and --elfstory-chase for the chase camera.
			for a: String in OS.get_cmdline_user_args():
				if a == "--elfstory-chase":
					chase_mode = true
				elif a.begins_with("--elfstory-warp=") and session.has_method("debug_warp"):
					var parts := a.trim_prefix("--elfstory-warp=").split(",")
					var wx := int(parts[1]) if parts.size() > 2 else -1
					var wy := int(parts[2]) if parts.size() > 2 else -1
					session.debug_warp(parts[0], wx, wy)
					camera_snapped = false
					refresh()
				elif a.begins_with("--elfstory-demo-town=") and session.has_method("debug_set_town"):
					var parts := a.trim_prefix("--elfstory-demo-town=").split(",")
					var arg_v := int(parts[0])
					var tier_pops: Array[int] = [12, 22, 38, 58]
					var pop: int = tier_pops[arg_v] if (arg_v >= 0 and arg_v <= 3) else arg_v
					var lot := parts[1] if parts.size() > 1 else ""
					session.debug_set_town(pop, lot)
					refresh()
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
	if not (parsed is Dictionary):
		push_warning("ElfStory: state_json() did not parse")
		return
	state = parsed as Dictionary

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
	if _as_int(after.get("x", 0)) != _as_int(before.get("x", 0)) or _as_int(after.get("y", 0)) != _as_int(before.get("y", 0)):
		play_step()


func do_interact() -> void:
	var ev: String = session.interact()
	handle_event(ev)
	refresh()


## Convert a screen-space step (x = right, y = up/away from camera) into a
## world tile step, honouring the quarter-turn camera yaw. Yaw is always a
## multiple of 90 degrees, so the result is axis- or diagonal-aligned.
func screen_dir_to_world(sd: Vector2i) -> Vector2i:
	var offset := cam_offset()
	var rotated := offset.rotated(Vector3.UP, cam_yaw)
	var fwd := Vector2(-rotated.x, -rotated.z).normalized() # away from camera, tile (x, y)
	var right := Vector2(-fwd.y, fwd.x) # screen right, tile (x, y)
	var w := right * float(sd.x) + fwd * float(sd.y)
	return Vector2i(roundi(w.x), roundi(w.y))


# ---------------------------------------------------------------------------
# World building: terrain MultiMeshes, water, landmark .glb placements
# ---------------------------------------------------------------------------

## Per-zone light mood (the report's "per-biome LUT + fog"): golden hour
## above ground, cooler and dimmer below, tinted per side branch.
## [sun colour, sun energy, sun pitch, ambient colour, ambient energy, fog colour, fog density,
##  floor colour, wall colour] (null = keep tile_color's default)
var mood_floor: Variant = null
var mood_wall: Variant = null
const ZONE_MOODS := {
	"surface": [Color(1.0, 0.78, 0.56), 1.9, -26.0, Color(0.56, 0.66, 0.86), 0.55, Color(0.94, 0.74, 0.58), 0.0012, null, null],
	"dungeon": [Color(0.78, 0.80, 0.95), 1.15, -52.0, Color(0.40, 0.46, 0.62), 0.75, Color(0.20, 0.22, 0.30), 0.010, Color(0.36, 0.37, 0.34), Color(0.30, 0.29, 0.33)],
	"barrow": [Color(0.86, 0.80, 0.70), 1.05, -55.0, Color(0.42, 0.40, 0.46), 0.75, Color(0.22, 0.20, 0.22), 0.012, Color(0.42, 0.36, 0.28), Color(0.34, 0.30, 0.26)],
	"mire": [Color(0.80, 0.95, 0.78), 1.1, -50.0, Color(0.36, 0.52, 0.48), 0.8, Color(0.16, 0.26, 0.22), 0.014, Color(0.26, 0.42, 0.24), Color(0.22, 0.27, 0.24)],
	"frost": [Color(0.82, 0.92, 1.0), 1.35, -48.0, Color(0.52, 0.64, 0.86), 0.85, Color(0.55, 0.66, 0.80), 0.005, Color(0.62, 0.68, 0.74), Color(0.42, 0.48, 0.58)],
}


func apply_zone_mood(zone_id: String) -> void:
	var key := "surface"
	for k: String in ["dungeon", "barrow", "mire", "frost"]:
		if zone_id.begins_with(k + "_"):
			key = k
	var m: Array = ZONE_MOODS[key]
	sun.light_color = m[0] as Color
	sun.light_energy = _as_float(m[1], 1.0)
	sun.rotation_degrees = Vector3(_as_float(m[2], -45.0), -45.0, 0.0)
	var we: WorldEnvironment = $WorldEnvironment as WorldEnvironment
	var env: Environment = we.environment
	env.ambient_light_color = m[3] as Color
	env.ambient_light_energy = _as_float(m[4], 0.5)
	env.fog_light_color = m[5] as Color
	env.fog_density = _as_float(m[6], 0.001)
	mood_floor = m[7]
	mood_wall = m[8]


func rebuild_world() -> void:
	var zone_obj: Dictionary = state["zone"]
	apply_zone_mood(str(zone_obj["id"]))
	clear_children(terrain_root)
	clear_children(landmarks_root)
	cutaway_mat = null
	forge_lights.clear() # the lights were children of landmarks_root (freed above)

	var zone: Dictionary = state["zone"]
	var tiles: Array = zone["tiles"]
	var is_town := _as_bool(zone.get("isTown", false))
	var placements := fetch_placements()

	# GUARDRAIL: tiles under a placed model's hideTerrainFootprint are never
	# drawn — the .glb owns that ground, and camera cutaways must not bring
	# the tiles back. Keyed "x,y" in tile coords (footprint z == tile y).
	var covered := {}
	if str(zone["id"]) == "oakhaven":
		for p_var: Variant in placements:
			var p: Dictionary = p_var as Dictionary
			if p.has("hideTerrainFootprint"):
				var f: Dictionary = p["hideTerrainFootprint"]
				for tx: int in range(_as_int(f["minX"]), _as_int(f["maxX"]) + 1):
					for tz: int in range(_as_int(f["minZ"]), _as_int(f["maxZ"]) + 1):
						covered["%d,%d" % [tx, tz]] = true

	var used := {}
	for u_var: Variant in (zone.get("usedTiles", []) as Array):
		var u: Array = u_var as Array
		used["%d,%d" % [_as_int(u[0]), _as_int(u[1])]] = true

	grass_zone = GRASS_ZONES.has(str(zone["id"]))
	var flat_items: Array = []
	var tall_items: Array = []
	for y: int in tiles.size():
		var row: Array = tiles[y]
		for x: int in row.size():
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

	var grass_skip := covered.duplicate()
	if str(zone["id"]) == "oakhaven":
		for p_var: Variant in placements:
			var p: Dictionary = p_var as Dictionary
			if str(p.get("url", "")).begins_with("procedural://"):
				var hw := _as_float(p.get("targetWidthTiles", 2.0), 2.0) * 0.5
				var px := _as_float(p.get("x", 0.0))
				var pz := _as_float(p.get("z", 0.0))
				for tx: int in range(floori(px - hw - 0.5), ceili(px + hw + 0.5) + 1):
					for tz: int in range(floori(pz - hw - 0.5), ceili(pz + hw + 0.5) + 1):
						if absf(float(tx) - px) < hw + 0.4 and absf(float(tz) - pz) < hw + 0.4:
							grass_skip["%d,%d" % [tx, tz]] = true

	build_water(tiles, is_town, covered)
	if grass_zone:
		var grass := Grass.build(tiles, grass_skip)
		if grass != null:
			terrain_root.add_child(grass)
	# Placements are the Oakhaven town-lot models; anywhere else they'd
	# draw the town's buildings at their Oakhaven tile coordinates.
	if str(zone["id"]) == "oakhaven":
		build_landmarks(placements)


func fetch_placements() -> Array:
	var parsed: Variant = JSON.parse_string(session.placements_json())
	if parsed is Array:
		return parsed as Array
	return []


## One MultiMesh of unit boxes. Flat tiles top out at y = 0 (water sits
## lower, chests/fountains sit on top); tall kinds rise from y = -1.
func make_multimesh(items: Array, tall: bool, is_town: bool) -> MultiMeshInstance3D:
	if items.is_empty():
		return null
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	mm.use_custom_data = true # r = 1: wall fully enclosed by wall (dark rock cap)
	var wall_set := {}
	if tall and not is_town:
		for it_var: Variant in items:
			var it: Dictionary = it_var as Dictionary
			if str(it["kind"]) == "wall":
				wall_set["%d,%d" % [_as_int(it["x"]), _as_int(it["y"])]] = true
	var box := BoxMesh.new()
	box.size = Vector3.ONE
	mm.mesh = box
	mm.instance_count = items.size()
	for i: int in items.size():
		var it: Dictionary = items[i] as Dictionary
		var kind := str(it["kind"])
		var h := 1.0
		var bottom := -1.0
		if tall:
			h = _as_float(TALL_HEIGHTS[kind], 1.0)
		else:
			var fb := flat_box(kind)
			h = fb.x
			bottom = fb.y
		# Small gaps between boxes draw the tile grid; meadow floor closes
		# them so the grass reads as one field (Garth, Nov 21).
		var enclosed := 0.0
		var ix := _as_int(it["x"])
		var iy := _as_int(it["y"])
		if wall_set.has("%d,%d" % [ix, iy]):
			enclosed = 1.0
			for oy: int in range(-1, 2):
				for ox: int in range(-1, 2):
					if not wall_set.has("%d,%d" % [ix + ox, iy + oy]):
						enclosed = 0.0
		var gap := 1.0 if ((grass_zone and kind == "floor") or enclosed > 0.0) else 0.97
		var basis := Basis.from_scale(Vector3(gap, h, gap))
		var origin := Vector3(float(ix), bottom + h * 0.5, float(iy))
		mm.set_instance_transform(i, Transform3D(basis, origin))
		var c := tile_color(kind, is_town)
		if _as_bool(it.get("used", false)):
			c = c.darkened(0.45) # looted chest / spent brazier / attuned waystone
		mm.set_instance_color(i, c)
		mm.set_instance_custom_data(i, Color(enclosed, 0.0, 0.0, 0.0))
	var mmi := MultiMeshInstance3D.new()
	mmi.multimesh = mm
	var mat := ShaderMaterial.new()
	mat.shader = TILE_SHADER
	if tall:
		mat.set_shader_parameter("courses", 1.0)
		mat.set_shader_parameter("cutaway", 1.0)
		cutaway_mat = mat
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
	if kind == "floor" and mood_floor is Color:
		return mood_floor as Color
	if kind == "wall" and mood_wall is Color:
		return mood_wall as Color
	match kind:
		"floor":
			if grass_zone:
				return Grass.GROUND
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


## Instance each .glb placement (or procedural structure), scaled so its
## footprint spans roughly targetWidthTiles; fall back to a labelled
## placeholder box when a model file is missing.
func build_landmarks(placements: Array) -> void:
	forge_lights.clear()
	for p_var: Variant in placements:
		var p: Dictionary = p_var as Dictionary
		var url := str(p.get("url", ""))
		var fname := url.get_file()
		var target := _as_float(p.get("targetWidthTiles", 3.0), 3.0)
		var y_off := _as_float(p.get("yOffset", 0.0))
		var node: Node3D = null
		if url == "procedural://cold_forge_ruin":
			node = make_cold_forge_ruin(target)
		elif url == "procedural://village_cottage":
			node = make_village_cottage(target)
		elif url == "procedural://village_field":
			node = make_village_field(target)
		else:
			var path := "res://assets/models/" + fname
			if fname != "" and ResourceLoader.exists(path):
				var packed: Resource = load(path)
				if packed is PackedScene:
					node = (packed as PackedScene).instantiate() as Node3D
					var aabb := scene_aabb(node)
					var w := maxf(aabb.size.x, aabb.size.z)
					if w > 0.01:
						var s := target / w
						node.scale = Vector3.ONE * s
						y_off += -aabb.position.y * s # rest the model's base on y = 0
		if node == null:
			node = make_placeholder(str(p.get("id", fname)), target)
		var px := _as_float(p["x"])
		var pz := _as_float(p["z"])
		node.position = Vector3(px, y_off, pz)
		if p.has("rotationY"):
			node.rotation.y = _as_float(p["rotationY"])
		landmarks_root.add_child(node)
		if url != "procedural://village_field":
			add_ground_skirt(Vector3(px, 0.0, pz), target, node.rotation.y)
		if p.get("light") != null:
			var ld: Dictionary = p["light"] as Dictionary
			var omni := OmniLight3D.new()
			var col := _as_int(ld.get("color", 0xffffff), 0xffffff)
			omni.light_color = Color(
				float((col >> 16) & 0xff) / 255.0,
				float((col >> 8) & 0xff) / 255.0,
				float(col & 0xff) / 255.0)
			omni.light_energy = _as_float(ld.get("intensity", 2.0), 2.0)
			omni.set_meta("base_energy", omni.light_energy)
			omni.omni_range = _as_float(ld.get("distance", 6.0), 6.0)
			omni.omni_attenuation = 1.2
			var off: Array = ld.get("offset", [0.0, 1.0, 0.0]) as Array
			# TS authored offsets in three.js handedness; Godot x runs the other way.
			var local := Vector3(-_as_float(off[0]), _as_float(off[1]), _as_float(off[2]))
			omni.position = node.position + local.rotated(Vector3.UP, node.rotation.y)
			landmarks_root.add_child(omni)
			if col == 0xff8c2b:
				forge_lights.append(omni)

	var town_dict: Dictionary = state.get("town", {}) as Dictionary
	if not _as_bool(town_dict.get("blacksmithComplete", false)):
		build_lot_markers()


## Trodden-dirt Decal under a building so it sits *in* the ground instead
## of on it. Projects onto layer 1 only (terrain, grass, the building's own
## base); actors live on layer 2 and never pick up the stain.
func add_ground_skirt(at: Vector3, width: float, yaw: float) -> void:
	var d := Decal.new()
	d.texture_albedo = dirt_texture()
	var w := width * 0.95 + 1.4
	d.size = Vector3(w, 0.9, w)
	d.position = at + Vector3(0.0, 0.1, 0.0)
	d.rotation.y = yaw
	d.cull_mask = 1
	d.albedo_mix = 0.85
	d.upper_fade = 0.6
	d.lower_fade = 0.3
	landmarks_root.add_child(d)


var _dirt_tex: ImageTexture
func dirt_texture() -> ImageTexture:
	if _dirt_tex != null:
		return _dirt_tex
	const N := 128
	var noise := FastNoiseLite.new()
	noise.seed = 7
	noise.frequency = 0.045
	noise.fractal_octaves = 4
	var img := Image.create(N, N, false, Image.FORMAT_RGBA8)
	for y in N:
		for x in N:
			var u := (float(x) + 0.5) / N * 2.0 - 1.0
			var v := (float(y) + 0.5) / N * 2.0 - 1.0
			# Rounded-square falloff, edge broken up by noise.
			var r := pow(pow(absf(u), 4.0) + pow(absf(v), 4.0), 0.25)
			var n := noise.get_noise_2d(x, y) * 0.5 + 0.5
			var a := clampf((1.0 - smoothstep(0.55, 1.0, r + (n - 0.5) * 0.35)) * 1.0, 0.0, 1.0)
			var tone := lerpf(0.75, 1.15, n)
			var c := Color(0.42 * tone, 0.33 * tone, 0.23 * tone, a * 0.92)
			img.set_pixel(x, y, c)
	_dirt_tex = ImageTexture.create_from_image(img)
	return _dirt_tex


func _add_part_box(
	parent: Node3D,
	size: Vector3,
	pos: Vector3,
	color: Color,
	rot: Vector3 = Vector3.ZERO,
	emission: Color = Color.BLACK,
	metallic: float = 0.0,
	roughness: float = 0.85
) -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	var box := BoxMesh.new()
	box.size = size
	mi.mesh = box
	mi.position = pos
	mi.rotation = rot
	var mat := StandardMaterial3D.new()
	mat.albedo_color = color
	mat.metallic = metallic
	mat.roughness = roughness
	if emission != Color.BLACK:
		mat.emission_enabled = true
		mat.emission = emission
		mat.emission_energy_multiplier = 1.6
	mi.material_override = mat
	parent.add_child(mi)
	return mi


func make_cold_forge_ruin(target: float) -> Node3D:
	var root := Node3D.new()
	var d := 2.6
	var stone := Color(0.37, 0.35, 0.38)
	var stone_dark := Color(0.26, 0.25, 0.28)
	var charred := Color(0.20, 0.14, 0.10)
	var iron := Color(0.24, 0.26, 0.29)
	# Weathered stone foundation plinth
	_add_part_box(root, Vector3(target, 0.14, d), Vector3(0.0, 0.07, 0.0), stone_dark)
	# Broken back & side stone walls
	_add_part_box(root, Vector3(target * 0.92, 0.65, 0.28), Vector3(0.0, 0.46, -d * 0.42), stone)
	_add_part_box(root, Vector3(0.28, 0.52, d * 0.72), Vector3(-target * 0.44, 0.40, -0.1), stone)
	_add_part_box(root, Vector3(0.28, 0.38, d * 0.52), Vector3(target * 0.44, 0.33, -0.28), stone)
	# Cold stone forge hearth + broken chimney stump (unlit ash inside)
	_add_part_box(root, Vector3(1.15, 0.85, 0.95), Vector3(-1.15, 0.56, -0.58), stone)
	_add_part_box(root, Vector3(0.70, 0.65, 0.70), Vector3(-1.15, 1.30, -0.62), stone_dark)
	_add_part_box(root, Vector3(0.60, 0.22, 0.35), Vector3(-1.15, 0.36, -0.18), Color(0.12, 0.12, 0.14))
	# Charred timber corner posts + fallen ridge beam
	_add_part_box(root, Vector3(0.18, 1.15, 0.18), Vector3(target * 0.38, 0.71, 0.88), charred)
	_add_part_box(root, Vector3(0.18, 0.92, 0.18), Vector3(-target * 0.38, 0.60, 0.88), charred)
	_add_part_box(root, Vector3(target * 0.55, 0.12, 0.14), Vector3(0.45, 0.48, 0.15), charred, Vector3(0.0, 0.22, -0.32))
	# Cold iron anvil on weathered oak block in front
	_add_part_box(root, Vector3(0.46, 0.34, 0.46), Vector3(0.65, 0.31, 0.55), Color(0.32, 0.22, 0.14))
	_add_part_box(root, Vector3(0.56, 0.24, 0.28), Vector3(0.65, 0.58, 0.55), iron, Vector3.ZERO, Color.BLACK, 0.65, 0.45)
	return root


func make_village_cottage(target: float) -> Node3D:
	var root := Node3D.new()
	var w := target * 0.84
	var d := target * 0.68
	var stone := Color(0.46, 0.43, 0.40)
	var plaster := Color(0.88, 0.81, 0.67)
	var timber := Color(0.28, 0.18, 0.11)
	var shingle := Color(0.56, 0.24, 0.16)
	var glow := Color(1.0, 0.74, 0.30)
	# Stone plinth >= 0.28 tall so meadow grass stays clear
	_add_part_box(root, Vector3(target, 0.28, d + 0.24), Vector3(0.0, 0.14, 0.0), stone)
	# Cream plaster cabin + corner timber posts
	_add_part_box(root, Vector3(w, 0.92, d), Vector3(0.0, 0.74, 0.0), plaster)
	for sx: float in [-w * 0.48, w * 0.48]:
		for sz: float in [-d * 0.48, d * 0.48]:
			_add_part_box(root, Vector3(0.14, 0.96, 0.14), Vector3(sx, 0.76, sz), timber)
	# Pitched gable roof + ridge beam + stone chimney
	var roof_w := target * 0.96
	var slope_d := d * 0.68
	_add_part_box(root, Vector3(roof_w, 0.11, slope_d), Vector3(0.0, 1.46, -d * 0.25), shingle, Vector3(0.58, 0.0, 0.0))
	_add_part_box(root, Vector3(roof_w, 0.11, slope_d), Vector3(0.0, 1.46, d * 0.25), shingle, Vector3(-0.58, 0.0, 0.0))
	_add_part_box(root, Vector3(roof_w + 0.08, 0.14, 0.16), Vector3(0.0, 1.74, 0.0), timber)
	_add_part_box(root, Vector3(0.32, 0.85, 0.32), Vector3(w * 0.28, 1.62, -d * 0.18), stone)
	# Front door + warm glowing window
	_add_part_box(root, Vector3(0.38, 0.68, 0.08), Vector3(-w * 0.20, 0.62, d * 0.51), timber)
	_add_part_box(root, Vector3(0.38, 0.34, 0.08), Vector3(w * 0.22, 0.76, d * 0.51), glow, Vector3.ZERO, Color(0.95, 0.65, 0.22))
	return root


func make_village_field(target: float) -> Node3D:
	var root := Node3D.new()
	var d := target * 0.82
	var timber := Color(0.36, 0.24, 0.14)
	var soil := Color(0.25, 0.17, 0.11)
	var ridge := Color(0.31, 0.22, 0.14)
	var crop_gold := Color(0.80, 0.72, 0.26)
	var crop_leaf := Color(0.36, 0.60, 0.24)
	# Timber raised bed frame + dark soil base
	_add_part_box(root, Vector3(target, 0.10, d), Vector3(0.0, 0.05, 0.0), soil)
	_add_part_box(root, Vector3(target, 0.16, 0.10), Vector3(0.0, 0.08, -d * 0.48), timber)
	_add_part_box(root, Vector3(target, 0.16, 0.10), Vector3(0.0, 0.08, d * 0.48), timber)
	_add_part_box(root, Vector3(0.10, 0.16, d), Vector3(-target * 0.48, 0.08, 0.0), timber)
	_add_part_box(root, Vector3(0.10, 0.16, d), Vector3(target * 0.48, 0.08, 0.0), timber)
	# Four tilled rows with alternating golden wheat and herb sprouts
	for r in 4:
		var rz := -d * 0.30 + float(r) * (d * 0.20)
		_add_part_box(root, Vector3(target * 0.80, 0.08, 0.16), Vector3(0.0, 0.12, rz), ridge)
		for c in 4:
			var cx := -target * 0.30 + float(c) * (target * 0.20)
			var col := crop_gold if (r + c) % 2 == 0 else crop_leaf
			_add_part_box(root, Vector3(0.14, 0.24, 0.14), Vector3(cx, 0.26, rz), col)
	return root


func build_lot_markers() -> void:
	var lots: Array[Dictionary] = [
		{"pos": Vector3(17.5, 0.0, 13.0), "size": Vector2(3.6, 2.6), "label": "Pondside Lot ★"},
		{"pos": Vector3(11.0, 0.0, 5.0), "size": Vector2(2.6, 2.6), "label": "Market Lot"},
		{"pos": Vector3(4.5, 0.0, 9.5), "size": Vector2(3.2, 2.6), "label": "West Gate Lot"},
	]
	var stake_col := Color(0.72, 0.54, 0.24)
	var rim_col := Color(0.92, 0.72, 0.28)
	for entry: Dictionary in lots:
		var marker := Node3D.new()
		marker.position = entry["pos"] as Vector3
		var sz: Vector2 = entry["size"] as Vector2
		var hx := sz.x * 0.5
		var hz := sz.y * 0.5
		for sx: float in [-hx, hx]:
			for sz_p: float in [-hz, hz]:
				_add_part_box(marker, Vector3(0.12, 0.52, 0.12), Vector3(sx, 0.26, sz_p), stake_col)
		_add_part_box(marker, Vector3(sz.x, 0.06, 0.07), Vector3(0.0, 0.05, -hz), rim_col)
		_add_part_box(marker, Vector3(sz.x, 0.06, 0.07), Vector3(0.0, 0.05, hz), rim_col)
		_add_part_box(marker, Vector3(0.07, 0.06, sz.y), Vector3(-hx, 0.05, 0.0), rim_col)
		_add_part_box(marker, Vector3(0.07, 0.06, sz.y), Vector3(hx, 0.05, 0.0), rim_col)
		var lbl := Label3D.new()
		lbl.text = str(entry["label"])
		lbl.font_size = 44
		lbl.pixel_size = 0.007
		lbl.billboard = BaseMaterial3D.BILLBOARD_ENABLED
		lbl.modulate = Color(1.0, 0.88, 0.52)
		lbl.outline_size = 8
		lbl.outline_modulate = Color(0.08, 0.07, 0.05)
		lbl.position = Vector3(0.0, 1.85, 0.0)
		marker.add_child(lbl)
		landmarks_root.add_child(marker)


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
	label.font_size = 48
	label.pixel_size = 0.0065
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

## Actors persist across refreshes (keyed by entity id) so lunges, flashes
## and deaths can play on them. HP drops between snapshots become damage
## numbers, released by the matching fx cue. A zone change starts clean.
func refresh_entities() -> void:
	var zone: Dictionary = state["zone"]
	var pl: Dictionary = state["player"]
	var zid := str(zone["id"])
	if zid != entities_zone:
		entities_zone = zid
		clear_children(entities_root)
		entity_nodes.clear()
		entity_targets.clear()
		entity_hp.clear()
		dying.clear()
		cue_queue.clear()
	pending_numbers.clear()
	var seen := {}

	player_node = _sync_actor(seen, "player", Vector3(_as_float(pl["x"]), 0.0, _as_float(pl["y"])), _as_int(pl.get("hp", -1), -1),
		func() -> Node3D: return make_actor(RIM_PLAYER, 1.0, str(PLAYER_FIGURES.get(str(pl.get("starterClass", "")), "")), Color(0.30, 0.55, 1.00)))

	var comp_var: Variant = pl.get("companion")
	if comp_var is Dictionary:
		var comp: Dictionary = comp_var as Dictionary
		if _as_int(comp.get("hp", 0)) > 0 and _as_int(comp.get("respawnCooldown", 0)) == 0:
			var ckind := str(comp.get("kind", ""))
			var ccol := Color(0.45, 0.72, 1.00) if ckind in ["starlight_owl", "StarlightOwl"] else Color(0.22, 0.86, 0.78)
			var cname := str(comp.get("name", "Companion"))
			_sync_actor(seen, "companion", Vector3(_as_float(comp["x"]), 0.0, _as_float(comp["y"])), _as_int(comp.get("hp", -1), -1),
				func() -> Node3D:
					var cstem := str(COMPANION_FIGURES.get(ckind, ""))
					var cn := make_actor(RIM_COMPANION, 1.0, cstem, ccol)
					if figure_scene(cstem) == null:
						add_label(cn, cname) # a capsule needs its name; a figure doesn't
					return cn)

	for e_var: Variant in (zone.get("enemies", []) as Array):
		var e: Dictionary = e_var as Dictionary
		var boss := _as_bool(e.get("isBoss", false))
		var arch := str(e.get("archetype", ""))
		_sync_actor(seen, "enemy:" + str(e["id"]), Vector3(_as_float(e["x"]), 0.0, _as_float(e["y"])), _as_int(e.get("hp", -1), -1),
			func() -> Node3D: return make_actor(RIM_BOSS if boss else RIM_ENEMY, 1.35 if boss else 1.0,
				str(ENEMY_FIGURES.get(arch, "")), Color(0.90, 0.25, 0.20)))

	for npc_var: Variant in (zone.get("npcs", []) as Array):
		var npc: Dictionary = npc_var as Dictionary
		var nname := str(npc["name"])
		_sync_actor(seen, "npc:" + str(npc["id"]), Vector3(_as_float(npc["x"]), 0.0, _as_float(npc["y"])), -1,
			func() -> Node3D:
				var n := make_actor(RIM_NPC, 1.0, "", Color(0.30, 0.85, 0.40))
				add_label(n, nname)
				return n)

	for l_var: Variant in (zone.get("loot", []) as Array):
		var l: Dictionary = l_var as Dictionary
		_sync_actor(seen, "loot:%d,%d" % [_as_int(l["x"]), _as_int(l["y"])], Vector3(_as_float(l["x"]), 0.0, _as_float(l["y"])), -1,
			func() -> Node3D: return make_loot_box())

	# Gone from the snapshot: enemies (and a collapsing companion) wait for
	# their kill cue to pop; everything else just goes.
	for key_var: Variant in entity_nodes.keys():
		var key: String = str(key_var)
		if seen.has(key):
			continue
		var n: Node3D = entity_nodes[key] as Node3D
		var tgt: Vector3 = entity_targets.get(n, n.position) as Vector3
		entity_nodes.erase(key)
		entity_targets.erase(n)
		var last_hp := _as_int(entity_hp.get(key, 0))
		entity_hp.erase(key)
		if (key.begins_with("enemy:") or key == "companion") and is_instance_valid(n):
			var tk := _tile_key(tgt)
			if last_hp > 0:
				pending_numbers[tk] = _as_int(pending_numbers.get(tk, 0)) + last_hp
			dying[tk] = n
		elif is_instance_valid(n):
			n.queue_free()


func _sync_actor(seen: Dictionary, key: String, target: Vector3, hp: int, make: Callable) -> Node3D:
	seen[key] = true
	var n: Node3D = entity_nodes.get(key) as Node3D
	if n == null or not is_instance_valid(n):
		n = make.call() as Node3D
		n.set_meta("ekey", key)
		_set_layer(n, 2) # ground decals skip actors
		entities_root.add_child(n)
		n.position = target
		entity_nodes[key] = n
	var old_target: Vector3 = entity_targets.get(n, target) as Vector3
	if old_target != target:
		face_actor(n, target - old_target)
	entity_targets[n] = target
	if hp >= 0:
		if entity_hp.has(key):
			var dmg := _as_int(entity_hp[key]) - hp
			if dmg > 0:
				var tk := _tile_key(target)
				pending_numbers[tk] = _as_int(pending_numbers.get(tk, 0)) + dmg
		entity_hp[key] = hp
	return n


func _tile_key(p: Vector3) -> String:
	return "%d,%d" % [roundi(p.x), roundi(p.z)]


## Live actor standing on (or walking to) a tile; loot never counts.
func actor_at(tx: int, ty: int) -> Node3D:
	var tk := "%d,%d" % [tx, ty]
	for key_var: Variant in entity_nodes:
		var key: String = str(key_var)
		if key.begins_with("loot:"):
			continue
		var n: Node3D = entity_nodes[key] as Node3D
		if _tile_key(entity_targets.get(n, n.position) as Vector3) == tk:
			return n
	return dying.get(tk) as Node3D


func _set_layer(n: Node, layer: int) -> void:
	if n is VisualInstance3D:
		(n as VisualInstance3D).layers = 1 << (layer - 1)
	for c in n.get_children():
		_set_layer(c, layer)


# Tabletop miniatures: each actor is a figure on a walnut plinth with a
# faction-coloured rim. Figures live in assets/models/figures/<stem>.glb;
# an actor without one yet keeps the old capsule on the same plinth.
const FIGURE_DIR := "res://assets/models/figures/"
const PLAYER_FIGURES := {"Vanguard": "fig_vanguard"}
const COMPANION_FIGURES := {"silverwind_lynx": "fig_lynx", "SilverwindLynx": "fig_lynx"}
const ENEMY_FIGURES := {"plague_rat": "fig_blight_rat", "crypt_skeleton": "fig_crypt_sentinel", "goblin_skirmisher": "fig_goblin", "bone_acolyte": "fig_bone_acolyte", "ash_cultist": "fig_ash_cultist", "grave_husk": "fig_grave_husk", "shadow_stalker": "fig_shadow_stalker", "blight_weaver": "fig_blight_weaver", "dread_wraith": "fig_dread_wraith", "rime_revenant": "fig_rime_revenant", "iron_golem": "fig_iron_golem", "cinder_wisp": "fig_cinder_wisp", "obsidian_warden": "fig_obsidian_warden", "spore_hulk": "fig_spore_hulk"}
## Figure height in tiles (before the actor's scale factor).
const FIGURE_HEIGHT := {"fig_vanguard": 1.3, "fig_lynx": 0.85, "fig_blight_rat": 0.78, "fig_crypt_sentinel": 1.4, "fig_goblin": 0.95, "fig_bone_acolyte": 1.35, "fig_ash_cultist": 1.35, "fig_grave_husk": 1.38, "fig_shadow_stalker": 1.35, "fig_blight_weaver": 1.38, "fig_dread_wraith": 1.35, "fig_rime_revenant": 1.40, "fig_iron_golem": 1.48, "fig_cinder_wisp": 1.32, "fig_obsidian_warden": 1.52, "fig_spore_hulk": 1.54}
const PLINTH_H := 0.09
const RIM_PLAYER := Color(0.30, 0.55, 1.00)
const RIM_COMPANION := Color(0.22, 0.86, 0.78)
const RIM_ENEMY := Color(0.95, 0.22, 0.16)
const RIM_BOSS := Color(1.0, 0.72, 0.2)
const RIM_NPC := Color(0.40, 0.85, 0.45)
var _figure_cache := {} # stem -> PackedScene (or null when missing)


func figure_scene(stem: String) -> PackedScene:
	if stem == "":
		return null
	if not _figure_cache.has(stem):
		var path := FIGURE_DIR + stem + ".glb"
		_figure_cache[stem] = load(path) if ResourceLoader.exists(path) else null
	return _figure_cache[stem] as PackedScene


## Root (lerped to the tile) -> "Body" pivot (juice transforms) ->
## plinth + "Model" pivot (facing yaw) -> figure or capsule.
func make_actor(rim: Color, scale_factor: float, stem := "", capsule_color := Color.WHITE) -> Node3D:
	var root := Node3D.new()
	var body := Node3D.new()
	body.name = "Body"
	root.add_child(body)

	var plinth := MeshInstance3D.new()
	var cyl := CylinderMesh.new()
	cyl.top_radius = 0.34
	cyl.bottom_radius = 0.38
	cyl.height = PLINTH_H
	cyl.radial_segments = 24
	plinth.mesh = cyl
	plinth.position.y = PLINTH_H * 0.5
	var pm := StandardMaterial3D.new()
	pm.albedo_color = Color(0.20, 0.13, 0.08)
	pm.roughness = 0.55
	plinth.material_override = pm
	body.add_child(plinth)
	var ring := MeshInstance3D.new()
	var rc := CylinderMesh.new()
	rc.top_radius = 0.395
	rc.bottom_radius = 0.40
	rc.height = 0.035
	rc.radial_segments = 24
	ring.mesh = rc
	ring.position.y = 0.0175
	var rm := StandardMaterial3D.new()
	rm.albedo_color = rim
	rm.emission_enabled = true
	rm.emission = rim
	rm.emission_energy_multiplier = 0.5
	ring.material_override = rm
	body.add_child(ring)

	var model := Node3D.new()
	model.name = "Model"
	model.position.y = PLINTH_H
	body.add_child(model)
	var packed := figure_scene(stem)
	if packed != null:
		var fig: Node3D = packed.instantiate() as Node3D
		var aabb := scene_aabb(fig)
		if aabb.size.y > 0.001:
			var s := _as_float(FIGURE_HEIGHT.get(stem, 1.0), 1.0) / aabb.size.y
			fig.scale = Vector3.ONE * s
			var c := aabb.get_center()
			fig.position = Vector3(-c.x * s, -aabb.position.y * s, -c.z * s)
		model.add_child(fig)
	else:
		var mi := MeshInstance3D.new()
		var cap := CapsuleMesh.new()
		cap.radius = 0.26
		cap.height = 0.95
		mi.mesh = cap
		mi.position.y = 0.475
		var mat := StandardMaterial3D.new()
		mat.albedo_color = capsule_color
		mat.roughness = 0.7
		mi.material_override = mat
		model.add_child(mi)

	# Hit-flash: an additive overlay on the figure (not the plinth); juice
	# tweens its alpha.
	var flash := StandardMaterial3D.new()
	flash.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	flash.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	flash.blend_mode = BaseMaterial3D.BLEND_MODE_ADD
	flash.albedo_color = Color(1, 1, 1, 0)
	_set_overlay(model, flash)
	root.set_meta("flash", flash)
	root.scale = Vector3.ONE * scale_factor
	return root


func _set_overlay(n: Node, mat: Material) -> void:
	if n is GeometryInstance3D:
		(n as GeometryInstance3D).material_overlay = mat
	for c in n.get_children():
		_set_overlay(c, mat)


## Turn a figure to face a world direction (x, z); figures face +Z at rest.
func face_actor(n: Node3D, dir: Vector3) -> void:
	if n == null or dir.length() < 0.01:
		return
	var model := n.get_node_or_null("Body/Model") as Node3D
	if model != null:
		model.rotation.y = atan2(dir.x, dir.z)


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
	label.font_size = 48
	label.pixel_size = 0.0065
	label.billboard = BaseMaterial3D.BILLBOARD_ENABLED
	label.modulate = Color(0.95, 0.97, 1.0)
	label.outline_size = 8
	label.outline_modulate = Color(0.05, 0.05, 0.08)
	label.position = Vector3(0.0, 1.6, 0.0)
	parent.add_child(label)


# ---------------------------------------------------------------------------
# Camera, input, per-frame updates
# ---------------------------------------------------------------------------

func _process(delta: float) -> void:
	# Smooth entity movement toward their tile positions.
	var w := minf(1.0, delta * 10.0)
	for node_var: Variant in entity_targets.keys():
		var node: Node3D = node_var as Node3D
		if is_instance_valid(node):
			var tgt: Vector3 = entity_targets[node] as Vector3
			node.position = node.position.lerp(tgt, w)
	_drain_cues(delta)
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
	var offset := cam_offset()
	camera.position = offset.rotated(Vector3.UP, cam_yaw)
	var shake: Vector2 = juice.shake_offset(delta)
	camera.h_offset = shake.x
	camera.v_offset = shake.y
	var look := camera_rig.position + Vector3(0.0, 0.8, 0.0)
	if camera.global_position.distance_squared_to(look) > 0.01:
		camera.look_at(look, Vector3.UP)
	if cutaway_mat != null:
		cutaway_mat.set_shader_parameter("player_pos", player_node.position)
		cutaway_mat.set_shader_parameter("cam_pos", camera.global_position)
		# Chase view sits low behind the player, so it needs a wider cut.
		cutaway_mat.set_shader_parameter("cut_radius", 2.4 if chase_mode else 1.8)


## Camera offset from the player. Overhead is a ~45-degree diorama tilt
## (close enough that actors read as figures, far enough to see a room).
func cam_offset() -> Vector3:
	if chase_mode:
		return Vector3(0.0, 7.0, 9.0)
	return Vector3(0.0, 13.5, 12.0) * cam_zoom


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseButton and (event as InputEventMouseButton).pressed and title_screen == null:
		var mb := event as InputEventMouseButton
		if mb.button_index == MOUSE_BUTTON_WHEEL_UP:
			cam_zoom = clampf(cam_zoom - 0.08, CAM_ZOOM_MIN, CAM_ZOOM_MAX)
		elif mb.button_index == MOUSE_BUTTON_WHEEL_DOWN:
			cam_zoom = clampf(cam_zoom + 0.08, CAM_ZOOM_MIN, CAM_ZOOM_MAX)
		return
	if event is InputEventKey:
		var k := event as InputEventKey
		if not k.pressed or k.echo:
			return
		if title_screen != null:
			return # Continue / New Game first
		flush_cues() # never make the player wait on an animation
		match k.physical_keycode:
			KEY_V:
				chase_mode = not chase_mode
			KEY_MINUS:
				cam_zoom = clampf(cam_zoom + 0.12, CAM_ZOOM_MIN, CAM_ZOOM_MAX)
			KEY_EQUAL:
				cam_zoom = clampf(cam_zoom - 0.12, CAM_ZOOM_MIN, CAM_ZOOM_MAX)
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
	if move_timer > 0.0 or title_screen != null or merchant_panel.visible or dialogue_panel.visible or skills_panel.visible or is_aiming():
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
		flush_cues()
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
	hud_title.add_theme_font_size_override("font_size", 18)
	hud_title.add_theme_color_override("font_color", UI_GOLD)
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
	log_label.add_theme_font_size_override("normal_font_size", 14)
	log_label.add_theme_color_override("default_color", Color(UI_INK, 0.92))
	log_panel.add_child(log_label)

	# Town panel, right side (visible in town zones only).
	town_panel = make_panel()
	town_panel.anchor_left = 1.0
	town_panel.anchor_right = 1.0
	town_panel.offset_left = -330
	town_panel.offset_right = -10
	town_panel.offset_top = 10
	town_panel.offset_bottom = 480
	layer.add_child(town_panel)
	var town_scroll := ScrollContainer.new()
	town_scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	town_scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	town_panel.add_child(town_scroll)
	town_box = VBoxContainer.new()
	town_box.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	town_box.add_theme_constant_override("separation", 4)
	town_scroll.add_child(town_box)

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
	merchant_title.add_theme_font_size_override("font_size", 18)
	merchant_title.add_theme_color_override("font_color", UI_GOLD)
	mer_box.add_child(merchant_title)
	merchant_rows = VBoxContainer.new()
	mer_box.add_child(merchant_rows)
	var mer_close := make_button("Close")
	mer_close.pressed.connect(func() -> void: merchant_panel.hide())
	mer_box.add_child(mer_close)
	merchant_panel.hide()


# Storybook UI skin: walnut-timber panels with a brass edge, cream
# parchment text, serif type. One Theme shared by every in-game panel.
const UI_INK := Color(0.96, 0.91, 0.80)
const UI_GOLD := Color(0.95, 0.78, 0.42)
const UI_BRASS := Color(0.72, 0.55, 0.28)
var _ui_theme: Theme

func ui_theme() -> Theme:
	if _ui_theme != null:
		return _ui_theme
	var t := Theme.new()
	var body := SystemFont.new()
	body.font_names = PackedStringArray(["Palatino", "Book Antiqua", "Georgia", "serif"])
	t.default_font = body
	t.default_font_size = 15
	t.set_color("font_color", "Label", UI_INK)
	t.set_color("default_color", "RichTextLabel", UI_INK)
	for st: String in ["normal", "hover", "pressed", "disabled", "focus"]:
		var sb := StyleBoxFlat.new()
		match st:
			"hover":
				sb.bg_color = Color(0.42, 0.28, 0.14, 0.95)
			"pressed":
				sb.bg_color = Color(0.30, 0.19, 0.09, 0.95)
			"disabled":
				sb.bg_color = Color(0.18, 0.14, 0.11, 0.7)
			"focus":
				sb.bg_color = Color(0, 0, 0, 0)
			_:
				sb.bg_color = Color(0.30, 0.20, 0.11, 0.92)
		sb.border_color = UI_BRASS if st != "disabled" else Color(UI_BRASS, 0.35)
		sb.set_border_width_all(1)
		sb.set_corner_radius_all(5)
		sb.content_margin_left = 10
		sb.content_margin_right = 10
		sb.content_margin_top = 5
		sb.content_margin_bottom = 5
		t.set_stylebox(StringName(st), &"Button", sb)
	t.set_color("font_color", "Button", UI_INK)
	t.set_color("font_hover_color", "Button", Color(1.0, 0.95, 0.82))
	t.set_color("font_disabled_color", "Button", Color(UI_INK, 0.4))
	_ui_theme = t
	return t


func make_panel() -> PanelContainer:
	var p := PanelContainer.new()
	p.theme = ui_theme()
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.13, 0.085, 0.05, 0.9)
	sb.border_color = UI_BRASS
	sb.set_border_width_all(2)
	sb.set_corner_radius_all(9)
	sb.shadow_color = Color(0, 0, 0, 0.35)
	sb.shadow_size = 6
	sb.shadow_offset = Vector2(0, 3)
	sb.set_content_margin_all(12.0)
	p.add_theme_stylebox_override("panel", sb)
	return p


func make_label() -> Label:
	var l := Label.new()
	l.add_theme_font_size_override("font_size", 15)
	l.add_theme_color_override("font_color", UI_INK)
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
	var derived: Dictionary = state.get("derived", {}) as Dictionary
	hud_title.text = "%s — %s" % [str(zone["name"]), str(zone.get("subtitle", ""))]
	var max_hp := _as_int(derived.get("maxHp", pl.get("maxHp", 1)), 1)
	var max_mp := _as_int(derived.get("maxMana", pl.get("maxMana", 0)))
	var cur_hp := _as_int(pl["hp"])
	var cur_mp := _as_int(pl["mana"])
	hud_body.text = "HP %s %d/%d\nMP %s %d/%d\nLv %d · %d g · ATK %d · DEF %d" % [
		text_bar(cur_hp, max_hp), cur_hp, max_hp,
		text_bar(cur_mp, max_mp), cur_mp, max_mp,
		_as_int(pl["level"]), _as_int(pl["gold"]),
		_as_int(derived.get("atk", 0)), _as_int(derived.get("def", 0)),
	]
	var sts: PackedStringArray = []
	for st_var: Variant in (pl.get("statuses", []) as Array):
		var st: Dictionary = st_var as Dictionary
		sts.append("%s %dt" % [str(st["kind"]).capitalize(), _as_int(st["turns"])])
	if not sts.is_empty():
		hud_body.text += "\n" + " · ".join(sts)
	var comp_var: Variant = pl.get("companion")
	if comp_var is Dictionary:
		var comp: Dictionary = comp_var as Dictionary
		var cname := str(comp.get("name", "Companion"))
		var cd := _as_int(comp.get("respawnCooldown", 0))
		if cd > 0:
			hud_body.text += "\n%s: reforming (%dt)" % [cname, cd]
		else:
			hud_body.text += "\n%s %d/%d HP" % [cname, _as_int(comp.get("hp", 0)), _as_int(comp.get("maxHp", 0))]
	var sp := _as_int(pl.get("skillPoints", 0))
	if sp > 0:
		hud_body.text += "\n%d skill point(s) — press K" % sp


func text_bar(cur: int, maxv: int, width: int = 14) -> String:
	var filled := 0
	if maxv > 0:
		filled = clampi(roundi(float(cur) / float(maxv) * float(width)), 0, width)
	return "█".repeat(filled) + "░".repeat(width - filled)


func update_log() -> void:
	var lines: PackedStringArray = []
	for l: Variant in (state.get("log", []) as Array):
		lines.append(str(l))
	log_label.text = "\n".join(lines)


func rebuild_town_panel() -> void:
	clear_children(town_box)
	var zone: Dictionary = state["zone"]
	if not _as_bool(zone.get("isTown", false)):
		town_panel.hide()
		return
	town_panel.show()

	var town: Dictionary = state.get("town", {}) as Dictionary
	var pieces: Array = town.get("piecesFound", []) as Array
	var supplies: Array = town.get("suppliesFound", []) as Array
	var tier := _as_int(state.get("populationTier", 0))
	var pop := _as_int(town.get("population", 0))
	var returns := _as_int(town.get("returns", 0))
	var stall_cleared := _as_bool(town.get("stallItemFound", false))

	var head := make_label()
	head.add_theme_font_size_override("font_size", 15)
	head.add_theme_color_override("font_color", Color(0.98, 0.85, 0.45))
	head.text = "Oakhaven — Town & Forge"
	town_box.add_child(head)

	var summary := make_label()
	summary.add_theme_font_size_override("font_size", 12)
	summary.text = "Pieces %d/3 · Pop %d (Tier %d) · Returns %d" % [pieces.size(), pop, tier, returns]
	town_box.add_child(summary)

	var bonuses: Dictionary = state.get("townTierBonuses", {}) as Dictionary
	var bonus_l := make_label()
	bonus_l.add_theme_font_size_override("font_size", 12)
	if tier > 0:
		var def_b := _as_int(bonuses.get("def", 0))
		var def_txt := " · +%d DEF" % def_b if def_b > 0 else ""
		bonus_l.text = "Tier %d Bonus: +%d HP · +%d ATK · +%d Spell%s" % [
			tier, _as_int(bonuses.get("maxHp", 0)), _as_int(bonuses.get("atk", 0)), _as_int(bonuses.get("spellPower", 0)), def_txt]
		bonus_l.add_theme_color_override("font_color", Color(0.55, 0.92, 0.68))
	else:
		bonus_l.text = "Next tier at Pop 20 (+HP/ATK/Spell & village growth)"
		bonus_l.add_theme_color_override("font_color", Color(0.72, 0.76, 0.82))
	bonus_l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	town_box.add_child(bonus_l)

	# Forge pieces + Heartcoal stall checklist
	var piece_defs := [
		["blacksmith_anvil", "Torin's Lost Anvil"],
		["blacksmith_bellows", "Great Forge Bellows"],
		["blacksmith_sign", "The Smith's Hanging Sign"],
	]
	for pd: Array in piece_defs:
		var has_p := pieces.has(pd[0])
		var pl := make_label()
		pl.add_theme_font_size_override("font_size", 12)
		pl.text = "%s %s" % ["✓" if has_p else "○", str(pd[1])]
		pl.add_theme_color_override("font_color", Color(0.82, 0.95, 0.84) if has_p else Color(0.66, 0.70, 0.76))
		town_box.add_child(pl)

	var stall_l := make_label()
	stall_l.add_theme_font_size_override("font_size", 12)
	stall_l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	if stall_cleared:
		stall_l.text = "✓ Heartcoal — forge lit (growth active)"
		stall_l.add_theme_color_override("font_color", Color(0.55, 0.92, 0.68))
	else:
		stall_l.text = "✕ Heartcoal — STALLED (growth frozen)"
		stall_l.add_theme_color_override("font_color", Color(0.95, 0.56, 0.54))
	town_box.add_child(stall_l)

	var carried: Array = state.get("carriedTownTags", [])
	if not carried.is_empty():
		var c := make_label()
		c.add_theme_font_size_override("font_size", 12)
		c.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		c.text = "Carrying (bank on return): " + join_strs(carried)
		c.add_theme_color_override("font_color", Color(0.98, 0.85, 0.45))
		town_box.add_child(c)

	var wish_l := make_label()
	wish_l.add_theme_font_size_override("font_size", 11)
	wish_l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	wish_l.text = "Torin's wish: Pondside Lot (+75g bonus, relocate free anytime)"
	wish_l.add_theme_color_override("font_color", Color(0.65, 0.82, 0.96))
	town_box.add_child(wish_l)

	var lot_labels := {
		"pondside": "Pondside Lot",
		"market": "Market Green Lot",
		"west_gate": "West Gate Lot",
	}
	if _as_bool(state.get("canCompleteBlacksmith", false)):
		var hint := make_label()
		hint.add_theme_font_size_override("font_size", 12)
		hint.text = "Ready! Choose a lot to rebuild:"
		hint.add_theme_color_override("font_color", Color(0.98, 0.85, 0.45))
		town_box.add_child(hint)
		for lot: String in LOTS:
			var label := str(lot_labels.get(lot, lot))
			if lot == "pondside":
				label += " ★ (+75g wish)"
			var b := make_button("Build: " + label)
			b.add_theme_font_size_override("font_size", 12)
			b.pressed.connect(_on_complete_blacksmith.bind(lot))
			town_box.add_child(b)
	elif _as_bool(town.get("blacksmithComplete", false)):
		var cur := str(town.get("blacksmithLot", ""))
		var at := make_label()
		at.add_theme_font_size_override("font_size", 12)
		at.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		var wish_note := " (wish met)" if _as_bool(town.get("wishSatisfied", false)) else ""
		at.text = "Forge standing: %s%s" % [str(lot_labels.get(cur, cur)), wish_note]
		at.add_theme_color_override("font_color", Color(0.55, 0.92, 0.68))
		town_box.add_child(at)
		var shop_btn := make_button("Open Blacksmith Shop")
		shop_btn.add_theme_font_size_override("font_size", 13)
		shop_btn.pressed.connect(func() -> void: open_merchant("blacksmith"))
		town_box.add_child(shop_btn)
		var bonus_claimed := _as_bool(town.get("wishBonusClaimed", false))
		for lot: String in LOTS:
			if lot == cur:
				continue
			var label := str(lot_labels.get(lot, lot))
			if lot == "pondside" and not bonus_claimed:
				label += " (+75g wish)"
			var b := make_button("Relocate: " + label)
			b.add_theme_font_size_override("font_size", 12)
			b.pressed.connect(_on_relocate_blacksmith.bind(lot))
			town_box.add_child(b)

	var sup_head := make_label()
	sup_head.add_theme_font_size_override("font_size", 12)
	sup_head.text = "Regional Supplies (side branches):"
	sup_head.add_theme_color_override("font_color", Color(0.95, 0.82, 0.50))
	town_box.add_child(sup_head)
	var supply_defs := [
		["barrow_iron", "Barrow Iron (Barrow Fl.1 → Hauberk)"],
		["mire_timber", "Mire Timber (Fungal Fl.1 → Greatbow)"],
		["frost_coal", "Frost White-Coal (Frost Fl.1 → Greathelm)"],
	]
	for sd: Array in supply_defs:
		var has_s := supplies.has(sd[0])
		var sl := make_label()
		sl.add_theme_font_size_override("font_size", 11)
		sl.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		sl.text = "%s %s" % ["✓" if has_s else "○", str(sd[1])]
		sl.add_theme_color_override("font_color", Color(0.82, 0.95, 0.84) if has_s else Color(0.68, 0.72, 0.78))
		town_box.add_child(sl)


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
	for c_var: Variant in (node.get("choices", []) as Array):
		var choice: Dictionary = c_var as Dictionary
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
	for o_var: Variant in (p.get("options", []) as Array):
		var opt: Dictionary = o_var as Dictionary
		var b := make_button(str(opt.get("label", "...")))
		b.disabled = _as_bool(opt.get("disabled", false))
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
	var stock: Array = parsed as Array
	for i: int in stock.size():
		var item: Dictionary = stock[i] as Dictionary
		var row := HBoxContainer.new()
		var l := make_label()
		l.text = "%s (%s) — %d g" % [str(item.get("name", "?")), str(item.get("rarity", "")), _as_int(item.get("value", 0))]
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

static func _as_int(v: Variant, fallback: int = 0) -> int:
	if v is int:
		return v
	if v is float:
		var vf: float = v
		return int(vf)
	return fallback


static func _as_float(v: Variant, fallback: float = 0.0) -> float:
	if v is float:
		return v
	if v is int:
		var vi: int = v
		return float(vi)
	return fallback


static func _as_bool(v: Variant, fallback: bool = false) -> bool:
	return v if v is bool else fallback


func join_strs(arr: Array) -> String:
	var parts: PackedStringArray = []
	for v: Variant in arr:
		parts.append(str(v))
	return ", ".join(parts)


func clear_children(node: Node) -> void:
	for child: Node in node.get_children():
		node.remove_child(child)
		child.free()


func _flicker_forge(delta: float) -> void:
	if forge_lights.is_empty():
		return
	_flicker_t += delta
	for i: int in forge_lights.size():
		var l: OmniLight3D = forge_lights[i] as OmniLight3D
		if is_instance_valid(l):
			var base := _as_float(l.get_meta("base_energy", 3.0), 3.0)
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
	# Blows in one turn play one after another (player swing, companion,
	# each enemy's reply) so a multi-action turn reads; a key press flushes
	# the rest at once (flush_cues).
	var t := 0.0
	for c_var: Variant in (parsed as Array):
		var cue: Dictionary = c_var as Dictionary
		cue_queue.append({"t": t, "cue": cue})
		if ATTACK_CUES.has(str(cue.get("kind", ""))):
			t += CUE_STAGGER
	_drain_cues(0.0)


const ATTACK_CUES := ["hit", "crit", "player_hit", "companion_hit", "kill"]


func _drain_cues(delta: float) -> void:
	if cue_queue.is_empty():
		return
	for q_var: Variant in cue_queue:
		var q: Dictionary = q_var as Dictionary
		q["t"] = _as_float(q["t"]) - delta
	while not cue_queue.is_empty():
		var head: Dictionary = cue_queue[0] as Dictionary
		if _as_float(head["t"]) > 0.0:
			break
		var popped: Dictionary = cue_queue.pop_front() as Dictionary
		_play_cue(popped["cue"] as Dictionary)
	if cue_queue.is_empty():
		_settle_leftovers()


## Play everything still queued right now (input buffering).
func flush_cues() -> void:
	while not cue_queue.is_empty():
		var popped: Dictionary = cue_queue.pop_front() as Dictionary
		_play_cue(popped["cue"] as Dictionary)
	_settle_leftovers()
	juice.snap()


## Deaths and damage the cues didn't claim (DoT ticks, thorns...).
func _settle_leftovers() -> void:
	for tk_var: Variant in dying.keys():
		juice.die(dying[tk_var] as Node3D)
	dying.clear()
	for tk_var: Variant in pending_numbers.keys():
		var tk: String = str(tk_var)
		var parts: PackedStringArray = tk.split(",")
		var at := Vector3(float(parts[0]), 0.0, float(parts[1]))
		juice.number(fx_root, at, str(pending_numbers[tk_var]), Color(0.85, 0.75, 1.0))
	pending_numbers.clear()


## Lunge / flash / number / hit-stop / shake for one cue, then its VFX + SFX.
func _cue_juice(kind: String, x: int, y: int, dx: int, dy: int) -> void:
	var tk := "%d,%d" % [x, y]
	var target := actor_at(x, y)
	var dir := Vector3(float(dx), 0.0, float(dy))
	match kind:
		"hit", "crit", "companion_hit", "player_hit":
			if maxi(absi(dx), absi(dy)) == 1:
				var attacker := actor_at(x - dx, y - dy)
				face_actor(attacker, dir)
				juice.lunge(attacker, dir)
			elif dir.length() > 0.0 and player_node != null:
				face_actor(player_node, dir)
				juice.lunge(player_node, dir) # ranged: a cast pose
			if target != null:
				juice.flash(target, Color(1.0, 0.45, 0.4) if kind == "player_hit" else Color.WHITE)
		"ember_lance", "venom_tip", "executioners_mark", "shield_bash", "frost_nova":
			if player_node != null and dir.length() > 0.0:
				juice.lunge(player_node, dir)
			if target != null:
				juice.flash(target)
		"kill":
			if dying.has(tk):
				juice.die(dying[tk] as Node3D)
				dying.erase(tk)
		"deathless", "spore_burst":
			if target != null:
				juice.flash(target, Color(0.7, 1.0, 0.5))
	if pending_numbers.has(tk) and kind != "windup":
		var dmg := _as_int(pending_numbers[tk])
		pending_numbers.erase(tk)
		var col := Color(1.0, 0.36, 0.3) if kind == "player_hit" else (Color(1.0, 0.85, 0.25) if kind == "crit" else Color(1.0, 0.98, 0.92))
		juice.number(fx_root, Vector3(float(x), 0.0, float(y)), str(dmg) + ("!" if kind == "crit" else ""), col, kind == "crit")
	match kind:
		"crit":
			juice.hit_stop(0.06)
			juice.add_trauma(0.3)
		"kill":
			juice.hit_stop(0.07)
			juice.add_trauma(0.28)
		"player_hit":
			juice.hit_stop(0.05)
			juice.add_trauma(0.42)
		"spore_burst", "frost_nova":
			juice.add_trauma(0.5)
		"boss_roar":
			juice.add_trauma(0.6)


func _play_cue(cue: Dictionary) -> void:
	var kind := str(cue.get("kind", ""))
	var cx := _as_int(cue.get("x", 0))
	var cy := _as_int(cue.get("y", 0))
	var cdx := _as_int(cue.get("dx", 0))
	var cdy := _as_int(cue.get("dy", 0))
	_cue_juice(kind, cx, cy, cdx, cdy)
	var at := Vector3(float(cx), 0.6, float(cy))
	var dir := Vector3(float(cdx), 0.0, float(cdy))
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
		"town_build":
			spawn_vfx(VFX_HIT, at + Vector3(0.0, 0.5, 0.0), 2.4)
			spawn_sparkle(at, 2.0)
			spawn_sparkle(at + Vector3(-0.9, 0.3, 0.4), 1.5)
			spawn_sparkle(at + Vector3(0.9, 0.3, -0.4), 1.5)
		"town_relocate":
			spawn_sparkle(at, 1.8)
		"town_tier_up":
			spawn_sparkle(at, 2.2)
		"companion", "haste":
			spawn_sparkle(at, 1.0)
		"companion_hit":
			if dir.length() > 0.0:
				var from := Vector3(at.x - dir.x, 0.0, at.z - dir.z)
				var swing := VFXEffect.spawn(VFX_SLASH, fx_root, from, Basis.looking_at(dir.normalized(), Vector3.UP))
				swing.scale = Vector3.ONE * (SLASH_SCALE * 0.8)
			spawn_vfx(VFX_HIT, at, 0.85)
		"deathless", "boss_roar":
			spawn_vfx(VFX_HIT, at + Vector3(0.0, 0.5, 0.0), 2.0)
			spawn_sparkle(at, 1.4)
		"spore_burst":
			spawn_vfx(VFX_HIT, at, 2.2)
	play_sfx(kind)


func spawn_vfx(scene: PackedScene, at: Vector3, scale: float) -> void:
	var fx := VFXEffect.spawn(scene, fx_root, at)
	fx.scale = Vector3.ONE * scale


## pickup_sparkle loops until stopped; stop() lets live particles finish
## and then frees the node (auto_free).
func spawn_sparkle(at: Vector3, scale: float) -> void:
	var fx := VFXEffect.spawn(VFX_SPARKLE, fx_root, at)
	fx.scale = Vector3.ONE * scale
	get_tree().create_timer(SPARKLE_SECONDS).timeout.connect(func() -> void:
		if is_instance_valid(fx):
			fx.stop())


func play_sfx(kind: String) -> void:
	var stream := sfx_stream(kind)
	if stream == null:
		return
	var ap: AudioStreamPlayer = sfx_players[sfx_next] as AudioStreamPlayer
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
	var ap: AudioStreamPlayer = sfx_players[sfx_next] as AudioStreamPlayer
	sfx_next = (sfx_next + 1) % sfx_players.size()
	ap.stream = sfx_cache[path] as AudioStream
	ap.volume_db = -8.0
	ap.play()


func sfx_stream(kind: String) -> AudioStream:
	for path_var: Variant in (SFX.get(kind, []) as Array):
		var path: String = str(path_var)
		if not sfx_cache.has(path):
			sfx_cache[path] = load(path) if ResourceLoader.exists(path) else null
		if sfx_cache[path] != null:
			return sfx_cache[path] as AudioStream
	return null


# ---------------------------------------------------------------------------
# Skills: hotbar, aiming, telegraphs, skills panel (rules in combat_turn.rs)
# ---------------------------------------------------------------------------

func is_aiming() -> bool:
	return state.get("armedSkill") != null


## Forward one session call that returns a bridge event, then repaint.
func do_session_action(call: Callable) -> void:
	var ev: String = str(call.call())
	handle_event(ev)
	refresh()


func update_hotbar() -> void:
	clear_children(hotbar_box)
	var armed: Variant = state.get("armedSkill")
	var slots: Array = state.get("hotbar", []) as Array
	for i: int in slots.size():
		var slot: Variant = slots[i]
		var b := make_button("")
		b.custom_minimum_size = Vector2(102, 52)
		b.add_theme_font_size_override("font_size", 12)
		if slot == null:
			b.text = "%d\n—" % (i + 1)
			b.disabled = true
		else:
			var sk: Dictionary = slot as Dictionary
			var cd := _as_int(sk.get("cooldown", 0))
			var tail := "%d MP" % _as_int(sk.get("manaCost", 0))
			if cd > 0:
				tail = "cd %dt" % cd
			b.text = "%d %s %s\n%s" % [i + 1, str(sk.get("glyph", "")), str(sk.get("name", "")), tail]
			b.tooltip_text = "%s · rank %d · range %d" % [str(sk.get("name", "")), _as_int(sk.get("rank", 0)), _as_int(sk.get("range", 0))]
			if not _as_bool(sk.get("ready", false)):
				b.modulate = Color(0.6, 0.6, 0.65)
			if armed != null and str(armed) == str(sk.get("id", "")):
				b.modulate = Color(1.0, 0.9, 0.4)
			b.pressed.connect(func() -> void:
				do_session_action(func() -> String: return session.trigger_hotbar(i)))
		hotbar_box.add_child(b)


## Aim cursor on the aimed tile; red tiles where telegraphed enemy attacks
## will land next turn.
func update_combat_overlays() -> void:
	var aim_var: Variant = state.get("aimedTarget")
	aim_marker.visible = is_aiming() and aim_var is Dictionary
	if aim_marker.visible:
		var aim: Dictionary = aim_var as Dictionary
		aim_marker.position = Vector3(_as_float(aim["x"]), 0.04, _as_float(aim["y"]))
	clear_children(telegraph_root)
	var zone: Dictionary = state["zone"]
	for e_var: Variant in (zone.get("enemies", []) as Array):
		var e: Dictionary = e_var as Dictionary
		var intent_var: Variant = e.get("intent")
		if not (intent_var is Dictionary):
			continue
		var intent: Dictionary = intent_var as Dictionary
		for t_var: Variant in (intent.get("targetTiles", []) as Array):
			var t: Dictionary = t_var as Dictionary
			var m := make_tile_marker(Color(0.95, 0.15, 0.1, 0.45), 0.03)
			m.position = Vector3(_as_float(t["x"]), 0.02, _as_float(t["y"]))
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
	var data: Dictionary = parsed as Dictionary
	var head := make_label()
	head.add_theme_font_size_override("font_size", 17)
	head.text = "Skills — %d point(s) to spend   (K / Esc to close)" % _as_int(data.get("skillPoints", 0))
	skills_box.add_child(head)
	var branch := ""
	for sk_v: Variant in (data.get("skills", []) as Array):
		var sk: Dictionary = sk_v as Dictionary
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
		var kind := "passive" if str(sk["kind"]) == "passive" else "%d MP · cd %d" % [_as_int(sk["manaCost"]), _as_int(sk["cooldown"])]
		var desc := str(sk["summary"])
		if sk.get("next") != null:
			desc += "\nNext: " + str(sk["next"])
		l.text = "%s %s  (tier %d, rank %d/3, %s)\n%s" % [str(sk["glyph"]), str(sk["name"]), _as_int(sk["tier"]), _as_int(sk["rank"]), kind, desc]
		row.add_child(l)
		var b := make_button("Learn")
		b.disabled = not _as_bool(sk.get("canLearn", false))
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
# Save / continue (slots 1-3; rules + format in session_save.rs)
# ---------------------------------------------------------------------------

func save_dir() -> String:
	if save_dir_override != "":
		return save_dir_override
	return OS.get_user_data_dir().path_join("saves")


func autosave() -> void:
	if not saving_enabled or title_screen != null:
		return # nothing to save before the player picks Continue / New Game
	actions_since_save = 0
	if not session.save_game(save_dir(), active_slot):
		push_warning("ElfStory: autosave failed (%s)" % save_dir())


func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		autosave()


## On launch: the full-screen title (title_screen.gd) over the idle town.
## Picking New Adventure or Continue hands the game over and closes it.
func show_title() -> void:
	title_screen = TitleScreen.new() as TitleScreen
	title_screen.setup(session, save_dir())
	title_screen.start_new.connect(_on_title_start_new)
	title_screen.continue_slot.connect(_on_title_continue)
	title_screen.ui_click.connect(play_sfx.bind("ui_click"))
	var layer := CanvasLayer.new()
	layer.name = "Title"
	layer.layer = 10 # above the HUD
	layer.add_child(title_screen)
	add_child(layer)


func _on_title_start_new(slot: int, starter_class: String, difficulty: String) -> void:
	var seed := int(Time.get_unix_time_from_system()) & 0x7fffffff
	session.new_game_with_difficulty("Garth", starter_class, seed, difficulty)
	active_slot = slot
	close_title()
	reset_view()
	autosave() # the slot shows the new adventure straight away


func _on_title_continue(slot: int) -> void:
	var res: String = session.load_game(save_dir(), slot)
	if res != "ok":
		title_screen.show_slot_error(slot, res)
		return
	active_slot = slot
	close_title()
	reset_view()


## A different game is now in the session: rebuild everything for it.
func reset_view() -> void:
	current_zone_id = ""
	current_build_key = ""
	camera_snapped = false
	actions_since_save = 0
	refresh()


func close_title() -> void:
	if title_screen != null:
		title_screen.get_parent().queue_free() # its CanvasLayer
		title_screen = null
