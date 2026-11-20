//! World generation, port of `elfstoryreborn/src/game/world.ts`.
//!
//! In scope:
//! - `create_oakhaven_map` — hand-authored Oakhaven, Torin at (7, 4).
//! - `create_highspire_map` / `create_thornwick_map` /
//!   `create_cinderwatch_map` / `create_overworld_map` — the remaining
//!   hand-authored towns and the Silverwood Vale overworld (Milestone 3).
//! - `generate_dungeon_floor` — room/corridor generator with the
//!   orthogonal connectivity guarantee and, on Floor 1 of the main
//!   descent, the guaranteed 4 town items (3 Blacksmith pieces +
//!   Heartcoal) as ground loot. Story bosses (Floors 2/4/5), mini-boss
//!   wardens (Floors 1/3), Abyss echo bosses (even Abyss floors) and
//!   harbingers (odd Abyss floors), all four set pieces (sepulcher,
//!   spore-heart, rime-circle, sealed-brazier) and the sealed treasure
//!   vault are ported.
//! - `generate_side_dungeon_floor` — barrow/mire/frost branches reusing
//!   the dungeon generator at an effective depth, with biome pools,
//!   floor-1 town supplies and named terminal-floor side bosses.
//! - `generate_zone_by_id` / `create_initial_zones` / `get_zone_context`.
//! - `create_enemy` / `promote_enemy_rank` — every spawn funnels through
//!   `difficulty::scale_enemy_for_difficulty`, then the KILL_XP_FACTOR
//!   trim, exactly as TS `createEnemy` does.
//!
//! Rust divergences from TS (deliberate, no behaviour change):
//! - TS uses a module-global enemy id counter and the global seeded
//!   stream; here ids come from an explicit `seq` counter and all
//!   randomness from a threaded `&mut Rng`.
//! - TS reads difficulty from module globals; here `tier`/`cycle` are
//!   parameters threaded to the scaling funnel.
//! - TS calls `loot.ts` generators (`createQuestItem`,
//!   `createConsumable`, `generateProceduralItem`); per the M1 contract
//!   this module does NOT import `crate::loot`, so those calls are
//!   mirrored by the local `*_local` functions at the bottom of the
//!   generator section, with the same call shape and data tables.

use crate::difficulty::{scale_enemy_for_difficulty, DifficultyTier};
use crate::rng::Rng;
use crate::types::{
    CritterKind, CritterState, Enemy, EnemyArchetype, EnemyRank, GroundLoot, Item, ItemAffixId,
    ItemCategory, ItemRarity, ItemSlot, ItemStats, NpcPlacement, Pos, SkillId, Tile, TileKind,
    ZoneId, ZoneMap,
};
use std::collections::HashSet;

fn rand_f(rng: &mut Rng) -> f64 {
    rng.next_f32() as f64
}

/// TS `Math.floor(random() * n)` over the seeded stream.
fn rand_below(rng: &mut Rng, n: i32) -> i32 {
    if n <= 1 {
        return 0;
    }
    (rand_f(rng) * n as f64).floor() as i32
}

fn round(x: f64) -> i32 {
    x.round() as i32
}

// ---------------------------------------------------------------- tiles

/// Port of `makeTile`: glyph/colour/walkability/sight table per kind.
pub fn make_tile(x: i32, y: i32, kind: TileKind) -> Tile {
    let (glyph, color, walkable, blocks_sight, label): (&str, &str, bool, bool, Option<&str>) =
        match kind {
            TileKind::Wall => ("#", "#3F4756", false, true, None),
            TileKind::Floor => ("·", "#39414E", true, false, None),
            TileKind::Road => ("░", "#5E5647", true, false, None),
            TileKind::Water => ("≈", "#3B7EA1", false, false, Some("Fountain Water")),
            TileKind::Chasm => ("~", "#C2512D", false, false, Some("Ember Fissure")),
            TileKind::Door => ("+", "#D99B26", true, false, Some("Timber Gate")),
            TileKind::StairsDown => (">", "#F59E0B", true, false, Some("Descent Stair")),
            TileKind::StairsUp => ("<", "#93C5FD", true, false, Some("Ascent Stair")),
            TileKind::GateCity => ("Ω", "#FBBF24", true, false, Some("Citadel Road Gate")),
            TileKind::Torch => ("*", "#FB923C", false, false, Some("Wall Torch")),
            TileKind::Shrine => ("┼", "#60A5FA", true, false, Some("Glyph Shrine")),
            TileKind::Brazier => ("☼", "#F59E0B", true, false, Some("Restorative Brazier")),
            TileKind::Chest => ("$", "#FBBF24", true, false, Some("Ironbound Chest")),
            TileKind::Trap => ("^", "#EF4444", true, false, Some("Spike Snare")),
            TileKind::DartTrap => ("»", "#A3E635", true, false, Some("Venom Dart Snare")),
            TileKind::LoreTablet => ("‡", "#A78BFA", true, false, Some("Carved Stele")),
            TileKind::Tree => ("♣", "#4EA885", false, true, Some("Ashen Pine")),
            TileKind::Pillar => ("║", "#6B7280", false, true, Some("Stone Pillar")),
            TileKind::MarketStall => ("Π", "#D99B26", false, false, Some("Market Stall")),
            TileKind::Fountain => ("◎", "#60A5FA", false, false, Some("Citadel Fountain")),
            TileKind::HerbNode => ("✿", "#34D399", true, false, Some("Pale Nightbloom")),
            TileKind::ContrabandCache => ("▤", "#F472B6", true, false, Some("Smuggler Cache")),
            TileKind::WhisperingBell => ("⍾", "#60A5FA", true, false, Some("The Whispering Bell")),
            TileKind::VaultDoor => (
                "⊞",
                "#F59E0B",
                false,
                true,
                Some("Sealed Treasure Vault Door (Danger)"),
            ),
            TileKind::Waystone => ("◈", "#38BDF8", true, false, Some("Elven Waystone")),
            TileKind::ForgottenForge => ("⚒", "#FB923C", true, false, Some("Forgotten Forge")),
            TileKind::CursedAltar => ("☠", "#C084FC", true, false, Some("Occult Blood Altar")),
            TileKind::SporePool => (
                "≈",
                "#10B981",
                true,
                false,
                Some("Blight Spore Pool (Poison Hazard)"),
            ),
            TileKind::FrostVent => (
                "❄",
                "#38BDF8",
                true,
                false,
                Some("Crypt Frost Vent (Chill Hazard)"),
            ),
            TileKind::EmberCrust => (
                "~",
                "#F97316",
                true,
                false,
                Some("Molten Ember Crust (Burn Hazard)"),
            ),
            TileKind::ShockRune => (
                "⚡",
                "#C084FC",
                true,
                false,
                Some("Arcane Shock Rune (Mana/HP Hazard)"),
            ),
        };
    Tile {
        x,
        y,
        kind,
        glyph: glyph.to_string(),
        color: color.to_string(),
        bg: None,
        walkable,
        blocks_sight,
        explored: false,
        visible: false,
        interact_id: None,
        used: None,
        revealed: if matches!(kind, TileKind::Trap | TileKind::DartTrap) {
            Some(false)
        } else {
            None
        },
        label: label.map(|s| s.to_string()),
    }
}

/// Local mirror of `ambient.makeCritter` (ambient.ts is not part of the
/// M1 source set; names/glyphs are the slice's own — see port report).
pub fn make_critter(kind: CritterKind, x: i32, y: i32, seq: u32) -> CritterState {
    let (name, glyph, color) = match kind {
        CritterKind::ValeRabbit => ("Vale Rabbit", "r", "#E5DEC9"),
        CritterKind::AshFox => ("Ash Fox", "f", "#F97316"),
        CritterKind::CinderCrow => ("Cinder Crow", "k", "#FB923C"),
        CritterKind::CaveBat => ("Cave Bat", "b", "#94A3B8"),
        CritterKind::TownCat => ("Town Cat", "c", "#FDE68A"),
        CritterKind::FenFrog => ("Fen Frog", "F", "#34D399"),
    };
    CritterState {
        id: format!("crit_{}_{seq}", kind.as_str()),
        kind,
        name: name.to_string(),
        glyph: glyph.to_string(),
        color: color.to_string(),
        x,
        y,
        home_x: x,
        home_y: y,
    }
}

// ------------------------------------------------------- town building

fn build_town_from_ascii(
    id: ZoneId,
    name: &str,
    subtitle: &str,
    rows: &[&str],
    npcs: Vec<NpcPlacement>,
    spawn_point: Pos,
    return_point: Pos,
) -> ZoneMap {
    let height = rows.len() as i32;
    let width = rows.first().map(|r| r.chars().count()).unwrap_or(0) as i32;
    let mut tiles: Vec<Vec<Tile>> = Vec::new();
    for (y, row_str) in rows.iter().enumerate() {
        let chars: Vec<char> = row_str.chars().collect();
        let mut row: Vec<Tile> = Vec::new();
        for x in 0..width as usize {
            let ch = chars.get(x).copied().unwrap_or('#');
            let kind = match ch {
                '#' => TileKind::Wall,
                '░' | '=' => TileKind::Road,
                '♣' => TileKind::Tree,
                '≈' => TileKind::Water,
                '~' => TileKind::Chasm,
                '◎' => TileKind::Fountain,
                'Π' => TileKind::MarketStall,
                '║' => TileKind::Pillar,
                '*' => TileKind::Torch,
                '┼' => TileKind::Shrine,
                '☼' => TileKind::Brazier,
                '$' => TileKind::Chest,
                '◈' => TileKind::Waystone,
                '✿' => TileKind::HerbNode,
                '%' => TileKind::SporePool,
                '+' => TileKind::Door,
                '>' => TileKind::StairsDown,
                'Ω' => TileKind::GateCity,
                '‡' => TileKind::LoreTablet,
                _ => TileKind::Floor,
            };
            let mut tile = make_tile(x as i32, y as i32, kind);
            if id == ZoneId::Highspire && kind == TileKind::Wall {
                tile.color = "#596377".to_string();
            }
            if id == ZoneId::Highspire && kind == TileKind::Road {
                tile.color = "#7C7465".to_string();
            }
            if kind == TileKind::Shrine {
                tile.interact_id = Some(match id {
                    ZoneId::Oakhaven => "respec_shrine".to_string(),
                    ZoneId::Highspire => "citadel_shrine".to_string(),
                    other => format!("{}_shrine", other.to_string_id()),
                });
            }
            if kind == TileKind::LoreTablet {
                tile.interact_id = Some(match id {
                    ZoneId::Oakhaven => "lore_oakhaven_post".to_string(),
                    ZoneId::Highspire => "lore_highspire_archive".to_string(),
                    other => format!("lore_{}_post", other.to_string_id()),
                });
            }
            tile.explored = id != ZoneId::Overworld;
            tile.visible = id != ZoneId::Overworld;
            row.push(tile);
        }
        tiles.push(row);
    }
    ZoneMap {
        id,
        name: name.to_string(),
        subtitle: subtitle.to_string(),
        is_town: true,
        depth: 0,
        width,
        height,
        tiles,
        enemies: vec![],
        npcs,
        critters: Some(vec![]),
        loot: vec![],
        spawn_point,
        return_point,
    }
}

fn npc(id: &str, x: i32, y: i32, color: &str, name: &str, title: &str) -> NpcPlacement {
    NpcPlacement {
        id: id.to_string(),
        x,
        y,
        glyph: "@".to_string(),
        color: color.to_string(),
        name: name.to_string(),
        title: title.to_string(),
        home_x: None,
        home_y: None,
        roams: None,
    }
}

fn roaming_npc(
    id: &str,
    x: i32,
    y: i32,
    color: &str,
    name: &str,
    title: &str,
) -> NpcPlacement {
    let mut n = npc(id, x, y, color, name, title);
    n.home_x = Some(x);
    n.home_y = Some(y);
    n.roams = Some(true);
    n
}

/// Blacksmith lot footprints, mirroring `modelLoader.ts`
/// `OAKHAVEN_LOT_ANCHORS[*].hideTerrainFootprint` (min_x, max_x, min_z,
/// max_z in tile coordinates; z is the tile row/y). The rebuilt
/// Blacksmith `.glb` mounts on the chosen lot and the terrain beneath
/// the footprint is hidden, so these rects must never drift from
/// modelLoader's anchors.
pub fn blacksmith_lot_footprint(lot: &str) -> Option<(i32, i32, i32, i32)> {
    match lot {
        "pondside" => Some((14, 18, 12, 14)),
        "market" => Some((10, 12, 4, 6)),
        "west_gate" => Some((3, 6, 8, 10)),
        _ => None,
    }
}

/// Torin's original forge landmark footprint in Oakhaven
/// (`landmark:torin_forge` in modelLoader.ts): tiles x 5..=9, z 1..=3.
/// Torin himself stands at (7, 4) — one row south of the mesh, outside
/// the building — never at (7, 2) inside it.
pub const TORIN_FORGE_FOOTPRINT: (i32, i32, i32, i32) = (5, 9, 1, 3);

pub fn create_oakhaven_map() -> ZoneMap {
    let layout: [&str; 17] = [
        "♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣",
        "♣♣...#####..♣....♣..#####..♣",
        "♣.♣..#...#........♣.#...#..♣",
        "♣....##+##..........##+##..♣",
        "♣......░.Π.Π..........░.♣..♣",
        "♣......░.......Π....✿.░✿.✿.♣",
        "♣..♣...░....*.*.....┼✿░*║.║♣",
        "Ω░░░░░░░░░░░░◎░░░░░░░░░░..░♣",
        "♣............░.........░>>░♣",
        "♣.♣.♣........░.........░░░*♣",
        "♣.....♣......░░░░░░░......♣♣",
        "♣....................≈≈≈≈..♣",
        "♣..###..✿.✿...#####.≈≈≈≈≈≈.♣",
        "♣..#.+...✿....#...+.≈≈≈≈≈≈≈♣",
        "♣..###........#####♣.≈≈≈≈≈≈≈♣",
        "♣..........♣..........≈≈≈≈.♣",
        "♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣♣",
    ];
    let npcs = vec![
        npc("torin", 7, 4, "#F59E0B", "Torin Ironhand", "Blacksmith & Forgemaster"),
        npc("mira", 22, 4, "#4EA885", "Herbalist Mira", "Apothecary"),
        npc("vance", 14, 8, "#60A5FA", "Warden Vance", "Outpost Captain"),
        npc("vex", 6, 13, "#C084FC", "Vex the Quiet", "Shadow Broker"),
        roaming_npc("pell", 10, 5, "#A8A29E", "Pell the Carter", "Outpost Carter"),
        roaming_npc("wren", 19, 8, "#FDE68A", "Sister Wren", "Shrine Keeper"),
    ];
    let mut town = build_town_from_ascii(
        ZoneId::Oakhaven,
        "Oakhaven Outpost",
        "Sunny Frontier Village",
        &layout,
        npcs,
        Pos::new(13, 9),
        Pos::new(13, 11),
    );
    town.tiles[7][0].label = Some("Silverwood Vale West Gate".to_string());
    // Phase 3 POI: the Old Outpost Oak — flavor landmark by the north wall.
    town.tiles[1][1].interact_id = Some("poi_old_oak".to_string());
    town.tiles[1][1].label = Some("The Old Outpost Oak".to_string());
    town.critters = Some(vec![make_critter(CritterKind::TownCat, 15, 6, 1)]);
    // Village pond in the south-east (Phase 7 village redo).
    let pond: [(usize, usize); 27] = [
        (21, 11), (22, 11), (23, 11), (24, 11),
        (20, 12), (21, 12), (22, 12), (23, 12), (24, 12), (25, 12),
        (20, 13), (21, 13), (22, 13), (23, 13), (24, 13), (25, 13), (26, 13),
        (21, 14), (22, 14), (23, 14), (24, 14), (25, 14), (26, 14),
        (22, 15), (23, 15), (24, 15), (25, 15),
    ];
    for (px, py) in pond {
        town.tiles[py][px].label = Some("Oakhaven Pond".to_string());
    }
    town
}

// ---------------------------------------------------------------- enemy

/// Kill XP trim (Audit Tier 1): full-clear XP paid ~13x the design, so
/// kill XP is trimmed once, at the creation funnel.
pub const KILL_XP_FACTOR: f64 = 0.45;

#[allow(clippy::too_many_arguments)]
fn base_enemy(
    seq: u32,
    archetype: EnemyArchetype,
    name: &str,
    glyph: &str,
    color: &str,
    x: i32,
    y: i32,
    hp: i32,
    atk: i32,
    def: i32,
    xp: i32,
    gold: i32,
    elite: bool,
    boss: bool,
    description: &str,
) -> Enemy {
    Enemy {
        id: format!("en_{seq}"),
        archetype,
        name: name.to_string(),
        glyph: glyph.to_string(),
        color: color.to_string(),
        x,
        y,
        hp,
        max_hp: hp,
        atk,
        def,
        xp_reward: xp,
        gold_reward: gold,
        is_boss: if boss { Some(true) } else { None },
        is_elite: if elite { Some(true) } else { None },
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
        description: description.to_string(),
    }
}

