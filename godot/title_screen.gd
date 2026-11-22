extends Control
## Title screen: crypt backdrop, ElfStory crest, starting discipline,
## difficulty, and three save slots (Continue / Delete / New Adventure).
## Ported from the web version's TitleScreen.tsx (layout and copy);
## Hardcore, Hall of Fame, Settings and Credits are not ported yet.
##
## Pure view: slot summaries and deletes go through the ElfStorySession
## bridge; starting or continuing a game is signalled to Main.gd.

signal start_new(slot: int, starter_class: String, difficulty: String)
signal continue_slot(slot: int)
signal ui_click

const BACKDROP := preload("res://assets/portraits/title_crypt_backdrop_1794536484639.jpg")

const GOLD := Color(0.90, 0.66, 0.23)
const GOLD_PALE := Color(0.96, 0.90, 0.78)
const INK := Color(0.93, 0.89, 0.83)
const MUTED := Color(0.66, 0.68, 0.73)
const CARD := Color(0.07, 0.08, 0.11, 0.82)
const DANGER := Color(0.92, 0.45, 0.45)

const CLASSES := [
	{"id": "Vanguard", "glyph": "⚔", "focus": "Sundering Cleave & Iron Guard",
		"stats": "Might 3 · Resolve 2 · Agility 1 · Arcana 1 · Starts with Shielding Rune",
		"lore": "Once a bridge-warden of the Silverglades (it was a small bridge; he describes it as large), Garth answers cinder-giants with broadsteel and unearned confidence."},
	{"id": "Shadowcraft", "glyph": "◈", "focus": "Shadowstep Ambush & Precision Crit",
		"stats": "Agility 3 · Resolve 2 · Might 1 · Arcana 1 · Starts with Phase Door Rune",
		"lore": "Trained among elven night-scouts, who asked him to leave for being too loud, Garth spots hidden traps long before he steps in them anyway."},
	{"id": "Runecaster", "glyph": "✶", "focus": "Ember Lance & Arcane Mastery",
		"stats": "Arcana 3 · Resolve 2 · Might 1 · Agility 1 · Starts with Shielding + Phase Runes",
		"lore": "An exiled scholar of the Ashen Glyph (exiled for setting the library on fire, twice), Garth bends ancient sigils into fire. Mostly on purpose."},
	{"id": "Wanderer", "glyph": "➶", "focus": "Balanced Hybrid & Viper Edge",
		"stats": "Might 2 · Agility 2 · Arcana 2 · Resolve 1 · Balanced Kit",
		"lore": "A seasoned frontier ranger who mixes venom-tipped darts, wild infusions, and quick steel as the ruin demands."},
]
# Labels/blurbs mirror elfstory-core difficulty_info().
const DIFFICULTIES := [
	{"id": "story", "label": "Story", "blurb": "Foes are frailer and strike softer; XP flows 15% faster. For the tale, not the trial."},
	{"id": "standard", "label": "Standard", "blurb": "The descent as tuned — every foe at full strength, XP as earned."},
	{"id": "hardened", "label": "Hardened", "blurb": "Foes are tougher and hit harder; XP flows 10% faster as compensation."},
]

var session: Object
var save_dir := ""
var active_slot := 1
var selected_class := "Vanguard"
var selected_difficulty := "standard"
var confirm_replace := false # second click on New Adventure over a save
var confirm_delete := 0 # slot awaiting a second Delete click (0 = none)

var title_font: SystemFont
var class_buttons: Array[Button] = []
var class_lore: Label
var class_stats: Label
var diff_buttons: Array[Button] = []
var diff_blurb: Label
var start_button: Button
var slots_box: VBoxContainer
var summaries := {} # slot -> Dictionary


