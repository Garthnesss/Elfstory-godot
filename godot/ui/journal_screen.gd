class_name JournalScreen
extends Control
## Quest journal: every quest, what to do next, and what happened so far.

signal closed

var session: ElfStorySession
var _body: VBoxContainer


func setup(p_session: ElfStorySession, theme_in: Theme, panel: PanelContainer) -> void:
	session = p_session
	var col := UiKit.modal(self, theme_in, panel, "Journal", Vector2(720, 560), _close)
	var s := UiKit.scroll()
	col.add_child(s)
	_body = UiKit.column(6)
	s.add_child(_body)
	hide()


func open() -> void:
	UiKit.clear(_body)
	var d := UiKit.parse(session.journal_json())
	var rows := UiKit.as_list(d.get("quests"))
	var done := 0
	for r_v: Variant in rows:
		if str(UiKit.as_dict(r_v).get("status")) == "completed":
			done += 1
	_body.add_child(UiKit.label("%d of %d quests finished" % [done, UiKit.as_int(d.get("total"))], 14, UiKit.MUTED))
	if rows.is_empty():
		_body.add_child(UiKit.wrapped("No quests yet. Warden Vance, by the breach, has work for you.", 15))
	for r_v: Variant in rows:
		var q := UiKit.as_dict(r_v)
		var finished := str(q["status"]) == "completed"
		_body.add_child(UiKit.label(("✓ " if finished else "• ") + str(q["title"]), 18, UiKit.GOOD if finished else UiKit.GOLD))
		if finished:
			if q.get("outcome") != null:
				_body.add_child(UiKit.label(str(q["outcome"]), 14, UiKit.MUTED))
		else:
			_body.add_child(UiKit.wrapped(str(q["summary"]), 14))
			if q.get("hint") != null:
				_body.add_child(UiKit.wrapped("Next: " + str(q["hint"]), 14, UiKit.GOLD))
		for h: Variant in UiKit.as_list(q.get("history")):
			_body.add_child(UiKit.wrapped("   " + str(h), 13, UiKit.MUTED))
	show()


func _close() -> void:
	hide()
	closed.emit()
