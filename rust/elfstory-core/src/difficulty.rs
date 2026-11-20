//! Difficulty tiers + NG+ ascension cycles, ported from
//! `elfstoryreborn/src/game/difficulty.ts`.
//!
//! Rust divergence from TS: the active tier/cycle are NOT module globals.
//! Callers pass `DifficultyTier` and cycle explicitly to the scaling
//! funnels, which keeps `elfstory-core` deterministic and testable.
//! At Standard + cycle 0 every multiplier is exactly 1.0.

use crate::types::Enemy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DifficultyTier {
    Story,
    Standard,
    Hardened,
}

pub const DEFAULT_DIFFICULTY: DifficultyTier = DifficultyTier::Standard;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DifficultyMultipliers {
    pub enemy_hp: f64,
    pub enemy_atk: f64,
    pub xp: f64,
}

pub fn difficulty_multipliers(tier: DifficultyTier) -> DifficultyMultipliers {
    match tier {
        DifficultyTier::Story => DifficultyMultipliers { enemy_hp: 0.75, enemy_atk: 0.85, xp: 1.15 },
        DifficultyTier::Standard => DifficultyMultipliers { enemy_hp: 1.0, enemy_atk: 1.0, xp: 1.0 },
        DifficultyTier::Hardened => DifficultyMultipliers { enemy_hp: 1.25, enemy_atk: 1.15, xp: 1.1 },
    }
}

pub fn difficulty_info(tier: DifficultyTier) -> (&'static str, &'static str) {
    match tier {
        DifficultyTier::Story => ("Story", "Foes are frailer and strike softer; XP flows 15% faster. For the tale, not the trial."),
        DifficultyTier::Standard => ("Standard", "The descent as tuned — every foe at full strength, XP as earned."),
        DifficultyTier::Hardened => ("Hardened", "Foes are tougher and hit harder; XP flows 10% faster as compensation."),
    }
}

pub const DIFFICULTY_ORDER: [DifficultyTier; 3] = [
    DifficultyTier::Story,
    DifficultyTier::Standard,
    DifficultyTier::Hardened,
];

/// Anything unrecognized (including a legacy save's missing field) is Standard.
pub fn normalize_difficulty(value: Option<&str>) -> DifficultyTier {
    match value {
        Some("story") => DifficultyTier::Story,
        Some("hardened") => DifficultyTier::Hardened,
        _ => DEFAULT_DIFFICULTY,
    }
}

/// Normalize an arbitrary stored value into a valid cycle count.
pub fn normalize_cycle(value: f64) -> u32 {
    if !value.is_finite() || value < 0.0 {
        0
    } else {
        value.floor() as u32
    }
}

/// The ascension multipliers for a given NG+ cycle (cycle 0 = all 1.0).
pub fn cycle_multipliers(cycle: u32) -> DifficultyMultipliers {
    let c = cycle as f64;
    DifficultyMultipliers {
        enemy_hp: 1.0 + 0.35 * c,
        enemy_atk: 1.0 + 0.25 * c,
        xp: 1.0 + 0.2 * c,
    }
}

/// Apply tier AND cycle multipliers to a freshly created enemy.
/// This is the single scaling funnel every spawn path uses.
pub fn scale_enemy_for_difficulty(mut enemy: Enemy, tier: DifficultyTier, cycle: u32) -> Enemy {
    let t = difficulty_multipliers(tier);
    let c = cycle_multipliers(cycle);
    let hp_mult = t.enemy_hp * c.enemy_hp;
    let atk_mult = t.enemy_atk * c.enemy_atk;
    if hp_mult == 1.0 && atk_mult == 1.0 {
        return enemy;
    }
    enemy.max_hp = (enemy.max_hp as f64 * hp_mult).round().max(1.0) as i32;
    enemy.hp = (enemy.hp as f64 * hp_mult).round().max(1.0) as i32;
    enemy.atk = (enemy.atk as f64 * atk_mult).round().max(1.0) as i32;
    enemy
}

/// Apply the tier × cycle XP multipliers inside the XP grant function.
pub fn scale_xp_for_difficulty(xp: i32, tier: DifficultyTier, cycle: u32) -> i32 {
    let mult = difficulty_multipliers(tier).xp * cycle_multipliers(cycle).xp;
    if mult == 1.0 {
        xp
    } else {
        (xp as f64 * mult).round() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EnemyArchetype};

    fn enemy(hp: i32, atk: i32) -> Enemy {
        Enemy {
            id: "e1".into(),
            archetype: EnemyArchetype::CryptSkeleton,
            name: "Skeleton".into(),
            glyph: "S".into(),
            color: "#fff".into(),
            x: 0,
            y: 0,
            hp,
            max_hp: hp,
            atk,
            def: 0,
            xp_reward: 10,
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

    #[test]
    fn standard_cycle0_is_identity() {
        let e = scale_enemy_for_difficulty(enemy(58, 11), DifficultyTier::Standard, 0);
        assert_eq!((e.max_hp, e.atk), (58, 11));
        assert_eq!(scale_xp_for_difficulty(100, DifficultyTier::Standard, 0), 100);
    }

    #[test]
    fn cycle_scaling_matches_ts_formula() {
        // TS pins: skeleton 58/11 at cycle 1 -> 78/14; cycle 3 -> 119/19.
        let e1 = scale_enemy_for_difficulty(enemy(58, 11), DifficultyTier::Standard, 1);
        assert_eq!((e1.max_hp, e1.atk), (78, 14));
        let e3 = scale_enemy_for_difficulty(enemy(58, 11), DifficultyTier::Standard, 3);
        assert_eq!((e3.max_hp, e3.atk), (119, 19));
    }

    #[test]
    fn normalize_defaults_to_standard() {
        assert_eq!(normalize_difficulty(None), DifficultyTier::Standard);
        assert_eq!(normalize_difficulty(Some("bogus")), DifficultyTier::Standard);
        assert_eq!(normalize_difficulty(Some("story")), DifficultyTier::Story);
    }
}
