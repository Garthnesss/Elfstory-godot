//! ElfStory core simulation: pure Rust, zero Godot dependency.
//! Ported from elfstoryreborn (TS) Milestones 1-3 — see HANDOFF.md guardrails.

pub mod combat;
pub mod combat_turn;
pub mod difficulty;
pub mod loot;
pub mod npcs;
pub mod rng;
pub mod run_rules;
pub mod save;
pub mod session;
pub mod session_save;
pub mod skills;
pub mod town;
pub mod town_layout;
pub mod types;
pub mod world;
pub mod world_interact;
