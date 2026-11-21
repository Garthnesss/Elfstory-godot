//! Save-slot storage, ported from `elfstoryreborn/src/game/saveSlots.ts`.
//!
//! Versioning + per-slot corruption isolation (TS Phase 4 design):
//! - Bundles carry `version: SAVE_VERSION` **inside** the JSON; the
//!   storage key scheme (`ashen_glyph_save_v2_slot_N`) is unchanged.
//! - Missing `version` => legacy bundle, loads normally.
//! - `version > SAVE_VERSION` => written by a newer build; refused
//!   gracefully, never an exception, never a white screen.
//! - Unparseable JSON / failed validation in ONE slot is reported for
//!   that slot only and never affects the other slots.
//!
//! Rust divergence: TS works on the typed `SavedBundle` interface and
//! browser `localStorage`. This port operates on `serde_json::Value`
//! bundles (the typed core types live in `types.rs`; the town state type
//! is owned by the parallel `town.rs` port) and abstracts storage behind
//! the `SlotStorage` trait so the logic is testable without a DOM.
//!
//! Exact JSON field paths used by this module (TS `SavedBundle` shape):
//! - `version`, `player`, `zones`, `currentZoneId`, `worldFlags`,
//!   `quests`, `merchantStocks`, `logs`, `updatedAtMs`, `difficulty`,
//!   `completedAtMs`, `playTimeMs`, `ngPlusCycle`, `hardcore`,
//!   `deadAtMs`, `deathNote`, `town`
//! - NG+ rule: `town.blacksmithComplete` (bool) gates preservation of
//!   `merchantStocks.blacksmith` (array of Item) across ascension.

use crate::difficulty::{normalize_cycle, normalize_difficulty, DifficultyTier};
use crate::types::SaveSlotId;
use serde_json::{Map, Value};

/// Current save-bundle format version. Bump when the bundle shape changes
/// in a way older builds cannot safely read.
pub const SAVE_VERSION: i64 = 1;

pub const LEGACY_SAVE_KEY: &str = "ashen_glyph_save_v1";
pub const SLOT_KEY_PREFIX: &str = "ashen_glyph_save_v2_";

pub const SAVE_SLOTS: [SaveSlotId; 3] = [SaveSlotId::Slot1, SaveSlotId::Slot2, SaveSlotId::Slot3];

pub fn get_local_slot_key(slot_id: SaveSlotId) -> String {
    format!("{SLOT_KEY_PREFIX}{}", slot_id.as_str())
}

/// Per-slot read outcome. A corrupt or future-version slot is a value,
/// not an error — callers render it and move on (TS `SlotReadResult`).
#[derive(Debug, Clone, PartialEq)]
pub enum SlotReadResult {
    Empty,
    Ok(Value),
    Corrupt { message: String },
    FutureVersion { version: i64, message: String },
}

impl SlotReadResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, SlotReadResult::Ok(_))
    }
    pub fn bundle(&self) -> Option<&Value> {
        match self {
            SlotReadResult::Ok(b) => Some(b),
            _ => None,
        }
    }
}

pub fn corrupt_slot_message() -> String {
    "Save data in this slot is corrupted and cannot be loaded. You can delete it and start fresh."
        .to_string()
}

pub fn future_version_message(version: i64) -> String {
    format!(
        "This save was written by a newer version of ElfStory (save format v{version}; this build reads up to v{SAVE_VERSION}) and cannot be loaded here."
    )
}