fn create_enemy_base(archetype: EnemyArchetype, x: i32, y: i32, depth: i32, seq: u32) -> Enemy {
    let abyss_bonus = if depth > 5 { (depth - 5) as f64 * 0.16 } else { 0.0 };
    let scale = 1.0 + (depth - 1) as f64 * 0.28 + abyss_bonus;
    let half = depth / 2; // TS Math.floor(depth / 2) for depth >= 0
    let s = |v: f64| round(v * scale);
    match archetype {
        EnemyArchetype::PlagueRat => base_enemy(seq, archetype, "Blight Rat", "r", "#A8A29E", x, y, s(32.0), s(7.0), 0, s(12.0), s(5.0), false, false, "Quick tunnel scavenger whose bite carries rot."),
        EnemyArchetype::GoblinSkirmisher => base_enemy(seq, archetype, "Goblin Skirmisher", "g", "#4EA885", x, y, s(46.0), s(9.0), 1 + half, s(18.0), s(9.0), false, false, "Nimble raider that telegraphs ferocious lunging slashes."),
        EnemyArchetype::CryptSkeleton => base_enemy(seq, archetype, "Crypt Sentinel", "s", "#E5DEC9", x, y, s(58.0), s(11.0), 2 + half, s(22.0), s(11.0), false, false, "Ancient guard animated by lingering oath-runes."),
        EnemyArchetype::BoneAcolyte => base_enemy(seq, archetype, "Bone Acolyte", "a", "#A78BFA", x, y, s(50.0), s(10.0), 1 + half, s(28.0), s(14.0), false, false, "Channels telegraphed Hex Bolts from afar."),
        EnemyArchetype::AshCultist => base_enemy(seq, archetype, "Ash Cultist", "c", "#FB923C", x, y, s(64.0), s(10.0), 2 + half, s(32.0), s(16.0), false, false, "Zealot of the Ember Vaults who ignites the stone."),
        EnemyArchetype::ShadowStalker => base_enemy(seq, archetype, "Shadow Stalker", "S", "#C084FC", x, y, s(69.0), s(12.0), 2 + half, s(38.0), s(20.0), true, false, "Elusive predator that winds up lethal backstabs."),
        EnemyArchetype::IronGolem => base_enemy(seq, archetype, "Runed Iron Golem", "G", "#94A3B8", x, y, s(104.0), s(13.0), 5 + depth, s(48.0), s(24.0), true, false, "Armored automaton that telegraphs earth-shaking Ground Slams."),
        EnemyArchetype::CinderWisp => base_enemy(seq, archetype, "Cinder Wisp", "w", "#FBBF24", x, y, s(41.0), s(10.0), 1, s(20.0), s(10.0), false, false, "Volatile spark summoned from deep obsidian hearths."),
        EnemyArchetype::DreadWraith => base_enemy(seq, archetype, "Dread Wraith", "W", "#38BDF8", x, y, s(71.0), s(11.0), 2 + half, s(42.0), s(22.0), true, false, "Frost-shrouded specter of fallen delvers that channels Soul Chill."),
        EnemyArchetype::BlightWeaver => base_enemy(seq, archetype, "Blight Weaver", "v", "#34D399", x, y, s(62.0), s(10.0), 2 + half, s(36.0), s(19.0), false, false, "Venom-scribing hexer whose bolts sap vitality from afar."),
        EnemyArchetype::ObsidianWarden => base_enemy(seq, archetype, "Obsidian Warden", "O", "#FB7185", x, y, s(120.0), s(14.0), 6 + depth, s(58.0), s(30.0), true, false, "Towering black-glass sentinel wielding crushing Obsidian Cleaves."),
        EnemyArchetype::GraveHusk => base_enemy(seq, archetype, "Grave Husk", "h", "#D97706", x, y, s(66.0), s(12.0), 3 + half, s(30.0), s(13.0), false, false, "Desiccated barrow-guard. Deathless: the first killing blow leaves it standing."),
        EnemyArchetype::SporeHulk => base_enemy(seq, archetype, "Spore Hulk", "H", "#84CC16", x, y, s(130.0), s(13.0), 4 + half, s(55.0), s(26.0), true, false, "Siege-sized bloom-mother. Bursts in a spore cloud when slain — kill it at range."),
        EnemyArchetype::RimeRevenant => base_enemy(seq, archetype, "Rime Revenant", "R", "#7DD3FC", x, y, s(56.0), s(12.0), 2 + half, s(40.0), s(21.0), true, false, "Frostpeak dead that refused the thaw. Its Rime Bolts Chill their target."),
        EnemyArchetype::BossBoneCantor => base_enemy(seq, archetype, "Vaelen, Bone Cantor", "B", "#F472B6", x, y, 620, 30, 5, 180, 65, false, true, "Lord of the Sunken Reliquary. Resonates with sorrowful dirges."),
        EnemyArchetype::BossAshenInquisitor => base_enemy(seq, archetype, "Inquisitor Serath", "I", "#FB923C", x, y, 1250, 45, 7, 280, 110, false, true, "High sentinel of the Ember Vaults wielding purifying cinder."),
        EnemyArchetype::BossMalakor => base_enemy(seq, archetype, "Malakor, Sovereign of Cinder", "M", "#EF4444", x, y, 1750, 55, 9, 550, 250, false, true, "The undying monarch anchored to the Obsidian Sanctum."),
    }
}

/// Every enemy spawn funnels through here: difficulty scaling is applied
/// exactly once at creation (via `difficulty::scale_enemy_for_difficulty`),
/// then the kill-XP trim. `seq` replaces TS's global id counter.
pub fn create_enemy(
    archetype: EnemyArchetype,
    x: i32,
    y: i32,
    depth: i32,
    tier: DifficultyTier,
    cycle: u32,
    seq: u32,
) -> Enemy {
    let mut enemy = scale_enemy_for_difficulty(
        create_enemy_base(archetype, x, y, depth, seq),
        tier,
        cycle,
    );
    enemy.xp_reward = (enemy.xp_reward as f64 * KILL_XP_FACTOR).round().max(1.0) as i32;
    enemy
}

const RARE_PROFILES: [(&str, [SkillId; 2]); 4] = [
    ("Pyromancer", [SkillId::EmberLance, SkillId::FrostNova]),
    ("Nightblade", [SkillId::Shadowstep, SkillId::VenomTip]),
    ("Juggernaut", [SkillId::ShieldBash, SkillId::Cleave]),
    ("Spellblade", [SkillId::EmberLance, SkillId::ShieldBash]),
];

/// Port of `promoteEnemyRank`: rare/randboss promotion with an
/// archetype-weighted skill profile drawn from `rng`.
pub fn promote_enemy_rank(
    mut enemy: Enemy,
    depth: i32,
    rank: EnemyRank,
    rng: &mut Rng,
) -> Enemy {
    let mut profile = RARE_PROFILES[rand_below(rng, 4) as usize];
    match enemy.archetype {
        EnemyArchetype::BoneAcolyte | EnemyArchetype::AshCultist | EnemyArchetype::CinderWisp => {
            profile = if rand_f(rng) < 0.65 { RARE_PROFILES[0] } else { RARE_PROFILES[3] };
        }
        EnemyArchetype::ShadowStalker | EnemyArchetype::GoblinSkirmisher => {
            profile = RARE_PROFILES[1];
        }
        EnemyArchetype::IronGolem | EnemyArchetype::CryptSkeleton => {
            profile = if rand_f(rng) < 0.7 { RARE_PROFILES[2] } else { RARE_PROFILES[3] };
        }
        _ => {}
    }
    let is_randboss = rank == EnemyRank::Randboss;
    let hp_mult = if is_randboss { 2.3 } else { 1.6 };
    let atk_mult = if is_randboss { 1.35 } else { 1.2 };
    let def_bonus = if is_randboss { 4 } else { 2 };
    let reward_mult = if is_randboss { 4.0 } else { 2.5 };

    enemy.name = format!(
        "{}, {}",
        enemy.name,
        if is_randboss { format!("Dread {}", profile.0) } else { profile.0.to_string() }
    );
    enemy.hp = round(enemy.hp as f64 * hp_mult);
    enemy.max_hp = round(enemy.max_hp as f64 * hp_mult);
    enemy.atk = round(enemy.atk as f64 * atk_mult);
    enemy.def += def_bonus;
    enemy.xp_reward = round(enemy.xp_reward as f64 * reward_mult);
    enemy.gold_reward = round(enemy.gold_reward as f64 * reward_mult);
    enemy.is_elite = Some(true);
    enemy.rank = Some(rank);
    enemy.rank_title = Some(format!(
        "{} · {}",
        if is_randboss { "Randboss" } else { "Rare" },
        profile.0
    ));
    enemy.description = format!(
        "{} Wields {} & {}.",
        enemy.description,
        profile.1[0].as_str().replace('_', " "),
        profile.1[1].as_str().replace('_', " ")
    );
    enemy.skill_loadout = Some(profile.1.to_vec());
    enemy.skill_cooldowns = Some(std::collections::HashMap::new());
    enemy.rune_charge = Some(is_randboss || depth >= 4);
    enemy
}

// ------------------------------------------------- local loot mirrors
// Mirrors of loot.ts calls used by world.ts (see module docs). These are
// NOT crate::loot; they exist so world.rs stays decoupled while the
// loot port lands in parallel. Call shapes match the TS originals:
// create_quest_item(tag), create_consumable(kind, quantity),
// generate_procedural_item(depth, { minRarity }).

use crate::types::ConsumableKind;

pub const RARITY_DEPTH_STEP: i32 = 5;
pub const RARITY_DEPTH_CAP: i32 = 50;

fn rarity_rank(r: ItemRarity) -> i32 {
    match r {
        ItemRarity::Common => 0,
        ItemRarity::Uncommon => 1,
        ItemRarity::Rare => 2,
        ItemRarity::Epic => 3,
        ItemRarity::Relic => 4,
    }
}

/// Mirror of loot.ts `rollRarity` (Nov 16 retuned curve).
pub fn roll_rarity_local(
    depth: i32,
    min_rarity: Option<ItemRarity>,
    rng: &mut Rng,
) -> ItemRarity {
    let roll = rand_f(rng) * 100.0
        + (depth.max(0) * RARITY_DEPTH_STEP).min(RARITY_DEPTH_CAP) as f64;
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
    let premium = matches!(min_rarity, Some(ItemRarity::Epic) | Some(ItemRarity::Relic));
    if picked == ItemRarity::Relic && !premium {
        picked = ItemRarity::Epic;
    }
    if let Some(min) = min_rarity {
        if rarity_rank(picked) < rarity_rank(min) {
            picked = min;
        }
    }
    picked
}

#[derive(Clone, Copy)]
enum StatKey {
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

fn add_stat(stats: &mut ItemStats, key: StatKey, amount: f64) {
    let inc = (amount.round() as i32).max(1);
    let inc_f = inc as f64;
    match key {
        StatKey::Atk => stats.atk = Some(stats.atk.unwrap_or(0) + inc),
        StatKey::Def => stats.def = Some(stats.def.unwrap_or(0) + inc),
        StatKey::SpellPower => stats.spell_power = Some(stats.spell_power.unwrap_or(0) + inc),
        StatKey::MaxHp => stats.max_hp = Some(stats.max_hp.unwrap_or(0) + inc),
        StatKey::MaxMana => stats.max_mana = Some(stats.max_mana.unwrap_or(0) + inc),
        StatKey::CritChance => stats.crit_chance = Some(stats.crit_chance.unwrap_or(0.0) + inc_f),
        StatKey::DodgeChance => {
            stats.dodge_chance = Some(stats.dodge_chance.unwrap_or(0.0) + inc_f)
        }
        StatKey::Lifesteal => stats.lifesteal = Some(stats.lifesteal.unwrap_or(0.0) + inc_f),
        StatKey::Thorns => stats.thorns = Some(stats.thorns.unwrap_or(0) + inc),
    }
}

fn scale_for_stat(key: StatKey, base_power: f64) -> f64 {
    match key {
        StatKey::MaxHp => base_power * 3.0,
        StatKey::MaxMana => base_power * 2.0,
        StatKey::CritChance | StatKey::DodgeChance => {
            (base_power * 1.4).round().clamp(2.0, 18.0)
        }
        StatKey::Lifesteal => (base_power * 0.7).round().clamp(2.0, 10.0),
        _ => base_power,
    }
}

struct SlotBase {
    name: &'static str,
    glyph: &'static str,
    primary: StatKey,
    secondary: Option<StatKey>,
}

fn slot_bases(slot: ItemSlot) -> &'static [SlotBase] {
    match slot {
        ItemSlot::Weapon => &[
            SlotBase { name: "Iron Longsword", glyph: "/", primary: StatKey::Atk, secondary: None },
            SlotBase { name: "Obsidian Dagger", glyph: "†", primary: StatKey::Atk, secondary: Some(StatKey::CritChance) },
            SlotBase { name: "Runed Ash Staff", glyph: "⚚", primary: StatKey::SpellPower, secondary: Some(StatKey::MaxMana) },
            SlotBase { name: "Halberd", glyph: "↑", primary: StatKey::Atk, secondary: Some(StatKey::Def) },
            SlotBase { name: "Cinder Falchion", glyph: "⚔", primary: StatKey::Atk, secondary: Some(StatKey::SpellPower) },
        ],
        ItemSlot::Armor => &[
            SlotBase { name: "Brigandine Coat", glyph: "⛨", primary: StatKey::Def, secondary: Some(StatKey::MaxHp) },
            SlotBase { name: "Shadowweave Vest", glyph: "▒", primary: StatKey::Def, secondary: Some(StatKey::DodgeChance) },
            SlotBase { name: "Plate Cuirass", glyph: "☗", primary: StatKey::Def, secondary: Some(StatKey::Thorns) },
            SlotBase { name: "Archivist Robes", glyph: "☥", primary: StatKey::SpellPower, secondary: Some(StatKey::MaxMana) },
        ],
        ItemSlot::Helm => &[
            SlotBase { name: "Barred Sallet", glyph: "⍾", primary: StatKey::Def, secondary: Some(StatKey::MaxHp) },
            SlotBase { name: "Raven Hood", glyph: "∩", primary: StatKey::DodgeChance, secondary: Some(StatKey::CritChance) },
            SlotBase { name: "Rune Circlet", glyph: "⍟", primary: StatKey::SpellPower, secondary: Some(StatKey::MaxMana) },
        ],
        ItemSlot::Ring => &[
            SlotBase { name: "Signet Band", glyph: "°", primary: StatKey::Atk, secondary: Some(StatKey::CritChance) },
            SlotBase { name: "Opal Loop", glyph: "◦", primary: StatKey::SpellPower, secondary: Some(StatKey::MaxMana) },
            SlotBase { name: "Bastion Ring", glyph: "○", primary: StatKey::Def, secondary: Some(StatKey::MaxHp) },
        ],
        ItemSlot::Amulet => &[
            SlotBase { name: "Amber Pendant", glyph: "◈", primary: StatKey::MaxHp, secondary: Some(StatKey::Def) },
            SlotBase { name: "Aether Locket", glyph: "❖", primary: StatKey::MaxMana, secondary: Some(StatKey::SpellPower) },
            SlotBase { name: "Tooth Talisman", glyph: "◊", primary: StatKey::CritChance, secondary: Some(StatKey::Lifesteal) },
        ],
    }
}

const PREFIXES: [(&str, StatKey, f64); 8] = [
    ("Keen", StatKey::Atk, 1.2),
    ("Tempered", StatKey::Def, 1.2),
    ("Aetherial", StatKey::SpellPower, 1.25),
    ("Stalwart", StatKey::MaxHp, 2.5),
    ("Whispering", StatKey::DodgeChance, 1.4),
    ("Wicked", StatKey::CritChance, 1.5),
    ("Vampiric", StatKey::Lifesteal, 1.0),
    ("Barbed", StatKey::Thorns, 1.3),
];

const SUFFIXES: [(&str, StatKey, f64); 7] = [
    ("of Ash", StatKey::SpellPower, 1.0),
    ("of the Bastion", StatKey::Def, 1.0),
    ("of Precision", StatKey::CritChance, 1.3),
    ("of Resolve", StatKey::MaxHp, 2.2),
    ("of Echoes", StatKey::MaxMana, 1.8),
    ("of Ruin", StatKey::Atk, 1.1),
    ("of Mists", StatKey::DodgeChance, 1.2),
];

const AFFIX_POOL: [(ItemAffixId, &str); 8] = [
    (ItemAffixId::BleedOnCrit, "Passive: Critical hits inflict Bleed (2t)"),
    (ItemAffixId::ManaOnKill, "Passive: Slaying a foe restores 4 Mana"),
    (ItemAffixId::ThornsBurst, "Passive: Reflects +4 extra damage when hit"),
    (ItemAffixId::FirstStrike, "Passive: First hit in each skirmish deals +35% dmg"),
    (ItemAffixId::ExecuteBonus, "Passive: +25% damage to foes below 40% HP"),
    (ItemAffixId::ChillOnHit, "Passive: Strikes have 30% chance to Chill target"),
    (ItemAffixId::VampiricTouch, "Passive: Heals 3 HP on every Critical Hit"),
    (ItemAffixId::ArcaneWard, "Passive: Start floor with Shielded (+4 DEF, 4t)"),
];

