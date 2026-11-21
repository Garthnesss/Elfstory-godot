//! Item generation and loot rules, ported from
//! `elfstoryreborn/src/game/loot.ts`.
//!
//! Rust divergences from TS (no global mutable state):
//! - TS draws from the module-global seeded stream (`random()` in rng.ts).
//!   Here every function that TS backed with `random()` takes
//!   `rng: &mut Rng` explicitly, in the same draw order as the TS source.
//! - TS item ids come from a module-global counter plus `Date.now()`
//!   (`itm_<base36 timestamp>_<counter>`, counter starting at 100). Here
//!   callers thread an [`ItemIdGen`] instead; ids are `itm_<counter>`.
//!   The timestamp component is dropped so ids stay deterministic under a
//!   seeded run — uniqueness within a run is preserved by the counter.
//! - Merchant stock itself (`createMerchantStock`, incl. the gated
//!   'Oakhaven-Forged Blade') lives in TS `npcs.ts`, not `loot.ts`; this
//!   module provides the item constructors that stock builder consumes.

use crate::rng::Rng;
use crate::run_rules::NORMAL_KILL_DROP_CHANCE;
use crate::types::{
    ConsumableKind, Item, ItemAffixId, ItemCategory, ItemRarity, ItemSlot, ItemStats, RelicSetId,
};
use std::collections::HashMap;

/// Deterministic item-id source replacing TS's global `lootCounter`.
#[derive(Debug, Clone)]
pub struct ItemIdGen {
    counter: u64,
}

impl ItemIdGen {
    /// TS counter starts at 100 and is incremented before each use.
    pub fn new() -> Self {
        Self { counter: 100 }
    }

    /// Counter value, for saves.
    pub fn counter(&self) -> u64 {
        self.counter
    }

    /// Resume from a saved counter.
    pub fn with_counter(counter: u64) -> Self {
        Self { counter }
    }

    pub fn next_id(&mut self) -> String {
        self.counter += 1;
        format!("itm_{}", self.counter)
    }
}

impl Default for ItemIdGen {
    fn default() -> Self {
        Self::new()
    }
}

/// The nine item stat fields, mirroring TS `keyof ItemStats`.
/// Needed because TS indexes `ItemStats` dynamically (slot bases,
/// prefixes, suffixes); Rust stats are a fixed struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatKind {
    Atk,
    Def,
    SpellPower,
    MaxHp,
    MaxMana,
    CritChance,
    DodgeChance,
    Lifesteal,
    Thorns,
}

impl StatKind {
    /// Current value as f64 (0 when unset), matching TS `stats[k] ?? 0`.
    pub fn get(&self, stats: &ItemStats) -> f64 {
        match self {
            Self::Atk => stats.atk.unwrap_or(0) as f64,
            Self::Def => stats.def.unwrap_or(0) as f64,
            Self::SpellPower => stats.spell_power.unwrap_or(0) as f64,
            Self::MaxHp => stats.max_hp.unwrap_or(0) as f64,
            Self::MaxMana => stats.max_mana.unwrap_or(0) as f64,
            Self::CritChance => stats.crit_chance.unwrap_or(0.0),
            Self::DodgeChance => stats.dodge_chance.unwrap_or(0.0),
            Self::Lifesteal => stats.lifesteal.unwrap_or(0.0),
            Self::Thorns => stats.thorns.unwrap_or(0) as f64,
        }
    }

    fn set(&self, stats: &mut ItemStats, value: f64) {
        match self {
            Self::Atk => stats.atk = Some(value as i32),
            Self::Def => stats.def = Some(value as i32),
            Self::SpellPower => stats.spell_power = Some(value as i32),
            Self::MaxHp => stats.max_hp = Some(value as i32),
            Self::MaxMana => stats.max_mana = Some(value as i32),
            Self::CritChance => stats.crit_chance = Some(value),
            Self::DodgeChance => stats.dodge_chance = Some(value),
            Self::Lifesteal => stats.lifesteal = Some(value),
            Self::Thorns => stats.thorns = Some(value as i32),
        }
    }

    /// TS `addStat`: `stats[stat] = (stats[stat] ?? 0) + max(1, round(amount))`.
    fn add(&self, stats: &mut ItemStats, amount: f64) {
        let inc = amount.round().max(1.0);
        self.set(stats, self.get(stats) + inc);
    }
}

/// One relic set's display data, mirroring TS `RELIC_SETS` values.
#[derive(Debug, Clone, PartialEq)]
pub struct RelicSetDef {
    pub name: String,
    pub bonus_label: String,
    pub summary: String,
    pub pieces: Vec<String>,
}

/// TS `RELIC_SETS`, keyed by set id.
pub fn relic_sets() -> HashMap<RelicSetId, RelicSetDef> {
    let mut m = HashMap::new();
    m.insert(
        RelicSetId::StarlightOutrider,
        RelicSetDef {
            name: "Starlight Outrider".into(),
            bonus_label: "+4 ATK · +8% Crit · +6% Dodge · +10 Max HP".into(),
            summary: "2-Piece Set: Elven ranger regalia woven with silver-star thread.".into(),
            pieces: vec![
                "Sylvanthorn Longblade".into(),
                "Starlight Outrider Mantle".into(),
            ],
        },
    );
    m.insert(
        RelicSetId::Cinderforged,
        RelicSetDef {
            name: "Cinderforged Covenant".into(),
            bonus_label: "+4 DEF · +6 Spell · +6 Thorns · +15 Max HP".into(),
            summary: "2-Piece Set: Obsidian-plate & emberheart wards tempered in deep vaults."
                .into(),
            pieces: vec!["Cinderforged Crown".into(), "Emberheart Signet".into()],
        },
    );
    m.insert(
        RelicSetId::Thornbound,
        RelicSetDef {
            name: "Thornbound Covenant".into(),
            bonus_label: "+3 ATK · +4 Thorns · +3% Lifesteal · +12 Max HP".into(),
            summary: "2-Piece Set: Fen-root regalia grown, not forged, in the Fungal Hollow."
                .into(),
            pieces: vec!["Thornbound Circlet".into(), "Fenroot Talisman".into()],
        },
    );
    m.insert(
        RelicSetId::Rimebound,
        RelicSetDef {
            name: "Rimebound Regalia".into(),
            bonus_label: "+5 DEF · +8% Dodge · +10 Max Mana · +8 Max HP".into(),
            summary: "2-Piece Set: Glacier-heart plate sealed in Frostpeak rime.".into(),
            pieces: vec!["Glacierheart Pendant".into(), "Rimebound Plate".into()],
        },
    );
    m
}

