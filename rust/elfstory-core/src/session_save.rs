//! Save / continue for the live session: GameSession <-> save bundle.
//!
//! The bundle follows the TS `SavedBundle` field names save.rs documents
//! (`player`, `zones`, `currentZoneId`, `worldFlags`, `quests`,
//! `merchantStocks`, `logs`, `town`, `difficulty`, `ngPlusCycle`,
//! `updatedAtMs`), plus a `rust` object for state the web version kept in
//! React refs or never had: the RNG state (so a loaded game continues the
//! same seeded sequence), the item-id counter, extra dialogue flags, the
//! delve counters the return-tick guards read, and the tile revision.
//! Presentation-only state (armed skill, open prompt/dialogue, fx cues) is
//! never saved; a loaded game starts with none of them open.

use crate::difficulty::DifficultyTier;
use crate::loot::ItemIdGen;
use crate::rng::Rng;
use crate::save::{
    can_load_bundle, read_slot_state, write_slot_state, DirStorage, SlotReadResult,
};
use crate::session::GameSession;
use crate::types::*;
use serde_json::{json, Value};
use std::collections::HashMap;

fn difficulty_str(t: DifficultyTier) -> &'static str {
    match t {
        DifficultyTier::Story => "story",
        DifficultyTier::Standard => "standard",
        DifficultyTier::Hardened => "hardened",
    }
}

fn field<T: serde::de::DeserializeOwned>(b: &Value, key: &str) -> Result<T, String> {
    let v = b.get(key).cloned().ok_or_else(|| format!("save is missing `{key}`"))?;
    serde_json::from_value(v).map_err(|e| format!("save field `{key}` is unreadable: {e}"))
}

/// What the title screen shows for the save slot.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SaveSummary {
    pub status: &'static str, // "empty" | "ok" | "corrupt" | "future" | "dead"
    pub message: Option<String>,
    pub zone: Option<String>,
    pub level: Option<i64>,
    #[serde(rename = "updatedAtMs")]
    pub updated_at_ms: Option<i64>,
    #[serde(rename = "starterClass")]
    pub starter_class: Option<String>,
    /// Display label ("Story" / "Standard" / "Hardened").
    pub difficulty: Option<&'static str>,
    pub kills: Option<i64>,
    #[serde(rename = "ngPlusCycle")]
    pub ng_plus_cycle: Option<i64>,
}

impl GameSession {
    /// Snapshot the session as a save bundle (unstamped; save.rs stamps
    /// the version on write).
    pub fn to_bundle(&self, now_ms: i64) -> Value {
        let zones: HashMap<&String, &ZoneMap> = self.zones.iter().collect();
        json!({
            "player": self.player,
            "zones": zones,
            "currentZoneId": self.current,
            "worldFlags": self.flags,
            "quests": self.quests,
            "merchantStocks": self.merchant_stocks,
            "logs": self.log.iter().rev().take(40).rev().collect::<Vec<_>>(),
            "town": self.town,
            "difficulty": difficulty_str(self.tier),
            "ngPlusCycle": self.cycle,
            "updatedAtMs": now_ms,
            "rust": {
                "rngState": self.rng.state(),
                "itemIdCounter": self.id_gen.counter(),
                "extraFlags": self.extra_flags_ref(),
                "delveProgress": self.delve_counters().0,
                "killsThisDelve": self.delve_counters().1,
                "seq": self.seq_value(),
                "tileRev": self.tile_rev,
            },
        })
    }

    /// Rebuild a session from a bundle. Fails (with a reason) rather than
    /// guessing when a required piece is missing or unreadable.
    pub fn from_bundle(b: &Value) -> Result<Self, String> {
        let player: PlayerState = field(b, "player")?;
        let zones: HashMap<String, ZoneMap> = field(b, "zones")?;
        let current: ZoneId = field(b, "currentZoneId")?;
        if !zones.contains_key(&current.to_string_id()) {
            return Err(format!("save's current zone `{current}` is not in its zone list"));
        }
        let flags: WorldFlags = field(b, "worldFlags")?;
        let town = field(b, "town")?;
        let quests: HashMap<String, QuestProgress> = field(b, "quests").unwrap_or_default();
        let merchant_stocks: HashMap<String, Vec<Item>> = field(b, "merchantStocks").unwrap_or_default();
        let log: Vec<String> = field(b, "logs").unwrap_or_default();
        let r = b.get("rust").cloned().unwrap_or(Value::Null);
        let rng_state = r.get("rngState").and_then(|v| v.as_u64()).unwrap_or(0xE1F5_2026) as u32;
        let counter = r.get("itemIdCounter").and_then(|v| v.as_u64()).unwrap_or(100_000);
        let extra: HashMap<String, Value> = r
            .get("extraFlags")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let as_i32 = |k: &str| r.get(k).and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        let mut s = GameSession::new_game(&player.name, player.starter_class, rng_state);
        s.player = player;
        s.zones = zones;
        s.current = current;
        s.flags = flags;
        s.town = town;
        s.quests = quests;
        s.merchant_stocks = merchant_stocks;
        s.log = log;
        s.rng = Rng::new(rng_state);
        s.id_gen = ItemIdGen::with_counter(counter);
        s.tier = crate::save::bundle_difficulty(b);
        s.cycle = crate::save::bundle_cycle(b);
        s.tile_rev = r.get("tileRev").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        s.restore_internals(extra, as_i32("delveProgress"), as_i32("killsThisDelve"), as_i32("seq").max(1000) as u32);
        s.fx.clear();
        s.refresh_visibility();
        s.say("Welcome back to ElfStory.");
        Ok(s)
    }

