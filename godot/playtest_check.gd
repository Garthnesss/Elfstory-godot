extends SceneTree

## Headless full playtest for ElfStory (bridge level, real session).
## Run from the project root (godot/):
##   godot --headless --script playtest_check.gd
## Plays like a first-hour run: new game -> overworld rat fight (the new
## fig_blight_rat) -> dungeon_1 fight -> skills/hotbar smoke -> Torin
## dialogue -> completed-blacksmith merchant stock -> save/load round trip.
## Prints PLAYTEST_OK with a summary, or PLAYTEST_FAIL <reason> (exit 1).
## Soft findings (things that look off but don't break the run) print as
## PLAYTEST_NOTE lines for the audit log.

var s: Object
var notes: Array[String] = []
var kills_seen := 0
var damage_seen := false
var player_hurt := false


func _as_bool(v: Variant) -> bool:
	return v is bool and (v as bool)


func _as_int(v: Variant, fallback: int = 0) -> int:
	if v is int:
		return v
	if v is float:
		var vf: float = v
		return int(vf)
	return fallback


func fail(msg: String) -> void:
	push_error("PLAYTEST_FAIL: " + msg)
	quit(1)


func state() -> Dictionary:
	var st: Variant = JSON.parse_string(str(s.call("state_json")))
	if not (st is Dictionary):
		fail("state_json() did not parse")
		return {}
	return st as Dictionary


func player() -> Dictionary:
	return state()["player"] as Dictionary


func enemies() -> Array:
	var z: Dictionary = state()["zone"] as Dictionary
	return z.get("enemies", []) as Array


func find_enemy(eid: String) -> Dictionary:
	for e_var: Variant in enemies():
		if e_var is Dictionary:
			var e: Dictionary = e_var
			if str(e.get("id", "")) == eid:
				return e
	return {}


## Chase enemy `eid` with bump attacks for up to `steps` turns.
## Returns true when the enemy died or took damage.
func chase_and_fight(eid: String, steps: int) -> bool:
	var start_hp := _as_int(player().get("hp", 0))
	var hurt_or_kill := false
	for i: int in steps:
		var e := find_enemy(eid)
		if e.is_empty():
			kills_seen += 1
			return true
		if _as_int(e.get("hp", 0)) < _as_int(e.get("maxHp", 0)):
			damage_seen = true
			hurt_or_kill = true
		var p := player()
		var dx: int = _as_int(e["x"]) - _as_int(p["x"])
		var dy: int = _as_int(e["y"]) - _as_int(p["y"])
		if absi(dx) + absi(dy) == 1:
			s.call("move_player", signi(dx), signi(dy)) # bump = attack
		elif absi(dx) >= absi(dy):
			var ev: String = str(s.call("move_player", signi(dx), 0))
			if _as_int(player()["x"]) == _as_int(p["x"]) and signi(dy) != 0:
				s.call("move_player", 0, signi(dy))
			if ev == "player_died":
				break
		else:
			var ev2: String = str(s.call("move_player", 0, signi(dy)))
			if _as_int(player()["y"]) == _as_int(p["y"]) and signi(dx) != 0:
				s.call("move_player", signi(dx), 0)
			if ev2 == "player_died":
				break
		if _as_int(player().get("hp", 0)) < start_hp:
			player_hurt = true
		var e2 := find_enemy(eid)
		if e2.is_empty():
			kills_seen += 1
			return true
		if _as_int(e2.get("hp", 0)) < _as_int(e2.get("maxHp", 0)):
			damage_seen = true
			hurt_or_kill = true
	return hurt_or_kill


func signi(v: int) -> int:
	return 1 if v > 0 else (-1 if v < 0 else 0)


