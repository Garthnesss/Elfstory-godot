class_name GameSettings
extends RefCounted
## Player options: volume, accessibility switches, display. Saved as plain JSON
## in user://settings.json; a missing or damaged file just means defaults.

const PATH := "user://settings.json"
const BUSES: Array[String] = ["Master", "SFX", "UI"]

static var master := 1.0
static var sfx := 1.0
static var ui := 0.8
static var shake := 1.0
static var hit_stop := true
static var flashes := true
static var fullscreen := false
static var _loaded := false


static func load_once() -> void:
	if _loaded:
		return
	_loaded = true
	for bus: String in BUSES:
		if AudioServer.get_bus_index(bus) < 0:
			AudioServer.add_bus()
			AudioServer.set_bus_name(AudioServer.bus_count - 1, bus)
			AudioServer.set_bus_send(AudioServer.bus_count - 1, "Master")
	var json := JSON.new()
	if FileAccess.file_exists(PATH) and json.parse(FileAccess.get_file_as_string(PATH)) == OK and json.data is Dictionary:
		var d: Dictionary = json.data
		master = _num(d, "master", master)
		sfx = _num(d, "sfx", sfx)
		ui = _num(d, "ui", ui)
		shake = _num(d, "shake", shake)
		hit_stop = _flag(d, "hit_stop", hit_stop)
		flashes = _flag(d, "flashes", flashes)
		fullscreen = _flag(d, "fullscreen", fullscreen)
	apply()


static func apply() -> void:
	_volume("Master", master)
	_volume("SFX", sfx)
	_volume("UI", ui)
	if DisplayServer.get_name() != "headless":
		var want := DisplayServer.WINDOW_MODE_FULLSCREEN if fullscreen else DisplayServer.WINDOW_MODE_WINDOWED
		var now := DisplayServer.window_get_mode()
		var is_full := now == DisplayServer.WINDOW_MODE_FULLSCREEN or now == DisplayServer.WINDOW_MODE_EXCLUSIVE_FULLSCREEN
		if fullscreen != is_full:
			DisplayServer.window_set_mode(want)


static func save() -> void:
	var f := FileAccess.open(PATH + ".tmp", FileAccess.WRITE)
	if f == null:
		return
	f.store_string(JSON.stringify({
		"master": master, "sfx": sfx, "ui": ui, "shake": shake,
		"hit_stop": hit_stop, "flashes": flashes, "fullscreen": fullscreen,
	}))
	f.close()
	DirAccess.rename_absolute(PATH + ".tmp", PATH)


static func _volume(bus: String, level: float) -> void:
	var i := AudioServer.get_bus_index(bus)
	if i >= 0:
		AudioServer.set_bus_volume_db(i, linear_to_db(maxf(level, 0.0001)))
		AudioServer.set_bus_mute(i, level <= 0.0)


static func _num(d: Dictionary, key: String, fallback: float) -> float:
	var v: Variant = d.get(key)
	if v is float:
		var f: float = v
		return clampf(f, 0.0, 1.0)
	return fallback


static func _flag(d: Dictionary, key: String, fallback: bool) -> bool:
	var v: Variant = d.get(key)
	return v if v is bool else fallback
