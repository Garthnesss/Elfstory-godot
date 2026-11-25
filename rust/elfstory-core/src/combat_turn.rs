//! Turn-based combat for the session: the player's melee strike, the eight
//! active skills (with Arcane Echo, Flexible Combat, Carry the World and
//! Arcane Conduit), skill aiming/hotbar/learning, companion turns, and the
//! enemy turn with status bookkeeping and boss confrontations.

use crate::combat::{
    apply_status, calculate_player_hit, chebyshev, choose_enemy_intent, compute_visibility,
    find_path_next_step, has_affix, has_line_of_sight, PlayerHitResult,
};
use crate::session::{GameSession, SessionEvent};
use crate::skills::{can_learn_skill, has_prodigy, skill_def};
use crate::types::*;

pub const SIGHT_RADIUS: i32 = 8;

pub(crate) fn companion_max_hp(kind: CompanionKind, level: i32) -> i32 {
    match kind {
        CompanionKind::StarlightOwl => 20 + (level - 1) * 5,
        CompanionKind::SilverwindLynx => 24 + (level - 1) * 7,
    }
}

pub(crate) fn make_companion(kind: CompanionKind, x: i32, y: i32, level: i32) -> CompanionState {
    let max_hp = companion_max_hp(kind, level);
    let name = match kind {
        CompanionKind::StarlightOwl => "Starlight Owl",
        CompanionKind::SilverwindLynx => "Silverwind Lynx",
    };
    CompanionState {
        kind,
        name: name.to_string(),
        x,
        y,
        hp: max_hp,
        max_hp,
        respawn_cooldown: 0,
    }
}

pub(crate) fn place_companion_near(zone: &ZoneMap, px: i32, py: i32) -> Pos {
    const OFFSETS: [(i32, i32); 8] = [
        (-1, 0), (1, 0), (0, 1), (0, -1),
        (-1, 1), (1, 1), (-1, -1), (1, -1),
    ];
    for (dx, dy) in OFFSETS {
        let nx = px + dx;
        let ny = py + dy;
        if nx < 0 || ny < 0 || nx >= zone.width || ny >= zone.height {
            continue;
        }
        let tile = &zone.tiles[ny as usize][nx as usize];
        if tile.walkable
            && !zone.enemies.iter().any(|e| e.hp > 0 && e.x == nx && e.y == ny)
            && !zone.npcs.iter().any(|n| n.x == nx && n.y == ny)
        {
            return Pos::new(nx, ny);
        }
    }
    Pos::new(px, py)
}

pub fn skill_rank(p: &PlayerState, id: SkillId) -> i32 {
    p.skills.get(id.as_str()).copied().unwrap_or(0)
}

fn is_dot(kind: StatusKind) -> bool {
    matches!(kind, StatusKind::Bleed | StatusKind::Burn | StatusKind::Poison)
}

/// TS getInRangeEnemies' effective range (also used by executeSkill).
pub fn effective_range(id: SkillId, rank: i32, base: i32) -> i32 {
    match id {
        SkillId::EmberLance | SkillId::VenomTip if rank >= 2 => base + 1,
        SkillId::Shadowstep => {
            if rank == 1 {
                4
            } else {
                5
            }
        }
        _ => base,
    }
}

impl GameSession {
    pub(crate) fn sight_radius(&self) -> i32 {
        let owl = self
            .player
            .companion
            .as_ref()
            .is_some_and(|c| c.kind == CompanionKind::StarlightOwl && c.hp > 0 && c.respawn_cooldown == 0);
        SIGHT_RADIUS + if owl { 1 } else { 0 }
    }

    pub(crate) fn refresh_visibility(&mut self) {
        let (px, py) = (self.player.x, self.player.y);
        let radius = self.sight_radius();
        compute_visibility(self.zone_mut(), px, py, radius);
    }

    fn enemy_idx(&self, id: &str) -> Option<usize> {
        self.zone().enemies.iter().position(|e| e.id == id)
    }

    fn player_hit(
        &mut self,
        idx: usize,
        mult: f64,
        is_spell: bool,
        force_crit: bool,
        ignore_def: f64,
    ) -> PlayerHitResult {
        let stats = self.derived();
        let target = self.zone().enemies[idx].clone();
        calculate_player_hit(&self.player, &target, mult, is_spell, force_crit, ignore_def, &stats, &mut self.rng)
    }

    fn heal_player(&mut self, amount: i32) {
        let max = self.derived().max_hp;
        self.player.hp = (self.player.hp + amount).min(max);
    }

    fn restore_mana(&mut self, amount: i32) {
        let max = self.derived().max_mana;
        self.player.mana = (self.player.mana + amount).min(max);
    }

    /// Reward every enemy at 0 HP (handleEnemyDeath) and take it off the
    /// board. Returns the most notable event (a level-up).
    pub(crate) fn reap_dead(&mut self) -> SessionEvent {
        let mut event = SessionEvent::None;
        while let Some(idx) = self.zone().enemies.iter().position(|e| e.hp <= 0) {
            let ev = self.on_enemy_killed(idx);
            if ev != SessionEvent::None {
                event = ev;
            }
        }
        event
    }

    /// Passive sustain hooks from TS handleEnemyDeath (called by
    /// on_enemy_killed after gold): Crimson Vigor r3, Soul Siphon, and the
    /// mana_on_kill affix.
    pub(crate) fn kill_passives(&mut self) {
        let d = self.derived();
        self.player.max_hp = d.max_hp;
        self.player.max_mana = d.max_mana;
        if skill_rank(&self.player, SkillId::Bloodlust) >= 3 {
            self.player.hp = (self.player.hp + 5).min(d.max_hp);
        }
        let siphon = skill_rank(&self.player, SkillId::SiphonSoul);
        if siphon > 0 {
            let mana = match siphon {
                1 => 3,
                2 => 5,
                _ => 7,
            };
            self.player.mana = (self.player.mana + mana).min(d.max_mana);
            if siphon >= 2 {
                let hp = if siphon == 2 { 4 } else { 7 };
                self.player.hp = (self.player.hp + hp).min(d.max_hp);
            }
        }
        if has_affix(&self.player, ItemAffixId::ManaOnKill) {
            self.player.mana = (self.player.mana + 4).min(d.max_mana);
        }
    }

    // ------------------------------------------------------------ melee

    /// TS attemptStep "Enemy Bump -> Basic Melee Strike".
    pub(crate) fn melee_attack(&mut self, idx: usize) -> SessionEvent {
        self.aimed_target = None;
        let hit = self.player_hit(idx, 1.0, false, false, 0.0);
        let (tx, ty, name) = {
            let t = &mut self.zone_mut().enemies[idx];
            t.hp -= hit.damage;
            (t.x, t.y, t.name.clone())
        };
        self.player.first_strike_ready = false;
        if hit.lifesteal_heal > 0 {
            self.heal_player(hit.lifesteal_heal);
        }
        if hit.is_crit && has_affix(&self.player, ItemAffixId::BleedOnCrit) {
            apply_status(&mut self.zone_mut().enemies[idx].statuses, StatusKind::Bleed, 2, 4);
        }
        if has_affix(&self.player, ItemAffixId::ChillOnHit) && self.rng.next_f32() < 0.3 {
            apply_status(&mut self.zone_mut().enemies[idx].statuses, StatusKind::Chilled, 2, 1);
        }
        if hit.is_crit && skill_rank(&self.player, SkillId::SiphonSoul) >= 3 {
            self.restore_mana(2);
        }
        let (dx, dy) = (tx - self.player.x, ty - self.player.y);
        self.fx_dir(if hit.is_crit { "crit" } else { "hit" }, tx, ty, dx, dy);
        if hit.is_crit {
            self.say(&format!("Critical strike on {name} for {} dmg!", hit.damage));
        } else {
            self.say(&format!("You strike {name} for {} dmg.", hit.damage));
        }
        if has_prodigy(&self.player, ProdigyId::FlexibleCombat)
            && self.zone().enemies[idx].hp > 0
            && self.rng.next_f32() < 0.45
        {
            let combo = self.player_hit(idx, 0.65, false, false, 0.0);
            self.zone_mut().enemies[idx].hp -= combo.damage;
            self.say(&format!("Flexible Combat lands a follow-up strike for {}!", combo.damage));
        }
        let kill_event = self.reap_dead();
        let turn_event = self.run_enemy_turns();
        if turn_event != SessionEvent::None {
            turn_event
        } else {
            kill_event
        }
    }

    /// TS attemptStep(0, 0): pass a turn.
    pub fn wait_turn(&mut self) -> SessionEvent {
        if self.player.hp <= 0 || self.active_prompt.is_some() {
            return SessionEvent::None;
        }
        self.run_enemy_turns()
    }

    // ------------------------------------------------------------ hotbar / aim

    /// TS getInRangeEnemies: visible enemies in range with line of sight,
    /// nearest first then weakest. Empty for self skills and Shadowstep.
    pub fn in_range_enemies(&self, id: SkillId) -> Vec<Pos> {
        let def = skill_def(id);
        if def.targeting == SkillTargeting::Self_ || id == SkillId::Shadowstep {
            return vec![];
        }
        let range = effective_range(id, skill_rank(&self.player, id), def.range);
        let z = self.zone();
        let (px, py) = (self.player.x, self.player.y);
        let mut list: Vec<&Enemy> = z
            .enemies
            .iter()
            .filter(|e| e.hp > 0)
            .filter(|e| z.tiles[e.y as usize][e.x as usize].visible)
            .filter(|e| chebyshev(px, py, e.x, e.y) <= range)
            .filter(|e| has_line_of_sight(z, px, py, e.x, e.y))
            .collect();
        list.sort_by_key(|e| (chebyshev(px, py, e.x, e.y), e.hp));
        list.iter().map(|e| Pos::new(e.x, e.y)).collect()
    }

