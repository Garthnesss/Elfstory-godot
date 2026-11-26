//! The pack: equipment, consumables, selling, tempering, attribute points,
//! capstones and resting. Ported from App.tsx's inventory handlers, which the
//! Milestone 3 port missed (loot could be picked up but never worn or drunk).
//!
//! Every action returns `Ok(line)` (also logged) or `Err(reason)` and changes
//! nothing on failure.

use crate::combat::{apply_status, chebyshev, manhattan};
use crate::loot::{
    format_stat_list, get_salvage_gold_yield, rarity_color, relic_sets, upgrade_equipment_item,
};
use crate::session::{GameSession, SessionEvent};
use crate::skills::{
    all_prodigies_list, attribute_meta, can_learn_prodigy, compute_derived_stats,
    get_equipped_set_counts, has_prodigy, prodigy_def, PRODIGY_SKILL_COST,
};
use crate::types::*;
use serde_json::{json, Value};

pub const PACK_LIMIT: usize = 28;
pub const MAX_UPGRADE: i32 = 3;

pub type PackResult = Result<String, String>;

const SLOTS: [ItemSlot; 5] =
    [ItemSlot::Weapon, ItemSlot::Armor, ItemSlot::Helm, ItemSlot::Amulet, ItemSlot::Ring];

/// TS `itemPowerScore`: one number to sort and compare gear by.
pub fn item_power(item: &Item) -> f64 {
    if item.category != ItemCategory::Equipment {
        return 0.0;
    }
    let s = &item.stats;
    let i = |v: Option<i32>| f64::from(v.unwrap_or(0));
    (i(s.atk) + i(s.def) + i(s.spell_power)) * 1.2
        + i(s.max_hp) * 0.22
        + i(s.max_mana) * 0.25
        + (s.crit_chance.unwrap_or(0.0) + s.dodge_chance.unwrap_or(0.0)) * 0.65
        + s.lifesteal.unwrap_or(0.0) * 0.8
        + i(s.thorns) * 0.5
        + if item.affix.is_some() { 3.0 } else { 0.0 }
}

pub fn sell_price(item: &Item) -> i32 {
    get_salvage_gold_yield(item)
}

pub fn temper_cost(item: &Item) -> i32 {
    (25 + item.upgrade_level * 25).max(8)
}

impl GameSession {
    fn pack_index(&self, id: &str) -> Result<usize, String> {
        self.player.inventory.iter().position(|i| i.id == id).ok_or_else(|| "That item is gone.".to_string())
    }

    /// Re-read max HP/MP after a gear or attribute change; never heals.
    pub(crate) fn settle_stats(&mut self) {
        let d = self.derived();
        let p = &mut self.player;
        p.max_hp = d.max_hp;
        p.max_mana = d.max_mana;
        p.hp = p.hp.min(d.max_hp);
        p.mana = p.mana.min(d.max_mana);
    }

    fn done(&mut self, line: String) -> PackResult {
        self.say(&line);
        Ok(line)
    }

    /// Remove one unit of a stack (or the item) and return that unit.
    fn take_one(&mut self, idx: usize) -> Item {
        let qty = self.player.inventory[idx].quantity.unwrap_or(1);
        if qty > 1 {
            self.player.inventory[idx].quantity = Some(qty - 1);
            let mut one = self.player.inventory[idx].clone();
            one.quantity = Some(1);
            one
        } else {
            self.player.inventory.remove(idx)
        }
    }

    pub fn equip_item(&mut self, id: &str) -> PackResult {
        let idx = self.pack_index(id)?;
        let Some(slot) = self.player.inventory[idx].slot else {
            return Err("That can't be worn.".into());
        };
        let item = self.player.inventory.remove(idx);
        let name = item.name.clone();
        if let Some(prev) = self.player.equipped.insert(slot.as_str().to_string(), item) {
            self.player.inventory.push(prev);
        }
        self.settle_stats();
        self.done(format!("Equipped {name}."))
    }

    pub fn unequip_slot(&mut self, slot: ItemSlot) -> PackResult {
        if !self.player.equipped.contains_key(slot.as_str()) {
            return Err("Nothing is worn there.".into());
        }
        if self.player.inventory.len() >= PACK_LIMIT {
            return Err(format!("Pack is full ({PACK_LIMIT}/{PACK_LIMIT}). Salvage or sell something first."));
        }
        let Some(item) = self.player.equipped.remove(slot.as_str()) else {
            return Err("Nothing is worn there.".into());
        };
        let name = item.name.clone();
        self.player.inventory.push(item);
        self.settle_stats();
        self.done(format!("Unequipped {name}."))
    }