fn blank_item(seq: u32, name: &str, category: ItemCategory, rarity: ItemRarity, glyph: &str, value: i32, description: &str) -> Item {
    Item {
        id: format!("itm_{seq}"),
        name: name.to_string(),
        category,
        slot: None,
        consumable_kind: None,
        rarity,
        glyph: glyph.to_string(),
        level_req: 1,
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

/// Mirror of loot.ts `generateProceduralItem(depth, { minRarity })`.
pub fn generate_procedural_item_local(
    depth: i32,
    min_rarity: Option<ItemRarity>,
    rng: &mut Rng,
    seq: u32,
) -> Item {
    const SLOTS: [ItemSlot; 5] = [
        ItemSlot::Weapon,
        ItemSlot::Armor,
        ItemSlot::Helm,
        ItemSlot::Ring,
        ItemSlot::Amulet,
    ];
    let slot = SLOTS[rand_below(rng, 5) as usize];
    let rarity = roll_rarity_local(depth, min_rarity, rng);
    let rarity_scale = match rarity {
        ItemRarity::Common => 1.0,
        ItemRarity::Uncommon => 1.35,
        ItemRarity::Rare => 1.75,
        ItemRarity::Epic => 2.2,
        ItemRarity::Relic => 2.7,
    };
    let bases = slot_bases(slot);
    let base = &bases[rand_below(rng, bases.len() as i32) as usize];
    let power = round((1.8 + depth as f64 * 1.1) * rarity_scale).max(1);

    let mut stats = ItemStats::default();
    add_stat(&mut stats, base.primary, scale_for_stat(base.primary, power as f64));
    if let Some(secondary) = base.secondary {
        if rarity != ItemRarity::Common {
            add_stat(
                &mut stats,
                secondary,
                scale_for_stat(secondary, power as f64 * 0.55).round().max(1.0),
            );
        }
    }
    let mut name = base.name.to_string();
    if rarity != ItemRarity::Common {
        let prefix = PREFIXES[rand_below(rng, 8) as usize];
        name = format!("{} {}", prefix.0, name);
        add_stat(&mut stats, prefix.1, power as f64 * 0.65 * prefix.2);
    }
    if matches!(rarity, ItemRarity::Rare | ItemRarity::Epic | ItemRarity::Relic) {
        let suffix = SUFFIXES[rand_below(rng, 7) as usize];
        name = format!("{} {}", name, suffix.0);
        add_stat(&mut stats, suffix.1, power as f64 * 0.6 * suffix.2);
    }
    let (affix, affix_label) = if matches!(rarity, ItemRarity::Rare | ItemRarity::Epic | ItemRarity::Relic) {
        let picked = AFFIX_POOL[rand_below(rng, 8) as usize];
        (Some(picked.0), Some(picked.1.to_string()))
    } else {
        (None, None)
    };
    let value = round(14.0 * rarity_scale + depth as f64 * 10.0);
    let origin = match depth {
        1 => "Moss-Sunken Reliquary",
        2 => "Frost-Bound Ossuary",
        3 => "Ember Vaults",
        4 => "Inquisitor Crucible",
        5 => "Obsidian Sanctum",
        d if d >= 6 => "Endless Abyss",
        _ => "outer catacombs",
    };
    let mut item = blank_item(
        seq,
        &name,
        ItemCategory::Equipment,
        rarity,
        base.glyph,
        value,
        &format!("{} {} recovered from the {}.", rarity.as_str(), slot.as_str(), origin),
    );
    item.slot = Some(slot);
    item.level_req = depth.max(1);
    item.stats = stats;
    item.affix = affix;
    item.affix_label = affix_label;
    item
}

/// Mirror of loot.ts `createConsumable(kind, quantity)`.
pub fn create_consumable_local(kind: ConsumableKind, quantity: i32, seq: u32) -> Item {
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
    let mut item = blank_item(seq, name, ItemCategory::Consumable, rarity, glyph, value, description);
    item.consumable_kind = Some(kind);
    item.quantity = Some(quantity);
    item
}

/// Mirror of loot.ts `createQuestItem(tag)`.
pub fn create_quest_item_local(tag: &str, seq: u32) -> Item {
    let (name, rarity, glyph, description) = match tag {
        "smuggler_ledger" => ("Sealed Contraband Ledger", ItemRarity::Rare, "▤", "Wax-sealed manifest naming Citadel officials trading in crypt relics."),
        "pale_nightbloom" => ("Pale Nightbloom", ItemRarity::Uncommon, "✿", "Luminescent moss-flower that neutralizes crypt miasma."),
        "wayfarer_satchel" => ("Wayfarer’s Stolen Satchel", ItemRarity::Rare, "▤", "Rowan’s weathered leather map-satchel, plundered by Silverbridge brigands."),
        "morvath_seal" => ("Morvath’s Barrow Seal", ItemRarity::Epic, "◈", "Tarnished iron crown-seal taken from Arch-Wight Morvath in Whispering Barrow."),
        "fungal_heart" => ("Bioluminescent Fungal Heart", ItemRarity::Epic, "✿", "Pulsing core harvested from Spore-Matron Vexis deep inside Fungal Hollow."),
        "ashen_ore_heart" => ("Ashen Ore Heart", ItemRarity::Relic, "♦", "White-hot meteoric slag-core torn from Ignis-Warden Kaelthar in Ashen Caldera."),
        "blacksmith_anvil" => ("Torin’s Lost Anvil", ItemRarity::Rare, "▬", "Torin’s original anvil, dragged into the Reliquary when the old forge fell. One of three pieces needed to rebuild the Blacksmith."),
        "blacksmith_bellows" => ("Great Forge Bellows", ItemRarity::Rare, "≋", "Paired oak-and-hide bellows big enough to wake a cold forge. One of three pieces needed to rebuild the Blacksmith."),
        "blacksmith_sign" => ("The Smith’s Hanging Sign", ItemRarity::Uncommon, "▤", "A hammer-and-star sign, paint still bright. A forge without a sign is just a man shouting at metal. One of three pieces needed to rebuild the Blacksmith."),
        "heartcoal" => ("Heartcoal", ItemRarity::Epic, "♦", "A coal that has burned since before Oakhaven had a name. Torin’s forge is STALLED without it — cold iron lights for no one."),
        "barrow_iron" => ("Whispering Barrow Iron", ItemRarity::Rare, "▬", "Grave-tempered iron ingots recovered from Whispering Barrow. Banking them in Oakhaven grows population (+4) and unlocks the Barrow-Iron Hauberk at Torin’s rebuilt Blacksmith."),
        "mire_timber" => ("Thornwick Ironwood Timber", ItemRarity::Rare, "≡", "Rot-proof ironwood heartbeams harvested from Fungal Hollow. Banking them in Oakhaven grows population (+4) and unlocks the Mire-Timber Greatbow at Torin’s rebuilt Blacksmith."),
        "frost_coal" => ("Frostpeak White-Coal", ItemRarity::Epic, "♦", "Glacier-veined anthracite mined in Frostpeak Cavern. Banking it in Oakhaven grows population (+4) and unlocks the Frost-Quenched Greathelm at Torin’s rebuilt Blacksmith."),
        _ => ("Whispering Bell", ItemRarity::Relic, "⍾", "Ancient bronze bell humming with dormant ember resonance."),
    };
    let quest_tag = if matches!(
        tag,
        "smuggler_ledger" | "pale_nightbloom" | "wayfarer_satchel" | "morvath_seal"
            | "fungal_heart" | "ashen_ore_heart" | "blacksmith_anvil" | "blacksmith_bellows"
            | "blacksmith_sign" | "heartcoal" | "barrow_iron" | "mire_timber" | "frost_coal"
    ) {
        tag
    } else {
        "whispering_bell"
    };
    let mut item = blank_item(seq, name, ItemCategory::Quest, rarity, glyph, 0, description);
    item.quest_tag = Some(quest_tag.to_string());
    item
}

// ------------------------------------------------------------ generator

#[derive(Debug, Clone, Copy)]
struct Room {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    cx: i32,
    cy: i32,
}

fn floor_tint(depth: i32) -> Option<&'static str> {
    if depth >= 6 {
        Some("#3B3054")
    } else if depth >= 3 {
        Some("#483D3B")
    } else {
        None
    }
}

fn carve_floor(tiles: &mut [Vec<Tile>], x: i32, y: i32, depth: i32) {
    let mut t = make_tile(x, y, TileKind::Floor);
    if let Some(tint) = floor_tint(depth) {
        t.color = tint.to_string();
    }
    tiles[y as usize][x as usize] = t;
}

fn paint_room_tint(tiles: &mut [Vec<Tile>], room: &Room, tint: &str) {
    for ry in room.y..room.y + room.h {
        for rx in room.x..room.x + room.w {
            if tiles[ry as usize][rx as usize].kind == TileKind::Floor {
                tiles[ry as usize][rx as usize].color = tint.to_string();
            }
        }
    }
}

fn set_tile(
    tiles: &mut [Vec<Tile>],
    x: i32,
    y: i32,
    kind: TileKind,
    label: Option<&str>,
    interact_id: Option<String>,
) {
    let mut t = make_tile(x, y, kind);
    if let Some(l) = label {
        t.label = Some(l.to_string());
    }
    t.interact_id = interact_id;
    tiles[y as usize][x as usize] = t;
}

/// TS `placeSetPieceEnemy`: place a set-piece guard on the first spot
/// that is plain floor (TS `setPieceTileOk`: never the spawn/return
/// tile) and not already holding a set-piece enemy. At call time
/// `enemies` contains only set-piece enemies, as in TS.
#[allow(clippy::too_many_arguments)]
fn place_set_piece_enemy(
    tiles: &[Vec<Tile>],
    enemies: &mut Vec<Enemy>,
    enemy_seq: &mut u32,
    arch: EnemyArchetype,
    spots: &[(i32, i32)],
    spawn_point: Pos,
    return_point: Pos,
    depth: i32,
    tier: DifficultyTier,
    cycle: u32,
) {
    for &(sx, sy) in spots {
        let tile_ok = tiles
            .get(sy as usize)
            .and_then(|r| r.get(sx as usize))
            .map_or(false, |t| t.kind == TileKind::Floor)
            && !(sx == spawn_point.x && sy == spawn_point.y)
            && !(sx == return_point.x && sy == return_point.y);
        if tile_ok && !enemies.iter().any(|e| e.x == sx && e.y == sy) {
            *enemy_seq += 1;
            enemies.push(create_enemy(arch, sx, sy, depth, tier, cycle, *enemy_seq));
            return;
        }
    }
}

/// TS `setTileMeta` (overworld): replace a tile wholesale and stamp its
/// interact id + label.
fn set_tile_meta(zone: &mut ZoneMap, x: i32, y: i32, kind: TileKind, interact_id: &str, label: &str) {
    let mut t = make_tile(x, y, kind);
    t.interact_id = Some(interact_id.to_string());
    t.label = Some(label.to_string());
    zone.tiles[y as usize][x as usize] = t;
}

/// 4-directional flood fill over walkable tiles (TS `getReachable`).
pub fn reachable_positions(zone: &ZoneMap, sx: i32, sy: i32) -> HashSet<(i32, i32)> {
    reachable_in_tiles(&zone.tiles, zone.width, zone.height, sx, sy)
}

fn reachable_in_tiles(
    tiles: &[Vec<Tile>],
    width: i32,
    height: i32,
    sx: i32,
    sy: i32,
) -> HashSet<(i32, i32)> {
    let mut visited: HashSet<(i32, i32)> = HashSet::new();
    let mut queue: Vec<(i32, i32)> = vec![(sx, sy)];
    visited.insert((sx, sy));
    while let Some((cx, cy)) = queue.pop() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (cx + dx, cy + dy);
            if nx < 0 || nx >= width || ny < 0 || ny >= height || visited.contains(&(nx, ny)) {
                continue;
            }
            if tiles[ny as usize][nx as usize].walkable {
                visited.insert((nx, ny));
                queue.push((nx, ny));
            }
        }
    }
    visited
}

fn pool_for(depth: i32) -> &'static [EnemyArchetype] {
    match depth {
        1 => &[
            EnemyArchetype::PlagueRat,
            EnemyArchetype::GoblinSkirmisher,
            EnemyArchetype::CryptSkeleton,
        ],
        2 => &[
            EnemyArchetype::GoblinSkirmisher,
            EnemyArchetype::CryptSkeleton,
            EnemyArchetype::BoneAcolyte,
            EnemyArchetype::BlightWeaver,
        ],
        3 => &[
            EnemyArchetype::CryptSkeleton,
            EnemyArchetype::BoneAcolyte,
            EnemyArchetype::AshCultist,
            EnemyArchetype::ShadowStalker,
            EnemyArchetype::BlightWeaver,
            EnemyArchetype::DreadWraith,
        ],
        4 => &[
            EnemyArchetype::AshCultist,
            EnemyArchetype::ShadowStalker,
            EnemyArchetype::IronGolem,
            EnemyArchetype::CinderWisp,
            EnemyArchetype::DreadWraith,
            EnemyArchetype::ObsidianWarden,
        ],
        5 => &[
            EnemyArchetype::AshCultist,
            EnemyArchetype::ShadowStalker,
            EnemyArchetype::IronGolem,
            EnemyArchetype::DreadWraith,
            EnemyArchetype::ObsidianWarden,
        ],
        d if d >= 6 => &[
            EnemyArchetype::AshCultist,
            EnemyArchetype::ShadowStalker,
            EnemyArchetype::IronGolem,
            EnemyArchetype::CinderWisp,
            EnemyArchetype::BoneAcolyte,
            EnemyArchetype::DreadWraith,
            EnemyArchetype::BlightWeaver,
            EnemyArchetype::ObsidianWarden,
            EnemyArchetype::GraveHusk,
            EnemyArchetype::SporeHulk,
            EnemyArchetype::RimeRevenant,
        ],
        _ => &[
            EnemyArchetype::PlagueRat,
            EnemyArchetype::GoblinSkirmisher,
            EnemyArchetype::CryptSkeleton,
        ],
    }
}

/// Branch of a side dungeon (TS `generateSideDungeonFloor`'s
/// `'barrow' | 'mire' | 'frost'` string union).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideDungeonBranch {
    Barrow,
    Mire,
    Frost,
}

impl SideDungeonBranch {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Barrow => "barrow",
            Self::Mire => "mire",
            Self::Frost => "frost",
        }
    }
}

/// TS `sidePools`: side dungeons draw from their own biome pools so each
/// branch has a distinct roster.
fn side_pool_for(branch: SideDungeonBranch) -> &'static [EnemyArchetype] {
    match branch {
        SideDungeonBranch::Barrow => &[
            EnemyArchetype::CryptSkeleton,
            EnemyArchetype::GraveHusk,
            EnemyArchetype::BoneAcolyte,
            EnemyArchetype::GraveHusk,
            EnemyArchetype::CryptSkeleton,
            EnemyArchetype::PlagueRat,
        ],
        SideDungeonBranch::Mire => &[
            EnemyArchetype::BlightWeaver,
            EnemyArchetype::SporeHulk,
            EnemyArchetype::PlagueRat,
            EnemyArchetype::BlightWeaver,
            EnemyArchetype::SporeHulk,
            EnemyArchetype::AshCultist,
        ],
        SideDungeonBranch::Frost => &[
            EnemyArchetype::DreadWraith,
            EnemyArchetype::RimeRevenant,
            EnemyArchetype::RimeRevenant,
            EnemyArchetype::IronGolem,
            EnemyArchetype::ObsidianWarden,
            EnemyArchetype::CinderWisp,
        ],
    }
}

/// Port of `generateDungeonFloor` for the main descent.
pub fn generate_dungeon_floor(
    depth: i32,
    rng: &mut Rng,
    tier: DifficultyTier,
    cycle: u32,
) -> ZoneMap {
    generate_dungeon_floor_impl(depth, rng, tier, cycle, false, None)
}

