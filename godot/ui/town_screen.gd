class_name TownScreen
extends Control
## Oakhaven's progress: what is banked, what is missing, what the town gives you.
## Information only. Building happens in the world, standing at a lot.

signal closed

var _body: VBoxContainer


func setup(theme_in: Theme, panel: PanelContainer) -> void:
	var col := UiKit.modal(self, theme_in, panel, "Oakhaven", Vector2(640, 540), _close)
	var s := UiKit.scroll()
	col.add_child(s)
	_body = UiKit.column(5)
	s.add_child(_body)
	hide()


func open(board: Dictionary) -> void:
	UiKit.clear(_body)
	var tier := UiKit.as_int(board.get("tier"))
	_body.add_child(UiKit.label("Population %d    Tier %d of 3    Returns %d" % [UiKit.as_int(board.get("population")), tier, UiKit.as_int(board.get("returns"))], 16))
	var b := UiKit.as_dict(board.get("bonuses"))
	if tier > 0:
		_body.add_child(UiKit.label("The town gives you  +%d HP   +%d ATK   +%d Spell   +%d DEF" % [UiKit.as_int(b.get("maxHp")), UiKit.as_int(b.get("atk")), UiKit.as_int(b.get("spellPower")), UiKit.as_int(b.get("def"))], 15, UiKit.GOOD))
	else:
		_body.add_child(UiKit.wrapped("At population 20 the village starts to grow, and you grow with it.", 14, UiKit.MUTED))

	_body.add_child(UiKit.label("Torin's Blacksmith", 18, UiKit.GOLD))
	if UiKit.as_flag(board.get("complete")):
		for l_v: Variant in UiKit.as_list(board.get("lots")):
			var l := UiKit.as_dict(l_v)
			if UiKit.as_flag(l["current"]):
				_body.add_child(UiKit.label("Standing at the %s. Its shop is open." % str(l["label"]), 15, UiKit.GOOD))
		_body.add_child(UiKit.wrapped("To move it, stand at another staked lot and press F. Moving is free.", 14, UiKit.MUTED))
	else:
		for p_v: Variant in UiKit.as_list(board.get("pieces")):
			var p := UiKit.as_dict(p_v)
			_tick(UiKit.as_flag(p["found"]), str(p["name"]))
		_tick(UiKit.as_flag(board.get("heartcoal")), "Heartcoal (until it is home, the town does not grow)")
		if UiKit.as_flag(board.get("canBuild")):
			_body.add_child(UiKit.wrapped("Ready. Choose a staked lot in town, stand at it, and press F.", 15, UiKit.GOLD))
		else:
			_body.add_child(UiKit.wrapped("All four lie on the first floor of the Reliquary. They count once you come home alive. Rebuilding is up to you; nothing else waits on it.", 14, UiKit.MUTED))
	if not UiKit.as_flag(board.get("wishClaimed")):
		_body.add_child(UiKit.wrapped("Torin's wish: the Pondside Lot (+75 gold).", 14, UiKit.MUTED))

	_body.add_child(UiKit.label("Supplies from the side dungeons", 18, UiKit.GOLD))
	for s_v: Variant in UiKit.as_list(board.get("supplies")):
		var sup := UiKit.as_dict(s_v)
		_tick(UiKit.as_flag(sup["found"]), "%s: adds %s to the Blacksmith's shelf" % [str(sup["name"]), str(sup["unlocks"])])
	show()


func _tick(done: bool, text: String) -> void:
	_body.add_child(UiKit.wrapped(("✓  " if done else "○  ") + text, 14, UiKit.GOOD if done else UiKit.INK))


func _close() -> void:
	hide()
	closed.emit()
