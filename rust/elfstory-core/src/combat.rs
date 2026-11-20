//! Combat helpers, ported from `elfstoryreborn/src/game/combat.ts`.
//!
//! Rust divergences from TS (deliberate, no behaviour change):
//! - TS draws from the global `Math.random` / seeded stream in
//!   `choose_enemy_intent` and `calculate_player_hit`; here every random
//!   draw comes from an explicitly threaded `&mut Rng`.
//! - TS `calculatePlayerHit` calls `skills.computeDerivedStats` internally.
//!   The skills port lands in parallel, so this port takes the caller's
//!   precomputed `DerivedStats` instead of importing `crate::skills`.
//!   Everything else (bloodlust crit bonus, affix multipliers, marked
//!   bonus, crit multiplier, DEF mitigation, lifesteal) is ported 1:1.
//! - TS pathfinders are breadth-first searches with a diagonal
//!   corner-cut guard (despite the "A*" shorthand often used for them);
//!   they are ported as BFS, exactly as written, corner guard included.
//! - TS `computeVisibility` is a 72-ray cast, not recursive shadowcasting;
//!   ported faithfully as written.

use crate::rng::Rng;
use crate::types::{
    DerivedStats, Enemy, EnemyArchetype, EnemyIntent, IntentType, ItemAffixId, PlayerState, Pos,
    SkillId, StatusEffect, StatusKind, TileKind, ZoneMap,
};
use std::collections::{HashMap, HashSet, VecDeque};

const DIRS8: [(i32, i32); 8] = [
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, 0),
    (1, -1),
    (1, 1),
    (-1, 1),
    (-1, -1),
];

fn key(x: i32, y: i32) -> (i32, i32) {
    (x, y)
}

fn tile_at<'a>(zone: &'a ZoneMap, x: i32, y: i32) -> Option<&'a crate::types::Tile> {
    if x < 0 || y < 0 || x >= zone.width || y >= zone.height {
        return None;
    }
    zone.tiles.get(y as usize)?.get(x as usize)
}

/// Port of `computeVisibility`. Towns are fully visible/explored; dungeons
/// ray-cast 72 directions out to `radius` (default 8 in TS).
pub fn compute_visibility(zone: &mut ZoneMap, px: i32, py: i32, radius: i32) {
    if zone.is_town {
        for row in zone.tiles.iter_mut() {
            for tile in row.iter_mut() {
                tile.visible = true;
                tile.explored = true;
            }
        }
        return;
    }
    for row in zone.tiles.iter_mut() {
        for tile in row.iter_mut() {
            tile.visible = false;
        }
    }
    let steps = 72;
    for i in 0..steps {
        let angle = (i as f64 / steps as f64) * std::f64::consts::PI * 2.0;
        let dx = angle.cos();
        let dy = angle.sin();
        let mut rx = px as f64 + 0.5;
        let mut ry = py as f64 + 0.5;
        let mut d = 0.0f64;
        while d <= radius as f64 {
            let tx = rx.floor() as i32;
            let ty = ry.floor() as i32;
            if tx < 0 || ty < 0 || tx >= zone.width || ty >= zone.height {
                break;
            }
            let (blocks, is_self) = {
                let tile = &zone.tiles[ty as usize][tx as usize];
                (tile.blocks_sight, tx == px && ty == py)
            };
            zone.tiles[ty as usize][tx as usize].visible = true;
            zone.tiles[ty as usize][tx as usize].explored = true;
            if blocks && !is_self {
                break;
            }
            rx += dx * 0.5;
            ry += dy * 0.5;
            d += 0.5;
        }
    }
}

pub fn manhattan(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    (ax - bx).abs() + (ay - by).abs()
}

pub fn chebyshev(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    (ax - bx).abs().max((ay - by).abs())
}

/// Port of `hasLineOfSight`: samples the rounded points strictly between
/// the endpoints; any sight-blocking tile breaks the line.
pub fn has_line_of_sight(zone: &ZoneMap, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    let dist = (x1 - x0).abs().max((y1 - y0).abs());
    if dist == 0 {
        return true;
    }
    for i in 1..dist {
        let t = i as f64 / dist as f64;
        let cx = (x0 as f64 + (x1 - x0) as f64 * t).round() as i32;
        let cy = (y0 as f64 + (y1 - y0) as f64 * t).round() as i32;
        if let Some(tile) = tile_at(zone, cx, cy) {
            if tile.blocks_sight {
                return false;
            }
        }
    }
    true
}