/// The shared generator behind `generate_dungeon_floor` and
/// `generate_side_dungeon_floor`. `skip_story_boss` / `side_branch` are
/// TS's `options.skipStoryBoss` / `options.sideBranch`: side dungeons
/// reuse this generator at an effective depth but must not inherit the
/// main descent's story nodes, bosses or town items.
#[allow(clippy::too_many_arguments)]
fn generate_dungeon_floor_impl(
    depth: i32,
    rng: &mut Rng,
    tier: DifficultyTier,
    cycle: u32,
    skip_story_boss: bool,
    side_branch: Option<SideDungeonBranch>,
) -> ZoneMap {
    let width: i32 = 46;
    let height: i32 = 32;
    let mut tiles: Vec<Vec<Tile>> = (0..height)
        .map(|y| (0..width).map(|x| make_tile(x, y, TileKind::Wall)).collect())
        .collect();
    let mut enemy_seq: u32 = 1;
    let mut item_seq: u32 = 100;
    let next_enemy = |seq: &mut u32| {
        *seq += 1;
        *seq
    };

    let (name, subtitle) = match depth {
        1 => ("Moss-Sunken Reliquary", "Floor 1 · Blight Spore Pools"),
        2 => ("Frost-Bound Ossuary", "Floor 2 · Cantor Crypt & Frost Vents"),
        3 => ("The Ember Vaults", "Floor 3 · Basalt Halls & Molten Crusts"),
        4 => ("Inquisitor Crucible", "Floor 4 · Brass Forges & Shock Runes"),
        5 => ("The Obsidian Sanctum", "Floor 5 · Sovereign Throne"),
        _ => ("The Endless Abyss", "Abyssal Depths"),
    };
    let subtitle = if depth >= 6 {
        format!("Floor {depth} · Abyssal Depths")
    } else {
        subtitle.to_string()
    };

    // --- rooms -------------------------------------------------------
    let mut rooms: Vec<Room> = Vec::new();
    let target_rooms = (9 + depth).min(18);
    let mut attempt = 0;
    while attempt < 95 && (rooms.len() as i32) < target_rooms {
        attempt += 1;
        let w = 5 + rand_below(rng, 5);
        let h = 4 + rand_below(rng, 4);
        let x = 2 + rand_below(rng, width - w - 4);
        let y = 2 + rand_below(rng, height - h - 4);
        let overlaps = rooms
            .iter()
            .any(|r| x <= r.x + r.w + 1 && x + w + 1 >= r.x && y <= r.y + r.h + 1 && y + h + 1 >= r.y);
        if overlaps {
            continue;
        }
        rooms.push(Room { x, y, w, h, cx: x + w / 2, cy: y + h / 2 });
        for ry in y..y + h {
            for rx in x..x + w {
                carve_floor(&mut tiles, rx, ry, depth);
            }
        }
    }

    // --- corridors (sequential L: x first, then y) --------------------
    for i in 1..rooms.len() {
        let (a, b) = (rooms[i - 1], rooms[i]);
        let (mut cx, mut cy) = (a.cx, a.cy);
        while cx != b.cx {
            carve_floor(&mut tiles, cx, cy, depth);
            cx += if cx < b.cx { 1 } else { -1 };
        }
        while cy != b.cy {
            carve_floor(&mut tiles, cx, cy, depth);
            cy += if cy < b.cy { 1 } else { -1 };
        }
    }

    let start_room = rooms[0];
    let end_room = rooms[rooms.len() - 1];
    let spawn_point = Pos::new(start_room.cx, start_room.cy);
    let return_point = Pos::new(end_room.cx, end_room.cy);
    tiles[spawn_point.y as usize][spawn_point.x as usize] =
        make_tile(spawn_point.x, spawn_point.y, TileKind::StairsUp);
    tiles[return_point.y as usize][return_point.x as usize] =
        make_tile(return_point.x, return_point.y, TileKind::StairsDown);
    if depth == 5 && !skip_story_boss {
        tiles[return_point.y as usize][return_point.x as usize].label =
            Some("Abyssal Descent (Unlocks after Malakor falls)".to_string());
    } else if depth >= 6 && !skip_story_boss {
        tiles[return_point.y as usize][return_point.x as usize].label =
            Some(format!("Abyssal Stair (Floor {})", depth + 1));
    }

    // --- torches & pillars -------------------------------------------
    for (idx, room) in rooms.iter().enumerate() {
        if room.w >= 7 && room.h >= 6 && idx != 0 && idx != rooms.len() - 1 {
            tiles[(room.y + 1) as usize][(room.x + 1) as usize] =
                make_tile(room.x + 1, room.y + 1, TileKind::Torch);
            tiles[(room.y + room.h - 2) as usize][(room.x + room.w - 2) as usize] =
                make_tile(room.x + room.w - 2, room.y + room.h - 2, TileKind::Pillar);
        } else if rand_f(rng) < 0.65 {
            let above_walkable = tiles
                .get((room.y - 1) as usize)
                .and_then(|r| r.get(room.x as usize))
                .map_or(false, |t| t.walkable);
            let left_walkable = tiles
                .get(room.y as usize)
                .and_then(|r| r.get((room.x - 1) as usize))
                .map_or(false, |t| t.walkable);
            if !above_walkable
                && !left_walkable
                && tiles[room.y as usize][room.x as usize].kind == TileKind::Floor
            {
                tiles[room.y as usize][room.x as usize] =
                    make_tile(room.x, room.y, TileKind::Torch);
            }
        }
    }

    // --- story quest nodes (main descent only) ------------------------
    // Story nodes belong only to the main descent: side dungeons reuse
    // this generator at effective depths 1-3 and must not inherit them
    // (TS gates all three on `!options?.skipStoryBoss`).
    if depth == 1 && rooms.len() > 2 && !skip_story_boss {
        let cache = rooms[2];
        set_tile(
            &mut tiles,
            cache.cx,
            cache.cy,
            TileKind::ContrabandCache,
            None,
            Some("contraband_cache".to_string()),
        );
    }
    if (depth == 1 || depth == 2) && rooms.len() > 3 && !skip_story_boss {
        let herb = rooms[3];
        set_tile(
            &mut tiles,
            herb.cx,
            herb.cy,
            TileKind::HerbNode,
            None,
            Some("herb_node".to_string()),
        );
    }
    if depth == 3 && rooms.len() > 1 && !skip_story_boss {
        let mid = rooms[rooms.len() / 2];
        set_tile(
            &mut tiles,
            mid.cx,
            mid.cy,
            TileKind::WhisperingBell,
            None,
            Some("whispering_bell".to_string()),
        );
    }

    // --- special landmark rooms (waystone, forge / altar) -------------
    let mut reserved: HashSet<usize> = HashSet::new();
    reserved.insert(0);
    reserved.insert(rooms.len() - 1);
    if depth == 1 && rooms.len() > 2 {
        reserved.insert(2);
    }
    if (depth == 1 || depth == 2) && rooms.len() > 3 {
        reserved.insert(3);
    }
    if depth == 3 && rooms.len() > 1 {
        reserved.insert(rooms.len() / 2);
    }
    let mut candidates: Vec<usize> = (0..rooms.len())
        .filter(|idx| !reserved.contains(idx))
        .collect();
    let mut special_rooms: HashSet<usize> = HashSet::new();
    if !candidates.is_empty() && (depth <= 4 || rand_f(rng) < 0.6) {
        let pick = candidates.remove(0);
        special_rooms.insert(pick);
        let room = rooms[pick];
        paint_room_tint(&mut tiles, &room, "#284552");
        set_tile(
            &mut tiles,
            room.cx,
            room.cy,
            TileKind::Waystone,
            None,
            Some(format!("waystone_{depth}")),
        );
    }
    if !candidates.is_empty() {
        let pick = candidates.remove(0);
        special_rooms.insert(pick);
        let room = rooms[pick];
        let kind = if depth % 2 == 1 {
            TileKind::ForgottenForge
        } else {
            TileKind::CursedAltar
        };
        paint_room_tint(
            &mut tiles,
            &room,
            if kind == TileKind::ForgottenForge { "#452A21" } else { "#342347" },
        );
        set_tile(&mut tiles, room.cx, room.cy, kind, None, None);
    }

    // --- set pieces (Phase 3): one hand-built room per floor ----------
    // Kind by branch/depth, mirroring TS: mire -> Spore-Heart Chamber,
    // frost -> Rime Circle, barrow / Floor 1 -> Whispering Sepulcher,
    // main descent otherwise -> Sealed Brazier Hall. Set pieces only
    // redecorate a spare room's interior, so the connectivity guarantee
    // below still holds (never the first or final room).
    let mut set_piece_rooms: HashSet<usize> = HashSet::new();
    let mut enemies: Vec<Enemy> = Vec::new();
    {
        // TS setPieceTileOk: plain floor only, never spawn/return.
        let tile_ok = |tiles: &[Vec<Tile>], x: i32, y: i32| -> bool {
            tiles
                .get(y as usize)
                .and_then(|r| r.get(x as usize))
                .map_or(false, |t| t.kind == TileKind::Floor)
                && !(x == spawn_point.x && y == spawn_point.y)
                && !(x == return_point.x && y == return_point.y)
        };
        // TS setPieceDecorOk: decor may also overwrite the generator's
        // decorative pillars and torches — but never gameplay tiles.
        let decor_ok = |tiles: &[Vec<Tile>], x: i32, y: i32| -> bool {
            tiles
                .get(y as usize)
                .and_then(|r| r.get(x as usize))
                .map_or(false, |t| {
                    matches!(t.kind, TileKind::Floor | TileKind::Pillar | TileKind::Torch)
                })
                && !(x == spawn_point.x && y == spawn_point.y)
                && !(x == return_point.x && y == return_point.y)
        };
        #[derive(Clone, Copy, PartialEq)]
        enum SetPieceKind {
            Sepulcher,
            SporeHeart,
            RimeCircle,
            SealedBrazier,
        }
        let mut kind = match side_branch {
            Some(SideDungeonBranch::Mire) => SetPieceKind::SporeHeart,
            Some(SideDungeonBranch::Frost) => SetPieceKind::RimeCircle,
            Some(SideDungeonBranch::Barrow) => SetPieceKind::Sepulcher,
            None if depth == 1 => SetPieceKind::Sepulcher,
            None => SetPieceKind::SealedBrazier,
        };
        // TS findSetPieceRoom: largest qualifying room (stable sort, so
        // the earliest room wins area ties).
        let find_room = |min_w: i32, min_h: i32| -> Option<(usize, Room)> {
            let mut cands: Vec<(usize, Room)> = rooms
                .iter()
                .enumerate()
                .filter(|(idx, r)| {
                    !reserved.contains(idx)
                        && !special_rooms.contains(idx)
                        && r.w >= min_w
                        && r.h >= min_h
                })
                .map(|(idx, r)| (idx, *r))
                .collect();
            cands.sort_by_key(|(_, r)| std::cmp::Reverse(r.w * r.h));
            cands.into_iter().next()
        };
        let mut pick = if kind == SetPieceKind::SealedBrazier {
            find_room(7, 6)
        } else {
            find_room(6, 5)
        };
        if pick.is_none() && kind == SetPieceKind::SealedBrazier {
            kind = SetPieceKind::Sepulcher;
            pick = find_room(6, 5);
        }
        if let Some((idx, room)) = pick {
            set_piece_rooms.insert(idx);
            let (rx0, ry0, rw, rh, cx, cy) =
                (room.x, room.y, room.w, room.h, room.cx, room.cy);
            match kind {
                SetPieceKind::Sepulcher => {
                    // Whispering Sepulcher: pillar corners, twin
                    // sarcophagi guarded by trapped approaches, and an
                    // epitaph stele at the heart.
                    paint_room_tint(&mut tiles, &room, "#3A362F");
                    for (px, py) in [
                        (rx0 + 1, ry0 + 1),
                        (rx0 + rw - 2, ry0 + 1),
                        (rx0 + 1, ry0 + rh - 2),
                        (rx0 + rw - 2, ry0 + rh - 2),
                    ] {
                        if decor_ok(&tiles, px, py) {
                            set_tile(
                                &mut tiles,
                                px,
                                py,
                                TileKind::Pillar,
                                Some("Sepulcher Pillar"),
                                None,
                            );
                        }
                    }
                    if tile_ok(&tiles, cx - 2, cy) {
                        set_tile(
                            &mut tiles,
                            cx - 2,
                            cy,
                            TileKind::Chest,
                            Some("Stone Sarcophagus"),
                            None,
                        );
                    }
                    if tile_ok(&tiles, cx + 2, cy) {
                        set_tile(
                            &mut tiles,
                            cx + 2,
                            cy,
                            TileKind::Chest,
                            Some("Stone Sarcophagus"),
                            None,
                        );
                    }
                    if tile_ok(&tiles, cx, cy + 1) {
                        set_tile(
                            &mut tiles,
                            cx,
                            cy + 1,
                            TileKind::LoreTablet,
                            Some("Sepulcher Epitaph"),
                            Some(format!("lore_floor_{depth}")),
                        );
                    }
                    if tile_ok(&tiles, cx - 2, cy + 1) {
                        set_tile(&mut tiles, cx - 2, cy + 1, TileKind::Trap, None, None);
                    }
                    if tile_ok(&tiles, cx + 2, cy + 1) {
                        let trap_kind = if depth >= 2 {
                            TileKind::DartTrap
                        } else {
                            TileKind::Trap
                        };
                        set_tile(&mut tiles, cx + 2, cy + 1, trap_kind, None, None);
                    }
                    let guards: [EnemyArchetype; 2] = match side_branch {
                        Some(SideDungeonBranch::Barrow) => {
                            [EnemyArchetype::GraveHusk, EnemyArchetype::CryptSkeleton]
                        }
                        _ => [EnemyArchetype::CryptSkeleton, EnemyArchetype::PlagueRat],
                    };
                    place_set_piece_enemy(
                        &tiles,
                        &mut enemies,
                        &mut enemy_seq,
                        guards[0],
                        &[(cx - 1, cy - 1), (cx, cy - 1)],
                        spawn_point,
                        return_point,
                        depth,
                        tier,
                        cycle,
                    );
                    place_set_piece_enemy(
                        &tiles,
                        &mut enemies,
                        &mut enemy_seq,
                        guards[1],
                        &[(cx + 1, cy - 1), (cx, cy + 2)],
                        spawn_point,
                        return_point,
                        depth,
                        tier,
                        cycle,
                    );
                }
                SetPieceKind::SporeHeart => {
                    // Spore-Heart Chamber: a cursed altar "heart" ringed
                    // by live spore pools — cross the poison or skirt the
                    // walls to make the pact.
                    paint_room_tint(&mut tiles, &room, "#1F3D2B");
                    set_tile(
                        &mut tiles,
                        cx,
                        cy,
                        TileKind::CursedAltar,
                        Some("Spore-Heart Bloom"),
                        None,
                    );
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            if tile_ok(&tiles, cx + dx, cy + dy) {
                                set_tile(
                                    &mut tiles,
                                    cx + dx,
                                    cy + dy,
                                    TileKind::SporePool,
                                    Some("Heart-Sap Pool (Poison Hazard)"),
                                    None,
                                );
                            }
                        }
                    }
                    if tile_ok(&tiles, rx0 + 1, ry0 + 1) {
                        set_tile(
                            &mut tiles,
                            rx0 + 1,
                            ry0 + 1,
                            TileKind::Chest,
                            Some("Bloom-Husk Cache"),
                            None,
                        );
                    }
                    place_set_piece_enemy(
                        &tiles,
                        &mut enemies,
                        &mut enemy_seq,
                        EnemyArchetype::SporeHulk,
                        &[(cx - 3, cy), (cx, cy - 3)],
                        spawn_point,
                        return_point,
                        depth,
                        tier,
                        cycle,
                    );
                    place_set_piece_enemy(
                        &tiles,
                        &mut enemies,
                        &mut enemy_seq,
                        EnemyArchetype::BlightWeaver,
                        &[(cx + 3, cy), (cx, cy + 3)],
                        spawn_point,
                        return_point,
                        depth,
                        tier,
                        cycle,
                    );
                }
                SetPieceKind::RimeCircle => {
                    // Rime Circle: a rimebound cache ringed by frost
                    // vents and pillars, watched by the revenants who
                    // froze mid-ritual.
                    paint_room_tint(&mut tiles, &room, "#22384A");
                    for (px, py) in [
                        (cx - 2, cy - 2),
                        (cx + 2, cy - 2),
                        (cx - 2, cy + 2),
                        (cx + 2, cy + 2),
                    ] {
                        if decor_ok(&tiles, px, py) {
                            set_tile(
                                &mut tiles,
                                px,
                                py,
                                TileKind::Pillar,
                                Some("Rime-Circle Pillar"),
                                None,
                            );
                        }
                    }
                    for (hx, hy) in [(cx - 1, cy), (cx + 1, cy), (cx, cy - 1), (cx, cy + 1)] {
                        if tile_ok(&tiles, hx, hy) {
                            set_tile(
                                &mut tiles,
                                hx,
                                hy,
                                TileKind::FrostVent,
                                Some("Rime-Circle Vent (Chill Hazard)"),
                                None,
                            );
                        }
                    }
                    set_tile(
                        &mut tiles,
                        cx,
                        cy,
                        TileKind::Chest,
                        Some("Rimebound Cache"),
                        None,
                    );
                    if tile_ok(&tiles, cx, cy + 2) {
                        set_tile(
                            &mut tiles,
                            cx,
                            cy + 2,
                            TileKind::LoreTablet,
                            Some("Rime-Circle Litany"),
                            Some(format!("lore_floor_{depth}")),
                        );
                    }
                    if tile_ok(&tiles, cx - 1, cy - 2) {
                        set_tile(&mut tiles, cx - 1, cy - 2, TileKind::DartTrap, None, None);
                    }
                    place_set_piece_enemy(
                        &tiles,
                        &mut enemies,
                        &mut enemy_seq,
                        EnemyArchetype::RimeRevenant,
                        &[(cx - 3, cy), (cx - 2, cy - 1)],
                        spawn_point,
                        return_point,
                        depth,
                        tier,
                        cycle,
                    );
                    place_set_piece_enemy(
                        &tiles,
                        &mut enemies,
                        &mut enemy_seq,
                        EnemyArchetype::DreadWraith,
                        &[(cx + 3, cy), (cx + 2, cy + 1)],
                        spawn_point,
                        return_point,
                        depth,
                        tier,
                        cycle,
                    );
                }
                SetPieceKind::SealedBrazier => {
                    // Sealed Brazier Hall (main descent): a treasure
                    // nook in the corner, its vault door bound to three
                    // Seal Braziers spread across the hall. Lighting all
                    // three (App: interactWithTile) grinds the seal open.
                    paint_room_tint(&mut tiles, &room, "#3A2E28");
                    set_tile(
                        &mut tiles,
                        rx0 + 1,
                        ry0 + 1,
                        TileKind::Chest,
                        Some("Sealed Reliquary Cache"),
                        None,
                    );
                    set_tile(&mut tiles, rx0 + 2, ry0, TileKind::Wall, None, None);
                    set_tile(
                        &mut tiles,
                        rx0 + 2,
                        ry0 + 1,
                        TileKind::VaultDoor,
                        Some("Brazier-Sealed Door"),
                        Some("seal_door".to_string()),
                    );
                    set_tile(&mut tiles, rx0, ry0 + 2, TileKind::Wall, None, None);
                    set_tile(&mut tiles, rx0 + 1, ry0 + 2, TileKind::Wall, None, None);
                    set_tile(&mut tiles, rx0 + 2, ry0 + 2, TileKind::Wall, None, None);
                    for (sx, sy) in [
                        (rx0 + rw - 2, ry0 + 1),
                        (rx0 + rw - 2, ry0 + rh - 2),
                        (rx0 + 1, ry0 + rh - 2),
                    ] {
                        if decor_ok(&tiles, sx, sy) {
                            set_tile(
                                &mut tiles,
                                sx,
                                sy,
                                TileKind::Brazier,
                                Some("Seal Brazier"),
                                Some("seal_brazier".to_string()),
                            );
                        }
                    }
                    let guards: [EnemyArchetype; 2] = if depth >= 6 {
                        [EnemyArchetype::ObsidianWarden, EnemyArchetype::DreadWraith]
                    } else if depth >= 4 {
                        [EnemyArchetype::IronGolem, EnemyArchetype::ShadowStalker]
                    } else {
                        [EnemyArchetype::CryptSkeleton, EnemyArchetype::BoneAcolyte]
                    };
                    place_set_piece_enemy(
                        &tiles,
                        &mut enemies,
                        &mut enemy_seq,
                        guards[0],
                        &[(cx + 1, cy), (cx, cy)],
                        spawn_point,
                        return_point,
                        depth,
                        tier,
                        cycle,
                    );
                    place_set_piece_enemy(
                        &tiles,
                        &mut enemies,
                        &mut enemy_seq,
                        guards[1],
                        &[(cx - 1, cy + 1), (cx, cy + 1)],
                        spawn_point,
                        return_point,
                        depth,
                        tier,
                        cycle,
                    );
                }
            }
        }
    }

    // --- chests, braziers, lore, traps, hazards, ground loot ----------
    let mut loot: Vec<GroundLoot> = Vec::new();
    for (slice_idx, room) in rooms.iter().skip(1).enumerate() {
        let room_idx = slice_idx + 1;
        let rx = room.x + 1 + rand_below(rng, (room.w - 2).max(1));
        let ry = room.y + 1 + rand_below(rng, (room.h - 2).max(1));
        if tiles[ry as usize][rx as usize].kind == TileKind::Floor
            && !special_rooms.contains(&room_idx)
            && !set_piece_rooms.contains(&room_idx)
        {
            if slice_idx % 3 == 0 {
                tiles[ry as usize][rx as usize] = make_tile(rx, ry, TileKind::Chest);
            } else if slice_idx == 1 {
                tiles[ry as usize][rx as usize] = make_tile(rx, ry, TileKind::Brazier);
            } else if slice_idx == 2 {
                set_tile(
                    &mut tiles,
                    rx,
                    ry,
                    TileKind::LoreTablet,
                    None,
                    Some(format!("lore_floor_{depth}")),
                );
            } else if rand_f(rng) < 0.45 {
                let dart = depth >= 2 && rand_f(rng) < 0.4;
                tiles[ry as usize][rx as usize] =
                    make_tile(rx, ry, if dart { TileKind::DartTrap } else { TileKind::Trap });
            }
        }
        if !special_rooms.contains(&room_idx)
            && !set_piece_rooms.contains(&room_idx)
            && rand_f(rng) < 0.55
        {
            let hx = room.x + 1 + rand_below(rng, (room.w - 2).max(1));
            let hy = room.y + 1 + rand_below(rng, (room.h - 2).max(1));
            if tiles[hy as usize][hx as usize].kind == TileKind::Floor
                && !(hx == spawn_point.x && hy == spawn_point.y)
                && !(hx == return_point.x && hy == return_point.y)
            {
                let hazard = match depth {
                    1 => TileKind::SporePool,
                    2 => {
                        if rand_f(rng) < 0.7 {
                            TileKind::FrostVent
                        } else {
                            TileKind::SporePool
                        }
                    }
                    3 => TileKind::EmberCrust,
                    4 => {
                        if rand_f(rng) < 0.7 {
                            TileKind::ShockRune
                        } else {
                            TileKind::EmberCrust
                        }
                    }
                    _ => {
                        if rand_f(rng) < 0.5 {
                            TileKind::EmberCrust
                        } else {
                            TileKind::ShockRune
                        }
                    }
                };
                tiles[hy as usize][hx as usize] = make_tile(hx, hy, hazard);
            }
        }
        if rand_f(rng) < 0.45 {
            let lx = room.cx + if rand_f(rng) < 0.5 { 1 } else { -1 };
            let ly = room.cy;
            let lt = &tiles[ly as usize][lx as usize];
            if lt.walkable && lt.kind == TileKind::Floor {
                let gold = if rand_f(rng) < 0.5 { Some(8 + depth * 5) } else { None };
                let item = if rand_f(rng) < 0.6 {
                    item_seq += 1;
                    let kind = if rand_f(rng) < 0.6 {
                        ConsumableKind::HealPotion
                    } else {
                        ConsumableKind::ManaPotion
                    };
                    Some(create_consumable_local(kind, 1, item_seq))
                } else {
                    item_seq += 1;
                    Some(generate_procedural_item_local(depth, None, rng, item_seq))
                };
                loot.push(GroundLoot {
                    id: format!("gl_{depth}_{slice_idx}"),
                    x: lx,
                    y: ly,
                    gold,
                    item,
                });
            }
        }
    }

    // --- enemies -------------------------------------------------------
    // Side dungeons draw from their own biome pools (TS sidePools);
    // otherwise the depth pool (Abyss pool at depth >= 6).
    let pool: &[EnemyArchetype] = match side_branch {
        Some(branch) => side_pool_for(branch),
        None => pool_for(depth),
    };
    let mut rare_count = 0;
    let mut roaming_randboss_spawned = false;
    for (r_idx, room) in rooms.iter().skip(1).enumerate() {
        let is_final = r_idx == rooms.len() - 2;
        if !skip_story_boss && is_final && (depth == 2 || depth == 4 || depth == 5) {
            let boss = match depth {
                2 => EnemyArchetype::BossBoneCantor,
                4 => EnemyArchetype::BossAshenInquisitor,
                _ => EnemyArchetype::BossMalakor,
            };
            let seq = next_enemy(&mut enemy_seq);
            enemies.push(create_enemy(
                boss,
                (room.x + 1).max(room.cx - 1),
                room.cy,
                depth,
                tier,
                cycle,
                seq,
            ));
            continue;
        }
        if !skip_story_boss && is_final && depth >= 6 && depth % 2 == 0 {
            // Abyss echo boss: a story boss from the descent, scaled up.
            let archs = [
                EnemyArchetype::BossBoneCantor,
                EnemyArchetype::BossAshenInquisitor,
                EnemyArchetype::BossMalakor,
            ];
            let arch = archs[((depth / 2) % 3) as usize];
            let seq = next_enemy(&mut enemy_seq);
            let mut warden = create_enemy(
                arch,
                (room.x + 1).max(room.cx - 1),
                room.cy,
                depth,
                tier,
                cycle,
                seq,
            );
            let mult = 1.0 + (depth - 5) as f64 * 0.24;
            warden.name = format!("Abyssal Echo · {}", warden.name);
            warden.hp = round(warden.hp as f64 * mult);
            warden.max_hp = warden.hp;
            warden.atk = round(warden.atk as f64 * (1.0 + (depth - 5) as f64 * 0.14));
            warden.def += (depth - 4) / 2;
            warden.xp_reward = round(warden.xp_reward as f64 * mult);
            warden.gold_reward = round(warden.gold_reward as f64 * mult);
            enemies.push(warden);
            continue;
        }
        if !skip_story_boss
            && is_final
            && (depth == 1 || depth == 3 || (depth >= 7 && depth % 2 == 1))
        {
            // Named floor mini-boss wardens on non-major-boss floors
            // (Floors 1, 3, and odd Abyss floors).
            let (arch, rank, boss_name, title): (EnemyArchetype, EnemyRank, String, &str) =
                if depth == 1 {
                    (
                        EnemyArchetype::GoblinSkirmisher,
                        EnemyRank::Rare,
                        "Skrag, Reliquary Gatekeeper".to_string(),
                        "Mini-Boss · Gatekeeper",
                    )
                } else if depth == 3 {
                    (
                        EnemyArchetype::BlightWeaver,
                        EnemyRank::Randboss,
                        "Vaelkor, Blight-Weaver Warden".to_string(),
                        "Mini-Boss · Floor Warden",
                    )
                } else {
                    (
                        EnemyArchetype::ObsidianWarden,
                        EnemyRank::Randboss,
                        format!("Abyssal Harbinger (Depth {depth})"),
                        "Mini-Boss · Floor Warden",
                    )
                };
            let seq = next_enemy(&mut enemy_seq);
            let mut mini = promote_enemy_rank(
                create_enemy(arch, (room.x + 1).max(room.cx - 1), room.cy, depth, tier, cycle, seq),
                depth,
                rank,
                rng,
            );
            mini.name = boss_name;
            mini.rank_title = Some(title.to_string());
            enemies.push(mini);
            continue;
        }
        let mut count = 1;
        if rand_f(rng) < 0.65 {
            count += 1;
        }
        if depth >= 3 && rand_f(rng) < 0.35 {
            count += 1;
        }
        for i in 0..count {
            let ex = room.x + 1 + rand_below(rng, (room.w - 2).max(1));
            let ey = room.y + 1 + rand_below(rng, (room.h - 2).max(1));
            let t = &tiles[ey as usize][ex as usize];
            if t.walkable
                && t.kind == TileKind::Floor
                && !(ex == spawn_point.x && ey == spawn_point.y)
                && !(ex == return_point.x && ey == return_point.y)
                && !enemies.iter().any(|e| e.x == ex && e.y == ey)
            {
                let arch = pool[rand_below(rng, pool.len() as i32) as usize];
                let seq = next_enemy(&mut enemy_seq);
                let mut created = create_enemy(arch, ex, ey, depth, tier, cycle, seq);
                if depth >= 6 && !roaming_randboss_spawned && i == 0 && rand_f(rng) < 0.12 {
                    created = promote_enemy_rank(created, depth, EnemyRank::Randboss, rng);
                    roaming_randboss_spawned = true;
                } else if depth >= 2 && rare_count < 2 && i == 0 && rand_f(rng) < 0.16 {
                    created = promote_enemy_rank(created, depth, EnemyRank::Rare, rng);
                    rare_count += 1;
                }
                enemies.push(created);
            }
        }
    }

    // --- sealed high-risk treasure vault (depth >= 2, 88%) ------------
    // Finds an uncarved 7x5 wall block, seals it behind a vault door,
    // and links it to the nearest existing floor with a short corridor.
    if depth >= 2 && rand_f(rng) < 0.88 {
        let mut floor_points: Vec<(i32, i32)> = Vec::new();
        for y in 2..height - 2 {
            for x in 2..width - 2 {
                if tiles[y as usize][x as usize].kind == TileKind::Floor {
                    floor_points.push((x, y));
                }
            }
        }
        #[derive(Clone, Copy)]
        struct VaultSpot {
            vx: i32,
            vy: i32,
            door_x: i32,
            door_y: i32,
            link_x: i32,
            link_y: i32,
            dist: i32,
        }
        let mut best_vault: Option<VaultSpot> = None;
        for vy in 2..=height - 7 {
            for vx in 2..=width - 9 {
                let mut all_wall = true;
                'scan: for py in (vy - 1)..=(vy + 5) {
                    for px in (vx - 1)..=(vx + 7) {
                        if tiles[py as usize][px as usize].kind != TileKind::Wall {
                            all_wall = false;
                            break 'scan;
                        }
                    }
                }
                if !all_wall {
                    continue;
                }
                // Nearest existing floor tile to one of the 4 cardinal
                // door midpoints, approached along a safe outward axis.
                let door_midpoints = [
                    (vx + 3, vy, 0, -1),
                    (vx + 3, vy + 4, 0, 1),
                    (vx, vy + 2, -1, 0),
                    (vx + 6, vy + 2, 1, 0),
                ];
                for (door_x, door_y, ox, oy) in door_midpoints {
                    let (out_x, out_y) = (door_x + ox, door_y + oy);
                    if out_x < 1 || out_x >= width - 1 || out_y < 1 || out_y >= height - 1 {
                        continue;
                    }
                    for &(fx, fy) in &floor_points {
                        let safe_l = if oy != 0 {
                            fx < vx || fx > vx + 6 || (fy - (vy + 2)).signum() == oy
                        } else {
                            fy < vy || fy > vy + 4 || (fx - (vx + 3)).signum() == ox
                        };
                        if !safe_l {
                            continue;
                        }
                        let dist = (fx - out_x).abs() + (fy - out_y).abs();
                        if dist >= 1 && dist <= 7 && best_vault.map_or(true, |b| dist < b.dist) {
                            best_vault = Some(VaultSpot {
                                vx,
                                vy,
                                door_x,
                                door_y,
                                link_x: fx,
                                link_y: fy,
                                dist,
                            });
                        }
                    }
                }
            }
        }
        if let Some(v) = best_vault {
            let (vx, vy) = (v.vx, v.vy);
            for ry in (vy + 1)..=(vy + 3) {
                for rx in (vx + 1)..=(vx + 5) {
                    let mut t = make_tile(rx, ry, TileKind::Floor);
                    t.color = "#4D3D2C".to_string();
                    tiles[ry as usize][rx as usize] = t;
                }
            }
            tiles[v.door_y as usize][v.door_x as usize] =
                make_tile(v.door_x, v.door_y, TileKind::VaultDoor);
            // Carve the approach corridor along the safe outward axis
            // first, without breaching the vault walls.
            let horiz_door = v.door_x == vx || v.door_x == vx + 6;
            let mut cx = v.door_x
                + if v.door_x == vx {
                    -1
                } else if v.door_x == vx + 6 {
                    1
                } else {
                    0
                };
            let mut cy = v.door_y
                + if v.door_y == vy {
                    -1
                } else if v.door_y == vy + 4 {
                    1
                } else {
                    0
                };
            while cx != v.link_x || cy != v.link_y {
                let inside_vault_box =
                    cx >= vx && cx <= vx + 6 && cy >= vy && cy <= vy + 4;
                if !inside_vault_box
                    && cx >= 1
                    && cx < width - 1
                    && cy >= 1
                    && cy < height - 1
                    && tiles[cy as usize][cx as usize].kind == TileKind::Wall
                {
                    tiles[cy as usize][cx as usize] = make_tile(cx, cy, TileKind::Floor);
                }
                if horiz_door {
                    if cy != v.link_y {
                        cy += if cy < v.link_y { 1 } else { -1 };
                    } else if cx != v.link_x {
                        cx += if cx < v.link_x { 1 } else { -1 };
                    }
                } else {
                    if cx != v.link_x {
                        cx += if cx < v.link_x { 1 } else { -1 };
                    } else if cy != v.link_y {
                        cy += if cy < v.link_y { 1 } else { -1 };
                    }
                }
            }
            // Vault chests + relic + guard(s).
            set_tile(
                &mut tiles,
                vx + 1,
                vy + 1,
                TileKind::Chest,
                Some("Vault Relic Chest"),
                Some("vault_chest".to_string()),
            );
            set_tile(
                &mut tiles,
                vx + 5,
                vy + 1,
                TileKind::Chest,
                Some("Vault Relic Chest"),
                Some("vault_chest".to_string()),
            );
            item_seq += 1;
            loot.push(GroundLoot {
                id: format!("gl_vault_{depth}"),
                x: vx + 3,
                y: vy + 1,
                gold: Some(25 + depth * 12),
                item: Some(generate_procedural_item_local(
                    depth + 1,
                    Some(if depth >= 4 {
                        ItemRarity::Epic
                    } else {
                        ItemRarity::Rare
                    }),
                    rng,
                    item_seq,
                )),
            });
            // P1: the guard is created at the floor's own depth and only
            // becomes a randboss in the Abyss (depth >= 6), so a vault
            // guard can no longer out-stat the floor's boss.
            let guard_arch = pool[rand_below(rng, pool.len() as i32) as usize];
            let seq = next_enemy(&mut enemy_seq);
            let main_guard = promote_enemy_rank(
                create_enemy(guard_arch, vx + 3, vy + 2, depth, tier, cycle, seq),
                depth,
                if depth >= 6 {
                    EnemyRank::Randboss
                } else {
                    EnemyRank::Rare
                },
                rng,
            );
            enemies.push(main_guard);
            if depth >= 3 {
                let wing_arch = pool[rand_below(rng, pool.len() as i32) as usize];
                let seq = next_enemy(&mut enemy_seq);
                let wing_guard = promote_enemy_rank(
                    create_enemy(wing_arch, vx + 2, vy + 3, depth, tier, cycle, seq),
                    depth,
                    EnemyRank::Rare,
                    rng,
                );
                enemies.push(wing_guard);
            }
        }
    }

    // --- connectivity guarantee ---------------------------------------
    // Every room centre and the return point must be orthogonally
    // reachable from spawn; carve straight corridors until they are.
    let carve_if_wall = |tiles: &mut Vec<Vec<Tile>>, tx: i32, ty: i32, depth: i32| {
        let t = &tiles[ty as usize][tx as usize];
        if t.walkable || t.kind == TileKind::VaultDoor {
            return;
        }
        carve_floor(tiles, tx, ty, depth);
    };
    let mut reachable = reachable_in_tiles(&tiles, width, height, spawn_point.x, spawn_point.y);
    let mut targets: Vec<Pos> = rooms.iter().map(|r| Pos::new(r.cx, r.cy)).collect();
    targets.push(return_point);
    for tgt in targets {
        if reachable.contains(&(tgt.x, tgt.y)) {
            continue;
        }
        let mut best_src = spawn_point;
        let mut best_dist = i32::MAX;
        for (kx, ky) in reachable.iter() {
            let d = (kx - tgt.x).abs() + (ky - tgt.y).abs();
            if d < best_dist {
                best_dist = d;
                best_src = Pos::new(*kx, *ky);
            }
        }
        let (mut cx, mut cy) = (best_src.x, best_src.y);
        while cx != tgt.x {
            carve_if_wall(&mut tiles, cx, cy, depth);
            cx += if cx < tgt.x { 1 } else { -1 };
        }
        while cy != tgt.y {
            carve_if_wall(&mut tiles, cx, cy, depth);
            cy += if cy < tgt.y { 1 } else { -1 };
        }
        carve_if_wall(&mut tiles, tgt.x, tgt.y, depth);
        reachable = reachable_in_tiles(&tiles, width, height, spawn_point.x, spawn_point.y);
    }

    // --- ambient critters (cave bats) ----------------------------------
    let mut critters: Vec<CritterState> = Vec::new();
    let mut critter_seq: u32 = 0;
    for _ in 0..3 {
        if rand_f(rng) >= 0.65 {
            continue;
        }
        if rooms.len() < 2 {
            continue;
        }
        let room = rooms[1 + rand_below(rng, ((rooms.len() - 1) as i32).max(1)) as usize];
        let bx = room.x + 1 + rand_below(rng, (room.w - 2).max(1));
        let by = room.y + 1 + rand_below(rng, (room.h - 2).max(1));
        if tiles[by as usize][bx as usize].kind == TileKind::Floor
            && !(bx == spawn_point.x && by == spawn_point.y)
            && !enemies.iter().any(|e| e.x == bx && e.y == by)
            && !critters.iter().any(|c| c.x == bx && c.y == by)
        {
            critter_seq += 1;
            critters.push(make_critter(CritterKind::CaveBat, bx, by, critter_seq));
        }
    }

    // --- Floor 1 town-item guarantee -----------------------------------
    // The 3 Blacksmith pieces + Heartcoal, as ground loot on spaced-out
    // walkable floor tiles away from the entrance. Guaranteed finds,
    // not drop chances: the town loop must be completable on a first
    // clear of the floor. (TS gates this on `depth === 1 &&
    // !options?.sideBranch` — the main descent only.)
    if depth == 1 && side_branch.is_none() {
        const TAGS: [&str; 4] = [
            "blacksmith_anvil",
            "blacksmith_bellows",
            "blacksmith_sign",
            "heartcoal",
        ];
        let mut occupied: HashSet<(i32, i32)> = HashSet::new();
        for l in &loot {
            occupied.insert((l.x, l.y));
        }
        for e in &enemies {
            occupied.insert((e.x, e.y));
        }
        occupied.insert((spawn_point.x, spawn_point.y));
        occupied.insert((return_point.x, return_point.y));
        let dist_from_spawn =
            |x: i32, y: i32| (x - spawn_point.x).abs().max((y - spawn_point.y).abs());
        let mut candidates: Vec<(i32, i32)> = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let t = &tiles[y as usize][x as usize];
                if t.kind != TileKind::Floor || !t.walkable {
                    continue;
                }
                if occupied.contains(&(x, y)) {
                    continue;
                }
                if dist_from_spawn(x, y) < 6 {
                    continue;
                }
                candidates.push((x, y));
            }
        }
        candidates.sort_by_key(|(x, y)| -dist_from_spawn(*x, *y));
        let mut placed: Vec<(i32, i32)> = Vec::new();
        for c in &candidates {
            if placed.len() >= TAGS.len() {
                break;
            }
            if placed
                .iter()
                .any(|p| (p.0 - c.0).abs().max((p.1 - c.1).abs()) < 6)
            {
                continue;
            }
            placed.push(*c);
        }
        // Relaxed fallback (tiny floors): fill remaining tags anywhere free.
        for c in &candidates {
            if placed.len() >= TAGS.len() {
                break;
            }
            if placed.contains(c) {
                continue;
            }
            placed.push(*c);
        }
        for (i, pos) in placed.iter().enumerate() {
            item_seq += 1;
            loot.push(GroundLoot {
                id: format!("town_loot_{}", TAGS[i]),
                x: pos.0,
                y: pos.1,
                gold: None,
                item: Some(create_quest_item_local(TAGS[i], item_seq)),
            });
        }
    }

    ZoneMap {
        id: ZoneId::Dungeon(depth.max(0) as u32),
        name: name.to_string(),
        subtitle,
        is_town: false,
        depth,
        width,
        height,
        tiles,
        enemies,
        npcs: vec![],
        critters: Some(critters),
        loot,
        spawn_point,
        return_point,
    }
}

