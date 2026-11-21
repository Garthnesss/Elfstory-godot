//! World session behaviors for the M3 world tiles (port of the matching
//! branches of App.tsx `interactWithTile`): the sealed treasure vault door,
//! the Seal Brazier hall and its brazier-sealed door, Elven Waystone
//! attunement, and chest looting (ordinary, vault reliquary, side-dungeon
//! boss hoard, overworld satchel) plus restorative braziers.
//!
//! TS opens a `specialPrompt` modal for the vault door, the seal door and
//! the waystone; here that is [`SpecialPrompt`] on the session, surfaced to
//! the front-end as `SessionEvent::OpenPrompt` + `prompt_json` /
//! `choose_prompt` / `close_prompt`. Every TS `Math.random()` draw in these
//! branches comes from the session's seeded RNG instead.

use crate::combat::apply_status;
use crate::loot::{
    create_consumable, create_quest_item, create_random_set_relic, generate_procedural_item,
    ProceduralOptions,
};
use crate::session::{GameSession, SessionEvent};
use crate::skills::compute_derived_stats;
use crate::types::*;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptKind {
    SealDoor,
    Vault,
    Waystone,
}

impl PromptKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PromptKind::SealDoor => "seal_door",
            PromptKind::Vault => "vault",
            PromptKind::Waystone => "waystone",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PromptOption {
    pub id: String,
    pub label: String,
    pub tone: String,
    pub disabled: bool,
}

/// TS `specialPrompt` for the world tiles ported here. `x`/`y` is the tile
/// the prompt acts on.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpecialPrompt {
    pub kind: PromptKind,
    pub x: i32,
    pub y: i32,
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub options: Vec<PromptOption>,
}

fn opt(id: &str, label: String, tone: &str, disabled: bool) -> PromptOption {
    PromptOption { id: id.to_string(), label, tone: tone.to_string(), disabled }
}

fn chebyshev(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    (ax - bx).abs().max((ay - by).abs())
}

/// Turn a vault-door tile into an open door (TS mutation, verbatim fields).
fn unseal(tile: &mut Tile) {
    tile.kind = TileKind::Door;
    tile.glyph = "+".to_string();
    tile.walkable = true;
    tile.blocks_sight = false;
    tile.used = Some(true);
}

impl GameSession {
    /// TS `addItemToPlayer`: consumables stack onto an existing stack of
    /// the same kind; everything else is appended.
    pub(crate) fn add_item_to_player(&mut self, item: Item) {
        if item.category == ItemCategory::Consumable {
            if let Some(existing) = self
                .player
                .inventory
                .iter_mut()
                .find(|i| i.consumable_kind.is_some() && i.consumable_kind == item.consumable_kind)
            {
                existing.quantity = Some(existing.quantity.unwrap_or(1) + item.quantity.unwrap_or(1));
                return;
            }
        }
        self.player.inventory.push(item);
    }

    fn tile_ref(&self, x: i32, y: i32) -> Option<&Tile> {
        let z = self.zone();
        if x < 0 || y < 0 || x >= z.width || y >= z.height {
            return None;
        }
        Some(&z.tiles[y as usize][x as usize])
    }

    fn tile_mut(&mut self, x: i32, y: i32) -> Option<&mut Tile> {
        let (w, h) = (self.zone().width, self.zone().height);
        if x < 0 || y < 0 || x >= w || y >= h {
            return None;
        }
        Some(&mut self.zone_mut().tiles[y as usize][x as usize])
    }

    fn seal_brazier_counts(&self) -> (usize, usize) {
        let braziers: Vec<&Tile> = self
            .zone()
            .tiles
            .iter()
            .flatten()
            .filter(|t| t.interact_id.as_deref() == Some("seal_brazier"))
            .collect();
        let lit = braziers.iter().filter(|t| t.used == Some(true)).count();
        (lit, braziers.len())
    }