/// TS `RARITY_COLOR`.
pub fn rarity_color(rarity: ItemRarity) -> &'static str {
    match rarity {
        ItemRarity::Common => "#C2B9A7",
        ItemRarity::Uncommon => "#4EA885",
        ItemRarity::Rare => "#5B8DEF",
        ItemRarity::Epic => "#C084FC",
        ItemRarity::Relic => "#F59E0B",
    }
}

struct SlotBase {
    name: &'static str,
    glyph: &'static str,
    primary: StatKind,
    secondary: Option<StatKind>,
}

/// TS `SLOT_BASES`.
fn slot_bases(slot: ItemSlot) -> Vec<SlotBase> {
    use StatKind::*;
    match slot {
        ItemSlot::Weapon => vec![
            SlotBase { name: "Iron Longsword", glyph: "/", primary: Atk, secondary: None },
            SlotBase { name: "Obsidian Dagger", glyph: "†", primary: Atk, secondary: Some(CritChance) },
            SlotBase { name: "Runed Ash Staff", glyph: "⚚", primary: SpellPower, secondary: Some(MaxMana) },
            SlotBase { name: "Halberd", glyph: "↑", primary: Atk, secondary: Some(Def) },
            SlotBase { name: "Cinder Falchion", glyph: "⚔", primary: Atk, secondary: Some(SpellPower) },
        ],
        ItemSlot::Armor => vec![
            SlotBase { name: "Brigandine Coat", glyph: "⛨", primary: Def, secondary: Some(MaxHp) },
            SlotBase { name: "Shadowweave Vest", glyph: "▒", primary: Def, secondary: Some(DodgeChance) },
            SlotBase { name: "Plate Cuirass", glyph: "☗", primary: Def, secondary: Some(Thorns) },
            SlotBase { name: "Archivist Robes", glyph: "☥", primary: SpellPower, secondary: Some(MaxMana) },
        ],
        ItemSlot::Helm => vec![
            SlotBase { name: "Barred Sallet", glyph: "⍾", primary: Def, secondary: Some(MaxHp) },
            SlotBase { name: "Raven Hood", glyph: "∩", primary: DodgeChance, secondary: Some(CritChance) },
            SlotBase { name: "Rune Circlet", glyph: "⍟", primary: SpellPower, secondary: Some(MaxMana) },
        ],
        ItemSlot::Ring => vec![
            SlotBase { name: "Signet Band", glyph: "°", primary: Atk, secondary: Some(CritChance) },
            SlotBase { name: "Opal Loop", glyph: "◦", primary: SpellPower, secondary: Some(MaxMana) },
            SlotBase { name: "Bastion Ring", glyph: "○", primary: Def, secondary: Some(MaxHp) },
        ],
        ItemSlot::Amulet => vec![
            SlotBase { name: "Amber Pendant", glyph: "◈", primary: MaxHp, secondary: Some(Def) },
            SlotBase { name: "Aether Locket", glyph: "❖", primary: MaxMana, secondary: Some(SpellPower) },
            SlotBase { name: "Tooth Talisman", glyph: "◊", primary: CritChance, secondary: Some(Lifesteal) },
        ],
    }
}

struct AffixWord {
    name: &'static str,
    stat: StatKind,
    mult: f64,
}

/// TS `PREFIXES`.
fn prefixes() -> Vec<AffixWord> {
    use StatKind::*;
    vec![
        AffixWord { name: "Keen", stat: Atk, mult: 1.2 },
        AffixWord { name: "Tempered", stat: Def, mult: 1.2 },
        AffixWord { name: "Aetherial", stat: SpellPower, mult: 1.25 },
        AffixWord { name: "Stalwart", stat: MaxHp, mult: 2.5 },
        AffixWord { name: "Whispering", stat: DodgeChance, mult: 1.4 },
        AffixWord { name: "Wicked", stat: CritChance, mult: 1.5 },
        AffixWord { name: "Vampiric", stat: Lifesteal, mult: 1.0 },
        AffixWord { name: "Barbed", stat: Thorns, mult: 1.3 },
    ]
}

/// TS `SUFFIXES`.
fn suffixes() -> Vec<AffixWord> {
    use StatKind::*;
    vec![
        AffixWord { name: "of Ash", stat: SpellPower, mult: 1.0 },
        AffixWord { name: "of the Bastion", stat: Def, mult: 1.0 },
        AffixWord { name: "of Precision", stat: CritChance, mult: 1.3 },
        AffixWord { name: "of Resolve", stat: MaxHp, mult: 2.2 },
        AffixWord { name: "of Echoes", stat: MaxMana, mult: 1.8 },
        AffixWord { name: "of Ruin", stat: Atk, mult: 1.1 },
        AffixWord { name: "of Mists", stat: DodgeChance, mult: 1.2 },
    ]
}

/// TS `AFFIX_POOL`.
fn affix_pool() -> Vec<(ItemAffixId, &'static str)> {
    vec![
        (ItemAffixId::BleedOnCrit, "Passive: Critical hits inflict Bleed (2t)"),
        (ItemAffixId::ManaOnKill, "Passive: Slaying a foe restores 4 Mana"),
        (ItemAffixId::ThornsBurst, "Passive: Reflects +4 extra damage when hit"),
        (ItemAffixId::FirstStrike, "Passive: First hit in each skirmish deals +35% dmg"),
        (ItemAffixId::ExecuteBonus, "Passive: +25% damage to foes below 40% HP"),
        (ItemAffixId::ChillOnHit, "Passive: Strikes have 30% chance to Chill target"),
        (ItemAffixId::VampiricTouch, "Passive: Heals 3 HP on every Critical Hit"),
        (ItemAffixId::ArcaneWard, "Passive: Start floor with Shielded (+4 DEF, 4t)"),
    ]
}

fn rarity_order(rarity: ItemRarity) -> usize {
    match rarity {
        ItemRarity::Common => 0,
        ItemRarity::Uncommon => 1,
        ItemRarity::Rare => 2,
        ItemRarity::Epic => 3,
        ItemRarity::Relic => 4,
    }
}

fn rarity_scale(rarity: ItemRarity) -> f64 {
    match rarity {
        ItemRarity::Common => 1.0,
        ItemRarity::Uncommon => 1.35,
        ItemRarity::Rare => 1.75,
        ItemRarity::Epic => 2.2,
        ItemRarity::Relic => 2.7,
    }
}

