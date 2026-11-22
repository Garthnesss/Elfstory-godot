//! Thin Godot 4 GDExtension bridge (godot crate 0.5, api-4-7 for Godot 4.7.2).
//! Exposes `ElfStorySession` to GDScript. The bridge is deliberately
//! JSON-shaped: the Rust core owns all state and rules; Godot renders the
//! snapshot from `state_json()` and sends back player intents.

use elfstory_core::session::{GameSession, SessionEvent};
use elfstory_core::town::BlacksmithLot;
use elfstory_core::types::{SaveSlotId, StarterClass};
use godot::prelude::*;

struct ElfStoryExtension;

#[gdextension]
unsafe impl ExtensionLibrary for ElfStoryExtension {}

/// Title-screen slot number (1-3) to a core slot id; out of range is slot 1.
fn slot_id(n: i32) -> SaveSlotId {
    match n {
        2 => SaveSlotId::Slot2,
        3 => SaveSlotId::Slot3,
        _ => SaveSlotId::Slot1,
    }
}

fn parse_class(s: &str) -> StarterClass {
    match s {
        "Shadowcraft" => StarterClass::Shadowcraft,
        "Runecaster" => StarterClass::Runecaster,
        "Wanderer" => StarterClass::Wanderer,
        _ => StarterClass::Vanguard,
    }
}

fn parse_lot(s: &str) -> BlacksmithLot {
    match s {
        "market" => BlacksmithLot::Market,
        "west_gate" => BlacksmithLot::WestGate,
        _ => BlacksmithLot::Pondside,
    }
}

fn event_name(ev: SessionEvent) -> String {
    match ev {
        SessionEvent::None => "none".into(),
        SessionEvent::OpenDialogue(id) => format!("dialogue:{id}"),
        SessionEvent::OpenMerchant(id) => format!("merchant:{id}"),
        SessionEvent::ZoneChanged(z) => format!("zone_changed:{}", z.to_string_id()),
        SessionEvent::PlayerDied => "player_died".into(),
        SessionEvent::LevelUp(l) => format!("level_up:{l}"),
        SessionEvent::OpenPrompt(kind) => format!("prompt:{kind}"),
    }
}

#[derive(GodotClass)]
#[class(base = RefCounted)]
struct ElfStorySession {
    base: Base<RefCounted>,
    inner: Option<GameSession>,
}

#[godot_api]
impl IRefCounted for ElfStorySession {
    fn init(base: Base<RefCounted>) -> Self {
        Self { base, inner: None }
    }
}

#[godot_api]
impl ElfStorySession {
    #[func]
    fn new_game(&mut self, name: GString, starter_class: GString, seed: i64) {
        self.inner = Some(GameSession::new_game(
            &name.to_string(),
            parse_class(&starter_class.to_string()),
            seed as u32,
        ));
    }

    /// New game with a difficulty id ("story" / "standard" / "hardened";
    /// anything else is Standard).
    #[func]
    fn new_game_with_difficulty(&mut self, name: GString, starter_class: GString, seed: i64, difficulty: GString) {
        let tier = elfstory_core::difficulty::normalize_difficulty(Some(&difficulty.to_string().to_lowercase()));
        self.inner = Some(GameSession::new_game_with_difficulty(
            &name.to_string(),
            parse_class(&starter_class.to_string()),
            seed as u32,
            tier,
        ));
    }

    #[func]
    fn has_game(&self) -> bool {
        self.inner.is_some()
    }

    #[func]
    fn state_json(&self) -> GString {
        match &self.inner {
            Some(s) => GString::from(&s.state_json().to_string()),
            None => GString::from("{}"),
        }
    }

    #[func]
    fn move_player(&mut self, dx: i32, dy: i32) -> GString {
        match &mut self.inner {
            Some(s) => GString::from(&event_name(s.move_player(dx, dy))),
            None => GString::from("none"),
        }
    }

    #[func]
    fn interact(&mut self) -> GString {
        match &mut self.inner {
            Some(s) => GString::from(&event_name(s.interact())),
            None => GString::from("none"),
        }
    }

