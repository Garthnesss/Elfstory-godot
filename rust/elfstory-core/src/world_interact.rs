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
    BossConfrontation,
}

impl PromptKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PromptKind::SealDoor => "seal_door",
            PromptKind::Vault => "vault",
            PromptKind::Waystone => "waystone",
            PromptKind::BossConfrontation => "boss_confrontation",
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

/// TS `specialPrompt` for the world tiles and boss confrontations.
/// `x`/`y` is the tile the prompt acts on; `target_id` names the boss enemy
/// when `kind == PromptKind::BossConfrontation`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpecialPrompt {
    pub kind: PromptKind,
    pub x: i32,
    pub y: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
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
            TileKind::Shrine if tile.interact_id.as_deref() == Some("respec_shrine") => {
                let d = self.derived();
                self.player.hp = d.max_hp;
                self.player.mana = d.max_mana;
                self.fx("brazier", x, y);
                self.say("Sister Wren's vigil candles warm your hands — HP and MP restored.");
                Some(SessionEvent::None)
            }
            _ if tile.interact_id.as_deref() == Some("poi_old_oak") => {
                self.fx("pickup", x, y);
                self.say("The Old Outpost Oak grips the breach rim with roots thicker than siege timbers.");
                Some(SessionEvent::None)
            }
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
            target_id: None,
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
            target_id: None,
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
            target_id: None,
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

    // ------------------------------------------------------------ boss confrontations

    /// Check newly visible mini-boss wardens and story bosses (Vaelen / Serath /
    /// Malakor). Mini-bosses emit a roar and log once; story bosses open a
    /// pre-fight confrontation prompt once per archetype.
    pub(crate) fn check_boss_confrontations(&mut self) -> Option<SessionEvent> {
        if self.in_town() || self.active_prompt.is_some() {
            return None;
        }
        let z = self.zone();
        let seen = self.flags.confronted_bosses.as_deref().unwrap_or(&[]);

        let mini = z
            .enemies
            .iter()
            .find(|e| {
                e.hp > 0
                    && e.is_boss != Some(true)
                    && e.rank == Some(EnemyRank::Randboss)
                    && z.tiles[e.y as usize][e.x as usize].visible
                    && !seen.iter().any(|s| s == &e.id)
            })
            .map(|e| (e.id.clone(), e.name.clone(), e.x, e.y));
        if let Some((id, name, mx, my)) = mini {
            self.flags
                .confronted_bosses
                .get_or_insert_with(Vec::new)
                .push(id);
            self.fx("boss_roar", mx, my);
            self.say(&format!("Mini-Boss Warden spotted: {name}!"));
        }

        let z = self.zone();
        let seen = self.flags.confronted_bosses.as_deref().unwrap_or(&[]);
        let boss = z
            .enemies
            .iter()
            .find(|e| {
                e.hp > 0
                    && e.is_boss == Some(true)
                    && !e.name.starts_with("Abyssal Echo")
                    && matches!(
                        e.archetype,
                        EnemyArchetype::BossBoneCantor
                            | EnemyArchetype::BossAshenInquisitor
                            | EnemyArchetype::BossMalakor
                    )
                    && z.tiles[e.y as usize][e.x as usize].visible
                    && !seen.iter().any(|s| s == e.archetype.as_str())
            })
            .map(|e| (e.id.clone(), e.archetype, e.x, e.y));
        let (boss_id, archetype, bx, by) = boss?;

        self.flags
            .confronted_bosses
            .get_or_insert_with(Vec::new)
            .push(archetype.as_str().to_string());
        self.fx("boss_roar", bx, by);
        let a = self.player.attributes;
        let prompt = match archetype {
            EnemyArchetype::BossBoneCantor => SpecialPrompt {
                kind: PromptKind::BossConfrontation,
                x: bx,
                y: by,
                target_id: Some(boss_id),
                title: "Vaelen, Bone Cantor".into(),
                subtitle: "Floor 2 Ossuary Confrontation".into(),
                body: "A skeletal choir-master lowers his carved femur baton as your lantern touches the ossuary arch. \"Another hero. Let me guess — you have a nickname.\" He sighs, a remarkable feat without lungs. \"Speak before my dirge begins. It is in four movements. You will not survive the second.\"".into(),
                options: vec![
                    opt(
                        "pacify_vaelen",
                        format!(
                            "[Resolve {}/3 or Arcana {}/3] Chant the Oath of Release and lay Vaelen to peaceful rest (+1 Skill Pt)",
                            a.resolve, a.arcana
                        ),
                        "primary",
                        a.resolve < 3 && a.arcana < 3,
                    ),
                    opt(
                        "sunder_vaelen",
                        format!(
                            "[Might {}/3] Hurl a stone to shatter his resonance staff (-24 Boss HP, -2 DEF)",
                            a.might
                        ),
                        "secondary",
                        a.might < 3,
                    ),
                    opt(
                        "fight_vaelen",
                        "Draw steel and silence the Bone Cantor".into(),
                        "danger",
                        false,
                    ),
                ],
            },
            EnemyArchetype::BossAshenInquisitor => SpecialPrompt {
                kind: PromptKind::BossConfrontation,
                x: bx,
                y: by,
                target_id: Some(boss_id),
                title: "Inquisitor Serath".into(),
                subtitle: "Floor 4 Crucible Confrontation".into(),
                body: "Cinder-wreathed iron plates clang as Inquisitor Serath raises his great-flail. \"None pass into the Sovereign’s sanctum unpurged by flame!\" He squints at you. \"…Are you the one who keeps writing your name on the walls? In letters THAT size?\"".into(),
                options: vec![
                    opt(
                        "stagger_serath",
                        format!(
                            "[Might {}/4] Bull-rush his shield stance (Stuns Serath 1t & deals 26 dmg)",
                            a.might
                        ),
                        "primary",
                        a.might < 4,
                    ),
                    opt(
                        "flank_serath",
                        format!(
                            "[Agility {}/4] Slip into flanking poise (Marks Serath +35% dmg & grants +6 DEF Ward)",
                            a.agility
                        ),
                        "secondary",
                        a.agility < 4,
                    ),
                    opt(
                        "fight_serath",
                        "Engage Inquisitor Serath".into(),
                        "danger",
                        false,
                    ),
                ],
            },
            EnemyArchetype::BossMalakor => SpecialPrompt {
                kind: PromptKind::BossConfrontation,
                x: bx,
                y: by,
                target_id: Some(boss_id),
                title: "Malakor, Sovereign of Cinder".into(),
                subtitle: "Floor 5 Throne Confrontation".into(),
                body: "Beside an ember-lit dais, an armored monarch rises, crown glowing white-hot. \"So. Garth Elfstrom. My spies have sent reports.\" He lifts a scroll that unrolls to the floor. \"Most of them are complaints. Have you come to claim my crown — or to tell me about the bridge again?\"".into(),
                options: vec![
                    opt(
                        "unravel_malakor",
                        format!(
                            "[Arcana {}/5] Unravel his cinder ward (-45 Boss HP, -3 DEF)",
                            a.arcana
                        ),
                        "primary",
                        a.arcana < 5,
                    ),
                    opt(
                        "defy_malakor",
                        format!(
                            "[Resolve {}/5] Defy the Sovereign (Refill full Mana & gain +8 DEF Ward 4t)",
                            a.resolve
                        ),
                        "secondary",
                        a.resolve < 5,
                    ),
                    opt(
                        "fight_malakor",
                        "Cross blades with Malakor".into(),
                        "danger",
                        false,
                    ),
                ],
            },
            _ => return None,
        };
        Some(self.set_prompt(prompt))
    }

    fn resolve_boss_confrontation(&mut self, boss_id: Option<&str>, option_id: &str) -> SessionEvent {
        let Some(id) = boss_id else { return SessionEvent::None };
        let Some(idx) = self.zone().enemies.iter().position(|e| e.id == id) else {
            return SessionEvent::None;
        };
        match option_id {
            "pacify_vaelen" => {
                let boss = self.zone_mut().enemies.remove(idx);
                self.player.skill_points += 1;
                self.player.gold += boss.gold_reward;
                let ev = self.gain_xp(boss.xp_reward);
                let drop = generate_procedural_item(
                    &mut self.id_gen,
                    &mut self.rng,
                    2,
                    ProceduralOptions { slot: None, min_rarity: Some(ItemRarity::Epic) },
                );
                let seq = self.next_seq();
                self.zone_mut().loot.push(GroundLoot {
                    id: format!("drop_{seq}"),
                    x: boss.x,
                    y: boss.y,
                    gold: None,
                    item: Some(drop),
                });
                self.flags.cantor_defeated = Some(true);
                self.flags.cantor_pacified = Some(true);
                self.flags.blight_choice = Some("cantor_appeased".into());
                self.fx("level_up", boss.x, boss.y);
                self.say("You spoke the Oath of Release. Vaelen bows and dissolves into calm dust (+1 Skill Pt)!");
                ev
            }
            "sunder_vaelen" => {
                let (bx, by) = {
                    let e = &mut self.zone_mut().enemies[idx];
                    e.hp = 10.max(e.hp - 24);
                    e.def = 1.max(e.def - 2);
                    (e.x, e.y)
                };
                self.fx("hit", bx, by);
                self.say("Your thrown stone cracks Vaelen’s bone baton before battle begins!");
                SessionEvent::None
            }
            "stagger_serath" => {
                let (bx, by) = {
                    let e = &mut self.zone_mut().enemies[idx];
                    e.hp = 15.max(e.hp - 26);
                    apply_status(&mut e.statuses, StatusKind::Stunned, 1, 1);
                    (e.x, e.y)
                };
                self.fx("crit", bx, by);
                self.say("You bull-rush Serath’s guard, staggering him before the clash!");
                SessionEvent::None
            }
            "flank_serath" => {
                apply_status(&mut self.player.statuses, StatusKind::Shielded, 3, 6);
                let (bx, by) = {
                    let e = &mut self.zone_mut().enemies[idx];
                    apply_status(&mut e.statuses, StatusKind::Marked, 3, 35);
                    (e.x, e.y)
                };
                self.fx("executioners_mark", bx, by);
                self.say("You read Serath’s blind spot—he is Marked and your ward is raised!");
                SessionEvent::None
            }
            "unravel_malakor" => {
                let (bx, by) = {
                    let e = &mut self.zone_mut().enemies[idx];
                    e.hp = 95.max(e.hp - 45);
                    e.def = 3.max(e.def - 3);
                    (e.x, e.y)
                };
                self.fx("ember_lance", bx, by);
                self.say("Your arcane counter-glyph fractures Malakor’s cinder armor!");
                SessionEvent::None
            }
            "defy_malakor" => {
                let d = compute_derived_stats(&self.player, Some(&self.flags), Some(&self.town));
                self.player.mana = d.max_mana;
                apply_status(&mut self.player.statuses, StatusKind::Shielded, 4, 8);
                let (px, py) = (self.player.x, self.player.y);
                self.fx("iron_fortress", px, py);
                self.say("Unbroken resolve fills your spirit (+Full Mana & +8 DEF Ward)!");
                SessionEvent::None
            }
            _ => SessionEvent::None,
        }
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
            (PromptKind::Vault, "open_vault") => {
                self.open_vault(prompt.x, prompt.y);
                SessionEvent::None
            }
            (PromptKind::Waystone, "attune_waystone") => {
                self.attune_waystone(prompt.x, prompt.y, false);
                SessionEvent::None
            }
            (PromptKind::Waystone, "surge_waystone") => {
                self.attune_waystone(prompt.x, prompt.y, true);
                SessionEvent::None
            }
            (PromptKind::BossConfrontation, _) => {
                self.resolve_boss_confrontation(prompt.target_id.as_deref(), option_id)
            }
            _ => SessionEvent::None, // leave / step away / fight
        }
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

    #[test]
    fn boss_confrontation_prompts_trigger_once_and_resolve() {
        // Vaelen on Floor 2: pacify removes boss, grants +1 SP & Epic loot.
        let mut s = session();
        let mut rng = Rng::new(11);
        let mut z2 = crate::world::generate_zone_by_id(ZoneId::Dungeon(2), &mut rng, DifficultyTier::Standard, 0).unwrap();
        z2.enemies.clear();
        for y in 4..12 {
            for x in 4..12 {
                z2.tiles[y][x] = crate::world::make_tile(x as i32, y as i32, TileKind::Floor);
            }
        }
        let mut vaelen = crate::world::create_enemy(EnemyArchetype::BossBoneCantor, 9, 7, 2, DifficultyTier::Standard, 0, 1);
        vaelen.id = "vaelen_1".into();
        vaelen.is_boss = Some(true);
        let mut warden = crate::world::create_enemy(EnemyArchetype::CryptSkeleton, 8, 8, 2, DifficultyTier::Standard, 0, 2);
        warden.id = "warden_1".into();
        warden.rank = Some(EnemyRank::Randboss);
        warden.name = "Dread Warden".into();
        z2.enemies.push(vaelen);
        z2.enemies.push(warden);
        load(&mut s, z2);
        s.player.x = 7;
        s.player.y = 7;
        s.player.attributes.resolve = 3;
        s.player.xp_to_next = 10_000; // avoid level-up +1 SP mixing with Vaelen's +1 SP
        let sp_before = s.player.skill_points;

        let ev = s.wait_turn();
        assert_eq!(ev, SessionEvent::OpenPrompt("boss_confrontation".into()));
        assert_eq!(s.prompt_json()["title"], "Vaelen, Bone Cantor");
        assert!(s.log.iter().any(|l| l.contains("Mini-Boss Warden spotted: Dread Warden!")));

        s.choose_prompt("pacify_vaelen");
        assert!(!s.zone().enemies.iter().any(|e| e.id == "vaelen_1"));
        assert_eq!(s.player.skill_points, sp_before + 1);
        assert_eq!(s.flags.cantor_defeated, Some(true));
        assert_eq!(s.flags.cantor_pacified, Some(true));
        assert_eq!(s.flags.blight_choice.as_deref(), Some("cantor_appeased"));
        assert!(s.zone().loot.iter().any(|l| l.x == 9 && l.y == 7 && l.item.is_some()));

        // Serath on Floor 4: stagger deals 26 dmg and stuns 1t.
        let mut z4 = crate::world::generate_zone_by_id(ZoneId::Dungeon(4), &mut rng, DifficultyTier::Standard, 0).unwrap();
        z4.enemies.clear();
        for y in 4..12 {
            for x in 4..12 {
                z4.tiles[y][x] = crate::world::make_tile(x as i32, y as i32, TileKind::Floor);
            }
        }
        let mut serath = crate::world::create_enemy(EnemyArchetype::BossAshenInquisitor, 9, 7, 4, DifficultyTier::Standard, 0, 3);
        serath.id = "serath_1".into();
        serath.is_boss = Some(true);
        let serath_hp = serath.hp;
        z4.enemies.push(serath);
        load(&mut s, z4);
        s.player.x = 7;
        s.player.y = 7;
        s.player.attributes.might = 4;

        let ev4 = s.wait_turn();
        assert_eq!(ev4, SessionEvent::OpenPrompt("boss_confrontation".into()));
        assert_eq!(s.prompt_json()["title"], "Inquisitor Serath");
        s.choose_prompt("stagger_serath");
        let after_serath = s.zone().enemies.iter().find(|e| e.id == "serath_1").unwrap();
        assert_eq!(after_serath.hp, 15.max(serath_hp - 26));
        assert!(after_serath.statuses.iter().any(|st| st.kind == StatusKind::Stunned));

        // Malakor on Floor 5: unravel drops HP by 45 and DEF by 3.
        let mut z5 = crate::world::generate_zone_by_id(ZoneId::Dungeon(5), &mut rng, DifficultyTier::Standard, 0).unwrap();
        z5.enemies.clear();
        for y in 4..12 {
            for x in 4..12 {
                z5.tiles[y][x] = crate::world::make_tile(x as i32, y as i32, TileKind::Floor);
            }
        }
        let mut malakor = crate::world::create_enemy(EnemyArchetype::BossMalakor, 9, 7, 5, DifficultyTier::Standard, 0, 4);
        malakor.id = "malakor_1".into();
        malakor.is_boss = Some(true);
        let (m_hp, m_def) = (malakor.hp, malakor.def);
        z5.enemies.push(malakor);
        load(&mut s, z5);
        s.player.x = 7;
        s.player.y = 7;
        s.player.attributes.arcana = 5;

        let ev5 = s.wait_turn();
        assert_eq!(ev5, SessionEvent::OpenPrompt("boss_confrontation".into()));
        assert_eq!(s.prompt_json()["title"], "Malakor, Sovereign of Cinder");
        s.choose_prompt("unravel_malakor");
        let after_malakor = s.zone().enemies.iter().find(|e| e.id == "malakor_1").unwrap();
        assert_eq!(after_malakor.hp, 95.max(m_hp - 45));
        assert_eq!(after_malakor.def, 3.max(m_def - 3));
    }
}