/// Loot curve (retuned Nov 16, 2026): depth term is ×5, capped at +50.
pub const RARITY_DEPTH_STEP: i32 = 5;
pub const RARITY_DEPTH_CAP: i32 = 50;

/// The depth bonus added to a rarity roll: `min(50, max(0, depth) * 5)`.
pub fn rarity_depth_bonus(depth: i32) -> i32 {
    (depth.max(0) * RARITY_DEPTH_STEP).min(RARITY_DEPTH_CAP)
}

/// Pure rarity thresholds, split out so tests can pin the exact
/// boundaries (TS tests drive this by swapping `Math.random`).
/// `roll` is the full roll value (`random × 100 + depth bonus`).
/// Thresholds are strict: Rare > 75, Epic > 104, Relic > 125,
/// Uncommon > 42. A Relic pick survives only for premium callers
/// (`force_min_rarity` of Epic or Relic); ordinary drops cap at Epic.
pub fn rarity_from_roll(roll: f64, force_min_rarity: Option<ItemRarity>) -> ItemRarity {
    let mut picked = if roll > 125.0 {
        ItemRarity::Relic
    } else if roll > 104.0 {
        ItemRarity::Epic
    } else if roll > 75.0 {
        ItemRarity::Rare
    } else if roll > 42.0 {
        ItemRarity::Uncommon
    } else {
        ItemRarity::Common
    };

    let premium = matches!(force_min_rarity, Some(ItemRarity::Epic) | Some(ItemRarity::Relic));
    if picked == ItemRarity::Relic && !premium {
        picked = ItemRarity::Epic;
    }

    if let Some(min) = force_min_rarity {
        if rarity_order(picked) < rarity_order(min) {
            picked = min;
        }
    }
    picked
}

/// TS `rollRarity`. Consumes exactly one draw from `rng`.
pub fn roll_rarity(
    rng: &mut Rng,
    depth: i32,
    force_min_rarity: Option<ItemRarity>,
) -> ItemRarity {
    let roll = rng.next_f32() as f64 * 100.0 + rarity_depth_bonus(depth) as f64;
    rarity_from_roll(roll, force_min_rarity)
}

/// Options for [`generate_procedural_item`] (TS's optional options bag).
#[derive(Debug, Clone, Copy, Default)]
pub struct ProceduralOptions {
    pub slot: Option<ItemSlot>,
    pub min_rarity: Option<ItemRarity>,
}

/// TS `scaleForStat` — takes the (possibly fractional) base power,
/// exactly as TS passes `power * 0.55` for secondary stats.
fn scale_for_stat(stat: StatKind, base_power: f64) -> f64 {
    match stat {
        StatKind::MaxHp => base_power * 3.0,
        StatKind::MaxMana => base_power * 2.0,
        StatKind::CritChance | StatKind::DodgeChance => {
            (base_power * 1.4).round().clamp(2.0, 18.0)
        }
        StatKind::Lifesteal => (base_power * 0.7).round().clamp(2.0, 10.0),
        _ => base_power,
    }
}

/// TS `generateProceduralItem`. Draw order matches TS: slot (when not
/// forced), rarity roll, base pick, prefix (non-Common), suffix
/// (Rare+), affix (Rare+).
pub fn generate_procedural_item(
    gen: &mut ItemIdGen,
    rng: &mut Rng,
    depth: i32,
    options: ProceduralOptions,
) -> Item {
    const SLOTS: [ItemSlot; 5] = [
        ItemSlot::Weapon,
        ItemSlot::Armor,
        ItemSlot::Helm,
        ItemSlot::Ring,
        ItemSlot::Amulet,
    ];
    let slot = options.slot.unwrap_or_else(|| SLOTS[rng.range_usize(SLOTS.len())]);
    let rarity = roll_rarity(rng, depth, options.min_rarity);
    let scale = rarity_scale(rarity);

    let base_list = slot_bases(slot);
    let base = &base_list[rng.range_usize(base_list.len())];
    let power = ((1.8 + depth as f64 * 1.1) * scale).round().max(1.0) as i32;

    let mut stats = ItemStats::default();
    base.primary.add(&mut stats, scale_for_stat(base.primary, power as f64));
    if let Some(secondary) = base.secondary {
        if rarity != ItemRarity::Common {
            secondary.add(&mut stats, scale_for_stat(secondary, power as f64 * 0.55));
        }
    }

    let mut name = base.name.to_string();
    if rarity != ItemRarity::Common {
        let prefix_list = prefixes();
        let prefix = &prefix_list[rng.range_usize(prefix_list.len())];
        name = format!("{} {}", prefix.name, name);
        prefix
            .stat
            .add(&mut stats, power as f64 * 0.65 * prefix.mult);
    }

    if matches!(rarity, ItemRarity::Rare | ItemRarity::Epic | ItemRarity::Relic) {
        let suffix_list = suffixes();
        let suffix = &suffix_list[rng.range_usize(suffix_list.len())];
        name = format!("{} {}", name, suffix.name);
        suffix
            .stat
            .add(&mut stats, power as f64 * 0.6 * suffix.mult);
    }

    let (affix, affix_label) =
        if matches!(rarity, ItemRarity::Rare | ItemRarity::Epic | ItemRarity::Relic) {
            let pool = affix_pool();
            let picked = &pool[rng.range_usize(pool.len())];
            (Some(picked.0), Some(picked.1.to_string()))
        } else {
            (None, None)
        };

    let value = (14.0 * scale + depth as f64 * 10.0).round() as i32;

    let origin_label = if depth >= 6 {
        "Endless Abyss"
    } else {
        match depth {
            1 => "Moss-Sunken Reliquary",
            2 => "Frost-Bound Ossuary",
            3 => "Ember Vaults",
            4 => "Inquisitor Crucible",
            5 => "Obsidian Sanctum",
            _ => "outer catacombs",
        }
    };

    Item {
        id: gen.next_id(),
        name,
        category: ItemCategory::Equipment,
        slot: Some(slot),
        consumable_kind: None,
        rarity,
        glyph: base.glyph.to_string(),
        level_req: depth.max(1),
        upgrade_level: 0,
        stats,
        affix,
        affix_label,
        set_id: None,
        value,
        description: format!("{} {} recovered from the {}.", rarity, slot, origin_label),
        quantity: None,
        quest_tag: None,
    }
}

