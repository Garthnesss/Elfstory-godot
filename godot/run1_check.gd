extends SceneTree

## Headless bridge smoke test for Milestone 2.
## Run from the project root (godot/):
##   godot --headless --script run1_check.gd
## It only proves the GDExtension loads, a new game starts in Oakhaven and
## movement round-trips through the Rust core. The full Run 1 gate flow
## (bank pieces -> rebuild forge -> buy the blade) is covered by Rust tests.

func _initialize() -> void:
	# ClassDB.instantiate avoids a hard parse-time dependency on the
	# extension class, so a missing .dylib reports cleanly instead of
	# failing to compile this script.
	var session: Object = null
	if ClassDB.class_exists("ElfStorySession"):
		session = ClassDB.instantiate("ElfStorySession")
	if session == null:
		push_error("RUN1_CHECK_FAIL: ElfStorySession class not available (extension not loaded?)")
		quit(1)
		return

	session.call("new_game", "Garth", "Vanguard", 20261120)

	var state_raw: Variant = JSON.parse_string(str(session.call("state_json")))
	if not (state_raw is Dictionary):
		push_error("RUN1_CHECK_FAIL: state_json() did not parse")
		quit(1)
		return
	var state: Dictionary = state_raw
	var zone_dict: Dictionary = state["zone"]
	var zone_id := str(zone_dict["id"])
	if zone_id != "oakhaven":
		push_error("RUN1_CHECK_FAIL: expected zone 'oakhaven', got '" + zone_id + "'")
		quit(1)
		return

	# A few steps in a small square; the core clamps illegal moves itself.
	session.call("move_player", 1, 0)
	session.call("move_player", 0, 1)
	session.call("move_player", -1, 0)
	session.call("move_player", 0, -1)

	# M3 dialogue bridge smoke: no conversation is open yet, so the node
	# JSON is null and a stray choice is a safe no-op.
	if str(session.call("dialogue_json")) != "null":
		push_error("RUN1_CHECK_FAIL: dialogue_json() should be 'null' with no active dialogue")
		quit(1)
		return
	if str(session.call("choose_dialogue", "trade")) != "none":
		push_error("RUN1_CHECK_FAIL: choose_dialogue with no dialogue should return 'none'")
		quit(1)
		return
	session.call("close_dialogue")

	# World-tile prompt bridge smoke: nothing is open in Oakhaven at start.
	if str(session.call("prompt_json")) != "null":
		push_error("RUN1_CHECK_FAIL: prompt_json() should be 'null' with no open prompt")
		quit(1)
		return
	if str(session.call("choose_prompt", "open_vault")) != "none":
		push_error("RUN1_CHECK_FAIL: choose_prompt with no prompt should return 'none'")
		quit(1)
		return
	session.call("close_prompt")

	var after_raw: Variant = JSON.parse_string(str(session.call("state_json")))
	var after: Dictionary = after_raw
	var after_zone: Dictionary = after["zone"]
	var p: Dictionary = after["player"]

	# Title / save-slot bridge smoke (slots 1-3 + difficulty).
	var save_dir := "/tmp/elfstory_run1_check_saves"
	for s: int in [1, 2, 3]:
		session.call("delete_slot", save_dir, s)
	session.call("new_game_with_difficulty", "Garth", "Shadowcraft", 123, "hardened")
	if not _as_bool(session.call("save_game", save_dir, 2)):
		push_error("RUN1_CHECK_FAIL: save_game(slot 2) returned false")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	var s1_raw: Variant = JSON.parse_string(str(session.call("save_summary_json", save_dir, 1)))
	var s2_raw: Variant = JSON.parse_string(str(session.call("save_summary_json", save_dir, 2)))
	if not (s1_raw is Dictionary):
		push_error("RUN1_CHECK_FAIL: slot 1 summary should be 'empty'")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	var s1: Dictionary = s1_raw
	if str(s1.get("status", "")) != "empty":
		push_error("RUN1_CHECK_FAIL: slot 1 summary should be 'empty'")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	if not (s2_raw is Dictionary):
		push_error("RUN1_CHECK_FAIL: slot 2 summary should be 'ok' with Hardened difficulty")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	var s2: Dictionary = s2_raw
	if str(s2.get("status", "")) != "ok" or str(s2.get("difficulty", "")) != "Hardened":
		push_error("RUN1_CHECK_FAIL: slot 2 summary should be 'ok' with Hardened difficulty")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	if str(session.call("load_game", save_dir, 2)) != "ok":
		push_error("RUN1_CHECK_FAIL: load_game(slot 2) failed")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	session.call("delete_slot", save_dir, 2)
	var s2_after_raw: Variant = JSON.parse_string(str(session.call("save_summary_json", save_dir, 2)))
	if not (s2_after_raw is Dictionary):
		push_error("RUN1_CHECK_FAIL: delete_slot(2) did not empty slot 2")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	var s2_after: Dictionary = s2_after_raw
	if str(s2_after.get("status", "")) != "empty":
		push_error("RUN1_CHECK_FAIL: delete_slot(2) did not empty slot 2")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	_cleanup_save_dir(session, save_dir)

	print("RUN1_CHECK_OK player=(%d,%d) zone=%s" % [_as_int(p["x"]), _as_int(p["y"]), str(after_zone["id"])])
	quit(0)


func _as_bool(v: Variant) -> bool:
	return v is bool and (v as bool)


func _as_int(v: Variant, fallback: int = 0) -> int:
	if v is int:
		return v
	if v is float:
		var vf: float = v
		return int(vf)
	return fallback


func _cleanup_save_dir(session: Object, save_dir: String) -> void:
	for s: int in [1, 2, 3]:
		session.call("delete_slot", save_dir, s)
	if DirAccess.dir_exists_absolute(save_dir):
		DirAccess.remove_absolute(save_dir)
