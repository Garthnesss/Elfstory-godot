class_name EndingScreen
extends Control
## The epilogue: what your choices did, then carry on or begin New Game+.

signal carry_on
signal ascend

var session: ElfStorySession
var _body: VBoxContainer
var _buttons: HBoxContainer
var _ascend: Button
var _armed := false


func setup(p_session: ElfStorySession, theme_in: Theme, panel: PanelContainer) -> void:
	session = p_session
	var col := UiKit.modal(self, theme_in, panel, "The Cinder Throne Cools", Vector2(760, 560), _carry)
	var s := UiKit.scroll()
	col.add_child(s)
	_body = UiKit.column(12)
	s.add_child(_body)
	_buttons = HBoxContainer.new()
	_buttons.add_theme_constant_override("separation", 10)
	col.add_child(_buttons)
	var go := UiKit.button("Carry on into the Abyss")
	go.pressed.connect(_carry)
	_buttons.add_child(go)
	_ascend = UiKit.button("")
	_ascend.pressed.connect(_on_ascend)
	_buttons.add_child(_ascend)
	hide()


func open() -> void:
	_armed = false
	UiKit.clear(_body)
	var d := UiKit.parse(session.ending_json())
	for line: Variant in UiKit.as_list(d.get("lines")):
		_body.add_child(UiKit.wrapped(str(line), 17))
	_body.add_child(UiKit.label("Level %d   %d foes felled   %d turns   %d gold   %d quests finished" % [
		UiKit.as_int(d.get("level")), UiKit.as_int(d.get("kills")), UiKit.as_int(d.get("turns")),
		UiKit.as_int(d.get("gold")), UiKit.as_int(d.get("quests"))], 14, UiKit.MUTED))
	_ascend.text = "Begin Cycle %d (harder, keep your hero)" % (UiKit.as_int(d.get("cycle")) + 1)
	show()
	UiKit.focus_first(_buttons)


func _carry() -> void:
	hide()
	carry_on.emit()


func _on_ascend() -> void:
	if not _armed: # it replaces the world, so ask twice
		_armed = true
		_ascend.text = "The Vale will be remade. Press again to ascend."
		return
	hide()
	ascend.emit()