fn blank_item(gen: &mut ItemIdGen, name: &str, category: ItemCategory, rarity: ItemRarity, glyph: &str, level_req: i32, value: i32, description: &str) -> Item {
    Item {
        id: gen.next_id(),
        name: name.to_string(),
        category,
        slot: None,
        consumable_kind: None,
        rarity,
        glyph: glyph.to_string(),
        level_req,
        upgrade_level: 0,
        stats: ItemStats::default(),
        affix: None,
        affix_label: None,
        set_id: None,
        value,
        description: description.to_string(),
        quantity: None,
        quest_tag: None,
    }
}

/// TS `createConsumable`.
pub fn create_consumable(gen: &mut ItemIdGen, kind: ConsumableKind, quantity: i32) -> Item {
    let (name, rarity, glyph, value, description) = match kind {
        ConsumableKind::HealPotion => (
            "Crimson Draught",
            ItemRarity::Common,
            "!",
            18,
            "Restores at least 35% of Max HP immediately without ending your combat poise.",
        ),
        ConsumableKind::ManaPotion => (
            "Aether Vial",
            ItemRarity::Uncommon,
            "¡",
            22,
            "Restores at least 40% of Max Mana and clears minor fatigue.",
        ),
        ConsumableKind::ScrollBlink => (
            "Scroll of Phase",
            ItemRarity::Rare,
            "?",
            32,
            "Instantly blinks you away to a safe nearby floor tile.",
        ),
        ConsumableKind::WardTalisman => (
            "Warden Charm",
            ItemRarity::Uncommon,
            "◊",
            26,
            "Grants Shielded (+7 DEF for 4 turns).",
        ),
        ConsumableKind::ThrowingKnife => (
            "Serrated Dart",
            ItemRarity::Common,
            "-",
            14,
            "Strikes the nearest visible foe for 14 damage + Bleed.",
        ),
    };
    let mut item = blank_item(gen, name, ItemCategory::Consumable, rarity, glyph, 1, value, description);
    item.consumable_kind = Some(kind);
    item.quantity = Some(quantity);
    item
}

/// TS `createStarterItem` (unknown tags get the Wanderer sabre, as in TS).
pub fn create_starter_item(gen: &mut ItemIdGen, tag: &str) -> Item {
    let (name, glyph, stats, description) = match tag {
        "starter_vanguard" => (
            "Recruit Broadsword",
            "/",
            ItemStats { atk: Some(5), def: Some(1), ..Default::default() },
            "Balanced steel issued to Oakhaven gate-guards.",
        ),
        "starter_shadow" => (
            "Twin-Edged Dirk",
            "†",
            ItemStats {
                atk: Some(4),
                crit_chance: Some(7.0),
                dodge_chance: Some(3.0),
                ..Default::default()
            },
            "Light blackened blade favoring vital gaps.",
        ),
        "starter_rune" => (
            "Charred Oak Focus",
            "⚚",
            ItemStats {
                atk: Some(2),
                spell_power: Some(6),
                max_mana: Some(6),
                ..Default::default()
            },
            "Etched with silver cinder-glyphs to channel ember spells.",
        ),
        _ => (
            "Traveler's Sabre",
            "/",
            ItemStats {
                atk: Some(4),
                spell_power: Some(3),
                crit_chance: Some(4.0),
                ..Default::default()
            },
            "Versatile curved blade carried across border roads.",
        ),
    };
    let mut item = blank_item(
        gen,
        name,
        ItemCategory::Equipment,
        ItemRarity::Uncommon,
        glyph,
        1,
        24,
        description,
    );
    item.slot = Some(ItemSlot::Weapon);
    item.stats = stats;
    item
}