func _initialize() -> void:
	if not ClassDB.class_exists("ElfStorySession"):
		fail("ElfStorySession class not available")
		return
	s = ClassDB.instantiate("ElfStorySession")
	if s == null:
		fail("could not instantiate ElfStorySession")
		return

	# --- 1. New game -------------------------------------------------------
	s.call("new_game", "Playtest", "Vanguard", 20261120)
	var st := state()
	var zone_d: Dictionary = st["zone"] as Dictionary
	if str(zone_d["id"]) != "oakhaven":
		fail("new game should start in oakhaven, got " + str(zone_d["id"]))
		return
	var comp: Variant = player().get("companion", null)
	if comp is Dictionary and not (comp as Dictionary).is_empty():
		var comp_d: Dictionary = comp
		print("PLAYTEST companion: ", str(comp_d.get("name", "?")), " kind=", str(comp_d.get("kind", "?")))
	else:
		notes.append("Vanguard new game has no companion in state_json")

	# --- 2. Overworld: fight the fixed Blight Rat at (9,17) ----------------
	s.call("debug_warp", "overworld", 8, 17)
	st = state()
	zone_d = st["zone"] as Dictionary
	if str(zone_d["id"]) != "overworld":
		fail("warp to overworld failed")
		return
	var rat: Dictionary = {}
	for e_var: Variant in enemies():
		if e_var is Dictionary:
			var e: Dictionary = e_var
			if str(e.get("archetype", "")) == "plague_rat":
				rat = e
				break
	if rat.is_empty():
		fail("no plague_rat in overworld state (archetype field missing or no rat)")
		return
	print("PLAYTEST rat: id=", str(rat["id"]), " hp=", _as_int(rat["hp"]), "/", _as_int(rat["maxHp"]),
		" at ", _as_int(rat["x"]), ",", _as_int(rat["y"]))
	if not chase_and_fight(str(rat["id"]), 80):
		fail("could not damage or kill the overworld Blight Rat in 80 turns")
		return
	print("PLAYTEST rat fight done: damage_seen=", damage_seen, " kills=", kills_seen,
		" player_hurt=", player_hurt, " player_hp=", _as_int(player()["hp"]))

	# --- 3. Dungeon 1: fight the first enemy found --------------------------
	s.call("debug_warp", "dungeon_1", -1, -1)
	st = state()
	zone_d = st["zone"] as Dictionary
	if str(zone_d["id"]) != "dungeon_1":
		fail("warp to dungeon_1 failed, got " + str(zone_d["id"]))
		return
	var foes := enemies()
	if foes.is_empty():
		fail("dungeon_1 has no enemies")
		return
	var archs: Array[String] = []
	for e_var: Variant in foes:
		if e_var is Dictionary:
			var e: Dictionary = e_var
			archs.append(str(e.get("archetype", "?")))
	print("PLAYTEST dungeon_1 enemies: ", foes.size(), " archetypes=", archs)
	var target: Dictionary = foes[0] as Dictionary
	# Stand next to the target first (try its four neighbours).
	var p0 := player()
	var dirs: Array[Vector2i] = [Vector2i(-1, 0), Vector2i(1, 0), Vector2i(0, -1), Vector2i(0, 1)]
	for d: Vector2i in dirs:
		s.call("debug_warp", "dungeon_1", _as_int(target["x"]) + d.x, _as_int(target["y"]) + d.y)
		var pp := player()
		if _as_int(pp["x"]) == _as_int(target["x"]) + d.x and _as_int(pp["y"]) == _as_int(target["y"]) + d.y:
			break
	p0 = player()
	print("PLAYTEST dungeon target: ", str(target.get("name", "?")), " (", str(target.get("archetype", "?")),
		") player at ", _as_int(p0["x"]), ",", _as_int(p0["y"]))
	var hp_before := _as_int(player()["hp"])
	if not chase_and_fight(str(target["id"]), 80):
		notes.append("dungeon target took no damage in 80 turns (pathing or combat issue?)")
	print("PLAYTEST dungeon fight done: player_hp ", hp_before, " -> ", _as_int(player()["hp"]),
		" kills=", kills_seen)

	# --- 4. Skills / hotbar: cast Sundering Cleave (hotbar slot 0) -----------
	# Slots are 0-based in the bridge; targeted skills arm the aim and
	# confirm_target() fires. Stand next to a fresh dungeon enemy and check
	# that mana is spent or the enemy takes damage.
	var sk: Variant = JSON.parse_string(str(s.call("skills_json")))
	if sk == null:
		notes.append("skills_json() did not parse")
	var foes2 := enemies()
	if not foes2.is_empty():
		var t2: Dictionary = foes2[0] as Dictionary
		for d: Vector2i in dirs:
			s.call("debug_warp", "dungeon_1", _as_int(t2["x"]) + d.x, _as_int(t2["y"]) + d.y)
			var pp2 := player()
			if _as_int(pp2["x"]) == _as_int(t2["x"]) + d.x and _as_int(pp2["y"]) == _as_int(t2["y"]) + d.y:
				break
		var mana0 := _as_int(player().get("mana", player().get("mp", -1)), -1)
		var ehp0 := _as_int(find_enemy(str(t2["id"])).get("hp", -1), -1)
		s.call("trigger_hotbar", 0) # arms the aim (event is None by design)
		s.call("cycle_target")
		var ev_conf: String = str(s.call("confirm_target"))
		var e2 := find_enemy(str(t2["id"]))
		var ehp1 := _as_int(e2.get("hp", 0)) if not e2.is_empty() else 0
		var mana1 := _as_int(player().get("mana", player().get("mp", -1)), -1)
		print("PLAYTEST cleave: target=", str(t2.get("name", "?")), " hp ", ehp0, " -> ", ehp1,
			" mana ", mana0, " -> ", mana1, " confirm_event=", ev_conf)
		if ehp1 >= ehp0 and mana1 >= mana0 and not e2.is_empty():
			notes.append("Sundering Cleave via hotbar had no visible effect (no damage, no mana spent)")
		s.call("cancel_aim")
	else:
		notes.append("no dungeon enemy left for the skill cast test")

	# --- 5. Torin dialogue ----------------------------------------------------
	s.call("debug_warp", "oakhaven", 7, 5)
	var ev_int: String = str(s.call("interact"))
	var dj: String = str(s.call("dialogue_json"))
	print("PLAYTEST interact near Torin -> ", ev_int, " dialogue_open=", dj != "null")
	if dj == "null":
		notes.append("interact() next to Torin did not open a dialogue")
	else:
		s.call("close_dialogue")

	# --- 6. Completed blacksmith -> merchant stock -----------------------------
	s.call("debug_set_town", 12, "pondside")
	var stock: Variant = JSON.parse_string(str(s.call("merchant_stock_json", "blacksmith")))
	if stock == null:
		fail("merchant_stock_json(blacksmith) did not parse after town completion")
		return
	var names: Array[String] = []
	if stock is Array:
		for it_var: Variant in stock as Array:
			if it_var is Dictionary:
				var it: Dictionary = it_var
				names.append(str(it.get("name", "?")))
	print("PLAYTEST blacksmith stock: ", names)
	if names.is_empty():
		notes.append("blacksmith stock empty after debug_set_town completion")

	# --- 7. Save / load round trip ----------------------------------------------
	var dir := "/tmp/elfstory_playtest_saves"
	for sl: int in [1, 2, 3]:
		s.call("delete_slot", dir, sl)
	if not _as_bool(s.call("save_game", dir, 1)):
		fail("save_game(slot 1) failed")
		return
	var hp_saved := _as_int(player()["hp"])
	if str(s.call("load_game", dir, 1)) != "ok":
		fail("load_game(slot 1) failed")
		return
	if _as_int(player()["hp"]) != hp_saved:
		fail("save/load round trip changed player HP")
		return
	for sl: int in [1, 2, 3]:
		s.call("delete_slot", dir, sl)
	if DirAccess.dir_exists_absolute(dir):
		DirAccess.remove_absolute(dir)

	for n: String in notes:
		print("PLAYTEST_NOTE: ", n)
	var final_zone: Dictionary = state()["zone"] as Dictionary
	print("PLAYTEST_OK kills=", kills_seen, " damage_seen=", damage_seen,
		" player_hurt=", player_hurt, " notes=", notes.size(),
		" final_zone=", str(final_zone["id"]))
	quit(0)