/// Port of `applyStatus`: refresh keeps the max turns and max potency.
pub fn apply_status(statuses: &mut Vec<StatusEffect>, kind: StatusKind, turns: i32, potency: i32) {
    if let Some(existing) = statuses.iter_mut().find(|s| s.kind == kind) {
        existing.turns = existing.turns.max(turns);
        existing.potency = existing.potency.max(potency);
    } else {
        statuses.push(StatusEffect { kind, turns, potency });
    }
}

pub fn has_affix(player: &PlayerState, affix: ItemAffixId) -> bool {
    player.equipped.values().any(|item| item.affix == Some(affix))
}

/// Display names for the skills enemy intents reference (mirror of the
/// `SKILLS[id].name` lookups in combat.ts; full table lives in skills).
pub fn skill_name(id: SkillId) -> &'static str {
    match id {
        SkillId::Cleave => "Sundering Cleave",
        SkillId::ShieldBash => "Bastion Bash",
        SkillId::Bloodlust => "Crimson Vigor",
        SkillId::IronFortress => "Iron Fortress",
        SkillId::Shadowstep => "Shadowstep",
        SkillId::VenomTip => "Viper Edge",
        SkillId::EvasiveInstinct => "Phantom Reflex",
        SkillId::ExecutionersMark => "Executioner's Mark",
        SkillId::EmberLance => "Ember Lance",
        SkillId::FrostNova => "Rime Nova",
        SkillId::SiphonSoul => "Soul Siphon",
        SkillId::ArcaneEcho => "Arcane Echo",
    }
}

fn roll(rng: &mut Rng) -> f64 {
    rng.next_f32() as f64
}