/// TS `createNamedRelic`. Unknown tags fall back to
/// `generateProceduralItem(3, { minRarity: 'Rare' })`, as in TS.
pub fn create_named_relic(gen: &mut ItemIdGen, rng: &mut Rng, tag: &str) -> Item {
    // (name, slot, rarity, glyph, level_req, stats, affix, affix_label, set_id, value, description)
    let spec: Option<(
        &str,
        ItemSlot,
        ItemRarity,
        &str,
        i32,
        ItemStats,
        Option<ItemAffixId>,
        Option<&str>,
        Option<RelicSetId>,
        i32,
        &str,
    )> = match tag {
        "vex_whisperblade" => Some((
            "Vex's Whisperblade", ItemSlot::Weapon, ItemRarity::Relic, "†", 2,
            ItemStats { atk: Some(12), crit_chance: Some(12.0), dodge_chance: Some(6.0), ..Default::default() },
            Some(ItemAffixId::BleedOnCrit), Some("Passive: Critical hits inflict Bleed (2t)"), None, 145,
            "Smuggler heirloom forged from silent star-iron.",
        )),
        "warden_oathplate" => Some((
            "Warden's Oathplate", ItemSlot::Armor, ItemRarity::Relic, "☗", 2,
            ItemStats { def: Some(11), max_hp: Some(22), thorns: Some(5), ..Default::default() },
            Some(ItemAffixId::ArcaneWard), Some("Passive: Start floor with Shielded (+4 DEF, 4t)"), None, 150,
            "Ceremonial Highspire plate tempered against crypt hexes.",
        )),
        "chime_first_ash" => Some((
            "Chime of First Ash", ItemSlot::Amulet, ItemRarity::Relic, "❖", 3,
            ItemStats { spell_power: Some(12), max_mana: Some(16), crit_chance: Some(6.0), ..Default::default() },
            Some(ItemAffixId::ManaOnKill), Some("Passive: Slaying a foe restores 4 Mana"), None, 185,
            "Resonates softly whenever an arcane glyph is spoken.",
        )),
        "citadel_halberd" => Some((
            "Magistrate Greatspear", ItemSlot::Weapon, ItemRarity::Epic, "↑", 3,
            ItemStats { atk: Some(15), def: Some(4), max_hp: Some(12), ..Default::default() },
            Some(ItemAffixId::ExecuteBonus), Some("Passive: +25% damage to foes below 40% HP"), None, 160,
            "Presented by Magistrate Corvus to Citadel champions.",
        )),
        "sylvan_longblade" => Some((
            "Sylvanthorn Longblade", ItemSlot::Weapon, ItemRarity::Relic, "/", 2,
            ItemStats { atk: Some(13), crit_chance: Some(9.0), dodge_chance: Some(4.0), ..Default::default() },
            Some(ItemAffixId::FirstStrike), Some("Passive: First hit in each skirmish deals +35% dmg"),
            Some(RelicSetId::StarlightOutrider), 175,
            "Elven star-silver longblade carried by Highspire’s ancient Outriders.",
        )),
        "starlight_mantle" => Some((
            "Starlight Outrider Mantle", ItemSlot::Armor, ItemRarity::Relic, "▒", 2,
            ItemStats { def: Some(9), dodge_chance: Some(9.0), max_hp: Some(18), spell_power: Some(4), ..Default::default() },
            Some(ItemAffixId::ChillOnHit), Some("Passive: Strikes have 30% chance to Chill target"),
            Some(RelicSetId::StarlightOutrider), 175,
            "Midnight-green elven cloak threaded with shimmer-silver wards.",
        )),
        "cinder_crown" => Some((
            "Cinderforged Crown", ItemSlot::Helm, ItemRarity::Relic, "⍟", 3,
            ItemStats { def: Some(8), spell_power: Some(9), max_hp: Some(16), max_mana: Some(10), ..Default::default() },
            Some(ItemAffixId::ArcaneWard), Some("Passive: Start floor with Shielded (+4 DEF, 4t)"),
            Some(RelicSetId::Cinderforged), 190,
            "Smelted in the Ember Vaults, humming with defiant heat.",
        )),
        "emberheart_signet" => Some((
            "Emberheart Signet", ItemSlot::Ring, ItemRarity::Relic, "°", 3,
            ItemStats { atk: Some(6), spell_power: Some(8), thorns: Some(6), crit_chance: Some(6.0), ..Default::default() },
            Some(ItemAffixId::VampiricTouch), Some("Passive: Heals 3 HP on every Critical Hit"),
            Some(RelicSetId::Cinderforged), 190,
            "A glowing ruby band forged from Sovereign furnace embers.",
        )),
        "thornbound_circlet" => Some((
            "Thornbound Circlet", ItemSlot::Helm, ItemRarity::Relic, "⍟", 3,
            ItemStats { atk: Some(5), thorns: Some(6), max_hp: Some(14), def: Some(4), ..Default::default() },
            Some(ItemAffixId::ThornsBurst), Some("Passive: Thorns reflect +6 bonus damage"),
            Some(RelicSetId::Thornbound), 180,
            "A living circlet of fen-thorn, still slowly growing around its wearer.",
        )),
        "fenroot_talisman" => Some((
            "Fenroot Talisman", ItemSlot::Amulet, ItemRarity::Relic, "◊", 3,
            ItemStats { lifesteal: Some(4.0), max_hp: Some(16), spell_power: Some(5), def: Some(3), ..Default::default() },
            Some(ItemAffixId::VampiricTouch), Some("Passive: Heals 3 HP on every Critical Hit"),
            Some(RelicSetId::Thornbound), 180,
            "Root-heart of a Hollow bloom-mother, pulsing with stolen sap.",
        )),
        "glacierheart_pendant" => Some((
            "Glacierheart Pendant", ItemSlot::Amulet, ItemRarity::Relic, "❖", 4,
            ItemStats { def: Some(5), max_mana: Some(18), dodge_chance: Some(6.0), spell_power: Some(6), ..Default::default() },
            Some(ItemAffixId::ArcaneWard), Some("Passive: Start floor with Shielded (+4 DEF, 4t)"),
            Some(RelicSetId::Rimebound), 195,
            "A shard of the first Frostpeak glacier, cold enough to slow arrows.",
        )),
        "rimebound_plate" => Some((
            "Rimebound Plate", ItemSlot::Armor, ItemRarity::Relic, "☗", 4,
            ItemStats { def: Some(12), dodge_chance: Some(5.0), max_hp: Some(16), ..Default::default() },
            Some(ItemAffixId::ChillOnHit), Some("Passive: Strikes have 30% chance to Chill target"),
            Some(RelicSetId::Rimebound), 195,
            "Caldera-forged plate quenched in glacier melt; frost blooms across it nightly.",
        )),
        "surveyor_compass" => Some((
            "Surveyor’s Star-Compass", ItemSlot::Amulet, ItemRarity::Epic, "✦", 2,
            ItemStats { spell_power: Some(7), max_mana: Some(12), dodge_chance: Some(6.0), crit_chance: Some(5.0), ..Default::default() },
            Some(ItemAffixId::FirstStrike), Some("Passive: First Strike on floor deals +30% damage"), None, 150,
            "Rowan’s own survey compass; its needle points at the nearest unread stele.",
        )),
        "silverbridge_band" => Some((
            "Wayfarer’s Silverbridge Band", ItemSlot::Ring, ItemRarity::Rare, "°", 1,
            ItemStats { atk: Some(3), def: Some(2), dodge_chance: Some(5.0), max_mana: Some(6), ..Default::default() },
            Some(ItemAffixId::FirstStrike), Some("Passive: First Strike on floor deals +30% damage"), None, 95,
            "Engraved with compass notches worn smooth on Silverwood patrol roads.",
        )),
        "sporeguard_amulet" => Some((
            "Thornwick Sporeguard Charm", ItemSlot::Amulet, ItemRarity::Epic, "♀", 2,
            ItemStats { def: Some(4), max_hp: Some(18), max_mana: Some(10), thorns: Some(5), ..Default::default() },
            Some(ItemAffixId::ManaOnKill), Some("Passive: Restore +3 Mana on kill"), None, 145,
            "Amber-resin fen talisman woven by Elder Sylva to ward off bog rot.",
        )),
        "morvath_shroud" => Some((
            "Barrow-Wight Shroud", ItemSlot::Helm, ItemRarity::Epic, "^", 2,
            ItemStats { def: Some(5), spell_power: Some(7), crit_chance: Some(7.0), dodge_chance: Some(4.0), ..Default::default() },
            Some(ItemAffixId::ChillOnHit), Some("Passive: Hits have 25% chance to Chill foes"), None, 155,
            "Cold funerary circlet claimed from Arch-Wight Morvath’s tomb.",
        )),
        _ => None,
    };

    match spec {
        Some((name, slot, rarity, glyph, level_req, stats, affix, affix_label, set_id, value, description)) => {
            let mut item = blank_item(gen, name, ItemCategory::Equipment, rarity, glyph, level_req, value, description);
            item.slot = Some(slot);
            item.stats = stats;
            item.affix = affix;
            item.affix_label = affix_label.map(|s| s.to_string());
            item.set_id = set_id;
            item
        }
        None => generate_procedural_item(
            gen,
            rng,
            3,
            ProceduralOptions { slot: None, min_rarity: Some(ItemRarity::Rare) },
        ),
    }
}