func setup(p_session: Object, p_save_dir: String) -> void:
	session = p_session
	save_dir = p_save_dir


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_STOP
	title_font = SystemFont.new()
	title_font.font_names = PackedStringArray(["Cinzel", "Trajan Pro", "Georgia", "Times New Roman", "serif"])
	title_font.font_weight = 700

	# Backdrop + darkening gradients (heavy on the left behind the text).
	var bg := ColorRect.new()
	bg.color = Color(0.03, 0.035, 0.05)
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(bg)
	var art := TextureRect.new()
	art.texture = BACKDROP
	art.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	art.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_COVERED
	art.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	art.modulate = Color(1, 1, 1, 0.85)
	add_child(art)
	add_child(_gradient(Vector2(0, 0), Vector2(1, 0), [Color(0.027, 0.035, 0.05, 0.95), Color(0.027, 0.035, 0.05, 0.78), Color(0.027, 0.035, 0.05, 0.35)]))
	add_child(_gradient(Vector2(0, 1), Vector2(0, 0), [Color(0.027, 0.035, 0.05, 0.9), Color(0, 0, 0, 0), Color(0.027, 0.035, 0.05, 0.5)]))

	var margin := MarginContainer.new()
	margin.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	for side in ["left", "right"]:
		margin.add_theme_constant_override("margin_" + side, 44)
	margin.add_theme_constant_override("margin_top", 26)
	margin.add_theme_constant_override("margin_bottom", 18)
	add_child(margin)
	var outer := VBoxContainer.new()
	outer.add_theme_constant_override("separation", 10)
	margin.add_child(outer)

	var crest := _label("◈  THE SUNKEN CITADEL", 12, GOLD.darkened(0.1))
	outer.add_child(crest)

	var row := HBoxContainer.new()
	row.size_flags_vertical = Control.SIZE_EXPAND_FILL
	row.add_theme_constant_override("separation", 36)
	outer.add_child(row)

	row.add_child(_build_left())
	row.add_child(_build_slots())

	var hint := _label("WASD / Arrows move · F interact · 1–5 skills · K skill tree · V camera · [ ] rotate", 12, MUTED)
	hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	outer.add_child(hint)

	refresh_slots()
	_select_class(selected_class)
	_select_difficulty(selected_difficulty)


func _build_left() -> Control:
	var col := VBoxContainer.new()
	col.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	col.size_flags_stretch_ratio = 1.6
	col.add_theme_constant_override("separation", 8)

	var logo := _label("ELFSTORY", 54, GOLD)
	logo.add_theme_font_override("font", title_font)
	logo.add_theme_constant_override("outline_size", 2)
	logo.add_theme_color_override("font_outline_color", Color(0.35, 0.2, 0.05))
	col.add_child(logo)
	var rule := ColorRect.new()
	rule.color = Color(GOLD, 0.4)
	rule.custom_minimum_size = Vector2(300, 1)
	rule.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	col.add_child(rule)

	var tag := _label("The Ashen Glyph has woken, the Ember Blight is spreading, and the realm has called for a hero. It got Garth “Big Dick” Elfstrom, an elven outrider of tremendous self-belief and modest track record. Descend beneath Oakhaven, read enemy windups, gear up from the deep, and confront Sovereign Malakor — who has heard of Garth, and is not worried.", 13, INK)
	tag.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	tag.custom_minimum_size = Vector2(480, 0)
	col.add_child(tag)

	col.add_child(_section("Starting discipline  ·  every skill branch stays open"))
	var grid := GridContainer.new()
	grid.columns = 2
	grid.add_theme_constant_override("h_separation", 8)
	grid.add_theme_constant_override("v_separation", 6)
	col.add_child(grid)
	for c in CLASSES:
		var b := _choice_button("%s  %s\n%s" % [c["glyph"], c["id"], c["focus"]])
		b.custom_minimum_size = Vector2(250, 46)
		b.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		b.pressed.connect(_select_class.bind(str(c["id"])))
		grid.add_child(b)
		class_buttons.append(b)
	class_lore = _label("", 12, INK.darkened(0.08))
	class_lore.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	class_lore.custom_minimum_size = Vector2(480, 0)
	col.add_child(class_lore)
	class_stats = _label("", 11, GOLD_PALE.darkened(0.2))
	col.add_child(class_stats)

	col.add_child(_section("Difficulty"))
	var drow := HBoxContainer.new()
	drow.add_theme_constant_override("separation", 8)
	col.add_child(drow)
	for d in DIFFICULTIES:
		var b := _choice_button(str(d["label"]))
		b.custom_minimum_size = Vector2(110, 34)
		b.pressed.connect(_select_difficulty.bind(str(d["id"])))
		drow.add_child(b)
		diff_buttons.append(b)
	diff_blurb = _label("", 12, INK.darkened(0.08))
	diff_blurb.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	diff_blurb.custom_minimum_size = Vector2(480, 0)
	col.add_child(diff_blurb)

	start_button = Button.new()
	start_button.custom_minimum_size = Vector2(300, 44)
	start_button.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	start_button.add_theme_font_size_override("font_size", 16)
	start_button.add_theme_font_override("font", title_font)
	_style_button(start_button, Color(0.55, 0.34, 0.08, 0.92), GOLD, Color(0.68, 0.43, 0.12, 0.95))
	start_button.add_theme_color_override("font_color", Color(1.0, 0.96, 0.88))
	start_button.pressed.connect(_on_start)
	col.add_child(start_button)
	return col


func _build_slots() -> Control:
	var col := VBoxContainer.new()
	col.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	col.add_theme_constant_override("separation", 8)
	var spacer := Control.new()
	spacer.custom_minimum_size = Vector2(0, 66)
	col.add_child(spacer)
	col.add_child(_section("Save slots"))
	slots_box = VBoxContainer.new()
	slots_box.add_theme_constant_override("separation", 8)
	col.add_child(slots_box)
	return col


