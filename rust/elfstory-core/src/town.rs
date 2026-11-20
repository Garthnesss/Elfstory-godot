//! Oakhaven town state, ported from `elfstoryreborn/src/game/town.ts`.
//!
//! Combined Model (approved Nov 17, 2026): Dark Cloud = bones (specific
//! pieces found in runs, a major completed by hand, a resident wish,
//! free relocation later), ActRaiser = life (population grows automatically
//! per return unless stalled, Population Tier grants modest global bonuses
//! and visible town growth, side-dungeon branches hold zone-specific
//! town supplies that drive Run-2+ zone choice).
//!
//! Rust divergences from TS (behaviour preserved):
//! - TS functions are pure and return the *same reference* when nothing
//!   changes; Rust versions take/return owned `TownState` values, so
//!   "no change" is observable as value equality instead of identity.
//! - TS `bankCarriedTownItems` / `splitUnbankedTownItems` are generic over
//!   any item shape with a `questTag`. The Rust port is concrete over
//!   `crate::types::Item` (whose `quest_tag` field is the same tag), which
//!   is the only item type the game has. No loot.ts constructors are
//!   needed by this module.

use crate::types::Item;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Blacksmith lots
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlacksmithLot {
    Pondside,
    Market,
    WestGate,
}

impl BlacksmithLot {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pondside => "pondside",
            Self::Market => "market",
            Self::WestGate => "west_gate",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pondside" => Some(Self::Pondside),
            "market" => Some(Self::Market),
            "west_gate" => Some(Self::WestGate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TownLotDef {
    pub id: BlacksmithLot,
    pub label: &'static str,
    pub blurb: &'static str,
    /// True when this lot satisfies Torin's stated wish.
    #[serde(rename = "satisfiesWish")]
    pub satisfies_wish: bool,
}

pub const BLACKSMITH_LOTS: [TownLotDef; 3] = [
    TownLotDef {
        id: BlacksmithLot::Pondside,
        label: "Pondside Lot",
        blurb: "Down by Oakhaven Pond, south-east of the green.",
        satisfies_wish: true,
    },
    TownLotDef {
        id: BlacksmithLot::Market,
        label: "Market Green Lot",
        blurb: "In the middle of everything, loud at all hours.",
        satisfies_wish: false,
    },
    TownLotDef {
        id: BlacksmithLot::WestGate,
        label: "West Gate Lot",
        blurb: "By the Silverwood gate, first thing delvers see.",
        satisfies_wish: false,
    },
];

/// Torin's wish, in his voice — shown BEFORE the lot is chosen.
pub const BLACKSMITH_WISH_TEXT: &str = "Build the new forge annex pondside. Damp air keeps the sparks honest — and a smith who breathes pond air does not rush a blade.";

// ---------------------------------------------------------------------------
// Blacksmith pieces + stall item
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BlacksmithPieceDef {
    /// questTag of the Item that represents this piece (see loot).
    pub tag: &'static str,
    pub name: &'static str,
}

pub const BLACKSMITH_PIECES: [BlacksmithPieceDef; 3] = [
    BlacksmithPieceDef { tag: "blacksmith_anvil", name: "Torin’s Lost Anvil" },
    BlacksmithPieceDef { tag: "blacksmith_bellows", name: "Great Forge Bellows" },
    BlacksmithPieceDef { tag: "blacksmith_sign", name: "The Smith’s Hanging Sign" },
];

/// The stall item: distinct from the 3 pieces. Per PROJECT_TRUTH.md:
/// "Town stalls freeze GROWTH only, cured by zone-specific discoveries."
/// Heartcoal cures the cold-forge stall so population growth resumes.
pub const STALL_ITEM_TAG: &str = "heartcoal";
pub const STALL_ITEM_NAME: &str = "Heartcoal";

// ---------------------------------------------------------------------------
// Regional town supplies
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TownSupplyTag {
    BarrowIron,
    MireTimber,
    FrostCoal,
}

impl TownSupplyTag {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BarrowIron => "barrow_iron",
            Self::MireTimber => "mire_timber",
            Self::FrostCoal => "frost_coal",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "barrow_iron" => Some(Self::BarrowIron),
            "mire_timber" => Some(Self::MireTimber),
            "frost_coal" => Some(Self::FrostCoal),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TownSupplyDef {
    pub tag: TownSupplyTag,
    pub name: &'static str,
    #[serde(rename = "zoneId")]
    pub zone_id: &'static str,
    #[serde(rename = "zoneLabel")]
    pub zone_label: &'static str,
    #[serde(rename = "unlockItemName")]
    pub unlock_item_name: &'static str,
    pub blurb: &'static str,
}

/// Zone-specific Oakhaven town supplies found on Floor 1 of the three
/// side-dungeon branches. Banking each supply grants +POP_PER_SUPPLY
/// population and unlocks a signature piece of gear in Torin's rebuilt
/// Blacksmith shop — driving Run-2+ zone selection ("I chose my zone
/// because of my town").
pub const TOWN_SUPPLIES: [TownSupplyDef; 3] = [
    TownSupplyDef {
        tag: TownSupplyTag::BarrowIron,
        name: "Whispering Barrow Iron",
        zone_id: "barrow_1",
        zone_label: "Whispering Barrow (Floor 1)",
        unlock_item_name: "Barrow-Iron Hauberk",
        blurb: "Grave-tempered iron ingots from Whispering Barrow (+4 Pop, unlocks Barrow-Iron Hauberk at the Blacksmith).",
    },
    TownSupplyDef {
        tag: TownSupplyTag::MireTimber,
        name: "Thornwick Ironwood Timber",
        zone_id: "mire_1",
        zone_label: "Fungal Hollow (Floor 1)",
        unlock_item_name: "Mire-Timber Greatbow",
        blurb: "Rot-proof ironwood beams from Fungal Hollow (+4 Pop, unlocks Mire-Timber Greatbow at the Blacksmith).",
    },
    TownSupplyDef {
        tag: TownSupplyTag::FrostCoal,
        name: "Frostpeak White-Coal",
        zone_id: "frost_1",
        zone_label: "Frostpeak Cavern (Floor 1)",
        unlock_item_name: "Frost-Quenched Greathelm",
        blurb: "Glacier-veined anthracite from Frostpeak Cavern (+4 Pop, unlocks Frost-Quenched Greathelm at the Blacksmith).",
    },
];

// ---------------------------------------------------------------------------
// Population constants
// ---------------------------------------------------------------------------

pub const BASE_POPULATION: i32 = 12;
pub const POP_PER_RETURN: i32 = 2;
pub const POP_ON_COMPLETE: i32 = 5;
pub const POP_PER_SUPPLY: i32 = 4;
pub const WISH_BONUS_GOLD: i32 = 75;

/// Population thresholds for Town Tier (the smooth ActRaiser bar).
pub const POP_TIER_THRESHOLDS: [i32; 3] = [20, 35, 55];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TownTierBonuses {
    pub tier: u32,
    #[serde(rename = "maxHp")]
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    #[serde(rename = "spellPower")]
    pub spell_power: i32,
}

// ---------------------------------------------------------------------------
// TownState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TownState {
    /// questTags of blacksmith pieces banked so far (deduped, order kept).
    #[serde(rename = "piecesFound")]
    pub pieces_found: Vec<String>,
    /// True once the Heartcoal stall item has been banked in Oakhaven.
    #[serde(rename = "stallItemFound")]
    pub stall_item_found: bool,
    /// Zone-specific side-dungeon supplies banked so far (deduped).
    #[serde(rename = "suppliesFound")]
    pub supplies_found: Vec<TownSupplyTag>,
    #[serde(rename = "blacksmithComplete")]
    pub blacksmith_complete: bool,
    /// Lot chosen at completion; null until complete.
    #[serde(rename = "blacksmithLot")]
    pub blacksmith_lot: Option<BlacksmithLot>,
    /// True when the current lot satisfies Torin's wish.
    #[serde(rename = "wishSatisfied")]
    pub wish_satisfied: bool,
    /// True once the wish gold bonus has been paid (never twice).
    #[serde(rename = "wishBonusClaimed")]
    pub wish_bonus_claimed: bool,
    pub population: i32,
    /// Count of dungeon -> Oakhaven returns that ticked population.
    pub returns: i32,
}

impl Default for TownState {
    fn default() -> Self {
        create_town_state()
    }
}

pub fn create_town_state() -> TownState {
    TownState {
        pieces_found: Vec::new(),
        stall_item_found: false,
        supplies_found: Vec::new(),
        blacksmith_complete: false,
        blacksmith_lot: None,
        wish_satisfied: false,
        wish_bonus_claimed: false,
        population: BASE_POPULATION,
        returns: 0,
    }
}

/// Partial / untrusted town state, as read from a save bundle before
/// normalization. Mirrors the TS `Partial<TownState>` input: every field
/// optional, piece/supply entries kept as raw JSON values so non-string
/// garbage can be dropped exactly like the TS filters do, and
/// population/returns kept as floats so the TS `Number.isFinite` + floor
/// behaviour can be reproduced.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TownStatePartial {
    pub pieces_found: Option<Vec<serde_json::Value>>,
    pub stall_item_found: Option<bool>,
    pub supplies_found: Option<Vec<serde_json::Value>>,
    pub blacksmith_complete: Option<bool>,
    pub blacksmith_lot: Option<String>,
    pub wish_satisfied: Option<bool>,
    pub wish_bonus_claimed: Option<bool>,
    pub population: Option<f64>,
    pub returns: Option<f64>,
}

/// Hydrate town state from a save bundle. Bundles written before Reborn
/// have no town field at all — they get the fresh default, so old saves
/// load unchanged in behaviour. Partial/garbage fields fall back per-key.
pub fn normalize_town_state(raw: Option<TownStatePartial>) -> TownState {
    let base = create_town_state();
    let Some(raw) = raw else { return base };

    let mut pieces: Vec<String> = Vec::new();
    if let Some(entries) = &raw.pieces_found {
        for entry in entries {
            if let Some(tag) = entry.as_str() {
                if BLACKSMITH_PIECES.iter().any(|p| p.tag == tag) && !pieces.iter().any(|t| t == tag) {
                    pieces.push(tag.to_string());
                }
            }
        }
    }
    let mut supplies: Vec<TownSupplyTag> = Vec::new();
    if let Some(entries) = &raw.supplies_found {
        for entry in entries {
            if let Some(tag) = entry.as_str().and_then(TownSupplyTag::parse) {
                if !supplies.contains(&tag) {
                    supplies.push(tag);
                }
            }
        }
    }
    let sane_count = |v: Option<f64>, fallback: i32| -> i32 {
        match v {
            Some(n) if n.is_finite() && n >= 0.0 => n.floor() as i32,
            _ => fallback,
        }
    };
    TownState {
        pieces_found: pieces,
        stall_item_found: raw.stall_item_found == Some(true),
        supplies_found: supplies,
        blacksmith_complete: raw.blacksmith_complete == Some(true),
        blacksmith_lot: raw.blacksmith_lot.as_deref().and_then(BlacksmithLot::parse),
        wish_satisfied: raw.wish_satisfied == Some(true),
        wish_bonus_claimed: raw.wish_bonus_claimed == Some(true),
        population: sane_count(raw.population, base.population),
        returns: sane_count(raw.returns, base.returns),
    }
}

/// Convenience for save loading: normalize straight from a raw JSON value
/// (the bundle's `town` field, or None for a pre-Reborn bundle).
pub fn normalize_town_state_json(raw: Option<&serde_json::Value>) -> TownState {
    match raw {
        Some(v) if v.is_object() => {
            normalize_town_state(serde_json::from_value::<TownStatePartial>(v.clone()).ok())
        }
        _ => create_town_state(),
    }
}

// ---------------------------------------------------------------------------
// Tag classification
// ---------------------------------------------------------------------------

/// The supply tag for `tag`, if it is one the town system tracks.
pub fn town_supply_tag(tag: &str) -> Option<TownSupplyTag> {
    TownSupplyTag::parse(tag).filter(|t| TOWN_SUPPLIES.iter().any(|s| s.tag == *t))
}

pub fn is_town_supply_tag(tag: Option<&str>) -> bool {
    tag.map_or(false, |t| town_supply_tag(t).is_some())
}

/// Is this questTag one the town system tracks?
pub fn is_town_item_tag(tag: Option<&str>) -> bool {
    let Some(tag) = tag else { return false };
    tag == STALL_ITEM_TAG
        || BLACKSMITH_PIECES.iter().any(|p| p.tag == tag)
        || is_town_supply_tag(Some(tag))
}

// ---------------------------------------------------------------------------
// Recording / banking / death split
// ---------------------------------------------------------------------------

/// Record a found town item, returning whether the state actually changed
/// (the Rust stand-in for the TS reference-identity check).
fn record_town_item_inner(mut state: TownState, tag: &str) -> (TownState, bool) {
    if tag == STALL_ITEM_TAG {
        if state.stall_item_found {
            return (state, false);
        }
        state.stall_item_found = true;
        return (state, true);
    }
    if BLACKSMITH_PIECES.iter().any(|p| p.tag == tag) {
        if state.pieces_found.iter().any(|t| t == tag) {
            return (state, false);
        }
        state.pieces_found.push(tag.to_string());
        return (state, true);
    }
    if let Some(supply) = town_supply_tag(tag) {
        if state.supplies_found.contains(&supply) {
            return (state, false);
        }
        state.supplies_found.push(supply);
        state.population += POP_PER_SUPPLY;
        return (state, true);
    }
    (state, false)
}

/// Record a found town item (piece, stall cure, or zone supply). Idempotent:
/// finding a second copy changes nothing, so re-generated floors can never
/// double count progress.
pub fn record_town_item(state: TownState, tag: &str) -> TownState {
    record_town_item_inner(state, tag).0
}

#[derive(Debug, Clone, PartialEq)]
pub struct BankTownItemsResult {
    pub state: TownState,
    pub remaining_inventory: Vec<Item>,
    pub banked_tags: Vec<String>,
}

/// Bank any carried town items from the player's inventory on an alive
/// return to Oakhaven: records each town item into TownState and removes
/// the banked town quest items from `inventory` so they never clutter the
/// player's Pack permanently. Town items are always consumed on return,
/// even when that tag was already recorded (its tag is still reported
/// once in `banked_tags`, matching the TS behaviour).
pub fn bank_carried_town_items(state: TownState, inventory: Vec<Item>) -> BankTownItemsResult {
    let mut next_state = state;
    let mut remaining_inventory: Vec<Item> = Vec::new();
    let mut banked_tags: Vec<String> = Vec::new();
    for item in inventory {
        if is_town_item_tag(item.quest_tag.as_deref()) {
            let tag = item.quest_tag.clone().unwrap_or_default();
            let (updated, changed) = record_town_item_inner(next_state, &tag);
            next_state = updated;
            if changed || !banked_tags.contains(&tag) {
                if !banked_tags.contains(&tag) {
                    banked_tags.push(tag);
                }
            }
            // Town items are always consumed into Oakhaven on return.
        } else {
            remaining_inventory.push(item);
        }
    }
    BankTownItemsResult { state: next_state, remaining_inventory, banked_tags }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplitUnbankedTownItemsResult {
    pub kept_inventory: Vec<Item>,
    pub dropped_town_items: Vec<Item>,
}

/// Split unbanked carried town items out of an inventory on death
/// ("Wake in Oakhaven"), so they can be dropped back onto the dungeon
/// floor where the player fell while non-town inventory is preserved.
pub fn split_unbanked_town_items(inventory: Vec<Item>) -> SplitUnbankedTownItemsResult {
    let mut kept_inventory: Vec<Item> = Vec::new();
    let mut dropped_town_items: Vec<Item> = Vec::new();
    for item in inventory {
        if is_town_item_tag(item.quest_tag.as_deref()) {
            dropped_town_items.push(item);
        } else {
            kept_inventory.push(item);
        }
    }
    SplitUnbankedTownItemsResult { kept_inventory, dropped_town_items }
}

// ---------------------------------------------------------------------------
// Blacksmith completion / relocation
// ---------------------------------------------------------------------------

pub fn pieces_count(state: &TownState) -> usize {
    BLACKSMITH_PIECES.iter().filter(|p| state.pieces_found.iter().any(|t| t == p.tag)).count()
}

/// The completion gate: all 3 pieces found AND the Heartcoal stall item
/// banked. (Stalls freeze growth generally, and the TS tests pin that the
/// Blacksmith cannot complete while the cold-forge stall is active.)
pub fn can_complete_blacksmith(state: &TownState) -> bool {
    !state.blacksmith_complete
        && state.stall_item_found
        && pieces_count(state) == BLACKSMITH_PIECES.len()
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompleteResult {
    pub state: TownState,
    /// True only when this call actually completed the Blacksmith.
    pub completed: bool,
    /// Gold bonus owed to the player by this completion (wish, once).
    pub wish_bonus_gold: i32,
}

/// Complete the Blacksmith on the chosen lot. A no-op (completed:false)
/// unless canCompleteBlacksmith — the gate cannot be bypassed. The wish
/// is satisfied only by the pondside lot; any lot completes the shop.
/// Population grows once, here, by POP_ON_COMPLETE (the smith's kin and
/// apprentices move in) — later calls never re-pay it.
pub fn complete_blacksmith(state: TownState, lot: BlacksmithLot) -> CompleteResult {
    if !can_complete_blacksmith(&state) {
        return CompleteResult { state, completed: false, wish_bonus_gold: 0 };
    }
    let lot_def = BLACKSMITH_LOTS.iter().find(|l| l.id == lot);
    let wish_satisfied = lot_def.map_or(false, |l| l.satisfies_wish);
    let wish_bonus_gold = if wish_satisfied && !state.wish_bonus_claimed { WISH_BONUS_GOLD } else { 0 };
    let mut next = state;
    next.blacksmith_complete = true;
    next.blacksmith_lot = Some(lot);
    next.wish_satisfied = wish_satisfied;
    next.wish_bonus_claimed = next.wish_bonus_claimed || wish_bonus_gold > 0;
    next.population += POP_ON_COMPLETE;
    CompleteResult { state: next, completed: true, wish_bonus_gold }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelocateResult {
    pub state: TownState,
    pub relocated: bool,
    pub wish_bonus_gold: i32,
}

/// Move the already-completed Blacksmith to a different lot for free
/// (PROJECT_TRUTH.md: "movable later for free, no permanent missables").
/// If moving to the pondside lot satisfies Torin's wish for the first
/// time, pays the one-time WISH_BONUS_GOLD bonus.
pub fn relocate_blacksmith(state: TownState, lot: BlacksmithLot) -> RelocateResult {
    if !state.blacksmith_complete || state.blacksmith_lot == Some(lot) {
        return RelocateResult { state, relocated: false, wish_bonus_gold: 0 };
    }
    let Some(lot_def) = BLACKSMITH_LOTS.iter().find(|l| l.id == lot) else {
        return RelocateResult { state, relocated: false, wish_bonus_gold: 0 };
    };
    let wish_satisfied = lot_def.satisfies_wish;
    let wish_bonus_gold = if wish_satisfied && !state.wish_bonus_claimed { WISH_BONUS_GOLD } else { 0 };
    let mut next = state;
    next.blacksmith_lot = Some(lot);
    next.wish_satisfied = wish_satisfied;
    next.wish_bonus_claimed = next.wish_bonus_claimed || wish_bonus_gold > 0;
    RelocateResult { state: next, relocated: true, wish_bonus_gold }
}

// ---------------------------------------------------------------------------
// Return tick + population tier
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TownReturnOptions {
    /// Kills + turns + loot activity earned during the delve; <= 0 blocks Turn-0 stair-hop farming.
    pub delve_progress: Option<i32>,
    /// True when returning via "Wake in Oakhaven" death respawn.
    pub is_death_return: bool,
    /// True to enforce the Heartcoal stall freeze on population growth.
    pub require_stall_cleared: bool,
}

/// The ActRaiser tick: one dungeon -> Oakhaven return. Population grows
/// by POP_PER_RETURN and the return is counted.
///
/// Guards:
/// - If `options.is_death_return` is true, no population tick occurs.
/// - If `options.delve_progress` is provided and <= 0, empty Turn-0 stair-hops
///   do not tick population.
/// - If `options.require_stall_cleared` is true and `!state.stall_item_found`,
///   population growth is frozen by the town stall (PROJECT_TRUTH.md:
///   "Town stalls freeze GROWTH only").
pub fn town_return_tick(state: TownState, options: TownReturnOptions) -> TownState {
    if options.is_death_return {
        return state;
    }
    if let Some(progress) = options.delve_progress {
        if progress <= 0 {
            return state;
        }
    }
    if options.require_stall_cleared && !state.stall_item_found {
        return state;
    }
    let mut next = state;
    next.population += POP_PER_RETURN;
    next.returns += 1;
    next
}

/// Town Tier: 0..3 from population alone (thresholds above).
/// This exact signature is the contract `skills.rs` depends on.
pub fn population_tier(town: &TownState) -> u32 {
    POP_TIER_THRESHOLDS.iter().filter(|t| town.population >= **t).count() as u32
}

/// Modest global combat bonuses granted by Oakhaven's Population Tier
/// (PROJECT_TRUTH.md: "Population Tier = a second, smaller progression
/// bar (town tiers gate zones/districts and grant modest global bonuses)").
/// Tier 1: +4 Max HP, +1 ATK, +1 Spell Power
/// Tier 2: +8 Max HP, +2 ATK, +2 Spell Power, +1 DEF
/// Tier 3: +12 Max HP, +3 ATK, +3 Spell Power, +2 DEF
pub fn get_town_tier_bonuses(state: Option<&TownState>) -> TownTierBonuses {
    let Some(state) = state else {
        return TownTierBonuses::default();
    };
    let tier = population_tier(state);
    if tier == 0 {
        return TownTierBonuses::default();
    }
    TownTierBonuses {
        tier,
        max_hp: tier as i32 * 4,
        atk: tier as i32,
        def: if tier >= 3 { 2 } else if tier >= 2 { 1 } else { 0 },
        spell_power: tier as i32,
    }
}

/// The gated shop item's display name (stock itself lives in npcs).
pub const BLACKSMITH_GATED_ITEM_NAME: &str = "Oakhaven-Forged Blade";

// ---------------------------------------------------------------------------
// Tests (ported from ts/tests/town.test.ts, town sections)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ItemCategory, ItemRarity, ItemStats};

    /// Minimal local stand-in for loot.ts `createQuestItem` — town logic
    /// only ever reads `quest_tag`, so tests construct items directly
    /// instead of depending on the loot module.
    fn quest_item(tag: &str) -> Item {
        Item {
            id: format!("quest:{tag}"),
            name: tag.to_string(),
            category: ItemCategory::Quest,
            slot: None,
            consumable_kind: None,
            rarity: ItemRarity::Common,
            glyph: "?".into(),
            level_req: 0,
            upgrade_level: 0,
            stats: ItemStats::default(),
            affix: None,
            affix_label: None,
            set_id: None,
            value: 0,
            description: String::new(),
            quantity: None,
            quest_tag: Some(tag.to_string()),
        }
    }

    fn plain_item(name: &str) -> Item {
        let mut it = quest_item(name);
        it.category = ItemCategory::Consumable;
        it.quest_tag = None;
        it
    }

    fn state_with_all_finds() -> TownState {
        let mut s = create_town_state();
        for p in BLACKSMITH_PIECES {
            s = record_town_item(s, p.tag);
        }
        record_town_item(s, STALL_ITEM_TAG)
    }

    #[test]
    fn fresh_state_starts_empty_at_base_population() {
        let s = create_town_state();
        assert_eq!(s.population, BASE_POPULATION);
        assert_eq!(pieces_count(&s), 0);
        assert!(!s.blacksmith_complete);
        assert!(!can_complete_blacksmith(&s));
        assert_eq!(population_tier(&s), 0);
    }

    #[test]
    fn piece_collection_dedupes_and_ignores_unrelated_tags() {
        let mut s = create_town_state();
        s = record_town_item(s, "blacksmith_anvil");
        s = record_town_item(s, "blacksmith_anvil");
        assert_eq!(pieces_count(&s), 1);
        let before = s.clone();
        s = record_town_item(s, "pale_nightbloom");
        assert_eq!(s, before);
        assert!(is_town_item_tag(Some("blacksmith_bellows")));
        assert!(is_town_item_tag(Some("heartcoal")));
        assert!(is_town_item_tag(Some("barrow_iron")));
        assert!(!is_town_item_tag(Some("pale_nightbloom")));
        assert!(!is_town_item_tag(None));
    }

    #[test]
    fn completion_gate_needs_all_pieces_and_heartcoal() {
        let mut pieces_only = create_town_state();
        for p in BLACKSMITH_PIECES {
            pieces_only = record_town_item(pieces_only, p.tag);
        }
        assert_eq!(pieces_count(&pieces_only), 3);
        assert!(!can_complete_blacksmith(&pieces_only)); // stalled

        let stall_only = record_town_item(create_town_state(), "heartcoal");
        assert!(!can_complete_blacksmith(&stall_only));

        assert!(can_complete_blacksmith(&state_with_all_finds()));
    }

    #[test]
    fn completion_on_pondside_satisfies_wish_and_pays_bonus_once() {
        let ready = state_with_all_finds();
        let res = complete_blacksmith(ready.clone(), BlacksmithLot::Pondside);
        assert!(res.completed);
        assert!(res.state.blacksmith_complete);
        assert_eq!(res.state.blacksmith_lot, Some(BlacksmithLot::Pondside));
        assert!(res.state.wish_satisfied);
        assert_eq!(res.wish_bonus_gold, WISH_BONUS_GOLD);
        assert_eq!(res.state.population, ready.population + POP_ON_COMPLETE);
        // Second completion is a no-op: no double population, no double bonus.
        let again = complete_blacksmith(res.state.clone(), BlacksmithLot::Pondside);
        assert!(!again.completed);
        assert_eq!(again.wish_bonus_gold, 0);
        assert_eq!(again.state.population, res.state.population);
    }

    #[test]
    fn relocate_free_and_pondside_bonus_once() {
        // Complete on market: shop completes, wish unsatisfied, no bonus.
        let res = complete_blacksmith(state_with_all_finds(), BlacksmithLot::Market);
        assert!(res.completed);
        assert!(!res.state.wish_satisfied);
        assert_eq!(res.wish_bonus_gold, 0);

        // Relocating before completion / to the same lot is a no-op.
        let noop = relocate_blacksmith(create_town_state(), BlacksmithLot::Pondside);
        assert!(!noop.relocated);
        let same = relocate_blacksmith(res.state.clone(), BlacksmithLot::Market);
        assert!(!same.relocated);
        assert_eq!(same.wish_bonus_gold, 0);

        // Free relocation to pondside satisfies the wish, pays +75g once.
        let moved = relocate_blacksmith(res.state, BlacksmithLot::Pondside);
        assert!(moved.relocated);
        assert_eq!(moved.state.blacksmith_lot, Some(BlacksmithLot::Pondside));
        assert!(moved.state.wish_satisfied);
        assert_eq!(moved.wish_bonus_gold, WISH_BONUS_GOLD);

        // Away and back never double-pays.
        let away = relocate_blacksmith(moved.state, BlacksmithLot::WestGate);
        assert!(away.relocated);
        assert!(!away.state.wish_satisfied);
        assert_eq!(away.wish_bonus_gold, 0);
        let back = relocate_blacksmith(away.state, BlacksmithLot::Pondside);
        assert!(back.relocated);
        assert!(back.state.wish_satisfied);
        assert_eq!(back.wish_bonus_gold, 0);
    }

    #[test]
    fn completion_refused_before_gate_is_met() {
        let res = complete_blacksmith(create_town_state(), BlacksmithLot::Pondside);
        assert!(!res.completed);
        assert!(!res.state.blacksmith_complete);
    }

    #[test]
    fn return_tick_guards() {
        let s0 = create_town_state();
        let s1 = town_return_tick(s0.clone(), TownReturnOptions::default());
        assert_eq!(s1.population, BASE_POPULATION + POP_PER_RETURN);
        assert_eq!(s1.returns, 1);
        let s2 = town_return_tick(s1, TownReturnOptions::default());
        assert_eq!(s2.population, BASE_POPULATION + POP_PER_RETURN * 2);
        assert_eq!(s2.returns, 2);

        // Guard 1: empty Turn-0 stair-hop (delveProgress <= 0).
        assert_eq!(
            town_return_tick(s0.clone(), TownReturnOptions { delve_progress: Some(0), ..Default::default() }),
            s0
        );
        // Guard 2: death return ("Wake in Oakhaven").
        assert_eq!(
            town_return_tick(
                s0.clone(),
                TownReturnOptions { delve_progress: Some(10), is_death_return: true, ..Default::default() }
            ),
            s0
        );
        // Guard 3: active stall freezes growth when requireStallCleared.
        assert_eq!(
            town_return_tick(
                s0.clone(),
                TownReturnOptions { delve_progress: Some(10), require_stall_cleared: true, ..Default::default() }
            ),
            s0
        );
        let unstalled = record_town_item(s0.clone(), "heartcoal");
        let ticked = town_return_tick(
            unstalled,
            TownReturnOptions { delve_progress: Some(10), require_stall_cleared: true, ..Default::default() },
        );
        assert_eq!(ticked.population, BASE_POPULATION + POP_PER_RETURN);

        // Tier rises at the documented threshold (20): base 12 + 4 returns = 20.
        let mut s = s0;
        for _ in 0..4 {
            s = town_return_tick(s, TownReturnOptions::default());
        }
        assert_eq!(s.population, 20);
        assert_eq!(population_tier(&s), 1);
    }

    #[test]
    fn death_drops_unbanked() {
        let potion = plain_item("potion");
        let anvil = quest_item("blacksmith_anvil");
        let coal = quest_item("heartcoal");
        let nightbloom = quest_item("pale_nightbloom");
        let split = split_unbanked_town_items(vec![potion, anvil, coal, nightbloom]);
        assert_eq!(split.kept_inventory.len(), 2);
        assert_eq!(split.dropped_town_items.len(), 2);
        let dropped_tags: Vec<Option<String>> =
            split.dropped_town_items.iter().map(|i| i.quest_tag.clone()).collect();
        assert_eq!(
            dropped_tags,
            vec![Some("blacksmith_anvil".to_string()), Some("heartcoal".to_string())]
        );
    }

    #[test]
    fn bank_on_alive_return() {
        let potion = plain_item("potion");
        let anvil = quest_item("blacksmith_anvil");
        let coal = quest_item("heartcoal");
        let nightbloom = quest_item("pale_nightbloom");
        let banked = bank_carried_town_items(create_town_state(), vec![potion, anvil, coal, nightbloom]);
        assert!(banked.state.pieces_found.iter().any(|t| t == "blacksmith_anvil"));
        assert!(banked.state.stall_item_found);
        assert_eq!(banked.remaining_inventory.len(), 2);
        assert!(banked
            .remaining_inventory
            .iter()
            .all(|i| !is_town_item_tag(i.quest_tag.as_deref())));
        assert_eq!(banked.banked_tags, vec!["blacksmith_anvil".to_string(), "heartcoal".to_string()]);
    }

    #[test]
    fn population_tier_bonuses() {
        // TS pins via computeDerivedStats at tier 2 (pop 36): +8 HP, +2 ATK,
        // +2 Spell Power, +1 DEF over tier 0. Here we pin the bonus table
        // itself (skills.rs applies it in compute_derived_stats).
        let tier2 = TownState { population: 36, ..create_town_state() };
        let b = get_town_tier_bonuses(Some(&tier2));
        assert_eq!(b.tier, 2);
        assert_eq!((b.max_hp, b.atk, b.spell_power, b.def), (8, 2, 2, 1));

        let tier1 = TownState { population: 20, ..create_town_state() };
        let b1 = get_town_tier_bonuses(Some(&tier1));
        assert_eq!((b1.tier, b1.max_hp, b1.atk, b1.spell_power, b1.def), (1, 4, 1, 1, 0));

        let tier3 = TownState { population: 55, ..create_town_state() };
        let b3 = get_town_tier_bonuses(Some(&tier3));
        assert_eq!((b3.tier, b3.max_hp, b3.atk, b3.spell_power, b3.def), (3, 12, 3, 3, 2));

        assert_eq!(get_town_tier_bonuses(Some(&create_town_state())), TownTierBonuses::default());
        assert_eq!(get_town_tier_bonuses(None), TownTierBonuses::default());

        // Threshold edges: 19/20, 34/35, 54/55.
        for (pop, tier) in [(19, 0), (20, 1), (34, 1), (35, 2), (54, 2), (55, 3), (99, 3)] {
            let s = TownState { population: pop, ..create_town_state() };
            assert_eq!(population_tier(&s), tier, "pop {pop}");
        }
    }

    #[test]
    fn supplies_grow_population_once_each() {
        let town = complete_blacksmith(state_with_all_finds(), BlacksmithLot::Pondside).state;
        let pop_before = town.population;
        let mut town = town;
        for supply in TOWN_SUPPLIES {
            town = record_town_item(town, supply.tag.as_str());
        }
        assert_eq!(town.population, pop_before + TOWN_SUPPLIES.len() as i32 * POP_PER_SUPPLY);
        assert_eq!(town.supplies_found.len(), 3);
        // Re-recording a supply never double-pays population.
        let again = record_town_item(town.clone(), "barrow_iron");
        assert_eq!(again.population, town.population);
    }

    #[test]
    fn normalize_defaults_old_and_partial_saves() {
        assert_eq!(normalize_town_state(None), create_town_state());
        assert_eq!(normalize_town_state_json(None), create_town_state());
        assert_eq!(
            normalize_town_state_json(Some(&serde_json::json!({ "player": { "name": "Garth" } }))),
            create_town_state()
        );
        let partial = normalize_town_state_json(Some(&serde_json::json!({
            "piecesFound": ["blacksmith_anvil", "not_a_piece", "blacksmith_anvil", 42],
            "suppliesFound": ["barrow_iron", "bogus"],
            "blacksmithLot": "moon",
            "population": 33.9,
            "returns": -2
        })));
        assert_eq!(partial.pieces_found, vec!["blacksmith_anvil".to_string()]);
        assert_eq!(partial.supplies_found, vec![TownSupplyTag::BarrowIron]);
        assert_eq!(partial.blacksmith_lot, None);
        assert_eq!(partial.population, 33);
        assert_eq!(partial.returns, 0);
        assert!(!partial.blacksmith_complete);
    }

    #[test]
    fn town_state_serde_round_trip_uses_ts_field_names() {
        let completed = complete_blacksmith(state_with_all_finds(), BlacksmithLot::Pondside).state;
        let json = serde_json::to_value(&completed).unwrap();
        assert_eq!(json["piecesFound"].as_array().unwrap().len(), 3);
        assert_eq!(json["stallItemFound"], serde_json::json!(true));
        assert_eq!(json["blacksmithLot"], serde_json::json!("pondside"));
        assert_eq!(json["wishBonusClaimed"], serde_json::json!(true));
        let back: TownState = serde_json::from_value(json).unwrap();
        assert_eq!(back, completed);
        // And normalize of the round-tripped state is identity.
        let via_partial = normalize_town_state_json(Some(&serde_json::to_value(&completed).unwrap()));
        assert_eq!(via_partial, completed);
    }
}
