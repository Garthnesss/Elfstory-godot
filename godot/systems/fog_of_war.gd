class_name FogOfWar
extends RefCounted
## What the player has seen, as one small texture every world shader samples.
## Red: 0 never seen, 0.45 remembered, 1 in sight. One texel per tile, bilinear,
## so the edge of sight is soft. Texels ease toward their goal for a smooth reveal.

const REMEMBERED := 0.45
const REVEAL_PER_SECOND := 5.0

var texture: ImageTexture
var size := Vector2i.ONE

var _goal := PackedByteArray()
var _now := PackedByteArray()
var _image: Image
var _settled := true


## `rows` is the snapshot's zone.sight; `fresh` snaps (zone change) instead of easing.
## `least` is the dimmest a tile may be: outdoors the land itself is known.
func update(rows: Array, fresh: bool, least := 0.0) -> void:
	var floor_v := int(least * 255.0)
	if rows.is_empty():
		return
	var h := rows.size()
	var w := str(rows[0]).length()
	if fresh or Vector2i(w, h) != size or texture == null:
		size = Vector2i(w, h)
		_now = PackedByteArray()
		_now.resize(w * h)
		_goal = PackedByteArray()
		_goal.resize(w * h)
		fresh = true
	for y: int in h:
		var row := str(rows[y])
		for x: int in mini(w, row.length()):
			var c := row.unicode_at(x) - 48
			_goal[y * w + x] = 255 if c == 2 else (maxi(floor_v, int(REMEMBERED * 255.0)) if c == 1 else floor_v)
	if fresh:
		_now = _goal.duplicate()
		_image = Image.create_from_data(w, h, false, Image.FORMAT_R8, _now)
		texture = ImageTexture.create_from_image(_image)
		_settled = true
	else:
		_settled = _now == _goal


func tick(delta: float) -> void:
	if _settled or texture == null:
		return
	var step := maxi(1, int(REVEAL_PER_SECOND * 255.0 * delta))
	var moving := false
	for i: int in _now.size():
		var d := _goal[i] - _now[i]
		if d != 0:
			_now[i] += clampi(d, -step, step)
			moving = moving or _now[i] != _goal[i]
	_image.set_data(size.x, size.y, false, Image.FORMAT_R8, _now)
	texture.update(_image)
	_settled = not moving


func bind(mat: ShaderMaterial, enabled: bool) -> void:
	mat.set_shader_parameter("fog_on", 1.0 if enabled and texture != null else 0.0)
	if texture != null:
		mat.set_shader_parameter("fog_tex", texture)
		mat.set_shader_parameter("fog_exact", texture)
		mat.set_shader_parameter("fog_size", Vector2(size))
