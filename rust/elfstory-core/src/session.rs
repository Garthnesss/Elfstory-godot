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
    /// New game at a chosen difficulty (title screen). Difficulty only
    /// scales foes generated after this point and XP, so setting it right
    /// after creation is equivalent to creating with it.
    pub fn new_game_with_difficulty(name: &str, starter_class: StarterClass, seed: u32, tier: DifficultyTier) -> Self {
        let mut s = Self::new_game(name, starter_class, seed);
        s.tier = tier;
        s
    }

    pub fn new_game(name: &str, starter_class: StarterClass, seed: u32) -> Self {
        let preset = get_starter_preset(starter_class);
        let mut id_gen = ItemIdGen::new();
        let weapon = create_starter_item(&mut id_gen, &preset.weapon_tag);
        let oakhaven = create_oakhaven_map();
        let spawn = oakhaven.spawn_point;
        let comp_kind = if starter_class == StarterClass::Runecaster {
            CompanionKind::StarlightOwl
        } else {
            CompanionKind::SilverwindLynx
        };
        let comp_pos = crate::combat_turn::place_companion_near(&oakhaven, spawn.x, spawn.y);
        let companion = crate::combat_turn::make_companion(comp_kind, comp_pos.x, comp_pos.y, 1);
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
            companion: Some(companion),
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
        s.say("Welcome to Oakhaven Waystation. Warden Vance holds watch at the breach ahead; the Sunken Reliquary stairs descend into the pit.");
        s.refresh_visibility();
        s
    }

    // Save glue (session_save.rs) for the private bookkeeping fields.
    pub(crate) fn extra_flags_ref(&self) -> &HashMap<String, Value> {
        &self.extra_flags
    }
    pub(crate) fn delve_counters(&self) -> (i32, i32) {
        (self.delve_progress, self.kills_this_delve)
    }
    pub(crate) fn seq_value(&self) -> u32 {
        self.seq
    }
    pub(crate) fn next_seq(&mut self) -> u32 {
        let s = self.seq;
        self.seq += 1;
        s
    }
    pub(crate) fn restore_internals(&mut self, extra: HashMap<String, Value>, delve: i32, kills: i32, seq: u32) {
        self.extra_flags = extra;
        self.delve_progress = delve;
        self.kills_this_delve = kills;
        self.seq = seq;
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
            let new_level = self.player.level;
            if let Some(comp) = self.player.companion.as_mut() {
                let comp_max = crate::combat_turn::companion_max_hp(comp.kind, new_level);
                let delta = comp_max - comp.max_hp;
                comp.max_hp = comp_max;
                if comp.hp > 0 {
                    comp.hp += delta;
                }
            }
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
        // Spirit Companion tile swap when stepping onto an active companion outside town.
        let swap_name = if !self.in_town() {
            self.player
                .companion
                .as_ref()
                .filter(|c| c.hp > 0 && c.respawn_cooldown == 0 && c.x == nx && c.y == ny)
                .map(|c| c.name.clone())
        } else {
            None
        };
        if let Some(cname) = swap_name {
            let (old_x, old_y) = (self.player.x, self.player.y);
            if let Some(c) = self.player.companion.as_mut() {
                c.x = old_x;
                c.y = old_y;
            }
            self.fx("companion", nx, ny);
            self.say(&format!("Swapped places with {cname}."));
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
                self.add_item_to_player(item);
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
            TileKind::Chest
            | TileKind::Brazier
            | TileKind::Waystone
            | TileKind::LoreTablet
            | TileKind::ContrabandCache
            | TileKind::HerbNode
            | TileKind::WhisperingBell
            | TileKind::ForgottenForge
            | TileKind::CursedAltar => {
                if let Some(ev) = self.interact_with_world_tile(px, py) {
                    return ev;
                }
            }
            TileKind::Trap
            | TileKind::DartTrap
            | TileKind::SporePool
            | TileKind::FrostVent
            | TileKind::EmberCrust
            | TileKind::ShockRune => {
                self.trigger_hazard(px, py, kind);
                let max_hp = self.derived().max_hp;
                if self.check_player_fatal(max_hp) {
                    return self.wake_in_oakhaven();
                }
            }
            _ => {}
        }
        if let Some(steps) = self.player.haste_steps.filter(|&s| s > 0) {
            self.player.haste_steps = Some(steps - 1);
            self.refresh_visibility();
            self.fx("haste", px, py);
            return self.check_boss_confrontations().unwrap_or(SessionEvent::None);
        }
        self.run_enemy_turns()
    }

    pub(crate) fn on_enemy_killed(&mut self, idx: usize) -> SessionEvent {
        // Grave Husk "Deathless" — first lethal blow revives it at 30% HP
        // with no kill credit or loot.
        let is_deathless = {
            let e = &self.zone().enemies[idx];
            e.archetype == EnemyArchetype::GraveHusk && e.undying_used != Some(true)
        };
        if is_deathless {
            let (ex, ey, revived_hp) = {
                let e = &mut self.zone_mut().enemies[idx];
                e.hp = 1.max((e.max_hp as f64 * 0.3).round() as i32);
                e.undying_used = Some(true);
                e.alerted = true;
                e.intent = None;
                (e.x, e.y, e.hp)
            };
            self.fx("deathless", ex, ey);
            self.say(&format!(
                "The Grave Husk refuses death — burial oaths knit it back together ({revived_hp} HP)!"
            ));
            return SessionEvent::None;
        }

        let enemy = self.zone_mut().enemies.remove(idx);
        if !enemy.name.starts_with("Abyssal Echo") {
            match enemy.archetype {
                EnemyArchetype::BossBoneCantor => self.flags.cantor_defeated = Some(true),
                EnemyArchetype::BossAshenInquisitor => self.flags.inquisitor_defeated = Some(true),
                EnemyArchetype::BossMalakor => self.flags.malakor_defeated = Some(true),
                _ => {}
            }
        }
        self.fx("kill", enemy.x, enemy.y);
        self.player.kills += 1;
        self.kills_this_delve += 1;
        self.delve_progress += 2;
        self.player.gold += enemy.gold_reward;
        self.say(&format!("{} falls. +{} gold.", enemy.name, enemy.gold_reward));
        self.kill_passives();

        // Spore Hulk death-burst — point-blank kills detonate poison spores.
        if enemy.archetype == EnemyArchetype::SporeHulk {
            self.fx("spore_burst", enemy.x, enemy.y);
            if crate::combat::chebyshev(self.player.x, self.player.y, enemy.x, enemy.y) <= 1 {
                let burst_dmg = 6 + self.zone().depth * 2;
                self.player.hp -= burst_dmg;
                crate::combat::apply_status(&mut self.player.statuses, StatusKind::Poison, 3, 3);
                self.fx("player_hit", self.player.x, self.player.y);
                self.say(&format!(
                    "The Spore Hulk BURSTS for {burst_dmg} dmg and Poison (3t)!"
                ));
                let max_hp = self.derived().max_hp;
                let _ = self.check_player_fatal(max_hp);
            } else {
                self.say("The Spore Hulk bursts in a green cloud — you kept your distance.");
            }
        }

        // XP with difficulty scaling + level-ups (TS curve: xpToNext x1.48).
        let xp = crate::difficulty::scale_xp_for_difficulty(enemy.xp_reward, self.tier, self.cycle);
        let event = self.gain_xp(xp);
        for drop in self.roll_kill_drops(&enemy) {
            let seq = self.next_seq();
            self.zone_mut().loot.push(GroundLoot {
                id: format!("drop_{seq}"),
                x: enemy.x,
                y: enemy.y,
                gold: None,
                item: Some(drop),
            });
        }
        self.claim_boss_trophy(&enemy);
        event
    }

    /// TS handleEnemyDeath drop table: wardens and bosses pay premium loot,
    /// ordinary kills roll, and a young hero's first kill always drops gear.
    fn roll_kill_drops(&mut self, enemy: &Enemy) -> Vec<Item> {
        use crate::loot::{create_random_set_relic, generate_kill_drop, generate_procedural_item, ProceduralOptions};
        let depth = self.zone().depth;
        let premium = |gen: &mut ItemIdGen, rng: &mut Rng, d: i32, min: ItemRarity| {
            generate_procedural_item(gen, rng, d, ProceduralOptions { slot: None, min_rarity: Some(min) })
        };
        let (gen, rng) = (&mut self.id_gen, &mut self.rng);
        match enemy.rank {
            Some(EnemyRank::Randboss) => {
                let first = if rng.next_f32() < 0.4 {
                    create_random_set_relic(gen, rng, depth)
                } else {
                    premium(gen, rng, depth + 1, ItemRarity::Epic)
                };
                return vec![first, premium(gen, rng, depth, ItemRarity::Rare)];
            }
            Some(EnemyRank::Rare) => return vec![premium(gen, rng, depth + 1, ItemRarity::Rare)],
            _ => {}
        }
        if enemy.is_boss == Some(true) {
            return vec![if rng.next_f32() < 0.35 {
                create_random_set_relic(gen, rng, depth)
            } else {
                premium(gen, rng, depth, ItemRarity::Epic)
            }];
        }
        if enemy.is_elite == Some(true) {
            return vec![premium(gen, rng, depth, ItemRarity::Rare)];
        }
        let first_done = self.flags.first_gear_dropped == Some(true);
        let guaranteed = crate::loot::is_first_gear_guaranteed(self.player.level, first_done);
        match generate_kill_drop(gen, rng, depth, self.player.level, first_done) {
            None => vec![],
            Some(gear) => {
                if guaranteed {
                    self.flags.first_gear_dropped = Some(true);
                    vec![gear]
                } else if self.rng.next_f32() < 0.18 {
                    let kind = if self.rng.next_f32() < 0.6 { ConsumableKind::HealPotion } else { ConsumableKind::ManaPotion };
                    vec![create_consumable(&mut self.id_gen, kind, 1)]
                } else {
                    vec![gear]
                }
            }
        }
    }

    /// Side bosses yield the trophy their quest asks for; Malakor ends the story.
    fn claim_boss_trophy(&mut self, enemy: &Enemy) {
        let (quest, tag, log, line) = match enemy.id.as_str() {
            "boss_morvath" => {
                self.flags.barrow_boss_defeated = Some(true);
                ("barrow_purge", "morvath_seal", "Slew Arch-Wight Morvath and claimed Morvath’s Barrow Seal.",
                 "Obtained Morvath’s Barrow Seal! Return to Captain Darek in Cinderwatch Bastion.")
            }
            "boss_vexis" => {
                self.flags.mire_boss_defeated = Some(true);
                ("spores_of_bog", "fungal_heart", "Slew Spore-Matron Vexis and harvested the Bioluminescent Fungal Heart.",
                 "Harvested Bioluminescent Fungal Heart! Deliver it to Elder Sylva in Thornwick Fen.")
            }
            "boss_kaelthar" => {
                self.flags.frost_boss_defeated = Some(true);
                ("cinder_and_frost", "ashen_ore_heart", "Slew Ignis-Warden Kaelthar and tore free the Ashen Ore Heart.",
                 "Recovered Ashen Ore Heart! Bring it to Forgemistress Halda in Cinderwatch Bastion.")
            }
            _ => {
                if enemy.archetype == EnemyArchetype::BossMalakor && !enemy.name.starts_with("Abyssal Echo") {
                    let mut history = self.quests.get("sovereign_cinder").map(|q| q.history.clone()).unwrap_or_default();
                    history.push("Shattered Malakor, Sovereign of Cinder, on Floor 5.".into());
                    self.quests.insert(
                        "sovereign_cinder".into(),
                        QuestProgress {
                            id: "sovereign_cinder".into(),
                            status: QuestStatus::Completed,
                            stage: 2,
                            outcome_label: Some("Malakor Vanquished".into()),
                            history,
                        },
                    );
                }
                return;
            }
        };
        let trophy = crate::loot::create_quest_item(&mut self.id_gen, tag);
        self.add_item_to_player(trophy);
        if let Some(q) = self.quests.get_mut(quest) {
            if q.status == QuestStatus::Active {
                q.stage = q.stage.max(2);
                q.history.push(log.into());
            }
        }
        self.say(line);
    }

    /// True from Malakor's fall until the ending has been shown once.
    pub fn ending_pending(&self) -> bool {
        self.flags.malakor_defeated == Some(true) && self.extra_flags.get("endingSeen") != Some(&Value::Bool(true))
    }

    pub fn acknowledge_ending(&mut self) {
        self.extra_flags.insert("endingSeen".into(), Value::Bool(true));
    }

    /// TS EndingScreen: the epilogue reflects the run's choices.
    pub fn ending_json(&self) -> Value {
        let f = &self.flags;
        let mut lines = vec!["With Malakor unmade, the cinder-throne cools for the first time in an age. The Ember Blight gutters out. Somewhere far above, Oakhaven feels the change, looks around, and reluctantly concludes that Garth did it."];
        match f.bell_choice.as_deref() {
            Some("attuned_lyra") => lines.push("In Highspire, Archivist Lyra rings the attuned Whispering Bell once at dawn. Its note carries over the Vale: a ward renewed, a promise kept. She files your name under “Heroes (Unexpected).”"),
            Some("sold_corvus") => lines.push("The Whispering Bell hangs silent in Magistrate Corvus’s vault. What he paid for it buys roads, roofs, and quiet. He sends you an invoice for the paperwork anyway."),
            Some("shattered_self") => lines.push("The Bell you shattered sings on inside you — a low cinder-hum beneath your heartbeat. You tell everyone it is the sound of destiny. It is mostly indigestion."),
            _ => {}
        }
        match f.caravan_choice.as_deref() {
            Some("arrested_vex") => lines.push("The Contraband Ledger, delivered to Warden Vance, closes the smuggler roads beneath Oakhaven. Vance shakes your hand, which he has never done, and immediately regrets it, which he has always done."),
            Some("smuggler_pact") => lines.push("Your shadow pact with Vex the Quiet holds: certain doors in Oakhaven will never close to you again. Vex still won’t say your name out loud. Nobody can say it with a straight face."),
            _ => {}
        }
        match f.blight_choice.as_deref() {
            Some("cantor_appeased") => lines.push("Vaelen, the Bone Cantor, was laid to rest by words rather than steel. His last request was that you never sing. You have honoured it, mostly."),
            Some("herbs_gathered") => lines.push("Mira’s apothecary, restocked with Pale Nightbloom, tends the blight-sick of Oakhaven back to health. She names a wart cream after you. It sells well."),
            _ => {}
        }
        if f.malakor_weakened == Some(true) {
            lines.push("Those who study such things will note the Sovereign’s ward was broken before the final descent — and that you broke it. You will mention this at every dinner for the rest of your life.");
        }
        lines.push("But the stair below Floor 5 does not end. The Endless Abyss waits with its lanterns lit. The town would very much like you to go down there for a while.");
        json!({
            "lines": lines, "level": self.player.level, "kills": self.player.kills,
            "turns": self.player.turns_elapsed, "gold": self.player.gold, "cycle": self.cycle,
            "quests": self.quests.values().filter(|q| q.status == QuestStatus::Completed).count(),
        })
    }

    /// New Game+: the Vale is remade one cycle harder; the hero, gear and
    /// town are kept. Only after the ending.
    pub fn ascend(&mut self) -> bool {
        if self.flags.malakor_defeated != Some(true) {
            return false;
        }
        let seed = (self.rng.next_f32() * 4_000_000_000.0) as u32;
        let fresh = Self::new_game(&self.player.name, self.player.starter_class, seed);
        let mut next = fresh;
        next.tier = self.tier;
        next.cycle = self.cycle + 1;
        next.town = self.town.clone();
        next.id_gen = ItemIdGen::with_counter(self.id_gen.counter());
        let (x, y) = (next.player.x, next.player.y);
        next.player = self.player.clone();
        let p = &mut next.player;
        p.x = x;
        p.y = y;
        // Last cycle's story items would finish this cycle's quests on turn one.
        p.inventory.retain(|i| i.category != ItemCategory::Quest);
        p.statuses.clear();
        p.cooldowns.clear();
        p.prodigy_cooldowns = Some(HashMap::new());
        p.haste_steps = Some(0);
        p.kills = 0;
        p.turns_elapsed = 0;
        p.first_strike_ready = true;
        next.place_companion_on_entry(true);
        if next.town.blacksmith_complete {
            let stock = crate::npcs::create_merchant_stock(&mut next.id_gen, &mut next.rng, "blacksmith", Some(&next.town));
            next.merchant_stocks.insert("blacksmith".into(), stock);
        }
        let d = next.derived();
        next.player.max_hp = d.max_hp;
        next.player.hp = d.max_hp;
        next.player.max_mana = d.max_mana;
        next.player.mana = d.max_mana;
        next.tile_rev = self.tile_rev + 1;
        next.refresh_visibility();
        next.say(&format!("The Ashen Glyph turns once more. You ascend to Cycle {}: the Vale is remade, its foes hardened, and everything earned is carried into the new descent.", next.cycle));
        *self = next;
        true
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

    /// Dev/capture helper (debug bridge builds only): configure `town` at a
    /// chosen population and optional completed Blacksmith lot for visual verification.
    pub fn debug_set_town(&mut self, pop: i32, lot: Option<BlacksmithLot>) {
        if let Some(target_lot) = lot {
            for p in crate::town::BLACKSMITH_PIECES {
                self.town = crate::town::record_town_item(
                    std::mem::replace(&mut self.town, create_town_state()),
                    p.tag,
                );
            }
            self.town = crate::town::record_town_item(
                std::mem::replace(&mut self.town, create_town_state()),
                crate::town::STALL_ITEM_TAG,
            );
            for s in crate::town::TOWN_SUPPLIES {
                self.town = crate::town::record_town_item(
                    std::mem::replace(&mut self.town, create_town_state()),
                    s.tag.as_str(),
                );
            }
            let _ = self.complete_blacksmith(target_lot);
        }
        self.town.population = pop;
        if pop > crate::town::BASE_POPULATION && self.town.returns == 0 {
            self.town.returns = 4;
        }
        if self.town.blacksmith_complete {
            let stock = crate::npcs::create_merchant_stock(
                &mut self.id_gen,
                &mut self.rng,
                "blacksmith",
                Some(&self.town),
            );
            self.merchant_stocks.insert("blacksmith".into(), stock);
        }
        let d = self.derived();
        self.player.max_hp = d.max_hp;
        self.player.hp = d.max_hp;
        self.player.max_mana = d.max_mana;
        self.player.mana = d.max_mana;
        self.fx.clear();
    }

    fn place_companion_on_entry(&mut self, heal: bool) {
        let (px, py) = (self.player.x, self.player.y);
        let pos = crate::combat_turn::place_companion_near(self.zone(), px, py);
        if let Some(c) = self.player.companion.as_mut() {
            if heal {
                c.hp = c.max_hp;
                c.respawn_cooldown = 0;
            }
            c.x = pos.x;
            c.y = pos.y;
        }
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
        let heal_comp = self.in_town();
        self.place_companion_on_entry(heal_comp);
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
        let prev_tier = crate::town::population_tier(&self.town);
        let prev_returns = self.town.returns;
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
            if banked.banked_tags.iter().any(|t| t == crate::town::STALL_ITEM_TAG) {
                self.say("Heartcoal banked in Oakhaven! Torin's forge stall is cleared and town growth resumes.");
            }
            for supply in crate::town::TOWN_SUPPLIES {
                if banked.banked_tags.iter().any(|t| t == supply.tag.as_str()) {
                    self.say(&format!(
                        "Banked {} in Oakhaven! +4 Population and {} unlocked at Torin's Blacksmith.",
                        supply.name, supply.unlock_item_name
                    ));
                    if self.town.blacksmith_complete {
                        let stock = crate::npcs::create_merchant_stock(
                            &mut self.id_gen,
                            &mut self.rng,
                            "blacksmith",
                            Some(&self.town),
                        );
                        self.merchant_stocks.insert("blacksmith".into(), stock);
                    }
                }
            }
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
        let next_tier = crate::town::population_tier(&self.town);
        if self.town.returns > prev_returns {
            let tier_note = if next_tier > prev_tier {
                format!(" (Town Tier {next_tier} reached!)")
            } else {
                String::new()
            };
            self.say(&format!(
                "Oakhaven stirs as you return — two more souls call it home. Population {}{tier_note}.",
                self.town.population
            ));
        } else if progress > 0 && !self.town.stall_item_found {
            self.say("Oakhaven's growth is stalled while Torin's forge lies cold — bring Heartcoal from Floor 1 to resume population growth.");
        }
        self.delve_progress = 0;
        self.kills_this_delve = 0;
        self.current = ZoneId::Oakhaven;
        let spawn = self.zone().spawn_point;
        self.player.x = spawn.x;
        self.player.y = spawn.y;
        self.place_companion_on_entry(true);
        if next_tier > prev_tier {
            let d = self.derived();
            self.player.max_hp = d.max_hp;
            self.player.max_mana = d.max_mana;
            self.fx("town_tier_up", spawn.x, spawn.y);
        }
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
        self.place_companion_on_entry(true);
        // TS respawn reset: statuses, cooldowns, haste and prodigy cooldowns clear.
        self.player.statuses.clear();
        self.player.cooldowns.clear();
        self.player.haste_steps = Some(0);
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
        let prev_tier = crate::town::population_tier(&self.town);
        let result = complete_blacksmith(
            std::mem::replace(&mut self.town, create_town_state()),
            lot,
        );
        self.town = result.state;
        if result.completed {
            let anchor = crate::town_layout::oakhaven_lot_anchor(lot);
            self.fx("town_build", anchor.x.round() as i32, anchor.z.round() as i32);
            if result.wish_bonus_gold > 0 {
                self.player.gold += result.wish_bonus_gold;
                self.say(&format!(
                    "Torin presses {} gold into your hand. \"Pond air. Told you. Do NOT spend it all on draughts.\"",
                    result.wish_bonus_gold
                ));
            }
            self.say("Torin's Blacksmith rises again. The rebuilt shop is open — talk to Torin or inspect the rebuilt forge lot to browse its stock!");
            let stock = crate::npcs::create_merchant_stock(
                &mut self.id_gen,
                &mut self.rng,
                "blacksmith",
                Some(&self.town),
            );
            self.merchant_stocks.insert("blacksmith".into(), stock);
            let d = self.derived();
            self.player.max_hp = d.max_hp;
            self.player.max_mana = d.max_mana;
            if crate::town::population_tier(&self.town) > prev_tier {
                let (px, py) = (self.player.x, self.player.y);
                self.fx("town_tier_up", px, py);
            }
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
        if changed {
            let anchor = crate::town_layout::oakhaven_lot_anchor(lot);
            self.fx("town_relocate", anchor.x.round() as i32, anchor.z.round() as i32);
            if result.wish_bonus_gold > 0 {
                self.player.gold += result.wish_bonus_gold;
                self.say("Torin's wish fulfilled: +75 gold. \"Now that's proper air for quenching steel.\"");
            } else {
                self.say(&format!("Relocated the rebuilt Blacksmith to the {}.", anchor.label));
            }
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
        // One of each: the shelf used to refill itself for free.
        let shelf = self.resolve_merchant(merchant_id);
        if let Some(stock) = self.merchant_stocks.get_mut(&shelf) {
            if index < stock.len() {
                stock.remove(index);
            }
        }
        self.add_item_to_player(item.clone());
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

    fn has_quest_item(&self, tag: &str) -> bool {
        self.player
            .inventory
            .iter()
            .any(|i| i.quest_tag.as_deref() == Some(tag))
    }

    /// Per-NPC role/quest markers ("turnin" | "offer") for overhead badges.
    pub fn npc_markers_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        let has_ledger = self.has_quest_item("smuggler_ledger");
        let has_herb = self.has_quest_item("pale_nightbloom");
        let has_bell = self.has_quest_item("whispering_bell");
        let has_satchel = self.has_quest_item("wayfarer_satchel");
        let has_bog_heart = self.has_quest_item("fungal_heart");
        let has_barrow_seal = self.has_quest_item("morvath_seal");
        let has_caldera_heart = self.has_quest_item("ashen_ore_heart");

        let is_active = |id: &str| crate::npcs::is_quest_active(&self.quests, id);
        let not_started = |id: &str| !self.quests.contains_key(id);

        if is_active("shadows_beneath") && has_ledger {
            map.insert("vance".into(), json!("turnin"));
            map.insert("vex".into(), json!("turnin"));
        } else if not_started("shadows_beneath") {
            map.insert("vance".into(), json!("offer"));
            map.insert("vex".into(), json!("offer"));
        }

        if is_active("herbalist_blight")
            && (has_herb || self.flags.cantor_defeated == Some(true))
        {
            map.insert("mira".into(), json!("turnin"));
        } else if not_started("herbalist_blight") {
            map.insert("mira".into(), json!("offer"));
        }

        if is_active("whispering_bell") && has_bell {
            map.insert("lyra".into(), json!("turnin"));
            map.insert("corvus".into(), json!("turnin"));
        } else if not_started("whispering_bell") {
            map.insert("lyra".into(), json!("offer"));
            map.insert("corvus".into(), json!("offer"));
        }

        if is_active("sovereign_cinder") && self.flags.malakor_defeated == Some(true) {
            map.insert("kaelen".into(), json!("turnin"));
        } else if not_started("sovereign_cinder") {
            map.insert("kaelen".into(), json!("offer"));
        }

        if is_active("silverwood_road") && has_satchel {
            map.insert("rowan".into(), json!("turnin"));
        } else if not_started("silverwood_road") {
            map.insert("rowan".into(), json!("offer"));
        }

        if is_active("spores_of_bog") && has_bog_heart {
            map.insert("sylva".into(), json!("turnin"));
        } else if not_started("spores_of_bog") {
            map.insert("sylva".into(), json!("offer"));
        }

        if is_active("barrow_purge") && has_barrow_seal {
            map.insert("darek".into(), json!("turnin"));
        } else if not_started("barrow_purge") {
            map.insert("darek".into(), json!("offer"));
        }

        if is_active("cinder_and_frost") && has_caldera_heart {
            map.insert("halda".into(), json!("turnin"));
        } else if not_started("cinder_and_frost") {
            map.insert("halda".into(), json!("offer"));
        }

        Value::Object(map)
    }

    /// Compact quest-tracker rows for the top-right HUD.
    pub fn quest_tracker_json(&self) -> Value {
        let mut items: Vec<Value> = Vec::new();
        for (qid, prog) in &self.quests {
            if prog.status != QuestStatus::Active {
                continue;
            }
            let title = crate::npcs::quest_def(qid)
                .map(|d| d.title)
                .unwrap_or(qid.as_str());
            let (status, target_npc, hint): (&str, &str, String) = match qid.as_str() {
                "shadows_beneath" => {
                    if self.has_quest_item("smuggler_ledger") {
                        (
                            "turnin",
                            "Warden Vance / Vex",
                            "Return Sealed Ledger to Vance (breach) or Vex (SW alcove)".into(),
                        )
                    } else {
                        (
                            "active",
                            "Sunken Reliquary Fl.1",
                            "Find the Sealed Contraband Ledger on Floor 1".into(),
                        )
                    }
                }
                "herbalist_blight" => {
                    if self.has_quest_item("pale_nightbloom")
                        || self.flags.cantor_defeated == Some(true)
                    {
                        (
                            "turnin",
                            "Herbalist Mira",
                            "Report back to Herbalist Mira (NE terrace)".into(),
                        )
                    } else {
                        (
                            "active",
                            "Reliquary Fl.1–2",
                            "Gather Pale Nightbloom or quell the Bone Cantor".into(),
                        )
                    }
                }
                "whispering_bell" => {
                    if self.has_quest_item("whispering_bell") {
                        ("turnin", "Lyra / Corvus", "Return the Whispering Bell in Highspire".into())
                    } else {
                        ("active", "Floor 3", "Recover the bronze bell from Ember Vaults (Fl.3)".into())
                    }
                }
                "sovereign_cinder" => {
                    if self.flags.malakor_defeated == Some(true) {
                        ("turnin", "Guildmaster Kaelen", "Report Malakor's fall to Kaelen in Highspire".into())
                    } else {
                        ("active", "Floor 5", "Defeat Malakor in the Obsidian Sanctum (Fl.5)".into())
                    }
                }
                "outrider_oath" => {
                    let count = self.flags.waystones_attuned.unwrap_or(0);
                    if count >= 2 {
                        ("turnin", "Archivist Lyra", "Return to Archivist Lyra in Highspire".into())
                    } else {
                        ("active", "Floors 1–4", format!("Attune Elven Waystones ({count}/2)"))
                    }
                }
                "silverwood_road" => {
                    if self.has_quest_item("wayfarer_satchel") {
                        ("turnin", "Rowan", "Return Rowan's Stolen Satchel".into())
                    } else {
                        ("active", "Silverwood Vale", "Cross Silverbridge & reclaim Rowan's Satchel".into())
                    }
                }
                "spores_of_bog" => {
                    if self.has_quest_item("fungal_heart") {
                        ("turnin", "Elder Sylva", "Return Fungal Heart to Thornwick Fen".into())
                    } else {
                        ("active", "Fungal Hollow", "Slay Spore-Matron Vexis east of Thornwick".into())
                    }
                }
                "barrow_purge" => {
                    if self.has_quest_item("morvath_seal") {
                        ("turnin", "Captain Darek", "Return Barrow Seal to Cinderwatch".into())
                    } else {
                        ("active", "Whispering Barrow", "Slay Arch-Wight Morvath in Whispering Barrow".into())
                    }
                }
                "cinder_and_frost" => {
                    if self.has_quest_item("ashen_ore_heart") {
                        ("turnin", "Forgemistress Halda", "Return Ashen Ore Heart to Cinderwatch".into())
                    } else {
                        ("active", "Frostpeak Cavern", "Slay Ignis-Warden Kaelthar in Frostpeak".into())
                    }
                }
                "deep_survey" => {
                    let count = prog.stage;
                    if count >= 6 {
                        ("turnin", "Rowan", "Return completed survey to Rowan".into())
                    } else {
                        ("active", "Towns & Vaults", format!("Transcribe carved steles ({count}/6)"))
                    }
                }
                _ => ("active", "", prog.history.last().cloned().unwrap_or_default()),
            };
            items.push(json!({
                "id": qid,
                "title": title,
                "targetNpc": target_npc,
                "status": status,
                "hint": hint,
            }));
        }
        items.sort_by(|a, b| {
            let sa = a["status"].as_str().unwrap_or("");
            let sb = b["status"].as_str().unwrap_or("");
            let ra = if sa == "turnin" { 0 } else { 1 };
            let rb = if sb == "turnin" { 0 } else { 1 };
            ra.cmp(&rb).then_with(|| {
                a["id"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["id"].as_str().unwrap_or(""))
            })
        });
        if items.is_empty() {
            items = vec![
                json!({
                    "id": "lead_vance",
                    "title": "Speak with Warden Vance",
                    "targetNpc": "vance",
                    "status": "lead",
                    "hint": "South breach overlook (right ahead)",
                }),
                json!({
                    "id": "lead_mira",
                    "title": "Rest & Draughts",
                    "targetNpc": "mira",
                    "status": "lead",
                    "hint": "Mira (NE terrace) or Candle Shrine (SE)",
                }),
                json!({
                    "id": "lead_descent",
                    "title": "Sunken Reliquary",
                    "targetNpc": "",
                    "status": "lead",
                    "hint": "Descend the stone stairs into the central breach",
                }),
            ];
        }
        Value::Array(items)
    }

    /// `state_json` without the terrain when the caller already has this
    /// zone at this revision. Terrain is most of the snapshot and changes
    /// on a few actions in a hundred.
    pub fn snapshot_json(&self, known_zone: &str, known_rev: u32) -> Value {
        let mut v = self.state_json();
        if known_zone == self.current.to_string_id() && known_rev == self.tile_rev {
            if let Some(z) = v.get_mut("zone").and_then(Value::as_object_mut) {
                z.remove("tiles");
                z.remove("usedTiles");
            }
        }
        v
    }

    /// JSON snapshot for the Godot front-end.
    pub fn state_json(&self) -> Value {
        let zone = self.zone();
        let tiles: Vec<Vec<String>> = zone
            .tiles
            .iter()
            .map(|row| row.iter().map(|t| shown_kind(t).to_string()).collect())
            .collect();
        // Per tile: 0 never seen, 1 remembered, 2 in sight now.
        let sight: Vec<String> = zone
            .tiles
            .iter()
            .map(|row| row.iter().map(|t| if t.visible { '2' } else if t.explored { '1' } else { '0' }).collect())
            .collect();
        let seen = |x: i32, y: i32, need_now: bool| {
            zone.tiles
                .get(y as usize)
                .and_then(|r| r.get(x as usize))
                .is_some_and(|t| t.visible || (!need_now && t.explored))
        };
        let critters: Vec<Value> = zone
            .critters
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "kind": c.kind.as_str(),
                    "name": c.name,
                    "x": c.x,
                    "y": c.y,
                    "visible": seen(c.x, c.y, true),
                })
            })
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
                "sight": sight,
                "tileRev": self.tile_rev,
                "usedTiles": self.used_tiles(),
                "enemies": zone.enemies.iter().filter(|e| e.hp > 0).map(|e| json!({
                    "id": e.id, "name": e.name, "glyph": e.glyph, "x": e.x, "y": e.y,
                    "archetype": e.archetype.as_str(),
                    "hp": e.hp, "maxHp": e.max_hp, "isBoss": e.is_boss.unwrap_or(false),
                    "alerted": e.alerted,
                    "visible": seen(e.x, e.y, true),
                    "statuses": e.statuses.iter().map(|st| json!({
                        "kind": st.kind.as_str(), "turns": st.turns, "potency": st.potency,
                    })).collect::<Vec<_>>(),
                    "intent": e.intent.as_ref().map(|it| json!({
                        "label": it.label, "damage": it.damage, "targetTiles": it.target_tiles,
                    })),
                })).collect::<Vec<_>>(),
                "npcs": zone.npcs.iter().map(|n| json!({
                    "id": n.id, "name": n.name, "title": n.title, "glyph": n.glyph, "x": n.x, "y": n.y,
                    "visible": seen(n.x, n.y, false),
                })).collect::<Vec<_>>(),
                "critters": critters,
                "loot": zone.loot.iter().map(|l| json!({
                    "x": l.x, "y": l.y, "gold": l.gold, "visible": seen(l.x, l.y, false),
                    "itemName": l.item.as_ref().map(|i| i.name.clone()),
                })).collect::<Vec<_>>(),
            },
            "player": serde_json::to_value(&self.player).unwrap_or(Value::Null),
            "derived": serde_json::to_value(self.derived()).unwrap_or(Value::Null),
            "town": serde_json::to_value(&self.town).unwrap_or(Value::Null),
            "populationTier": crate::town::population_tier(&self.town),
            "townTierBonuses": crate::town::get_town_tier_bonuses(Some(&self.town)),
            "canCompleteBlacksmith": self.can_complete_blacksmith(),
            "quests": serde_json::to_value(&self.quests).unwrap_or(Value::Null),
            "questTracker": self.quest_tracker_json(),
            "npcMarkers": self.npc_markers_json(),
            "carriedTownTags": self.carried_town_tags(),
            "endingPending": self.ending_pending(),
            "storyFinished": self.flags.malakor_defeated == Some(true),
            "cycle": self.cycle,
            "difficulty": crate::difficulty::difficulty_info(self.tier).0,
            "prompt": self.active_prompt.as_ref().map(|p| p.kind.as_str()),
            "hotbar": self.hotbar_json(),
            "armedSkill": self.armed_skill.map(|k| k.as_str()),
            "aimedTarget": self.aimed_target,
            "log": self.log.iter().rev().take(8).rev().cloned().collect::<Vec<_>>(),
        })
    }
}