    /// TS handleTriggerHotbar. Self skills fire at once; targeted skills
    /// toggle the aim (auto-aimed at the nearest valid enemy; Shadowstep
    /// aims at the player's tile for the cursor to move from).
    pub fn trigger_hotbar(&mut self, slot: usize) -> SessionEvent {
        if self.player.hp <= 0 || self.active_prompt.is_some() {
            return SessionEvent::None;
        }
        let Some(Some(id)) = self.player.hotbar.get(slot).copied() else {
            return SessionEvent::None;
        };
        let def = skill_def(id);
        if skill_rank(&self.player, id) == 0 {
            return SessionEvent::None;
        }
        let cd = self.player.cooldowns.get(id.as_str()).copied().unwrap_or(0);
        if cd > 0 {
            self.say(&format!("{} is on cooldown ({cd}t).", def.name));
            return SessionEvent::None;
        }
        if self.player.mana < def.mana_cost {
            self.say(&format!("Not enough Mana for {}.", def.name));
            return SessionEvent::None;
        }
        if def.targeting == SkillTargeting::Self_ {
            let (px, py) = (self.player.x, self.player.y);
            return self.execute_skill(id, px, py);
        }
        if self.armed_skill == Some(id) {
            self.cancel_aim();
        } else {
            self.armed_skill = Some(id);
            self.aimed_target = if id == SkillId::Shadowstep {
                Some(Pos::new(self.player.x, self.player.y))
            } else {
                self.in_range_enemies(id).first().copied()
            };
        }
        SessionEvent::None
    }

    pub fn cancel_aim(&mut self) {
        self.armed_skill = None;
        self.aimed_target = None;
    }

    /// TS handleCycleTarget.
    pub fn cycle_target(&mut self) {
        let Some(id) = self.armed_skill else { return };
        let list = self.in_range_enemies(id);
        if list.is_empty() {
            if id != SkillId::Shadowstep {
                self.aimed_target = None;
            }
            return;
        }
        let cur = self.aimed_target.and_then(|a| list.iter().position(|p| *p == a));
        let next = cur.map_or(0, |i| (i + 1) % list.len());
        self.aimed_target = Some(list[next]);
    }

    /// Keyboard aim cursor (the TS game aimed with the mouse): nudge the
    /// aimed tile, starting from the player when nothing is aimed.
    pub fn move_aim(&mut self, dx: i32, dy: i32) {
        if self.armed_skill.is_none() {
            return;
        }
        let from = self.aimed_target.unwrap_or(Pos::new(self.player.x, self.player.y));
        let (w, h) = (self.zone().width, self.zone().height);
        self.aimed_target = Some(Pos::new((from.x + dx).clamp(0, w - 1), (from.y + dy).clamp(0, h - 1)));
    }

    /// TS handleConfirmAimedTarget.
    pub fn confirm_target(&mut self) -> SessionEvent {
        match (self.armed_skill, self.aimed_target) {
            (Some(id), Some(t)) => self.execute_skill(id, t.x, t.y),
            _ => SessionEvent::None,
        }
    }

    // ------------------------------------------------------------ learning

    /// TS handleLearnSkill: spend a point, rank up, and slot a newly
    /// learned active into the first empty hotbar slot.
    pub fn learn_skill(&mut self, id: SkillId) -> bool {
        if !can_learn_skill(&self.player, id).ok {
            return false;
        }
        let rank = skill_rank(&self.player, id) + 1;
        self.player.skill_points -= 1;
        self.player.skills.insert(id.as_str().to_string(), rank);
        if skill_def(id).kind == "active" && !self.player.hotbar.contains(&Some(id)) {
            if let Some(slot) = self.player.hotbar.iter().position(|s| s.is_none()) {
                self.player.hotbar[slot] = Some(id);
            }
        }
        let d = self.derived();
        self.player.max_hp = d.max_hp;
        self.player.max_mana = d.max_mana;
        self.say(&format!("Learned {} (rank {rank}).", skill_def(id).name));
        true
    }

    /// TS handleAssignHotbar.
    pub fn assign_hotbar(&mut self, id: SkillId, slot: usize) -> bool {
        if slot >= self.player.hotbar.len() || skill_rank(&self.player, id) == 0 {
            return false;
        }
        for s in self.player.hotbar.iter_mut() {
            if *s == Some(id) {
                *s = None;
            }
        }
        self.player.hotbar[slot] = Some(id);
        true
    }

    /// Hotbar slots for the front-end: skill, rank, cost, cooldown left,
    /// and whether it can fire right now.
    pub fn hotbar_json(&self) -> serde_json::Value {
        let slots: Vec<serde_json::Value> = self
            .player
            .hotbar
            .iter()
            .map(|slot| match slot {
                None => serde_json::Value::Null,
                Some(id) => {
                    let def = skill_def(*id);
                    let rank = skill_rank(&self.player, *id);
                    let cd = self.player.cooldowns.get(id.as_str()).copied().unwrap_or(0);
                    serde_json::json!({
                        "id": id.as_str(), "name": def.name, "glyph": def.glyph,
                        "rank": rank, "manaCost": def.mana_cost, "cooldown": cd,
                        "targeting": def.targeting.as_str(),
                        "range": effective_range(*id, rank, def.range),
                        "ready": rank > 0 && cd == 0 && self.player.mana >= def.mana_cost,
                    })
                }
            })
            .collect();
        serde_json::Value::Array(slots)
    }

    /// Every skill with its rank and whether it can be learned now, for the
    /// skills panel.
    pub fn skills_json(&self) -> serde_json::Value {
        let list: Vec<serde_json::Value> = crate::skills::all_skills_list()
            .into_iter()
            .map(|def| {
                let rank = skill_rank(&self.player, def.id);
                let check = can_learn_skill(&self.player, def.id);
                serde_json::json!({
                    "id": def.id.as_str(), "name": def.name, "glyph": def.glyph,
                    "branch": def.branch.as_str(), "tier": def.tier, "kind": def.kind,
                    "rank": rank, "summary": def.summary,
                    "next": def.rank_descriptions.get(rank as usize).cloned(),
                    "current": if rank > 0 { def.rank_descriptions.get(rank as usize - 1).cloned() } else { None },
                    "canLearn": check.ok, "reason": check.reason,
                    "manaCost": def.mana_cost, "cooldown": def.cooldown,
                })
            })
            .collect();
        serde_json::json!({ "skillPoints": self.player.skill_points, "skills": list })
    }

    // ------------------------------------------------------------ skills

    fn echo_rank(&self) -> i32 {
        skill_rank(&self.player, SkillId::ArcaneEcho)
    }

    /// TS applyEchoIfTriggered.
    fn apply_echo(&mut self, idx: usize, cast: SkillId) {
        let rank = self.echo_rank();
        let chance = match rank {
            1 => 0.25,
            2 => 0.4,
            3 => 0.55,
            _ => 0.0,
        };
        if chance > 0.0 && self.zone().enemies[idx].hp > 0 && (self.rng.next_f32() as f64) < chance {
            let mult = match rank {
                1 => 0.45,
                2 => 0.55,
                _ => 0.7,
            };
            let hit = self.player_hit(idx, mult, true, false, 0.0);
            let name = {
                let e = &mut self.zone_mut().enemies[idx];
                e.hp -= hit.damage;
                e.name.clone()
            };
            if rank >= 2 {
                let cd = self.player.cooldowns.entry(cast.as_str().to_string()).or_insert(1);
                *cd = (*cd - 1).max(0);
            }
            self.say(&format!("Arcane Echo blasts {name} for {}!", hit.damage));
        }
    }

    /// TS applyFlexibleFollowUp.
    fn apply_flexible(&mut self, idx: usize) {
        if has_prodigy(&self.player, ProdigyId::FlexibleCombat)
            && self.zone().enemies[idx].hp > 0
            && self.rng.next_f32() < 0.45
        {
            let combo = self.player_hit(idx, 0.65, false, false, 0.0);
            let name = {
                let e = &mut self.zone_mut().enemies[idx];
                e.hp -= combo.damage;
                e.name.clone()
            };
            self.say(&format!("Flexible Combat strikes {name} for +{}!", combo.damage));
        }
    }

    fn damage_cue(&mut self, idx: usize, crit: bool, directed: bool) {
        let (x, y) = (self.zone().enemies[idx].x, self.zone().enemies[idx].y);
        let (dx, dy) = if directed { (x - self.player.x, y - self.player.y) } else { (0, 0) };
        self.fx_dir(if crit { "crit" } else { "hit" }, x, y, dx, dy);
    }

