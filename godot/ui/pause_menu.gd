class_name PauseMenu
extends Control
## Pause, options and controls. The world is turn-based, so nothing needs freezing.

signal resumed
signal save_requested
signal title_requested
signal ending_requested

const CONTROLS := """Move  WASD / arrows, diagonals Q E Z C       Click a tile to walk there
Act / talk  F or Enter       Wait  Space       Rest  R       Auto-explore  X
Pack  I       Skills  K       Journal  J       Town  T       Drink a healing draught  H
Hotbar  1 to 5       Next target  Tab       Camera  V, [ ], mouse wheel
Gamepad:  stick or D-pad move,  A act,  B back,  X pack,  Y draught,  LB RB turn camera,  Start menu,  Back journal"""

var _ending: Button
var _note: Label
var _body: VBoxContainer


func setup(theme_in: Theme, panel: PanelContainer) -> void:
	var col := UiKit.modal(self, theme_in, panel, "Paused", Vector2(720, 520), _resume)
	_body = UiKit.column(8)
	col.add_child(_body)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 8)
	_body.add_child(row)
	for a: Array in [["Resume", _resume], ["Save now", _save], ["Save and quit to title", _title]]:
		var b := UiKit.button(str(a[0]))
		b.pressed.connect(a[1] as Callable)
		row.add_child(b)
	_ending = UiKit.button("Return to the ending")
	_ending.pressed.connect(func() -> void:
		hide()
		ending_requested.emit())
	row.add_child(_ending)
	_body.add_child(UiKit.label("Options", 18, UiKit.GOLD))
	_slider("Master volume", func() -> float: return GameSettings.master, func(v: float) -> void: GameSettings.master = v)
	_slider("Effects volume", func() -> float: return GameSettings.sfx, func(v: float) -> void: GameSettings.sfx = v)
	_slider("Interface volume", func() -> float: return GameSettings.ui, func(v: float) -> void: GameSettings.ui = v)
	_slider("Screen shake", func() -> float: return GameSettings.shake, func(v: float) -> void: GameSettings.shake = v)
	_check("Hit-stop on heavy blows", GameSettings.hit_stop, func(on: bool) -> void: GameSettings.hit_stop = on)
	_check("Hit flashes", GameSettings.flashes, func(on: bool) -> void: GameSettings.flashes = on)
	_check("Fullscreen", GameSettings.fullscreen, func(on: bool) -> void: GameSettings.fullscreen = on)
	_body.add_child(UiKit.label("Controls", 18, UiKit.GOLD))
	_body.add_child(UiKit.wrapped(CONTROLS, 13, UiKit.MUTED))
	_note = UiKit.label("", 14, UiKit.GOOD)
	_body.add_child(_note)
	hide()


func open(can_show_ending: bool) -> void:
	_note.text = ""
	_ending.visible = can_show_ending
	show()
	UiKit.focus_first(_body)


func note(text: String) -> void:
	_note.text = text


func _resume() -> void:
	GameSettings.save()
	hide()
	resumed.emit()


func _save() -> void:
	save_requested.emit()


func _title() -> void:
	GameSettings.save()
	hide()
	title_requested.emit()


func _slider(text: String, read: Callable, write: Callable) -> void:
	var row := HBoxContainer.new()
	var l := UiKit.label(text, 14)
	l.custom_minimum_size.x = 190
	row.add_child(l)
	var s := HSlider.new()
	s.max_value = 1.0
	s.step = 0.05
	s.value = UiKit.as_num(read.call())
	s.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	s.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	s.value_changed.connect(func(v: float) -> void:
		write.call(v)
		GameSettings.apply())
	row.add_child(s)
	_body.add_child(row)


func _check(text: String, on: bool, write: Callable) -> void:
	var c := CheckBox.new()
	c.text = text
	c.button_pressed = on
	c.toggled.connect(func(v: bool) -> void:
		write.call(v)
		GameSettings.apply())
	_body.add_child(c)