    /// Write this session to `slot` under `dir` (one JSON file per slot).
    pub fn save_to_dir(&self, dir: &str, slot: SaveSlotId, now_ms: i64) -> bool {
        let mut storage = DirStorage::new(dir);
        write_slot_state(slot, &self.to_bundle(now_ms), Some(&mut storage))
    }

    /// Load `slot` from `dir`. Corrupt, future-version and fallen-hardcore
    /// slots are refused with the save.rs message.
    pub fn load_from_dir(dir: &str, slot: SaveSlotId) -> Result<Self, String> {
        let storage = DirStorage::new(dir);
        match read_slot_state(slot, Some(&storage)) {
            SlotReadResult::Ok(b) if can_load_bundle(&b) => Self::from_bundle(&b),
            SlotReadResult::Ok(b) => Err(crate::save::fallen_bundle_message(&b)),
            SlotReadResult::Empty => Err("No save in this slot.".into()),
            SlotReadResult::Corrupt { message } => Err(message),
            SlotReadResult::FutureVersion { message, .. } => Err(message),
        }
    }

    /// Remove `slot`'s save file under `dir` (title-screen Delete).
    /// Deleting an empty slot is a no-op.
    pub fn delete_slot(dir: &str, slot: SaveSlotId) {
        let mut storage = DirStorage::new(dir);
        crate::save::SlotStorage::remove_item(&mut storage, &crate::save::get_local_slot_key(slot));
    }