/// Port of `chooseEnemyIntent`. Stunned enemies telegraph nothing.
/// Chilled enemies telegraph at 65% damage. All TS `Math.random()` gates
/// draw from `rng` in the same order TS evaluates them.
pub fn choose_enemy_intent(
    enemy: &Enemy,
    player: &PlayerState,
    zone: &ZoneMap,
    rng: &mut Rng,
) -> Option<EnemyIntent> {
    if enemy.statuses.iter().any(|s| s.kind == StatusKind::Stunned) {
        return None;
    }
    let dist = chebyshev(enemy.x, enemy.y, player.x, player.y);
    let chilled = enemy.statuses.iter().any(|s| s.kind == StatusKind::Chilled);
    let dmg_scale = if chilled { 0.65 } else { 1.0 };
    let scaled = |mult: f64| (enemy.atk as f64 * mult * dmg_scale).round() as i32;

    // Rare / Randboss enemies roll real player skills off cooldown.
    if let Some(loadout) = &enemy.skill_loadout {
        if !loadout.is_empty() && roll(rng) < 0.65 {
            let ready: Vec<SkillId> = loadout
                .iter()
                .copied()
                .filter(|id| {
                    enemy
                        .skill_cooldowns
                        .as_ref()
                        .and_then(|cds| cds.get(id.as_str()))
                        .copied()
                        .unwrap_or(0)
                        == 0
                })
                .collect();
            for skill_id in ready {
                let name = skill_name(skill_id);
                match skill_id {
                    SkillId::EmberLance
                        if (2..=5).contains(&dist)
                            && has_line_of_sight(zone, enemy.x, enemy.y, player.x, player.y) =>
                    {
                        return Some(EnemyIntent {
                            kind: IntentType::EnemySkill,
                            skill_id: Some(skill_id),
                            label: format!("Charging {name}"),
                            damage: scaled(1.45),
                            target_tiles: vec![Pos::new(player.x, player.y)],
                        });
                    }
                    SkillId::VenomTip
                        if (2..=4).contains(&dist)
                            && has_line_of_sight(zone, enemy.x, enemy.y, player.x, player.y) =>
                    {
                        return Some(EnemyIntent {
                            kind: IntentType::EnemySkill,
                            skill_id: Some(skill_id),
                            label: format!("Aiming {name}"),
                            damage: scaled(1.25),
                            target_tiles: vec![Pos::new(player.x, player.y)],
                        });
                    }
                    SkillId::FrostNova if dist <= 2 => {
                        let mut area = Vec::new();
                        for dy in -2..=2 {
                            for dx in -2..=2 {
                                if dx == 0 && dy == 0 {
                                    continue;
                                }
                                let (tx, ty) = (enemy.x + dx, enemy.y + dy);
                                if tile_at(zone, tx, ty).map_or(false, |t| t.walkable) {
                                    area.push(Pos::new(tx, ty));
                                }
                            }
                        }
                        return Some(EnemyIntent {
                            kind: IntentType::EnemySkill,
                            skill_id: Some(skill_id),
                            label: format!("Gathering {name}"),
                            damage: scaled(1.35),
                            target_tiles: area,
                        });
                    }
                    SkillId::ShieldBash | SkillId::Cleave if dist == 1 => {
                        let mult = if skill_id == SkillId::Cleave { 1.55 } else { 1.3 };
                        return Some(EnemyIntent {
                            kind: IntentType::EnemySkill,
                            skill_id: Some(skill_id),
                            label: format!("Telegraphing {name}"),
                            damage: scaled(mult),
                            target_tiles: vec![Pos::new(player.x, player.y)],
                        });
                    }
                    SkillId::Shadowstep
                        if (2..=4).contains(&dist)
                            && has_line_of_sight(zone, enemy.x, enemy.y, player.x, player.y) =>
                    {
                        return Some(EnemyIntent {
                            kind: IntentType::EnemySkill,
                            skill_id: Some(skill_id),
                            label: format!("Priming {name} Ambush"),
                            damage: scaled(1.4),
                            target_tiles: vec![Pos::new(player.x, player.y)],
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    // Boss / heavy archetypes telegraph impactful windups.
    if matches!(
        enemy.archetype,
        EnemyArchetype::IronGolem
            | EnemyArchetype::ObsidianWarden
            | EnemyArchetype::SporeHulk
            | EnemyArchetype::BossAshenInquisitor
            | EnemyArchetype::BossMalakor
    ) && dist <= 2
        && roll(rng) < 0.45
    {
        let mut targets = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (tx, ty) = (enemy.x + dx, enemy.y + dy);
                if tile_at(zone, tx, ty).map_or(false, |t| t.walkable) {
                    targets.push(Pos::new(tx, ty));
                }
            }
        }
        let label = match enemy.archetype {
            EnemyArchetype::ObsidianWarden => "Winding up Obsidian Cleave",
            EnemyArchetype::SporeHulk => "Swelling Spore Slam",
            _ => "Winding up Ground Slam",
        };
        return Some(EnemyIntent {
            kind: IntentType::GroundSlam,
            skill_id: None,
            label: label.to_string(),
            damage: scaled(1.55),
            target_tiles: targets,
        });
    }

    if matches!(
        enemy.archetype,
        EnemyArchetype::BoneAcolyte
            | EnemyArchetype::AshCultist
            | EnemyArchetype::BlightWeaver
            | EnemyArchetype::RimeRevenant
            | EnemyArchetype::BossBoneCantor
            | EnemyArchetype::BossMalakor
    ) && (2..=5).contains(&dist)
        && has_line_of_sight(zone, enemy.x, enemy.y, player.x, player.y)
        && roll(rng) < 0.65
    {
        let label = match enemy.archetype {
            EnemyArchetype::BlightWeaver => "Weaving Venom Hex",
            EnemyArchetype::RimeRevenant => "Charging Rime Bolt",
            _ => "Charging Hex Bolt",
        };
        return Some(EnemyIntent {
            kind: IntentType::HexBolt,
            skill_id: None,
            label: label.to_string(),
            damage: scaled(1.25),
            target_tiles: vec![Pos::new(player.x, player.y)],
        });
    }

    if matches!(
        enemy.archetype,
        EnemyArchetype::GoblinSkirmisher
            | EnemyArchetype::CryptSkeleton
            | EnemyArchetype::ShadowStalker
            | EnemyArchetype::GraveHusk
            | EnemyArchetype::DreadWraith
    ) && dist == 1
        && roll(rng) < 0.38
    {
        let label = match enemy.archetype {
            EnemyArchetype::DreadWraith => "Channeling Soul Chill",
            EnemyArchetype::GraveHusk => "Raising a Tomb-Cleaver Blow",
            _ => "Telegraphing Heavy Strike",
        };
        return Some(EnemyIntent {
            kind: IntentType::HeavyWindup,
            skill_id: None,
            label: label.to_string(),
            damage: scaled(1.45),
            target_tiles: vec![Pos::new(player.x, player.y)],
        });
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerHitResult {
    pub damage: i32,
    pub is_crit: bool,
    pub lifesteal_heal: i32,
}

/// Port of `calculatePlayerHit`. `stats` is the caller's
/// `computeDerivedStats` result (see module docs). Crit chance is a
/// percentage, as in TS.
#[allow(clippy::too_many_arguments)]
pub fn calculate_player_hit(
    player: &PlayerState,
    target: &Enemy,
    multiplier: f64,
    is_spell: bool,
    force_crit: bool,
    ignore_def_ratio: f64,
    stats: &DerivedStats,
    rng: &mut Rng,
) -> PlayerHitResult {
    let raw_power = if is_spell { stats.spell_power } else { stats.atk };
    let mut crit_chance = stats.crit_chance;
    let bloodlust = player.skills.get("bloodlust").copied().unwrap_or(0);
    // TS: target.hp < target.maxHp * 0.5 — kept in float to match
    // odd maxHp behaviour exactly.
    if bloodlust > 0 && (target.hp as f64) < target.max_hp as f64 * 0.5 {
        crit_chance += match bloodlust {
            1 => 10.0,
            2 => 18.0,
            _ => 25.0,
        };
    }

    let is_crit = force_crit || roll(rng) * 100.0 < crit_chance;
    let mut dmg = raw_power as f64 * multiplier;

    if player.first_strike_ready && has_affix(player, ItemAffixId::FirstStrike) {
        dmg *= 1.35;
    }
    if (target.hp as f64) < target.max_hp as f64 * 0.4
        && has_affix(player, ItemAffixId::ExecuteBonus)
    {
        dmg *= 1.25;
    }
    if let Some(marked) = target.statuses.iter().find(|s| s.kind == StatusKind::Marked) {
        dmg *= 1.0 + marked.potency as f64 / 100.0;
    }
    if is_crit {
        dmg *= 1.65;
    }

    let effective_def = (target.def as f64 * (1.0 - ignore_def_ratio)).round();
    let final_damage = (dmg - effective_def * 0.6).round().max(2.0) as i32;

    let mut lifesteal_heal = 0;
    if !is_spell && stats.lifesteal > 0.0 {
        lifesteal_heal += ((final_damage as f64 * stats.lifesteal / 100.0).round() as i32).max(1);
    }
    if is_crit && has_affix(player, ItemAffixId::VampiricTouch) {
        lifesteal_heal += 3;
    }

    PlayerHitResult { damage: final_damage, is_crit, lifesteal_heal }
}

/// Diagonal moves may not cut corners: both orthogonal neighbours of a
/// diagonal step must be walkable (and, for explored-only searches,
/// explored). Shared by all three searches below.
fn diagonal_ok(zone: &ZoneMap, cx: i32, cy: i32, nx: i32, ny: i32, explored_only: bool) -> bool {
    let c1 = tile_at(zone, nx, cy);
    let c2 = tile_at(zone, cx, ny);
    match (c1, c2) {
        (Some(a), Some(b)) => {
            a.walkable && b.walkable && (!explored_only || (a.explored && b.explored))
        }
        _ => false,
    }
}

/// Port of `findPathNextStep`: BFS first step from (sx, sy) to (tx, ty),
/// or None when already there / unreachable. Enemies and NPCs block
/// every tile except the destination itself.
pub fn find_path_next_step(zone: &ZoneMap, sx: i32, sy: i32, tx: i32, ty: i32) -> Option<Pos> {
    if sx == tx && sy == ty {
        return None;
    }
    let mut visited: HashSet<(i32, i32)> = HashSet::new();
    let mut queue: VecDeque<(i32, i32, Option<Pos>)> = VecDeque::new();
    visited.insert(key(sx, sy));
    queue.push_back((sx, sy, None));

    while let Some((cx, cy, first_step)) = queue.pop_front() {
        if cx == tx && cy == ty {
            return first_step;
        }
        for (dx, dy) in DIRS8 {
            let (nx, ny) = (cx + dx, cy + dy);
            if visited.contains(&key(nx, ny)) {
                continue;
            }
            let tile = match tile_at(zone, nx, ny) {
                Some(t) => t,
                None => continue,
            };
            if !tile.walkable {
                continue;
            }
            if dx != 0 && dy != 0 && !diagonal_ok(zone, cx, cy, nx, ny, false) {
                continue;
            }
            let is_dest = nx == tx && ny == ty;
            if !is_dest
                && (zone.enemies.iter().any(|e| e.x == nx && e.y == ny)
                    || zone.npcs.iter().any(|n| n.x == nx && n.y == ny))
            {
                continue;
            }
            visited.insert(key(nx, ny));
            queue.push_back((nx, ny, first_step.or(Some(Pos::new(nx, ny)))));
        }
    }
    None
}

/// Port of `findPathPoints`: full BFS path (excluding the start,
/// including the destination), at most `max_steps` long. Explored-only
/// by default in TS; stairs are never routed *through*.
pub fn find_path_points(
    zone: &ZoneMap,
    sx: i32,
    sy: i32,
    tx: i32,
    ty: i32,
    explored_only: bool,
    max_steps: usize,
) -> Vec<Pos> {
    if sx == tx && sy == ty {
        return Vec::new();
    }
    let dest = match tile_at(zone, tx, ty) {
        Some(t) => t,
        None => return Vec::new(),
    };
    if !dest.walkable || (explored_only && !dest.explored) {
        return Vec::new();
    }
    let mut visited: HashSet<(i32, i32)> = HashSet::new();
    let mut parent: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut queue: VecDeque<(i32, i32, usize)> = VecDeque::new();
    visited.insert(key(sx, sy));
    queue.push_back((sx, sy, 0));
    let mut found = false;

    while let Some((cx, cy, dist)) = queue.pop_front() {
        if cx == tx && cy == ty {
            found = true;
            break;
        }
        if dist >= max_steps {
            continue;
        }
        for (dx, dy) in DIRS8 {
            let (nx, ny) = (cx + dx, cy + dy);
            if visited.contains(&key(nx, ny)) {
                continue;
            }
            let tile = match tile_at(zone, nx, ny) {
                Some(t) => t,
                None => continue,
            };
            if !tile.walkable || (explored_only && !tile.explored) {
                continue;
            }
            if dx != 0 && dy != 0 && !diagonal_ok(zone, cx, cy, nx, ny, explored_only) {
                continue;
            }
            let is_dest = nx == tx && ny == ty;
            // TS: enemies block only when their tile is visible here.
            if !is_dest
                && ((tile.visible && zone.enemies.iter().any(|e| e.x == nx && e.y == ny))
                    || zone.npcs.iter().any(|n| n.x == nx && n.y == ny))
            {
                continue;
            }
            if matches!(tile.kind, TileKind::StairsUp | TileKind::StairsDown) && !is_dest {
                continue;
            }
            visited.insert(key(nx, ny));
            parent.insert(key(nx, ny), key(cx, cy));
            queue.push_back((nx, ny, dist + 1));
        }
    }

    if !found {
        return Vec::new();
    }
    let mut path = Vec::new();
    let mut cur = key(tx, ty);
    while cur != key(sx, sy) {
        path.push(Pos::new(cur.0, cur.1));
        match parent.get(&cur) {
            Some(p) => cur = *p,
            None => break,
        }
    }
    path.reverse();
    path
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoExploreReason {
    Loot,
    Chest,
    Frontier,
    Stairs,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoExploreResult {
    pub next_step: Option<Pos>,
    pub reason: AutoExploreReason,
}

fn auto_bfs_search(
    zone: &ZoneMap,
    px: i32,
    py: i32,
    predicate: &dyn Fn(&ZoneMap, i32, i32) -> bool,
) -> Option<Pos> {
    let mut visited: HashSet<(i32, i32)> = HashSet::new();
    let mut queue: VecDeque<(i32, i32, Option<Pos>)> = VecDeque::new();
    visited.insert(key(px, py));
    queue.push_back((px, py, None));

    while let Some((cx, cy, first_step)) = queue.pop_front() {
        if (cx != px || cy != py) && predicate(zone, cx, cy) {
            return first_step;
        }
        for (dx, dy) in DIRS8 {
            let (nx, ny) = (cx + dx, cy + dy);
            if visited.contains(&key(nx, ny)) {
                continue;
            }
            let tile = match tile_at(zone, nx, ny) {
                Some(t) => t,
                None => continue,
            };
            if !tile.explored || !tile.walkable {
                continue;
            }
            if dx != 0 && dy != 0 && !diagonal_ok(zone, cx, cy, nx, ny, true) {
                continue;
            }
            if matches!(tile.kind, TileKind::Trap | TileKind::DartTrap)
                && tile.revealed == Some(true)
                && tile.used != Some(true)
            {
                continue;
            }
            if matches!(
                tile.kind,
                TileKind::SporePool | TileKind::FrostVent | TileKind::EmberCrust | TileKind::ShockRune
            ) && tile.used != Some(true)
            {
                continue;
            }
            if zone.npcs.iter().any(|n| n.x == nx && n.y == ny) {
                continue;
            }
            // Stairs auto-trigger a zone change: never a waypoint.
            // stairs_down may be entered only when it is the BFS target.
            if tile.kind == TileKind::StairsUp {
                continue;
            }
            if tile.kind == TileKind::StairsDown && !predicate(zone, nx, ny) {
                continue;
            }
            visited.insert(key(nx, ny));
            queue.push_back((nx, ny, first_step.or(Some(Pos::new(nx, ny)))));
        }
    }
    None
}

/// Port of `findAutoExploreTarget`: loot → unopened chest → frontier →
/// stop *beside* the descent stairs (never step on them) → done.
pub fn find_auto_explore_target(zone: &ZoneMap, px: i32, py: i32) -> AutoExploreResult {
    if let Some(step) = auto_bfs_search(zone, px, py, &|z, x, y| {
        z.loot.iter().any(|l| l.x == x && l.y == y)
    }) {
        return AutoExploreResult { next_step: Some(step), reason: AutoExploreReason::Loot };
    }
    if let Some(step) = auto_bfs_search(zone, px, py, &|z, x, y| {
        tile_at(z, x, y).map_or(false, |t| t.kind == TileKind::Chest && t.used != Some(true))
    }) {
        return AutoExploreResult { next_step: Some(step), reason: AutoExploreReason::Chest };
    }
    let is_frontier = |z: &ZoneMap, x: i32, y: i32| {
        DIRS8.iter().any(|(dx, dy)| {
            tile_at(z, x + dx, y + dy).map_or(false, |t| !t.explored)
        })
    };
    if let Some(step) = auto_bfs_search(zone, px, py, &is_frontier) {
        return AutoExploreResult { next_step: Some(step), reason: AutoExploreReason::Frontier };
    }
    if !zone.is_town
        && tile_at(zone, px, py).map_or(false, |t| t.kind != TileKind::StairsDown)
    {
        if let Some(step) = auto_bfs_search(zone, px, py, &|z, x, y| {
            tile_at(z, x, y).map_or(false, |t| t.kind == TileKind::StairsDown)
        }) {
            if tile_at(zone, step.x, step.y).map_or(false, |t| t.kind == TileKind::StairsDown) {
                return AutoExploreResult { next_step: None, reason: AutoExploreReason::Stairs };
            }
            return AutoExploreResult { next_step: Some(step), reason: AutoExploreReason::Stairs };
        }
    }
    AutoExploreResult { next_step: None, reason: AutoExploreReason::Done }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Attributes, EnemyArchetype, StarterClass, Tile, TileKind as TK, ZoneId,
    };
    use std::collections::HashMap;

    fn tile(x: i32, y: i32, kind: TK) -> Tile {
        let walkable = !matches!(kind, TK::Wall | TK::Tree | TK::Pillar);
        Tile {
            x,
            y,
            kind,
            glyph: "?".into(),
            color: "#000".into(),
            bg: None,
            walkable,
            blocks_sight: matches!(kind, TK::Wall | TK::Tree | TK::Pillar),
            explored: true,
            visible: true,
            interact_id: None,
            used: None,
            revealed: None,
            label: None,
        }
    }

    fn zone(width: i32, height: i32, fill: TK) -> ZoneMap {
        let tiles = (0..height)
            .map(|y| (0..width).map(|x| tile(x, y, fill)).collect())
            .collect();
        ZoneMap {
            id: ZoneId::Dungeon(1),
            name: "Test".into(),
            subtitle: String::new(),
            is_town: false,
            depth: 1,
            width,
            height,
            tiles,
            enemies: vec![],
            npcs: vec![],
            critters: None,
            loot: vec![],
            spawn_point: Pos::new(0, 0),
            return_point: Pos::new(0, 0),
        }
    }

    fn player(x: i32, y: i32) -> PlayerState {
        PlayerState {
            name: "Garth".into(),
            starter_class: StarterClass::Vanguard,
            x,
            y,
            hp: 50,
            max_hp: 50,
            mana: 10,
            max_mana: 10,
            level: 1,
            xp: 0,
            xp_to_next: 65,
            gold: 0,
            attribute_points: 0,
            skill_points: 0,
            attributes: Attributes::default(),
            skills: HashMap::new(),
            prodigies: None,
            prodigy_cooldowns: None,
            hotbar: vec![None; 5],
            cooldowns: HashMap::new(),
            haste_steps: None,
            bonus_hp: None,
            statuses: vec![],
            equipped: HashMap::new(),
            inventory: vec![],
            kills: 0,
            turns_elapsed: 0,
            first_strike_ready: false,
            last_dir_x: None,
            companion: None,
        }
    }

    fn enemy(archetype: EnemyArchetype, x: i32, y: i32, atk: i32) -> Enemy {
        Enemy {
            id: "e".into(),
            archetype,
            name: "Foe".into(),
            glyph: "f".into(),
            color: "#fff".into(),
            x,
            y,
            hp: 50,
            max_hp: 50,
            atk,
            def: 0,
            xp_reward: 1,
            gold_reward: 0,
            is_boss: None,
            is_elite: None,
            rank: None,
            rank_title: None,
            skill_loadout: None,
            skill_cooldowns: None,
            rune_charge: None,
            undying_used: None,
            statuses: vec![],
            intent: None,
            alerted: false,
            pacified: None,
            description: String::new(),
        }
    }

    fn stats(atk: i32) -> DerivedStats {
        DerivedStats {
            max_hp: 50,
            max_mana: 10,
            atk,
            def: 0,
            spell_power: 0,
            crit_chance: 0.0,
            dodge_chance: 0.0,
            lifesteal: 0.0,
            thorns: 0,
            build_title: "Test".into(),
        }
    }

    #[test]
    fn los_blocked_by_wall_and_clear_on_open_floor() {
        let mut z = zone(5, 1, TK::Floor);
        assert!(has_line_of_sight(&z, 0, 0, 4, 0));
        z.tiles[0][2] = tile(2, 0, TK::Wall);
        assert!(!has_line_of_sight(&z, 0, 0, 4, 0));
        assert!(has_line_of_sight(&z, 0, 0, 0, 0));
    }

    #[test]
    fn visibility_town_sees_all_dungeon_wall_blocks() {
        let mut town = zone(3, 3, TK::Floor);
        town.is_town = true;
        for row in town.tiles.iter_mut() {
            for t in row.iter_mut() {
                t.visible = false;
                t.explored = false;
            }
        }
        compute_visibility(&mut town, 1, 1, 8);
        assert!(town.tiles.iter().flatten().all(|t| t.visible && t.explored));

        let mut dung = zone(5, 1, TK::Floor);
        dung.tiles[0][2] = tile(2, 0, TK::Wall);
        compute_visibility(&mut dung, 0, 0, 8);
        assert!(dung.tiles[0][1].visible);
        assert!(dung.tiles[0][2].visible, "the wall itself is seen");
        assert!(!dung.tiles[0][4].visible, "tiles behind the wall stay hidden");
    }

    #[test]
    fn path_never_cuts_wall_corners() {
        // 2x2: start (0,0), wall at (1,0), target (1,1). The diagonal
        // (0,0)->(1,1) would cut the wall's corner and must be refused;
        // the orthogonal route via (0,1) is the only path.
        let mut z = zone(2, 2, TK::Floor);
        z.tiles[0][1] = tile(1, 0, TK::Wall);
        let step = find_path_next_step(&z, 0, 0, 1, 1);
        assert_eq!(step, Some(Pos::new(0, 1)));
        let path = find_path_points(&z, 0, 0, 1, 1, true, 16);
        assert_eq!(path, vec![Pos::new(0, 1), Pos::new(1, 1)]);
        // Fully sealed diagonal: no path at all.
        z.tiles[1][0] = tile(0, 1, TK::Wall);
        assert_eq!(find_path_next_step(&z, 0, 0, 1, 1), None);
    }

    #[test]
    fn auto_explore_stops_beside_stairs() {
        // Mirrors ts/tests/runRules.test.ts: 5x3 corridor with stairs_down.
        let mut z = zone(5, 3, TK::Wall);
        for x in 1..=3usize {
            z.tiles[1][x] = tile(x as i32, 1, TK::Floor);
        }
        z.tiles[1][3] = tile(3, 1, TK::StairsDown);
        let far = find_auto_explore_target(&z, 1, 1);
        assert_eq!(
            far,
            AutoExploreResult { next_step: Some(Pos::new(2, 1)), reason: AutoExploreReason::Stairs }
        );
        let beside = find_auto_explore_target(&z, 2, 1);
        assert_eq!(
            beside,
            AutoExploreResult { next_step: None, reason: AutoExploreReason::Stairs }
        );
    }

    #[test]
    fn hit_math_forced_crit_and_min_damage() {
        let p = player(0, 0);
        let mut t = enemy(EnemyArchetype::CryptSkeleton, 1, 0, 5);
        t.def = 0;
        let mut rng = Rng::new(7);
        // atk 20, no crit bonus, forced crit: 20 * 1.65 = 33.
        let hit = calculate_player_hit(&p, &t, 1.0, false, true, 0.0, &stats(20), &mut rng);
        assert!(hit.is_crit);
        assert_eq!(hit.damage, 33);
        // DEF soaks: def 100 -> max(2, round(20 - 60)) = 2 (minimum damage).
        t.def = 100;
        let hit2 = calculate_player_hit(&p, &t, 1.0, false, false, 0.0, &stats(20), &mut rng);
        assert_eq!(hit2.damage, 2);
    }

    #[test]
    fn stunned_enemy_telegraphs_nothing_and_skill_intent_scales() {
        let z = zone(9, 9, TK::Floor);
        let p = player(4, 4);
        let mut stunned = enemy(EnemyArchetype::IronGolem, 4, 3, 10);
        stunned.statuses.push(StatusEffect { kind: StatusKind::Stunned, turns: 1, potency: 0 });
        let mut rng = Rng::new(1);
        assert_eq!(choose_enemy_intent(&stunned, &p, &z, &mut rng), None);

        // Ember Lance loadout at range 3: over many seeds the telegraphed
        // skill intent must appear with damage round(10 * 1.45) = 15.
        let mut caster = enemy(EnemyArchetype::BoneAcolyte, 4, 1, 10);
        caster.skill_loadout = Some(vec![SkillId::EmberLance]);
        caster.skill_cooldowns = Some(HashMap::new());
        let mut seen = false;
        for seed in 0..200u32 {
            let mut r = Rng::new(seed);
            if let Some(intent) = choose_enemy_intent(&caster, &p, &z, &mut r) {
                assert_eq!(intent.kind, IntentType::EnemySkill);
                assert_eq!(intent.damage, 15);
                seen = true;
                break;
            }
        }
        assert!(seen, "ember lance intent should trigger for some seed");
    }

    #[test]
    fn apply_status_keeps_max_turns_and_potency() {
        let mut s = vec![];
        apply_status(&mut s, StatusKind::Poison, 3, 5);
        apply_status(&mut s, StatusKind::Poison, 2, 9);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].turns, s[0].potency), (3, 9));
    }
}