    /// TS `interactWithTile` for the ported world tiles. `Some(event)` when
    /// the tile handled the interaction (TS `return true`), `None` when the
    /// tile has nothing to do (TS `return false`).
    pub(crate) fn interact_with_world_tile(&mut self, x: i32, y: i32) -> Option<SessionEvent> {
        let tile = self.tile_ref(x, y)?.clone();
        let used = tile.used == Some(true);
        match tile.kind {
            TileKind::VaultDoor if tile.interact_id.as_deref() == Some("seal_door") => {
                Some(self.open_seal_door_prompt(x, y))
            }
            TileKind::VaultDoor => Some(self.open_vault_prompt(x, y)),
            TileKind::Chest if !used => {
                self.loot_chest(x, y);
                Some(SessionEvent::None)
            }
            TileKind::Brazier if !used => {
                self.light_brazier(x, y);
                Some(SessionEvent::None)
            }
            TileKind::Waystone if !used => Some(self.open_waystone_prompt(x, y)),
            _ => None,
        }
    }

    fn set_prompt(&mut self, prompt: SpecialPrompt) -> SessionEvent {
        let kind = prompt.kind;
        self.active_prompt = Some(prompt);
        SessionEvent::OpenPrompt(kind.as_str().to_string())
    }

    // ------------------------------------------------------------ vault

    fn open_seal_door_prompt(&mut self, x: i32, y: i32) -> SessionEvent {
        // Phase 3 set piece: this door is bound to the room's Seal Braziers.
        // Lighting the last brazier opens it automatically, so reaching
        // this prompt means the seal still holds.
        let (lit, total) = self.seal_brazier_counts();
        let depth = self.zone().depth;
        self.set_prompt(SpecialPrompt {
            kind: PromptKind::SealDoor,
            x,
            y,
            title: "Brazier-Sealed Door".into(),
            subtitle: format!("Depth {depth} · Seal Brazier Hall"),
            body: format!(
                "Rune-chains bind this door to the hall's Seal Braziers (☼). {lit}/{total} burn so far — light every Seal Brazier in this hall and the seal will grind open on its own."
            ),
            options: vec![opt("leave_seal_door", "Step away".into(), "secondary", false)],
        })
    }

    fn open_vault_prompt(&mut self, x: i32, y: i32) -> SessionEvent {
        let depth = self.zone().depth;
        self.set_prompt(SpecialPrompt {
            kind: PromptKind::Vault,
            x,
            y,
            title: "Sealed Treasure Vault".into(),
            subtitle: format!("Depth {depth} · High-Risk Chamber"),
            body: "Heavy obsidian wards bind this vault door. Rare or Randboss guardians watch over reliquary chests inside. Unseal the chamber and prepare for battle?".into(),
            options: vec![
                opt("open_vault", "Break the Seal & Open the Vault Door".into(), "danger", false),
                opt("leave_vault", "Leave the Vault sealed for now".into(), "secondary", false),
            ],
        })
    }

    fn open_vault(&mut self, x: i32, y: i32) {
        let Some(t) = self.tile_mut(x, y) else { return };
        if t.kind != TileKind::VaultDoor {
            return;
        }
        unseal(t);
        self.fx("unseal", x, y);
        for e in self.zone_mut().enemies.iter_mut() {
            if chebyshev(e.x, e.y, x, y) <= 8 {
                e.alerted = true;
            }
        }
        self.tile_rev += 1;
        self.say("Vault unsealed! Ancient guardians stir within the chamber.");
    }

    // ------------------------------------------------------------ chests

