extends Node
## Combat feel for the tile view: attack lunges, hit-flash + squash, death
## pops, floating damage numbers, selective hit-stop and camera trauma.
##
## Presentation only — every number shown here comes from the snapshot or
## the core's fx cues; nothing here decides an outcome. All tweens are
## tracked so `snap()` can finish them the instant the player presses a
## key: the view never makes the player wait for an animation.

const LUNGE_PULL := 0.05
const LUNGE_STRIKE := 0.07
const LUNGE_RETURN := 0.12
const FLASH_TIME := 0.12
const SQUASH := Vector3(1.25, 0.75, 1.25)

var tweens: Array[Tween] = []
var trauma := 0.0 # 0..1; camera offset scales with trauma^2
var _shake_t := 0.0
var _stops := 0


## Body pivot of an actor (make_actor() in Main.gd); juice never touches the
## root, which Main lerps toward the actor's tile.
static func body_of(n: Node3D) -> Node3D:
	if n == null or not is_instance_valid(n):
		return null
	return n.get_node_or_null("Body") as Node3D


func _track(t: Tween) -> Tween:
	tweens.append(t)
	return t


func lunge(n: Node3D, dir: Vector3) -> void:
	var body := body_of(n)
	if body == null or dir.length() < 0.01:
		return
	var d := dir.normalized()
	# Body offsets are in the root's scaled space; undo it so bosses lunge
	# the same distance as everyone else.
	var s := maxf(n.scale.x, 0.01)
	var t := _track(body.create_tween())
	t.tween_property(body, "position", -d * 0.12 / s, LUNGE_PULL).set_ease(Tween.EASE_OUT)
	t.tween_property(body, "position", d * 0.38 / s, LUNGE_STRIKE).set_trans(Tween.TRANS_BACK).set_ease(Tween.EASE_OUT)
	t.tween_property(body, "position", Vector3.ZERO, LUNGE_RETURN).set_trans(Tween.TRANS_QUAD).set_ease(Tween.EASE_IN_OUT)


func flash(n: Node3D, color := Color.WHITE) -> void:
	var body := body_of(n)
	if body == null:
		return
	var mat: StandardMaterial3D = n.get_meta("mat", null)
	if mat != null:
		mat.emission = color
		mat.emission_energy_multiplier = 2.6
		var tf := _track(n.create_tween())
		tf.tween_property(mat, "emission_energy_multiplier", 0.0, FLASH_TIME)
	body.scale = SQUASH
	var ts := _track(body.create_tween())
	ts.tween_property(body, "scale", Vector3.ONE, 0.18).set_trans(Tween.TRANS_BACK).set_ease(Tween.EASE_OUT)


## Flatten, flash and free an actor that left the snapshot.
func die(n: Node3D) -> void:
	var body := body_of(n)
	if body == null:
		if is_instance_valid(n):
			n.queue_free()
		return
	flash(n, Color(1.0, 0.9, 0.7))
	var t := _track(n.create_tween())
	t.tween_interval(0.06)
	t.tween_property(body, "scale", Vector3(1.45, 0.06, 1.45), 0.2).set_trans(Tween.TRANS_BACK).set_ease(Tween.EASE_IN)
	t.tween_callback(n.queue_free)


## Arcing damage number at a world position.
func number(parent: Node3D, at: Vector3, text: String, color: Color, big := false) -> void:
	var l := Label3D.new()
	l.text = text
	l.font_size = 96 if big else 72
	l.pixel_size = 0.009
	l.billboard = BaseMaterial3D.BILLBOARD_ENABLED
	l.no_depth_test = true
	l.render_priority = 10
	l.modulate = color
	l.outline_size = 14
	l.outline_modulate = Color(0.06, 0.04, 0.05)
	l.position = at + Vector3(0.0, 1.35, 0.0)
	parent.add_child(l)
	var drift := randf_range(-0.35, 0.35)
	var t := _track(l.create_tween())
	t.set_parallel(true)
	t.tween_property(l, "position:x", l.position.x + drift, 0.7)
	t.tween_property(l, "position:y", l.position.y + 0.9, 0.32).set_trans(Tween.TRANS_QUAD).set_ease(Tween.EASE_OUT)
	t.tween_property(l, "position:y", l.position.y + 0.55, 0.38).set_trans(Tween.TRANS_QUAD).set_ease(Tween.EASE_IN).set_delay(0.32)
	t.tween_property(l, "scale", Vector3.ONE * (1.35 if big else 1.15), 0.08).from(Vector3.ONE * 0.6)
	t.tween_property(l, "modulate:a", 0.0, 0.25).set_delay(0.45)
	t.chain().tween_callback(l.queue_free)


## Brief slow-motion on crits, kills and hits taken — never on routine hits.
func hit_stop(duration: float) -> void:
	_stops += 1
	Engine.time_scale = 0.08
	get_tree().create_timer(duration, true, false, true).timeout.connect(func():
		_stops -= 1
		if _stops <= 0:
			_stops = 0
			Engine.time_scale = 1.0)


func add_trauma(amount: float) -> void:
	trauma = clampf(trauma + amount, 0.0, 1.0)


## Camera shake offset for this frame (call once per frame).
func shake_offset(delta: float) -> Vector2:
	trauma = maxf(0.0, trauma - delta * 1.8)
	_shake_t += delta
	var s := trauma * trauma * 0.4
	return Vector2(
		s * (sin(_shake_t * 47.0) * 0.6 + sin(_shake_t * 29.0 + 1.3) * 0.4),
		s * (sin(_shake_t * 53.0 + 2.1) * 0.6 + sin(_shake_t * 31.0) * 0.4))


## Finish every running juice tween now (input-buffered feel): the next
## action always starts from a settled board.
func snap() -> void:
	_stops = 0
	Engine.time_scale = 1.0
	for t in tweens:
		if t != null and t.is_valid() and t.is_running():
			t.custom_step(10.0)
	tweens = tweens.filter(func(t): return t != null and t.is_valid() and t.is_running())
