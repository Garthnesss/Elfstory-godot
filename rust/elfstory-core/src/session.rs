//! Game session glue for the Milestone 2 vertical slice (Run 1).
//!
//! In the TS game this glue lives in App.tsx (unported). This module owns
//! the smallest honest turn loop that exercises the ported core end-to-end:
//! new game -> Oakhaven -> Floor 1 -> collect the 4 town items -> return
//! alive -> bank -> rebuild the Blacksmith -> buy the gated blade.
//!
//! Slice simplifications (vs the full App.tsx loop), all deliberate:
//! - Bump combat only; skills/cooldowns/status DoTs are M3.
//! - Enemy turns: an enemy with a telegraphed intent strikes for its intent
//!   damage; otherwise an alerted enemy steps toward the player and strikes
//!   when adjacent, using the TS formula max(1, atk - round(def * 0.65)).
//! - Merchant tables and dialogue trees are the M3 npcs.rs port; this
//!   module hosts the dialogue engine (conditions, checks, quest actions)
//!   mirroring App.tsx's handleSelectDialogueChoice.
//! - Wake in Oakhaven heals to full; the TS gold death toll is Normal-mode
//!   App logic (M3).


use crate::difficulty::DifficultyTier;
use crate::loot::{create_consumable, create_named_relic, create_starter_item, ItemIdGen};
use crate::rng::Rng;
use crate::skills::{compute_derived_stats, compute_respec_points, get_starter_preset};
use crate::town::{
    bank_carried_town_items, can_complete_blacksmith, complete_blacksmith, create_town_state,
    is_town_item_tag, relocate_blacksmith, split_unbanked_town_items, town_return_tick,
    BlacksmithLot, TownReturnOptions, TownState,
};
use crate::types::*;
#[cfg(test)]
use crate::town::BLACKSMITH_GATED_ITEM_NAME;
use crate::world::create_oakhaven_map;
use serde_json::{json, Value};
use std::collections::HashMap;

pub const RUN_SEED_DEFAULT: u32 = 0xE1F5_2026;

#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    None,
    OpenDialogue(String), // npc id
    OpenMerchant(String), // merchant id
    ZoneChanged(ZoneId),
    PlayerDied,
    LevelUp(i32),
    /// A world-tile prompt opened (vault / seal_door / waystone); read it
    /// with `prompt_json`.
    OpenPrompt(String),
}

/// Presentation cue for the front-end (VFX / sound): what just happened,
/// where. Not game state, never saved; drained by `take_fx` after each
/// action. `dx`/`dy` is a direction where it matters (attack swing).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FxEvent {
    pub kind: &'static str,
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
}

const FX_CAP: usize = 64;

pub struct GameSession {
    pub player: PlayerState,
    pub town: TownState,
    pub flags: WorldFlags,
    pub zones: HashMap<String, ZoneMap>,
    pub current: ZoneId,
    pub rng: Rng,
    pub id_gen: ItemIdGen,
    pub tier: DifficultyTier,
    pub cycle: u32,
    pub merchant_stocks: HashMap<String, Vec<Item>>,
    /// Quest journal (TS App `quests` state; lives in GameSave there).
    pub quests: HashMap<String, QuestProgress>,
    /// Active dialogue (npc id, node id) while a conversation is open.
    pub active_dialogue: Option<(String, String)>,
    /// Open world-tile prompt (TS `specialPrompt`), see world_interact.rs.
    pub active_prompt: Option<crate::world_interact::SpecialPrompt>,
    /// Bumped whenever a tile in a zone changes (doors unsealed, chests
    /// looted...) so the front-end knows to rebuild terrain.
    pub tile_rev: u32,
    /// Targeted skill waiting for a target (TS armedSkill / aimedTarget).
    pub armed_skill: Option<SkillId>,
    pub aimed_target: Option<Pos>,
    /// Presentation cues since the last `take_fx` (capped at FX_CAP).
    pub fx: Vec<FxEvent>,
    /// Dialogue set_flag values for keys outside the typed WorldFlags.
    extra_flags: HashMap<String, Value>,
    pub log: Vec<String>,
    delve_progress: i32,
    kills_this_delve: i32,
    seq: u32,
}

impl GameSession {
    pub fn new_game(name: &str, starter_class: StarterClass, seed: u32) -> Self {
        let preset = get_starter_preset(starter_class);
        let mut id_gen = ItemIdGen::new();
        let weapon = create_starter_item(&mut id_gen, &preset.weapon_tag);
        let oakhaven = create_oakhaven_map();
        let spawn = oakhaven.spawn_point;
        let mut player = PlayerState {
            name: name.to_string(),
            starter_class,
            x: spawn.x,
            y: spawn.y,
            hp: 48,
            max_hp: 48,
            mana: 18,
            max_mana: 18,
            level: 1,
            xp: 0,
            xp_to_next: 65,
            gold: 35,
            attribute_points: 0,
            skill_points: 1,
            attributes: preset.attributes,
            skills: HashMap::from([(preset.initial_skill.as_str().to_string(), 1)]),
            prodigies: Some(vec![]),
            prodigy_cooldowns: Some(HashMap::new()),
            hotbar: vec![Some(preset.initial_skill), None, None, None, None],
            cooldowns: HashMap::new(),
            haste_steps: Some(0),
            bonus_hp: Some(0),
            statuses: vec![],
            equipped: HashMap::from([("weapon".to_string(), weapon)]),
            inventory: vec![
                create_consumable(&mut id_gen, ConsumableKind::HealPotion, 2),
                create_consumable(&mut id_gen, ConsumableKind::ManaPotion, 1),
            ],
            kills: 0,
            turns_elapsed: 0,
            first_strike_ready: true,
            last_dir_x: Some(1),
            companion: None,
        };
        let flags = WorldFlags {
            discovered_zones: vec![ZoneId::Oakhaven],
            caravan_choice: None,
            blight_choice: None,
            bell_choice: None,
            vex_tip_claimed: None,
            malakor_weakened: None,
            cipher_inscribed: None,
            malakor_defeated: None,
            cantor_defeated: None,
            cantor_pacified: None,
            inquisitor_defeated: None,
            barrow_boss_defeated: None,
            mire_boss_defeated: None,
            frost_boss_defeated: None,
            silverbridge_cleared: None,
            mira_potion_tier: 0,
            citadel_access: false,
            floor3_shortcut: false,
            npc_met: HashMap::new(),
            npc_disposition: HashMap::new(),
            read_lore_ids: vec![],
            confronted_bosses: None,
            first_gear_dropped: Some(false),
            waystones_attuned: None,
        };
        let town = create_town_state();
        let derived = compute_derived_stats(&player, Some(&flags), Some(&town));
        player.max_hp = derived.max_hp;
        player.hp = derived.max_hp;
        player.max_mana = derived.max_mana;
        player.mana = derived.max_mana;
        let mut zones = HashMap::new();
        zones.insert(ZoneId::Oakhaven.to_string_id(), oakhaven);
        let mut s = Self {
            player,
            town,
            flags,
            zones,
            current: ZoneId::Oakhaven,
            rng: Rng::new(seed),
            id_gen,
            tier: DifficultyTier::Standard,
            cycle: 0,
            merchant_stocks: HashMap::new(),
            quests: HashMap::new(),
            active_dialogue: None,
            active_prompt: None,
            tile_rev: 0,
            armed_skill: None,
            aimed_target: None,
            fx: vec![],
            extra_flags: HashMap::new(),
            log: vec![],
            delve_progress: 0,
            kills_this_delve: 0,
            seq: 1000,
        };
        s.say("Welcome to Oakhaven. The Reliquary below holds what the town needs.");
        s.refresh_visibility();
        s
    }