## Re-read the three slot summaries and redraw the cards.
func refresh_slots() -> void:
	for slot in [1, 2, 3]:
		var parsed: Variant = JSON.parse_string(session.save_summary_json(save_dir, slot))
		summaries[slot] = parsed if parsed is Dictionary else {"status": "empty"}
	for c in slots_box.get_children():
		c.queue_free()
	for slot in [1, 2, 3]:
		slots_box.add_child(_slot_card(slot))
	_update_start_label()


func _slot_card(slot: int) -> Control:
	var sum: Dictionary = summaries[slot]
	var status := str(sum.get("status", "empty"))
	var active := slot == active_slot
	var card := Button.new() # the whole card selects the slot
	card.custom_minimum_size = Vector2(330, 84)
	card.focus_mode = Control.FOCUS_NONE
	_style_button(card, CARD, GOLD if active else Color(1, 1, 1, 0.12), Color(0.10, 0.11, 0.15, 0.9))
	card.pressed.connect(_select_slot.bind(slot))

	var m := MarginContainer.new()
	m.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	m.mouse_filter = Control.MOUSE_FILTER_IGNORE
	for side in ["left", "right", "top", "bottom"]:
		m.add_theme_constant_override("margin_" + side, 10)
	card.add_child(m)
	var box := VBoxContainer.new()
	box.mouse_filter = Control.MOUSE_FILTER_IGNORE
	box.add_theme_constant_override("separation", 4)
	m.add_child(box)

	var top := HBoxContainer.new()
	top.mouse_filter = Control.MOUSE_FILTER_IGNORE
	box.add_child(top)
	var name_l := _label("Slot %d" % slot, 15, GOLD_PALE if active else INK)
	name_l.add_theme_font_override("font", title_font)
	top.add_child(name_l)
	var state := ""
	var state_col := MUTED
	match status:
		"ok":
			state = "Adventure in progress"
			state_col = GOLD
		"empty":
			state = "Empty slot"
		"dead":
			state = "☠ Fallen"
			state_col = DANGER
		_:
			state = "Save corrupted" if status == "corrupt" else "Newer save format"
			state_col = DANGER
	var state_l := _label("   " + state, 12, state_col)
	state_l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	top.add_child(state_l)

	var detail := ""
	if status == "ok" or status == "dead":
		detail = "Lv %d · %s · %s · %d slain · %s" % [
			int(sum.get("level", 1)), _class_name(str(sum.get("starterClass", ""))),
			str(sum.get("zone", "?")), int(sum.get("kills", 0)), str(sum.get("difficulty", "Standard"))]
		var cyc := int(sum.get("ngPlusCycle", 0))
		if cyc > 0:
			detail += " · Cycle %d" % cyc
	elif sum.has("message") and sum["message"] != null:
		detail = str(sum["message"])
	if detail != "":
		var d := _label(detail, 12, INK.darkened(0.1))
		d.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		d.custom_minimum_size = Vector2(300, 0)
		box.add_child(d)

	if status != "empty":
		var btns := HBoxContainer.new()
		btns.add_theme_constant_override("separation", 8)
		box.add_child(btns)
		if status == "ok":
			var cont := Button.new()
			cont.text = "Continue"
			cont.add_theme_font_size_override("font_size", 13)
			_style_button(cont, Color(0.45, 0.30, 0.10, 0.9), GOLD.darkened(0.2), Color(0.58, 0.38, 0.12, 0.95))
			cont.pressed.connect(_on_continue.bind(slot))
			btns.add_child(cont)
		var del := Button.new()
		del.text = "Delete — click again to confirm" if confirm_delete == slot else "Delete"
		del.add_theme_font_size_override("font_size", 13)
		_style_button(del, Color(0.25, 0.08, 0.08, 0.85) if confirm_delete == slot else Color(0.12, 0.12, 0.15, 0.85), Color(DANGER, 0.6), Color(0.35, 0.1, 0.1, 0.9))
		del.pressed.connect(_on_delete.bind(slot))
		btns.add_child(del)
		card.custom_minimum_size.y = 112
	return card


func _select_slot(slot: int) -> void:
	ui_click.emit()
	active_slot = slot
	confirm_replace = false
	confirm_delete = 0
	refresh_slots()


func _select_class(id: String) -> void:
	selected_class = id
	for i in CLASSES.size():
		_set_choice(class_buttons[i], CLASSES[i]["id"] == id)
		if CLASSES[i]["id"] == id:
			class_lore.text = CLASSES[i]["lore"]
			class_stats.text = CLASSES[i]["stats"]


