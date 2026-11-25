//! Auto-explore, click-to-travel and the quest journal: App.tsx behaviours the
//! front-end needs. One step per call, so the view can animate and the player
//! can interrupt.

use crate::combat::{find_auto_explore_target, find_path_points};
use crate::session::{GameSession, SessionEvent};
use crate::types::*;
use serde_json::{json, Value};

impl GameSession {
    /// Why walking on its own must stop now, or `None` when it is safe.
    fn auto_stop_reason(&self) -> Option<&'static str> {
        if self.player.hp <= 0 || self.active_prompt.is_some() || self.active_dialogue.is_some() {
            return Some("Not now.");
        }
        let z = self.zone();
        if !z.is_town && z.enemies.iter().any(|e| e.hp > 0 && z.tiles[e.y as usize][e.x as usize].visible) {
            return Some("Enemy sighted!");
        }
        if f64::from(self.player.hp) < f64::from(self.derived().max_hp) * 0.35 {
            return Some("Low health!");
        }
        let hurt = |k| matches!(k, StatusKind::Bleed | StatusKind::Burn | StatusKind::Poison);
        if self.player.statuses.iter().any(|s| hurt(s.kind)) {
            return Some("Afflicted by status damage.");
        }
        None
    }

    fn step_toward(&mut self, to: Pos) -> Result<SessionEvent, String> {
        let before = (self.player.x, self.player.y, self.player.turns_elapsed, self.tile_rev);
        let ev = self.move_player((to.x - before.0).signum(), (to.y - before.1).signum());
        let after = (self.player.x, self.player.y, self.player.turns_elapsed, self.tile_rev);
        if ev == SessionEvent::None && before == after {
            return Err("The way is blocked.".into());
        }
        Ok(ev)
    }

    pub fn auto_explore_step(&mut self) -> Result<SessionEvent, String> {
        if self.in_town() {
            return Err("Nothing to explore in town.".into());
        }
        if let Some(why) = self.auto_stop_reason() {
            return Err(why.into());
        }
        let (px, py) = (self.player.x, self.player.y);
        match find_auto_explore_target(self.zone(), px, py).next_step {
            Some(p) => self.step_toward(p),
            None => Err("Floor explored.".into()),
        }
    }

    /// One step of a walk to an explored tile the player clicked.
    pub fn travel_step(&mut self, tx: i32, ty: i32) -> Result<SessionEvent, String> {
        if let Some(why) = self.auto_stop_reason() {
            return Err(why.into());
        }
        let (px, py) = (self.player.x, self.player.y);
        if (px, py) == (tx, ty) {
            return Err("Arrived.".into());
        }
        match find_path_points(self.zone(), px, py, tx, ty, true, 200).first().copied() {
            Some(p) => self.step_toward(p),
            None => Err("No known path there.".into()),
        }
    }

    /// The quest journal: active quests first, with the same hint the tracker shows.
    pub fn journal_json(&self) -> Value {
        let tracker = self.quest_tracker_json();
        let hint = |id: &str| {
            tracker
                .as_array()
                .and_then(|a| a.iter().find(|q| q["id"] == id))
                .map(|q| q["hint"].clone())
                .unwrap_or(Value::Null)
        };
        let mut rows: Vec<Value> = self
            .quests
            .values()
            .map(|q| {
                let def = crate::npcs::quest_def(&q.id);
                json!({
                    "id": q.id, "status": q.status.as_str(), "outcome": q.outcome_label, "history": q.history,
                    "title": def.as_ref().map_or(q.id.as_str(), |d| d.title),
                    "giver": def.as_ref().map_or("", |d| d.giver),
                    "summary": def.as_ref().map_or("", |d| d.summary),
                    "hint": hint(&q.id),
                })
            })
            .collect();
        let rank = |v: &Value| match v["status"].as_str() {
            Some("active") => 0,
            Some("completed") => 1,
            _ => 2,
        };
        rows.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a["title"].as_str().cmp(&b["title"].as_str())));
        json!({ "quests": rows, "total": crate::npcs::quest_defs().len() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_explore_uncovers_a_cleared_floor_and_stops() {
        for seed in 1..8 {
            let mut s = GameSession::new_game("G", StarterClass::Vanguard, seed);
            s.debug_warp(ZoneId::Dungeon(1), -1, -1);
            let seen = |s: &GameSession| s.zone().tiles.iter().flatten().filter(|t| t.explored).count();
            let start = seen(&s);
            let mut steps = 0;
            for _ in 0..4000 {
                s.zone_mut().enemies.clear();
                s.player.hp = s.derived().max_hp;
                s.player.statuses.clear();
                if s.active_prompt.is_some() {
                    s.close_prompt();
                }
                if s.auto_explore_step().is_err() {
                    break;
                }
                steps += 1;
                if s.current != ZoneId::Dungeon(1) {
                    break;
                }
            }
            assert!(steps < 4000, "seed {seed} never finished");
            assert!(s.current != ZoneId::Dungeon(1) || seen(&s) > start * 2, "seed {seed} barely moved");
        }
    }

    #[test]
    fn walking_alone_refuses_danger() {
        let mut s = GameSession::new_game("G", StarterClass::Vanguard, 2);
        assert!(s.auto_explore_step().is_err(), "town");
        s.debug_warp(ZoneId::Dungeon(1), -1, -1);
        s.zone_mut().enemies.truncate(1);
        let (px, py) = (s.player.x, s.player.y);
        {
            let z = s.zone_mut();
            z.enemies[0].x = px;
            z.enemies[0].y = py;
            z.tiles[py as usize][px as usize].visible = true;
        }
        assert_eq!(s.auto_explore_step().unwrap_err(), "Enemy sighted!");
        s.zone_mut().enemies.clear();
        s.player.hp = 1;
        assert_eq!(s.auto_explore_step().unwrap_err(), "Low health!");
    }

    #[test]
    fn travel_walks_to_a_known_tile() {
        let mut s = GameSession::new_game("G", StarterClass::Vanguard, 2);
        let t = s.zone().tiles.iter().flatten().find(|t| {
            t.kind == TileKind::Road && (t.x - s.player.x).abs() > 3 && !s.zone().npcs.iter().any(|n| n.x == t.x && n.y == t.y)
        }).map(|t| (t.x, t.y)).unwrap();
        let mut arrived = false;
        for _ in 0..80 {
            if let Err(e) = s.travel_step(t.0, t.1) {
                arrived = e == "Arrived.";
                break;
            }
        }
        assert!(arrived && (s.player.x, s.player.y) == t);
    }
}
