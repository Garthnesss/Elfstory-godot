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

	session.new_game("Garth", "Vanguard", 20261120)

	var state: Variant = JSON.parse_string(session.state_json())
	if state == null:
		push_error("RUN1_CHECK_FAIL: state_json() did not parse")
		quit(1)
		return
	var zone_id := str(state["zone"]["id"])
	if zone_id != "oakhaven":
		push_error("RUN1_CHECK_FAIL: expected zone 'oakhaven', got '" + zone_id + "'")
		quit(1)
		return

	# A few steps in a small square; the core clamps illegal moves itself.
	session.move_player(1, 0)
	session.move_player(0, 1)
	session.move_player(-1, 0)
	session.move_player(0, -1)

	# M3 dialogue bridge smoke: no conversation is open yet, so the node
	# JSON is null and a stray choice is a safe no-op.
	if str(session.dialogue_json()) != "null":
		push_error("RUN1_CHECK_FAIL: dialogue_json() should be 'null' with no active dialogue")
		quit(1)
		return
	if str(session.choose_dialogue("trade")) != "none":
		push_error("RUN1_CHECK_FAIL: choose_dialogue with no dialogue should return 'none'")
		quit(1)
		return
	session.close_dialogue()

	# World-tile prompt bridge smoke: nothing is open in Oakhaven at start.
	if str(session.prompt_json()) != "null":
		push_error("RUN1_CHECK_FAIL: prompt_json() should be 'null' with no open prompt")
		quit(1)
		return
	if str(session.choose_prompt("open_vault")) != "none":
		push_error("RUN1_CHECK_FAIL: choose_prompt with no prompt should return 'none'")
		quit(1)
		return
	session.close_prompt()

	var after: Variant = JSON.parse_string(session.state_json())
	var p: Dictionary = after["player"]

	# Title / save-slot bridge smoke (slots 1-3 + difficulty).
	var save_dir := "/tmp/elfstory_run1_check_saves"
	for s in [1, 2, 3]:
		session.delete_slot(save_dir, s)
	session.new_game_with_difficulty("Garth", "Shadowcraft", 123, "hardened")
	if not session.save_game(save_dir, 2):
		push_error("RUN1_CHECK_FAIL: save_game(slot 2) returned false")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	var s1: Variant = JSON.parse_string(session.save_summary_json(save_dir, 1))
	var s2: Variant = JSON.parse_string(session.save_summary_json(save_dir, 2))
	if not (s1 is Dictionary) or str(s1.get("status", "")) != "empty":
		push_error("RUN1_CHECK_FAIL: slot 1 summary should be 'empty'")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	if not (s2 is Dictionary) or str(s2.get("status", "")) != "ok" or str(s2.get("difficulty", "")) != "Hardened":
		push_error("RUN1_CHECK_FAIL: slot 2 summary should be 'ok' with Hardened difficulty")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	if str(session.load_game(save_dir, 2)) != "ok":
		push_error("RUN1_CHECK_FAIL: load_game(slot 2) failed")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	session.delete_slot(save_dir, 2)
	var s2_after: Variant = JSON.parse_string(session.save_summary_json(save_dir, 2))
	if not (s2_after is Dictionary) or str(s2_after.get("status", "")) != "empty":
		push_error("RUN1_CHECK_FAIL: delete_slot(2) did not empty slot 2")
		_cleanup_save_dir(session, save_dir)
		quit(1)
		return
	_cleanup_save_dir(session, save_dir)

	print("RUN1_CHECK_OK player=(%d,%d) zone=%s" % [int(p["x"]), int(p["y"]), str(after["zone"]["id"])])
	quit(0)


func _cleanup_save_dir(session: Object, save_dir: String) -> void:
	for s in [1, 2, 3]:
		session.delete_slot(save_dir, s)
	if DirAccess.dir_exists_absolute(save_dir):
		DirAccess.remove_absolute(save_dir)
