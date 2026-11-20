//! Core data types, ported from `elfstoryreborn/src/game/types.ts`.
//! Serde representations match the TS save format where practical:
//! string unions serialize as their TS string values, and `ZoneId`
//! serializes as e.g. "oakhaven" / "dungeon_3".

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $s:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $($variant),+
        }
        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

string_enum!(GameState {
    TitleMenu => "TITLE_MENU",
    Playing => "PLAYING",
    Paused => "PAUSED",
    Victory => "VICTORY",
    GameOver => "GAME_OVER",
});

string_enum!(ActivePanel {
    None => "NONE",
    Skills => "SKILLS",
    Inventory => "INVENTORY",
    Dialogue => "DIALOGUE",
    Quests => "QUESTS",
    Merchant => "MERCHANT",
    Travel => "TRAVEL",
    Legend => "LEGEND",
});

/// Zone identifier. TS: 'oakhaven' | 'overworld' | ... | `dungeon_${number}` etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZoneId {
    Oakhaven,
    Overworld,
    Highspire,
    Thornwick,
    Cinderwatch,
    Dungeon(u32),
    Barrow(u32),
    Mire(u32),
    Frost(u32),
}

impl ZoneId {
    pub fn to_string_id(&self) -> String {
        match self {
            Self::Oakhaven => "oakhaven".into(),
            Self::Overworld => "overworld".into(),
            Self::Highspire => "highspire".into(),
            Self::Thornwick => "thornwick".into(),
            Self::Cinderwatch => "cinderwatch".into(),
            Self::Dungeon(n) => format!("dungeon_{n}"),
            Self::Barrow(n) => format!("barrow_{n}"),
            Self::Mire(n) => format!("mire_{n}"),
            Self::Frost(n) => format!("frost_{n}"),
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "oakhaven" => return Some(Self::Oakhaven),
            "overworld" => return Some(Self::Overworld),
            "highspire" => return Some(Self::Highspire),
            "thornwick" => return Some(Self::Thornwick),
            "cinderwatch" => return Some(Self::Cinderwatch),
            _ => {}
        }
        let (prefix, num) = s.rsplit_once('_')?;
        let n: u32 = num.parse().ok()?;
        match prefix {
            "dungeon" => Some(Self::Dungeon(n)),
            "barrow" => Some(Self::Barrow(n)),
            "mire" => Some(Self::Mire(n)),
            "frost" => Some(Self::Frost(n)),
            _ => None,
        }
    }
    /// Depth of a dungeon-like zone (0 for towns/overworld).
    pub fn depth(&self) -> u32 {
        match self {
            Self::Dungeon(n) | Self::Barrow(n) | Self::Mire(n) | Self::Frost(n) => *n,
            _ => 0,
        }
    }
}

impl Serialize for ZoneId {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.to_string_id())
    }
}
impl<'de> Deserialize<'de> for ZoneId {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        Self::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown zone id: {s}")))
    }
}
impl fmt::Display for ZoneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_id())
    }
}

string_enum!(SaveSlotId { Slot1 => "slot_1", Slot2 => "slot_2", Slot3 => "slot_3" });

string_enum!(TileKind {
    Wall => "wall",
    Floor => "floor",
    Road => "road",
    Water => "water",
    Chasm => "chasm",
    Door => "door",
    StairsDown => "stairs_down",
    StairsUp => "stairs_up",
    GateCity => "gate_city",
    Torch => "torch",
    Shrine => "shrine",
    Brazier => "brazier",
    Chest => "chest",
    Trap => "trap",
    DartTrap => "dart_trap",
    LoreTablet => "lore_tablet",
    Tree => "tree",
    Pillar => "pillar",
    MarketStall => "market_stall",
    Fountain => "fountain",
    HerbNode => "herb_node",
    ContrabandCache => "contraband_cache",
    WhisperingBell => "whispering_bell",
    VaultDoor => "vault_door",
    Waystone => "waystone",
    ForgottenForge => "forgotten_forge",
    CursedAltar => "cursed_altar",
    SporePool => "spore_pool",
    FrostVent => "frost_vent",
    EmberCrust => "ember_crust",
    ShockRune => "shock_rune",
});

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub x: i32,
    pub y: i32,
    pub kind: TileKind,
    pub glyph: String,
    pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    pub walkable: bool,
    #[serde(rename = "blocksSight")]
    pub blocks_sight: bool,
    pub explored: bool,
    pub visible: bool,
    #[serde(default, rename = "interactId", skip_serializing_if = "Option::is_none")]
    pub interact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revealed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