    /// Grant XP and run level-ups (TS grantXpAndCheckLevel shape:
    /// xpToNext grows x1.48, +2 attribute points / +1 skill point per
    /// level, full heal on level). Returns LevelUp if a level was gained.
    pub fn gain_xp(&mut self, xp: i32) -> SessionEvent {
        self.player.xp += xp;
        let mut event = SessionEvent::None;
        while self.player.xp >= self.player.xp_to_next {
            self.player.xp -= self.player.xp_to_next;
            self.player.level += 1;
            self.player.xp_to_next = (self.player.xp_to_next as f64 * 1.48).round() as i32;
            self.player.attribute_points += 2;
            self.player.skill_points += 1;
            let d = self.derived();
            self.player.max_hp = d.max_hp;
            self.player.hp = d.max_hp;
            self.player.max_mana = d.max_mana;
            self.say(&format!("Level up! You are now level {}.", self.player.level));
            let (px, py) = (self.player.x, self.player.y);
            self.fx("level_up", px, py);
            event = SessionEvent::LevelUp(self.player.level);
        }
        event
    }

    pub(crate) fn fx(&mut self, kind: &'static str, x: i32, y: i32) {
        self.fx_dir(kind, x, y, 0, 0);
    }

    pub(crate) fn fx_dir(&mut self, kind: &'static str, x: i32, y: i32, dx: i32, dy: i32) {
        if self.fx.len() >= FX_CAP {
            self.fx.remove(0);
        }
        self.fx.push(FxEvent { kind, x, y, dx, dy });
    }

    /// Drain the presentation cues queued since the last call.
    pub fn take_fx(&mut self) -> Vec<FxEvent> {
        std::mem::take(&mut self.fx)
    }

    pub(crate) fn say(&mut self, text: &str) {
        self.log.push(text.to_string());
        if self.log.len() > 200 {
            self.log.drain(0..100);
        }
    }

    pub fn zone(&self) -> &ZoneMap {
        &self.zones[&self.current.to_string_id()]
    }
    pub fn zone_mut(&mut self) -> &mut ZoneMap {
        let key = self.current.to_string_id();
        self.zones.get_mut(&key).expect("current zone exists")
    }
    pub fn derived(&self) -> DerivedStats {
        compute_derived_stats(&self.player, Some(&self.flags), Some(&self.town))
    }
    pub fn in_town(&self) -> bool {
        self.zone().is_town
    }

    fn tile_at(&self, x: i32, y: i32) -> Option<Tile> {
        let z = self.zone();
        if x < 0 || y < 0 || x >= z.width || y >= z.height {
            return None;
        }
        Some(z.tiles[y as usize][x as usize].clone())
    }

    /// One player step (or bump attack). Returns what the UI should react to.
    pub fn move_player(&mut self, dx: i32, dy: i32) -> SessionEvent {
        if self.player.hp <= 0 || self.active_prompt.is_some() {
            return SessionEvent::None;
        }
        let nx = self.player.x + dx;
        let ny = self.player.y + dy;
        if dx != 0 {
            self.player.last_dir_x = Some(if dx > 0 { 1 } else { -1 });
        }
        // Bump combat.
        let enemy_idx = self
            .zone()
            .enemies
            .iter()
            .position(|e| e.x == nx && e.y == ny && e.hp > 0);
        if let Some(idx) = enemy_idx {
            if !self.in_town() {
                self.delve_progress += 1;
            }
            return self.melee_attack(idx);
        }
        let tile = match self.tile_at(nx, ny) {
            Some(t) => t,
            None => return SessionEvent::None,
        };
        // TS: vault doors prompt on bump, before the walkability check.
        if tile.kind == TileKind::VaultDoor {
            return self
                .interact_with_world_tile(nx, ny)
                .unwrap_or(SessionEvent::None);
        }
        if !tile.walkable {
            return SessionEvent::None;
        }
        // NPCs block movement.
        if self.zone().npcs.iter().any(|n| n.x == nx && n.y == ny) {
            return SessionEvent::None;
        }
        // TS: no diagonal corner-cutting past blocking walls.
        if dx != 0 && dy != 0 {
            let side_a = self.tile_at(nx, self.player.y).map_or(false, |t| t.walkable);
            let side_b = self.tile_at(self.player.x, ny).map_or(false, |t| t.walkable);
            if !side_a || !side_b {
                return SessionEvent::None;
            }
        }
        self.player.x = nx;
        self.player.y = ny;
        if !self.in_town() {
            self.delve_progress += 1;
        }
        self.after_step_on_tile(tile.kind)
    }

    fn after_step_on_tile(&mut self, kind: TileKind) -> SessionEvent {
        // Pick up ground loot underfoot.
        let (px, py) = (self.player.x, self.player.y);
        let loot_idx = self
            .zone()
            .loot
            .iter()
            .position(|l| l.x == px && l.y == py);
        if let Some(li) = loot_idx {
            let loot = self.zone_mut().loot.remove(li);
            self.fx("pickup", px, py);
            if let Some(g) = loot.gold {
                self.player.gold += g;
                self.say(&format!("Picked up {g} gold."));
            }
            if let Some(item) = loot.item {
                self.say(&format!("Picked up {}.", item.name));
                self.player.inventory.push(item);
            }
            if !self.in_town() {
                self.delve_progress += 1;
            }
        }
        match kind {
            TileKind::StairsDown => return self.transition_down(),
            TileKind::StairsUp => return self.transition_up(),
            TileKind::GateCity => return self.transition_gate(),
            // TS auto-triggers these when stepped onto. A handled chest or
            // brazier, and any unattuned waystone, ends the step without an
            // enemy turn; an already-used one is just floor.
            TileKind::Chest | TileKind::Brazier | TileKind::Waystone => {
                if let Some(ev) = self.interact_with_world_tile(px, py) {
                    return ev;
                }
            }
            _ => {}
        }
        self.run_enemy_turns()
    }

    pub(crate) fn on_enemy_killed(&mut self, idx: usize) -> SessionEvent {
        let enemy = self.zone_mut().enemies.remove(idx);
        self.fx("kill", enemy.x, enemy.y);
        self.player.kills += 1;
        self.kills_this_delve += 1;
        self.delve_progress += 2;
        self.player.gold += enemy.gold_reward;
        self.say(&format!("{} falls. +{} gold.", enemy.name, enemy.gold_reward));
        self.kill_passives();
        // XP with difficulty scaling + level-ups (TS curve: xpToNext x1.48).
        let xp = crate::difficulty::scale_xp_for_difficulty(enemy.xp_reward, self.tier, self.cycle);
        let event = self.gain_xp(xp);
        // Kill drop (first-gear guarantee honoured via loot rules).
        let first = self.flags.first_gear_dropped == Some(true);
        let depth = self.zone().depth;
        let level = self.player.level;
        if let Some(drop) =
            crate::loot::generate_kill_drop(&mut self.id_gen, &mut self.rng, depth, level, first)
        {
            if drop.category == ItemCategory::Equipment {
                self.flags.first_gear_dropped = Some(true);
            }
            let seq = self.seq;
            self.seq += 1;
            self.zone_mut().loot.push(GroundLoot {
                id: format!("drop_{seq}"),
                x: enemy.x,
                y: enemy.y,
                gold: None,
                item: Some(drop),
            });
        }
        event
    }