// ------------------------------------------------------------ town maps

/// Port of `createHighspireMap`.
pub fn create_highspire_map() -> ZoneMap {
    let layout: [&str; 17] = [
        "################################",
        "#║....║.....######......║.....║#",
        "#..*..║..‡..#....#..*...║..*...#",
        "#...........+....+.............#",
        "#..#######..######..#######....#",
        "#..#.....#....░░....#.....#....#",
        "#..#.....+░░░░░░░...+.....#....#",
        "#..#######....░░....#######....#",
        "#.............░░...............#",
        "Ω░░░░░░░░░░░░░◎░░░░░░░░░░░░░*..#",
        "#......Π.Π....░░....Π.Π........#",
        "#..*..........░░..........┼....#",
        "#.....║.......░░.......║.......#",
        "#.............░░...............#",
        "#......*......>>.......║.......#",
        "#.............░░...............#",
        "################################",
    ];
    let npcs = vec![
        npc("lyra", 14, 2, "#60A5FA", "Archivist Lyra", "Keeper of Glyphs"),
        npc("corvus", 5, 6, "#F59E0B", "Magistrate Corvus", "Citadel Arbiter"),
        npc("kaelen", 23, 6, "#EF4444", "Guildmaster Kaelen", "Warden of the Descent"),
        npc("orin", 21, 11, "#C084FC", "Relic Appraiser Orin", "Master Artificer"),
        roaming_npc("tomas", 10, 8, "#93C5FD", "Page Tomas", "Citadel Page"),
    ];
    let mut town = build_town_from_ascii(
        ZoneId::Highspire,
        "Highspire Citadel",
        "Upper Stone Bastion",
        &layout,
        npcs,
        Pos::new(14, 11),
        Pos::new(14, 13),
    );
    town.critters = Some(vec![make_critter(CritterKind::CinderCrow, 25, 2, 1)]);
    town
}

