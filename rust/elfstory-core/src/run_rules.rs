//! Pure run-level rules, ported from `elfstoryreborn/src/game/runRules.ts`.

use crate::types::ZoneMap;
use std::collections::HashMap;

/// Chance that an ordinary kill drops something. Was 0.42 (Nov 16 retune).
pub const NORMAL_KILL_DROP_CHANCE: f64 = 0.3;

/// The last hand-authored floor of the main descent; deeper is the Abyss.
pub const LAST_STORY_FLOOR: u32 = 5;

/// Depth of a main-descent / Abyss zone id (`dungeon_N`), or None otherwise.
pub fn main_floor_depth(zone_id: &str) -> Option<u32> {
    let rest = zone_id.strip_prefix("dungeon_")?;
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok()
}

/// Drop stored Abyss floors that are neither next to the player nor the
/// deepest one reached. Returns true if anything was dropped.
pub fn prune_abyss_zones<Z>(zones: &mut HashMap<String, Z>, current_zone_id: &str) -> bool {
    let current_depth = main_floor_depth(current_zone_id);
    let mut deepest: u32 = 0;
    for id in zones.keys() {
        if let Some(d) = main_floor_depth(id) {
            deepest = deepest.max(d);
        }
    }
    if deepest <= LAST_STORY_FLOOR {
        return false;
    }
    let mut dropped = false;
    zones.retain(|id, _| {
        let keep = match main_floor_depth(id) {
            None => true,
            Some(d) => {
                d <= LAST_STORY_FLOOR
                    || d == deepest
                    || id == current_zone_id
                    || current_depth.map_or(false, |c| (d as i64 - c as i64).abs() <= 1)
            }
        };
        if !keep {
            dropped = true;
        }
        keep
    });
    dropped
}

/// Whether the pause menu may offer "Return to the Ending" (NG+ ascend).
/// A Hardcore run outside a town may not use it as a death escape.
pub fn can_offer_return_to_ending(malakor_defeated: bool, hardcore: bool, in_town: bool) -> bool {
    if !malakor_defeated {
        return false;
    }
    !hardcore || in_town
}

/// The story boss still guarding a floor's way down, or None.
/// While one lives, the floor's stairs down stay sealed.
pub fn boss_blocking_descent(zone: &ZoneMap) -> Option<String> {
    if zone.is_town {
        return None;
    }
    zone.enemies
        .iter()
        .find(|e| e.is_boss == Some(true) && e.hp > 0)
        .map(|e| e.name.clone())
}

/// Deepest dungeon depth implied by a set of discovered zones.
pub fn deepest_depth_for_zones(discovered_zones: &[String], current_depth: i32) -> i32 {
    fn side_depth(id: &str) -> Option<i32> {
        Some(match id {
            "barrow_1" => 2,
            "barrow_2" => 3,
            "mire_1" => 3,
            "mire_2" => 4,
            "frost_1" => 4,
            "frost_2" => 5,
            "frost_3" => 6,
            _ => return None,
        })
    }
    let mut deepest = current_depth.max(0);
    for z in discovered_zones {
        if let Some(d) = side_depth(z) {
            deepest = deepest.max(d);
        } else if let Some(rest) = z.strip_prefix("dungeon_") {
            if let Ok(d) = rest.parse::<i32>() {
                deepest = deepest.max(d);
            }
        }
    }
    deepest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_chance_is_retuned_value() {
        assert_eq!(NORMAL_KILL_DROP_CHANCE, 0.3);
    }

    #[test]
    fn main_floor_depth_parses_dungeon_only() {
        assert_eq!(main_floor_depth("dungeon_3"), Some(3));
        assert_eq!(main_floor_depth("dungeon_12"), Some(12));
        assert_eq!(main_floor_depth("barrow_1"), None);
        assert_eq!(main_floor_depth("oakhaven"), None);
    }

    #[test]
    fn return_to_ending_rules() {
        assert!(!can_offer_return_to_ending(false, false, true));
        assert!(can_offer_return_to_ending(true, false, false));
        assert!(can_offer_return_to_ending(true, true, true));
        assert!(!can_offer_return_to_ending(true, true, false));
    }

    #[test]
    fn deepest_depth_counts_side_zones() {
        let zones = vec!["oakhaven".to_string(), "frost_2".to_string(), "dungeon_4".to_string()];
        assert_eq!(deepest_depth_for_zones(&zones, 0), 5);
        assert_eq!(deepest_depth_for_zones(&[], 3), 3);
    }

    #[test]
    fn prune_keeps_story_and_deepest() {
        let mut zones: HashMap<String, i32> = HashMap::new();
        for n in 1..=10 {
            zones.insert(format!("dungeon_{n}"), n as i32);
        }
        zones.insert("oakhaven".into(), 0);
        let dropped = prune_abyss_zones(&mut zones, "dungeon_6");
        assert!(dropped);
        assert!(zones.contains_key("dungeon_5"));
        assert!(zones.contains_key("dungeon_6"));
        assert!(zones.contains_key("dungeon_7"));
        assert!(zones.contains_key("dungeon_10"));
        assert!(!zones.contains_key("dungeon_8"));
        assert!(zones.contains_key("oakhaven"));
    }
}
