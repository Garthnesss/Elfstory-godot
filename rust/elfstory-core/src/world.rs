//! World generation, SLICE port of `elfstoryreborn/src/game/world.ts`.
//!
//! In scope (Milestone 1 vertical slice):
//! - `create_oakhaven_map` — hand-authored Oakhaven, Torin at (7, 4).
//! - `generate_dungeon_floor` — room/corridor generator with the
//!   orthogonal connectivity guarantee and, on Floor 1 of the main
//!   descent, the guaranteed 4 town items (3 Blacksmith pieces +
//!   Heartcoal) as ground loot.
//! - `create_enemy` / `promote_enemy_rank` — every spawn funnels through
//!   `difficulty::scale_enemy_for_difficulty`, then the KILL_XP_FACTOR
//!   trim, exactly as TS `createEnemy` does.
//!
//! Out of scope (Milestone 3, omitted — see the port report):
//! Highspire/Thornwick/Cinderwatch/Overworld maps, side-dungeon floors
//! and their town supplies, the sealed treasure vault, and the
//! spore-heart / rime-circle / sealed-brazier set pieces. The
//! sepulcher set piece (Floor 1) IS ported.
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

/// Port of `generateDungeonFloor` for the main descent. Floor 1 is the
/// M1 slice target; other depths generate through the same machinery
/// (story bosses on 2/4/5, mini-bosses on 1/3) minus the Milestone 3
/// pieces listed in the module docs.
pub fn generate_dungeon_floor(
    depth: i32,
    rng: &mut Rng,
    tier: DifficultyTier,
    cycle: u32,
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
    if depth == 5 {
        tiles[return_point.y as usize][return_point.x as usize].label =
            Some("Abyssal Descent (Unlocks after Malakor falls)".to_string());
    } else if depth >= 6 {
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
    if depth == 1 && rooms.len() > 2 {
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
    if (depth == 1 || depth == 2) && rooms.len() > 3 {
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
    if depth == 3 && rooms.len() > 1 {
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

    // --- set piece: Whispering Sepulcher (Floor 1) --------------------
    // Other set pieces are Milestone 3 (see module docs).
    let mut set_piece_rooms: HashSet<usize> = HashSet::new();
    let mut enemies: Vec<Enemy> = Vec::new();
    if depth == 1 {
        let pick = rooms
            .iter()
            .enumerate()
            .filter(|(idx, r)| {
                !reserved.contains(idx) && !special_rooms.contains(idx) && r.w >= 6 && r.h >= 5
            })
            .max_by_key(|(_, r)| r.w * r.h)
            .map(|(idx, r)| (idx, *r));
        if let Some((idx, room)) = pick {
            set_piece_rooms.insert(idx);
            paint_room_tint(&mut tiles, &room, "#3A362F");
            let (rx0, ry0, rw, rh, cx, cy) = (room.x, room.y, room.w, room.h, room.cx, room.cy);
            let decor_ok = |tiles: &[Vec<Tile>], x: i32, y: i32| {
                let k = tiles[y as usize][x as usize].kind;
                matches!(k, TileKind::Floor | TileKind::Pillar | TileKind::Torch)
                    && !(x == spawn_point.x && y == spawn_point.y)
                    && !(x == return_point.x && y == return_point.y)
            };
            let tile_ok = |tiles: &[Vec<Tile>], x: i32, y: i32| {
                tiles[y as usize][x as usize].kind == TileKind::Floor
                    && !(x == spawn_point.x && y == spawn_point.y)
                    && !(x == return_point.x && y == return_point.y)
            };
            for (px, py) in [
                (rx0 + 1, ry0 + 1),
                (rx0 + rw - 2, ry0 + 1),
                (rx0 + 1, ry0 + rh - 2),
                (rx0 + rw - 2, ry0 + rh - 2),
            ] {
                if decor_ok(&tiles, px, py) {
                    set_tile(&mut tiles, px, py, TileKind::Pillar, Some("Sepulcher Pillar"), None);
                }
            }
            if tile_ok(&tiles, cx - 2, cy) {
                set_tile(&mut tiles, cx - 2, cy, TileKind::Chest, Some("Stone Sarcophagus"), None);
            }
            if tile_ok(&tiles, cx + 2, cy) {
                set_tile(&mut tiles, cx + 2, cy, TileKind::Chest, Some("Stone Sarcophagus"), None);
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
                set_tile(&mut tiles, cx + 2, cy + 1, TileKind::Trap, None, None);
            }
            for (arch, spots) in [
                (EnemyArchetype::CryptSkeleton, [(cx - 1, cy - 1), (cx, cy - 1)]),
                (EnemyArchetype::PlagueRat, [(cx + 1, cy - 1), (cx, cy + 2)]),
            ] {
                for (sx, sy) in spots {
                    if tile_ok(&tiles, sx, sy)
                        && !enemies.iter().any(|e| e.x == sx && e.y == sy)
                    {
                        let seq = next_enemy(&mut enemy_seq);
                        enemies.push(create_enemy(arch, sx, sy, depth, tier, cycle, seq));
                        break;
                    }
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
    let pool = pool_for(depth);
    let mut rare_count = 0;
    let mut roaming_randboss_spawned = false;
    for (r_idx, room) in rooms.iter().skip(1).enumerate() {
        let is_final = r_idx == rooms.len() - 2;
        if is_final && (depth == 2 || depth == 4 || depth == 5) {
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
        if is_final && (depth == 1 || depth == 3) {
            let (arch, rank, boss_name, title) = if depth == 1 {
                (
                    EnemyArchetype::GoblinSkirmisher,
                    EnemyRank::Rare,
                    "Skrag, Reliquary Gatekeeper",
                    "Mini-Boss · Gatekeeper",
                )
            } else {
                (
                    EnemyArchetype::BlightWeaver,
                    EnemyRank::Randboss,
                    "Vaelkor, Blight-Weaver Warden",
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
            mini.name = boss_name.to_string();
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
    // clear of the floor. (TS gates this on the main descent; side
    // branches are Milestone 3 and never reach this function.)
    if depth == 1 {
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
}
