extends SceneTree
## Drives the real Main scene: pack, shop, rest, auto-explore, fog, journal,
## ending and New Game+, exactly as the buttons do. Never saves.

var _fails := 0
var _main: Node
var _session: Object


func _initialize() -> void:
	_main = (load("res://Main.tscn") as PackedScene).instantiate()
	_main.set("saving_enabled", false)
	root.add_child(_main)
	await process_frame
	_session = _main.get("session") as Object
	if _session == null:
		push_error("UI_CHECK_FAIL: no session (GDExtension not built?)")
		quit(1)
		return
	await _check_pack()
	await _check_shop()
	await _check_dungeon()
	await _check_story()
	print("UI_CHECK_OK" if _fails == 0 else "UI_CHECK_FAIL %d" % _fails)
	quit(0 if _fails == 0 else 1)


func _state() -> Dictionary:
	return _main.get("state") as Dictionary


func _player() -> Dictionary:
	return _state()["player"] as Dictionary


func _check_pack() -> void:
	var pack := _main.get("pack_screen") as PackScreen
	pack.open()
	_ok(pack.visible and UiKit.as_flag(_main.call("modal_open")), "pack opens and is modal")
	var was := UiKit.as_int((_state()["derived"] as Dictionary)["atk"])
	pack.act("unequip", "weapon")
	await process_frame
	_ok(UiKit.as_int((_state()["derived"] as Dictionary)["atk"]) < was, "taking off the weapon lowers ATK in the HUD state")
	var id := ""
	for i: Variant in UiKit.as_list(UiKit.parse(str(_session.call("pack_json"))).get("items")):
		if UiKit.as_dict(i).get("slot") == "weapon":
			id = str(UiKit.as_dict(i)["id"])
	pack.act("equip", id)
	await process_frame
	_ok(UiKit.as_int((_state()["derived"] as Dictionary)["atk"]) == was, "wearing it again restores ATK")
	_main.call("_back")
	_ok(not pack.visible, "Esc closes the pack")


func _check_shop() -> void:
	_main.call("open_merchant", "torin")
	var before := UiKit.as_int(_player()["gold"])
	var shelf := (JSON.parse_string(str(_session.call("merchant_stock_json", "torin"))) as Array).size()
	_main.call("_on_buy_item", 3)
	await process_frame
	_ok(UiKit.as_int(_player()["gold"]) < before, "buying costs gold")
	_ok((JSON.parse_string(str(_session.call("merchant_stock_json", "torin"))) as Array).size() == shelf - 1, "the item leaves the shelf")
	var gold := UiKit.as_int(_player()["gold"])
	var items := UiKit.as_list(UiKit.parse(str(_session.call("pack_json"))).get("items"))
	_main.call("_merchant_act", "sell", str(UiKit.as_dict(items[0])["id"]))
	await process_frame
	_ok(UiKit.as_int(_player()["gold"]) > gold, "selling pays")
	_main.call("_back")


func _check_dungeon() -> void:
	_session.call("debug_warp", "dungeon_1", -1, -1)
	_main.call("refresh")
	await process_frame
	var zone: Dictionary = _state()["zone"]
	_ok((zone["tiles"] as Array).size() > 0, "terrain arrives on a zone change")
	var hidden := 0
	var shown_through_walls := 0
	var nodes: Dictionary = _main.get("entity_nodes")
	for e_v: Variant in zone["enemies"] as Array:
		var e := UiKit.as_dict(e_v)
		var n := nodes.get("enemy:" + str(e["id"])) as Node3D
		if not UiKit.as_flag(e["visible"]):
			hidden += 1
			if n != null and n.visible:
				shown_through_walls += 1
	_ok(hidden > 0 and shown_through_walls == 0, "foes out of sight are not drawn (%d hidden)" % hidden)
	_main.call("refresh")
	_ok(((_state()["zone"] as Dictionary)["tiles"] as Array).size() > 0, "cached terrain survives a slim snapshot")
	var start := JSON.stringify((_state()["zone"] as Dictionary)["sight"])
	_main.set("auto_kind", "explore")
	for i: int in 240:
		await process_frame
		if str(_main.get("auto_kind")) == "":
			break
	_ok(JSON.stringify((_state()["zone"] as Dictionary)["sight"]) != start, "auto-explore uncovers ground")


func _check_story() -> void:
	(_main.get("journal_screen") as JournalScreen).open()
	_main.call("_back")
	_session.call("debug_warp", "oakhaven", -1, -1)
	_session.call("debug_finish_story")
	_main.call("refresh")
	await process_frame
	var ending := _main.get("ending_screen") as EndingScreen
	_ok(ending.visible, "the ending shows itself when Malakor falls")
	_main.call("_back")
	_main.call("refresh")
	_ok(not ending.visible, "and only once")
	_main.call("_on_ascend")
	await process_frame
	_ok(UiKit.as_int(_state()["cycle"]) == 1, "ascending starts Cycle 1")
	_main.call("_open_pause")
	_ok((_main.get("pause_menu") as PauseMenu).visible, "pause opens")
	_main.call("_back")
	_ok(not UiKit.as_flag(_main.call("modal_open")), "everything closes")


func _ok(cond: bool, label: String) -> void:
	print(("PASS " if cond else "FAIL ") + label)
	if not cond:
		_fails += 1