    fn transition_down(&mut self) -> SessionEvent {
        // Portal tiles carry their destination in interact_id (TS
        // overworld portals: to_barrow_1 / to_frost_1 / to_mire_1 /
        // to_dungeon_1).
        let tile = self.tile_at(self.player.x, self.player.y);
        if let Some(t) = &tile {
            if let Some(dest) = t.interact_id.as_deref().and_then(ZoneId::parse) {
                return self.enter_zone(dest, true);
            }
            if let Some(id_str) = t.interact_id.as_deref().and_then(|s| s.strip_prefix("to_")) {
                if let Some(dest) = ZoneId::parse(id_str) {
                    return self.enter_zone(dest, true);
                }
            }
        }
        let next = match self.current {
            ZoneId::Oakhaven => ZoneId::Dungeon(1),
            ZoneId::Dungeon(n) => ZoneId::Dungeon(n + 1),
            ZoneId::Barrow(n) => ZoneId::Barrow(n + 1),
            ZoneId::Mire(n) => ZoneId::Mire(n + 1),
            ZoneId::Frost(n) => ZoneId::Frost(n + 1),
            other => other,
        };
        self.enter_zone(next, true)
    }

    fn transition_up(&mut self) -> SessionEvent {
        match self.current {
            // Main descent: Wake/banking path back to Oakhaven.
            ZoneId::Dungeon(1) => self.return_to_oakhaven(false),
            // Side branches exit onto the overworld beside their portals
            // (TS); carried supplies stay unbanked until Oakhaven.
            ZoneId::Barrow(1) => self.enter_zone_at(ZoneId::Overworld, Some(Pos::new(10, 5)), false),
            ZoneId::Mire(1) => self.enter_zone_at(ZoneId::Overworld, Some(Pos::new(42, 21)), false),
            ZoneId::Frost(1) => self.enter_zone_at(ZoneId::Overworld, Some(Pos::new(44, 4)), false),
            ZoneId::Dungeon(n) => self.enter_zone(ZoneId::Dungeon(n - 1), false),
            ZoneId::Barrow(n) => self.enter_zone(ZoneId::Barrow(n - 1), false),
            ZoneId::Mire(n) => self.enter_zone(ZoneId::Mire(n - 1), false),
            ZoneId::Frost(n) => self.enter_zone(ZoneId::Frost(n - 1), false),
            _ => SessionEvent::None,
        }
    }

    /// Surface travel gates (TS App gate_city graph). Entry positions are
    /// the TS transition targets.
    fn transition_gate(&mut self) -> SessionEvent {
        match self.current {
            ZoneId::Oakhaven => {
                self.enter_zone_at(ZoneId::Overworld, Some(Pos::new(2, 12)), false)
            }
            ZoneId::Highspire => {
                self.enter_zone_at(ZoneId::Overworld, Some(Pos::new(45, 12)), false)
            }
            ZoneId::Thornwick => {
                self.enter_zone_at(ZoneId::Overworld, Some(Pos::new(32, 24)), false)
            }
            ZoneId::Cinderwatch => {
                self.enter_zone_at(ZoneId::Overworld, Some(Pos::new(31, 2)), false)
            }
            ZoneId::Overworld => {
                let tile = self.tile_at(self.player.x, self.player.y);
                match tile.and_then(|t| t.interact_id) {
                    Some(id) if id == "gate_to_thornwick" => {
                        self.enter_zone(ZoneId::Thornwick, false)
                    }
                    Some(id) if id == "gate_to_cinderwatch" => {
                        self.enter_zone(ZoneId::Cinderwatch, false)
                    }
                    Some(id) if id == "gate_to_highspire" => {
                        if self.flags.citadel_access {
                            self.enter_zone_at(
                                ZoneId::Highspire,
                                Some(Pos::new(1, 9)),
                                false,
                            )
                        } else {
                            self.say("The Highspire Causeway Gate is sealed. Earn a Citadel Pass from Vance or Vex in Oakhaven.");
                            SessionEvent::None
                        }
                    }
                    // gate_to_oakhaven (and any unmarked overworld gate)
                    _ => self.arrive_oakhaven(Pos::new(1, 7)),
                }
            }
            _ => SessionEvent::None,
        }
    }

    /// Arriving in Oakhaven from the road: bank carried town items and run
    /// the return tick when a delve actually happened (anti-cheese guard
    /// lives in town_return_tick); otherwise just step through the gate.
    fn arrive_oakhaven(&mut self, entry: Pos) -> SessionEvent {
        if !self.carried_town_tags().is_empty() || self.kills_this_delve > 0 {
            self.return_to_oakhaven(false)
        } else {
            self.enter_zone_at(ZoneId::Oakhaven, Some(entry), false)
        }
    }

    /// Dev/capture helper (debug bridge builds only): enter `id`
    /// (generating it if needed) and stand at (x, y); x < 0 keeps the
    /// zone's normal entry tile. No banking, ticks or turn passes.
    pub fn debug_warp(&mut self, id: ZoneId, x: i32, y: i32) -> SessionEvent {
        self.active_prompt = None;
        self.active_dialogue = None;
        let entry = if x >= 0 { Some(Pos::new(x, y)) } else { None };
        self.enter_zone_at(id, entry, true)
    }

    fn enter_zone(&mut self, id: ZoneId, down: bool) -> SessionEvent {
        self.enter_zone_at(id, None, down)
    }

    fn enter_zone_at(&mut self, id: ZoneId, entry_override: Option<Pos>, down: bool) -> SessionEvent {
        let key = id.to_string_id();
        if !self.zones.contains_key(&key) {
            let zone = crate::world::generate_zone_by_id(id.clone(), &mut self.rng, self.tier, self.cycle)
                .unwrap_or_else(create_oakhaven_map);
            self.zones.insert(key.clone(), zone);
            if !self.flags.discovered_zones.contains(&id) {
                self.flags.discovered_zones.push(id);
            }
        }
        self.current = id;
        let zone = self.zone();
        let entry = if down { zone.return_point } else { zone.spawn_point };
        // Prefer the matching stairs tile when present.
        let stairs_kind = if down { TileKind::StairsUp } else { TileKind::StairsDown };
        let entry = zone
            .tiles
            .iter()
            .flatten()
            .find(|t| t.kind == stairs_kind)
            .map(|t| Pos::new(t.x, t.y))
            .unwrap_or(entry);
        let entry = entry_override.unwrap_or(entry);
        self.player.x = entry.x;
        self.player.y = entry.y;
        self.cancel_aim();
        self.refresh_visibility();
        self.say(&format!("You enter {}.", self.zone().name));
        SessionEvent::ZoneChanged(id)
    }