    fn loot_chest(&mut self, x: i32, y: i32) {
        let interact_id = {
            let Some(t) = self.tile_mut(x, y) else { return };
            t.used = Some(true);
            t.interact_id.clone()
        };
        let depth = self.zone().depth;
        self.tile_rev += 1;
        self.fx("chest", x, y);
        let id = interact_id.as_deref().unwrap_or("");

        if id == "overworld_satchel_chest" {
            let found_gold = 45;
            let found_item = generate_procedural_item(
                &mut self.id_gen,
                &mut self.rng,
                2,
                ProceduralOptions { slot: None, min_rarity: Some(ItemRarity::Rare) },
            );
            self.player.gold += found_gold;
            let satchel = create_quest_item(&mut self.id_gen, "wayfarer_satchel");
            self.add_item_to_player(satchel);
            let name = found_item.name.clone();
            self.add_item_to_player(found_item);
            if let Some(q) = self.quests.get_mut("silverwood_road") {
                q.stage = 2;
                q.history.push(
                    "Reclaimed Rowan’s Stolen Satchel from the Silverbridge Tollhouse chest.".into(),
                );
            }
            self.say(&format!(
                "Recovered Rowan’s Stolen Satchel (+{found_gold}g & {name})! Return to Rowan at Silverbridge."
            ));
            return;
        }

        if id.ends_with("_boss_chest") {
            let found_gold = 90 + depth * 22;
            let found_item = create_random_set_relic(&mut self.id_gen, &mut self.rng, depth);
            self.player.gold += found_gold + 10;
            let name = found_item.name.clone();
            self.add_item_to_player(found_item);
            self.say(&format!(
                "Side-Dungeon Hoard claimed: +{}g and {name}!",
                found_gold + 10
            ));
            return;
        }

        if id == "vault_chest" {
            let found_gold = 60 + depth * 24 + (self.rng.next_f32() * 25.0).floor() as i32;
            let found_item = generate_procedural_item(
                &mut self.id_gen,
                &mut self.rng,
                depth + 2,
                ProceduralOptions { slot: None, min_rarity: Some(ItemRarity::Epic) },
            );
            self.player.gold += found_gold;
            let name = found_item.name.clone();
            self.add_item_to_player(found_item);
            self.say(&format!("Vault Reliquary looted: +{found_gold}g and {name}!"));
            return;
        }

        let found_gold = 18 + depth * 14 + (self.rng.next_f32() * 14.0).floor() as i32;
        let found_item = generate_procedural_item(
            &mut self.id_gen,
            &mut self.rng,
            depth + 1,
            ProceduralOptions { slot: None, min_rarity: Some(ItemRarity::Uncommon) },
        );
        self.player.gold += found_gold;
        let name = found_item.name.clone();
        self.add_item_to_player(found_item);
        if self.rng.next_f32() < 0.5 {
            let potion = create_consumable(&mut self.id_gen, ConsumableKind::HealPotion, 1);
            self.add_item_to_player(potion);
        } else if self.rng.next_f32() < 0.35 {
            let potion = create_consumable(&mut self.id_gen, ConsumableKind::ManaPotion, 1);
            self.add_item_to_player(potion);
        }
        self.say(&format!("Opened chest: +{found_gold}g and {name}!"));
    }

    // ------------------------------------------------------------ braziers

    fn light_brazier(&mut self, x: i32, y: i32) {
        let interact_id = {
            let Some(t) = self.tile_mut(x, y) else { return };
            t.used = Some(true);
            t.interact_id.clone()
        };
        self.tile_rev += 1;
        self.fx("brazier", x, y);
        let d = compute_derived_stats(&self.player, Some(&self.flags), Some(&self.town));
        self.player.max_hp = d.max_hp;
        self.player.hp = d.max_hp;
        self.player.max_mana = d.max_mana;
        self.player.mana = d.max_mana;
        self.player.statuses.clear();

        match interact_id.as_deref() {
            Some("seal_brazier") => {
                // Lighting the last Seal Brazier grinds the hall's
                // Brazier-Sealed Door open.
                let (lit, total) = self.seal_brazier_counts();
                if lit >= total && total > 0 {
                    let mut opened = vec![];
                    for t in self.zone_mut().tiles.iter_mut().flatten() {
                        if t.interact_id.as_deref() == Some("seal_door")
                            && t.kind == TileKind::VaultDoor
                        {
                            unseal(t);
                            t.label = Some("Unsealed Reliquary Door".into());
                            opened.push((t.x, t.y));
                        }
                    }
                    for &(dx, dy) in &opened {
                        self.fx("unseal", dx, dy);
                    }
                    let opened = !opened.is_empty();
                    if opened {
                        self.say(
                            "The last Seal Brazier catches — somewhere in the hall, a rune-seal grinds open!",
                        );
                    }
                } else {
                    self.say(&format!(
                        "Seal Brazier lit ({lit}/{total}). Its warmth restores you as the seal weakens."
                    ));
                }
            }
            Some("poi_hunter_camp") => self.say(
                "A hunter's cold camp, recently left: cold ash, a skinned rabbit, and a note — 'Fox took the good snare. Vale provides.' The banked embers still mend you.",
            ),
            _ => self.say("Warm brazier embers mend your wounds and refill your mana."),
        }
    }