string_enum!(AttributeKey { Might => "might", Agility => "agility", Arcana => "arcana", Resolve => "resolve" });

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attributes {
    pub might: i32,
    pub agility: i32,
    pub arcana: i32,
    pub resolve: i32,
}
impl Attributes {
    pub fn get(&self, key: AttributeKey) -> i32 {
        match key {
            AttributeKey::Might => self.might,
            AttributeKey::Agility => self.agility,
            AttributeKey::Arcana => self.arcana,
            AttributeKey::Resolve => self.resolve,
        }
    }
}

string_enum!(SkillBranch { Vanguard => "vanguard", Shadowcraft => "shadowcraft", Runecaster => "runecaster" });

string_enum!(SkillId {
    Cleave => "cleave",
    ShieldBash => "shield_bash",
    Bloodlust => "bloodlust",
    IronFortress => "iron_fortress",
    Shadowstep => "shadowstep",
    VenomTip => "venom_tip",
    EvasiveInstinct => "evasive_instinct",
    ExecutionersMark => "executioners_mark",
    EmberLance => "ember_lance",
    FrostNova => "frost_nova",
    SiphonSoul => "siphon_soul",
    ArcaneEcho => "arcane_echo",
});

string_enum!(SkillTargeting { Self_ => "self", Adjacent => "adjacent", Ranged => "ranged", Line => "line", Blink => "blink", Passive => "passive" });

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillDef {
    pub id: SkillId,
    pub name: String,
    pub branch: SkillBranch,
    pub tier: u8,
    pub kind: String, // "active" | "passive"
    pub targeting: SkillTargeting,
    pub range: i32,
    #[serde(rename = "manaCost")]
    pub mana_cost: i32,
    pub cooldown: i32,
    pub glyph: String,
    pub summary: String,
    #[serde(rename = "rankDescriptions")]
    pub rank_descriptions: [String; 3],
    #[serde(rename = "scalingNote")]
    pub scaling_note: String,
}

string_enum!(ItemSlot { Weapon => "weapon", Armor => "armor", Helm => "helm", Ring => "ring", Amulet => "amulet" });

string_enum!(ItemRarity { Common => "Common", Uncommon => "Uncommon", Rare => "Rare", Epic => "Epic", Relic => "Relic" });

string_enum!(ItemAffixId {
    BleedOnCrit => "bleed_on_crit",
    ManaOnKill => "mana_on_kill",
    ThornsBurst => "thorns_burst",
    FirstStrike => "first_strike",
    ExecuteBonus => "execute_bonus",
    ChillOnHit => "chill_on_hit",
    VampiricTouch => "vampiric_touch",
    ArcaneWard => "arcane_ward",
});

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ItemStats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atk: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub def: Option<i32>,
    #[serde(default, rename = "spellPower", skip_serializing_if = "Option::is_none")]
    pub spell_power: Option<i32>,
    #[serde(default, rename = "maxHp", skip_serializing_if = "Option::is_none")]
    pub max_hp: Option<i32>,
    #[serde(default, rename = "maxMana", skip_serializing_if = "Option::is_none")]
    pub max_mana: Option<i32>,
    #[serde(default, rename = "critChance", skip_serializing_if = "Option::is_none")]
    pub crit_chance: Option<f64>,
    #[serde(default, rename = "dodgeChance", skip_serializing_if = "Option::is_none")]
    pub dodge_chance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifesteal: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thorns: Option<i32>,
}

string_enum!(ConsumableKind {
    HealPotion => "heal_potion",
    ManaPotion => "mana_potion",
    ScrollBlink => "scroll_blink",
    WardTalisman => "ward_talisman",
    ThrowingKnife => "throwing_knife",
});

string_enum!(ProdigyId {
    Cauterize => "cauterize",
    DraconicWill => "draconic_will",
    FlexibleCombat => "flexible_combat",
    CarryTheWorld => "carry_the_world",
    ArcaneConduit => "arcane_conduit",
    SpineOfWorld => "spine_of_world",
});

string_enum!(RelicSetId {
    StarlightOutrider => "starlight_outrider",
    Cinderforged => "cinderforged",
    Thornbound => "thornbound",
    Rimebound => "rimebound",
});

string_enum!(CompanionKind { SilverwindLynx => "silverwind_lynx", StarlightOwl => "starlight_owl" });

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompanionState {
    pub kind: CompanionKind,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    #[serde(rename = "maxHp")]
    pub max_hp: i32,
    #[serde(rename = "respawnCooldown")]
    pub respawn_cooldown: i32,
}