    /// Alive return to Oakhaven: bank carried town items, then the
    /// ActRaiser return tick (guarded), then rest at the spawn point.
    pub fn return_to_oakhaven(&mut self, is_death: bool) -> SessionEvent {
        if is_death {
            return self.wake_in_oakhaven();
        }
        let banked = bank_carried_town_items(
            std::mem::replace(&mut self.town, create_town_state()),
            std::mem::take(&mut self.player.inventory),
        );
        self.town = banked.state;
        self.player.inventory = banked.remaining_inventory;
        if !banked.banked_tags.is_empty() {
            self.say(&format!(
                "Banked in Oakhaven: {}.",
                banked.banked_tags.join(", ")
            ));
        }
        let progress = self.delve_progress;
        self.town = town_return_tick(
            std::mem::replace(&mut self.town, create_town_state()),
            TownReturnOptions {
                delve_progress: Some(progress),
                is_death_return: false,
                require_stall_cleared: true,
            },
        );
        self.delve_progress = 0;
        self.kills_this_delve = 0;
        self.current = ZoneId::Oakhaven;
        let spawn = self.zone().spawn_point;
        self.player.x = spawn.x;
        self.player.y = spawn.y;
        self.say("You return to Oakhaven.");
        SessionEvent::ZoneChanged(ZoneId::Oakhaven)
    }

    /// Death: unbanked town items drop on the dungeon floor where you fell;
    /// you wake in Oakhaven. No return tick, no banking.
    pub fn wake_in_oakhaven(&mut self) -> SessionEvent {
        let split = split_unbanked_town_items(std::mem::take(&mut self.player.inventory));
        self.player.inventory = split.kept_inventory;
        if !split.dropped_town_items.is_empty() {
            let (px, py) = (self.player.x, self.player.y);
            let zone = self.zone_mut();
            for item in split.dropped_town_items {
                zone.loot.push(GroundLoot {
                    id: format!("death_drop_{}", item.id),
                    x: px,
                    y: py,
                    gold: None,
                    item: Some(item),
                });
            }
            self.say("Your unbanked town finds slip from your pack where you fell.");
        }
        self.delve_progress = 0;
        self.kills_this_delve = 0;
        self.current = ZoneId::Oakhaven;
        let spawn = self.zone().spawn_point;
        self.player.x = spawn.x;
        self.player.y = spawn.y;
        // TS respawn reset: statuses, cooldowns and prodigy cooldowns clear.
        self.player.statuses.clear();
        self.player.cooldowns.clear();
        self.player.prodigy_cooldowns = Some(Default::default());
        self.player.first_strike_ready = true;
        self.cancel_aim();
        self.refresh_visibility();
        let d = self.derived();
        self.player.hp = d.max_hp;
        self.player.max_hp = d.max_hp;
        self.say("You wake in Oakhaven, bruised but alive. The town endures.");
        SessionEvent::PlayerDied
    }