func _select_difficulty(id: String) -> void:
	selected_difficulty = id
	for i in DIFFICULTIES.size():
		_set_choice(diff_buttons[i], DIFFICULTIES[i]["id"] == id)
		if DIFFICULTIES[i]["id"] == id:
			diff_blurb.text = DIFFICULTIES[i]["blurb"]


func _update_start_label() -> void:
	var occupied := str(summaries.get(active_slot, {}).get("status", "empty")) != "empty"
	if confirm_replace:
		start_button.text = "Replace Slot %d? Click again" % active_slot
	elif occupied:
		start_button.text = "New Adventure in Slot %d" % active_slot
	else:
		start_button.text = "Start Adventure (Slot %d)" % active_slot


## A stray click must never destroy a run: starting over an occupied
## slot takes a second, confirming click (as in the web version).
func _on_start() -> void:
	ui_click.emit()
	var occupied := str(summaries.get(active_slot, {}).get("status", "empty")) != "empty"
	if occupied and not confirm_replace:
		confirm_replace = true
		_update_start_label()
		return
	confirm_replace = false
	start_new.emit(active_slot, selected_class, selected_difficulty)


func _on_continue(slot: int) -> void:
	ui_click.emit()
	active_slot = slot
	continue_slot.emit(slot)


func _on_delete(slot: int) -> void:
	ui_click.emit()
	if confirm_delete != slot:
		confirm_delete = slot
		refresh_slots()
		return
	confirm_delete = 0
	session.delete_slot(save_dir, slot)
	refresh_slots()


## Shown on the slot card when its save can't be loaded after all.
func show_slot_error(slot: int, message: String) -> void:
	summaries[slot] = {"status": "corrupt", "message": message}
	for c in slots_box.get_children():
		c.queue_free()
	for s in [1, 2, 3]:
		slots_box.add_child(_slot_card(s))


func _class_name(raw: String) -> String:
	for c in CLASSES:
		if raw.to_lower().contains(str(c["id"]).to_lower()):
			return str(c["id"])
	return raw if raw != "" else "?"


# --- small UI helpers -------------------------------------------------------

func _label(text: String, size: int, color: Color) -> Label:
	var l := Label.new()
	l.text = text
	l.add_theme_font_size_override("font_size", size)
	l.add_theme_color_override("font_color", color)
	l.mouse_filter = Control.MOUSE_FILTER_IGNORE
	return l


func _section(text: String) -> Label:
	var l := _label(text.to_upper(), 11, GOLD.darkened(0.05))
	l.add_theme_constant_override("line_spacing", 2)
	return l


func _choice_button(text: String) -> Button:
	var b := Button.new()
	b.text = text
	b.alignment = HORIZONTAL_ALIGNMENT_LEFT
	b.add_theme_font_size_override("font_size", 13)
	b.focus_mode = Control.FOCUS_NONE
	return b


func _set_choice(b: Button, on: bool) -> void:
	if on:
		_style_button(b, Color(0.45, 0.30, 0.08, 0.35), Color(GOLD, 0.8), Color(0.5, 0.33, 0.1, 0.45))
		b.add_theme_color_override("font_color", Color(1.0, 0.96, 0.88))
	else:
		_style_button(b, Color(0.07, 0.08, 0.12, 0.8), Color(1, 1, 1, 0.12), Color(0.1, 0.11, 0.16, 0.85))
		b.add_theme_color_override("font_color", Color(0.80, 0.82, 0.86))


func _style_button(b: Button, bg: Color, border: Color, hover_bg: Color) -> void:
	for st in ["normal", "hover", "pressed", "focus"]:
		var sb := StyleBoxFlat.new()
		sb.bg_color = hover_bg if st == "hover" or st == "pressed" else bg
		if st == "focus":
			sb.bg_color = Color(0, 0, 0, 0)
		sb.border_color = border
		sb.set_border_width_all(1)
		sb.set_corner_radius_all(4)
		sb.content_margin_left = 12
		sb.content_margin_right = 12
		sb.content_margin_top = 6
		sb.content_margin_bottom = 6
		b.add_theme_stylebox_override(st, sb)


func _gradient(from: Vector2, to: Vector2, colors: Array) -> TextureRect:
	var g := Gradient.new()
	g.offsets = PackedFloat32Array([0.0, 0.5, 1.0])
	g.colors = PackedColorArray(colors)
	var gt := GradientTexture2D.new()
	gt.gradient = g
	gt.fill_from = from
	gt.fill_to = to
	gt.width = 64
	gt.height = 64
	var tr := TextureRect.new()
	tr.texture = gt
	tr.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	tr.stretch_mode = TextureRect.STRETCH_SCALE
	tr.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	tr.mouse_filter = Control.MOUSE_FILTER_IGNORE
	return tr