/// Port of `createThornwickMap`.
pub fn create_thornwick_map() -> ZoneMap {
    let layout: [&str; 17] = [
        "############################",
        "#≈≈≈♣..#####....#####..♣≈≈≈#",
        "#≈≈....#...#....#...#...≈≈≈#",
        "#≈.....#...+....+...#....≈≈#",
        "#♣.....#####.░░.#####...♣..#",
        "#............░░........┼...#",
        "#..♣....*....░░....*.......#",
        "#............░░............#",
        "Ω░░░░░░░░░░░░░░░.....‡.....#",
        "#......Π.Π...░░............#",
        "#..*.........░░.....*...♣..#",
        "#.....#####..░░..#####.....#",
        "#.....#...#..░░..#...#..≈≈≈#",
        "#≈≈...#...+..░░..+...#..≈≈≈#",
        "#≈≈≈..#####......#####..≈≈≈#",
        "#≈≈≈≈♣.......✿........♣≈≈≈≈#",
        "############################",
    ];
    let npcs = vec![
        npc("sylva", 16, 2, "#34D399", "Elder Sylva", "Mire-Druid & Lorekeeper"),
        npc("bram", 9, 2, "#FBBF24", "Tinker Bram", "Relic Scavenger"),
        roaming_npc("fenwick", 18, 9, "#4ADE80", "Fenwick the Fisher", "Fen Fisher"),
    ];
    let mut zone = build_town_from_ascii(
        ZoneId::Thornwick,
        "Thornwick Fen",
        "Stilt-Timber Bog Settlement",
        &layout,
        npcs,
        Pos::new(2, 8),
        Pos::new(2, 8),
    );
    zone.tiles[8][0].interact_id = Some("gate_to_overworld".to_string());
    zone.tiles[8][0].label = Some("Gate to Silverwood Vale".to_string());
    zone.critters = Some(vec![
        make_critter(CritterKind::FenFrog, 20, 7, 1),
        make_critter(CritterKind::FenFrog, 12, 14, 2),
    ]);
    zone
}

/// Port of `createCinderwatchMap`.
pub fn create_cinderwatch_map() -> ZoneMap {
    let layout: [&str; 17] = [
        "##############################",
        "#~~~║......########......║~~~#",
        "#~~........#......#.........~#",
        "#.....*....+......+....*.....#",
        "#..######..########..######..#",
        "#..#....#.....░░.....#....#..#",
        "#..#....+░░░░░░░░░░░░+....#..#",
        "#..######.....░░.....######..#",
        "#.............░░.......┼.....#",
        "#.....*.......░░.....*.......#",
        "#......‡......░░.............#",
        "#............░░░░............#",
        "#..║.........░░░░........║...#",
        "#.............░░.............#",
        "#......~......░░......~......#",
        "#~~~..........░░..........~~~#",
        "##############Ω###############",
    ];
    let npcs = vec![
        npc("halda", 6, 6, "#FB923C", "Forgemistress Halda", "Master Runesmith"),
        npc("darek", 23, 6, "#EF4444", "Captain Darek", "Rift Sentinel"),
        roaming_npc("sella", 10, 10, "#FDE68A", "Sella the Quartermaster", "Bastion Quartermaster"),
    ];
    let mut zone = build_town_from_ascii(
        ZoneId::Cinderwatch,
        "Cinderwatch Bastion",
        "Volcanic Ridge Garrison",
        &layout,
        npcs,
        Pos::new(14, 14),
        Pos::new(14, 14),
    );
    zone.tiles[16][14].interact_id = Some("gate_to_overworld".to_string());
    zone.tiles[16][14].label = Some("Ridge Gate to Silverwood Vale".to_string());
    zone.critters = Some(vec![make_critter(CritterKind::CinderCrow, 8, 10, 1)]);
    zone
}

/// Port of `createOverworldMap`: the Silverwood Vale, with its four
/// town gates, four dungeon portals, waystone, lore stele and POIs.
pub fn create_overworld_map(rng: &mut Rng, tier: DifficultyTier, cycle: u32) -> ZoneMap {
    let layout: [&str; 26] = [
        "################################################",
        "#♣♣♣♣....♣♣♣.......♣♣≈≈♣♣......Ω........♣♣♣♣♣♣♣#",
        "#♣♣......♣♣....◈.....≈≈........░.......♣♣...♣♣♣#",
        "#♣...♣♣......♣♣♣.....≈≈........░............>.♣#",
        "#♣...♣..░░>░░░░░░░░░░░≈≈░░░░░░░░░░░░░░░░░░░░░..#",
        "#♣......░............≈≈........░......♣♣....♣..#",
        "#....♣..░...♣♣.......≈≈...♣....░...............#",
        "#♣......░............≈≈........░...♣♣.....♣♣...#",
        "#.......░.....♣♣.....≈≈........░...............#",
        "#..♣....░............≈≈♣.......░.......♣.......#",
        "#.......░.....☼......≈≈........░...............#",
        "#.......░............≈≈........░..........♣....#",
        "Ω░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░Ω",
        "#.......░............≈≈........░...............#",
        "#..♣....░.....‡......≈≈........░...######......#",
        "#.......░............≈≈...♣....░...#....#..♣...#",
        "#.......░.....♣......≈≈........░...+..$.#......#",
        "#...♣...░............≈≈........░...######......#",
        "#.......░............≈≈........░...............#",
        "#♣......░..♣♣........≈≈...♣....░....♣♣.....♣...#",
        "#.......░............≈≈........░...............#",
        "#..♣....░.......♣....≈≈........░.........%%....#",
        "#.......░░░░░░░░░░░░.≈≈....░░░░░░░░░░░░░░░>.✿..#",
        "#♣..........♣.......>≈≈........░..........%....#",
        "#♣♣..♣♣.............♣≈≈...♣♣...░.....♣♣.......♣#",
        "################################Ω###############",
    ];
    let npcs = vec![npc(
        "rowan",
        15,
        11,
        "#34D399",
        "Rowan the Wayfarer",
        "Outrider Scout",
    )];
    let mut zone = build_town_from_ascii(
        ZoneId::Overworld,
        "Silverwood Vale",
        "Frontier Overworld · Crossroads of the Realm",
        &layout,
        npcs,
        Pos::new(2, 12),
        Pos::new(45, 12),
    );
    zone.is_town = false;
    zone.depth = 0;

    // Crisp interactIds & labels for every gate & dungeon portal.
    set_tile_meta(&mut zone, 0, 12, TileKind::GateCity, "gate_to_oakhaven", "Oakhaven Outpost West Gate");
    set_tile_meta(&mut zone, 47, 12, TileKind::GateCity, "gate_to_highspire", "Highspire Citadel Causeway Gate");
    set_tile_meta(&mut zone, 31, 1, TileKind::GateCity, "gate_to_cinderwatch", "Cinderwatch Bastion Ridge Gate");
    set_tile_meta(&mut zone, 32, 25, TileKind::GateCity, "gate_to_thornwick", "Thornwick Fen Timber Gate");

    set_tile_meta(&mut zone, 10, 4, TileKind::StairsDown, "to_barrow_1", "Whispering Barrow Entrance (Lv 2–4)");
    set_tile_meta(&mut zone, 44, 3, TileKind::StairsDown, "to_frost_1", "Frostpeak Cavern Entrance (Lv 5–8)");
    set_tile_meta(&mut zone, 42, 22, TileKind::StairsDown, "to_mire_1", "Fungal Hollow Entrance (Lv 3–5)");
    set_tile_meta(&mut zone, 20, 23, TileKind::StairsDown, "to_dungeon_1", "Sunken Reliquary Outer Descent (Lv 1–2)");

    set_tile_meta(&mut zone, 15, 2, TileKind::Waystone, "waystone_overworld", "Silverwood Elven Waystone");
    set_tile_meta(&mut zone, 14, 14, TileKind::LoreTablet, "lore_overworld_vale", "Silverbridge Crossroads Stele");

    // Phase 3 POIs: quiet landmarks that reward a second look.
    set_tile_meta(&mut zone, 13, 18, TileKind::Pillar, "poi_standing_stones", "Standing Stones of the Vale");
    set_tile_meta(&mut zone, 15, 18, TileKind::Pillar, "poi_standing_stones", "Standing Stones of the Vale");
    set_tile_meta(&mut zone, 14, 19, TileKind::Pillar, "poi_standing_stones", "Standing Stones of the Vale");
    set_tile_meta(&mut zone, 14, 20, TileKind::Pillar, "poi_standing_stones", "Standing Stones of the Vale");
    set_tile_meta(&mut zone, 33, 19, TileKind::Brazier, "poi_hunter_camp", "Hunter's Cold Camp");
    set_tile_meta(&mut zone, 24, 13, TileKind::LoreTablet, "lore_roadside_shrine", "Roadside Shrine of the Vale");

    // Phase 3 ambient life: Vale wildlife (non-combat, flees the player).
    zone.critters = Some(vec![
        make_critter(CritterKind::ValeRabbit, 6, 16, 1),
        make_critter(CritterKind::ValeRabbit, 18, 20, 2),
        make_critter(CritterKind::AshFox, 27, 8, 3),
        make_critter(CritterKind::CinderCrow, 30, 5, 4),
    ]);
    set_tile_meta(&mut zone, 14, 10, TileKind::Brazier, "brazier_overworld", "Wayfarer Campfire");
    set_tile_meta(&mut zone, 38, 16, TileKind::Chest, "overworld_satchel_chest", "Brigand Tollhouse Chest");
    set_tile_meta(&mut zone, 44, 22, TileKind::HerbNode, "herb_node", "Pale Nightbloom");
    zone.tiles[21][41] = make_tile(41, 21, TileKind::SporePool);
    zone.tiles[21][42] = make_tile(42, 21, TileKind::SporePool);
    zone.tiles[23][42] = make_tile(42, 23, TileKind::SporePool);
    zone.tiles[5][43] = make_tile(43, 5, TileKind::FrostVent);
    zone.tiles[5][45] = make_tile(45, 5, TileKind::FrostVent);

    // Ensure the Silverstream bridges & approaches are cleanly walkable road.
    for (x, y) in [
        (21, 4),
        (22, 4),
        (21, 12),
        (22, 12),
        (19, 23),
        (19, 22),
        (20, 22),
        (32, 24),
        (32, 23),
        (32, 22),
        (31, 2),
        (31, 3),
        (44, 4),
    ] {
        zone.tiles[y as usize][x as usize] = make_tile(x, y, TileKind::Road);
    }
    zone.tiles[2][15] = make_tile(15, 2, TileKind::Waystone);
    zone.tiles[2][15].interact_id = Some("waystone_overworld".to_string());
    zone.tiles[2][15].label = Some("Silverwood Elven Waystone".to_string());
    zone.tiles[3][15] = make_tile(15, 3, TileKind::Floor);

    // Populate overworld encounters (kept clear of the Oakhaven west
    // road spawn). Audit Tier 1: the brigand captain is NOT rank-promoted
    // (a rare promotion on an elite archetype out-statted most dungeon
    // bosses); he stays a named tollhouse threat at honest stats.
    let mut enemy_seq: u32 = 1;
    let next_enemy = |seq: &mut u32| {
        *seq += 1;
        *seq
    };
    let mut brigand_captain = create_enemy(
        EnemyArchetype::ShadowStalker,
        37,
        16,
        2,
        tier,
        cycle,
        next_enemy(&mut enemy_seq),
    );
    brigand_captain.id = "overworld_brigand_captain".to_string();
    brigand_captain.name = "Harlan, Silverbridge Brigand".to_string();
    brigand_captain.rank_title = Some("Rare Outlaw · Tollhouse".to_string());

    zone.enemies = vec![
        create_enemy(EnemyArchetype::PlagueRat, 9, 17, 1, tier, cycle, next_enemy(&mut enemy_seq)),
        create_enemy(EnemyArchetype::GoblinSkirmisher, 11, 7, 1, tier, cycle, next_enemy(&mut enemy_seq)),
        create_enemy(EnemyArchetype::CryptSkeleton, 12, 5, 2, tier, cycle, next_enemy(&mut enemy_seq)),
        create_enemy(EnemyArchetype::GoblinSkirmisher, 28, 17, 2, tier, cycle, next_enemy(&mut enemy_seq)),
        create_enemy(EnemyArchetype::GoblinSkirmisher, 34, 16, 2, tier, cycle, next_enemy(&mut enemy_seq)),
        brigand_captain,
        create_enemy(EnemyArchetype::BlightWeaver, 39, 21, 2, tier, cycle, next_enemy(&mut enemy_seq)),
        create_enemy(EnemyArchetype::PlagueRat, 37, 23, 2, tier, cycle, next_enemy(&mut enemy_seq)),
        create_enemy(EnemyArchetype::IronGolem, 40, 5, 2, tier, cycle, next_enemy(&mut enemy_seq)),
        create_enemy(EnemyArchetype::CinderWisp, 35, 6, 2, tier, cycle, next_enemy(&mut enemy_seq)),
    ];

    zone.loot = vec![
        GroundLoot {
            id: "gl_overworld_camp".to_string(),
            x: 17,
            y: 11,
            gold: Some(18),
            item: Some(create_consumable_local(ConsumableKind::HealPotion, 1, 1)),
        },
        GroundLoot {
            id: "gl_overworld_ridge".to_string(),
            x: 38,
            y: 6,
            gold: Some(30),
            item: Some(generate_procedural_item_local(3, None, rng, 2)),
        },
    ];

    zone
}

// ---------------------------------------------------------- side dungeons

struct SideBossMeta {
    id: &'static str,
    name: &'static str,
    title: &'static str,
    arch: EnemyArchetype,
    chest_label: &'static str,
    hp_tune: f64,
    atk_tune: f64,
}

struct SideFloorMeta {
    name: &'static str,
    subtitle: &'static str,
    up_label: &'static str,
    down_label: Option<&'static str>,
    boss: Option<SideBossMeta>,
}

