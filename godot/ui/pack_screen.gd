class_name PackScreen
extends Control
## Pack and character: wear, use, salvage, spend points, claim capstones.
## Pure view: draws pack_json(), sends pack_action(). Rules live in pack.rs.

signal acted(result: Dictionary)
signal closed

var session: ElfStorySession

var _panel: PanelContainer
var _equipped: VBoxContainer
var _items: VBoxContainer
var _detail: VBoxContainer
var _character: VBoxContainer
var _status: Label
var _selected := ""
var _filter := "all"


func setup(p_session: ElfStorySession, theme_in: Theme, panel: PanelContainer) -> void:
	session = p_session
	_panel = panel
	var col := UiKit.modal(self, theme_in, panel, "Pack", Vector2(1080, 590), _close)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 18)
	row.size_flags_vertical = Control.SIZE_EXPAND_FILL
	col.add_child(row)

	var left := UiKit.column()
	left.custom_minimum_size.x = 270
	row.add_child(left)
	left.add_child(UiKit.label("Worn", 17, UiKit.GOLD))
	_equipped = UiKit.column()
	left.add_child(_equipped)
	var cs := UiKit.scroll()
	left.add_child(cs)
	_character = UiKit.column()
	cs.add_child(_character)

	var mid := UiKit.column()
	row.add_child(mid)
	var filters := HBoxContainer.new()
	mid.add_child(filters)
	for f: Array in [["all", "All"], ["equipment", "Gear"], ["consumable", "Supplies"], ["quest", "Quest"]]:
		var b := UiKit.button(str(f[1]))
		b.pressed.connect(_set_filter.bind(str(f[0])))
		filters.add_child(b)
	var s := UiKit.scroll()
	mid.add_child(s)
	_items = UiKit.column(3)
	s.add_child(_items)

	var right := UiKit.column()
	right.custom_minimum_size.x = 320
	row.add_child(right)
	_detail = UiKit.column(6)
	right.add_child(_detail)

	_status = UiKit.label("", 14, UiKit.MUTED)
	col.add_child(_status)
	hide()


func open() -> void:
	_status.text = ""
	rebuild()
	show()
	UiKit.focus_first(_items)


func _close() -> void:
	hide()
	closed.emit()


func _set_filter(f: String) -> void:
	_filter = f
	rebuild()


func act(action: String, arg: String) -> void:
	var r := UiKit.parse(session.pack_action(action, arg))
	_status.text = str(r.get("msg", ""))
	_status.add_theme_color_override("font_color", UiKit.GOOD if UiKit.as_flag(r.get("ok")) else UiKit.BAD)
	acted.emit(r)
	if visible:
		rebuild()


func rebuild() -> void:
	var d := UiKit.parse(session.pack_json())
	UiKit.clear(_equipped)
	for e_v: Variant in UiKit.as_list(d.get("equipped")):
		var e := UiKit.as_dict(e_v)
		var item := UiKit.as_dict(e.get("item"))
		var b := UiKit.button("%s:  %s" % [str(e["slot"]).capitalize(), str(item.get("name", "empty"))])
		b.alignment = HORIZONTAL_ALIGNMENT_LEFT
		b.clip_text = true
		b.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		if item.is_empty():
			b.disabled = true
		else:
			b.add_theme_color_override("font_color", Color(str(item["color"])))
			b.pressed.connect(_select.bind(str(item["id"])))
		_equipped.add_child(b)

	UiKit.clear(_items)
	var shown := 0
	var pick := {}
	for i_v: Variant in UiKit.as_list(d.get("items")):
		var it := UiKit.as_dict(i_v)
		if str(it["id"]) == _selected:
			pick = it
		if _filter != "all" and str(it["category"]) != _filter:
			continue
		shown += 1
		_items.add_child(_row(it))
	if shown == 0:
		_items.add_child(UiKit.label("Nothing here.", 14, UiKit.MUTED))
	for e_v: Variant in UiKit.as_list(d.get("equipped")):
		var item := UiKit.as_dict(UiKit.as_dict(e_v).get("item"))
		if str(item.get("id", "")) == _selected:
			pick = item
	_build_detail(pick, d)
	_build_character(d)


func _row(it: Dictionary) -> Button:
	var qty := UiKit.as_int(it["quantity"])
	var text := "%s %s%s" % [str(it["glyph"]), str(it["name"]), "  ×%d" % qty if qty > 1 else ""]
	if UiKit.as_flag(it["upgrade"]):
		text += "   ▲ better"
	var b := UiKit.button(text)
	b.alignment = HORIZONTAL_ALIGNMENT_LEFT
	b.clip_text = true
	b.add_theme_color_override("font_color", Color(str(it["color"])))
	b.pressed.connect(_select.bind(str(it["id"])))
	return b


func _select(id: String) -> void:
	_selected = id
	rebuild()
	UiKit.focus_first(_detail)