    /// TS executeSkill. Invalid targets log the TS hint and leave the skill
    /// armed with nothing spent; a valid cast spends mana, starts the
    /// cooldown, resolves, and gives the enemies their turn.
    pub fn execute_skill(&mut self, id: SkillId, tx: i32, ty: i32) -> SessionEvent {
        let def = skill_def(id);
        let rank = skill_rank(&self.player, id);
        if rank == 0 || self.player.hp <= 0 {
            return SessionEvent::None;
        }
        if self.player.mana < def.mana_cost {
            self.say(&format!("Not enough Mana for {} ({} MP needed).", def.name, def.mana_cost));
            self.cancel_aim();
            return SessionEvent::None;
        }
        let (px, py) = (self.player.x, self.player.y);

        // Validate before spending (TS builds nextP but discards it on an
        // invalid target).
        let target_idx = match def.targeting {
            SkillTargeting::Self_ | SkillTargeting::Passive => None,
            SkillTargeting::Blink => {
                let max_range = if rank == 1 { 4 } else { 5 };
                let dist = chebyshev(px, py, tx, ty);
                let z = self.zone();
                let ok_tile = tx >= 0
                    && ty >= 0
                    && tx < z.width
                    && ty < z.height
                    && z.tiles[ty as usize][tx as usize].walkable
                    && z.tiles[ty as usize][tx as usize].visible;
                let occupied = z.enemies.iter().any(|e| e.hp > 0 && e.x == tx && e.y == ty)
                    || z.npcs.iter().any(|n| n.x == tx && n.y == ty);
                if !ok_tile || occupied || dist == 0 || dist > max_range {
                    self.say("Choose an unoccupied visible floor tile within Shadowstep range.");
                    return SessionEvent::None;
                }
                None
            }
            SkillTargeting::Adjacent | SkillTargeting::Ranged | SkillTargeting::Line => {
                let range = effective_range(id, rank, def.range);
                let idx = self.zone().enemies.iter().position(|e| e.hp > 0 && e.x == tx && e.y == ty);
                let ok = idx.is_some()
                    && chebyshev(px, py, tx, ty) <= range
                    && has_line_of_sight(self.zone(), px, py, tx, ty);
                if !ok {
                    self.say(&format!("Select a visible enemy within range {range}."));
                    return SessionEvent::None;
                }
                idx
            }
        };

        self.player.mana -= def.mana_cost;
        self.player.cooldowns.insert(id.as_str().to_string(), def.cooldown);
        if def.branch == SkillBranch::Runecaster && has_prodigy(&self.player, ProdigyId::ArcaneConduit) {
            for (k, v) in self.player.cooldowns.iter_mut() {
                if k != id.as_str() && *v > 0 {
                    *v = (*v - 1).max(0);
                }
            }
        }
        self.fx_dir(id.as_str(), tx, ty, tx - px, ty - py);
        let carry = if has_prodigy(&self.player, ProdigyId::CarryTheWorld) { 1.25 } else { 1.0 };

        match id {
            SkillId::IronFortress => {
                let def_bonus = match rank {
                    1 => 8,
                    2 => 11,
                    _ => 14,
                };
                let turns = if rank == 3 { 4 } else { 3 };
                apply_status(&mut self.player.statuses, StatusKind::Shielded, turns, def_bonus);
                if rank >= 2 {
                    self.player
                        .statuses
                        .retain(|s| s.kind != StatusKind::Bleed && s.kind != StatusKind::Poison);
                }
                if rank == 3 {
                    self.heal_player(12);
                }
                self.say(&format!("Iron Fortress raised (+{def_bonus} DEF for {turns} turns)."));
            }
            SkillId::FrostNova => {
                let radius = if rank == 3 { 3 } else { 2 };
                let (mult, base) = match rank {
                    1 => (1.1, 10),
                    2 => (1.4, 14),
                    _ => (1.7, 18),
                };
                let ids: Vec<String> = self
                    .zone()
                    .enemies
                    .iter()
                    .filter(|e| e.hp > 0 && chebyshev(px, py, e.x, e.y) <= radius)
                    .map(|e| e.id.clone())
                    .collect();
                for eid in &ids {
                    let Some(i) = self.enemy_idx(eid) else { continue };
                    let dist = chebyshev(px, py, self.zone().enemies[i].x, self.zone().enemies[i].y);
                    let res = self.player_hit(i, mult, true, false, 0.0);
                    let e = &mut self.zone_mut().enemies[i];
                    e.hp -= base + res.damage;
                    apply_status(&mut e.statuses, StatusKind::Chilled, if rank == 3 { 3 } else { 2 }, 1);
                    if rank >= 2 && dist == 1 {
                        apply_status(&mut e.statuses, StatusKind::Stunned, 1, 1);
                    }
                    self.damage_cue(i, false, false);
                    self.apply_echo(i, id);
                }
                if ids.is_empty() {
                    self.say("Rime Nova chills the empty stone.");
                } else {
                    self.say(&format!("Rime Nova freezes {} foe(s)!", ids.len()));
                }
            }
            SkillId::Shadowstep => {
                self.player.x = tx;
                self.player.y = ty;
                let adj = self
                    .zone()
                    .enemies
                    .iter()
                    .position(|e| e.hp > 0 && chebyshev(e.x, e.y, tx, ty) == 1);
                if let Some(i) = adj {
                    let mult = match rank {
                        1 => 1.25,
                        2 => 1.5,
                        _ => 1.75,
                    };
                    let hit = self.player_hit(i, mult, false, rank >= 2, 0.0);
                    let name = {
                        let e = &mut self.zone_mut().enemies[i];
                        e.hp -= hit.damage;
                        e.name.clone()
                    };
                    if hit.lifesteal_heal > 0 {
                        self.heal_player(hit.lifesteal_heal);
                    }
                    self.damage_cue(i, hit.is_crit, true);
                    self.apply_echo(i, id);
                    self.apply_flexible(i);
                    self.say(&format!("Shadowstep ambush hits {name} for {}!", hit.damage));
                } else {
                    self.say("Blinked through shadow.");
                }
            }
            SkillId::Cleave => {
                let i = target_idx.expect("validated");
                let (main, splash) = match rank {
                    1 => (1.35, 0.75),
                    2 => (1.55, 0.95),
                    _ => (1.8, 1.2),
                };
                let hit = self.player_hit(i, main * carry, false, false, 0.0);
                let (target_id, name) = {
                    let e = &mut self.zone_mut().enemies[i];
                    e.hp -= hit.damage;
                    if rank >= 2 {
                        apply_status(&mut e.statuses, StatusKind::Bleed, rank, 4);
                    }
                    (e.id.clone(), e.name.clone())
                };
                self.damage_cue(i, hit.is_crit, true);
                self.apply_echo(i, id);
                self.apply_flexible(i);
                let adj: Vec<String> = self
                    .zone()
                    .enemies
                    .iter()
                    .filter(|e| e.hp > 0 && e.id != target_id && chebyshev(px, py, e.x, e.y) == 1)
                    .map(|e| e.id.clone())
                    .collect();
                for eid in adj {
                    let Some(j) = self.enemy_idx(&eid) else { continue };
                    let sp = self.player_hit(j, splash * carry, false, false, 0.0);
                    self.zone_mut().enemies[j].hp -= sp.damage;
                    self.damage_cue(j, false, false);
                }
                self.say(&format!("Sundering Cleave strikes {name} for {}!", hit.damage));
            }
            SkillId::ShieldBash => {
                let i = target_idx.expect("validated");
                let d = self.derived();
                let (atk_mult, def_mult) = match rank {
                    1 => (0.9, 1.0),
                    2 => (1.1, 1.3),
                    _ => (1.3, 1.6),
                };
                let base = self.player_hit(i, atk_mult * carry, false, false, 0.0);
                let bonus = (d.def as f64 * def_mult * carry).round() as i32;
                let total = base.damage + bonus;
                let name = {
                    let e = &mut self.zone_mut().enemies[i];
                    e.hp -= total;
                    apply_status(&mut e.statuses, StatusKind::Stunned, if rank == 3 { 2 } else { 1 }, 1);
                    e.intent = None;
                    e.name.clone()
                };
                if rank >= 2 {
                    let (turns, pot) = if rank == 3 { (3, 6) } else { (2, 4) };
                    apply_status(&mut self.player.statuses, StatusKind::Shielded, turns, pot);
                }
                self.damage_cue(i, base.is_crit, true);
                self.apply_echo(i, id);
                self.apply_flexible(i);
                self.say(&format!("Bastion Bash stuns {name} for {total} dmg!"));
            }
            SkillId::VenomTip => {
                let i = target_idx.expect("validated");
                let mult = match rank {
                    1 => 0.95,
                    2 => 1.15,
                    _ => 1.35,
                };
                let ignore = if rank == 3 { 0.5 } else { 0.0 };
                let hit = self.player_hit(i, mult, false, false, ignore);
                let total = hit.damage + self.player.attributes.agility;
                let name = {
                    let e = &mut self.zone_mut().enemies[i];
                    e.hp -= total;
                    let pot = match rank {
                        1 => 3,
                        2 => 5,
                        _ => 7,
                    };
                    apply_status(&mut e.statuses, StatusKind::Poison, if rank == 3 { 4 } else { 3 }, pot);
                    if rank >= 2 {
                        apply_status(&mut e.statuses, StatusKind::Bleed, rank, 4);
                    }
                    e.name.clone()
                };
                self.damage_cue(i, hit.is_crit, false);
                self.apply_echo(i, id);
                self.apply_flexible(i);
                self.say(&format!("Viper Edge pierces {name} for {total} dmg."));
            }
            SkillId::ExecutionersMark => {
                let i = target_idx.expect("validated");
                let (pct, mult) = match rank {
                    1 => (35, 1.1),
                    2 => (45, 1.4),
                    _ => (60, 1.7),
                };
                apply_status(
                    &mut self.zone_mut().enemies[i].statuses,
                    StatusKind::Marked,
                    if rank == 3 { 5 } else { 4 },
                    pct,
                );
                let hit = self.player_hit(i, mult, false, false, 0.0);
                let name = {
                    let e = &mut self.zone_mut().enemies[i];
                    e.hp -= hit.damage;
                    e.name.clone()
                };
                self.damage_cue(i, hit.is_crit, false);
                self.apply_echo(i, id);
                self.apply_flexible(i);
                self.say(&format!("Marked {name} (+{pct}% dmg taken) for {}!", hit.damage));
                if self.zone().enemies[i].hp <= 0 && rank == 3 {
                    self.player.cooldowns.insert(id.as_str().to_string(), 0);
                }
            }
            SkillId::EmberLance => {
                let i = target_idx.expect("validated");
                // Audit Tier 1 trim (TS comment): 1.25/1.45/1.55x + 10/13/13.
                let (mult, base) = match rank {
                    1 => (1.25, 10),
                    2 => (1.45, 13),
                    _ => (1.55, 13),
                };
                let hit = self.player_hit(i, mult, true, false, 0.0);
                let total = base + hit.damage;
                let (target_id, cx, cy, name) = {
                    let e = &mut self.zone_mut().enemies[i];
                    e.hp -= total;
                    let pot = match rank {
                        1 => 3,
                        2 => 5,
                        _ => 7,
                    };
                    apply_status(&mut e.statuses, StatusKind::Burn, if rank == 1 { 2 } else { 3 }, pot);
                    (e.id.clone(), e.x, e.y, e.name.clone())
                };
                self.damage_cue(i, hit.is_crit, false);
                self.apply_echo(i, id);
                if rank == 3 {
                    let splash = (total as f64 * 0.6).round() as i32;
                    for e in self.zone_mut().enemies.iter_mut() {
                        if e.hp > 0 && e.id != target_id && chebyshev(e.x, e.y, cx, cy) == 1 {
                            e.hp -= splash;
                            apply_status(&mut e.statuses, StatusKind::Burn, 2, 5);
                        }
                    }
                }
                self.say(&format!("Ember Lance sears {name} for {total} fire dmg!"));
                // Rank 2 "pierces 1 tile behind" (in the web version's text
                // but never implemented; added at Garth's call, Nov 21): the
                // lance carries on one tile past the target, away from the
                // caster, at the rank-3 splash strength (60% + Burn 5/t 2t).
                // Rank 3's blast already covers that tile, so no double hit.
                if rank == 2 {
                    let (bx, by) = (cx + (cx - px).signum(), cy + (cy - py).signum());
                    if let Some(j) = self
                        .zone()
                        .enemies
                        .iter()
                        .position(|e| e.hp > 0 && e.id != target_id && e.x == bx && e.y == by)
                    {
                        let pierce = (total as f64 * 0.6).round() as i32;
                        let behind = {
                            let e = &mut self.zone_mut().enemies[j];
                            e.hp -= pierce;
                            apply_status(&mut e.statuses, StatusKind::Burn, 2, 5);
                            e.name.clone()
                        };
                        self.damage_cue(j, false, false);
                        self.say(&format!("Ember Lance pierces through to {behind} for {pierce}!"));
                    }
                }
            }
            // Passives are never cast (no rank-gated hotbar entry, and
            // targeting Passive validates to no target above).
            SkillId::Bloodlust | SkillId::EvasiveInstinct | SkillId::SiphonSoul | SkillId::ArcaneEcho => {
                return SessionEvent::None;
            }
        }

        self.cancel_aim();
        let kill_event = self.reap_dead();
        let turn_event = self.run_enemy_turns();
        if turn_event != SessionEvent::None {
            turn_event
        } else {
            kill_event
        }
    }