/// TS `generateSideDungeonFloor` titles table (incl. SIDE_BOSS_TUNE).
fn side_floor_meta(branch: SideDungeonBranch, floor: i32) -> Option<SideFloorMeta> {
    match (branch, floor) {
        (SideDungeonBranch::Barrow, 1) => Some(SideFloorMeta {
            name: "Whispering Barrow · Upper Vaults",
            subtitle: "Barrow Floor 1 · Spectral Crypts",
            up_label: "Ascend to Silverwood Vale",
            down_label: Some("Descent to Morvath’s Crypt (Floor 2)"),
            boss: None,
        }),
        (SideDungeonBranch::Barrow, 2) => Some(SideFloorMeta {
            name: "Whispering Barrow · Morvath’s Crypt",
            subtitle: "Barrow Floor 2 · Wight-Lord Sanctum",
            up_label: "Ascend to Upper Vaults (Floor 1)",
            down_label: None,
            boss: Some(SideBossMeta {
                id: "boss_morvath",
                name: "Arch-Wight Morvath",
                title: "Barrow Sovereign · Side Boss",
                arch: EnemyArchetype::DreadWraith,
                chest_label: "Morvath’s Barrow Chest",
                hp_tune: 2.05,
                atk_tune: 1.38,
            }),
        }),
        (SideDungeonBranch::Mire, 1) => Some(SideFloorMeta {
            name: "Fungal Hollow · Spore Galleries",
            subtitle: "Hollow Floor 1 · Bioluminescent Mire",
            up_label: "Ascend to Silverwood Vale",
            down_label: Some("Descent to Mycelial Heart (Floor 2)"),
            boss: None,
        }),
        (SideDungeonBranch::Mire, 2) => Some(SideFloorMeta {
            name: "Fungal Hollow · Mycelial Heart",
            subtitle: "Hollow Floor 2 · Spore-Matron Lair",
            up_label: "Ascend to Spore Galleries (Floor 1)",
            down_label: None,
            boss: Some(SideBossMeta {
                id: "boss_vexis",
                name: "Spore-Matron Vexis",
                title: "Broodmother of the Mire · Side Boss",
                arch: EnemyArchetype::BlightWeaver,
                chest_label: "Mycelial Relic Chest",
                hp_tune: 2.05,
                atk_tune: 1.32,
            }),
        }),
        (SideDungeonBranch::Frost, 1) => Some(SideFloorMeta {
            name: "Frostpeak Cavern · Glacial Drift",
            subtitle: "Frostpeak Floor 1 · Rime Tunnels",
            up_label: "Ascend to Silverwood Vale",
            down_label: Some("Descent to Shattered Chasm (Floor 2)"),
            boss: None,
        }),
        (SideDungeonBranch::Frost, 2) => Some(SideFloorMeta {
            name: "Frostpeak Cavern · Shattered Chasm",
            subtitle: "Frostpeak Floor 2 · Frozen Rune Vaults",
            up_label: "Ascend to Glacial Drift (Floor 1)",
            down_label: Some("Descent to Ashen Caldera (Floor 3)"),
            boss: None,
        }),
        (SideDungeonBranch::Frost, 3) => Some(SideFloorMeta {
            name: "Ashen Caldera · Magma Heart",
            subtitle: "Caldera Floor 3 · Crucible Core",
            up_label: "Ascend to Shattered Chasm (Floor 2)",
            down_label: None,
            boss: Some(SideBossMeta {
                id: "boss_kaelthar",
                name: "Ignis-Warden Kaelthar",
                title: "Lord of the Caldera · Side Boss",
                arch: EnemyArchetype::ObsidianWarden,
                chest_label: "Caldera Relic Hoard",
                hp_tune: 1.0,
                atk_tune: 0.78,
            }),
        }),
        _ => None,
    }
}

/// Port of `generateSideDungeonFloor`: reuses the dungeon generator at
/// an effective depth (barrow 1+f, mire 2+f, frost 3+f) with story
/// content skipped, then re-themes the floor: branch name/labels, lore
/// re-tagging, biome hazard bias, the floor-1 town supply, and — on the
/// terminal floor — a relic chest guarded by the named side boss.
pub fn generate_side_dungeon_floor(
    branch: SideDungeonBranch,
    floor: i32,
    rng: &mut Rng,
    tier: DifficultyTier,
    cycle: u32,
) -> ZoneMap {
    let effective_depth = match branch {
        SideDungeonBranch::Barrow => 1 + floor,
        SideDungeonBranch::Mire => 2 + floor,
        SideDungeonBranch::Frost => 3 + floor,
    };
    let mut zone = generate_dungeon_floor_impl(effective_depth, rng, tier, cycle, true, Some(branch));
    let zone_id = match branch {
        SideDungeonBranch::Barrow => ZoneId::Barrow(floor.max(0) as u32),
        SideDungeonBranch::Mire => ZoneId::Mire(floor.max(0) as u32),
        SideDungeonBranch::Frost => ZoneId::Frost(floor.max(0) as u32),
    };
    zone.id = zone_id;

    let meta = side_floor_meta(branch, floor);
    if let Some(meta) = &meta {
        zone.name = meta.name.to_string();
        zone.subtitle = meta.subtitle.to_string();
        let (sx, sy) = (zone.spawn_point.x, zone.spawn_point.y);
        zone.tiles[sy as usize][sx as usize].label = Some(meta.up_label.to_string());
        if let Some(down_label) = meta.down_label {
            let (rx, ry) = (zone.return_point.x, zone.return_point.y);
            if zone.tiles[ry as usize][rx as usize].kind == TileKind::StairsDown {
                zone.tiles[ry as usize][rx as usize].label = Some(down_label.to_string());
            }
        }
    }

    // Re-tag lore tablets & bias side-dungeon hazards to their biome.
    for y in 0..zone.height {
        for x in 0..zone.width {
            let kind = zone.tiles[y as usize][x as usize].kind;
            if kind == TileKind::LoreTablet {
                zone.tiles[y as usize][x as usize].interact_id =
                    Some(format!("lore_{}", branch.as_str()));
            }
            if branch == SideDungeonBranch::Mire
                && matches!(kind, TileKind::FrostVent | TileKind::EmberCrust)
            {
                zone.tiles[y as usize][x as usize] = make_tile(x, y, TileKind::SporePool);
            }
            if branch == SideDungeonBranch::Frost
                && floor <= 2
                && kind == TileKind::EmberCrust
            {
                zone.tiles[y as usize][x as usize] = make_tile(x, y, TileKind::FrostVent);
            }
        }
    }

    // ElfStory Reborn Phase 1: Floor 1 of each branch carries a
    // guaranteed Oakhaven town supply (barrow_1 -> barrow_iron,
    // mire_1 -> mire_timber, frost_1 -> frost_coal) on the walkable
    // floor tile farthest from the entrance, so the player chooses
    // their Run-2+ zone because of what Oakhaven needs.
    if floor == 1 {
        let supply_tag = match branch {
            SideDungeonBranch::Barrow => "barrow_iron",
            SideDungeonBranch::Mire => "mire_timber",
            SideDungeonBranch::Frost => "frost_coal",
        };
        let mut occupied: HashSet<(i32, i32)> = HashSet::new();
        for l in &zone.loot {
            occupied.insert((l.x, l.y));
        }
        for e in &zone.enemies {
            occupied.insert((e.x, e.y));
        }
        occupied.insert((zone.spawn_point.x, zone.spawn_point.y));
        occupied.insert((zone.return_point.x, zone.return_point.y));
        let mut best: Option<(i32, i32)> = None;
        let mut best_dist = -1;
        for y in 0..zone.height {
            for x in 0..zone.width {
                let t = &zone.tiles[y as usize][x as usize];
                if t.kind != TileKind::Floor || !t.walkable {
                    continue;
                }
                if occupied.contains(&(x, y)) {
                    continue;
                }
                let d = (x - zone.spawn_point.x)
                    .abs()
                    .max((y - zone.spawn_point.y).abs());
                if d > best_dist {
                    best_dist = d;
                    best = Some((x, y));
                }
            }
        }
        if let Some((bx, by)) = best {
            zone.loot.push(GroundLoot {
                id: format!("town_loot_{supply_tag}"),
                x: bx,
                y: by,
                gold: None,
                item: Some(create_quest_item_local(supply_tag, 7000 + floor.max(0) as u32)),
            });
        }
    }

    // Terminal floor: swap stairs_down into a guarded relic chest with
    // the branch's sole named side boss standing watch.
    if let Some(meta) = &meta {
        if let Some(boss_meta) = &meta.boss {
            let (rx, ry) = (zone.return_point.x, zone.return_point.y);
            let mut chest = make_tile(rx, ry, TileKind::Chest);
            chest.label = Some(boss_meta.chest_label.to_string());
            chest.interact_id = Some(format!("{}_boss_chest", zone_id.to_string_id()));
            zone.tiles[ry as usize][rx as usize] = chest;

            // Find a free walkable tile near the hoard chest so the boss
            // never stacks on another enemy or wall.
            let mut bx = (rx - 1).max(2);
            let mut by = ry;
            let mut placed = false;
            'search: for radius in 1..=5 {
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let (cx, cy) = (rx + dx, ry + dy);
                        let walkable = zone
                            .tiles
                            .get(cy as usize)
                            .and_then(|r| r.get(cx as usize))
                            .map_or(false, |t| t.walkable);
                        if walkable
                            && !(cx == zone.spawn_point.x && cy == zone.spawn_point.y)
                            && !zone.enemies.iter().any(|e| e.x == cx && e.y == cy)
                        {
                            bx = cx;
                            by = cy;
                            placed = true;
                            break 'search;
                        }
                    }
                }
            }
            if !placed {
                zone.enemies.retain(|e| !(e.x == bx && e.y == by));
            }

            let mut boss = promote_enemy_rank(
                create_enemy(boss_meta.arch, bx, by, effective_depth + 1, tier, cycle, 9000),
                effective_depth + 1,
                EnemyRank::Randboss,
                rng,
            );
            // Side bosses are tuned explicitly (TS SIDE_BOSS_TUNE) so
            // none of them out-stats the final boss.
            let new_hp = round(boss.hp as f64 * boss_meta.hp_tune);
            let new_max_hp = round(boss.max_hp as f64 * boss_meta.hp_tune);
            boss.hp = new_hp;
            boss.max_hp = new_max_hp;
            boss.atk = round(boss.atk as f64 * boss_meta.atk_tune).max(1);
            boss.id = boss_meta.id.to_string();
            boss.name = boss_meta.name.to_string();
            boss.rank_title = Some(boss_meta.title.to_string());
            boss.is_boss = Some(true);
            zone.enemies.push(boss);
        }
    }

    zone
}

// ------------------------------------------------------------ zone access

/// Port of `generateZoneById`: static zones build directly; dungeon
/// floors generate on demand (main descent depth >= 1, barrow/mire
/// floors 1-2, frost floors 1-3); anything else is `None`.
pub fn generate_zone_by_id(
    id: ZoneId,
    rng: &mut Rng,
    tier: DifficultyTier,
    cycle: u32,
) -> Option<ZoneMap> {
    match id {
        ZoneId::Oakhaven => Some(create_oakhaven_map()),
        ZoneId::Overworld => Some(create_overworld_map(rng, tier, cycle)),
        ZoneId::Highspire => Some(create_highspire_map()),
        ZoneId::Thornwick => Some(create_thornwick_map()),
        ZoneId::Cinderwatch => Some(create_cinderwatch_map()),
        ZoneId::Dungeon(n) if n >= 1 => {
            Some(generate_dungeon_floor(n as i32, rng, tier, cycle))
        }
        ZoneId::Barrow(n) if (1..=2).contains(&n) => Some(generate_side_dungeon_floor(
            SideDungeonBranch::Barrow,
            n as i32,
            rng,
            tier,
            cycle,
        )),
        ZoneId::Mire(n) if (1..=2).contains(&n) => Some(generate_side_dungeon_floor(
            SideDungeonBranch::Mire,
            n as i32,
            rng,
            tier,
            cycle,
        )),
        ZoneId::Frost(n) if (1..=3).contains(&n) => Some(generate_side_dungeon_floor(
            SideDungeonBranch::Frost,
            n as i32,
            rng,
            tier,
            cycle,
        )),
        _ => None,
    }
}

/// Port of `createInitialZones`: the 5 static zones plus main-descent
/// Floors 1-5, in TS record order.
pub fn create_initial_zones(
    rng: &mut Rng,
    tier: DifficultyTier,
    cycle: u32,
) -> Vec<(ZoneId, ZoneMap)> {
    vec![
        (ZoneId::Oakhaven, create_oakhaven_map()),
        (ZoneId::Overworld, create_overworld_map(rng, tier, cycle)),
        (ZoneId::Highspire, create_highspire_map()),
        (ZoneId::Thornwick, create_thornwick_map()),
        (ZoneId::Cinderwatch, create_cinderwatch_map()),
        (ZoneId::Dungeon(1), generate_dungeon_floor(1, rng, tier, cycle)),
        (ZoneId::Dungeon(2), generate_dungeon_floor(2, rng, tier, cycle)),
        (ZoneId::Dungeon(3), generate_dungeon_floor(3, rng, tier, cycle)),
        (ZoneId::Dungeon(4), generate_dungeon_floor(4, rng, tier, cycle)),
        (ZoneId::Dungeon(5), generate_dungeon_floor(5, rng, tier, cycle)),
    ]
}

/// Port of TS `ZoneContextBrief`.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneContextBrief {
    pub lore: String,
    pub tactical: String,
    pub recommended: String,
}