string_enum!(ItemCategory { Equipment => "equipment", Consumable => "consumable", Quest => "quest" });

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub category: ItemCategory,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<ItemSlot>,
    #[serde(default, rename = "consumableKind", skip_serializing_if = "Option::is_none")]
    pub consumable_kind: Option<ConsumableKind>,
    pub rarity: ItemRarity,
    pub glyph: String,
    #[serde(rename = "levelReq")]
    pub level_req: i32,
    #[serde(rename = "upgradeLevel")]
    pub upgrade_level: i32,
    pub stats: ItemStats,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affix: Option<ItemAffixId>,
    #[serde(default, rename = "affixLabel", skip_serializing_if = "Option::is_none")]
    pub affix_label: Option<String>,
    #[serde(default, rename = "setId", skip_serializing_if = "Option::is_none")]
    pub set_id: Option<RelicSetId>,
    pub value: i32,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<i32>,
    #[serde(default, rename = "questTag", skip_serializing_if = "Option::is_none")]
    pub quest_tag: Option<String>,
}

string_enum!(StatusKind {
    Bleed => "bleed",
    Burn => "burn",
    Poison => "poison",
    Stunned => "stunned",
    Shielded => "shielded",
    Marked => "marked",
    Chilled => "chilled",
    Regen => "regen",
    Hasted => "hasted",
    Cauterized => "cauterized",
    SpineGuard => "spine_guard",
});

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StatusEffect {
    pub kind: StatusKind,
    pub turns: i32,
    pub potency: i32,
}

string_enum!(EnemyArchetype {
    GoblinSkirmisher => "goblin_skirmisher",
    CryptSkeleton => "crypt_skeleton",
    PlagueRat => "plague_rat",
    BoneAcolyte => "bone_acolyte",
    AshCultist => "ash_cultist",
    ShadowStalker => "shadow_stalker",
    IronGolem => "iron_golem",
    CinderWisp => "cinder_wisp",
    DreadWraith => "dread_wraith",
    BlightWeaver => "blight_weaver",
    ObsidianWarden => "obsidian_warden",
    GraveHusk => "grave_husk",
    SporeHulk => "spore_hulk",
    RimeRevenant => "rime_revenant",
    BossBoneCantor => "boss_bone_cantor",
    BossAshenInquisitor => "boss_ashen_inquisitor",
    BossMalakor => "boss_malakor",
});

string_enum!(EnemyRank { Normal => "normal", Elite => "elite", Rare => "rare", Randboss => "randboss" });