    // ------------------------------------------------------------ enemy turn

    /// TS tryApplyPlayerAilment (Draconic Will resists 60%).
    fn try_apply_player_ailment(&mut self, kind: StatusKind, turns: i32, potency: i32) {
        if has_prodigy(&self.player, ProdigyId::DraconicWill) && self.rng.next_f32() < 0.6 {
            self.say("Draconic Will resists the affliction!");
            return;
        }
        apply_status(&mut self.player.statuses, kind, turns, potency);
    }

    /// TS checkPlayerFatal: true when the player is dead (Cauterize may
    /// deny the first death every 25 turns).
    pub(crate) fn check_player_fatal(&mut self, max_hp: i32) -> bool {
        if self.player.hp > 0 {
            return false;
        }
        let cd = self
            .player
            .prodigy_cooldowns
            .as_ref()
            .and_then(|m| m.get("cauterize").copied())
            .unwrap_or(0);
        if has_prodigy(&self.player, ProdigyId::Cauterize) && cd <= 0 {
            self.player.hp = 1.max((max_hp as f64 * 0.25).round() as i32);
            apply_status(&mut self.player.statuses, StatusKind::Cauterized, 3, 1);
            apply_status(&mut self.player.statuses, StatusKind::Shielded, 3, 10);
            self.player
                .prodigy_cooldowns
                .get_or_insert_with(Default::default)
                .insert("cauterize".into(), 25);
            self.say("Cauterize ignites! Death denied (+25% HP & +10 DEF Ward for 3t).");
            return false;
        }
        true
    }

    fn run_companion_turn(&mut self, stats: &DerivedStats, event: &mut SessionEvent) {
        let Some(comp) = self.player.companion.clone() else { return };
        let (px, py) = (self.player.x, self.player.y);

        if comp.respawn_cooldown > 0 {
            let rem = comp.respawn_cooldown - 1;
            if rem == 0 {
                let pos = place_companion_near(self.zone(), px, py);
                if let Some(c) = self.player.companion.as_mut() {
                    c.respawn_cooldown = 0;
                    c.hp = c.max_hp;
                    c.x = pos.x;
                    c.y = pos.y;
                }
                self.fx("companion", pos.x, pos.y);
                self.say(&format!("{} reforms beside you!", comp.name));
            } else if let Some(c) = self.player.companion.as_mut() {
                c.respawn_cooldown = rem;
            }
            return;
        }
        if comp.hp <= 0 {
            return;
        }

        // Aura trickle.
        match comp.kind {
            CompanionKind::SilverwindLynx
                if self.player.turns_elapsed % 3 == 0 && self.player.hp < stats.max_hp =>
            {
                self.heal_player(2);
            }
            CompanionKind::StarlightOwl
                if self.player.turns_elapsed % 2 == 0 && self.player.mana < stats.max_mana =>
            {
                self.restore_mana(1);
            }
            _ => {}
        }

        let adj_id = self
            .zone()
            .enemies
            .iter()
            .filter(|e| {
                e.hp > 0
                    && e.pacified != Some(true)
                    && chebyshev(comp.x, comp.y, e.x, e.y) == 1
            })
            .min_by_key(|e| e.hp)
            .map(|e| e.id.clone());

        if let Some(eid) = adj_id {
            if let Some(idx) = self.enemy_idx(&eid) {
                let (t_def, t_name, tx, ty) = {
                    let e = &self.zone().enemies[idx];
                    (e.def, e.name.clone(), e.x, e.y)
                };
                let base_dmg = match comp.kind {
                    CompanionKind::SilverwindLynx => {
                        6 + (self.player.level as f64 * 2.4).floor() as i32
                            + (self.player.attributes.agility as f64 * 0.6).floor() as i32
                    }
                    CompanionKind::StarlightOwl => {
                        5 + (self.player.level as f64 * 2.0).floor() as i32
                            + (self.player.attributes.arcana as f64 * 0.7).floor() as i32
                    }
                };
                let is_crit = comp.kind == CompanionKind::SilverwindLynx && self.rng.next_f32() < 0.2;
                let mult = if is_crit { 1.6 } else { 1.0 };
                let final_dmg = 2.max((base_dmg as f64 * mult - t_def as f64 * 0.45).round() as i32);
                self.zone_mut().enemies[idx].hp -= final_dmg;
                self.fx_dir("companion_hit", tx, ty, tx - comp.x, ty - comp.y);
                self.say(&format!("{} strikes {t_name} for {final_dmg} dmg!", comp.name));
                if comp.kind == CompanionKind::StarlightOwl {
                    self.restore_mana(1);
                }
                if self.zone().enemies[idx].hp <= 0 {
                    let ev = self.on_enemy_killed(idx);
                    if ev != SessionEvent::None {
                        *event = ev;
                    }
                }
            }
            return;
        }

        let max_leash = 6;
        let z = self.zone();
        let pursue = z
            .enemies
            .iter()
            .filter(|e| {
                e.hp > 0
                    && e.pacified != Some(true)
                    && chebyshev(px, py, e.x, e.y) <= max_leash
                    && (z.tiles[e.y as usize][e.x as usize].visible || e.alerted)
            })
            .min_by_key(|e| chebyshev(comp.x, comp.y, e.x, e.y))
            .map(|e| Pos::new(e.x, e.y));

        let goal = if let Some(target) = pursue {
            Some(target)
        } else if chebyshev(comp.x, comp.y, px, py) > 2 {
            Some(Pos::new(px, py))
        } else {
            None
        };

        if let Some(g) = goal {
            if let Some(step) = find_path_next_step(z, comp.x, comp.y, g.x, g.y) {
                let blocked = (step.x == px && step.y == py)
                    || z.enemies.iter().any(|e| e.hp > 0 && e.x == step.x && e.y == step.y);
                if !blocked {
                    if let Some(c) = self.player.companion.as_mut() {
                        c.x = step.x;
                        c.y = step.y;
                    }
                }
            }
        }
    }