    /// Interact with an adjacent NPC, the rebuilt Blacksmith lot, or a
    /// world tile underfoot / adjacent (vault, seal door, chest, brazier,
    /// waystone — world_interact.rs).
    pub fn interact(&mut self) -> SessionEvent {
        if self.active_prompt.is_some() {
            return SessionEvent::None;
        }
        let (px, py) = (self.player.x, self.player.y);
        let npc = self
            .zone()
            .npcs
            .iter()
            .find(|n| (n.x - px).abs().max((n.y - py).abs()) <= 1)
            .map(|n| n.id.clone());
        if let Some(npc_id) = npc {
            self.flags.npc_met.insert(npc_id.clone(), true);
            // TS: talking always opens the dialogue tree; the trade choice
            // inside resolves 'torin' -> 'blacksmith' once the forge is
            // rebuilt (see merchant_stock).
            let root = crate::npcs::npc_def(&npc_id)
                .map(|d| d.root_node_id.clone())
                .unwrap_or_else(|| "root".into());
            self.active_dialogue = Some((npc_id.clone(), root));
            return SessionEvent::OpenDialogue(npc_id);
        }
        if self.in_town()
            && self.town.blacksmith_complete
            && crate::town_layout::is_near_blacksmith_lot(px, py, Some(&self.town))
        {
            return SessionEvent::OpenMerchant("blacksmith".into());
        }
        // TS handleInteractAction: the tile underfoot, then the 8 around.
        if let Some(ev) = self.interact_with_world_tile(px, py) {
            return ev;
        }
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if let Some(ev) = self.interact_with_world_tile(px + dx, py + dy) {
                    return ev;
                }
            }
        }
        SessionEvent::None
    }

    pub fn can_complete_blacksmith(&self) -> bool {
        can_complete_blacksmith(&self.town)
    }

    pub fn complete_blacksmith(&mut self, lot: BlacksmithLot) -> bool {
        let result = complete_blacksmith(
            std::mem::replace(&mut self.town, create_town_state()),
            lot,
        );
        self.town = result.state;
        if result.completed {
            self.player.gold += result.wish_bonus_gold;
            self.say("Torin's Blacksmith rises again. The rebuilt shop is open.");
            let stock = crate::npcs::create_merchant_stock(
                &mut self.id_gen,
                &mut self.rng,
                "blacksmith",
                Some(&self.town),
            );
            self.merchant_stocks.insert("blacksmith".into(), stock);
        }
        result.completed
    }

    pub fn relocate_blacksmith(&mut self, lot: BlacksmithLot) -> bool {
        let result = relocate_blacksmith(
            std::mem::replace(&mut self.town, create_town_state()),
            lot,
        );
        let changed = result.relocated;
        self.town = result.state;
        if result.wish_bonus_gold > 0 {
            self.player.gold += result.wish_bonus_gold;
            self.say("Torin's wish fulfilled: +75 gold.");
        }
        changed
    }

    /// TS merchant resolution: once the Blacksmith is rebuilt, Torin's
    /// trade opens the rebuilt shop's stock instead of his field kit.
    fn resolve_merchant(&self, merchant_id: &str) -> String {
        if merchant_id == "torin" && self.town.blacksmith_complete {
            "blacksmith".to_string()
        } else {
            merchant_id.to_string()
        }
    }

    pub fn merchant_stock(&mut self, merchant_id: &str) -> Vec<Item> {
        let id = self.resolve_merchant(merchant_id);
        if !self.merchant_stocks.contains_key(&id) {
            let stock = crate::npcs::create_merchant_stock(
                &mut self.id_gen,
                &mut self.rng,
                &id,
                Some(&self.town),
            );
            self.merchant_stocks.insert(id.clone(), stock);
        }
        self.merchant_stocks.get(&id).cloned().unwrap_or_default()
    }

    pub fn buy_item(&mut self, merchant_id: &str, index: usize) -> Result<Item, String> {
        let stock = self.merchant_stock(merchant_id);
        let item = stock.get(index).cloned().ok_or("No such item.")?;
        if self.player.gold < item.value {
            return Err("Not enough gold.".into());
        }
        self.player.gold -= item.value;
        self.player.inventory.push(item.clone());
        self.say(&format!("Bought {}.", item.name));
        Ok(item)
    }

    // ------------------------------------------------------------------
    // Dialogue engine (M3) — mirrors App.tsx handleSelectDialogueChoice.
    // ------------------------------------------------------------------

    fn flag_value(&self, name: &str) -> Option<Value> {
        if let Some(v) = self.extra_flags.get(name) {
            return Some(v.clone());
        }
        let f = &self.flags;
        let v = match name {
            "caravanChoice" => f.caravan_choice.clone().map(Value::from),
            "blightChoice" => f.blight_choice.clone().map(Value::from),
            "bellChoice" => f.bell_choice.clone().map(Value::from),
            "vexTipClaimed" => f.vex_tip_claimed.map(Value::from),
            "malakorWeakened" => f.malakor_weakened.map(Value::from),
            "cipherInscribed" => f.cipher_inscribed.map(Value::from),
            "malakorDefeated" => f.malakor_defeated.map(Value::from),
            "cantorDefeated" => f.cantor_defeated.map(Value::from),
            "cantorPacified" => f.cantor_pacified.map(Value::from),
            "inquisitorDefeated" => f.inquisitor_defeated.map(Value::from),
            "barrowBossDefeated" => f.barrow_boss_defeated.map(Value::from),
            "mireBossDefeated" => f.mire_boss_defeated.map(Value::from),
            "frostBossDefeated" => f.frost_boss_defeated.map(Value::from),
            "silverbridgeCleared" => f.silverbridge_cleared.map(Value::from),
            "miraPotionTier" => Some(Value::from(f.mira_potion_tier)),
            "citadelAccess" => Some(Value::from(f.citadel_access)),
            "floor3Shortcut" => Some(Value::from(f.floor3_shortcut)),
            "firstGearDropped" => f.first_gear_dropped.map(Value::from),
            "waystonesAttuned" => f.waystones_attuned.map(Value::from),
            _ => None,
        };
        v
    }

    fn set_flag_value(&mut self, name: &str, value: Value) {
        let f = &mut self.flags;
        let as_bool = value.as_bool();
        let as_str = value.as_str().map(|s| s.to_string());
        match name {
            "caravanChoice" => f.caravan_choice = as_str,
            "blightChoice" => f.blight_choice = as_str,
            "bellChoice" => f.bell_choice = as_str,
            "vexTipClaimed" => f.vex_tip_claimed = as_bool,
            "malakorWeakened" => f.malakor_weakened = as_bool,
            "cipherInscribed" => f.cipher_inscribed = as_bool,
            "malakorDefeated" => f.malakor_defeated = as_bool,
            "cantorDefeated" => f.cantor_defeated = as_bool,
            "cantorPacified" => f.cantor_pacified = as_bool,
            "inquisitorDefeated" => f.inquisitor_defeated = as_bool,
            "barrowBossDefeated" => f.barrow_boss_defeated = as_bool,
            "mireBossDefeated" => f.mire_boss_defeated = as_bool,
            "frostBossDefeated" => f.frost_boss_defeated = as_bool,
            "silverbridgeCleared" => f.silverbridge_cleared = as_bool,
            "miraPotionTier" => {
                if let Some(n) = value.as_i64() {
                    f.mira_potion_tier = n as i32;
                }
            }
            "citadelAccess" => f.citadel_access = as_bool.unwrap_or(false),
            "floor3Shortcut" => f.floor3_shortcut = as_bool.unwrap_or(false),
            "firstGearDropped" => f.first_gear_dropped = as_bool,
            "waystonesAttuned" => {
                if let Some(n) = value.as_i64() {
                    f.waystones_attuned = Some(n as i32);
                }
            }
            _ => {
                self.extra_flags.insert(name.to_string(), value);
            }
        }
    }

    /// TS choice-condition gate (choices failing it are hidden).
    pub fn condition_met(&self, cond: &DialogueCondition) -> bool {
        if let Some(flag) = &cond.flag {
            let actual = self.flag_value(flag);
            match &cond.flag_equals {
                Some(want) => {
                    if actual.as_ref() != Some(want) {
                        return false;
                    }
                }
                None => {
                    let truthy = matches!(actual, Some(Value::Bool(true)))
                        || matches!(actual, Some(Value::String(ref s)) if !s.is_empty())
                        || matches!(actual, Some(Value::Number(ref n)) if n.as_i64() != Some(0));
                    if !truthy {
                        return false;
                    }
                }
            }
        }
        if let Some(not_flag) = &cond.not_flag {
            let actual = self.flag_value(not_flag);
            let truthy = matches!(actual, Some(Value::Bool(true)))
                || matches!(actual, Some(Value::String(ref s)) if !s.is_empty());
            if truthy {
                return false;
            }
        }
        if let Some(quest_id) = &cond.quest_id {
            match &cond.quest_status {
                // questStatus present: the quest must be in that status.
                Some(status) => match self.quests.get(quest_id) {
                    Some(q) if &q.status == status => {}
                    _ => return false,
                },
                // questId with questStatus undefined (npcs.ts convention):
                // the quest must not have been started.
                None => {
                    if self.quests.contains_key(quest_id) {
                        return false;
                    }
                }
            }
            if let Some(min) = cond.quest_min_stage {
                if self.quests.get(quest_id).map(|q| q.stage).unwrap_or(0) < min {
                    return false;
                }
            }
            if let Some(max) = cond.quest_max_stage {
                if self.quests.get(quest_id).map(|q| q.stage).unwrap_or(0) > max {
                    return false;
                }
            }
        }
        if let Some(tag) = &cond.has_item_tag {
            if !self
                .player
                .inventory
                .iter()
                .any(|i| i.quest_tag.as_deref() == Some(tag.as_str()))
            {
                return false;
            }
        }
        if let Some(min_gold) = cond.min_gold {
            if self.player.gold < min_gold {
                return false;
            }
        }
        true
    }

    /// Current dialogue node as JSON for the front-end (choices filtered
    /// by their conditions, like the TS renderer).
    pub fn dialogue_json(&self) -> Value {
        let Some((npc_id, node_id)) = &self.active_dialogue else {
            return Value::Null;
        };
        let Some(def) = crate::npcs::npc_def(npc_id) else {
            return Value::Null;
        };
        let Some(node) = def.nodes.get(node_id) else {
            return Value::Null;
        };
        let choices: Vec<Value> = node
            .choices
            .iter()
            .filter(|c| c.condition.as_ref().map(|k| self.condition_met(k)).unwrap_or(true))
            .map(|c| json!({ "id": c.id, "label": c.label }))
            .collect();
        json!({
            "npcId": def.id,
            "name": def.name,
            "title": def.title,
            "color": def.color,
            "nodeId": node.id,
            "speakerName": node.speaker_name,
            "text": node.text,
            "choices": choices,
        })
    }

    pub fn close_dialogue(&mut self) {
        self.active_dialogue = None;
    }

    fn apply_dialogue_action(&mut self, action: &DialogueAction) -> Option<SessionEvent> {
        match action {
            DialogueAction::CloseDialogue => {
                self.active_dialogue = None;
                None
            }
            DialogueAction::OpenMerchant { merchant_id } => {
                self.active_dialogue = None;
                Some(SessionEvent::OpenMerchant(merchant_id.clone()))
            }
            DialogueAction::HealFull => {
                let d = compute_derived_stats(&self.player, Some(&self.flags), Some(&self.town));
                self.player.max_hp = d.max_hp;
                self.player.hp = d.max_hp;
                self.player.max_mana = d.max_mana;
                self.player.mana = d.max_mana;
                self.player.statuses.clear();
                None
            }
            DialogueAction::GiveGold { amount } => {
                self.player.gold += amount;
                None
            }
            DialogueAction::SpendGold { amount } => {
                self.player.gold = (self.player.gold - amount).max(0);
                None
            }
            DialogueAction::ConsumeQuestItem { tag } => {
                self.player
                    .inventory
                    .retain(|i| i.quest_tag.as_deref() != Some(tag.as_str()));
                None
            }
            DialogueAction::SetFlag { flag, value } => {
                self.set_flag_value(flag, value.clone());
                None
            }
            DialogueAction::UnlockHighspire => {
                self.flags.citadel_access = true;
                self.say("Citadel Access unlocked! Walk through Oakhaven’s West Gate (Ω) or Travel [M].");
                None
            }
            DialogueAction::GrantBuff { text, hp_bonus } => {
                if let Some(bonus) = hp_bonus {
                    self.player.bonus_hp = Some(self.player.bonus_hp.unwrap_or(0) + bonus);
                    let d =
                        compute_derived_stats(&self.player, Some(&self.flags), Some(&self.town));
                    self.player.max_hp = d.max_hp;
                    self.player.hp = (self.player.hp + bonus).min(d.max_hp);
                }
                self.say(text);
                None
            }
            DialogueAction::StartQuest { quest_id, log } => {
                let stage = if quest_id == "deep_survey" {
                    (self.flags.read_lore_ids.len() as i32).clamp(1, 6)
                } else {
                    1
                };
                self.quests.insert(
                    quest_id.clone(),
                    QuestProgress {
                        id: quest_id.clone(),
                        status: QuestStatus::Active,
                        stage,
                        outcome_label: None,
                        history: vec![log.clone()],
                    },
                );
                let title = crate::npcs::quest_def(quest_id)
                    .map(|q| q.title.to_string())
                    .unwrap_or_else(|| quest_id.clone());
                self.say(&format!("Quest Started: {title}"));
                None
            }
            DialogueAction::AdvanceQuest {
                quest_id,
                stage,
                log,
            } => {
                if let Some(q) = self.quests.get_mut(quest_id) {
                    q.stage = *stage;
                    q.history.push(log.clone());
                }
                None
            }
            DialogueAction::CompleteQuest {
                quest_id,
                outcome_label,
                log,
                reward_gold,
                reward_xp,
                reward_skill_point,
                reward_attr_point,
                reward_item_tag,
            } => {
                // TS pay-once guard: a completed quest never pays twice.
                if self.quests.get(quest_id).map(|q| q.status.clone())
                    == Some(QuestStatus::Completed)
                {
                    return None;
                }
                let history = {
                    let mut h = self
                        .quests
                        .get(quest_id)
                        .map(|q| q.history.clone())
                        .unwrap_or_default();
                    h.push(log.clone());
                    h
                };
                self.quests.insert(
                    quest_id.clone(),
                    QuestProgress {
                        id: quest_id.clone(),
                        status: QuestStatus::Completed,
                        stage: 3,
                        outcome_label: Some(outcome_label.clone()),
                        history,
                    },
                );
                if let Some(g) = reward_gold {
                    self.player.gold += g;
                }
                if let Some(sp) = reward_skill_point {
                    self.player.skill_points += sp;
                }
                if let Some(ap) = reward_attr_point {
                    self.player.attribute_points += ap;
                }
                if let Some(tag) = reward_item_tag {
                    let relic = create_named_relic(&mut self.id_gen, &mut self.rng, tag);
                    self.say(&format!("Received {}!", relic.name));
                    self.player.inventory.push(relic);
                }
                if let Some(xp) = reward_xp {
                    self.gain_xp(*xp);
                }
                let title = crate::npcs::quest_def(quest_id)
                    .map(|q| q.title.to_string())
                    .unwrap_or_else(|| quest_id.clone());
                self.say(&format!("Quest Completed: {title}"));
                None
            }
            DialogueAction::RespecSkills { cost } => {
                if self.player.gold >= *cost {
                    self.player.gold -= cost;
                    let preset = get_starter_preset(self.player.starter_class);
                    let refunded =
                        compute_respec_points(self.player.level, &self.flags, &self.quests);
                    self.player.attributes = preset.attributes;
                    self.player.skills.clear();
                    self.player.hotbar = vec![None; 5];
                    self.player.attribute_points = refunded.attribute_points;
                    self.player.skill_points = refunded.skill_points;
                    let d =
                        compute_derived_stats(&self.player, Some(&self.flags), Some(&self.town));
                    self.player.max_hp = d.max_hp;
                    self.player.hp = d.max_hp;
                    self.player.max_mana = d.max_mana;
                    self.player.mana = d.max_mana;
                }
                None
            }
            DialogueAction::SwitchCompanion => {
                let next_kind = match self.player.companion.as_ref().map(|c| c.kind) {
                    Some(CompanionKind::StarlightOwl) => CompanionKind::SilverwindLynx,
                    _ => CompanionKind::StarlightOwl,
                };
                let (name, max_hp, blurb) = match next_kind {
                    CompanionKind::StarlightOwl => (
                        "Starlight Owl",
                        20 + (self.player.level - 1) * 5,
                        "+1 Sight Radius & Arcane Bolts",
                    ),
                    CompanionKind::SilverwindLynx => (
                        "Silverwind Lynx",
                        24 + (self.player.level - 1) * 7,
                        "Swift Melee Pounce",
                    ),
                };
                let (cx, cy) = self
                    .player
                    .companion
                    .as_ref()
                    .map(|c| (c.x, c.y))
                    .unwrap_or((self.player.x - 1, self.player.y));
                self.player.companion = Some(CompanionState {
                    kind: next_kind,
                    name: name.to_string(),
                    x: cx.max(1),
                    y: cy,
                    hp: max_hp,
                    max_hp,
                    respawn_cooldown: 0,
                });
                self.say(&format!("Spirit Companion bonded: {name} ({blurb})."));
                None
            }
        }
    }

    /// Pick a choice in the active dialogue. Mirrors the TS processor:
    /// a failed skill check routes to failNodeId with NO actions applied;
    /// otherwise actions run (action + extraActions), then navigation.
    pub fn choose_dialogue(&mut self, choice_id: &str) -> SessionEvent {
        let Some((npc_id, node_id)) = self.active_dialogue.clone() else {
            return SessionEvent::None;
        };
        let Some(def) = crate::npcs::npc_def(&npc_id) else {
            self.active_dialogue = None;
            return SessionEvent::None;
        };
        let Some(node) = def.nodes.get(&node_id) else {
            self.active_dialogue = None;
            return SessionEvent::None;
        };
        let Some(choice) = node.choices.iter().find(|c| c.id == choice_id).cloned() else {
            return SessionEvent::None;
        };
        if let Some(cond) = &choice.condition {
            if !self.condition_met(cond) {
                return SessionEvent::None;
            }
        }
        if let Some(check) = &choice.check {
            let pass = self.player.attributes.get(check.attribute) >= check.threshold;
            if !pass {
                let target = choice.fail_node_id.clone().or(choice.next_node_id.clone());
                match target {
                    Some(t) => {
                        self.active_dialogue = Some((npc_id, t));
                    }
                    None => {
                        self.active_dialogue = None;
                    }
                }
                return SessionEvent::None;
            }
        }
        let mut event = SessionEvent::None;
        let mut actions: Vec<DialogueAction> = Vec::new();
        if let Some(a) = &choice.action {
            actions.push(a.clone());
        }
        actions.extend(choice.extra_actions.iter().cloned());
        let mut closed = false;
        for action in &actions {
            if let Some(ev) = self.apply_dialogue_action(action) {
                event = ev;
                closed = true;
            }
            if matches!(action, DialogueAction::CloseDialogue) {
                closed = true;
            }
        }
        if closed {
            self.active_dialogue = None;
            return event;
        }
        match &choice.next_node_id {
            Some(next) => {
                self.active_dialogue = Some((npc_id, next.clone()));
                SessionEvent::None
            }
            None => {
                self.active_dialogue = None;
                SessionEvent::None
            }
        }
    }

    /// Carried (unbanked) town items in the player's pack.
    pub fn carried_town_tags(&self) -> Vec<String> {
        self.player
            .inventory
            .iter()
            .filter(|i| is_town_item_tag(i.quest_tag.as_deref()))
            .filter_map(|i| i.quest_tag.clone())
            .collect()
    }

    /// JSON snapshot for the Godot front-end.
    pub fn state_json(&self) -> Value {
        let zone = self.zone();
        let tiles: Vec<Vec<String>> = zone
            .tiles
            .iter()
            .map(|row| row.iter().map(|t| t.kind.as_str().to_string()).collect())
            .collect();
        json!({
            "zone": {
                "id": zone.id.to_string_id(),
                "name": zone.name,
                "subtitle": zone.subtitle,
                "isTown": zone.is_town,
                "width": zone.width,
                "height": zone.height,
                "tiles": tiles,
                "tileRev": self.tile_rev,
                "usedTiles": self.used_tiles(),
                "enemies": zone.enemies.iter().filter(|e| e.hp > 0).map(|e| json!({
                    "id": e.id, "name": e.name, "glyph": e.glyph, "x": e.x, "y": e.y,
                    "hp": e.hp, "maxHp": e.max_hp, "isBoss": e.is_boss.unwrap_or(false),
                    "alerted": e.alerted,
                    "statuses": e.statuses.iter().map(|st| json!({
                        "kind": st.kind.as_str(), "turns": st.turns, "potency": st.potency,
                    })).collect::<Vec<_>>(),
                    "intent": e.intent.as_ref().map(|it| json!({
                        "label": it.label, "damage": it.damage, "targetTiles": it.target_tiles,
                    })),
                })).collect::<Vec<_>>(),
                "npcs": zone.npcs.iter().map(|n| json!({
                    "id": n.id, "name": n.name, "title": n.title, "glyph": n.glyph, "x": n.x, "y": n.y,
                })).collect::<Vec<_>>(),
                "loot": zone.loot.iter().map(|l| json!({
                    "x": l.x, "y": l.y, "gold": l.gold,
                    "itemName": l.item.as_ref().map(|i| i.name.clone()),
                })).collect::<Vec<_>>(),
            },
            "player": serde_json::to_value(&self.player).unwrap_or(Value::Null),
            "derived": serde_json::to_value(self.derived()).unwrap_or(Value::Null),
            "town": serde_json::to_value(&self.town).unwrap_or(Value::Null),
            "populationTier": crate::town::population_tier(&self.town),
            "canCompleteBlacksmith": self.can_complete_blacksmith(),
            "quests": serde_json::to_value(&self.quests).unwrap_or(Value::Null),
            "carriedTownTags": self.carried_town_tags(),
            "prompt": self.active_prompt.as_ref().map(|p| p.kind.as_str()),
            "hotbar": self.hotbar_json(),
            "armedSkill": self.armed_skill.map(|k| k.as_str()),
            "aimedTarget": self.aimed_target,
            "log": self.log.iter().rev().take(8).rev().cloned().collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BFS next step toward (tx, ty) that treats enemy-occupied tiles as
    /// passable (so the walker bump-fights through chokepoints the way a
    /// player would; combat::find_path_next_step treats foes as walls).
    fn next_step_through_foes(s: &GameSession, tx: i32, ty: i32) -> Option<(i32, i32)> {
        use std::collections::{HashMap, HashSet, VecDeque};
        let (sx, sy) = (s.player.x, s.player.y);
        if sx == tx && sy == ty {
            return None;
        }
        let zone = s.zone();
        let passable = |x: i32, y: i32| -> bool {
            if x == tx && y == ty {
                return true;
            }
            match zone.tiles.get(y as usize).and_then(|r| r.get(x as usize)) {
                Some(t) => {
                    t.walkable
                        || zone.enemies.iter().any(|e| e.x == x && e.y == y && e.hp > 0)
                }
                None => false,
            }
        };
        let mut visited: HashSet<(i32, i32)> = HashSet::from([(sx, sy)]);
        let mut parent: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut queue: VecDeque<(i32, i32)> = VecDeque::from([(sx, sy)]);
        while let Some((cx, cy)) = queue.pop_front() {
            if cx == tx && cy == ty {
                let mut cur = (cx, cy);
                while parent[&cur] != (sx, sy) {
                    cur = parent[&cur];
                }
                return Some(cur);
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let (nx, ny) = (cx + dx, cy + dy);
                if visited.contains(&(nx, ny)) || !passable(nx, ny) {
                    continue;
                }
                // The game forbids diagonal corner-cutting past walls.
                let walk = |x: i32, y: i32| {
                    zone.tiles
                        .get(y as usize)
                        .and_then(|r| r.get(x as usize))
                        .map_or(false, |t| t.walkable)
                };
                if dx != 0 && dy != 0 && (!walk(cx + dx, cy) || !walk(cx, cy + dy)) {
                    continue;
                }
                visited.insert((nx, ny));
                parent.insert((nx, ny), (cx, cy));
                queue.push_back((nx, ny));
            }
        }
        None
    }

    /// Walk the player to (tx, ty) one real move at a time, bump-fighting
    /// anything in the way. Returns false if unreachable.
    fn walk_to(s: &mut GameSession, tx: i32, ty: i32) -> bool {
        for _ in 0..3000 {
            if s.player.hp <= 0 {
                return false;
            }
            let (px, py) = (s.player.x, s.player.y);
            if px == tx && py == ty {
                return true;
            }
            let zone_before = s.current;
            match next_step_through_foes(s, tx, ty) {
                Some((nx, ny)) => {
                    s.move_player(nx - px, ny - py);
                    // Walking over a waystone (or bumping a vault door)
                    // opens a prompt; this walker always steps away.
                    s.close_prompt();
                }
                None => return false,
            }
            if s.current != zone_before {
                // Zone changed mid-walk (stepped on stairs): caller re-checks.
                return s.player.x == tx && s.player.y == ty;
            }
        }
        false
    }

    fn find_tile(s: &GameSession, kind: TileKind) -> Option<(i32, i32)> {
        s.zone()
            .tiles
            .iter()
            .flatten()
            .find(|t| t.kind == kind)
            .map(|t| (t.x, t.y))
    }

    #[test]
    fn run1_gate_flow_end_to_end() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 42);
        // Test-only buff: this test proves flow logic, not balance.
        s.player.attributes.might = 40;
        s.player.hp = 5000;
        assert_eq!(s.current, ZoneId::Oakhaven);

        // Oakhaven -> Floor 1 via the descent tile.
        let down = find_tile(&s, TileKind::StairsDown)
            .or_else(|| find_tile(&s, TileKind::GateCity))
            .expect("oakhaven has a descent tile");
        walk_to(&mut s, down.0, down.1);
        if s.current == ZoneId::Oakhaven {
            // Stepping adjacent may be required if the tile itself is the gate.
            s.move_player(0, 1);
        }
        assert_eq!(s.current, ZoneId::Dungeon(1), "should be on Floor 1, log: {:?}", s.log);

        // Collect all 4 town items from ground loot.
        let mut guard = 0;
        loop {
            guard += 1;
            assert!(guard < 12, "too many loot trips");
            if s.carried_town_tags().len() >= 4 {
                break;
            }
            let target = s
                .zone()
                .loot
                .iter()
                .find(|l| {
                    l.item
                        .as_ref()
                        .map(|i| is_town_item_tag(i.quest_tag.as_deref()))
                        .unwrap_or(false)
                })
                .map(|l| (l.x, l.y));
            let (tx, ty) = target.expect("a town item remains on Floor 1");
            assert!(walk_to(&mut s, tx, ty), "walk to town item at {tx},{ty}");
            s.player.hp = 5000; // top up after any fights (flow test)
        }
        assert_eq!(s.carried_town_tags().len(), 4);

        // Return alive via stairs up -> banking happens.
        let up = find_tile(&s, TileKind::StairsUp).expect("floor 1 has stairs up");
        walk_to(&mut s, up.0, up.1);
        assert_eq!(s.current, ZoneId::Oakhaven, "back in Oakhaven");
        assert_eq!(s.town.pieces_found.len(), 3, "3 blacksmith pieces banked");
        assert!(s.town.stall_item_found, "heartcoal banked");
        assert!(s.carried_town_tags().is_empty(), "nothing left unbanked");
        assert!(s.can_complete_blacksmith());

        // Rebuild on the pondside lot (Torin's wish) and buy the blade.
        let gold_before = s.player.gold;
        assert!(s.complete_blacksmith(BlacksmithLot::Pondside));
        assert!(s.town.blacksmith_complete);
        assert_eq!(s.player.gold, gold_before + 75, "pondside wish bonus paid once");
        let stock = s.merchant_stock("blacksmith");
        let blade_idx = stock
            .iter()
            .position(|i| i.name == BLACKSMITH_GATED_ITEM_NAME)
            .expect("rebuilt stock carries the gated blade");
        s.player.gold += 1000;
        let bought = s.buy_item("blacksmith", blade_idx).expect("buy blade");
        assert_eq!(bought.name, BLACKSMITH_GATED_ITEM_NAME);
        assert!(s
            .player
            .inventory
            .iter()
            .any(|i| i.name == BLACKSMITH_GATED_ITEM_NAME));

        // Torin now opens his dialogue tree; the trade choice resolves to
        // the rebuilt 'blacksmith' stock (TS merchant resolution rule).
        s.player.x = crate::town_layout::TORIN_SPAWN.x;
        s.player.y = crate::town_layout::TORIN_SPAWN.y + 1;
        assert_eq!(
            s.interact(),
            SessionEvent::OpenDialogue("torin".to_string())
        );
        let dlg = s.dialogue_json();
        assert_eq!(dlg["npcId"], "torin");
        assert_eq!(
            s.choose_dialogue("trade"),
            SessionEvent::OpenMerchant("torin".to_string())
        );
        let resolved = s.merchant_stock("torin");
        assert!(resolved
            .iter()
            .any(|i| i.name == BLACKSMITH_GATED_ITEM_NAME));
    }

    #[test]
    fn dialogue_checks_conditions_and_quest_flow() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 7);
        // Torin skill check: Vanguard might is 3 -> pass to craft_pass.
        s.active_dialogue = Some(("torin".into(), "root".into()));
        assert_eq!(s.choose_dialogue("ask_craft"), SessionEvent::None);
        assert_eq!(s.active_dialogue.as_ref().unwrap().1, "craft_pass");
        // Drop might below the threshold -> fail node, no actions.
        s.active_dialogue = Some(("torin".into(), "root".into()));
        s.player.attributes.might = 1;
        assert_eq!(s.choose_dialogue("ask_craft"), SessionEvent::None);
        assert_eq!(s.active_dialogue.as_ref().unwrap().1, "craft_fail");
        s.close_dialogue();
        assert!(s.active_dialogue.is_none());

        // Mira: quest-offer choice is visible before the quest starts,
        // heal_full restores, and accepting starts herbalist_blight.
        s.active_dialogue = Some(("mira".into(), "root".into()));
        let dlg = s.dialogue_json();
        let labels: Vec<&str> = dlg["choices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect();
        assert!(labels.contains(&"quest_offer"), "offer visible: {labels:?}");
        s.player.hp = 1;
        s.player.mana = 0;
        assert_eq!(s.choose_dialogue("heal_free"), SessionEvent::None);
        assert_eq!(s.active_dialogue.as_ref().unwrap().1, "healed");
        assert_eq!(s.player.hp, s.player.max_hp);
        assert_eq!(s.player.mana, s.player.max_mana);

        s.active_dialogue = Some(("mira".into(), "root".into()));
        assert_eq!(s.choose_dialogue("quest_offer"), SessionEvent::None);
        assert_eq!(s.active_dialogue.as_ref().unwrap().1, "blight_intro");
        assert_eq!(s.choose_dialogue("accept_blight"), SessionEvent::None);
        assert!(crate::npcs::is_quest_active(&s.quests, "herbalist_blight"));
        // Offer is now hidden (quest started).
        s.active_dialogue = Some(("mira".into(), "root".into()));
        let dlg = s.dialogue_json();
        let labels: Vec<&str> = dlg["choices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect();
        assert!(!labels.contains(&"quest_offer"));
    }

    #[test]
    fn travel_graph_gates_portals_and_supply_banking() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 11);
        s.player.hp = 5000;

        // Oakhaven west gate (0,7) -> Overworld landing (2,12).
        s.player.x = 1;
        s.player.y = 7;
        s.move_player(-1, 0);
        assert_eq!(s.current, ZoneId::Overworld, "gate to overworld");
        assert_eq!((s.player.x, s.player.y), (2, 12));

        // Highspire gate is sealed without a Citadel Pass.
        s.player.x = 46;
        s.player.y = 12;
        s.move_player(1, 0);
        assert_eq!(s.current, ZoneId::Overworld, "highspire sealed");
        assert!(s.log.iter().any(|l| l.contains("Causeway Gate is sealed")));
        s.flags.citadel_access = true;
        s.move_player(-1, 0); // step off the gate, then back on
        s.move_player(1, 0);
        assert_eq!(s.current, ZoneId::Highspire, "citadel pass opens highspire");
        // Back out through Highspire's gate -> overworld (45,12).
        let gate = find_tile(&s, TileKind::GateCity).expect("highspire has a gate");
        s.player.x = gate.0 + 1;
        s.player.y = gate.1;
        s.move_player(-1, 0);
        assert_eq!(s.current, ZoneId::Overworld);
        assert_eq!((s.player.x, s.player.y), (45, 12));

        // Barrow portal (10,4) -> barrow_1; its stairs_up exits to (10,5)
        // on the overworld WITHOUT banking anything.
        s.player.x = 10;
        s.player.y = 5;
        s.move_player(0, -1);
        assert_eq!(s.current, ZoneId::Barrow(1), "portal to barrow_1");
        let up = find_tile(&s, TileKind::StairsUp).expect("barrow_1 has stairs up");
        // Stand next to the stairs and step on.
        s.player.x = up.0;
        s.player.y = up.1 + 1;
        s.move_player(0, -1);
        assert_eq!(s.current, ZoneId::Overworld, "barrow exit to overworld");
        assert_eq!((s.player.x, s.player.y), (10, 5));

        // Carry a Barrow Iron supply home: the Oakhaven gate banks it.
        let supply = crate::world::create_quest_item_local("barrow_iron", 999);
        s.player.inventory.push(supply);
        s.player.x = 1;
        s.player.y = 12;
        s.move_player(-1, 0);
        assert_eq!(s.current, ZoneId::Oakhaven, "gate home");
        assert!(
            s.town
                .supplies_found
                .contains(&crate::town::TownSupplyTag::BarrowIron),
            "barrow_iron banked: {:?}",
            s.town.supplies_found
        );
        assert!(s.carried_town_tags().is_empty(), "supply left the pack");
    }
}