/// TS `createRandomSetRelic`: the pool grows with depth across all
/// four relic sets (8 pieces). Consumes exactly one draw for the pick.
pub fn create_random_set_relic(gen: &mut ItemIdGen, rng: &mut Rng, depth: i32) -> Item {
    let pool: &[&str] = if depth <= 2 {
        &["sylvan_longblade", "starlight_mantle"]
    } else if depth == 3 {
        &[
            "sylvan_longblade",
            "starlight_mantle",
            "cinder_crown",
            "emberheart_signet",
            "thornbound_circlet",
            "fenroot_talisman",
        ]
    } else {
        &[
            "sylvan_longblade",
            "starlight_mantle",
            "cinder_crown",
            "emberheart_signet",
            "thornbound_circlet",
            "fenroot_talisman",
            "glacierheart_pendant",
            "rimebound_plate",
        ]
    };
    let picked = pool[rng.range_usize(pool.len())];
    create_named_relic(gen, rng, picked)
}

/// TS `createQuestItem` (unknown tags get the Whispering Bell, as in TS).
/// Includes the Reborn town items tracked by `town.rs` via `questTag`:
/// the 3 Blacksmith pieces, the Heartcoal stall item, and the 3
/// regional supplies (`barrow_iron`, `mire_timber`, `frost_coal`).
pub fn create_quest_item(gen: &mut ItemIdGen, tag: &str) -> Item {
    let (name, rarity, glyph, description) = match tag {
        "smuggler_ledger" => (
            "Sealed Contraband Ledger", ItemRarity::Rare, "▤",
            "Wax-sealed manifest naming Citadel officials trading in crypt relics.",
        ),
        "pale_nightbloom" => (
            "Pale Nightbloom", ItemRarity::Uncommon, "✿",
            "Luminescent moss-flower that neutralizes crypt miasma.",
        ),
        "wayfarer_satchel" => (
            "Wayfarer’s Stolen Satchel", ItemRarity::Rare, "▤",
            "Rowan’s weathered leather map-satchel, plundered by Silverbridge brigands.",
        ),
        "morvath_seal" => (
            "Morvath’s Barrow Seal", ItemRarity::Epic, "◈",
            "Tarnished iron crown-seal taken from Arch-Wight Morvath in Whispering Barrow.",
        ),
        "fungal_heart" => (
            "Bioluminescent Fungal Heart", ItemRarity::Epic, "✿",
            "Pulsing core harvested from Spore-Matron Vexis deep inside Fungal Hollow.",
        ),
        "ashen_ore_heart" => (
            "Ashen Ore Heart", ItemRarity::Relic, "♦",
            "White-hot meteoric slag-core torn from Ignis-Warden Kaelthar in Ashen Caldera.",
        ),
        "blacksmith_anvil" => (
            "Torin’s Lost Anvil", ItemRarity::Rare, "▬",
            "Torin’s original anvil, dragged into the Reliquary when the old forge fell. One of three pieces needed to rebuild the Blacksmith.",
        ),
        "blacksmith_bellows" => (
            "Great Forge Bellows", ItemRarity::Rare, "≋",
            "Paired oak-and-hide bellows big enough to wake a cold forge. One of three pieces needed to rebuild the Blacksmith.",
        ),
        "blacksmith_sign" => (
            "The Smith’s Hanging Sign", ItemRarity::Uncommon, "▤",
            "A hammer-and-star sign, paint still bright. A forge without a sign is just a man shouting at metal. One of three pieces needed to rebuild the Blacksmith.",
        ),
        "heartcoal" => (
            "Heartcoal", ItemRarity::Epic, "♦",
            "A coal that has burned since before Oakhaven had a name. Torin’s forge is STALLED without it — cold iron lights for no one.",
        ),
        "barrow_iron" => (
            "Whispering Barrow Iron", ItemRarity::Rare, "▬",
            "Grave-tempered iron ingots recovered from Whispering Barrow. Banking them in Oakhaven grows population (+4) and unlocks the Barrow-Iron Hauberk at Torin’s rebuilt Blacksmith.",
        ),
        "mire_timber" => (
            "Thornwick Ironwood Timber", ItemRarity::Rare, "≡",
            "Rot-proof ironwood heartbeams harvested from Fungal Hollow. Banking them in Oakhaven grows population (+4) and unlocks the Mire-Timber Greatbow at Torin’s rebuilt Blacksmith.",
        ),
        "frost_coal" => (
            "Frostpeak White-Coal", ItemRarity::Epic, "♦",
            "Glacier-veined anthracite mined in Frostpeak Cavern. Banking it in Oakhaven grows population (+4) and unlocks the Frost-Quenched Greathelm at Torin’s rebuilt Blacksmith.",
        ),
        _ => (
            "Whispering Bell", ItemRarity::Relic, "⍾",
            "Ancient bronze bell humming with dormant ember resonance.",
        ),
    };
    let quest_tag = if tag == "whispering_bell" || !matches!(
        tag,
        "smuggler_ledger" | "pale_nightbloom" | "wayfarer_satchel" | "morvath_seal"
            | "fungal_heart" | "ashen_ore_heart" | "blacksmith_anvil" | "blacksmith_bellows"
            | "blacksmith_sign" | "heartcoal" | "barrow_iron" | "mire_timber" | "frost_coal"
    ) {
        // TS default arm always yields the bell, whatever tag was passed.
        "whispering_bell"
    } else {
        tag
    };
    let mut item = blank_item(gen, name, ItemCategory::Quest, rarity, glyph, 1, 0, description);
    item.quest_tag = Some(quest_tag.to_string());
    item
}

/// TS `upgradeEquipmentItem` (the tempering funnel). Every positive
/// stat grows: Max HP +5, Max Mana +3, everything else +2; value ×1.3
/// (rounded); the ` +N` name suffix is replaced, not stacked.
pub fn upgrade_equipment_item(item: &Item) -> Item {
    let mut updated = item.clone();
    let s = &mut updated.stats;
    let bump = |cur: Option<i32>, amount: i32| -> Option<i32> {
        cur.map(|v| if v > 0 { v + amount } else { v })
    };
    let bump_f = |cur: Option<f64>, amount: f64| -> Option<f64> {
        cur.map(|v| if v > 0.0 { v + amount } else { v })
    };
    s.atk = bump(s.atk, 2);
    s.def = bump(s.def, 2);
    s.spell_power = bump(s.spell_power, 2);
    s.max_hp = bump(s.max_hp, 5);
    s.max_mana = bump(s.max_mana, 3);
    s.crit_chance = bump_f(s.crit_chance, 2.0);
    s.dodge_chance = bump_f(s.dodge_chance, 2.0);
    s.lifesteal = bump_f(s.lifesteal, 2.0);
    s.thorns = bump(s.thorns, 2);

    let next_lv = item.upgrade_level + 1;
    // Strip a trailing " +<digits>" (TS regex /\s\+\d+$/).
    let base_name = strip_upgrade_suffix(&item.name);
    updated.upgrade_level = next_lv;
    updated.name = format!("{} +{}", base_name, next_lv);
    updated.value = (item.value as f64 * 1.3).round() as i32;
    updated
}