    /// TS runEnemyTurns (combat core): tick the player's statuses and
    /// cooldowns, regen, DoTs on both sides, companion turn, then every
    /// enemy acts — stunned skip, telegraphed intents resolve, fresh intents
    /// prime, adjacent foes strike player or companion, the rest close in.
    pub(crate) fn run_enemy_turns(&mut self) -> SessionEvent {
        if self.player.hp <= 0 {
            return self.wake_in_oakhaven();
        }
        let prev_regen = self.player.statuses.iter().find(|s| s.kind == StatusKind::Regen).copied();
        let incoming_dots: Vec<StatusEffect> =
            self.player.statuses.iter().filter(|s| is_dot(s.kind)).copied().collect();

        let p = &mut self.player;
        if let Some(pc) = p.prodigy_cooldowns.as_mut() {
            pc.retain(|_, v| *v > 1);
            for v in pc.values_mut() {
                *v -= 1;
            }
        }
        p.turns_elapsed += 1;
        p.haste_steps = Some(0);
        for s in p.statuses.iter_mut() {
            s.turns -= 1;
        }
        p.statuses.retain(|s| s.turns > 0);
        for v in p.cooldowns.values_mut() {
            *v = (*v - 1).max(0);
        }
        p.cooldowns.retain(|_, v| *v > 0);

        if self.in_town() {
            self.restore_mana(2);
            let (px, py) = (self.player.x, self.player.y);
            let far = self
                .player
                .companion
                .as_ref()
                .is_some_and(|c| chebyshev(c.x, c.y, px, py) > 2);
            let pos = if far { Some(place_companion_near(self.zone(), px, py)) } else { None };
            if let Some(c) = self.player.companion.as_mut() {
                c.hp = c.max_hp;
                c.respawn_cooldown = 0;
                if let Some(np) = pos {
                    c.x = np.x;
                    c.y = np.y;
                }
            }
            self.refresh_visibility();
            return SessionEvent::None;
        }

        if let Some(regen) = prev_regen {
            self.heal_player(regen.potency);
        }
        if has_prodigy(&self.player, ProdigyId::DraconicWill) && self.player.turns_elapsed % 4 == 0 {
            let bad = self.player.statuses.iter().position(|s| {
                is_dot(s.kind) || matches!(s.kind, StatusKind::Chilled | StatusKind::Stunned)
            });
            if let Some(b) = bad {
                let cleared = self.player.statuses.remove(b);
                self.say(&format!("Draconic Will purges {}!", cleared.kind.as_str().to_uppercase()));
            }
        }
        if self.player.turns_elapsed % 2 == 0 {
            self.restore_mana(1);
        }

        // Alerts use visibility from where the player now stands.
        self.refresh_visibility();
        let stats = self.derived();
        let mut player_died = false;
        let mut event = SessionEvent::None;
        let (px, py) = (self.player.x, self.player.y);

        if !incoming_dots.is_empty() {
            let dmg: i32 = incoming_dots.iter().map(|s| s.potency.max(1)).sum();
            let labels: Vec<String> =
                incoming_dots.iter().map(|s| s.kind.as_str().to_uppercase()).collect();
            let labels = labels.join(" / ");
            self.player.hp -= dmg;
            self.fx("player_hit", px, py);
            self.say(&format!("{labels} sears you for {dmg} dmg!"));
            if self.check_player_fatal(stats.max_hp) {
                player_died = true;
            }
        }

        if !player_died {
            self.run_companion_turn(&stats, &mut event);
            if self.player.hp <= 0 {
                player_died = true;
            }
        }

        let sight = self.sight_radius();
        let ids: Vec<String> = self.zone().enemies.iter().map(|e| e.id.clone()).collect();
        for eid in ids {
            if player_died {
                break;
            }
            let Some(i) = self.enemy_idx(&eid) else { continue };
            if self.zone().enemies[i].hp <= 0 || self.zone().enemies[i].pacified == Some(true) {
                continue;
            }

            // Enemy skill cooldowns + DoT tick + status countdown.
            // Stun is read BEFORE the countdown (Garth, Nov 21): TS ticked
            // first, so a 1-turn stun had expired by the time the enemy
            // acted. Now a stun of N turns skips N enemy turns.
            let (dot, was_stunned) = {
                let e = &mut self.zone_mut().enemies[i];
                let was_stunned = e.statuses.iter().any(|s| s.kind == StatusKind::Stunned);
                if let Some(cds) = e.skill_cooldowns.as_mut() {
                    cds.retain(|_, v| *v > 1);
                    for v in cds.values_mut() {
                        *v -= 1;
                    }
                }
                let dot: i32 = e.statuses.iter().filter(|s| is_dot(s.kind)).map(|s| s.potency).sum();
                for s in e.statuses.iter_mut() {
                    s.turns -= 1;
                }
                e.statuses.retain(|s| s.turns > 0);
                e.hp -= dot;
                (dot, was_stunned)
            };
            if dot > 0 && self.zone().enemies[i].hp <= 0 {
                let name = self.zone().enemies[i].name.clone();
                self.say(&format!("{name} succumbs to its wounds."));
                let ev = self.on_enemy_killed(i);
                if ev != SessionEvent::None {
                    event = ev;
                }
                if self.player.hp <= 0 {
                    player_died = true;
                }
                continue;
            }

            let enemy = self.zone().enemies[i].clone();
            if was_stunned {
                self.zone_mut().enemies[i].intent = None;
                self.say(&format!("{} is stunned.", enemy.name));
                continue;
            }
            let dist = chebyshev(enemy.x, enemy.y, px, py);
            let tile_vis = self.zone().tiles[enemy.y as usize][enemy.x as usize].visible;
            if tile_vis && dist <= sight {
                self.zone_mut().enemies[i].alerted = true;
            }
            if !self.zone().enemies[i].alerted {
                continue;
            }

            // Shielding Rune (rare / randboss) below 55% HP.
            if enemy.intent.is_none()
                && enemy.rune_charge == Some(true)
                && (enemy.hp as f64) < enemy.max_hp as f64 * 0.55
            {
                let e = &mut self.zone_mut().enemies[i];
                e.rune_charge = Some(false);
                apply_status(&mut e.statuses, StatusKind::Shielded, 3, 6);
                self.say(&format!("{} activates a Shielding Rune (+6 DEF for 3t)!", enemy.name));
                continue;
            }

            // 1. Resolve a telegraphed intent.
            if let Some(intent) = enemy.intent.clone() {
                self.zone_mut().enemies[i].intent = None;
                let hits = intent.target_tiles.iter().any(|t| t.x == px && t.y == py);
                if let Some(sk) = intent.skill_id {
                    self.zone_mut().enemies[i]
                        .skill_cooldowns
                        .get_or_insert_with(Default::default)
                        .insert(sk.as_str().to_string(), 4);
                    if sk == SkillId::Shadowstep {
                        for (ox, oy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)] {
                            let (bx, by) = (px + ox, py + oy);
                            let z = self.zone();
                            let free = bx >= 0
                                && by >= 0
                                && bx < z.width
                                && by < z.height
                                && z.tiles[by as usize][bx as usize].walkable
                                && !z.enemies.iter().any(|o| o.hp > 0 && o.x == bx && o.y == by);
                            if free {
                                let e = &mut self.zone_mut().enemies[i];
                                e.x = bx;
                                e.y = by;
                                break;
                            }
                        }
                    }
                }
                if hits {
                    let dodged = (self.rng.next_f32() as f64) * 100.0 < stats.dodge_chance;
                    if dodged {
                        self.say(&format!("You evaded {}'s {}!", enemy.name, intent.label));
                        if skill_rank(&self.player, SkillId::EvasiveInstinct) >= 2 {
                            self.restore_mana(2);
                        }
                    } else {
                        let mitigated = 2.max(intent.damage - (stats.def as f64 * 0.7).round() as i32);
                        self.player.hp -= mitigated;
                        self.fx_dir("player_hit", px, py, px - enemy.x, py - enemy.y);
                        self.say(&format!("{} lands {} for {mitigated} dmg!", enemy.name, intent.label));
                        if has_prodigy(&self.player, ProdigyId::SpineOfWorld) {
                            apply_status(&mut self.player.statuses, StatusKind::SpineGuard, 3, 1);
                        }
                        match intent.skill_id {
                            Some(SkillId::EmberLance) => self.try_apply_player_ailment(StatusKind::Burn, 2, 3),
                            Some(SkillId::VenomTip) => self.try_apply_player_ailment(StatusKind::Poison, 3, 3),
                            Some(SkillId::FrostNova) => self.try_apply_player_ailment(StatusKind::Chilled, 2, 1),
                            Some(SkillId::Cleave) => self.try_apply_player_ailment(StatusKind::Bleed, 2, 3),
                            Some(SkillId::ShieldBash) => self.try_apply_player_ailment(StatusKind::Chilled, 1, 1),
                            _ if intent.kind == IntentType::HexBolt
                                && enemy.archetype == EnemyArchetype::RimeRevenant =>
                            {
                                self.try_apply_player_ailment(StatusKind::Chilled, 2, 1)
                            }
                            _ => {}
                        }
                        if self.check_player_fatal(stats.max_hp) {
                            player_died = true;
                            break;
                        }
                    }
                } else {
                    self.say(&format!("You sidestepped {}'s telegraphed attack.", enemy.name));
                }
                continue;
            }

            // 2. Prime a fresh intent, or strike / close in.
            let zone_snapshot = self.zone().clone();
            let enemy = zone_snapshot.enemies[i].clone();
            if let Some(it) = choose_enemy_intent(&enemy, &self.player, &zone_snapshot, &mut self.rng) {
                self.fx("windup", enemy.x, enemy.y);
                self.say(&format!("{} winds up: {}!", enemy.name, it.label));
                self.zone_mut().enemies[i].intent = Some(it);
                continue;
            }

            if dist == 1 {
                let dodged = (self.rng.next_f32() as f64) * 100.0 < stats.dodge_chance;
                if dodged {
                    self.say(&format!("Dodged {}'s strike.", enemy.name));
                    let ev_rank = skill_rank(&self.player, SkillId::EvasiveInstinct);
                    if ev_rank >= 2 {
                        self.restore_mana(2);
                    }
                    if ev_rank >= 3 {
                        let counter = self.player_hit(i, 0.85, false, false, 0.0);
                        self.zone_mut().enemies[i].hp -= counter.damage;
                        self.fx_dir("hit", enemy.x, enemy.y, enemy.x - px, enemy.y - py);
                        self.say(&format!("Phantom Reflex counters {} for {}!", enemy.name, counter.damage));
                        if self.zone().enemies[i].hp <= 0 {
                            let ev = self.on_enemy_killed(i);
                            if ev != SessionEvent::None {
                                event = ev;
                            }
                            if self.player.hp <= 0 {
                                player_died = true;
                                break;
                            }
                        }
                    }
                } else {
                    let chilled = enemy.statuses.iter().any(|s| s.kind == StatusKind::Chilled);
                    let raw = if chilled { (enemy.atk as f64 * 0.65).round() as i32 } else { enemy.atk };
                    let dmg = 1.max(raw - (stats.def as f64 * 0.65).round() as i32);
                    self.player.hp -= dmg;
                    self.fx_dir("player_hit", px, py, px - enemy.x, py - enemy.y);
                    self.say(&format!("{} hits you for {dmg} dmg.", enemy.name));
                    if has_prodigy(&self.player, ProdigyId::SpineOfWorld) {
                        apply_status(&mut self.player.statuses, StatusKind::SpineGuard, 3, 1);
                    }
                    let thorns = stats.thorns + if has_affix(&self.player, ItemAffixId::ThornsBurst) { 4 } else { 0 };
                    if thorns > 0 {
                        self.zone_mut().enemies[i].hp -= thorns;
                        if self.zone().enemies[i].hp <= 0 {
                            let ev = self.on_enemy_killed(i);
                            if ev != SessionEvent::None {
                                event = ev;
                            }
                            if self.player.hp <= 0 {
                                player_died = true;
                                break;
                            }
                        }
                    }
                    if self.check_player_fatal(stats.max_hp) {
                        player_died = true;
                        break;
                    }
                }
            } else {
                let adj_comp = self
                    .player
                    .companion
                    .as_ref()
                    .filter(|c| c.hp > 0 && c.respawn_cooldown == 0)
                    .filter(|c| chebyshev(enemy.x, enemy.y, c.x, c.y) == 1)
                    .map(|c| (c.kind, c.x, c.y));
                if let Some((ckind, cx, cy)) = adj_comp {
                    let chilled = enemy.statuses.iter().any(|s| s.kind == StatusKind::Chilled);
                    let raw_atk = if chilled { (enemy.atk as f64 * 0.65).round() as i32 } else { enemy.atk };
                    let comp_def = (if ckind == CompanionKind::SilverwindLynx { 2 } else { 1 })
                        + self.player.level / 3;
                    let comp_dmg = 1.max(raw_atk - comp_def);
                    let mut collapsed = None;
                    let cname = if let Some(c) = self.player.companion.as_mut() {
                        c.hp = 0.max(c.hp - comp_dmg);
                        if c.hp <= 0 {
                            c.respawn_cooldown = 8;
                            collapsed = Some(c.name.clone());
                        }
                        c.name.clone()
                    } else {
                        String::new()
                    };
                    self.fx("player_hit", cx, cy);
                    self.say(&format!("{} strikes {cname} for {comp_dmg} dmg.", enemy.name));
                    if let Some(cname) = collapsed {
                        self.say(&format!(
                            "{cname} collapses! Will reform in 8 turns or upon returning to town."
                        ));
                    }
                } else if let Some(step) = find_path_next_step(&zone_snapshot, enemy.x, enemy.y, px, py) {
                    let occupied_by_comp = self
                        .player
                        .companion
                        .as_ref()
                        .is_some_and(|c| c.hp > 0 && c.respawn_cooldown == 0 && step.x == c.x && step.y == c.y);
                    let blocked = (step.x == px && step.y == py)
                        || occupied_by_comp
                        || self.zone().enemies.iter().any(|o| o.hp > 0 && o.x == step.x && o.y == step.y);
                    if !blocked {
                        let e = &mut self.zone_mut().enemies[i];
                        e.x = step.x;
                        e.y = step.y;
                    }
                }
            }
        }

