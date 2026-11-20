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
//! - The rebuilt Blacksmith stock is built here from loot.rs constructors;
//!   the full merchant tables live in npcs.ts (M3).
//! - Wake in Oakhaven heals to full; the TS gold death toll is Normal-mode
//!   App logic (M3).

use crate::combat::{calculate_player_hit, choose_enemy_intent, find_path_next_step};
use crate::difficulty::DifficultyTier;
use crate::loot::{create_consumable, create_starter_item, ItemIdGen};
use crate::rng::Rng;
use crate::skills::{compute_derived_stats, get_starter_preset};
use crate::town::{
    bank_carried_town_items, can_complete_blacksmith, complete_blacksmith, create_town_state,
    is_town_item_tag, relocate_blacksmith, split_unbanked_town_items, town_return_tick,
    BlacksmithLot, TownReturnOptions, TownState, BLACKSMITH_GATED_ITEM_NAME,
};
use crate::types::*;
use crate::world::{create_oakhaven_map, generate_dungeon_floor};
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
}

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
            log: vec![],
            delve_progress: 0,
            kills_this_delve: 0,
            seq: 1000,
        };
        s.say("Welcome to Oakhaven. The Reliquary below holds what the town needs.");
        s
    }

    fn say(&mut self, text: &str) {
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
        if self.player.hp <= 0 {
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
            return self.bump_attack(idx);
        }
        let tile = match self.tile_at(nx, ny) {
            Some(t) => t,
            None => return SessionEvent::None,
        };
        if !tile.walkable {
            return SessionEvent::None;
        }
        // NPCs block movement.
        if self.zone().npcs.iter().any(|n| n.x == nx && n.y == ny) {
            return SessionEvent::None;
        }
        self.player.x = nx;
        self.player.y = ny;
        self.player.turns_elapsed += 1;
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
            TileKind::GateCity => return self.transition_down(),
            _ => {}
        }
        self.enemy_turn();
        if self.player.hp <= 0 {
            return self.wake_in_oakhaven();
        }
        SessionEvent::None
    }

    fn bump_attack(&mut self, idx: usize) -> SessionEvent {
        let stats = self.derived();
        let target = self.zone().enemies[idx].clone();
        let hit = calculate_player_hit(
            &self.player,
            &target,
            1.0,
            false,
            false,
            0.0,
            &stats,
            &mut self.rng,
        );
        self.player.turns_elapsed += 1;
        if !self.in_town() {
            self.delve_progress += 1;
        }
        let mut event = SessionEvent::None;
        {
            let zone = self.zone_mut();
            zone.enemies[idx].hp -= hit.damage;
        }
        if hit.is_crit {
            self.say(&format!("Critical! You hit {} for {}.", target.name, hit.damage));
        } else {
            self.say(&format!("You hit {} for {}.", target.name, hit.damage));
        }
        if hit.lifesteal_heal > 0 {
            self.player.hp = (self.player.hp + hit.lifesteal_heal).min(stats.max_hp);
        }
        let dead = self.zone().enemies[idx].hp <= 0;
        if dead {
            event = self.on_enemy_killed(idx);
        } else {
            self.enemy_turn();
        }
        if self.player.hp <= 0 {
            return self.wake_in_oakhaven();
        }
        event
    }

    fn on_enemy_killed(&mut self, idx: usize) -> SessionEvent {
        let enemy = self.zone_mut().enemies.remove(idx);
        self.player.kills += 1;
        self.kills_this_delve += 1;
        self.delve_progress += 2;
        self.player.gold += enemy.gold_reward;
        self.say(&format!("{} falls. +{} gold.", enemy.name, enemy.gold_reward));
        // XP with difficulty scaling + level-ups (TS curve: xpToNext x1.48).
        let xp = crate::difficulty::scale_xp_for_difficulty(enemy.xp_reward, self.tier, self.cycle);
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
            event = SessionEvent::LevelUp(self.player.level);
        }
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

    /// Every living enemy in the current zone takes its turn.
    fn enemy_turn(&mut self) {
        let stats = self.derived();
        let (px, py) = (self.player.x, self.player.y);
        let count = self.zone().enemies.len();
        for i in 0..count {
            if self.player.hp <= 0 {
                break;
            }
            let enemy = self.zone().enemies[i].clone();
            if enemy.hp <= 0 || enemy.pacified == Some(true) {
                continue;
            }
            let dist = (enemy.x - px).abs().max((enemy.y - py).abs());
            let sight = 8;
            if dist > sight {
                continue;
            }
            // Telegraph resolution / intent choice.
            if let Some(intent) = enemy.intent.clone() {
                if intent.target_tiles.iter().any(|t| t.x == px && t.y == py) {
                    let dmg = (intent.damage - (stats.def as f64 * 0.65).round() as i32).max(1);
                    self.player.hp -= dmg;
                    self.say(&format!("{} lands {} on you for {}.", enemy.name, intent.label, dmg));
                }
                self.zone_mut().enemies[i].intent = None;
                continue;
            }
            if dist <= 1 {
                // Choose intent (may telegraph a heavy hit for next turn).
                let zone_snapshot = self.zone().clone();
                let intent =
                    choose_enemy_intent(&enemy, &self.player, &zone_snapshot, &mut self.rng);
                match intent {
                    Some(it) if it.kind == IntentType::HeavyWindup => {
                        self.say(&format!("{} winds up: {}!", enemy.name, it.label));
                        self.zone_mut().enemies[i].intent = Some(it);
                    }
                    _ => {
                        let dmg = (enemy.atk - (stats.def as f64 * 0.65).round() as i32).max(1);
                        self.player.hp -= dmg;
                        self.say(&format!("{} hits you for {}.", enemy.name, dmg));
                    }
                }
            } else {
                // Step toward the player.
                let zone_snapshot = self.zone().clone();
                if let Some(step) = find_path_next_step(&zone_snapshot, enemy.x, enemy.y, px, py) {
                    let occupied = self
                        .zone()
                        .enemies
                        .iter()
                        .any(|e| e.hp > 0 && e.x == step.x && e.y == step.y);
                    if !occupied && !(step.x == px && step.y == py) {
                        let e = &mut self.zone_mut().enemies[i];
                        e.x = step.x;
                        e.y = step.y;
                        e.alerted = true;
                    }
                }
            }
        }
    }

    fn transition_down(&mut self) -> SessionEvent {
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
            ZoneId::Dungeon(1) | ZoneId::Barrow(1) | ZoneId::Mire(1) | ZoneId::Frost(1) => {
                self.return_to_oakhaven(false)
            }
            ZoneId::Dungeon(n) => self.enter_zone(ZoneId::Dungeon(n - 1), false),
            ZoneId::Barrow(n) => self.enter_zone(ZoneId::Barrow(n - 1), false),
            ZoneId::Mire(n) => self.enter_zone(ZoneId::Mire(n - 1), false),
            ZoneId::Frost(n) => self.enter_zone(ZoneId::Frost(n - 1), false),
            _ => SessionEvent::None,
        }
    }

    fn enter_zone(&mut self, id: ZoneId, down: bool) -> SessionEvent {
        let key = id.to_string_id();
        if !self.zones.contains_key(&key) {
            let zone = match id {
                ZoneId::Oakhaven => create_oakhaven_map(),
                ZoneId::Dungeon(n) => {
                    generate_dungeon_floor(n as i32, &mut self.rng, self.tier, self.cycle)
                }
                // Side branches are M3; generate as main-descent floors of equal depth for now.
                ZoneId::Barrow(n) | ZoneId::Mire(n) | ZoneId::Frost(n) => {
                    generate_dungeon_floor(n as i32, &mut self.rng, self.tier, self.cycle)
                }
                _ => create_oakhaven_map(),
            };
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
        self.player.x = entry.x;
        self.player.y = entry.y;
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
        let d = self.derived();
        self.player.hp = d.max_hp;
        self.player.max_hp = d.max_hp;
        self.say("You wake in Oakhaven, bruised but alive. The town endures.");
        SessionEvent::PlayerDied
    }

    /// Interact with an adjacent NPC or the rebuilt Blacksmith lot.
    pub fn interact(&mut self) -> SessionEvent {
        let (px, py) = (self.player.x, self.player.y);
        let npc = self
            .zone()
            .npcs
            .iter()
            .find(|n| (n.x - px).abs().max((n.y - py).abs()) <= 1)
            .map(|n| n.id.clone());
        if let Some(npc_id) = npc {
            self.flags.npc_met.insert(npc_id.clone(), true);
            if npc_id == "torin" && self.town.blacksmith_complete {
                return SessionEvent::OpenMerchant("blacksmith".into());
            }
            return SessionEvent::OpenDialogue(npc_id);
        }
        if self.in_town()
            && self.town.blacksmith_complete
            && crate::town_layout::is_near_blacksmith_lot(px, py, Some(&self.town))
        {
            return SessionEvent::OpenMerchant("blacksmith".into());
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
            self.build_blacksmith_stock();
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

    /// Minimal rebuilt-Blacksmith stock (full tables are npcs.ts, M3).
    fn build_blacksmith_stock(&mut self) {
        let mut stock = vec![
            create_consumable(&mut self.id_gen, ConsumableKind::HealPotion, 1),
            create_consumable(&mut self.id_gen, ConsumableKind::ManaPotion, 1),
        ];
        // The gated blade (full named-item table is npcs.ts, M3; stats here
        // are slice values for a level-1 town-tier weapon).
        let blade = Item {
            id: self.id_gen.next_id(),
            name: BLACKSMITH_GATED_ITEM_NAME.to_string(),
            category: ItemCategory::Equipment,
            slot: Some(ItemSlot::Weapon),
            consumable_kind: None,
            rarity: ItemRarity::Rare,
            glyph: "†".to_string(),
            level_req: 1,
            upgrade_level: 0,
            stats: ItemStats {
                atk: Some(6),
                ..Default::default()
            },
            affix: None,
            affix_label: None,
            set_id: None,
            value: 150,
            description: "Forged in the rebuilt Oakhaven smithy; the edge holds a pond-blue sheen.".to_string(),
            quantity: None,
            quest_tag: None,
        };
        stock.push(blade);
        self.merchant_stocks.insert("blacksmith".into(), stock);
    }

    pub fn merchant_stock(&mut self, merchant_id: &str) -> Vec<Item> {
        if merchant_id == "blacksmith" && self.town.blacksmith_complete && !self.merchant_stocks.contains_key("blacksmith") {
            self.build_blacksmith_stock();
        }
        self.merchant_stocks.get(merchant_id).cloned().unwrap_or_default()
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
                "enemies": zone.enemies.iter().filter(|e| e.hp > 0).map(|e| json!({
                    "id": e.id, "name": e.name, "glyph": e.glyph, "x": e.x, "y": e.y,
                    "hp": e.hp, "maxHp": e.max_hp, "isBoss": e.is_boss.unwrap_or(false),
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
            "carriedTownTags": self.carried_town_tags(),
            "log": self.log.iter().rev().take(8).rev().cloned().collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::find_path_next_step;

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

        // Torin opens the rebuilt shop directly once complete.
        s.player.x = crate::town_layout::TORIN_SPAWN.x;
        s.player.y = crate::town_layout::TORIN_SPAWN.y + 1;
        assert_eq!(
            s.interact(),
            SessionEvent::OpenMerchant("blacksmith".to_string())
        );
    }
}