    // ------------------------------------------------------------ waystones

    fn open_waystone_prompt(&mut self, x: i32, y: i32) -> SessionEvent {
        let depth = self.zone().depth;
        let a = self.player.attributes;
        self.set_prompt(SpecialPrompt {
            kind: PromptKind::Waystone,
            x,
            y,
            title: "Elven Starlight Waystone".into(),
            subtitle: format!("Depth {depth} · Ancient Outrider Cairn"),
            body: "Silver-inlaid obelisks pulse softly with starlight left by earlier elven outriders. As Garth approaches, the runes respond to his tread.".into(),
            options: vec![
                opt(
                    "attune_waystone",
                    "Attune the Waystone (Restore 35% HP & MP, +4 DEF Ward 4t)".into(),
                    "primary",
                    false,
                ),
                // Label verbatim from TS (it says "+8 Dust"; the effect and
                // log line grant +15 Gold).
                opt(
                    "surge_waystone",
                    format!(
                        "[Arcana {}/3 or Resolve {}/3] Draw Starlight Surge (Reset Cooldowns, Full MP, +8 Dust)",
                        a.arcana, a.resolve
                    ),
                    "secondary",
                    a.arcana < 3 && a.resolve < 3,
                ),
                opt("leave_waystone", "Leave the Waystone untouched".into(), "secondary", false),
            ],
        })
    }

    fn attune_waystone(&mut self, x: i32, y: i32, surge: bool) {
        let d = compute_derived_stats(&self.player, Some(&self.flags), Some(&self.town));
        let log_msg = if surge {
            self.player.mana = d.max_mana;
            self.player.cooldowns.clear();
            self.player.gold += 15;
            "Starlight Surge resets all cooldowns, refills Mana, and yields +15 Gold!"
        } else {
            let hp_gain = 8.max((d.max_hp as f64 * 0.35).round() as i32);
            let mana_gain = 5.max((d.max_mana as f64 * 0.35).round() as i32);
            self.player.hp = d.max_hp.min(self.player.hp + hp_gain);
            self.player.mana = d.max_mana.min(self.player.mana + mana_gain);
            apply_status(&mut self.player.statuses, StatusKind::Shielded, 4, 4);
            "Starlight mends your vigor (+35% HP/MP & +4 DEF Ward) and attunes the Waystone!"
        };

        // recordWaystone
        self.fx("waystone", x, y);
        if let Some(t) = self.tile_mut(x, y) {
            t.used = Some(true);
        }
        self.tile_rev += 1;
        let next_attuned = self.flags.waystones_attuned.unwrap_or(0) + 1;
        self.flags.waystones_attuned = Some(next_attuned);
        if !self.flags.read_lore_ids.iter().any(|l| l == "lore_waystone") {
            self.flags.read_lore_ids.push("lore_waystone".into());
        }
        let z = self.zone();
        let place = if z.depth > 0 {
            format!("on Floor {}", z.depth)
        } else {
            format!("in {}", z.name)
        };
        let target_stage = if next_attuned >= 2 { 3 } else { 2 };
        let shown = next_attuned.min(2);
        match self.quests.get_mut("outrider_oath") {
            None => {
                self.quests.insert(
                    "outrider_oath".into(),
                    QuestProgress {
                        id: "outrider_oath".into(),
                        status: QuestStatus::Active,
                        stage: target_stage,
                        outcome_label: None,
                        history: vec![format!("Attuned an Elven Waystone {place} ({shown}/2).")],
                    },
                );
            }
            Some(q) if q.status == QuestStatus::Completed => {}
            Some(q) => {
                q.stage = q.stage.max(target_stage);
                q.history.push(format!("Attuned Elven Waystone {place} ({shown}/2)."));
            }
        }
        self.say(log_msg);
    }