string_enum!(IntentType { Strike => "strike", HeavyWindup => "heavy_windup", HexBolt => "hex_bolt", GroundSlam => "ground_slam", EnemySkill => "enemy_skill" });

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}
impl Pos {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnemyIntent {
    #[serde(rename = "type")]
    pub kind: IntentType,
    pub label: String,
    pub damage: i32,
    #[serde(rename = "targetTiles")]
    pub target_tiles: Vec<Pos>,
    #[serde(default, rename = "skillId", skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<SkillId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Enemy {
    pub id: String,
    pub archetype: EnemyArchetype,
    pub name: String,
    pub glyph: String,
    pub color: String,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    #[serde(rename = "maxHp")]
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    #[serde(rename = "xpReward")]
    pub xp_reward: i32,
    #[serde(rename = "goldReward")]
    pub gold_reward: i32,
    #[serde(default, rename = "isBoss", skip_serializing_if = "Option::is_none")]
    pub is_boss: Option<bool>,
    #[serde(default, rename = "isElite", skip_serializing_if = "Option::is_none")]
    pub is_elite: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<EnemyRank>,
    #[serde(default, rename = "rankTitle", skip_serializing_if = "Option::is_none")]
    pub rank_title: Option<String>,
    #[serde(default, rename = "skillLoadout", skip_serializing_if = "Option::is_none")]
    pub skill_loadout: Option<Vec<SkillId>>,
    #[serde(default, rename = "skillCooldowns", skip_serializing_if = "Option::is_none")]
    pub skill_cooldowns: Option<HashMap<String, i32>>,
    #[serde(default, rename = "runeCharge", skip_serializing_if = "Option::is_none")]
    pub rune_charge: Option<bool>,
    #[serde(default, rename = "undyingUsed", skip_serializing_if = "Option::is_none")]
    pub undying_used: Option<bool>,
    pub statuses: Vec<StatusEffect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<EnemyIntent>,
    pub alerted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pacified: Option<bool>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundLoot {
    pub id: String,
    pub x: i32,
    pub y: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gold: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<Item>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpcPlacement {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub glyph: String,
    pub color: String,
    pub name: String,
    pub title: String,
    #[serde(default, rename = "homeX", skip_serializing_if = "Option::is_none")]
    pub home_x: Option<i32>,
    #[serde(default, rename = "homeY", skip_serializing_if = "Option::is_none")]
    pub home_y: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roams: Option<bool>,
}

string_enum!(CritterKind {
    ValeRabbit => "vale_rabbit",
    AshFox => "ash_fox",
    CinderCrow => "cinder_crow",
    CaveBat => "cave_bat",
    TownCat => "town_cat",
    FenFrog => "fen_frog",
});

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CritterState {
    pub id: String,
    pub kind: CritterKind,
    pub name: String,
    pub glyph: String,
    pub color: String,
    pub x: i32,
    pub y: i32,
    #[serde(rename = "homeX")]
    pub home_x: i32,
    #[serde(rename = "homeY")]
    pub home_y: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneMap {
    pub id: ZoneId,
    pub name: String,
    pub subtitle: String,
    #[serde(rename = "isTown")]
    pub is_town: bool,
    pub depth: i32,
    pub width: i32,
    pub height: i32,
    /// Row-major: tiles[y][x], as in TS.
    pub tiles: Vec<Vec<Tile>>,
    pub enemies: Vec<Enemy>,
    pub npcs: Vec<NpcPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critters: Option<Vec<CritterState>>,
    pub loot: Vec<GroundLoot>,
    #[serde(rename = "spawnPoint")]
    pub spawn_point: Pos,
    #[serde(rename = "returnPoint")]
    pub return_point: Pos,
}

string_enum!(StarterClass { Vanguard => "Vanguard", Shadowcraft => "Shadowcraft", Runecaster => "Runecaster", Wanderer => "Wanderer" });

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerState {
    pub name: String,
    #[serde(rename = "starterClass")]
    pub starter_class: StarterClass,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    #[serde(rename = "maxHp")]
    pub max_hp: i32,
    pub mana: i32,
    #[serde(rename = "maxMana")]
    pub max_mana: i32,
    pub level: i32,
    pub xp: i32,
    #[serde(rename = "xpToNext")]
    pub xp_to_next: i32,
    pub gold: i32,
    #[serde(rename = "attributePoints")]
    pub attribute_points: i32,
    #[serde(rename = "skillPoints")]
    pub skill_points: i32,
    pub attributes: Attributes,
    pub skills: HashMap<String, i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prodigies: Option<Vec<ProdigyId>>,
    #[serde(default, rename = "prodigyCooldowns", skip_serializing_if = "Option::is_none")]
    pub prodigy_cooldowns: Option<HashMap<String, i32>>,
    pub hotbar: Vec<Option<SkillId>>,
    pub cooldowns: HashMap<String, i32>,
    #[serde(default, rename = "hasteSteps", skip_serializing_if = "Option::is_none")]
    pub haste_steps: Option<i32>,
    #[serde(default, rename = "bonusHp", skip_serializing_if = "Option::is_none")]
    pub bonus_hp: Option<i32>,
    pub statuses: Vec<StatusEffect>,
    pub equipped: HashMap<String, Item>,
    pub inventory: Vec<Item>,
    pub kills: i32,
    #[serde(rename = "turnsElapsed")]
    pub turns_elapsed: i32,
    #[serde(rename = "firstStrikeReady")]
    pub first_strike_ready: bool,
    #[serde(default, rename = "lastDirX", skip_serializing_if = "Option::is_none")]
    pub last_dir_x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion: Option<CompanionState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivedStats {
    #[serde(rename = "maxHp")]
    pub max_hp: i32,
    #[serde(rename = "maxMana")]
    pub max_mana: i32,
    pub atk: i32,
    pub def: i32,
    #[serde(rename = "spellPower")]
    pub spell_power: i32,
    #[serde(rename = "critChance")]
    pub crit_chance: f64,
    #[serde(rename = "dodgeChance")]
    pub dodge_chance: f64,
    pub lifesteal: f64,
    pub thorns: i32,
    #[serde(rename = "buildTitle")]
    pub build_title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldFlags {
    #[serde(rename = "discoveredZones")]
    pub discovered_zones: Vec<ZoneId>,
    #[serde(default, rename = "caravanChoice", skip_serializing_if = "Option::is_none")]
    pub caravan_choice: Option<String>,
    #[serde(default, rename = "blightChoice", skip_serializing_if = "Option::is_none")]
    pub blight_choice: Option<String>,
    #[serde(default, rename = "bellChoice", skip_serializing_if = "Option::is_none")]
    pub bell_choice: Option<String>,
    #[serde(default, rename = "vexTipClaimed", skip_serializing_if = "Option::is_none")]
    pub vex_tip_claimed: Option<bool>,
    #[serde(default, rename = "malakorWeakened", skip_serializing_if = "Option::is_none")]
    pub malakor_weakened: Option<bool>,
    #[serde(default, rename = "cipherInscribed", skip_serializing_if = "Option::is_none")]
    pub cipher_inscribed: Option<bool>,
    #[serde(default, rename = "malakorDefeated", skip_serializing_if = "Option::is_none")]
    pub malakor_defeated: Option<bool>,
    #[serde(default, rename = "cantorDefeated", skip_serializing_if = "Option::is_none")]
    pub cantor_defeated: Option<bool>,
    #[serde(default, rename = "cantorPacified", skip_serializing_if = "Option::is_none")]
    pub cantor_pacified: Option<bool>,
    #[serde(default, rename = "inquisitorDefeated", skip_serializing_if = "Option::is_none")]
    pub inquisitor_defeated: Option<bool>,
    #[serde(default, rename = "barrowBossDefeated", skip_serializing_if = "Option::is_none")]
    pub barrow_boss_defeated: Option<bool>,
    #[serde(default, rename = "mireBossDefeated", skip_serializing_if = "Option::is_none")]
    pub mire_boss_defeated: Option<bool>,
    #[serde(default, rename = "frostBossDefeated", skip_serializing_if = "Option::is_none")]
    pub frost_boss_defeated: Option<bool>,
    #[serde(default, rename = "silverbridgeCleared", skip_serializing_if = "Option::is_none")]
    pub silverbridge_cleared: Option<bool>,
    #[serde(rename = "miraPotionTier")]
    pub mira_potion_tier: i32,
    #[serde(rename = "citadelAccess")]
    pub citadel_access: bool,
    #[serde(rename = "floor3Shortcut")]
    pub floor3_shortcut: bool,
    #[serde(rename = "npcMet")]
    pub npc_met: HashMap<String, bool>,
    #[serde(rename = "npcDisposition")]
    pub npc_disposition: HashMap<String, i32>,
    #[serde(rename = "readLoreIds")]
    pub read_lore_ids: Vec<String>,
    #[serde(default, rename = "confrontedBosses", skip_serializing_if = "Option::is_none")]
    pub confronted_bosses: Option<Vec<String>>,
    #[serde(default, rename = "firstGearDropped", skip_serializing_if = "Option::is_none")]
    pub first_gear_dropped: Option<bool>,
    #[serde(default, rename = "waystonesAttuned", skip_serializing_if = "Option::is_none")]
    pub waystones_attuned: Option<i32>,
}

string_enum!(QuestStatus { Active => "active", Completed => "completed", Failed => "failed" });

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestProgress {
    pub id: String,
    pub status: QuestStatus,
    pub stage: i32,
    #[serde(default, rename = "outcomeLabel", skip_serializing_if = "Option::is_none")]
    pub outcome_label: Option<String>,
    pub history: Vec<String>,
}

string_enum!(LogTone {
    Neutral => "neutral",
    Combat => "combat",
    Crit => "crit",
    Loot => "loot",
    Quest => "quest",
    Danger => "danger",
    Arcane => "arcane",
});

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub turn: i32,
    pub text: String,
    pub tone: LogTone,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChronicleEntry {
    pub id: String,
    pub outcome: String, // "victory" | "fallen"
    #[serde(rename = "starterClass")]
    pub starter_class: StarterClass,
    #[serde(rename = "buildTitle")]
    pub build_title: String,
    pub level: i32,
    #[serde(rename = "deepestDepth")]
    pub deepest_depth: i32,
    pub kills: i32,
    #[serde(rename = "turnsElapsed")]
    pub turns_elapsed: i32,
    pub gold: i32,
    #[serde(rename = "keyDecisions")]
    pub key_decisions: Vec<String>,
    #[serde(rename = "createdAtMs")]
    pub created_at_ms: i64,
}
