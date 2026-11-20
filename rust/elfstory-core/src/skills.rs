//! Skills, prodigies, and derived stats, ported from
//! `elfstoryreborn/src/game/skills.ts`.
//!
//! Rust divergences from TS:
//! - `PlayerState.skills` / `cooldowns` are `HashMap<String, i32>` keyed
//!   by skill-id string (per types.rs), so rank lookups go through
//!   `SkillId::as_str()` instead of TS's typed record indexing.
//! - types.rs has no `ProdigyDef` struct, so it is defined locally here
//!   (see [`ProdigyDef`]); its `requirements` keep TS's
//!   `Partial<Attributes>` as an ordered `(AttributeKey, i32)` list so
//!   `can_learn_prodigy` reports failures in TS's entry order.
//! - Town Tier bonuses are derived from `crate::town::population_tier`
//!   (town.rs exposes the tier, not TS's `getTownTierBonuses`); the
//!   bonus formula itself is ported verbatim in
//!   [`town_tier_bonuses_for_tier`].

use crate::town::{population_tier, TownState};
use crate::types::{
    AttributeKey, Attributes, CompanionKind, DerivedStats, ItemStats, PlayerState, ProdigyId,
    QuestProgress, QuestStatus, RelicSetId, SkillBranch, SkillDef, SkillId, SkillTargeting,
    StarterClass, StatusKind, WorldFlags,
};
use std::collections::HashMap;

/// Result of a learn check (TS `{ ok: boolean; reason?: string }`).
#[derive(Debug, Clone, PartialEq)]
pub struct LearnCheck {
    pub ok: bool,
    pub reason: Option<String>,
}

impl LearnCheck {
    fn ok() -> Self {
        Self { ok: true, reason: None }
    }
    fn fail(reason: impl Into<String>) -> Self {
        Self { ok: false, reason: Some(reason.into()) }
    }
}

/// TS `ATTRIBUTE_META` value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttributeMeta {
    pub name: &'static str,
    pub short: &'static str,
    pub description: &'static str,
    pub color: &'static str,
}

/// TS `ATTRIBUTE_META`.
pub fn attribute_meta(key: AttributeKey) -> AttributeMeta {
    match key {
        AttributeKey::Might => AttributeMeta {
            name: "Might",
            short: "MIG",
            description: "+3 Attack Power and +5 Max HP per point. Breaks rusted gates & empowers Vanguard strikes.",
            color: "#D9534F",
        },
        AttributeKey::Agility => AttributeMeta {
            name: "Agility",
            short: "AGI",
            description: "+2.5% Crit Chance, +2% Dodge, and +1 Attack Power per point. Detects hidden traps & vaults.",
            color: "#4EA885",
        },
        AttributeKey::Arcana => AttributeMeta {
            name: "Arcana",
            short: "ARC",
            description: "+3 Spell Power and +4 Max Mana per point. Unlocks ancient runes & potent Runecaster spells.",
            color: "#5B8DEF",
        },
        AttributeKey::Resolve => AttributeMeta {
            name: "Resolve",
            short: "RES",
            description: "+2 Defense and +6 Max HP per point. Steadies morale in tense dialogue & boosts ward scaling.",
            color: "#D99B26",
        },
    }
}

/// TS `BRANCH_META` value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BranchMeta {
    pub name: &'static str,
    pub subtitle: &'static str,
    pub color: &'static str,
    pub accent_class: &'static str,
}

