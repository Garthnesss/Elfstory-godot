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

	var after: Variant = JSON.parse_string(session.state_json())
	var p: Dictionary = after["player"]
	print("RUN1_CHECK_OK player=(%d,%d) zone=%s" % [int(p["x"]), int(p["y"]), str(after["zone"]["id"])])
	quit(0)