/// What the player sees: an unsprung, unspotted trap looks like floor.
fn shown_kind(t: &Tile) -> &'static str {
    match t.kind {
        TileKind::Trap | TileKind::DartTrap if t.revealed != Some(true) => TileKind::Floor.as_str(),
        k => k.as_str(),
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
        // Torin sells the rebuilt shop's shelf, and the blade bought above is
        // off it: one of each, as in the web game.
        let resolved = s.merchant_stock("torin");
        assert!(!resolved.is_empty());
        assert_eq!(resolved, s.merchant_stock("blacksmith"));
        assert!(!resolved.iter().any(|i| i.name == BLACKSMITH_GATED_ITEM_NAME));
    }

    fn slay(s: &mut GameSession, id: &str, archetype: EnemyArchetype) {
        let mut e = s.zone().enemies.first().cloned().expect("a foe to reshape");
        e.id = id.into();
        e.name = id.into();
        e.archetype = archetype;
        e.is_boss = Some(true);
        e.rank = None;
        e.hp = 0;
        s.zone_mut().enemies.push(e);
        let idx = s.zone().enemies.len() - 1;
        s.on_enemy_killed(idx);
    }

    #[test]
    fn side_bosses_yield_the_trophy_their_quest_needs() {
        for (zone, id, quest, tag) in [
            (ZoneId::parse("barrow_2").unwrap(), "boss_morvath", "barrow_purge", "morvath_seal"),
            (ZoneId::parse("mire_2").unwrap(), "boss_vexis", "spores_of_bog", "fungal_heart"),
            (ZoneId::parse("frost_3").unwrap(), "boss_kaelthar", "cinder_and_frost", "ashen_ore_heart"),
        ] {
            let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 3);
            s.quests.insert(quest.into(), QuestProgress {
                id: quest.into(), status: QuestStatus::Active, stage: 1, outcome_label: None, history: vec![],
            });
            s.debug_warp(zone, -1, -1);
            slay(&mut s, id, EnemyArchetype::DreadWraith);
            assert!(s.has_quest_item(tag), "{id} drops {tag}");
            assert_eq!(s.quests[quest].stage, 2);
            let tracked = s.quest_tracker_json();
            let row = tracked.as_array().unwrap().iter().find(|q| q["id"] == quest).unwrap();
            assert_eq!(row["status"], "turnin", "the tracker sees {tag}");
        }
    }

    /// Every item a quest asks for must have a source in the real game: a
    /// tile that grants it or a boss that carries it.
    #[test]
    fn every_quest_item_can_be_obtained() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 42);
        let mut sources: Vec<&str> = vec![];
        for id in ["overworld", "dungeon_1", "dungeon_2", "dungeon_3", "dungeon_4", "dungeon_5",
                   "barrow_1", "barrow_2", "mire_1", "mire_2", "frost_1", "frost_2", "frost_3"] {
            s.debug_warp(ZoneId::parse(id).unwrap(), -1, -1);
            for t in s.zone().tiles.iter().flatten() {
                match t.kind {
                    TileKind::ContrabandCache => sources.push("smuggler_ledger"),
                    TileKind::HerbNode => sources.push("pale_nightbloom"),
                    TileKind::WhisperingBell if id == "dungeon_3" => sources.push("whispering_bell"),
                    TileKind::Chest if t.interact_id.as_deref() == Some("overworld_satchel_chest") => sources.push("wayfarer_satchel"),
                    _ => {}
                }
            }
            for e in &s.zone().enemies {
                match e.id.as_str() {
                    "boss_morvath" => sources.push("morvath_seal"),
                    "boss_vexis" => sources.push("fungal_heart"),
                    "boss_kaelthar" => sources.push("ashen_ore_heart"),
                    _ => {}
                }
            }
        }
        let npcs = include_str!("npcs.rs");
        let mut asked = 0;
        for part in npcs.split("has_item_tag: Some(\"").skip(1) {
            let tag = part.split('"').next().unwrap();
            asked += 1;
            assert!(sources.contains(&tag), "quest item `{tag}` has no source");
        }
        assert!(asked >= 8);
    }

    #[test]
    fn story_tiles_grant_their_items_and_stages() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 42);
        s.debug_warp(ZoneId::Dungeon(1), -1, -1);
        s.zone_mut().enemies.clear();
        let (x, y) = find_tile(&s, TileKind::ContrabandCache).unwrap();
        s.interact_with_world_tile(x, y);
        assert!(s.has_quest_item("smuggler_ledger"));
        assert_eq!(s.quests["shadows_beneath"].stage, 2);
        assert!(s.interact_with_world_tile(x, y).is_none(), "once only");

        s.debug_warp(ZoneId::Dungeon(3), -1, -1);
        s.zone_mut().enemies.clear();
        let (x, y) = find_tile(&s, TileKind::WhisperingBell).unwrap();
        s.player.attributes.arcana = 1;
        assert!(matches!(s.interact_with_world_tile(x, y), Some(SessionEvent::OpenPrompt(_))));
        s.choose_prompt("shatter_bell");
        assert!(s.active_prompt.is_some(), "too little Arcana");
        s.choose_prompt("take_bell");
        assert!(s.has_quest_item("whispering_bell"));
        assert_eq!(s.quests["whispering_bell"].stage, 2);
    }

    #[test]
    fn six_steles_finish_the_survey() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 42);
        s.quests.insert("deep_survey".into(), QuestProgress {
            id: "deep_survey".into(), status: QuestStatus::Active, stage: 1, outcome_label: None, history: vec![],
        });
        let mut read = 0;
        for id in ["oakhaven", "highspire", "overworld", "dungeon_1", "dungeon_2", "dungeon_3", "dungeon_4", "dungeon_5",
                   "barrow_1", "mire_1", "frost_1"] {
            s.debug_warp(ZoneId::parse(id).unwrap(), -1, -1);
            s.zone_mut().enemies.clear();
            let steles: Vec<(i32, i32)> = s.zone().tiles.iter().flatten()
                .filter(|t| t.kind == TileKind::LoreTablet).map(|t| (t.x, t.y)).collect();
            for (x, y) in steles {
                if matches!(s.interact_with_world_tile(x, y), Some(SessionEvent::OpenPrompt(_))) {
                    read += 1;
                    s.choose_prompt("ok");
                }
            }
        }
        assert!(read >= 6, "only {read} readable steles in the whole game");
        assert_eq!(s.quests["deep_survey"].stage, 6);
    }

    #[test]
    fn hazards_hit_once_and_lethal_ones_wake_you_in_town() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 5);
        s.debug_warp(ZoneId::Dungeon(2), -1, -1);
        s.zone_mut().enemies.clear();
        let (px, py) = (s.player.x, s.player.y);
        let (nx, ny) = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().map(|d| (px + d.0, py + d.1))
            .find(|(x, y)| s.tile_at(*x, *y).is_some_and(|t| t.walkable)).unwrap();
        s.zone_mut().tiles[py as usize][px as usize].kind = TileKind::Floor; // not the arrival stairs
        s.zone_mut().tiles[ny as usize][nx as usize].kind = TileKind::Trap;
        s.zone_mut().tiles[ny as usize][nx as usize].revealed = Some(false);
        assert_eq!(s.state_json()["zone"]["tiles"][ny as usize][nx as usize], "floor", "hidden until sprung");
        s.player.attributes.agility = 1;
        let hp = s.player.hp;
        s.move_player(nx - px, ny - py);
        // 5 + 2 per depth, then the Lynx may lick 2 HP back on its turn.
        assert!((7..=9).contains(&(hp - s.player.hp)), "took {}", hp - s.player.hp);
        assert_eq!(s.state_json()["zone"]["tiles"][ny as usize][nx as usize], "trap");
        s.move_player(px - nx, py - ny);
        let after = s.player.hp;
        s.move_player(nx - px, ny - py);
        assert!(s.player.hp >= after, "one use");
        let t = &mut s.zone_mut().tiles[py as usize][px as usize];
        t.kind = TileKind::EmberCrust;
        t.used = None;
        s.player.attributes.resolve = 1;
        s.player.hp = 1;
        assert_eq!(s.move_player(px - nx, py - ny), SessionEvent::PlayerDied);
        assert_eq!(s.current, ZoneId::Oakhaven);
    }

    #[test]
    fn malakor_ends_the_story_once_and_ascension_keeps_the_hero() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 9);
        assert!(!s.ascend(), "not before the ending");
        s.debug_warp(ZoneId::Dungeon(5), -1, -1);
        slay(&mut s, "boss_malakor", EnemyArchetype::BossMalakor);
        assert!(s.ending_pending());
        assert_eq!(s.quests["sovereign_cinder"].status, QuestStatus::Completed);
        assert!(s.ending_json()["lines"].as_array().unwrap().len() >= 2);
        let bundle = s.to_bundle(0);
        assert!(GameSession::from_bundle(&bundle).unwrap().ending_pending(), "survives a save");
        s.acknowledge_ending();
        assert!(!s.ending_pending());
        let seal = crate::loot::create_quest_item(&mut s.id_gen, "morvath_seal");
        s.player.inventory.push(seal);
        s.player.level = 8;
        let weapon = s.player.equipped["weapon"].clone();
        assert!(s.ascend());
        assert_eq!(s.cycle, 1);
        assert_eq!(s.player.level, 8);
        assert_eq!(s.player.equipped["weapon"], weapon);
        assert!(!s.has_quest_item("morvath_seal"));
        assert!(s.quests.is_empty());
        assert!(!s.ending_pending());
        assert_eq!(s.current, ZoneId::Oakhaven);
    }

    #[test]
    fn bosses_always_drop_premium_loot_and_potions_stack() {
        for seed in 1..25 {
            let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, seed);
            s.debug_warp(ZoneId::Dungeon(2), -1, -1);
            s.zone_mut().loot.clear();
            slay(&mut s, "boss_test", EnemyArchetype::BossBoneCantor);
            let drops: Vec<&Item> = s.zone().loot.iter().filter_map(|l| l.item.as_ref()).collect();
            assert_eq!(drops.len(), 1);
            assert!(matches!(drops[0].rarity, ItemRarity::Epic | ItemRarity::Relic), "{:?}", drops[0].rarity);
        }
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 1);
        let n = s.player.inventory.len();
        let potion = create_consumable(&mut s.id_gen, ConsumableKind::HealPotion, 1);
        s.add_item_to_player(potion);
        assert_eq!(s.player.inventory.len(), n);
    }

    #[test]
    fn snapshot_skips_unchanged_terrain_and_reports_sight() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 4);
        s.debug_warp(ZoneId::Dungeon(1), -1, -1);
        let full = s.snapshot_json("", 0);
        assert!(full["zone"].get("tiles").is_some());
        let slim = s.snapshot_json(&s.current.to_string_id(), s.tile_rev);
        assert!(slim["zone"].get("tiles").is_none());
        assert!(slim.to_string().len() * 2 < full.to_string().len());
        let sight = full["zone"]["sight"].as_array().unwrap();
        assert_eq!(sight.len() as i32, s.zone().height);
        let all: String = sight.iter().map(|r| r.as_str().unwrap()).collect();
        assert!(all.contains('0') && all.contains('2'), "a dungeon starts mostly unseen");
        assert!(full["zone"]["enemies"].as_array().unwrap().iter().any(|e| e["visible"] == false));
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

    #[test]
    fn town_loop_rebuild_fx_and_supply_stock_refresh() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 77);
        for p in crate::town::BLACKSMITH_PIECES {
            s.town = crate::town::record_town_item(s.town.clone(), p.tag);
        }
        s.town = crate::town::record_town_item(s.town.clone(), crate::town::STALL_ITEM_TAG);
        s.town.population = 16; // +5 on complete -> 21 (crosses Tier 1 at 20)

        assert!(s.complete_blacksmith(BlacksmithLot::Pondside));
        let cues = s.take_fx();
        let build_cue = cues.iter().find(|c| c.kind == "town_build").expect("town_build cue");
        let anchor = crate::town_layout::oakhaven_lot_anchor(BlacksmithLot::Pondside);
        assert_eq!((build_cue.x, build_cue.y), (anchor.x.round() as i32, anchor.z.round() as i32));
        assert!(cues.iter().any(|c| c.kind == "town_tier_up"), "tier 1 reached cue: {cues:?}");

        // Seed initial stock before supply banking, then walk out and bank Barrow Iron.
        assert!(!s.merchant_stock("blacksmith").iter().any(|i| i.name == "Barrow-Iron Hauberk"));
        s.player.x = 1;
        s.player.y = 7;
        s.move_player(-1, 0);
        assert_eq!(s.current, ZoneId::Overworld);
        s.player.inventory.push(crate::world::create_quest_item_local("barrow_iron", 9999));
        s.player.x = 1;
        s.player.y = 12;
        s.move_player(-1, 0);
        assert_eq!(s.current, ZoneId::Oakhaven);
        assert!(
            s.merchant_stock("blacksmith").iter().any(|i| i.name == "Barrow-Iron Hauberk"),
            "supply banked after completion refreshes cached blacksmith stock"
        );
    }

    #[test]
    fn quest_tracker_targets_correct_npcs_through_lifecycle() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 42);
        let markers0 = s.npc_markers_json();
        assert_eq!(markers0["vance"], "offer");
        assert_eq!(markers0["vex"], "offer");
        assert_eq!(markers0["mira"], "offer");
        let tracker0 = s.quest_tracker_json();
        let arr0 = tracker0.as_array().unwrap();
        assert_eq!(arr0[0]["id"], "lead_vance");
        assert_eq!(arr0[0]["targetNpc"], "vance");

        // Accept shadows_beneath from Vance.
        s.active_dialogue = Some(("vance".into(), "root".into()));
        s.choose_dialogue("start_shadows");
        s.choose_dialogue("accept");
        s.close_dialogue();

        let markers1 = s.npc_markers_json();
        assert!(markers1.get("vance").is_none());
        let tracker1 = s.quest_tracker_json();
        let row1 = tracker1.as_array().unwrap().iter().find(|r| r["id"] == "shadows_beneath").unwrap();
        assert_eq!(row1["status"], "active");

        // Pick up Sealed Contraband Ledger -> turn-in ready.
        s.player.inventory.push(crate::world::create_quest_item_local("smuggler_ledger", 9001));
        let markers2 = s.npc_markers_json();
        assert_eq!(markers2["vance"], "turnin");
        assert_eq!(markers2["vex"], "turnin");
        let tracker2 = s.quest_tracker_json();
        let row2 = tracker2.as_array().unwrap().iter().find(|r| r["id"] == "shadows_beneath").unwrap();
        assert_eq!(row2["status"], "turnin");
        assert_eq!(row2["targetNpc"], "Warden Vance / Vex");

        // Turn in to Vance -> quest leaves active tracker.
        s.active_dialogue = Some(("vance".into(), "root".into()));
        s.choose_dialogue("turn_in_ledger");
        s.choose_dialogue("claim_vance");
        s.close_dialogue();
        assert!(
            !s.quest_tracker_json().as_array().unwrap().iter().any(|r| r["id"] == "shadows_beneath"),
            "completed quest leaves active tracker"
        );
    }

    #[test]
    fn oakhaven_shrine_restores_hp_and_mp() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 7);
        s.player.hp = 5;
        s.player.mana = 1;
        // Stand east of the shrine (20, 13) so Sister Wren (18, 13) is not also adjacent.
        s.player.x = 20;
        s.player.y = 13;
        s.interact();
        let d = s.derived();
        assert_eq!(s.player.hp, d.max_hp);
        assert_eq!(s.player.mana, d.max_mana);
    }

    #[test]
    fn first_hour_oakhaven_flow() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 99);
        assert_eq!(s.current, ZoneId::Oakhaven);
        // Spawn at (13, 13), step north toward Vance (14, 12).
        s.move_player(0, -1);
        assert_eq!(s.interact(), SessionEvent::OpenDialogue("vance".into()));
        s.choose_dialogue("start_shadows");
        s.choose_dialogue("accept");
        s.close_dialogue();
        assert!(crate::npcs::is_quest_active(&s.quests, "shadows_beneath"));

        // Visit Torin (7, 4) -> shop opens.
        s.player.x = 7;
        s.player.y = 5;
        assert_eq!(s.interact(), SessionEvent::OpenDialogue("torin".into()));
        assert_eq!(s.choose_dialogue("trade"), SessionEvent::OpenMerchant("torin".into()));
        s.close_dialogue();

        // Visit Mira (22, 4) -> rest full.
        s.player.hp = 10;
        s.player.x = 22;
        s.player.y = 5;
        assert_eq!(s.interact(), SessionEvent::OpenDialogue("mira".into()));
        s.choose_dialogue("heal_free");
        s.close_dialogue();
        assert_eq!(s.player.hp, s.derived().max_hp);

        // Descend central stairs (13, 9) -> (13, 8) into dungeon_1.
        s.player.x = 13;
        s.player.y = 9;
        assert_eq!(s.move_player(0, -1), SessionEvent::ZoneChanged(ZoneId::Dungeon(1)));
        assert_eq!(s.current, ZoneId::Dungeon(1));
    }
}