/// TS `BRANCH_META`.
pub fn branch_meta(branch: SkillBranch) -> BranchMeta {
    match branch {
        SkillBranch::Vanguard => BranchMeta {
            name: "Vanguard",
            subtitle: "Heavy strikes, guard stances, and crimson sustain.",
            color: "#D9534F",
            accent_class: "text-rose-400",
        },
        SkillBranch::Shadowcraft => BranchMeta {
            name: "Shadowcraft",
            subtitle: "Blink ambushes, toxic edges, and lethal precision.",
            color: "#4EA885",
            accent_class: "text-emerald-400",
        },
        SkillBranch::Runecaster => BranchMeta {
            name: "Runecaster",
            subtitle: "Piercing ember lances, frost control, and mana echo.",
            color: "#5B8DEF",
            accent_class: "text-sky-400",
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn skill(
    id: SkillId,
    name: &str,
    branch: SkillBranch,
    tier: u8,
    kind: &str,
    targeting: SkillTargeting,
    range: i32,
    mana_cost: i32,
    cooldown: i32,
    glyph: &str,
    summary: &str,
    rank_descriptions: [&str; 3],
    scaling_note: &str,
) -> SkillDef {
    SkillDef {
        id,
        name: name.to_string(),
        branch,
        tier,
        kind: kind.to_string(),
        targeting,
        range,
        mana_cost,
        cooldown,
        glyph: glyph.to_string(),
        summary: summary.to_string(),
        rank_descriptions: rank_descriptions.map(|s| s.to_string()),
        scaling_note: scaling_note.to_string(),
    }
}

/// The SKILL_DEFS table: TS `SKILLS`, all 12 skills in TS object order.
pub fn skill_defs() -> Vec<SkillDef> {
    use SkillBranch::*;
    use SkillId::*;
    use SkillTargeting::*;
    vec![
        skill(
            Cleave, "Sundering Cleave", Vanguard, 1, "active", Adjacent, 1, 5, 2, "⚔",
            "Strike an enemy and sweep adjacent foes around you.",
            [
                "Deals 135% ATK to target and 75% ATK to adjacent enemies.",
                "Deals 155% ATK to target, 95% ATK splash, and applies Bleed (2t).",
                "Deals 180% ATK to target, 120% ATK splash, and Bleed (3t).",
            ],
            "Scales with Might & Weapon ATK",
        ),
        skill(
            ShieldBash, "Bastion Bash", Vanguard, 1, "active", Adjacent, 1, 6, 3, "⛨",
            "Slam a foe with armor-scaled force, interrupting telegraphed attacks.",
            [
                "Deals 90% ATK + 100% DEF and Stuns target for 1 turn.",
                "Deals 110% ATK + 130% DEF, Stuns 1t, and grants Shielded (+4 DEF, 2t).",
                "Deals 130% ATK + 160% DEF, Stuns 2t, and grants Shielded (+6 DEF, 3t).",
            ],
            "Scales with Defense & Resolve",
        ),
        skill(
            Bloodlust, "Crimson Vigor", Vanguard, 2, "passive", Passive, 0, 0, 0, "🩸",
            "Passive: Drain life on melee attacks and strike harder when wounded.",
            [
                "+4% Lifesteal and +10% Crit against enemies below 50% HP.",
                "+7% Lifesteal and +18% Crit against enemies below 50% HP.",
                "+10% Lifesteal, +25% Crit vs wounded foes, and kills heal 5 HP.",
            ],
            "Synergizes with Shadowcraft Crit",
        ),
        skill(
            IronFortress, "Iron Fortress", Vanguard, 3, "active", Self_, 0, 8, 5, "♜",
            "Brace behind an unyielding ward, gaining massive Defense and Thorns.",
            [
                "Grants Shielded (+8 DEF, 3t) and +6 Thorns retaliation.",
                "Grants Shielded (+11 DEF, 3t), +10 Thorns, and cleanses Bleed/Poison.",
                "Grants Shielded (+14 DEF, 4t), +15 Thorns, cleanses debuffs, and heals 12 HP.",
            ],
            "Keystone · Scales with Resolve",
        ),
        skill(
            Shadowstep, "Shadowstep", Shadowcraft, 1, "active", Blink, 4, 6, 3, "◈",
            "Blink through shadow to any visible tile within 4 paces; strikes next to a foe.",
            [
                "Teleport up to 4 tiles; if landing adjacent to a foe, ambush for 125% ATK.",
                "Teleport up to 5 tiles; ambush deals 150% ATK with guaranteed Critical Hit.",
                "Teleport up to 5 tiles; ambush deals 175% ATK Crit and grants 1t Dodge surge.",
            ],
            "Scales with Agility & Crit",
        ),
        skill(
            VenomTip, "Viper Edge", Shadowcraft, 1, "active", Ranged, 4, 5, 2, "➶",
            "Hurl a coated blade from range, inflicting stacking Poison and Bleed.",
            [
                "Range 4: Deals 95% ATK + Agility and applies Poison (3 dmg/turn, 3t).",
                "Range 5: Deals 115% ATK + Agility, applies Poison (5/t, 3t) and Bleed (2t).",
                "Range 5: Deals 135% ATK + Agility, Poison (7/t, 4t), Bleed (3t), ignores 50% DEF.",
            ],
            "Scales with Agility",
        ),
        skill(
            EvasiveInstinct, "Phantom Reflex", Shadowcraft, 2, "passive", Passive, 0, 0, 0, "✦",
            "Passive: Heighten evasion and retaliate when enemies miss.",
            [
                "+8% Dodge Chance and +5% Crit Chance.",
                "+13% Dodge Chance, +8% Crit Chance, and dodging restores 2 Mana.",
                "+18% Dodge Chance, +12% Crit, and dodging triggers a free counter-slash!",
            ],
            "Synergizes with Runecaster Mana",
        ),
        skill(
            ExecutionersMark, "Executioner's Mark", Shadowcraft, 3, "active", Ranged, 5, 7, 4, "⌖",
            "Brand a foe with a death glyph so all hits deal amplified damage.",
            [
                "Marks target for 4 turns (+35% damage taken) and deals 110% ATK.",
                "Marks target for 4 turns (+45% damage taken) and deals 140% ATK.",
                "Marks target for 5 turns (+60% damage taken), deals 170% ATK; kills reset cooldown.",
            ],
            "Keystone · Amplifies Spells & Strikes",
        ),
        skill(
            EmberLance, "Ember Lance", Runecaster, 1, "active", Ranged, 5, 7, 2, "🜂",
            "Project a piercing lance of cinder fire, scorching and igniting the target.",
            [
                "Range 5: Deals 10 + 125% Spell Power fire damage and inflicts Burn (3/t, 2t).",
                "Range 6: Deals 13 + 145% Spell Power, Burn (5/t, 3t), and pierces 1 tile behind.",
                "Range 6: Deals 13 + 155% Spell Power, Burn (7/t, 3t), and explodes in a 1-tile radius.",
            ],
            "Scales with Arcana & Spell Power",
        ),
        skill(
            FrostNova, "Rime Nova", Runecaster, 1, "active", Self_, 2, 8, 3, "❄",
            "Erupt an icy ring around you, damaging and Chilling nearby enemies.",
            [
                "Radius 2: Deals 10 + 110% Spell Power and Chills enemies (halves damage, 2t).",
                "Radius 2: Deals 14 + 140% Spell Power, Chills 2t, and Stuns adjacent foes 1t.",
                "Radius 3: Deals 18 + 170% Spell Power, Chills 3t, Stuns adjacent foes 1t.",
            ],
            "Crowd Control · Scales with Arcana",
        ),
        skill(
            SiphonSoul, "Soul Siphon", Runecaster, 2, "passive", Passive, 0, 0, 0, "☽",
            "Passive: Recover Mana and Health whenever you slay or cast spells.",
            [
                "+4 Spell Power. Slaying an enemy restores 3 Mana.",
                "+7 Spell Power. Slaying restores 5 Mana and 4 HP.",
                "+11 Spell Power. Slaying restores 7 Mana and 7 HP; crits restore 2 Mana.",
            ],
            "Sustain Engine · Pairs with any build",
        ),
        skill(
            ArcaneEcho, "Arcane Echo", Runecaster, 3, "passive", Passive, 0, 0, 0, "❖",
            "Keystone Passive: Active skills gain a chance to echo a second blast for free.",
            [
                "25% chance for active skills to trigger a bonus 50% power Arcane Bolt.",
                "40% chance for active skills to trigger a 65% power Arcane Bolt and reduce cooldowns by 1.",
                "55% chance to trigger an 85% power Arcane Bolt, reduce cooldowns, and gain +10% Crit.",
            ],
            "Keystone · Amplifies ALL Active Skills",
        ),
    ]
}

/// Look up one skill definition by id (TS `SKILLS[skillId]`).
pub fn skill_def(id: SkillId) -> SkillDef {
    skill_defs()
        .into_iter()
        .find(|d| d.id == id)
        .expect("all SkillId variants are in the table")
}

/// TS `ALL_SKILLS_LIST`.
pub fn all_skills_list() -> Vec<SkillDef> {
    skill_defs()
}

/// TS `getBranchInvestedPoints`.
pub fn get_branch_invested_points(skills: &HashMap<String, i32>, branch: SkillBranch) -> i32 {
    skill_defs()
        .iter()
        .filter(|s| s.branch == branch)
        .map(|s| skills.get(s.id.as_str()).copied().unwrap_or(0))
        .sum()
}

fn skill_rank(player: &PlayerState, id: SkillId) -> i32 {
    player.skills.get(id.as_str()).copied().unwrap_or(0)
}

/// TS `canLearnSkill`.
pub fn can_learn_skill(player: &PlayerState, skill_id: SkillId) -> LearnCheck {
    let def = skill_def(skill_id);
    let current_rank = skill_rank(player, skill_id);
    if current_rank >= 3 {
        return LearnCheck::fail("Mastered (Rank 3/3)");
    }
    if player.skill_points <= 0 {
        return LearnCheck::fail("Requires 1 Skill Point");
    }
    let branch_pts = get_branch_invested_points(&player.skills, def.branch);
    if def.tier == 2 && branch_pts < 1 {
        return LearnCheck::fail(format!(
            "Requires 1 pt in {}",
            branch_meta(def.branch).name
        ));
    }
    if def.tier == 3 && branch_pts < 3 {
        return LearnCheck::fail(format!(
            "Requires 3 pts in {}",
            branch_meta(def.branch).name
        ));
    }
    LearnCheck::ok()
}

/// TS `computeBuildTitle`.
pub fn compute_build_title(player: &PlayerState) -> String {
    let v = get_branch_invested_points(&player.skills, SkillBranch::Vanguard);
    let s = get_branch_invested_points(&player.skills, SkillBranch::Shadowcraft);
    let r = get_branch_invested_points(&player.skills, SkillBranch::Runecaster);
    let total = v + s + r;
    if total == 0 {
        return player.starter_class.to_string();
    }

    // (branch, points) in TS's vanguard/shadowcraft/runecaster order;
    // sort_by is stable, matching JS Array.sort stability.
    let mut active: Vec<(SkillBranch, i32)> = [
        (SkillBranch::Vanguard, v),
        (SkillBranch::Shadowcraft, s),
        (SkillBranch::Runecaster, r),
    ]
    .into_iter()
    .filter(|x| x.1 > 0)
    .collect();
    active.sort_by(|a, b| b.1.cmp(&a.1));

    if active.len() >= 3 && active[2].1 >= 1 {
        return "Tri-Glyph Paragon".to_string();
    }
    if active.len() >= 2 && active[1].1 >= 2 {
        let mut pair = [active[0].0.as_str(), active[1].0.as_str()];
        pair.sort();
        let pair = pair.join("+");
        match pair.as_str() {
            "shadowcraft+vanguard" => return "Crimson Duelist".to_string(),
            "runecaster+vanguard" => return "Ashen Templar".to_string(),
            "runecaster+shadowcraft" => return "Runic Nightblade".to_string(),
            _ => {}
        }
    }
    match active[0].0 {
        SkillBranch::Vanguard => {
            if v >= 5 { "Iron Juggernaut".into() } else { "Oathsworn Vanguard".into() }
        }
        SkillBranch::Shadowcraft => {
            if s >= 5 { "Phantom Executioner".into() } else { "Shadow Stalker".into() }
        }
        SkillBranch::Runecaster => {
            if r >= 5 { "Archon of Cinder".into() } else { "Runecaster Adept".into() }
        }
    }
}

/// TS `getEquippedSetCounts` (all four sets present, defaulting to 0).
pub fn get_equipped_set_counts(player: &PlayerState) -> HashMap<RelicSetId, i32> {
    let mut counts: HashMap<RelicSetId, i32> = HashMap::new();
    counts.insert(RelicSetId::StarlightOutrider, 0);
    counts.insert(RelicSetId::Cinderforged, 0);
    counts.insert(RelicSetId::Thornbound, 0);
    counts.insert(RelicSetId::Rimebound, 0);
    for item in player.equipped.values() {
        if let Some(set_id) = item.set_id {
            *counts.entry(set_id).or_insert(0) += 1;
        }
    }
    counts
}

/// TS `RELIC_SET_BONUSES` for one set (2-piece values promised by the
/// Pack's set cards / `RELIC_SETS` bonus labels in loot.rs).
pub fn relic_set_bonus(set_id: RelicSetId) -> ItemStats {
    match set_id {
        RelicSetId::StarlightOutrider => ItemStats {
            atk: Some(4),
            crit_chance: Some(8.0),
            dodge_chance: Some(6.0),
            max_hp: Some(10),
            ..Default::default()
        },
        RelicSetId::Cinderforged => ItemStats {
            def: Some(4),
            spell_power: Some(6),
            thorns: Some(6),
            max_hp: Some(15),
            ..Default::default()
        },
        RelicSetId::Thornbound => ItemStats {
            atk: Some(3),
            thorns: Some(4),
            lifesteal: Some(3.0),
            max_hp: Some(12),
            ..Default::default()
        },
        RelicSetId::Rimebound => ItemStats {
            def: Some(5),
            dodge_chance: Some(8.0),
            max_mana: Some(10),
            max_hp: Some(8),
            ..Default::default()
        },
    }
}

/// All four sets' 2-piece bonuses (TS `RELIC_SET_BONUSES`).
pub fn relic_set_bonuses() -> HashMap<RelicSetId, ItemStats> {
    [
        RelicSetId::StarlightOutrider,
        RelicSetId::Cinderforged,
        RelicSetId::Thornbound,
        RelicSetId::Rimebound,
    ]
    .into_iter()
    .map(|id| (id, relic_set_bonus(id)))
    .collect()
}

/// Population Tier bonuses (TS `TownTierBonuses` from town.ts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TownTierBonuses {
    pub tier: u32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub spell_power: i32,
}

/// The bonus formula from TS `getTownTierBonuses`:
/// per tier +4 Max HP, +1 ATK, +1 Spell Power;
/// +1 DEF at Tier 2, +2 DEF at Tier 3.
pub fn town_tier_bonuses_for_tier(tier: u32) -> TownTierBonuses {
    if tier == 0 {
        return TownTierBonuses { tier: 0, max_hp: 0, atk: 0, def: 0, spell_power: 0 };
    }
    TownTierBonuses {
        tier,
        max_hp: tier as i32 * 4,
        atk: tier as i32,
        def: if tier >= 3 { 2 } else if tier >= 2 { 1 } else { 0 },
        spell_power: tier as i32,
    }
}

/// TS `getTownTierBonuses`, via town.rs's `population_tier`.
pub fn get_town_tier_bonuses(town: Option<&TownState>) -> TownTierBonuses {
    match town {
        None => town_tier_bonuses_for_tier(0),
        Some(t) => town_tier_bonuses_for_tier(population_tier(t)),
    }
}

/// TS `computeDerivedStats`. The TS `worldFlags` parameter is accepted
/// for signature parity but is not read by the TS body.
pub fn compute_derived_stats(
    player: &PlayerState,
    _world_flags: Option<&WorldFlags>,
    town: Option<&TownState>,
) -> DerivedStats {
    let attrs = &player.attributes;
    let town_bonuses = get_town_tier_bonuses(town);
    let mut max_hp = 34
        + (player.level - 1) * 6
        + attrs.might * 5
        + attrs.resolve * 6
        + player.bonus_hp.unwrap_or(0)
        + town_bonuses.max_hp;
    let mut max_mana = 14 + (player.level - 1) * 2 + attrs.arcana * 4;
    let mut atk = 6 + attrs.might * 3 + attrs.agility + town_bonuses.atk;
    let mut def = 1 + attrs.resolve * 2 + town_bonuses.def;
    let mut spell_power = 4 + attrs.arcana * 3 + town_bonuses.spell_power;
    let mut crit_chance: f64 = 6.0 + attrs.agility as f64 * 2.5;
    let mut dodge_chance: f64 = 3.0 + attrs.agility as f64 * 2.0;
    let mut lifesteal: f64 = 0.0;
    let mut thorns: i32 = 0;

    for item in player.equipped.values() {
        let s = &item.stats;
        max_hp += s.max_hp.unwrap_or(0);
        max_mana += s.max_mana.unwrap_or(0);
        atk += s.atk.unwrap_or(0);
        def += s.def.unwrap_or(0);
        spell_power += s.spell_power.unwrap_or(0);
        crit_chance += s.crit_chance.unwrap_or(0.0);
        dodge_chance += s.dodge_chance.unwrap_or(0.0);
        lifesteal += s.lifesteal.unwrap_or(0.0);
        thorns += s.thorns.unwrap_or(0);
    }

    let set_counts = get_equipped_set_counts(player);
    for (set_id, bonus) in relic_set_bonuses() {
        if set_counts.get(&set_id).copied().unwrap_or(0) < 2 {
            continue;
        }
        max_hp += bonus.max_hp.unwrap_or(0);
        max_mana += bonus.max_mana.unwrap_or(0);
        atk += bonus.atk.unwrap_or(0);
        def += bonus.def.unwrap_or(0);
        spell_power += bonus.spell_power.unwrap_or(0);
        crit_chance += bonus.crit_chance.unwrap_or(0.0);
        dodge_chance += bonus.dodge_chance.unwrap_or(0.0);
        lifesteal += bonus.lifesteal.unwrap_or(0.0);
        thorns += bonus.thorns.unwrap_or(0);
    }

    let blood_rank = skill_rank(player, SkillId::Bloodlust);
    if blood_rank > 0 {
        lifesteal += match blood_rank {
            1 => 4.0,
            2 => 7.0,
            _ => 10.0,
        };
    }

    let evasive_rank = skill_rank(player, SkillId::EvasiveInstinct);
    if evasive_rank > 0 {
        dodge_chance += match evasive_rank {
            1 => 8.0,
            2 => 13.0,
            _ => 18.0,
        };
        crit_chance += match evasive_rank {
            1 => 5.0,
            2 => 8.0,
            _ => 12.0,
        };
    }

    let siphon_rank = skill_rank(player, SkillId::SiphonSoul);
    if siphon_rank > 0 {
        spell_power += match siphon_rank {
            1 => 4,
            2 => 7,
            _ => 11,
        };
    }

    if skill_rank(player, SkillId::ArcaneEcho) == 3 {
        crit_chance += 10.0;
    }

    let prodigies: &[ProdigyId] = player.prodigies.as_deref().unwrap_or(&[]);
    if prodigies.contains(&ProdigyId::CarryTheWorld) {
        atk += 12;
        max_hp += 20;
    }
    if prodigies.contains(&ProdigyId::DraconicWill) {
        def += 4;
        max_hp += 12;
    }
    if prodigies.contains(&ProdigyId::FlexibleCombat) {
        crit_chance += 5.0;
    }
    if prodigies.contains(&ProdigyId::ArcaneConduit) {
        spell_power += 8;
    }

    if let Some(companion) = &player.companion {
        if companion.hp > 0 {
            match companion.kind {
                CompanionKind::SilverwindLynx => crit_chance += 3.0,
                CompanionKind::StarlightOwl => spell_power += 2,
            }
        }
    }

    for st in &player.statuses {
        match st.kind {
            StatusKind::Shielded => def += st.potency,
            StatusKind::SpineGuard => {
                def += 5;
                thorns += 6;
            }
            _ => {}
        }
    }

    DerivedStats {
        max_hp,
        max_mana,
        atk,
        def,
        spell_power,
        crit_chance: crit_chance.round().min(75.0),
        dodge_chance: dodge_chance.round().min(55.0),
        lifesteal,
        thorns,
        build_title: compute_build_title(player),
    }
}

/// Prodigy (capstone) definition. NOT in types.rs (which only carries
/// `ProdigyId`), so it is defined here mirroring TS `ProdigyDef` in
/// types.ts. `requirements` preserves TS's `Partial<Attributes>` as an
/// ordered list in the TS literal's entry order.
#[derive(Debug, Clone, PartialEq)]
pub struct ProdigyDef {
    pub id: ProdigyId,
    pub name: String,
    pub glyph: String,
    /// TS `pillar`: 'Vanguard' | 'Shadowcraft' | 'Runecaster' | 'Hybrid'.
    pub pillar: String,
    pub requirement_label: String,
    pub requirements: Vec<(AttributeKey, i32)>,
    pub summary: String,
    pub effect_note: String,
}

fn prodigy(
    id: ProdigyId,
    name: &str,
    glyph: &str,
    pillar: &str,
    requirement_label: &str,
    requirements: Vec<(AttributeKey, i32)>,
    summary: &str,
    effect_note: &str,
) -> ProdigyDef {
    ProdigyDef {
        id,
        name: name.to_string(),
        glyph: glyph.to_string(),
        pillar: pillar.to_string(),
        requirement_label: requirement_label.to_string(),
        requirements,
        summary: summary.to_string(),
        effect_note: effect_note.to_string(),
    }
}

/// TS `PRODIGIES`, all 6 capstones.
pub fn prodigy_defs() -> Vec<ProdigyDef> {
    use AttributeKey::*;
    use ProdigyId::*;
    vec![
        prodigy(
            Cauterize, "Cauterize", "🜂", "Runecaster", "Level 6+ · Arcana 6",
            vec![(Arcana, 6)],
            "Your arcane fire refuses to let death claim you.",
            "Once every 25 turns, lethal damage leaves you alive at 25% HP with +10 DEF Cauterize Ward for 3 turns.",
        ),
        prodigy(
            DraconicWill, "Draconic Will", "⛨", "Vanguard", "Level 6+ · Resolve 6",
            vec![(Resolve, 6)],
            "Harden body and mind like ancient wyrm-scale.",
            "+4 DEF, +12 Max HP, 60% chance to resist incoming ailments, and auto-cleanse 1 debuff every 4 turns.",
        ),
        prodigy(
            FlexibleCombat, "Flexible Combat", "◈", "Shadowcraft", "Level 6+ · Agility 6",
            vec![(Agility, 6)],
            "Weave seamless unarmed follow-ups into every strike.",
            "+5% Crit Chance and 45% chance on melee or Shadowcraft hits to land a free bonus follow-up strike (65% ATK).",
        ),
        prodigy(
            CarryTheWorld, "I Can Carry the World", "⚔", "Vanguard", "Level 6+ · Might 6",
            vec![(Might, 6)],
            "Towering strength turns every swing into a shockwave.",
            "+12 Attack Power, +20 Max HP, and +25% bonus damage on Sundering Cleave and Bastion Bash.",
        ),
        prodigy(
            ArcaneConduit, "Arcane Conduit", "❖", "Hybrid", "Level 6+ · Arcana 5 · Resolve 5",
            vec![(Arcana, 5), (Resolve, 5)],
            "Channel spells in an unbroken cycle.",
            "+8 Spell Power; casting any Runecaster skill shaves 1 turn off all other cooldowns.",
        ),
        prodigy(
            SpineOfWorld, "Spine of the World", "♜", "Hybrid", "Level 6+ · Might 5 · Resolve 5",
            vec![(Might, 5), (Resolve, 5)],
            "Stone-hard poise punishes any foe bold enough to strike you.",
            "Whenever an enemy hits you, gain Spine Guard for 3 turns (+5 DEF and +6 Thorns retaliation).",
        ),
    ]
}

/// Look up one prodigy definition by id (TS `PRODIGIES[id]`).
pub fn prodigy_def(id: ProdigyId) -> ProdigyDef {
    prodigy_defs()
        .into_iter()
        .find(|d| d.id == id)
        .expect("all ProdigyId variants are in the table")
}

/// TS `ALL_PRODIGIES_LIST`.
pub fn all_prodigies_list() -> Vec<ProdigyDef> {
    prodigy_defs()
}

/// Capstones are bought with Skill Points (TS `PRODIGY_SKILL_COST`).
pub const PRODIGY_SKILL_COST: i32 = 2;

/// TS `hasProdigy`.
pub fn has_prodigy(player: &PlayerState, id: ProdigyId) -> bool {
    player.prodigies.as_ref().map_or(false, |p| p.contains(&id))
}

/// TS `canLearnProdigy`.
pub fn can_learn_prodigy(player: &PlayerState, id: ProdigyId) -> LearnCheck {
    if has_prodigy(player, id) {
        return LearnCheck::fail("Already Claimed");
    }
    let def = prodigy_def(id);
    if player.level < 6 {
        return LearnCheck::fail("Unlocks at Level 6");
    }
    if player.skill_points < PRODIGY_SKILL_COST {
        return LearnCheck::fail(format!("Costs {} Skill Points", PRODIGY_SKILL_COST));
    }
    for (attr, req) in &def.requirements {
        let cur = player.attributes.get(*attr);
        if cur < *req {
            return LearnCheck::fail(format!(
                "Needs {} {} ({}/{})",
                attribute_meta(*attr).name,
                req,
                cur,
                req
            ));
        }
    }
    LearnCheck::ok()
}

/// TS `getStarterPreset` result.
#[derive(Debug, Clone, PartialEq)]
pub struct StarterPreset {
    pub attributes: Attributes,
    pub initial_skill: SkillId,
    pub weapon_tag: String,
}

/// TS `getStarterPreset` (the default arm covers Wanderer).
pub fn get_starter_preset(starter_class: StarterClass) -> StarterPreset {
    match starter_class {
        StarterClass::Vanguard => StarterPreset {
            attributes: Attributes { might: 3, agility: 1, arcana: 1, resolve: 2 },
            initial_skill: SkillId::Cleave,
            weapon_tag: "starter_vanguard".to_string(),
        },
        StarterClass::Shadowcraft => StarterPreset {
            attributes: Attributes { might: 1, agility: 3, arcana: 1, resolve: 2 },
            initial_skill: SkillId::Shadowstep,
            weapon_tag: "starter_shadow".to_string(),
        },
        StarterClass::Runecaster => StarterPreset {
            attributes: Attributes { might: 1, agility: 1, arcana: 3, resolve: 2 },
            initial_skill: SkillId::EmberLance,
            weapon_tag: "starter_rune".to_string(),
        },
        StarterClass::Wanderer => StarterPreset {
            attributes: Attributes { might: 2, agility: 2, arcana: 2, resolve: 1 },
            initial_skill: SkillId::VenomTip,
            weapon_tag: "starter_wanderer".to_string(),
        },
    }
}

/// TS `computeRespecPoints` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RespecPoints {
    pub attribute_points: i32,
    pub skill_points: i32,
}

/// TS `computeRespecPoints`.
pub fn compute_respec_points(
    level: i32,
    world_flags: &WorldFlags,
    quests: &HashMap<String, QuestProgress>,
) -> RespecPoints {
    let safe_level = level.max(1);
    let mira_cantor_turned_in = quests
        .get("herbalist_blight")
        .map_or(false, |q| q.outcome_label.as_deref() == Some("Silenced the Bone Cantor"));
    let pacified_vaelen = world_flags.cantor_pacified == Some(true)
        || (world_flags.cantor_pacified.is_none()
            && world_flags.blight_choice.as_deref() == Some("cantor_appeased")
            && !mira_cantor_turned_in);

    let skill_points = 2
        + (safe_level - 1)
        + i32::from(world_flags.bell_choice.as_deref() == Some("attuned_lyra"))
        + i32::from(pacified_vaelen)
        + i32::from(mira_cantor_turned_in)
        + i32::from(
            quests.get("spores_of_bog").map_or(false, |q| q.status == QuestStatus::Completed),
        );

    let attribute_points = (safe_level - 1) * 2
        + if world_flags.bell_choice.as_deref() == Some("shattered_self") { 2 } else { 0 }
        + i32::from(
            quests.get("barrow_purge").map_or(false, |q| q.status == QuestStatus::Completed),
        )
        + i32::from(
            quests
                .get("cinder_and_frost")
                .map_or(false, |q| q.status == QuestStatus::Completed),
        );

    RespecPoints { attribute_points, skill_points }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Item, ItemCategory, ItemRarity, ItemSlot, ItemStats,
    };

    fn test_player() -> PlayerState {
        // Mirror of the player built in ts/tests/town.test.ts:
        // Vanguard preset attributes, level 1, starter broadsword.
        let preset = get_starter_preset(StarterClass::Vanguard);
        let mut equipped: HashMap<String, Item> = HashMap::new();
        equipped.insert(
            "weapon".to_string(),
            Item {
                id: "itm_test".into(),
                name: "Recruit Broadsword".into(),
                category: ItemCategory::Equipment,
                slot: Some(ItemSlot::Weapon),
                consumable_kind: None,
                rarity: ItemRarity::Uncommon,
                glyph: "/".into(),
                level_req: 1,
                upgrade_level: 0,
                stats: ItemStats { atk: Some(5), def: Some(1), ..Default::default() },
                affix: None,
                affix_label: None,
                set_id: None,
                value: 24,
                description: String::new(),
                quantity: None,
                quest_tag: None,
            },
        );
        PlayerState {
            name: "Garth".into(),
            starter_class: StarterClass::Vanguard,
            x: 0,
            y: 0,
            hp: 48,
            max_hp: 48,
            mana: 18,
            max_mana: 18,
            level: 1,
            xp: 0,
            xp_to_next: 65,
            gold: 0,
            attribute_points: 0,
            skill_points: 0,
            attributes: preset.attributes,
            skills: HashMap::new(),
            prodigies: None,
            prodigy_cooldowns: None,
            hotbar: vec![None, None, None, None, None],
            cooldowns: HashMap::new(),
            haste_steps: None,
            bonus_hp: None,
            statuses: vec![],
            equipped,
            inventory: vec![],
            kills: 0,
            turns_elapsed: 0,
            first_strike_ready: true,
            last_dir_x: Some(1),
            companion: None,
        }
    }

    fn town_with_population(population: i32) -> TownState {
        TownState {
            pieces_found: vec![],
            stall_item_found: false,
            supplies_found: vec![],
            blacksmith_complete: false,
            blacksmith_lot: None,
            wish_satisfied: false,
            wish_bonus_claimed: false,
            population,
            returns: 0,
        }
    }

    fn set_piece(name: &str, slot: ItemSlot, set_id: RelicSetId, stats: ItemStats) -> Item {
        Item {
            id: format!("itm_{name}"),
            name: name.into(),
            category: ItemCategory::Equipment,
            slot: Some(slot),
            consumable_kind: None,
            rarity: ItemRarity::Relic,
            glyph: "/".into(),
            level_req: 2,
            upgrade_level: 0,
            stats,
            affix: None,
            affix_label: None,
            set_id: Some(set_id),
            value: 175,
            description: String::new(),
            quantity: None,
            quest_tag: None,
        }
    }

    #[test]
    fn skill_table_has_all_12_skills() {
        let defs = skill_defs();
        assert_eq!(defs.len(), 12);
        assert_eq!(skill_def(SkillId::Cleave).name, "Sundering Cleave");
        assert_eq!(skill_def(SkillId::IronFortress).targeting, SkillTargeting::Self_);
        assert_eq!(skill_def(SkillId::FrostNova).targeting, SkillTargeting::Self_);
        assert_eq!(skill_def(SkillId::ArcaneEcho).kind, "passive");
        assert_eq!(prodigy_defs().len(), 6);
    }

    #[test]
    fn town_tier_bonus_formula() {
        assert_eq!(
            town_tier_bonuses_for_tier(0),
            TownTierBonuses { tier: 0, max_hp: 0, atk: 0, def: 0, spell_power: 0 }
        );
        assert_eq!(
            town_tier_bonuses_for_tier(1),
            TownTierBonuses { tier: 1, max_hp: 4, atk: 1, def: 0, spell_power: 1 }
        );
        assert_eq!(
            town_tier_bonuses_for_tier(2),
            TownTierBonuses { tier: 2, max_hp: 8, atk: 2, def: 1, spell_power: 2 }
        );
        assert_eq!(
            town_tier_bonuses_for_tier(3),
            TownTierBonuses { tier: 3, max_hp: 12, atk: 3, def: 2, spell_power: 3 }
        );
        // Via population thresholds [20, 35, 55] through town.rs.
        assert_eq!(get_town_tier_bonuses(Some(&town_with_population(19))).tier, 0);
        assert_eq!(get_town_tier_bonuses(Some(&town_with_population(36))).tier, 2);
        assert_eq!(get_town_tier_bonuses(Some(&town_with_population(55))).tier, 3);
        assert_eq!(get_town_tier_bonuses(None).tier, 0);
    }

    #[test]
    fn derived_stats_include_tier() {
        // Same expectations as ts/tests/town.test.ts: population 36 is
        // Tier 2, granting +8 Max HP, +2 ATK, +2 Spell Power, +1 DEF.
        let player = test_player();
        let town0 = town_with_population(12);
        let d0 = compute_derived_stats(&player, None, Some(&town0));
        let town2 = town_with_population(36);
        let bonuses = get_town_tier_bonuses(Some(&town2));
        assert_eq!(bonuses.tier, 2);
        let d2 = compute_derived_stats(&player, None, Some(&town2));
        assert_eq!(d2.max_hp - d0.max_hp, 8);
        assert_eq!(d2.atk - d0.atk, 2);
        assert_eq!(d2.spell_power - d0.spell_power, 2);
        assert_eq!(d2.def - d0.def, 1);
        // No town at all behaves as Tier 0.
        let d_none = compute_derived_stats(&player, None, None);
        assert_eq!(d_none.max_hp, d0.max_hp);
    }

    #[test]
    fn relic_two_piece_bonus() {
        // Starlight Outrider 2-piece: +4 ATK · +8% Crit · +6% Dodge · +10 Max HP.
        let mut player = test_player();
        player.equipped.clear();
        let base = compute_derived_stats(&player, None, None);

        let blade = set_piece(
            "Sylvanthorn Longblade",
            ItemSlot::Weapon,
            RelicSetId::StarlightOutrider,
            ItemStats {
                atk: Some(13),
                crit_chance: Some(9.0),
                dodge_chance: Some(4.0),
                ..Default::default()
            },
        );
        player
            .equipped
            .insert("weapon".to_string(), blade);
        let one = compute_derived_stats(&player, None, None);
        // One piece: only the item's own stats, no set bonus yet.
        assert_eq!(one.atk - base.atk, 13);
        assert_eq!(one.max_hp - base.max_hp, 0);

        let mantle = set_piece(
            "Starlight Outrider Mantle",
            ItemSlot::Armor,
            RelicSetId::StarlightOutrider,
            ItemStats {
                def: Some(9),
                dodge_chance: Some(9.0),
                max_hp: Some(18),
                spell_power: Some(4),
                ..Default::default()
            },
        );
        player.equipped.insert("armor".to_string(), mantle);
        let two = compute_derived_stats(&player, None, None);
        assert_eq!(get_equipped_set_counts(&player)[&RelicSetId::StarlightOutrider], 2);
        // Delta = the mantle's own stats + exactly the set bonus.
        assert_eq!(two.atk - one.atk, 4);
        assert_eq!(two.crit_chance - one.crit_chance, 8.0);
        assert_eq!(two.dodge_chance - one.dodge_chance, 9.0 + 6.0);
        assert_eq!(two.max_hp - one.max_hp, 18 + 10);
    }

    #[test]
    fn passive_skill_bonuses_in_derived_stats() {
        let mut player = test_player();
        player.equipped.clear();
        let base = compute_derived_stats(&player, None, None);
        player.skills.insert("bloodlust".to_string(), 2);
        player.skills.insert("evasive_instinct".to_string(), 1);
        player.skills.insert("siphon_soul".to_string(), 3);
        let d = compute_derived_stats(&player, None, None);
        assert_eq!(d.lifesteal - base.lifesteal, 7.0);
        assert_eq!(d.dodge_chance - base.dodge_chance, 8.0);
        assert_eq!(d.crit_chance - base.crit_chance, 5.0);
        assert_eq!(d.spell_power - base.spell_power, 11);
    }
}