    #[func]
    fn complete_blacksmith(&mut self, lot: GString) -> bool {
        match &mut self.inner {
            Some(s) => s.complete_blacksmith(parse_lot(&lot.to_string())),
            None => false,
        }
    }

    #[func]
    fn relocate_blacksmith(&mut self, lot: GString) -> bool {
        match &mut self.inner {
            Some(s) => s.relocate_blacksmith(parse_lot(&lot.to_string())),
            None => false,
        }
    }

    #[func]
    fn merchant_stock_json(&mut self, merchant_id: GString) -> GString {
        match &mut self.inner {
            Some(s) => {
                let stock = s.merchant_stock(&merchant_id.to_string());
                GString::from(&serde_json::to_string(&stock).unwrap_or_else(|_| "[]".into()))
            }
            None => GString::from("[]"),
        }
    }

    #[func]
    fn buy_item(&mut self, merchant_id: GString, index: i32) -> bool {
        match &mut self.inner {
            Some(s) => s
                .buy_item(&merchant_id.to_string(), index.max(0) as usize)
                .is_ok(),
            None => false,
        }
    }

    /// Current dialogue node (condition-filtered choices) as JSON, or
    /// "null" when no conversation is open.
    #[func]
    fn dialogue_json(&self) -> GString {
        match &self.inner {
            Some(s) => GString::from(&s.dialogue_json().to_string()),
            None => GString::from("null"),
        }
    }

    #[func]
    fn choose_dialogue(&mut self, choice_id: GString) -> GString {
        match &mut self.inner {
            Some(s) => GString::from(&event_name(s.choose_dialogue(&choice_id.to_string()))),
            None => GString::from("none"),
        }
    }

    #[func]
    fn close_dialogue(&mut self) {
        if let Some(s) = &mut self.inner {
            s.close_dialogue();
        }
    }

    /// Open world-tile prompt (vault door, seal door, waystone) as JSON:
    /// {kind, x, y, title, subtitle, body, options:[{id,label,tone,disabled}]},
    /// or "null" when none is open.
    #[func]
    fn prompt_json(&self) -> GString {
        match &self.inner {
            Some(s) => GString::from(&s.prompt_json().to_string()),
            None => GString::from("null"),
        }
    }

    #[func]
    fn choose_prompt(&mut self, option_id: GString) -> GString {
        match &mut self.inner {
            Some(s) => GString::from(&event_name(s.choose_prompt(&option_id.to_string()))),
            None => GString::from("none"),
        }
    }

    #[func]
    fn close_prompt(&mut self) {
        if let Some(s) = &mut self.inner {
            s.close_prompt();
        }
    }

    // ---- save / continue (session_save.rs), slots 1-3 ----

    /// Save the current game to `slot` (1-3) under `dir` (absolute path).
    #[func]
    fn save_game(&mut self, dir: GString, slot: i32) -> bool {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        match &self.inner {
            Some(s) => s.save_to_dir(&dir.to_string(), slot_id(slot), now_ms),
            None => false,
        }
    }

    /// Load `slot` (1-3) from `dir`, replacing the current game. Returns
    /// "ok" or the reason it can't load (the current game is kept on failure).
    #[func]
    fn load_game(&mut self, dir: GString, slot: i32) -> GString {
        match GameSession::load_from_dir(&dir.to_string(), slot_id(slot)) {
            Ok(s) => {
                self.inner = Some(s);
                GString::from("ok")
            }
            Err(e) => GString::from(&e),
        }
    }

    /// {status: empty|ok|corrupt|future|dead, message, zone, level,
    /// updatedAtMs, starterClass, difficulty, kills, ngPlusCycle}
    #[func]
    fn save_summary_json(&self, dir: GString, slot: i32) -> GString {
        let sum = GameSession::summarize_slot(&dir.to_string(), slot_id(slot));
        GString::from(&serde_json::to_string(&sum).unwrap_or_else(|_| "null".into()))
    }

    /// Delete `slot`'s save file (title-screen Delete, after confirmation).
    #[func]
    fn delete_slot(&mut self, dir: GString, slot: i32) {
        GameSession::delete_slot(&dir.to_string(), slot_id(slot));
    }