/// Port of `getZoneContext(zoneId, depth)`.
pub fn get_zone_context(zone: ZoneId, depth: i32) -> ZoneContextBrief {
    fn brief(lore: &str, tactical: &str, recommended: &str) -> ZoneContextBrief {
        ZoneContextBrief {
            lore: lore.to_string(),
            tactical: tactical.to_string(),
            recommended: recommended.to_string(),
        }
    }
    match zone {
        ZoneId::Oakhaven => brief(
            "A sunny timber village built around the outer Reliquary stair. Torin’s forge and Mira’s apothecary keep delvers supplied.",
            "Safe Haven · Step out the West Gate (Ω) onto Silverwood Vale Overworld, or descend >> into the Reliquary.",
            "Town Sanctuary",
        ),
        ZoneId::Overworld => brief(
            "Twilight forest roads, Silverstream crossings, and ruined watchtowers linking Oakhaven, Thornwick Fen, Highspire, and Cinderwatch.",
            "Overworld · Speak with Rowan at Silverbridge, hunt brigands, attune the forest Waystone, or enter 4 dungeon regions.",
            "Lv 1–6 Overworld",
        ),
        ZoneId::Highspire => brief(
            "Marble sanctuary perched above the sunken vaults. Guildmaster Kaelen and Archivist Lyra coordinate expeditions to Floor 5.",
            "Safe Haven · Temper gear, trade Relics with Orin, and attune the Whispering Bell.",
            "Citadel Sanctuary",
        ),
        ZoneId::Thornwick => brief(
            "A stilt-timber bog hamlet tucked beside the Fungal Hollow. Elder Sylva tends the mire wards while Tinker Bram salvages deep curios.",
            "Safe Haven · Trade with Tinker Bram or help Elder Sylva cleanse Fungal Hollow to the east.",
            "Fen Sanctuary",
        ),
        ZoneId::Cinderwatch => brief(
            "An iron-bound basalt garrison guarding the Frostpeak & Caldera passes. Forgemistress Halda tempers volcanic steel.",
            "Safe Haven · Gear up with Halda’s Runesmith stock or take bounties from Captain Darek.",
            "Ridge Sanctuary",
        ),
        ZoneId::Barrow(_) => brief(
            "Ancient pre-Citadel barrow tombs haunted by skeletal guardians and Arch-Wight Morvath.",
            "Side Dungeon (2 Floors) · Defeat Arch-Wight Morvath on Floor 2 for Barrow Relics and Captain Darek’s bounty.",
            "Lv 2–4",
        ),
        ZoneId::Mire(_) => brief(
            "Bioluminescent subterranean caverns choked with toxic spore pools beneath Thornwick Fen.",
            "Side Dungeon (2 Floors) · Watch Blight Spore Pools and slay Spore-Matron Vexis on Floor 2.",
            "Lv 3–5",
        ),
        ZoneId::Frost(_) => brief(
            "Rime-crusted crystal galleries descending straight into the molten veins of the Ashen Caldera.",
            "Side Dungeon (3 Floors) · Brave Frost Vents & Shock Runes to reach Ignis-Warden Kaelthar on Floor 3.",
            "Lv 5–8",
        ),
        ZoneId::Dungeon(_) => match depth {
            1 => brief(
                "Cold limestone arches scarred by decades of treasure hunters and restless bone-sentinels.",
                "Watch skeletal archer lines of fire and spike snares (Agility 3+ spots traps automatically).",
                "Lv 1–2",
            ),
            2 => brief(
                "Echoing ossuary galleries where Bone Cantors weave protective wards over the dead.",
                "Sealed Treasure Vaults appear here—expect Rare skill-using guards behind amber doors.",
                "Lv 2–4",
            ),
            3 => brief(
                "Basalt forge-halls warmed by deep fissures, home to Stone Golems and the Whispering Bell.",
                "Step off red Ground Slam tiles when Golems wind up, or break them with Stuns.",
                "Lv 4–6",
            ),
            4 => brief(
                "The Inquisitor Crucible burns with cinder wards and high-ranking Reliquary champions.",
                "Unlocks Skill Capstones at Lv 6+. Temper equipment to +2/+3 before entering Floor 5.",
                "Lv 6–8",
            ),
            5 => brief(
                "Black glass columns frame the Obsidian Sanctum where Malakor guards the Ashen Glyph.",
                "Boss Floor: Malakor, Sovereign of Cinder awaits. Bring full Runes, Infusions & Draughts.",
                "Lv 8+",
            ),
            _ => ZoneContextBrief {
                lore: format!(
                    "Untamed reality fractures across Abyssal Depth {depth}, teeming with empowered champions."
                ),
                tactical: format!(
                    "Endless Abyss (Floor {depth}): Randbosses and Relic-tier Sealed Vaults scale with depth."
                ),
                recommended: format!("Lv {}+", (depth + 4).max(9)),
            },
        },
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn floor1(seed: u32) -> ZoneMap {
        let mut rng = Rng::new(seed);
        generate_dungeon_floor(1, &mut rng, DifficultyTier::Standard, 0)
    }

    #[test]
    fn torin_spawns_outside_forge_door() {
        let oakhaven = create_oakhaven_map();
        assert_eq!((oakhaven.width, oakhaven.height), (28, 17));
        let torin = oakhaven
            .npcs
            .iter()
            .find(|n| n.id == "torin")
            .expect("Torin must be placed in Oakhaven");
        assert_eq!((torin.x, torin.y), (7, 4), "Torin stands outside his forge door");
        assert_ne!((torin.x, torin.y), (7, 2), "Torin must never stand inside the forge mesh");
        let (min_x, max_x, min_z, max_z) = TORIN_FORGE_FOOTPRINT;
        assert!(
            !(torin.x >= min_x && torin.x <= max_x && torin.y >= min_z && torin.y <= max_z),
            "Torin is outside the forge footprint {TORIN_FORGE_FOOTPRINT:?}"
        );
        assert!(
            oakhaven.tiles[torin.y as usize][torin.x as usize].walkable,
            "Torin's tile is walkable (mirrors ts/tests/town.test.ts)"
        );
    }

    #[test]
    fn blacksmith_lot_footprints_match_model_loader_anchors() {
        // modelLoader.ts OAKHAVEN_LOT_ANCHORS hideTerrainFootprint values.
        assert_eq!(blacksmith_lot_footprint("pondside"), Some((14, 18, 12, 14)));
        assert_eq!(blacksmith_lot_footprint("market"), Some((10, 12, 4, 6)));
        assert_eq!(blacksmith_lot_footprint("west_gate"), Some((3, 6, 8, 10)));
        assert_eq!(blacksmith_lot_footprint("nowhere"), None);
    }

    #[test]
    fn floor1_guarantees_four_town_items() {
        const EXPECTED: [&str; 4] = [
            "blacksmith_anvil",
            "blacksmith_bellows",
            "blacksmith_sign",
            "heartcoal",
        ];
        for seed in [1u32, 42, 777, 0xCAFE_1234, 2026] {
            let zone = floor1(seed);
            let mut tags: Vec<&str> = zone
                .loot
                .iter()
                .filter_map(|l| l.item.as_ref().and_then(|i| i.quest_tag.as_deref()))
                .collect();
            tags.sort();
            let mut expected = EXPECTED.to_vec();
            expected.sort();
            assert_eq!(tags, expected, "seed {seed}: all 4 town items exactly once");
            for l in &zone.loot {
                if let Some(item) = &l.item {
                    if item.quest_tag.is_some() {
                        assert_eq!(item.category, ItemCategory::Quest);
                    }
                }
            }
        }
        // Other floors do not carry the main-descent town items.
        let mut rng = Rng::new(9);
        let f2 = generate_dungeon_floor(2, &mut rng, DifficultyTier::Standard, 0);
        assert!(
            !f2.loot.iter().any(|l| l
                .item
                .as_ref()
                .and_then(|i| i.quest_tag.as_deref())
                .map_or(false, |t| EXPECTED.contains(&t))),
            "Floor 2 must not duplicate the Floor 1 town guarantee"
        );
    }

    #[test]
    fn floor1_all_loot_reachable() {
        for seed in [1u32, 42, 777, 0xCAFE_1234, 2026, 31337] {
            let zone = floor1(seed);
            let reachable = reachable_positions(&zone, zone.spawn_point.x, zone.spawn_point.y);
            assert!(
                reachable.contains(&(zone.return_point.x, zone.return_point.y)),
                "seed {seed}: stairs down reachable"
            );
            assert_eq!(
                zone.tiles[zone.spawn_point.y as usize][zone.spawn_point.x as usize].kind,
                TileKind::StairsUp,
                "seed {seed}: spawn sits on stairs up"
            );
            assert_eq!(
                zone.tiles[zone.return_point.y as usize][zone.return_point.x as usize].kind,
                TileKind::StairsDown,
                "seed {seed}: return sits on stairs down"
            );
            for l in &zone.loot {
                assert!(
                    reachable.contains(&(l.x, l.y)),
                    "seed {seed}: loot {} at ({},{}) must be reachable",
                    l.id,
                    l.x,
                    l.y
                );
            }
            for e in &zone.enemies {
                assert!(
                    zone.tiles[e.y as usize][e.x as usize].walkable,
                    "seed {seed}: enemy {} stands on a walkable tile",
                    e.id
                );
            }
        }
    }

    #[test]
    fn floor1_generation_is_deterministic_per_seed() {
        let a = floor1(0xCAFE_1234);
        let b = floor1(0xCAFE_1234);
        assert_eq!(a.spawn_point, b.spawn_point);
        assert_eq!(a.return_point, b.return_point);
        assert_eq!(a.enemies.len(), b.enemies.len());
        assert_eq!(a.loot.len(), b.loot.len());
    }

    #[test]
    fn create_enemy_uses_difficulty_funnel_and_xp_trim() {
        // Standard + cycle 0 is the identity: P1 constants land exactly,
        // with XP trimmed by KILL_XP_FACTOR (22 * 0.45 = 9.9 -> 10).
        let skel = create_enemy(
            EnemyArchetype::CryptSkeleton,
            3,
            3,
            1,
            DifficultyTier::Standard,
            0,
            1,
        );
        assert_eq!((skel.max_hp, skel.atk, skel.def), (58, 11, 2));
        assert_eq!(skel.xp_reward, 10);
        // Hardened funnels through difficulty.rs: hp 58*1.25=73 (72.5
        // rounds away from zero, as in JS), atk 11*1.15=13.
        let hard = create_enemy(
            EnemyArchetype::CryptSkeleton,
            3,
            3,
            1,
            DifficultyTier::Hardened,
            0,
            2,
        );
        assert_eq!((hard.max_hp, hard.atk), (73, 13));
        // Story boss: fixed base stats, XP 180 * 0.45 = 81.
        let vaelen = create_enemy(
            EnemyArchetype::BossBoneCantor,
            5,
            5,
            2,
            DifficultyTier::Standard,
            0,
            3,
        );
        assert_eq!((vaelen.max_hp, vaelen.atk), (620, 30));
        assert_eq!(vaelen.xp_reward, 81);
        assert_eq!(vaelen.is_boss, Some(true));
    }
    // ------------------------------------------------ M3 part 2 tests

    fn assert_map_invariants(zone: &ZoneMap) {
        let at = |p: Pos| &zone.tiles[p.y as usize][p.x as usize];
        assert!(
            at(zone.spawn_point).walkable,
            "{}: spawn {:?} not walkable",
            zone.name,
            zone.spawn_point
        );
        assert!(
            at(zone.return_point).walkable,
            "{}: return {:?} not walkable",
            zone.name,
            zone.return_point
        );
        for l in &zone.loot {
            let t = &zone.tiles[l.y as usize][l.x as usize];
            assert!(
                t.walkable,
                "{}: loot {} at ({},{}) on non-walkable {:?}",
                zone.name,
                l.id,
                l.x,
                l.y,
                t.kind
            );
        }
        for e in &zone.enemies {
            let t = &zone.tiles[e.y as usize][e.x as usize];
            assert!(
                t.walkable,
                "{}: enemy {} at ({},{}) on non-walkable {:?}",
                zone.name,
                e.name,
                e.x,
                e.y,
                t.kind
            );
        }
    }

    fn tile_at(zone: &ZoneMap, x: i32, y: i32) -> &Tile {
        &zone.tiles[y as usize][x as usize]
    }

    #[test]
    fn initial_zones_generate_with_walkable_spawn_return_and_contents() {
        let mut rng = Rng::new(42);
        let zones = create_initial_zones(&mut rng, DifficultyTier::Standard, 0);
        assert_eq!(zones.len(), 10);
        let ids: Vec<ZoneId> = zones.iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            vec![
                ZoneId::Oakhaven,
                ZoneId::Overworld,
                ZoneId::Highspire,
                ZoneId::Thornwick,
                ZoneId::Cinderwatch,
                ZoneId::Dungeon(1),
                ZoneId::Dungeon(2),
                ZoneId::Dungeon(3),
                ZoneId::Dungeon(4),
                ZoneId::Dungeon(5),
            ]
        );
        for (_, zone) in &zones {
            assert_map_invariants(zone);
        }
        // Main-descent floors: spawn is stairs_up, return is stairs_down.
        for (_, zone) in zones.iter().skip(5) {
            assert_eq!(
                tile_at(zone, zone.spawn_point.x, zone.spawn_point.y).kind,
                TileKind::StairsUp,
                "{} spawn should be stairs_up",
                zone.name
            );
            assert_eq!(
                tile_at(zone, zone.return_point.x, zone.return_point.y).kind,
                TileKind::StairsDown,
                "{} return should be stairs_down",
                zone.name
            );
        }
    }

    #[test]
    fn town_maps_have_expected_npcs_spawns_and_gates() {
        let mut rng = Rng::new(7);
        let highspire = create_highspire_map();
        assert_eq!((highspire.width, highspire.height), (32, 17));
        assert!(highspire.is_town);
        let names: Vec<&str> = highspire.npcs.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(names, vec!["lyra", "corvus", "kaelen", "orin", "tomas"]);
        assert_eq!(highspire.spawn_point, Pos::new(14, 11));
        assert_map_invariants(&highspire);

        let thornwick = create_thornwick_map();
        assert_eq!((thornwick.width, thornwick.height), (28, 17));
        let names: Vec<&str> = thornwick.npcs.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(names, vec!["sylva", "bram", "fenwick"]);
        assert_eq!(
            thornwick.tiles[8][0].interact_id.as_deref(),
            Some("gate_to_overworld")
        );
        assert_map_invariants(&thornwick);

        let cinderwatch = create_cinderwatch_map();
        assert_eq!((cinderwatch.width, cinderwatch.height), (30, 17));
        let names: Vec<&str> = cinderwatch.npcs.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(names, vec!["halda", "darek", "sella"]);
        assert_eq!(
            cinderwatch.tiles[16][14].interact_id.as_deref(),
            Some("gate_to_overworld")
        );
        assert_map_invariants(&cinderwatch);

        let overworld = create_overworld_map(&mut rng, DifficultyTier::Standard, 0);
        assert_eq!((overworld.width, overworld.height), (48, 26));
        assert!(!overworld.is_town);
        assert_map_invariants(&overworld);
        let gate = |x: i32, y: i32| overworld.tiles[y as usize][x as usize].interact_id.as_deref();
        assert_eq!(gate(0, 12), Some("gate_to_oakhaven"));
        assert_eq!(gate(47, 12), Some("gate_to_highspire"));
        assert_eq!(gate(31, 1), Some("gate_to_cinderwatch"));
        assert_eq!(gate(32, 25), Some("gate_to_thornwick"));
        assert_eq!(gate(10, 4), Some("to_barrow_1"));
        assert_eq!(gate(44, 3), Some("to_frost_1"));
        assert_eq!(gate(42, 22), Some("to_mire_1"));
        assert_eq!(gate(20, 23), Some("to_dungeon_1"));
        assert_eq!(gate(15, 2), Some("waystone_overworld"));
        assert_eq!(gate(14, 14), Some("lore_overworld_vale"));
        assert_eq!(tile_at(&overworld, 10, 4).kind, TileKind::StairsDown);
        assert_eq!(tile_at(&overworld, 0, 12).kind, TileKind::GateCity);
        // Named overworld threat, honest stats (no rank promotion).
        let captain = overworld
            .enemies
            .iter()
            .find(|e| e.id == "overworld_brigand_captain")
            .expect("brigand captain present");
        assert_eq!(captain.name, "Harlan, Silverbridge Brigand");
        assert_eq!(captain.is_boss, None);
        assert_eq!(overworld.enemies.len(), 10);
        assert_eq!(overworld.loot.len(), 2);
    }

    #[test]
    fn side_dungeon_floor1_carries_its_town_supply() {
        let cases = [
            (SideDungeonBranch::Barrow, "barrow_iron"),
            (SideDungeonBranch::Mire, "mire_timber"),
            (SideDungeonBranch::Frost, "frost_coal"),
        ];
        for (branch, tag) in cases {
            let mut rng = Rng::new(99);
            let zone = generate_side_dungeon_floor(branch, 1, &mut rng, DifficultyTier::Standard, 0);
            assert_map_invariants(&zone);
            assert_eq!(
                tile_at(&zone, zone.spawn_point.x, zone.spawn_point.y).kind,
                TileKind::StairsUp
            );
            assert_eq!(
                tile_at(&zone, zone.return_point.x, zone.return_point.y).kind,
                TileKind::StairsDown,
                "{} floor 1 must have a descent",
                zone.name
            );
            let supply = zone
                .loot
                .iter()
                .find(|l| l.id == format!("town_loot_{tag}"))
                .unwrap_or_else(|| panic!("{} floor 1 missing supply {tag}", zone.name));
            let item = supply.item.as_ref().expect("supply carries an item");
            assert_eq!(item.category, ItemCategory::Quest);
            assert_eq!(item.quest_tag.as_deref(), Some(tag));
            // No main-descent town pieces leak into side floors.
            assert!(
                !zone.loot.iter().any(|l| {
                    l.item
                        .as_ref()
                        .and_then(|i| i.quest_tag.as_deref())
                        .map_or(false, |t| t.starts_with("blacksmith_") || t == "heartcoal")
                }),
                "{} floor 1 must not carry blacksmith pieces",
                zone.name
            );
        }
    }

    #[test]
    fn side_dungeon_terminal_floors_have_named_boss_and_chest() {
        let cases = [
            (SideDungeonBranch::Barrow, 2, "Arch-Wight Morvath", "boss_morvath"),
            (SideDungeonBranch::Mire, 2, "Spore-Matron Vexis", "boss_vexis"),
            (SideDungeonBranch::Frost, 3, "Ignis-Warden Kaelthar", "boss_kaelthar"),
        ];
        for (branch, floor, name, id) in cases {
            let mut rng = Rng::new(1234);
            let zone = generate_side_dungeon_floor(branch, floor, &mut rng, DifficultyTier::Standard, 0);
            assert_map_invariants(&zone);
            let boss = zone
                .enemies
                .iter()
                .find(|e| e.id == id)
                .unwrap_or_else(|| panic!("{} missing boss {id}", zone.name));
            assert_eq!(boss.name, name);
            assert_eq!(boss.is_boss, Some(true));
            // The return tile became the boss's relic chest (no descent).
            let ret = tile_at(&zone, zone.return_point.x, zone.return_point.y);
            assert_eq!(ret.kind, TileKind::Chest);
            assert!(ret.interact_id.as_deref().map_or(false, |i| i.ends_with("_boss_chest")));
        }
        // Frost floor 2 is a mid floor: stairs_down present, no boss.
        let mut rng = Rng::new(5);
        let frost2 = generate_side_dungeon_floor(SideDungeonBranch::Frost, 2, &mut rng, DifficultyTier::Standard, 0);
        assert_map_invariants(&frost2);
        assert_eq!(
            tile_at(&frost2, frost2.return_point.x, frost2.return_point.y).kind,
            TileKind::StairsDown
        );
        assert!(!frost2.enemies.iter().any(|e| e.is_boss == Some(true)));
    }

    #[test]
    fn generate_zone_by_id_respects_ts_ranges() {
        let mut rng = Rng::new(3);
        assert!(generate_zone_by_id(ZoneId::Barrow(1), &mut rng, DifficultyTier::Standard, 0).is_some());
        assert!(generate_zone_by_id(ZoneId::Barrow(2), &mut rng, DifficultyTier::Standard, 0).is_some());
        assert!(generate_zone_by_id(ZoneId::Barrow(3), &mut rng, DifficultyTier::Standard, 0).is_none());
        assert!(generate_zone_by_id(ZoneId::Mire(2), &mut rng, DifficultyTier::Standard, 0).is_some());
        assert!(generate_zone_by_id(ZoneId::Mire(3), &mut rng, DifficultyTier::Standard, 0).is_none());
        assert!(generate_zone_by_id(ZoneId::Frost(3), &mut rng, DifficultyTier::Standard, 0).is_some());
        assert!(generate_zone_by_id(ZoneId::Frost(4), &mut rng, DifficultyTier::Standard, 0).is_none());
        assert!(generate_zone_by_id(ZoneId::Dungeon(1), &mut rng, DifficultyTier::Standard, 0).is_some());
        assert!(generate_zone_by_id(ZoneId::Dungeon(9), &mut rng, DifficultyTier::Standard, 0).is_some());
        assert!(generate_zone_by_id(ZoneId::Oakhaven, &mut rng, DifficultyTier::Standard, 0).is_some());
        let overworld = generate_zone_by_id(ZoneId::Overworld, &mut rng, DifficultyTier::Standard, 0).unwrap();
        assert_eq!(overworld.id, ZoneId::Overworld);
        let barrow = generate_zone_by_id(ZoneId::Barrow(1), &mut rng, DifficultyTier::Standard, 0).unwrap();
        assert_eq!(barrow.id, ZoneId::Barrow(1));
        assert!(barrow.name.starts_with("Whispering Barrow"));
    }

    #[test]
    fn main_descent_bosses_and_town_item_guardrail() {
        let mut rng = Rng::new(2024);
        let f2 = generate_dungeon_floor(2, &mut rng, DifficultyTier::Standard, 0);
        assert!(
            f2.enemies.iter().any(|e| e.name == "Vaelen, Bone Cantor" && e.is_boss == Some(true)),
            "floor 2 must contain the Bone Cantor"
        );
        let f5 = generate_dungeon_floor(5, &mut rng, DifficultyTier::Standard, 0);
        assert!(
            f5.enemies
                .iter()
                .any(|e| e.name == "Malakor, Sovereign of Cinder" && e.is_boss == Some(true)),
            "floor 5 must contain Malakor"
        );
        // Abyss echo boss on even floors >= 6; harbinger on odd floors >= 7.
        let f6 = generate_dungeon_floor(6, &mut rng, DifficultyTier::Standard, 0);
        assert!(
            f6.enemies.iter().any(|e| e.name.starts_with("Abyssal Echo · ")),
            "floor 6 must contain an Abyssal Echo boss"
        );
        let f7 = generate_dungeon_floor(7, &mut rng, DifficultyTier::Standard, 0);
        assert!(
            f7.enemies.iter().any(|e| e.name == "Abyssal Harbinger (Depth 7)"),
            "floor 7 must contain an Abyssal Harbinger"
        );
        // Guardrail: the blacksmith pieces + Heartcoal exist ONLY on
        // dungeon_1 (and never on side-dungeon floor 1, which maps to
        // effective depth 2 anyway).
        for depth in 2..=7 {
            let z = generate_dungeon_floor(depth, &mut rng, DifficultyTier::Standard, 0);
            assert!(
                !z.loot.iter().any(|l| {
                    l.item
                        .as_ref()
                        .and_then(|i| i.quest_tag.as_deref())
                        .map_or(false, |t| t.starts_with("blacksmith_") || t == "heartcoal")
                }),
                "floor {depth} must not carry town pieces"
            );
            assert_map_invariants(&z);
        }
    }

    #[test]
    fn zone_context_matches_ts_table() {
        let oakhaven = get_zone_context(ZoneId::Oakhaven, 0);
        assert_eq!(oakhaven.recommended, "Town Sanctuary");
        let barrow = get_zone_context(ZoneId::Barrow(1), 0);
        assert_eq!(barrow.recommended, "Lv 2–4");
        let f1 = get_zone_context(ZoneId::Dungeon(1), 1);
        assert_eq!(f1.recommended, "Lv 1–2");
        let f5 = get_zone_context(ZoneId::Dungeon(5), 5);
        assert!(f5.tactical.contains("Malakor"));
        let abyss = get_zone_context(ZoneId::Dungeon(9), 9);
        assert_eq!(abyss.recommended, "Lv 13+");
        assert!(abyss.lore.contains("Depth 9"));
    }

}