/// Parse + validate one slot's stored JSON. Never panics. A bundle is
/// loadable when it parses to an object carrying a `player` object;
/// deeper hydration stays with the caller (TS `parseStoredBundle`).
/// Validation order matches TS: shape/player first, version second.
pub fn parse_stored_bundle(raw: Option<&str>) -> SlotReadResult {
    let raw = match raw {
        None => return SlotReadResult::Empty,
        Some(s) if s.is_empty() => return SlotReadResult::Empty,
        Some(s) => s,
    };
    let parsed: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => {
            return SlotReadResult::Corrupt { message: corrupt_slot_message() };
        }
    };
    let obj = match parsed.as_object() {
        Some(o) => o,
        None => return SlotReadResult::Corrupt { message: corrupt_slot_message() },
    };
    match obj.get("player") {
        Some(p) if p.is_object() => {}
        _ => return SlotReadResult::Corrupt { message: corrupt_slot_message() },
    }
    if let Some(v) = obj.get("version").and_then(|v| v.as_i64()) {
        if v > SAVE_VERSION {
            return SlotReadResult::FutureVersion {
                version: v,
                message: future_version_message(v),
            };
        }
    }
    SlotReadResult::Ok(parsed)
}

/// Minimal storage abstraction (TS `StorageLike`): the Godot bridge can
/// back this with a file per slot; tests use `MemoryStorage`.
pub trait SlotStorage {
    fn get_item(&self, key: &str) -> Option<String>;
    fn set_item(&mut self, key: &str, value: String);
    fn remove_item(&mut self, key: &str);
}