    // ---- skills / combat (combat_turn.rs) ----

    /// Fire a hotbar slot (0-4): self skills cast at once; targeted skills
    /// arm/disarm the aim.
    #[func]
    fn trigger_hotbar(&mut self, slot: i32) -> GString {
        match &mut self.inner {
            Some(s) if slot >= 0 => GString::from(&event_name(s.trigger_hotbar(slot as usize))),
            _ => GString::from("none"),
        }
    }

    #[func]
    fn confirm_target(&mut self) -> GString {
        match &mut self.inner {
            Some(s) => GString::from(&event_name(s.confirm_target())),
            None => GString::from("none"),
        }
    }

    #[func]
    fn cycle_target(&mut self) {
        if let Some(s) = &mut self.inner {
            s.cycle_target();
        }
    }

    #[func]
    fn move_aim(&mut self, dx: i32, dy: i32) {
        if let Some(s) = &mut self.inner {
            s.move_aim(dx, dy);
        }
    }

    #[func]
    fn cancel_aim(&mut self) {
        if let Some(s) = &mut self.inner {
            s.cancel_aim();
        }
    }

    #[func]
    fn wait_turn(&mut self) -> GString {
        match &mut self.inner {
            Some(s) => GString::from(&event_name(s.wait_turn())),
            None => GString::from("none"),
        }
    }

    /// {skillPoints, skills:[{id,name,glyph,branch,tier,kind,rank,summary,
    /// current,next,canLearn,reason,manaCost,cooldown}]}
    #[func]
    fn skills_json(&self) -> GString {
        match &self.inner {
            Some(s) => GString::from(&s.skills_json().to_string()),
            None => GString::from("null"),
        }
    }

    #[func]
    fn learn_skill(&mut self, skill_id: GString) -> bool {
        let Some(id) = elfstory_core::types::SkillId::parse(&skill_id.to_string()) else {
            return false;
        };
        match &mut self.inner {
            Some(s) => s.learn_skill(id),
            None => false,
        }
    }

    /// Presentation cues since the last call, as a JSON array of
    /// {kind, x, y, dx, dy}: hit, crit, kill, player_hit, windup, pickup,
    /// chest, brazier, waystone, unseal, level_up. Draining.
    #[func]
    fn take_fx_json(&mut self) -> GString {
        match &mut self.inner {
            Some(s) => GString::from(&serde_json::to_string(&s.take_fx()).unwrap_or_else(|_| "[]".into())),
            None => GString::from("[]"),
        }
    }

    /// Dev/capture only (debug builds): warp to a zone id ("dungeon_3",
    /// "overworld", ...) at (x, y); x < 0 uses the zone's entry tile.
    #[cfg(debug_assertions)]
    #[func]
    fn debug_warp(&mut self, zone_id: GString, x: i32, y: i32) -> GString {
        let Some(id) = elfstory_core::types::ZoneId::parse(&zone_id.to_string()) else {
            return GString::from("none");
        };
        match &mut self.inner {
            Some(s) => GString::from(&event_name(s.debug_warp(id, x, y))),
            None => GString::from("none"),
        }
    }

    /// Dev/capture only (debug builds): set town population and optionally
    /// complete the Blacksmith on `lot` ("pondside" / "market" / "west_gate").
    #[cfg(debug_assertions)]
    #[func]
    fn debug_set_town(&mut self, pop: i32, complete_lot: GString) {
        if let Some(s) = &mut self.inner {
            let lot = BlacksmithLot::parse(&complete_lot.to_string());
            s.debug_set_town(pop, lot);
        }
    }

    /// Model placements for the current town state (Oakhaven landmarks,
    /// blacksmith lot mount, ActRaiser tier growth) as JSON.
    #[func]
    fn placements_json(&self) -> GString {
        match &self.inner {
            Some(s) => {
                let placements =
                    elfstory_core::town_layout::get_oakhaven_model_placements(Some(&s.town));
                GString::from(&serde_json::to_string(&placements).unwrap_or_else(|_| "[]".into()))
            }
            None => GString::from("[]"),
        }
    }
}