func _build_detail(it: Dictionary, d: Dictionary) -> void:
	UiKit.clear(_detail)
	if it.is_empty():
		_detail.add_child(UiKit.wrapped("Pick something to see what it does.", 14, UiKit.MUTED))
		if UiKit.as_int(d.get("commonCount")) > 0:
			var all := UiKit.button("Salvage all %d Common gear" % UiKit.as_int(d.get("commonCount")))
			all.pressed.connect(act.bind("salvage_common", ""))
			_detail.add_child(all)
		return
	var id := str(it["id"])
	_detail.add_child(UiKit.wrapped(str(it["name"]), 18, Color(str(it["color"]))))
	var kind := str(it["rarity"])
	if it.get("slot") != null:
		kind += " " + str(it["slot"])
	_detail.add_child(UiKit.label(kind, 13, UiKit.MUTED))
	for key: String in ["stats", "losses"]:
		for s_v: Variant in UiKit.as_list(it.get(key)):
			var s := UiKit.as_dict(s_v)
			var delta := UiKit.as_num(s["delta"])
			var line := "%s  %s" % [str(s["label"]), str(s["value"])]
			if delta != 0.0:
				line += "   (%+d vs worn)" % int(delta)
			_detail.add_child(UiKit.label(line, 14, UiKit.GOOD if delta > 0.0 else (UiKit.BAD if delta < 0.0 else UiKit.INK)))
	if it.get("affix") != null:
		_detail.add_child(UiKit.wrapped(str(it["affix"]), 14, UiKit.GOLD))
	_detail.add_child(UiKit.wrapped(str(it["description"]), 13, UiKit.MUTED))
	match str(it["category"]):
		"equipment":
			if UiKit.as_flag(it["worn"]):
				_action("Take off", "unequip", str(it["slot"]))
			else:
				_action("Wear", "equip", id)
				_action("Salvage  +%d gold" % UiKit.as_int(it["sell"]), "salvage", id)
		"consumable":
			_action("Use", "use", id)


func _action(text: String, action: String, arg: String) -> void:
	var b := UiKit.button(text)
	b.pressed.connect(act.bind(action, arg))
	_detail.add_child(b)


func _build_character(d: Dictionary) -> void:
	UiKit.clear(_character)
	var dv := UiKit.as_dict(d.get("derived"))
	_character.add_child(UiKit.label(str(dv.get("buildTitle", "")), 17, UiKit.GOLD))
	_character.add_child(UiKit.label("ATK %d   DEF %d   Spell %d" % [UiKit.as_int(dv.get("atk")), UiKit.as_int(dv.get("def")), UiKit.as_int(dv.get("spellPower"))], 14))
	_character.add_child(UiKit.label("Crit %d%%   Dodge %d%%   Lifesteal %d%%" % [UiKit.as_int(dv.get("critChance")), UiKit.as_int(dv.get("dodgeChance")), UiKit.as_int(dv.get("lifesteal"))], 14))
	_character.add_child(UiKit.label("%d gold   Pack %d/%d" % [UiKit.as_int(d.get("gold")), UiKit.as_int(d.get("count")), UiKit.as_int(d.get("limit"))], 14))
	var points := UiKit.as_int(d.get("attributePoints"))
	_character.add_child(UiKit.label("Attributes" + ("   %d to spend" % points if points > 0 else ""), 16, UiKit.GOLD))
	for a_v: Variant in UiKit.as_list(d.get("attributes")):
		var a := UiKit.as_dict(a_v)
		var row := HBoxContainer.new()
		var l := UiKit.label("%s  %d" % [str(a["name"]), UiKit.as_int(a["value"])], 14)
		l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		l.tooltip_text = str(a["description"])
		l.mouse_filter = Control.MOUSE_FILTER_PASS
		row.add_child(l)
		if points > 0:
			var b := UiKit.button("+1")
			b.pressed.connect(act.bind("attribute", str(a["id"])))
			row.add_child(b)
		_character.add_child(row)
	for s_v: Variant in UiKit.as_list(d.get("sets")):
		var s := UiKit.as_dict(s_v)
		if UiKit.as_int(s["count"]) == 0:
			continue
		_character.add_child(UiKit.wrapped("%s  %d/%d\n%s" % [str(s["name"]), UiKit.as_int(s["count"]), UiKit.as_int(s["total"]), str(s["bonus"])], 13, UiKit.GOOD if UiKit.as_flag(s["active"]) else UiKit.MUTED))
	_character.add_child(UiKit.label("Capstones", 16, UiKit.GOLD))
	for p_v: Variant in UiKit.as_list(d.get("prodigies")):
		var p := UiKit.as_dict(p_v)
		var claimed := UiKit.as_flag(p["claimed"])
		_character.add_child(UiKit.wrapped("%s %s%s" % [str(p["glyph"]), str(p["name"]), "  ✓" if claimed else ""], 14, UiKit.GOOD if claimed else UiKit.INK))
		_character.add_child(UiKit.wrapped(str(p["summary"]), 12, UiKit.MUTED))
		if UiKit.as_flag(p["canLearn"]):
			var b := UiKit.button("Claim")
			b.pressed.connect(act.bind("capstone", str(p["id"])))
			_character.add_child(b)
		elif not claimed:
			_character.add_child(UiKit.wrapped(str(p["reason"]), 12, UiKit.BAD))