fn strip_upgrade_suffix(name: &str) -> String {
    if let Some(idx) = name.rfind(" +") {
        let tail = &name[idx + 2..];
        if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
            return name[..idx].to_string();
        }
    }
    name.to_string()
}

/// TS `getSalvageGoldYield`: salvage pays the merchant sell price
/// (55% of value, temper premium included), floored at 4g.
pub fn get_salvage_gold_yield(item: &Item) -> i32 {
    ((item.value as f64 * 0.55).round() as i32).max(4)
}

/// One formatted stat row from TS `formatStatList`.
#[derive(Debug, Clone, PartialEq)]
pub struct StatListEntry {
    pub label: String,
    pub value: String,
    pub key: StatKind,
    pub num: f64,
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

/// TS `formatStatList`: non-zero stats only, in TS's fixed order.
pub fn format_stat_list(stats: &ItemStats) -> Vec<StatListEntry> {
    let map: [(StatKind, &str, &str); 9] = [
        (StatKind::Atk, "ATK", ""),
        (StatKind::Def, "DEF", ""),
        (StatKind::SpellPower, "Spell", ""),
        (StatKind::MaxHp, "Max HP", ""),
        (StatKind::MaxMana, "Max MP", ""),
        (StatKind::CritChance, "Crit", "%"),
        (StatKind::DodgeChance, "Dodge", "%"),
        (StatKind::Lifesteal, "Lifesteal", "%"),
        (StatKind::Thorns, "Thorns", ""),
    ];
    let mut out = Vec::new();
    for (key, label, suffix) in map {
        let val = key.get(stats);
        if val != 0.0 {
            out.push(StatListEntry {
                label: label.to_string(),
                value: format!("+{}{}", fmt_num(val), suffix),
                key,
                num: val,
            });
        }
    }
    out
}

/// Whether the run's guaranteed early gear drop is still owed:
/// the first regular kill for a player of level <= 3 guarantees
/// equipment (HANDOFF guardrail; tracked via `WorldFlags.firstGearDropped`
/// in TS, passed in here as `first_gear_dropped`).
pub fn is_first_gear_guaranteed(player_level: i32, first_gear_dropped: bool) -> bool {
    player_level <= 3 && !first_gear_dropped
}

/// Roll whether an ordinary (non-boss, non-elite, non-rare) kill drops
/// anything. The guaranteed first-gear kill always drops and consumes
/// no RNG draw; otherwise the chance is `NORMAL_KILL_DROP_CHANCE`
/// (0.3) from `run_rules`.
pub fn roll_ordinary_kill_drop(
    rng: &mut Rng,
    player_level: i32,
    first_gear_dropped: bool,
) -> bool {
    if is_first_gear_guaranteed(player_level, first_gear_dropped) {
        return true;
    }
    (rng.next_f32() as f64) < NORMAL_KILL_DROP_CHANCE
}

/// Generate the item for an ordinary kill drop, or `None` when the
/// roll fails. A guaranteed first-gear drop is always equipment.
pub fn generate_kill_drop(
    gen: &mut ItemIdGen,
    rng: &mut Rng,
    depth: i32,
    player_level: i32,
    first_gear_dropped: bool,
) -> Option<Item> {
    if !roll_ordinary_kill_drop(rng, player_level, first_gear_dropped) {
        return None;
    }
    Some(generate_procedural_item(gen, rng, depth, ProceduralOptions::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retuned_curve_thresholds() {
        // Depth bonus: ×5 capped at +50 (Nov 16 retune).
        assert_eq!(rarity_depth_bonus(0), 0);
        assert_eq!(rarity_depth_bonus(1), 5);
        assert_eq!(rarity_depth_bonus(3), 15);
        assert_eq!(rarity_depth_bonus(10), 50);
        assert_eq!(rarity_depth_bonus(60), 50);
        assert_eq!(rarity_depth_bonus(-2), 0);

        // Strict thresholds, ordinary caller (no force-min).
        assert_eq!(rarity_from_roll(42.0, None), ItemRarity::Common);
        assert_eq!(rarity_from_roll(42.01, None), ItemRarity::Uncommon);
        assert_eq!(rarity_from_roll(75.0, None), ItemRarity::Uncommon);
        assert_eq!(rarity_from_roll(75.01, None), ItemRarity::Rare);
        assert_eq!(rarity_from_roll(104.0, None), ItemRarity::Rare);
        assert_eq!(rarity_from_roll(104.01, None), ItemRarity::Epic);
        // Ordinary drops cap at Epic even past the Relic line.
        assert_eq!(rarity_from_roll(125.0, None), ItemRarity::Epic);
        assert_eq!(rarity_from_roll(149.99, None), ItemRarity::Epic);

        // Premium callers (force-min Epic/Relic) may reach Relic.
        assert_eq!(rarity_from_roll(125.0, Some(ItemRarity::Epic)), ItemRarity::Epic);
        assert_eq!(rarity_from_roll(125.01, Some(ItemRarity::Epic)), ItemRarity::Relic);
        assert_eq!(rarity_from_roll(130.0, Some(ItemRarity::Relic)), ItemRarity::Relic);
        // A non-premium force-min does NOT unlock Relic.
        assert_eq!(rarity_from_roll(130.0, Some(ItemRarity::Rare)), ItemRarity::Epic);
        // Force-min raises the floor.
        assert_eq!(rarity_from_roll(0.0, Some(ItemRarity::Rare)), ItemRarity::Rare);
        assert_eq!(rarity_from_roll(0.0, Some(ItemRarity::Epic)), ItemRarity::Epic);
    }

    #[test]
    fn retuned_curve_distribution_matches_ts_test() {
        // Mirror of ts/tests/runRules.test.ts distribution checks:
        // sweep the unit interval deterministically through the Rng-free
        // core by calling rarity_from_roll with roll = u*100 + bonus.
        let dist = |depth: i32, min: Option<ItemRarity>| {
            let mut counts: HashMap<String, i32> = HashMap::new();
            for i in 0..1000 {
                let u = (i as f64 + 0.5) / 1000.0;
                let r = rarity_from_roll(u * 100.0 + rarity_depth_bonus(depth) as f64, min);
                *counts.entry(r.to_string()).or_insert(0) += 1;
            }
            counts
        };
        let f1 = dist(1, None);
        assert!(*f1.get("Epic").unwrap_or(&0) <= 20, "Floor 1 Epic {:?}", f1);
        let f3 = dist(3, None);
        let f3_epic = *f3.get("Epic").unwrap_or(&0);
        assert!((80..=140).contains(&f3_epic), "Floor 3 Epic {f3_epic}");
        for d in [1, 5, 8, 20, 60] {
            assert_eq!(*dist(d, None).get("Relic").unwrap_or(&0), 0, "depth {d} ordinary Relic");
        }
        assert!(*dist(8, Some(ItemRarity::Epic)).get("Relic").unwrap_or(&0) > 0);
        assert_eq!(dist(60, None), dist(10, None), "depth bonus is capped");
    }

    #[test]
    fn ordinary_rolls_never_relic_across_seeds() {
        let mut gen = ItemIdGen::new();
        for seed in 0..200u32 {
            let mut rng = Rng::new(seed);
            for depth in [1, 5, 8, 20] {
                assert_ne!(roll_rarity(&mut rng, depth, None), ItemRarity::Relic);
                let item = generate_procedural_item(
                    &mut gen,
                    &mut rng,
                    depth,
                    ProceduralOptions::default(),
                );
                assert_ne!(item.rarity, ItemRarity::Relic);
                assert_eq!(item.category, ItemCategory::Equipment);
            }
        }
        // Premium rolls can reach Relic at depth.
        let mut saw_relic = false;
        for seed in 0..200u32 {
            let mut rng = Rng::new(seed);
            if roll_rarity(&mut rng, 8, Some(ItemRarity::Epic)) == ItemRarity::Relic {
                saw_relic = true;
            }
        }
        assert!(saw_relic, "premium caller should roll Relic sometimes");
    }

    #[test]
    fn first_kill_guarantee() {
        assert!(is_first_gear_guaranteed(1, false));
        assert!(is_first_gear_guaranteed(3, false));
        assert!(!is_first_gear_guaranteed(4, false));
        assert!(!is_first_gear_guaranteed(2, true));

        // Guaranteed: every seed drops, and the drop is equipment.
        let mut gen = ItemIdGen::new();
        for seed in 0..50u32 {
            let mut rng = Rng::new(seed);
            let drop = generate_kill_drop(&mut gen, &mut rng, 1, 2, false);
            let item = drop.expect("first kill at level <= 3 must drop gear");
            assert_eq!(item.category, ItemCategory::Equipment);
        }
        // Not guaranteed: level 4 uses the 30% ordinary chance, so over
        // many seeds both outcomes occur (and never a guaranteed drop).
        let mut drops = 0;
        for seed in 0..200u32 {
            let mut rng = Rng::new(seed);
            if roll_ordinary_kill_drop(&mut rng, 4, false) {
                drops += 1;
            }
        }
        assert!(drops > 0 && drops < 200, "ordinary chance should be partial, got {drops}/200");
        assert_eq!(NORMAL_KILL_DROP_CHANCE, 0.3);
    }

    #[test]
    fn town_quest_items_carry_their_tags() {
        let mut gen = ItemIdGen::new();
        for tag in [
            "blacksmith_anvil",
            "blacksmith_bellows",
            "blacksmith_sign",
            "heartcoal",
            "barrow_iron",
            "mire_timber",
            "frost_coal",
        ] {
            let item = create_quest_item(&mut gen, tag);
            assert_eq!(item.quest_tag.as_deref(), Some(tag));
            assert_eq!(item.category, ItemCategory::Quest);
            assert_eq!(item.value, 0);
        }
        // Unknown tags fall back to the Whispering Bell, as in TS.
        let bell = create_quest_item(&mut gen, "nonsense");
        assert_eq!(bell.name, "Whispering Bell");
        assert_eq!(bell.quest_tag.as_deref(), Some("whispering_bell"));
    }

    #[test]
    fn starter_and_set_relic_data() {
        let mut gen = ItemIdGen::new();
        let mut rng = Rng::new(7);
        let sword = create_starter_item(&mut gen, "starter_vanguard");
        assert_eq!(sword.name, "Recruit Broadsword");
        assert_eq!(sword.stats.atk, Some(5));

        let blade = create_named_relic(&mut gen, &mut rng, "sylvan_longblade");
        let mantle = create_named_relic(&mut gen, &mut rng, "starlight_mantle");
        assert_eq!(blade.set_id, Some(RelicSetId::StarlightOutrider));
        assert_eq!(mantle.set_id, Some(RelicSetId::StarlightOutrider));
        let sets = relic_sets();
        assert_eq!(sets[&RelicSetId::StarlightOutrider].pieces.len(), 2);

        // Set-relic pool at depth <= 2 only holds the Starlight pair.
        for seed in 0..20u32 {
            let mut r = Rng::new(seed);
            let item = create_random_set_relic(&mut gen, &mut r, 2);
            assert_eq!(item.set_id, Some(RelicSetId::StarlightOutrider));
        }
    }

    #[test]
    fn salvage_and_upgrade_match_ts() {
        let mut gen = ItemIdGen::new();
        let mut item = create_starter_item(&mut gen, "starter_vanguard");
        assert_eq!(get_salvage_gold_yield(&item), 13); // round(24 * 0.55) = 13
        item.value = 7;
        assert_eq!(get_salvage_gold_yield(&item), 4); // floor of 4g

        let up = upgrade_equipment_item(&create_starter_item(&mut gen, "starter_vanguard"));
        assert_eq!(up.upgrade_level, 1);
        assert_eq!(up.name, "Recruit Broadsword +1");
        assert_eq!(up.stats.atk, Some(7));
        assert_eq!(up.stats.def, Some(3));
        let up2 = upgrade_equipment_item(&up);
        assert_eq!(up2.name, "Recruit Broadsword +2");
        // Value compounds per temper with rounding each step:
        // round(24*1.3)=31, then round(31*1.3)=40.
        assert_eq!(up.value, 31);
        assert_eq!(up2.value, 40);
    }
}