    // ------------------------------------------------------------ prompt API

    /// Current prompt as JSON, or `null` when none is open.
    pub fn prompt_json(&self) -> Value {
        match &self.active_prompt {
            Some(p) => serde_json::to_value(p).unwrap_or(Value::Null),
            None => Value::Null,
        }
    }

    pub fn close_prompt(&mut self) {
        self.active_prompt = None;
    }

    /// Resolve a prompt option. Disabled or unknown options leave the
    /// prompt open and change nothing.
    pub fn choose_prompt(&mut self, option_id: &str) -> SessionEvent {
        let Some(prompt) = self.active_prompt.clone() else {
            return SessionEvent::None;
        };
        let Some(choice) = prompt.options.iter().find(|o| o.id == option_id) else {
            return SessionEvent::None;
        };
        if choice.disabled {
            return SessionEvent::None;
        }
        self.active_prompt = None;
        match (prompt.kind, option_id) {
            (PromptKind::Vault, "open_vault") => self.open_vault(prompt.x, prompt.y),
            (PromptKind::Waystone, "attune_waystone") => {
                self.attune_waystone(prompt.x, prompt.y, false)
            }
            (PromptKind::Waystone, "surge_waystone") => {
                self.attune_waystone(prompt.x, prompt.y, true)
            }
            _ => {} // leave / step away
        }
        SessionEvent::None
    }

    /// Positions of used tiles in the current zone (looted chests, lit
    /// braziers, attuned waystones) for the front-end.
    pub fn used_tiles(&self) -> Vec<[i32; 2]> {
        self.zone()
            .tiles
            .iter()
            .flatten()
            .filter(|t| t.used == Some(true))
            .map(|t| [t.x, t.y])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::difficulty::DifficultyTier;
    use crate::rng::Rng;

    fn session() -> GameSession {
        let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, 7);
        s.player.hp = 5000;
        s
    }

    /// Put `zone` in the session as the current zone.
    fn load(s: &mut GameSession, zone: ZoneMap) {
        let id = zone.id.clone();
        s.zones.insert(id.to_string_id(), zone);
        s.current = id;
    }

    /// First generated zone (over seeds) satisfying `pred`.
    fn find_zone(id: ZoneId, pred: impl Fn(&ZoneMap) -> bool) -> ZoneMap {
        for seed in 1..400u32 {
            let mut rng = Rng::new(seed);
            let z = crate::world::generate_zone_by_id(id.clone(), &mut rng, DifficultyTier::Standard, 0)
                .expect("zone generates");
            if pred(&z) {
                return z;
            }
        }
        panic!("no seed produced the wanted {id} layout");
    }

    fn tiles_where(z: &ZoneMap, f: impl Fn(&Tile) -> bool) -> Vec<(i32, i32)> {
        z.tiles.iter().flatten().filter(|t| f(t)).map(|t| (t.x, t.y)).collect()
    }

