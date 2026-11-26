class_name UiKit
extends RefCounted
## The storybook skin as plain helpers, so every screen is built the same way.

const INK := Color(0.96, 0.91, 0.80)
const GOLD := Color(0.95, 0.78, 0.42)
const BRASS := Color(0.72, 0.55, 0.28)
const MUTED := Color(0.74, 0.69, 0.60)
const GOOD := Color(0.55, 0.88, 0.55)
const BAD := Color(0.95, 0.48, 0.42)


## A centred modal: dim backdrop, panel, title row. Returns the body container.
static func modal(root: Control, theme: Theme, panel: PanelContainer, title: String, size: Vector2, on_close: Callable) -> VBoxContainer:
	root.theme = theme
	root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_STOP
	var dim := ColorRect.new()
	dim.color = Color(0.02, 0.015, 0.01, 0.62)
	dim.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	root.add_child(dim)
	var centre := CenterContainer.new()
	centre.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.add_child(centre)
	panel.custom_minimum_size = size
	centre.add_child(panel)
	var col := VBoxContainer.new()
	col.add_theme_constant_override("separation", 8)
	panel.add_child(col)
	var head := HBoxContainer.new()
	col.add_child(head)
	var t := label(title, 22, GOLD)
	t.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	head.add_child(t)
	var x := button("Close  Esc")
	x.pressed.connect(on_close)
	head.add_child(x)
	return col


static func label(text: String, size := 15, color := INK) -> Label:
	var l := Label.new()
	l.text = text
	l.add_theme_font_size_override("font_size", size)
	l.add_theme_color_override("font_color", color)
	return l


static func wrapped(text: String, size := 14, color := INK) -> Label:
	var l := label(text, size, color)
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	return l


static func button(text: String) -> Button:
	var b := Button.new()
	b.text = text
	b.add_theme_font_size_override("font_size", 14)
	var focus := StyleBoxFlat.new()
	focus.draw_center = false
	focus.border_color = GOLD
	focus.set_border_width_all(2)
	focus.set_corner_radius_all(5)
	b.add_theme_stylebox_override("focus", focus)
	return b


static func scroll() -> ScrollContainer:
	var s := ScrollContainer.new()
	s.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	s.size_flags_vertical = Control.SIZE_EXPAND_FILL
	s.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	s.follow_focus = true
	return s


static func column(sep := 4) -> VBoxContainer:
	var v := VBoxContainer.new()
	v.add_theme_constant_override("separation", sep)
	v.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	return v


static func clear(node: Node) -> void:
	for c: Node in node.get_children():
		node.remove_child(c)
		c.queue_free()


## First usable button under a node, for gamepad and keyboard focus.
static func focus_first(node: Node) -> bool:
	for c: Node in node.get_children():
		if c is Button and not (c as Button).disabled and (c as Button).is_visible_in_tree():
			(c as Button).grab_focus()
			return true
		if focus_first(c):
			return true
	return false


static func as_int(v: Variant) -> int:
	if v is float:
		var f: float = v
		return int(f)
	if v is int:
		return v
	return 0


static func as_num(v: Variant) -> float:
	if v is float:
		return v
	if v is int:
		var i: int = v
		return float(i)
	return 0.0


static func as_flag(v: Variant) -> bool:
	return v if v is bool else false


static func as_dict(v: Variant) -> Dictionary:
	return v if v is Dictionary else {}


static func as_list(v: Variant) -> Array:
	return v if v is Array else []


static func parse(text: String) -> Dictionary:
	return as_dict(JSON.parse_string(text))