        let ev = self.reap_dead();
        if ev != SessionEvent::None {
            event = ev;
        }
        self.player.first_strike_ready = self.zone().enemies.iter().all(|e| !e.alerted);
        self.refresh_visibility();
        if player_died || self.player.hp <= 0 {
            return self.wake_in_oakhaven();
        }
        if self.active_prompt.is_none() {
            if let Some(boss_ev) = self.check_boss_confrontations() {
                return boss_ev;
            }
        }
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::difficulty::DifficultyTier;
    use crate::world::{create_enemy, make_tile};

    /// 15x15 walled floor at depth 3, player at (7,7).
    fn arena(class: StarterClass) -> GameSession {
        let mut s = GameSession::new_game("Garth", class, 99);
        let (w, h) = (15, 15);
        let tiles = (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| {
                        let kind = if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                            TileKind::Wall
                        } else {
                            TileKind::Floor
                        };
                        make_tile(x, y, kind)
                    })
                    .collect()
            })
            .collect();
        let zone = ZoneMap {
            id: ZoneId::Dungeon(3),
            name: "Arena".into(),
            subtitle: String::new(),
            is_town: false,
            depth: 3,
            width: w,
            height: h,
            tiles,
            enemies: vec![],
            npcs: vec![],
            critters: None,
            loot: vec![],
            spawn_point: Pos::new(7, 7),
            return_point: Pos::new(7, 7),
        };
        s.zones.insert(zone.id.to_string_id(), zone);
        s.current = ZoneId::Dungeon(3);
        s.player.x = 7;
        s.player.y = 7;
        s.player.hp = 500;
        s.player.mana = 200;
        s.player.skills.clear();
        s.player.hotbar = vec![None; 5];
        s.player.companion = None;
        s.refresh_visibility();
        s
    }

    fn add_enemy(s: &mut GameSession, x: i32, y: i32, hp: i32) -> String {
        let seq = s.zone().enemies.len() as u32 + 1;
        let mut e = create_enemy(EnemyArchetype::CryptSkeleton, x, y, 3, DifficultyTier::Standard, 0, seq);
        e.id = format!("arena_{seq}");
        e.hp = hp;
        e.max_hp = hp.max(e.max_hp);
        e.skill_loadout = None;
        e.rune_charge = None;
        let id = e.id.clone();
        s.zone_mut().enemies.push(e);
        id
    }

    fn give(s: &mut GameSession, id: SkillId, rank: i32) {
        s.player.skills.insert(id.as_str().to_string(), rank);
        let slot = s.player.hotbar.iter().position(|x| x.is_none()).unwrap();
        s.player.hotbar[slot] = Some(id);
    }

    fn enemy<'a>(s: &'a GameSession, id: &str) -> Option<&'a Enemy> {
        s.zone().enemies.iter().find(|e| e.id == id)
    }

    fn status(e: &Enemy, kind: StatusKind) -> Option<StatusEffect> {
        e.statuses.iter().find(|st| st.kind == kind).copied()
    }

    #[test]
    fn cleave_spends_mana_starts_cooldown_and_splashes_adjacent_foes() {
        let mut s = arena(StarterClass::Vanguard);
        give(&mut s, SkillId::Cleave, 1);
        let a = add_enemy(&mut s, 8, 7, 400);
        let b = add_enemy(&mut s, 7, 8, 400);
        let mana = s.player.mana;

        s.trigger_hotbar(0);
        assert_eq!(s.armed_skill, Some(SkillId::Cleave));
        let aimed = s.aimed_target.expect("auto-aims the nearest foe");
        s.confirm_target();
        assert_eq!(s.armed_skill, None, "cast clears the aim");
        assert_eq!(s.player.mana, mana - 5);
        // TS sets the cooldown to 2, then the enemy turn ticks it to 1.
        assert_eq!(s.player.cooldowns.get("cleave"), Some(&1));
        for id in [&a, &b] {
            assert!(enemy(&s, id).unwrap().hp < 400, "{id} took cleave damage");
        }
        assert!(s.log.iter().any(|l| l.starts_with("Sundering Cleave strikes")));
        let _ = aimed;

        // On cooldown: refused with the TS line, nothing spent.
        let mana = s.player.mana;
        s.trigger_hotbar(0);
        assert_eq!(s.armed_skill, None);
        assert_eq!(s.player.mana, mana);
        assert!(s.log.last().unwrap().contains("is on cooldown (1t)"));
        // One more turn and it's ready.
        s.wait_turn();
        assert!(s.player.cooldowns.get("cleave").is_none());
    }

    #[test]
    fn bastion_bash_cancels_a_telegraphed_attack_and_shields_at_rank_2() {
        let mut s = arena(StarterClass::Vanguard);
        give(&mut s, SkillId::ShieldBash, 2);
        let id = add_enemy(&mut s, 8, 7, 400);
        {
            let e = s.zone_mut().enemies.iter_mut().find(|e| e.id == id).unwrap();
            e.alerted = true;
            e.intent = Some(EnemyIntent {
                kind: IntentType::HeavyWindup,
                label: "Crushing Blow".into(),
                damage: 999,
                target_tiles: vec![Pos::new(7, 7)],
                skill_id: None,
            });
        }
        s.execute_skill(SkillId::ShieldBash, 8, 7);
        assert!(s.player.hp > 0 && s.player.hp > 400, "the 999 windup never landed");
        assert!(!s.log.iter().any(|l| l.contains("Crushing Blow for")));
        // Rank 2 shield: (2t, +4) ticked once by the enemy turn.
        let sh = s.player.statuses.iter().find(|x| x.kind == StatusKind::Shielded).unwrap();
        assert_eq!((sh.turns, sh.potency), (1, 4));
    }

    #[test]
    fn a_one_turn_bash_stun_skips_the_enemys_next_turn() {
        let mut s = arena(StarterClass::Vanguard);
        give(&mut s, SkillId::ShieldBash, 1);
        let id = add_enemy(&mut s, 8, 7, 400);
        s.zone_mut().enemies[0].alerted = true;
        let hp = s.player.hp;
        s.execute_skill(SkillId::ShieldBash, 8, 7);
        assert_eq!(s.player.hp, hp, "the bashed foe loses its turn");
        assert!(s.log.last().unwrap().ends_with("is stunned."));
        // The stun is spent: next turn it acts again (strike or windup).
        assert!(status(enemy(&s, &id).unwrap(), StatusKind::Stunned).is_none());
        let log_len = s.log.len();
        s.wait_turn();
        assert!(
            s.log[log_len..].iter().any(|l| l.contains("Crypt Sentinel")),
            "acts after the stun: {:?}",
            &s.log[log_len..]
        );
    }

    #[test]
    fn stunned_enemies_skip_their_turn() {
        let mut s = arena(StarterClass::Vanguard);
        let id = add_enemy(&mut s, 8, 7, 400);
        {
            let e = s.zone_mut().enemies.iter_mut().find(|e| e.id == id).unwrap();
            e.alerted = true;
            apply_status(&mut e.statuses, StatusKind::Stunned, 3, 1);
        }
        let hp = s.player.hp;
        s.wait_turn();
        s.wait_turn();
        assert_eq!(s.player.hp, hp, "a stunned foe neither strikes nor winds up");
        assert!(enemy(&s, &id).unwrap().intent.is_none());
    }

    #[test]
    fn viper_edge_poison_ticks_on_the_enemy_turn() {
        let mut s = arena(StarterClass::Shadowcraft);
        give(&mut s, SkillId::VenomTip, 1);
        let id = add_enemy(&mut s, 10, 7, 400);
        // Out of range (4) and no line of sight cases are refused.
        let far = add_enemy(&mut s, 12, 12, 400);
        let mana = s.player.mana;
        s.execute_skill(SkillId::VenomTip, 12, 12);
        assert_eq!(s.player.mana, mana, "refused cast spends nothing");
        assert!(s.log.last().unwrap().contains("Select a visible enemy within range 4"));
        let _ = far;

        s.execute_skill(SkillId::VenomTip, 10, 7);
        let e = enemy(&s, &id).unwrap();
        // Applied 3t, already ticked once (and dealt 3) on the enemy turn.
        assert_eq!(status(e, StatusKind::Poison).map(|p| (p.turns, p.potency)), Some((2, 3)));
        // The next turn ticks exactly the poison (the foe is still closing in).
        let before = e.hp;
        s.wait_turn();
        assert_eq!(enemy(&s, &id).unwrap().hp, before - 3);
    }

    #[test]
    fn executioners_mark_and_ember_lance_apply_their_statuses() {
        let mut s = arena(StarterClass::Runecaster);
        give(&mut s, SkillId::ExecutionersMark, 1);
        give(&mut s, SkillId::EmberLance, 1);
        let id = add_enemy(&mut s, 7, 3, 900);
        s.execute_skill(SkillId::ExecutionersMark, 7, 3);
        let e = enemy(&s, &id).unwrap();
        assert_eq!(status(e, StatusKind::Marked).map(|m| (m.turns, m.potency)), Some((3, 35)));
        let (x, y) = (e.x, e.y);
        s.execute_skill(SkillId::EmberLance, x, y);
        let e = enemy(&s, &id).unwrap();
        assert_eq!(status(e, StatusKind::Burn).map(|b| (b.turns, b.potency)), Some((1, 3)));
        assert!(s.log.iter().any(|l| l.starts_with("Ember Lance sears")));
    }

    #[test]
    fn ember_lance_rank_2_pierces_one_tile_behind_the_target() {
        for (rank, expect_pierce) in [(1, false), (2, true)] {
            let mut s = arena(StarterClass::Runecaster);
            give(&mut s, SkillId::EmberLance, rank);
            let target = add_enemy(&mut s, 9, 9, 900); // diagonal from (7,7)
            let behind = add_enemy(&mut s, 10, 10, 900); // next tile on that line
            let beside = add_enemy(&mut s, 10, 9, 900); // adjacent, not behind
            s.execute_skill(SkillId::EmberLance, 9, 9);
            assert!(enemy(&s, &target).unwrap().hp < 900);
            let b = enemy(&s, &behind).unwrap();
            assert_eq!(b.hp < 900, expect_pierce, "rank {rank} pierce");
            assert_eq!(status(b, StatusKind::Burn).is_some(), expect_pierce);
            assert_eq!(enemy(&s, &beside).unwrap().hp, 900, "only the tile behind");
            assert_eq!(
                s.log.iter().any(|l| l.starts_with("Ember Lance pierces through")),
                expect_pierce
            );
        }
    }

    #[test]
    fn rime_nova_hits_and_chills_only_within_its_radius() {
        let mut s = arena(StarterClass::Runecaster);
        give(&mut s, SkillId::FrostNova, 1);
        let near = add_enemy(&mut s, 9, 7, 400);
        let far = add_enemy(&mut s, 11, 7, 400);
        s.trigger_hotbar(0); // self skill: fires immediately
        assert!(enemy(&s, &near).unwrap().hp < 400);
        assert!(status(enemy(&s, &near).unwrap(), StatusKind::Chilled).is_some());
        assert!(status(enemy(&s, &far).unwrap(), StatusKind::Chilled).is_none());
        assert!(s.log.iter().any(|l| l == "Rime Nova freezes 1 foe(s)!"));
    }

    #[test]
    fn shadowstep_blinks_to_a_valid_tile_and_ambushes() {
        let mut s = arena(StarterClass::Shadowcraft);
        give(&mut s, SkillId::Shadowstep, 1);
        let id = add_enemy(&mut s, 11, 7, 400);
        s.trigger_hotbar(0);
        assert_eq!(s.aimed_target, Some(Pos::new(7, 7)), "cursor starts on the player");
        // Own tile, a wall and out-of-range tiles are refused for free.
        let mana = s.player.mana;
        s.confirm_target();
        s.execute_skill(SkillId::Shadowstep, 0, 7);
        s.execute_skill(SkillId::Shadowstep, 13, 13);
        assert_eq!(s.player.mana, mana);
        assert_eq!((s.player.x, s.player.y), (7, 7));
        assert!(s.log.last().unwrap().contains("within Shadowstep range"));
        // Cursor to (10,7), beside the foe: blink + ambush.
        for _ in 0..3 {
            s.move_aim(1, 0);
        }
        s.confirm_target();
        assert_eq!((s.player.x, s.player.y), (10, 7));
        assert!(enemy(&s, &id).unwrap().hp < 400);
        assert!(s.log.iter().any(|l| l.starts_with("Shadowstep ambush hits")));
    }

    #[test]
    fn iron_fortress_raises_defense_for_its_duration() {
        let mut s = arena(StarterClass::Vanguard);
        give(&mut s, SkillId::IronFortress, 1);
        let def = s.derived().def;
        s.trigger_hotbar(0);
        let sh = s.player.statuses.iter().find(|x| x.kind == StatusKind::Shielded).unwrap();
        assert_eq!((sh.turns, sh.potency), (2, 8));
        assert_eq!(s.derived().def, def + 8);
        s.wait_turn();
        s.wait_turn();
        assert_eq!(s.derived().def, def, "ward expires");
    }

    #[test]
    fn turns_tick_player_dots_and_regenerate_mana_every_other_turn() {
        let mut s = arena(StarterClass::Vanguard);
        s.player.mana = 0;
        apply_status(&mut s.player.statuses, StatusKind::Poison, 3, 3);
        let hp = s.player.hp;
        s.player.turns_elapsed = 1; // next turn is even
        s.wait_turn();
        assert_eq!(s.player.hp, hp - 3);
        assert_eq!(s.player.mana, 1);
        assert!(s.log.last().unwrap().contains("POISON sears you for 3 dmg!"));
        s.wait_turn();
        assert_eq!(s.player.mana, 1, "no regen on odd turns");
    }

    #[test]
    fn enemy_dot_kills_pay_out_and_soul_siphon_restores_mana() {
        let mut s = arena(StarterClass::Runecaster);
        s.player.skills.insert("siphon_soul".into(), 1);
        let id = add_enemy(&mut s, 12, 12, 2);
        apply_status(&mut s.zone_mut().enemies[0].statuses, StatusKind::Burn, 2, 5);
        s.player.mana = 0;
        let (kills, gold) = (s.player.kills, s.player.gold);
        s.player.turns_elapsed = 2; // odd next turn: no passive regen
        s.wait_turn();
        assert!(enemy(&s, &id).is_none(), "burned to death");
        assert_eq!(s.player.kills, kills + 1);
        assert!(s.player.gold > gold);
        assert_eq!(s.player.mana, 3, "Soul Siphon r1: +3 MP per kill");
    }

    #[test]
    fn skill_ids_parse_from_their_ts_strings() {
        assert_eq!(SkillId::parse("shield_bash"), Some(SkillId::ShieldBash));
        assert_eq!(SkillId::parse("nope"), None);
        for def in crate::skills::all_skills_list() {
            assert_eq!(SkillId::parse(def.id.as_str()), Some(def.id));
        }
        // Record how serde spells it today (variant names, not TS ids).
        println!("serde SkillId: {}", serde_json::to_string(&SkillId::ShieldBash).unwrap());
    }

    #[test]
    fn learning_a_skill_spends_a_point_and_fills_the_hotbar() {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 1);
        let pts = s.player.skill_points;
        assert!(pts >= 1);
        let target = [SkillId::ShieldBash, SkillId::Cleave]
            .into_iter()
            .find(|id| skill_rank(&s.player, *id) == 0)
            .unwrap();
        assert!(s.learn_skill(target));
        assert_eq!(s.player.skill_points, pts - 1);
        assert!(s.player.hotbar.contains(&Some(target)));
        // Passives never take a hotbar slot.
        s.player.skill_points = 5;
        s.player.skills.insert("cleave".into(), 3);
        s.player.skills.insert("shield_bash".into(), 3);
        if s.learn_skill(SkillId::Bloodlust) {
            assert!(!s.player.hotbar.contains(&Some(SkillId::Bloodlust)));
        }
    }

    #[test]
    fn targets_sort_nearest_first_and_cycle() {
        let mut s = arena(StarterClass::Runecaster);
        give(&mut s, SkillId::EmberLance, 1);
        add_enemy(&mut s, 7, 4, 50);
        add_enemy(&mut s, 9, 7, 50);
        add_enemy(&mut s, 13, 13, 50); // out of range
        assert_eq!(s.in_range_enemies(SkillId::EmberLance), vec![Pos::new(9, 7), Pos::new(7, 4)]);
        s.trigger_hotbar(0);
        assert_eq!(s.aimed_target, Some(Pos::new(9, 7)));
        s.cycle_target();
        assert_eq!(s.aimed_target, Some(Pos::new(7, 4)));
        s.cycle_target();
        assert_eq!(s.aimed_target, Some(Pos::new(9, 7)));
        s.trigger_hotbar(0); // same slot again disarms
        assert_eq!(s.armed_skill, None);
    }

    #[test]
    fn companion_fights_heals_swaps_and_reforms() {
        let owl_run = GameSession::new_game("Garth", StarterClass::Runecaster, 1);
        assert_eq!(owl_run.player.companion.as_ref().unwrap().kind, CompanionKind::StarlightOwl);
        assert_eq!(owl_run.sight_radius(), SIGHT_RADIUS + 1);

        let lynx_run = GameSession::new_game("Garth", StarterClass::Vanguard, 1);
        assert_eq!(lynx_run.player.companion.as_ref().unwrap().kind, CompanionKind::SilverwindLynx);
        assert_eq!(lynx_run.sight_radius(), SIGHT_RADIUS);

        // Lynx attacks adjacent enemy and mends player on turns divisible by 3.
        let mut s = arena(StarterClass::Vanguard);
        s.player.companion = Some(make_companion(CompanionKind::SilverwindLynx, 8, 7, 1));
        s.player.hp = 20;
        s.player.turns_elapsed = 2; // next turn is 3
        let eid = add_enemy(&mut s, 9, 7, 100);
        s.wait_turn();
        assert!(enemy(&s, &eid).unwrap().hp < 100, "lynx strikes adjacent enemy");
        assert_eq!(s.player.hp, 22, "lynx mends +2 HP on turn 3");

        // Player swaps tiles with active companion in dungeon.
        s.zone_mut().enemies.clear();
        s.player.x = 7;
        s.player.y = 7;
        s.player.companion.as_mut().unwrap().x = 8;
        s.player.companion.as_mut().unwrap().y = 7;
        s.move_player(1, 0);
        assert_eq!((s.player.x, s.player.y), (8, 7));
        let comp = s.player.companion.as_ref().unwrap();
        assert_eq!((comp.x, comp.y), (7, 7));

        // Enemy knocks out low-HP companion; it reforms after 8 turns.
        let mut s2 = arena(StarterClass::Vanguard);
        s2.player.companion = Some(make_companion(CompanionKind::SilverwindLynx, 8, 7, 1));
        s2.player.companion.as_mut().unwrap().hp = 1;
        let eid2 = add_enemy(&mut s2, 9, 7, 200);
        // Move companion away from melee? Wait: companion strikes first, then enemy strikes companion.
        s2.zone_mut().enemies[0].alerted = true;
        s2.wait_turn();
        assert_eq!(s2.player.companion.as_ref().unwrap().hp, 0);
        assert_eq!(s2.player.companion.as_ref().unwrap().respawn_cooldown, 8);
        assert!(s2.log.iter().any(|l| l.contains("collapses!")));
        let _ = eid2;
        s2.zone_mut().enemies.clear();
        for _ in 0..8 {
            s2.wait_turn();
        }
        let reformed = s2.player.companion.as_ref().unwrap();
        assert_eq!(reformed.respawn_cooldown, 0);
        assert_eq!(reformed.hp, reformed.max_hp);
        assert!(s2.log.iter().any(|l| l.contains("reforms beside you!")));
    }

    #[test]
    fn grave_husk_revives_once_and_spore_hulk_bursts_adjacent() {
        let mut s = arena(StarterClass::Vanguard);
        s.player.attributes.might = 50;
        let mut husk = create_enemy(EnemyArchetype::GraveHusk, 8, 7, 3, DifficultyTier::Standard, 0, 1);
        husk.id = "husk_1".into();
        husk.hp = 5;
        husk.max_hp = 100;
        husk.skill_loadout = None;
        s.zone_mut().enemies.push(husk);

        // First lethal blow triggers Deathless at 30% HP with no kill credit.
        s.move_player(1, 0);
        assert_eq!(s.zone().enemies.len(), 1);
        assert_eq!(s.zone().enemies[0].hp, 30);
        assert_eq!(s.zone().enemies[0].undying_used, Some(true));
        assert_eq!(s.player.kills, 0);
        assert!(s.log.iter().any(|l| l.contains("refuses death")));

        // Second lethal blow kills it for real.
        s.move_player(1, 0);
        assert!(s.zone().enemies.is_empty());
        assert_eq!(s.player.kills, 1);

        // Spore Hulk adjacent death-burst damages and poisons player.
        let mut s2 = arena(StarterClass::Vanguard);
        s2.player.attributes.might = 50;
        let mut spore = create_enemy(EnemyArchetype::SporeHulk, 8, 7, 3, DifficultyTier::Standard, 0, 2);
        spore.id = "spore_1".into();
        spore.hp = 1;
        spore.skill_loadout = None;
        s2.zone_mut().enemies.push(spore);
        let hp_before = s2.player.hp;
        s2.move_player(1, 0);
        // Burst deals 6 + depth*2 = 12 and applies Poison (3t, 3/t), which ticks once at end of turn.
        assert_eq!(s2.player.hp, hp_before - (6 + 3 * 2) - 3);
        assert!(s2.player.statuses.iter().any(|st| st.kind == StatusKind::Poison));
        assert!(s2.log.iter().any(|l| l.contains("Spore Hulk BURSTS")));

        // Spore Hulk killed at range spares the player.
        let mut s3 = arena(StarterClass::Runecaster);
        give(&mut s3, SkillId::EmberLance, 1);
        let mut far_spore = create_enemy(EnemyArchetype::SporeHulk, 10, 7, 3, DifficultyTier::Standard, 0, 3);
        far_spore.id = "spore_2".into();
        far_spore.hp = 1;
        far_spore.skill_loadout = None;
        s3.zone_mut().enemies.push(far_spore);
        let hp_far = s3.player.hp;
        s3.execute_skill(SkillId::EmberLance, 10, 7);
        assert_eq!(s3.player.hp, hp_far);
        assert!(s3.log.iter().any(|l| l.contains("kept your distance")));
    }

    #[test]
    fn haste_steps_grant_free_moves_without_enemy_turn() {
        let mut s = arena(StarterClass::Shadowcraft);
        let eid = add_enemy(&mut s, 7, 8, 200);
        s.zone_mut().enemies[0].alerted = true;
        s.player.haste_steps = Some(2);
        let turns_before = s.player.turns_elapsed;
        let hp_before = s.player.hp;

        s.move_player(1, 0);
        s.move_player(1, 0);
        assert_eq!((s.player.x, s.player.y), (9, 7));
        assert_eq!(s.player.turns_elapsed, turns_before, "haste steps spend zero turns");
        assert_eq!(s.player.hp, hp_before, "adjacent enemy did not act during haste");
        assert_eq!(s.player.haste_steps, Some(0));
        assert!(s.take_fx().iter().any(|f| f.kind == "haste"));

        let _ = eid;
        s.move_player(1, 0);
        assert_eq!(s.player.turns_elapsed, turns_before + 1, "normal turn resumes");
    }
}