    /// TS handleUseConsumable. Does not spend a turn (as in the web game).
    /// A throw can kill, so the event matters.
    pub fn use_item(&mut self, id: &str) -> Result<SessionEvent, String> {
        if self.player.hp <= 0 || self.active_prompt.is_some() {
            return Err("Not now.".into());
        }
        let idx = self.pack_index(id)?;
        let Some(kind) = self.player.inventory[idx].consumable_kind else {
            return Err("That can't be used.".into());
        };
        let d = self.derived();
        let (px, py) = (self.player.x, self.player.y);
        match kind {
            ConsumableKind::HealPotion => {
                if self.player.hp >= d.max_hp {
                    return Err("You are already at full health.".into());
                }
                self.take_one(idx);
                let floor = if self.flags.mira_potion_tier > 1 { 38 } else { 24 };
                let amount = floor.max((f64::from(d.max_hp) * 0.35).round() as i32);
                self.player.hp = d.max_hp.min(self.player.hp + amount);
                self.fx("heal", px, py);
                self.say(&format!("Drank Crimson Draught (+{amount} HP)."));
            }
            ConsumableKind::ManaPotion => {
                if self.player.mana >= d.max_mana {
                    return Err("Your mana is already full.".into());
                }
                self.take_one(idx);
                let amount = 16.max((f64::from(d.max_mana) * 0.4).round() as i32);
                self.player.mana = d.max_mana.min(self.player.mana + amount);
                self.fx("mana", px, py);
                self.say(&format!("Drank Aether Vial (+{amount} MP)."));
            }
            ConsumableKind::WardTalisman => {
                self.take_one(idx);
                apply_status(&mut self.player.statuses, StatusKind::Shielded, 4, 7);
                self.fx("iron_fortress", px, py);
                self.say("Activated Warden Charm (+7 DEF, 4t).");
            }
            ConsumableKind::ScrollBlink => {
                let spots = self.blink_spots();
                if spots.is_empty() {
                    return Err("There is nowhere safe to phase to.".into());
                }
                self.take_one(idx);
                let to = spots[self.rng.range_usize(spots.len())];
                self.player.x = to.x;
                self.player.y = to.y;
                self.refresh_visibility();
                self.fx("shadowstep", to.x, to.y);
                self.say("Phased to a safer stone tile!");
            }
            ConsumableKind::ThrowingKnife => {
                let z = self.zone();
                let target = z
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.hp > 0 && z.tiles[e.y as usize][e.x as usize].visible)
                    .min_by_key(|(_, e)| manhattan(e.x, e.y, px, py))
                    .map(|(i, _)| i);
                let Some(t) = target else {
                    return Err("No visible enemy to throw a Serrated Dart at.".into());
                };
                self.take_one(idx);
                let (name, ex, ey) = {
                    let e = &mut self.zone_mut().enemies[t];
                    e.hp -= 14;
                    e.alerted = true;
                    apply_status(&mut e.statuses, StatusKind::Bleed, 3, 3);
                    (e.name.clone(), e.x, e.y)
                };
                self.fx_dir("hit", ex, ey, ex - px, ey - py);
                self.say(&format!("Serrated Dart hits {name} for 14 dmg + Bleed."));
                return Ok(self.reap_dead());
            }
        }
        Ok(SessionEvent::None)
    }

    /// Walkable, unoccupied tiles 3 to 7 away that the player could stand on.
    fn blink_spots(&self) -> Vec<Pos> {
        let z = self.zone();
        let (px, py) = (self.player.x, self.player.y);
        let mut out = vec![];
        for y in 1..z.height - 1 {
            for x in 1..z.width - 1 {
                let dist = chebyshev(x, y, px, py);
                let t = &z.tiles[y as usize][x as usize];
                let safe = matches!(t.kind, TileKind::Floor | TileKind::Road);
                // Explored only: a blink must not drop the player past a sealed door.
                if (3..=7).contains(&dist)
                    && t.walkable
                    && t.explored
                    && safe
                    && !z.enemies.iter().any(|e| e.hp > 0 && e.x == x && e.y == y)
                    && !z.npcs.iter().any(|n| n.x == x && n.y == y)
                {
                    out.push(Pos::new(x, y));
                }
            }
        }
        out
    }

    /// The quick-heal key.
    pub fn quick_heal(&mut self) -> Result<SessionEvent, String> {
        let id = self
            .player
            .inventory
            .iter()
            .find(|i| i.consumable_kind == Some(ConsumableKind::HealPotion))
            .map(|i| i.id.clone())
            .ok_or("No Crimson Draughts remaining.")?;
        self.use_item(&id)
    }

    pub fn salvage_item(&mut self, id: &str) -> PackResult {
        let idx = self.pack_index(id)?;
        if self.player.inventory[idx].category != ItemCategory::Equipment {
            return Err("Only gear can be salvaged.".into());
        }
        let item = self.player.inventory.remove(idx);
        let gold = get_salvage_gold_yield(&item);
        self.player.gold += gold;
        self.done(format!("Salvaged {} into +{gold} Gold.", item.name))
    }

    pub fn salvage_common(&mut self) -> PackResult {
        let is_common = |i: &Item| i.category == ItemCategory::Equipment && i.rarity == ItemRarity::Common;
        let (gone, kept): (Vec<Item>, Vec<Item>) =
            std::mem::take(&mut self.player.inventory).into_iter().partition(is_common);
        self.player.inventory = kept;
        if gone.is_empty() {
            return Err("No Common gear to salvage.".into());
        }
        let gold: i32 = gone.iter().map(get_salvage_gold_yield).sum();
        self.player.gold += gold;
        self.done(format!("Salvaged {} Common pieces into +{gold} Gold.", gone.len()))
    }

    /// Sell one unit to a merchant.
    pub fn sell_item(&mut self, id: &str) -> PackResult {
        if !self.in_town() {
            return Err("There is no merchant here.".into());
        }
        let idx = self.pack_index(id)?;
        if self.player.inventory[idx].category == ItemCategory::Quest {
            return Err("You can't sell that.".into());
        }
        let price = sell_price(&self.player.inventory[idx]);
        let item = self.take_one(idx);
        self.player.gold += price;
        self.done(format!("Sold {} for +{price}g.", item.name))
    }

    /// Temper worn or carried gear at the blacksmith.
    pub fn temper_item(&mut self, id: &str) -> PackResult {
        if !self.in_town() {
            return Err("Tempering needs a town forge.".into());
        }
        let worn = self.player.equipped.iter().find(|(_, i)| i.id == id).map(|(k, _)| k.clone());
        let target = match &worn {
            Some(slot) => self.player.equipped[slot].clone(),
            None => self.player.inventory[self.pack_index(id)?].clone(),
        };
        if target.category != ItemCategory::Equipment {
            return Err("Only gear can be tempered.".into());
        }
        if target.upgrade_level >= MAX_UPGRADE {
            return Err(format!("{} can't be tempered further.", target.name));
        }
        let cost = temper_cost(&target);
        if self.player.gold < cost {
            return Err(format!("Tempering costs {cost} gold."));
        }
        self.player.gold -= cost;
        let up = upgrade_equipment_item(&target);
        let name = up.name.clone();
        match worn {
            Some(slot) => {
                self.player.equipped.insert(slot, up);
            }
            None => {
                let idx = self.pack_index(id)?;
                self.player.inventory[idx] = up;
            }
        }
        self.settle_stats();
        self.done(format!("Tempered {name}!"))
    }

    pub fn spend_attribute(&mut self, attr: AttributeKey) -> PackResult {
        if self.player.attribute_points <= 0 {
            return Err("No attribute points to spend.".into());
        }
        let p = &mut self.player;
        p.attribute_points -= 1;
        match attr {
            AttributeKey::Might => p.attributes.might += 1,
            AttributeKey::Agility => p.attributes.agility += 1,
            AttributeKey::Arcana => p.attributes.arcana += 1,
            AttributeKey::Resolve => p.attributes.resolve += 1,
        }
        let d = self.derived();
        let p = &mut self.player;
        p.max_hp = d.max_hp;
        p.max_mana = d.max_mana;
        p.hp = d.max_hp.min(p.hp + 5);
        p.mana = d.max_mana.min(p.mana + 4);
        self.done(format!("{} rises to {}.", attribute_meta(attr).name, self.player.attributes.get(attr)))
    }

    pub fn claim_prodigy(&mut self, id: ProdigyId) -> PackResult {
        let check = can_learn_prodigy(&self.player, id);
        if !check.ok {
            return Err(check.reason.unwrap_or_else(|| "Not yet.".into()));
        }
        let before = self.player.max_hp;
        self.player.skill_points = (self.player.skill_points - PRODIGY_SKILL_COST).max(0);
        self.player.prodigies.get_or_insert_with(Vec::new).push(id);
        let d = self.derived();
        let p = &mut self.player;
        p.max_hp = d.max_hp;
        p.max_mana = d.max_mana;
        p.hp = d.max_hp.min(p.hp + (d.max_hp - before).max(0));
        self.done(format!("Claimed Capstone: {}!", prodigy_def(id).name))
    }

    /// TS handleSafeRest: up to half HP/MP in the field, full in town.
    pub fn safe_rest(&mut self) -> PackResult {
        if self.player.hp <= 0 || self.active_prompt.is_some() {
            return Err("Not now.".into());
        }
        let field = !self.in_town();
        let (px, py) = (self.player.x, self.player.y);
        if field {
            let z = self.zone();
            let threatened = z.enemies.iter().any(|e| {
                e.hp > 0
                    && (z.tiles[e.y as usize][e.x as usize].visible
                        || (e.alerted && chebyshev(e.x, e.y, px, py) <= 8))
            });
            if threatened {
                return Err("Cannot rest safely while enemies are nearby!".into());
            }
        }
        if self
            .player
            .statuses
            .iter()
            .any(|s| matches!(s.kind, StatusKind::Bleed | StatusKind::Burn | StatusKind::Poison))
        {
            return Err("Cannot safely rest while bleeding, burning, or poisoned!".into());
        }
        let d = self.derived();
        let turns = self.player.cooldowns.values().copied().max().unwrap_or(0).clamp(2, 12);
        let cap = |v: i32| if field { v / 2 } else { v };
        let p = &mut self.player;
        p.hp = p.hp.max(cap(d.max_hp));
        p.mana = p.mana.max(cap(d.max_mana));
        p.cooldowns.clear();
        if let Some(cds) = p.prodigy_cooldowns.as_mut() {
            for v in cds.values_mut() {
                *v -= turns;
            }
            cds.retain(|_, v| *v > 0);
        }
        p.statuses.clear();
        p.haste_steps = Some(0);
        p.first_strike_ready = true;
        p.turns_elapsed += turns;
        if let Some(c) = p.companion.as_mut() {
            let was_down = c.respawn_cooldown > 0;
            c.respawn_cooldown = (c.respawn_cooldown - turns).max(0);
            if was_down && c.respawn_cooldown == 0 {
                c.hp = cap(c.max_hp);
                c.x = px;
                c.y = py;
            } else if c.hp > 0 {
                c.hp = c.hp.max(cap(c.max_hp));
            }
        }
        if self.player.companion.as_ref().is_some_and(|c| c.hp > 0 && c.x == px && c.y == py) {
            let pos = crate::combat_turn::place_companion_near(self.zone(), px, py);
            if let Some(c) = self.player.companion.as_mut() {
                c.x = pos.x;
                c.y = pos.y;
            }
        }
        self.fx("heal", px, py);
        self.done(if field {
            format!("Field rest ({turns}t): HP & Mana restored up to half; skills refreshed. Full rest needs a town.")
        } else {
            format!("Rested safely ({turns}t): HP, Mana, and Skills restored.")
        })
    }

    fn item_json(&self, item: &Item, worn: bool) -> Value {
        let current = item.slot.and_then(|s| self.player.equipped.get(s.as_str()));
        let stats: Vec<Value> = format_stat_list(&item.stats)
            .iter()
            .map(|e| {
                let was = match current {
                    Some(c) if !worn => e.key.get(&c.stats),
                    _ => e.num,
                };
                json!({ "label": e.label, "value": e.value, "delta": e.num - was })
            })
            .collect();
        // Stats the worn piece has and this one lacks: a straight loss.
        let mut losses: Vec<Value> = vec![];
        if let (false, Some(c)) = (worn, current) {
            for e in format_stat_list(&c.stats) {
                if e.key.get(&item.stats) == 0.0 {
                    losses.push(json!({ "label": e.label, "value": "0", "delta": -e.num }));
                }
            }
        }
        json!({
            "id": item.id, "name": item.name, "glyph": item.glyph,
            "category": match item.category {
                ItemCategory::Equipment => "equipment",
                ItemCategory::Consumable => "consumable",
                ItemCategory::Quest => "quest",
            },
            "slot": item.slot.map(|s| s.as_str()),
            "rarity": item.rarity.as_str(), "color": rarity_color(item.rarity),
            "quantity": item.quantity.unwrap_or(1),
            "description": item.description, "affix": item.affix_label,
            "stats": stats, "losses": losses,
            "power": item_power(item),
            "upgrade": !worn && item.category == ItemCategory::Equipment
                && item_power(item) > current.map_or(0.0, item_power),
            "sell": sell_price(item),
            "temperCost": if item.category == ItemCategory::Equipment && item.upgrade_level < MAX_UPGRADE {
                Some(temper_cost(item))
            } else {
                None
            },
            "worn": worn,
        })
    }

    /// Everything the pack and character screens draw.
    pub fn pack_json(&self) -> Value {
        let mut items: Vec<&Item> = self.player.inventory.iter().collect();
        let rank = |c: &ItemCategory| match c {
            ItemCategory::Equipment => 0,
            ItemCategory::Consumable => 1,
            ItemCategory::Quest => 2,
        };
        items.sort_by(|a, b| {
            rank(&a.category)
                .cmp(&rank(&b.category))
                .then(item_power(b).total_cmp(&item_power(a)))
                .then(a.name.cmp(&b.name))
        });
        let equipped: Vec<Value> = SLOTS
            .iter()
            .map(|s| {
                json!({
                    "slot": s.as_str(),
                    "item": self.player.equipped.get(s.as_str()).map(|i| self.item_json(i, true)),
                })
            })
            .collect();
        let attrs: Vec<Value> =
            [AttributeKey::Might, AttributeKey::Agility, AttributeKey::Arcana, AttributeKey::Resolve]
                .iter()
                .map(|k| {
                    let m = attribute_meta(*k);
                    json!({ "id": k.as_str(), "name": m.name, "description": m.description, "value": self.player.attributes.get(*k) })
                })
                .collect();
        let defs = relic_sets();
        let mut sets: Vec<Value> = get_equipped_set_counts(&self.player)
            .iter()
            .filter_map(|(id, n)| {
                defs.get(id).map(|d| json!({ "name": d.name, "count": n, "total": d.pieces.len(), "bonus": d.bonus_label, "active": *n >= 2 }))
            })
            .collect();
        sets.sort_by_key(|v| v["name"].as_str().unwrap_or("").to_string());
        let prodigies: Vec<Value> = all_prodigies_list()
            .iter()
            .map(|p| {
                let c = can_learn_prodigy(&self.player, p.id);
                json!({
                    "id": p.id.as_str(), "name": p.name, "glyph": p.glyph, "requires": p.requirement_label,
                    "summary": p.summary, "claimed": has_prodigy(&self.player, p.id),
                    "canLearn": c.ok, "reason": c.reason,
                })
            })
            .collect();
        let d = compute_derived_stats(&self.player, Some(&self.flags), Some(&self.town));
        json!({
            "items": items.iter().map(|i| self.item_json(i, false)).collect::<Vec<_>>(),
            "equipped": equipped,
            "limit": PACK_LIMIT, "count": self.player.inventory.len(),
            "gold": self.player.gold, "inTown": self.in_town(),
            "attributes": attrs, "attributePoints": self.player.attribute_points,
            "skillPoints": self.player.skill_points,
            "prodigies": prodigies, "sets": sets, "derived": d,
            "commonCount": self.player.inventory.iter()
                .filter(|i| i.category == ItemCategory::Equipment && i.rarity == ItemRarity::Common).count(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loot::{create_consumable, create_named_relic, create_quest_item, generate_procedural_item, ProceduralOptions};

    fn session() -> GameSession {
        GameSession::new_game("Garth", StarterClass::Vanguard, 11)
    }

    fn gear(s: &mut GameSession, slot: ItemSlot) -> String {
        let item = generate_procedural_item(&mut s.id_gen, &mut s.rng, 3, ProceduralOptions { slot: Some(slot), min_rarity: None });
        let id = item.id.clone();
        s.player.inventory.push(item);
        id
    }

    #[test]
    fn equipping_swaps_and_changes_stats_and_unequipping_reverses_it() {
        let mut s = session();
        let before = s.derived();
        let starter = s.player.equipped["weapon"].id.clone();
        let id = gear(&mut s, ItemSlot::Weapon);
        s.equip_item(&id).unwrap();
        assert_eq!(s.player.equipped["weapon"].id, id);
        assert!(s.player.inventory.iter().any(|i| i.id == starter), "old weapon returns to the pack");
        s.equip_item(&starter).unwrap();
        assert_eq!(s.derived(), before, "swapping back restores the exact stats");
        s.unequip_slot(ItemSlot::Weapon).unwrap();
        assert!(s.unequip_slot(ItemSlot::Weapon).is_err());
        assert!(s.derived().atk < before.atk);
    }

    #[test]
    fn hp_never_exceeds_max_after_losing_gear() {
        let mut s = session();
        let id = gear(&mut s, ItemSlot::Armor);
        let i = s.pack_index(&id).unwrap();
        s.player.inventory[i].stats.max_hp = Some(40);
        s.equip_item(&id).unwrap();
        s.player.hp = s.derived().max_hp;
        s.unequip_slot(ItemSlot::Armor).unwrap();
        assert_eq!(s.player.hp, s.derived().max_hp);
        assert_eq!(s.player.max_hp, s.derived().max_hp);
    }

    #[test]
    fn full_pack_refuses_unequip_and_loses_nothing() {
        let mut s = session();
        while s.player.inventory.len() < PACK_LIMIT {
            gear(&mut s, ItemSlot::Ring);
        }
        assert!(s.unequip_slot(ItemSlot::Weapon).is_err());
        assert!(s.player.equipped.contains_key("weapon"));
    }

    #[test]
    fn potions_heal_stack_down_and_refuse_at_full() {
        let mut s = session();
        let id = s.player.inventory.iter().find(|i| i.consumable_kind == Some(ConsumableKind::HealPotion)).unwrap().id.clone();
        assert!(s.use_item(&id).is_err(), "full health keeps the potion");
        s.player.hp = 1;
        s.use_item(&id).unwrap();
        assert!(s.player.hp >= 25);
        assert_eq!(s.player.inventory.iter().find(|i| i.id == id).unwrap().quantity, Some(1));
        s.player.hp = 1;
        s.quick_heal().unwrap();
        assert!(s.player.inventory.iter().all(|i| i.id != id));
        s.player.hp = 1;
        assert!(s.quick_heal().is_err());
        assert_eq!(s.player.hp, 1);
    }

    #[test]
    fn dart_needs_a_target_and_kills_pay_out() {
        let mut s = session();
        let dart = create_consumable(&mut s.id_gen, ConsumableKind::ThrowingKnife, 2);
        let id = dart.id.clone();
        s.player.inventory.push(dart);
        assert!(s.use_item(&id).is_err());
        assert_eq!(s.player.inventory.iter().find(|i| i.id == id).unwrap().quantity, Some(2));
        s.debug_warp(ZoneId::Dungeon(1), -1, -1);
        let (px, py) = (s.player.x, s.player.y);
        let mut e = s.zone().enemies[0].clone();
        e.x = px + 1;
        e.y = py;
        e.hp = 5;
        e.archetype = EnemyArchetype::PlagueRat;
        s.zone_mut().tiles[py as usize][(px + 1) as usize].visible = true;
        s.zone_mut().enemies = vec![e];
        let kills = s.player.kills;
        s.use_item(&id).unwrap();
        assert_eq!(s.player.kills, kills + 1);
        assert!(s.zone().enemies.is_empty());
    }

    #[test]
    fn blink_only_lands_on_explored_free_floor() {
        for seed in 1..20 {
            let mut s = GameSession::new_game("G", StarterClass::Wanderer, seed);
            s.debug_warp(ZoneId::Dungeon(1), -1, -1);
            let scroll = create_consumable(&mut s.id_gen, ConsumableKind::ScrollBlink, 1);
            let id = scroll.id.clone();
            s.player.inventory.push(scroll);
            if s.use_item(&id).is_ok() {
                let t = &s.zone().tiles[s.player.y as usize][s.player.x as usize];
                assert!(t.walkable && t.explored);
                assert!(!s.zone().enemies.iter().any(|e| e.x == s.player.x && e.y == s.player.y));
            }
        }
    }

    #[test]
    fn selling_tempering_and_salvage_follow_the_rules() {
        let mut s = session();
        let q = create_quest_item(&mut s.id_gen, "smuggler_ledger");
        let qid = q.id.clone();
        s.player.inventory.push(q);
        assert!(s.sell_item(&qid).is_err(), "quest items are not for sale");
        assert!(s.salvage_item(&qid).is_err());
        let id = gear(&mut s, ItemSlot::Helm);
        s.player.gold = 1000;
        for n in 1..=MAX_UPGRADE {
            s.temper_item(&id).unwrap();
            assert_eq!(s.player.inventory.iter().find(|i| i.id == id).unwrap().upgrade_level, n);
        }
        assert!(s.temper_item(&id).is_err());
        assert_eq!(s.player.gold, 1000 - 25 - 50 - 75);
        let worn = s.player.equipped["weapon"].id.clone();
        s.temper_item(&worn).unwrap();
        assert_eq!(s.player.equipped["weapon"].upgrade_level, 1);
        s.player.gold = 0;
        assert!(s.temper_item(&worn).is_err());
        assert_eq!(s.player.equipped["weapon"].upgrade_level, 1);
        let gold = s.player.gold;
        s.sell_item(&id).unwrap();
        assert!(s.player.gold > gold);
        s.debug_warp(ZoneId::Dungeon(1), -1, -1);
        let id = gear(&mut s, ItemSlot::Helm);
        assert!(s.sell_item(&id).is_err(), "no merchant underground");
        s.salvage_item(&id).unwrap();
    }

    #[test]
    fn selling_a_stack_sells_one() {
        let mut s = session();
        let id = s.player.inventory.iter().find(|i| i.quantity == Some(2)).unwrap().id.clone();
        s.sell_item(&id).unwrap();
        assert_eq!(s.player.inventory.iter().find(|i| i.id == id).unwrap().quantity, Some(1));
    }

    #[test]
    fn attributes_and_capstones() {
        let mut s = session();
        assert!(s.spend_attribute(AttributeKey::Might).is_err());
        s.player.attribute_points = 2;
        let atk = s.derived().atk;
        s.spend_attribute(AttributeKey::Might).unwrap();
        assert!(s.derived().atk > atk);
        assert_eq!(s.player.attribute_points, 1);
        assert!(s.claim_prodigy(ProdigyId::Cauterize).is_err());
        s.player.level = 10;
        s.player.skill_points = 2;
        s.player.attributes = Attributes { might: 20, agility: 20, arcana: 20, resolve: 20 };
        s.claim_prodigy(ProdigyId::Cauterize).unwrap();
        assert_eq!(s.player.skill_points, 0);
        assert!(s.claim_prodigy(ProdigyId::Cauterize).is_err());
    }

    #[test]
    fn rest_is_half_in_the_field_full_in_town_and_never_hurts() {
        let mut s = session();
        s.player.hp = 1;
        s.safe_rest().unwrap();
        assert_eq!(s.player.hp, s.derived().max_hp);
        s.debug_warp(ZoneId::Dungeon(1), -1, -1);
        s.zone_mut().enemies.clear();
        s.player.hp = 1;
        s.safe_rest().unwrap();
        assert_eq!(s.player.hp, s.derived().max_hp / 2);
        s.player.hp = s.derived().max_hp;
        s.safe_rest().unwrap();
        assert_eq!(s.player.hp, s.derived().max_hp);
        apply_status(&mut s.player.statuses, StatusKind::Poison, 3, 3);
        assert!(s.safe_rest().is_err());
    }

    #[test]
    fn pack_json_marks_upgrades_and_lists_every_slot() {
        let mut s = session();
        let relic = create_named_relic(&mut s.id_gen, &mut s.rng, "sylvan_longblade");
        s.player.inventory.push(relic);
        let v = s.pack_json();
        assert_eq!(v["equipped"].as_array().unwrap().len(), 5);
        assert_eq!(v["items"][0]["upgrade"], true);
        assert!(v["items"].as_array().unwrap().iter().all(|i| i["worn"] == false));
    }
}