/// In-memory `SlotStorage` for tests and headless runs.
#[derive(Debug, Default)]
pub struct MemoryStorage {
    map: std::collections::HashMap<String, String>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SlotStorage for MemoryStorage {
    fn get_item(&self, key: &str) -> Option<String> {
        self.map.get(key).cloned()
    }
    fn set_item(&mut self, key: &str, value: String) {
        self.map.insert(key.to_string(), value);
    }
    fn remove_item(&mut self, key: &str) {
        self.map.remove(key);
    }
}

/// Read one slot with full isolation: any failure is reported in the
/// result, never thrown, and never affects other slots.
pub fn read_slot_state(slot_id: SaveSlotId, storage: Option<&dyn SlotStorage>) -> SlotReadResult {
    let storage = match storage {
        Some(s) => s,
        None => return SlotReadResult::Empty,
    };
    let raw = storage.get_item(&get_local_slot_key(slot_id));
    parse_stored_bundle(raw.as_deref())
}

/// Read every slot independently — a garbage string in one slot leaves
/// the others fully readable.
pub fn read_all_slot_states(
    storage: Option<&dyn SlotStorage>,
) -> Vec<(SaveSlotId, SlotReadResult)> {
    SAVE_SLOTS
        .iter()
        .map(|slot| (*slot, read_slot_state(*slot, storage)))
        .collect()
}

/// Stamp a bundle with the current format version before persisting.
/// Returns a new Value; non-object bundles are returned unchanged
/// (they can never be produced by the game, and TS's spread would have
/// silently coerced them — refusing to invent a shape here).
pub fn stamp_bundle(bundle: &Value) -> Value {
    let mut next = bundle.clone();
    if let Some(obj) = next.as_object_mut() {
        obj.insert("version".to_string(), Value::from(SAVE_VERSION));
    }
    next
}

/// Serialize a bundle for storage, stamped with `SAVE_VERSION`.
pub fn serialize_bundle(bundle: &Value) -> String {
    serde_json::to_string(&stamp_bundle(bundle)).unwrap_or_default()
}

/// Write one slot (stamped). Returns false only when storage is absent.
pub fn write_slot_state(
    slot_id: SaveSlotId,
    bundle: &Value,
    storage: Option<&mut dyn SlotStorage>,
) -> bool {
    let storage = match storage {
        Some(s) => s,
        None => return false,
    };
    let key = get_local_slot_key(slot_id);
    storage.set_item(&key, serialize_bundle(bundle));
    true
}

/// Record run completion on a bundle. Idempotent: the FIRST completion
/// time is kept, so continuing into the Endless Abyss (and re-saving)
/// never moves the stamp.
pub fn with_completion_stamp(bundle: &Value, now_ms: i64) -> Value {
    if bundle.get("completedAtMs").map_or(false, |v| v.is_number()) {
        return bundle.clone();
    }
    let mut next = bundle.clone();
    if let Some(obj) = next.as_object_mut() {
        obj.insert("completedAtMs".to_string(), Value::from(now_ms));
    }
    next
}

/// The difficulty a bundle plays at — legacy bundles default to Standard.
pub fn bundle_difficulty(bundle: &Value) -> DifficultyTier {
    normalize_difficulty(bundle.get("difficulty").and_then(|v| v.as_str()))
}

/// The NG+ cycle a bundle is on — pre-P5 bundles default to cycle 0.
pub fn bundle_cycle(bundle: &Value) -> u32 {
    let raw = bundle
        .get("ngPlusCycle")
        .and_then(|v| v.as_f64())
        .unwrap_or(f64::NAN);
    normalize_cycle(raw)
}

/// Whether a bundle is a hardcore (permadeath) run. Default false.
pub fn bundle_hardcore(bundle: &Value) -> bool {
    bundle.get("hardcore").and_then(|v| v.as_bool()) == Some(true)
}

/// Whether a bundle is a fallen hardcore run (death stamp present).
pub fn is_bundle_dead(bundle: &Value) -> bool {
    bundle.get("deadAtMs").map_or(false, |v| v.is_number())
}

/// Load policy: everything loads except a fallen hardcore run — death
/// is final, so a dead bundle can be deleted but never resumed.
pub fn can_load_bundle(bundle: &Value) -> bool {
    !is_bundle_dead(bundle)
}

pub fn fallen_bundle_message(bundle: &Value) -> String {
    let note = bundle
        .get("deathNote")
        .and_then(|v| v.as_str())
        .map(|n| format!(" {n}."))
        .unwrap_or_default();
    format!(
        "This delver has fallen — in Hardcore, death is final.{note} The slot can be deleted, but the run cannot be resumed."
    )
}

/// Stamp a bundle with its final death (hardcore). Idempotent: the
/// FIRST death stamp wins.
pub fn with_death_stamp(bundle: &Value, now_ms: i64, death_note: Option<&str>) -> Value {
    if is_bundle_dead(bundle) {
        return bundle.clone();
    }
    let mut next = bundle.clone();
    if let Some(obj) = next.as_object_mut() {
        obj.insert("deadAtMs".to_string(), Value::from(now_ms));
        match death_note {
            Some(note) => {
                obj.insert("deathNote".to_string(), Value::String(note.to_string()));
            }
            None => {
                obj.remove("deathNote");
            }
        }
    }
    next
}

// ---------------------------------------------------------------- NG+

/// Field path: `town.blacksmithComplete`.
pub fn town_blacksmith_complete(bundle: &Value) -> bool {
    bundle
        .get("town")
        .and_then(|t| t.get("blacksmithComplete"))
        .and_then(|v| v.as_bool())
        == Some(true)
}

/// Whether ascension from `bundle` must preserve the rebuilt
/// Blacksmith's merchant stock: true exactly when the town state at
/// `town.blacksmithComplete` is `true` and a stock array exists at
/// `merchantStocks.blacksmith`.
pub fn ngplus_should_preserve_blacksmith_stock(bundle: &Value) -> bool {
    town_blacksmith_complete(bundle)
        && bundle
            .get("merchantStocks")
            .and_then(|s| s.get("blacksmith"))
            .map_or(false, |v| v.is_array())
}

/// Apply the NG+ stock-preservation rule across an ascension:
/// `previous` is the bundle being ascended from, `next` is the fresh
/// NG+ bundle under construction. When `previous.town.blacksmithComplete`
/// is true, the array at `previous.merchantStocks.blacksmith` is copied
/// verbatim to `next.merchantStocks.blacksmith` (creating the
/// `merchantStocks` object on `next` when absent). Otherwise `next` is
/// returned unchanged. All other merchant stocks reset, as in TS.
pub fn apply_ngplus_stock_preservation(previous: &Value, next: &Value) -> Value {
    if !ngplus_should_preserve_blacksmith_stock(previous) {
        return next.clone();
    }
    let stock = previous
        .get("merchantStocks")
        .and_then(|s| s.get("blacksmith"))
        .cloned()
        .unwrap_or(Value::Array(vec![]));
    let mut out = next.clone();
    let obj: &mut Map<String, Value> = match out.as_object_mut() {
        Some(o) => o,
        None => return out,
    };
    let stocks = obj
        .entry("merchantStocks".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !stocks.is_object() {
        *stocks = Value::Object(Map::new());
    }
    if let Some(stocks_obj) = stocks.as_object_mut() {
        stocks_obj.insert("blacksmith".to_string(), stock);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_bundle() -> Value {
        json!({
            "player": { "name": "Garth", "level": 3, "hp": 40 },
            "zones": { "oakhaven": { "id": "oakhaven" }, "dungeon_1": { "id": "dungeon_1" } },
            "currentZoneId": "oakhaven",
            "worldFlags": { "malakorDefeated": false },
            "quests": {},
            "merchantStocks": { "blacksmith": [ { "id": "itm_blade", "name": "Oakhaven-Forged Blade" } ] },
            "logs": [],
            "town": { "blacksmithComplete": true, "blacksmithLot": "pondside", "population": 22 }
        })
    }

    #[test]
    fn save_round_trip_preserves_bundle_and_stamps_version() {
        let bundle = sample_bundle();
        let raw = serialize_bundle(&bundle);
        assert!(raw.contains("\"version\":1"));
        match parse_stored_bundle(Some(&raw)) {
            SlotReadResult::Ok(parsed) => {
                assert_eq!(parsed["player"]["name"], json!("Garth"));
                assert_eq!(parsed["currentZoneId"], json!("oakhaven"));
                assert_eq!(parsed["town"]["blacksmithLot"], json!("pondside"));
                assert_eq!(parsed["version"], json!(SAVE_VERSION));
            }
            other => panic!("expected Ok, got {other:?}"),
        }
        // Legacy bundle (no version field) still loads.
        let legacy = r#"{"player":{"name":"Garth"}}"#;
        assert!(matches!(parse_stored_bundle(Some(legacy)), SlotReadResult::Ok(_)));
        assert_eq!(bundle_difficulty(&sample_bundle()), DifficultyTier::Standard);
        assert_eq!(bundle_cycle(&sample_bundle()), 0);
    }

    #[test]
    fn corrupt_slot_is_isolated_from_other_slots() {
        let mut storage = MemoryStorage::new();
        let good = serialize_bundle(&sample_bundle());
        storage.set_item(&get_local_slot_key(SaveSlotId::Slot1), good.clone());
        storage.set_item(&get_local_slot_key(SaveSlotId::Slot2), "not json {{{".to_string());
        storage.set_item(&get_local_slot_key(SaveSlotId::Slot3), good);
        // Slot 3 also covers the "valid JSON, missing player" corruption.
        storage.set_item(
            &get_local_slot_key(SaveSlotId::Slot3),
            r#"{"zones":{}}"#.to_string(),
        );

        let results = read_all_slot_states(Some(&storage));
        assert_eq!(results.len(), 3);
        assert!(matches!(results[0].1, SlotReadResult::Ok(_)), "slot_1 must stay readable");
        assert!(
            matches!(results[1].1, SlotReadResult::Corrupt { .. }),
            "slot_2 garbage is corrupt"
        );
        assert!(
            matches!(results[2].1, SlotReadResult::Corrupt { .. }),
            "slot_3 missing player is corrupt"
        );

        // An untouched slot is Empty, and no storage at all is Empty.
        let empty_storage = MemoryStorage::new();
        assert_eq!(
            read_slot_state(SaveSlotId::Slot1, Some(&empty_storage)),
            SlotReadResult::Empty
        );
        assert_eq!(read_slot_state(SaveSlotId::Slot1, None), SlotReadResult::Empty);
        assert_eq!(parse_stored_bundle(None), SlotReadResult::Empty);
        assert_eq!(parse_stored_bundle(Some("")), SlotReadResult::Empty);
    }

    #[test]
    fn future_version_is_refused_with_message() {
        let raw = r#"{"player":{"name":"Garth"},"version":99}"#;
        match parse_stored_bundle(Some(raw)) {
            SlotReadResult::FutureVersion { version, message } => {
                assert_eq!(version, 99);
                assert!(message.contains("v99"), "message names the version: {message}");
                assert!(message.contains("v1"), "message names the readable cap: {message}");
            }
            other => panic!("expected FutureVersion, got {other:?}"),
        }
        // The current version itself is fine.
        let raw_now = r#"{"player":{"name":"Garth"},"version":1}"#;
        assert!(matches!(parse_stored_bundle(Some(raw_now)), SlotReadResult::Ok(_)));
    }

    #[test]
    fn ngplus_preserves_blacksmith_stock() {
        let previous = sample_bundle(); // town.blacksmithComplete == true
        let mut next = sample_bundle();
        next["merchantStocks"] = json!({}); // fresh NG+ bundle: stocks reset
        next["town"] = json!({ "blacksmithComplete": true, "population": 22 });

        let ascended = apply_ngplus_stock_preservation(&previous, &next);
        assert_eq!(
            ascended["merchantStocks"]["blacksmith"],
            json!([{ "id": "itm_blade", "name": "Oakhaven-Forged Blade" }]),
            "stock at merchantStocks.blacksmith survives ascension"
        );

        // Incomplete Blacksmith: nothing is preserved.
        let mut prev_incomplete = sample_bundle();
        prev_incomplete["town"]["blacksmithComplete"] = json!(false);
        let not_preserved = apply_ngplus_stock_preservation(&prev_incomplete, &next);
        assert_eq!(not_preserved["merchantStocks"], json!({}));

        // Missing merchantStocks object on the next bundle is created.
        let mut bare_next = sample_bundle();
        bare_next.as_object_mut().unwrap().remove("merchantStocks");
        let created = apply_ngplus_stock_preservation(&previous, &bare_next);
        assert!(created["merchantStocks"]["blacksmith"].is_array());
    }

    #[test]
    fn completion_and_death_stamps_are_first_write_wins() {
        let b = sample_bundle();
        let stamped = with_completion_stamp(&b, 1000);
        let restamped = with_completion_stamp(&stamped, 2000);
        assert_eq!(restamped["completedAtMs"], json!(1000));

        assert!(can_load_bundle(&b));
        let dead = with_death_stamp(&b, 500, Some("Fell on Floor 3"));
        assert!(is_bundle_dead(&dead));
        assert!(!can_load_bundle(&dead));
        assert!(fallen_bundle_message(&dead).contains("Fell on Floor 3"));
        let redead = with_death_stamp(&dead, 900, Some("Fell elsewhere"));
        assert_eq!(redead["deadAtMs"], json!(500));
    }
}

/// File-backed `SlotStorage`: one `<key>.json` per slot in `dir`. Writes go
/// to a temp file then rename, so a crash mid-save never leaves a torn
/// slot (the previous save survives).
#[derive(Debug, Clone)]
pub struct DirStorage {
    dir: std::path::PathBuf,
}

impl DirStorage {
    pub fn new(dir: impl Into<std::path::PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
    fn path(&self, key: &str) -> std::path::PathBuf {
        self.dir.join(format!("{key}.json"))
    }
}

impl SlotStorage for DirStorage {
    fn get_item(&self, key: &str) -> Option<String> {
        std::fs::read_to_string(self.path(key)).ok()
    }
    fn set_item(&mut self, key: &str, value: String) {
        let _ = std::fs::create_dir_all(&self.dir);
        let tmp = self.dir.join(format!("{key}.json.tmp"));
        if std::fs::write(&tmp, value).is_ok() {
            let _ = std::fs::rename(&tmp, self.path(key));
        }
    }
    fn remove_item(&mut self, key: &str) {
        let _ = std::fs::remove_file(self.path(key));
    }
}