    fn walkable_neighbor(z: &ZoneMap, x: i32, y: i32) -> (i32, i32) {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx >= 0 && ny >= 0 && nx < z.width && ny < z.height {
                let t = &z.tiles[ny as usize][nx as usize];
                if t.walkable && t.kind != TileKind::Chest && t.kind != TileKind::Brazier {
                    return (nx, ny);
                }
            }
        }
        panic!("no walkable neighbor at ({x},{y})");
    }

    fn is_vault(t: &Tile) -> bool {
        t.kind == TileKind::VaultDoor && t.interact_id.as_deref() != Some("seal_door")
    }

    #[test]
    fn vault_door_bump_prompts_and_breaking_the_seal_opens_and_alerts() {
        let z = find_zone(ZoneId::Dungeon(3), |z| z.tiles.iter().flatten().any(is_vault));
        let (vx, vy) = tiles_where(&z, is_vault)[0];
        let (px, py) = walkable_neighbor(&z, vx, vy);
        let mut s = session();
        load(&mut s, z);
        for e in s.zone_mut().enemies.iter_mut() {
            e.alerted = false;
        }
        s.player.x = px;
        s.player.y = py;

        // Bumping the door opens the prompt and does not move the player.
        let ev = s.move_player(vx - px, vy - py);
        assert_eq!(ev, SessionEvent::OpenPrompt("vault".into()));
        assert_eq!((s.player.x, s.player.y), (px, py));
        // Moves are ignored while a prompt is open.
        assert_eq!(s.move_player(vx - px, vy - py), SessionEvent::None);

        // Leaving keeps it sealed.
        s.choose_prompt("leave_vault");
        assert!(s.active_prompt.is_none());
        assert_eq!(s.zone().tiles[vy as usize][vx as usize].kind, TileKind::VaultDoor);

        // Breaking the seal opens it and wakes nearby guardians.
        s.move_player(vx - px, vy - py);
        let rev = s.tile_rev;
        s.choose_prompt("open_vault");
        let door = &s.zone().tiles[vy as usize][vx as usize];
        assert_eq!(door.kind, TileKind::Door);
        assert!(door.walkable && !door.blocks_sight && door.used == Some(true));
        assert!(s.tile_rev > rev, "front-end must see the tile change");
        for e in &s.zone().enemies {
            assert_eq!(e.alerted, chebyshev(e.x, e.y, vx, vy) <= 8, "{} alert radius", e.id);
        }
        assert!(s.log.iter().any(|l| l.starts_with("Vault unsealed!")));

        // Now walkable.
        s.move_player(vx - px, vy - py);
        assert_eq!((s.player.x, s.player.y), (vx, vy));
    }

    #[test]
    fn seal_braziers_open_the_sealed_door_only_when_all_are_lit() {
        let z = find_zone(ZoneId::Dungeon(2), |z| {
            let n = z
                .tiles
                .iter()
                .flatten()
                .filter(|t| t.interact_id.as_deref() == Some("seal_brazier"))
                .count();
            n >= 2 && z.tiles.iter().flatten().any(|t| t.interact_id.as_deref() == Some("seal_door"))
        });
        let braziers = tiles_where(&z, |t| t.interact_id.as_deref() == Some("seal_brazier"));
        let (dx, dy) = tiles_where(&z, |t| t.interact_id.as_deref() == Some("seal_door"))[0];
        let mut s = session();
        load(&mut s, z);
        let total = braziers.len();

        // The seal door explains itself and stays shut.
        let ev = s.interact_with_world_tile(dx, dy);
        assert_eq!(ev, Some(SessionEvent::OpenPrompt("seal_door".into())));
        let body = s.prompt_json()["body"].as_str().unwrap().to_string();
        assert!(body.contains(&format!("0/{total} burn so far")), "{body}");
        s.choose_prompt("leave_seal_door");

        for (i, &(bx, by)) in braziers.iter().enumerate() {
            s.player.hp = 1;
            assert_eq!(s.interact_with_world_tile(bx, by), Some(SessionEvent::None));
            assert_eq!(s.player.hp, s.derived().max_hp, "brazier restores");
            let door = &s.zone().tiles[dy as usize][dx as usize];
            if i + 1 < total {
                assert_eq!(door.kind, TileKind::VaultDoor, "still sealed after {}", i + 1);
                assert!(s.log.last().unwrap().contains(&format!("({}/{total})", i + 1)));
            } else {
                assert_eq!(door.kind, TileKind::Door);
                assert!(door.walkable);
                assert_eq!(door.label.as_deref(), Some("Unsealed Reliquary Door"));
                assert!(s.log.last().unwrap().contains("rune-seal grinds open"));
            }
            // A lit brazier does nothing the second time.
            assert_eq!(s.interact_with_world_tile(bx, by), None);
        }
    }

    #[test]
    fn waystone_attunement_tracks_the_outrider_oath() {
        let mut s = session();
        let mut rng = Rng::new(3);
        let ow = crate::world::generate_zone_by_id(ZoneId::Overworld, &mut rng, DifficultyTier::Standard, 0)
            .unwrap();
        load(&mut s, ow);
        // Overworld waystone at (15,2); approach from (15,3).
        s.player.x = 15;
        s.player.y = 3;
        s.player.hp = 10;
        let ev = s.move_player(0, -1);
        assert_eq!(ev, SessionEvent::OpenPrompt("waystone".into()));
        assert_eq!((s.player.x, s.player.y), (15, 2), "the waystone is stepped onto");

        // Vanguard starts below 3 Arcana and 3 Resolve: surge is disabled.
        let surge = s.prompt_json()["options"][1].clone();
        assert_eq!(surge["id"], "surge_waystone");
        if s.player.attributes.arcana < 3 && s.player.attributes.resolve < 3 {
            assert_eq!(surge["disabled"], true);
            s.choose_prompt("surge_waystone");
            assert!(s.active_prompt.is_some(), "disabled option is a no-op");
        }

        s.choose_prompt("attune_waystone");
        let d = s.derived();
        let expected_hp = d.max_hp.min(10 + 8.max((d.max_hp as f64 * 0.35).round() as i32));
        assert_eq!(s.player.hp, expected_hp);
        assert!(s
            .player
            .statuses
            .iter()
            .any(|st| st.kind == StatusKind::Shielded && st.turns == 4 && st.potency == 4));
        assert_eq!(s.flags.waystones_attuned, Some(1));
        assert!(s.flags.read_lore_ids.contains(&"lore_waystone".to_string()));
        let q = &s.quests["outrider_oath"];
        assert_eq!((q.status, q.stage), (QuestStatus::Active, 2));
        assert_eq!(q.history[0], format!("Attuned an Elven Waystone in {} (1/2).", s.zone().name));

        // Attuned waystones are inert.
        assert_eq!(s.interact_with_world_tile(15, 2), None);

        // A second waystone (a dungeon floor) advances the oath to stage 3,
        // via the surge this time.
        let z = find_zone(ZoneId::Dungeon(1), |z| z.tiles.iter().flatten().any(|t| t.kind == TileKind::Waystone));
        let (wx, wy) = tiles_where(&z, |t| t.kind == TileKind::Waystone)[0];
        load(&mut s, z);
        s.player.attributes.arcana = 3;
        s.player.cooldowns.insert("cleave".into(), 3);
        let gold = s.player.gold;
        assert_eq!(
            s.interact_with_world_tile(wx, wy),
            Some(SessionEvent::OpenPrompt("waystone".into()))
        );
        s.choose_prompt("surge_waystone");
        assert_eq!(s.player.gold, gold + 15);
        assert!(s.player.cooldowns.is_empty());
        assert_eq!(s.player.mana, s.derived().max_mana);
        assert_eq!(s.flags.waystones_attuned, Some(2));
        let q = &s.quests["outrider_oath"];
        assert_eq!(q.stage, 3);
        assert_eq!(q.history.last().unwrap(), "Attuned Elven Waystone on Floor 1 (2/2).");
    }

    #[test]
    fn boss_chest_grants_the_hoard_once_and_is_seed_deterministic() {
        let loot = |seed: u32| {
            let mut s = GameSession::new_game("Garth", StarterClass::Vanguard, seed);
            let mut rng = Rng::new(5);
            let z = crate::world::generate_zone_by_id(ZoneId::Barrow(2), &mut rng, DifficultyTier::Standard, 0)
                .unwrap();
            let (cx, cy) = tiles_where(&z, |t| {
                t.kind == TileKind::Chest && t.interact_id.as_deref() == Some("barrow_2_boss_chest")
            })[0];
            let depth = z.depth;
            load(&mut s, z);
            let gold = s.player.gold;
            let inv = s.player.inventory.len();
            assert_eq!(s.interact_with_world_tile(cx, cy), Some(SessionEvent::None));
            assert_eq!(s.player.gold, gold + 90 + depth * 22 + 10);
            assert_eq!(s.player.inventory.len(), inv + 1);
            let relic = s.player.inventory.last().unwrap().clone();
            assert!(relic.set_id.is_some(), "hoard is a set relic: {}", relic.name);
            assert!(s.log.last().unwrap().starts_with("Side-Dungeon Hoard claimed:"));
            // Looted chests are inert.
            assert_eq!(s.interact_with_world_tile(cx, cy), None);
            assert_eq!(s.player.gold, gold + 90 + depth * 22 + 10);
            relic.name
        };
        assert_eq!(loot(42), loot(42), "same seed, same hoard");
    }

    #[test]
    fn stepping_on_a_chest_loots_it_without_an_enemy_turn() {
        let z = find_zone(ZoneId::Dungeon(3), |z| {
            z.tiles.iter().flatten().any(|t| t.kind == TileKind::Chest && t.interact_id.is_none())
        });
        let (cx, cy) = tiles_where(&z, |t| t.kind == TileKind::Chest && t.interact_id.is_none())[0];
        let (px, py) = walkable_neighbor(&z, cx, cy);
        let mut s = session();
        load(&mut s, z);
        s.zone_mut().enemies.clear();
        s.player.x = px;
        s.player.y = py;
        let gold = s.player.gold;
        s.move_player(cx - px, cy - py);
        assert_eq!((s.player.x, s.player.y), (cx, cy));
        assert!(s.player.gold >= gold + 18 + 3 * 14);
        assert!(s.log.last().unwrap().starts_with("Opened chest:"));
        assert!(s.used_tiles().contains(&[cx, cy]));
        // The front-end gets a chest cue at the chest, once.
        let fx = s.take_fx();
        assert!(fx.iter().any(|f| f.kind == "chest" && (f.x, f.y) == (cx, cy)), "{fx:?}");
        assert!(s.take_fx().is_empty(), "take_fx drains");
    }

    #[test]
    fn bump_attack_emits_a_directed_hit_cue() {
        let z = find_zone(ZoneId::Dungeon(1), |z| !z.enemies.is_empty());
        let mut s = session();
        load(&mut s, z);
        let e = s.zone().enemies[0].clone();
        // Stand west of the enemy (tile validity doesn't matter for a bump).
        s.player.x = e.x - 1;
        s.player.y = e.y;
        s.take_fx();
        s.move_player(1, 0);
        let fx = s.take_fx();
        let hit = fx
            .iter()
            .find(|f| f.kind == "hit" || f.kind == "crit")
            .expect("hit cue");
        assert_eq!((hit.x, hit.y, hit.dx, hit.dy), (e.x, e.y, 1, 0));
    }

        #[test]
    fn consumables_stack_on_pickup() {
        let mut s = session();
        let before = s.player.inventory.len();
        let qty = s
            .player
            .inventory
            .iter()
            .find(|i| i.consumable_kind == Some(ConsumableKind::HealPotion))
            .and_then(|i| i.quantity)
            .unwrap_or(1);
        let p = create_consumable(&mut s.id_gen, ConsumableKind::HealPotion, 1);
        s.add_item_to_player(p);
        assert_eq!(s.player.inventory.len(), before);
        let now = s
            .player
            .inventory
            .iter()
            .find(|i| i.consumable_kind == Some(ConsumableKind::HealPotion))
            .and_then(|i| i.quantity)
            .unwrap();
        assert_eq!(now, qty + 1);
    }
}