    /// Title-screen summary of a slot, without loading it.
    pub fn summarize_slot(dir: &str, slot: SaveSlotId) -> SaveSummary {
        let storage = DirStorage::new(dir);
        let empty = |status, message| SaveSummary {
            status,
            message,
            zone: None,
            level: None,
            updated_at_ms: None,
            starter_class: None,
            difficulty: None,
            kills: None,
            ng_plus_cycle: None,
        };
        match read_slot_state(slot, Some(&storage)) {
            SlotReadResult::Empty => empty("empty", None),
            SlotReadResult::Corrupt { message } => empty("corrupt", Some(message)),
            SlotReadResult::FutureVersion { message, .. } => empty("future", Some(message)),
            SlotReadResult::Ok(b) => {
                let zone_id = b.get("currentZoneId").and_then(|v| v.as_str()).unwrap_or("");
                let zone_name = b
                    .get("zones")
                    .and_then(|z| z.get(zone_id))
                    .and_then(|z| z.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or(zone_id)
                    .to_string();
                SaveSummary {
                    status: if can_load_bundle(&b) { "ok" } else { "dead" },
                    message: if can_load_bundle(&b) { None } else { Some(crate::save::fallen_bundle_message(&b)) },
                    zone: Some(zone_name),
                    level: b.get("player").and_then(|p| p.get("level")).and_then(|l| l.as_i64()),
                    updated_at_ms: b.get("updatedAtMs").and_then(|v| v.as_i64()),
                    starter_class: b
                        .get("player")
                        .and_then(|p| p.get("starterClass"))
                        .and_then(|c| c.as_str())
                        .map(str::to_string),
                    difficulty: Some(crate::difficulty::difficulty_info(crate::save::bundle_difficulty(&b)).0),
                    kills: b.get("player").and_then(|p| p.get("kills")).and_then(|k| k.as_i64()),
                    ng_plus_cycle: Some(crate::save::bundle_cycle(&b) as i64),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Title screen: difficulty chosen at New Game survives save/load and
    /// shows in the slot summary; the three slots are independent; Delete
    /// empties only its own slot.
    #[test]
    fn slots_are_independent_and_carry_difficulty() {
        let dir = scratch_dir("slots");
        let hard = GameSession::new_game_with_difficulty("Garth", StarterClass::Shadowcraft, 5, DifficultyTier::Hardened);
        assert_eq!(hard.tier, DifficultyTier::Hardened);
        let story = GameSession::new_game_with_difficulty("Garth", StarterClass::Runecaster, 6, DifficultyTier::Story);
        assert!(hard.save_to_dir(&dir, SaveSlotId::Slot2, 1000));
        assert!(story.save_to_dir(&dir, SaveSlotId::Slot3, 2000));

        assert_eq!(GameSession::summarize_slot(&dir, SaveSlotId::Slot1).status, "empty");
        let s2 = GameSession::summarize_slot(&dir, SaveSlotId::Slot2);
        assert_eq!(s2.status, "ok");
        assert_eq!(s2.difficulty, Some("Hardened"));
        assert_eq!(s2.level, Some(1));
        assert_eq!(s2.kills, Some(0));
        assert!(s2.starter_class.as_deref().unwrap_or("").contains("Shadowcraft"), "{:?}", s2.starter_class);
        assert_eq!(GameSession::summarize_slot(&dir, SaveSlotId::Slot3).difficulty, Some("Story"));

        let loaded = GameSession::load_from_dir(&dir, SaveSlotId::Slot2).expect("slot 2 loads");
        assert_eq!(loaded.tier, DifficultyTier::Hardened);
        assert_eq!(loaded.player.starter_class, StarterClass::Shadowcraft);

        GameSession::delete_slot(&dir, SaveSlotId::Slot2);
        assert_eq!(GameSession::summarize_slot(&dir, SaveSlotId::Slot2).status, "empty");
        assert_eq!(GameSession::summarize_slot(&dir, SaveSlotId::Slot3).status, "ok");
        GameSession::delete_slot(&dir, SaveSlotId::Slot1); // empty slot: no-op
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn scratch_dir(tag: &str) -> String {
        let d = std::env::temp_dir().join(format!("elfstory_save_test_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.to_string_lossy().into_owned()
    }

    /// Walk into the overworld, change state everywhere, save, load, and
    /// compare: the loaded game is the same game, and it continues the
    /// same seeded RNG sequence.
    #[test]
    fn save_and_continue_round_trips_the_whole_game() {
        let mut s = GameSession::new_game("Garth", StarterClass::Runecaster, 77);
        s.player.x = 1;
        s.player.y = 7;
        s.move_player(-1, 0); // Oakhaven west gate -> overworld
        assert_eq!(s.current, ZoneId::Overworld);
        s.player.gold = 1234;
        s.player.skills.insert("frost_nova".into(), 2);
        s.flags.waystones_attuned = Some(1);
        s.town.population += 3;
        s.quests.insert(
            "outrider_oath".into(),
            QuestProgress { id: "outrider_oath".into(), status: QuestStatus::Active, stage: 2, outcome_label: None, history: vec!["x".into()] },
        );
        s.zone_mut().tiles[2][15].used = Some(true);

        let dir = scratch_dir("roundtrip");
        assert!(s.save_to_dir(&dir, SaveSlotId::Slot1, 1_700_000_000_000));
        let mut l = GameSession::load_from_dir(&dir, SaveSlotId::Slot1).expect("loads");

        assert_eq!(l.player, s.player);
        assert_eq!(l.current, s.current);
        assert_eq!(l.flags, s.flags);
        assert_eq!(l.town, s.town);
        assert_eq!(l.quests, s.quests);
        assert_eq!(l.zones.len(), s.zones.len());
        assert_eq!(l.zone().tiles[2][15].used, Some(true));
        assert_eq!(l.zone().enemies, s.zone().enemies);
        // Same RNG continuation.
        assert_eq!(l.rng.next_u32(), s.rng.clone().next_u32());

        let sum = GameSession::summarize_slot(&dir, SaveSlotId::Slot1);
        assert_eq!(sum.status, "ok");
        assert_eq!(sum.level, Some(s.player.level as i64));
        assert_eq!(sum.zone.as_deref(), Some(s.zone().name.as_str()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_and_corrupt_slots_are_reported_not_loaded() {
        let dir = scratch_dir("corrupt");
        assert_eq!(GameSession::summarize_slot(&dir, SaveSlotId::Slot1).status, "empty");
        assert!(GameSession::load_from_dir(&dir, SaveSlotId::Slot1).is_err());
        std::fs::create_dir_all(&dir).unwrap();
        let key = crate::save::get_local_slot_key(SaveSlotId::Slot1);
        std::fs::write(std::path::Path::new(&dir).join(format!("{key}.json")), "{not json").unwrap();
        let sum = GameSession::summarize_slot(&dir, SaveSlotId::Slot1);
        assert_eq!(sum.status, "corrupt");
        assert!(GameSession::load_from_dir(&dir, SaveSlotId::Slot1).err().unwrap().contains("corrupted"));
        // Slot 2 is untouched by slot 1's corruption.
        assert_eq!(GameSession::summarize_slot(&dir, SaveSlotId::Slot2).status, "empty");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
