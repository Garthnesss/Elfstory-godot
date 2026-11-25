//! Oakhaven 3D model placement derivation, ported from the pure-logic parts
//! of `elfstoryreborn/src/game/modelLoader.ts`.
//!
//! Only the placement anchors + footprint derivation are ported here: the
//! THREE.js `GLTFLoader` template cache (`loadModelTemplate`) and scene
//! instantiation (`instantiatePlacedModel`) are renderer concerns that
//! belong to the Godot side in the rewrite, not to `elfstory-core`.
//!
//! Coordinates follow the TS source: `x` / `z` are world tile coordinates
//! (z is the tile-map row / Y), matching `createOakhavenMap`'s 28x17 layout.

use crate::town::{population_tier, BlacksmithLot, TownState};
use crate::types::Pos;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Torin's spawn anchor in Oakhaven: he stands at tile (7, 4), outside his
/// forge door — one row south of the forge .glb footprint (z 1..=3) — and
/// never at (7, 2) inside `blacksmith.glb`. Guardrail pinned by the TS
/// test 'town: Torin stands at (7, 4) outside the 3D forge mesh'.
pub const TORIN_SPAWN: Pos = Pos { x: 7, y: 4 };

pub fn torin_spawn() -> Pos {
    TORIN_SPAWN
}

/// Tile bounding box whose procedural terrain meshes are hidden once a
/// .glb mounts over it. Tiles under a footprint stay hidden across camera
/// cutaway toggles (enforced by the renderer; the data lives here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainFootprint {
    #[serde(rename = "minX")]
    pub min_x: i32,
    #[serde(rename = "maxX")]
    pub max_x: i32,
    #[serde(rename = "minZ")]
    pub min_z: i32,
    #[serde(rename = "maxZ")]
    pub max_z: i32,
}

impl TerrainFootprint {
    pub const fn new(min_x: i32, max_x: i32, min_z: i32, max_z: i32) -> Self {
        Self { min_x, max_x, min_z, max_z }
    }
    /// Inclusive containment, in tile coordinates (ty is the map row = z).
    pub fn contains(&self, tx: i32, ty: i32) -> bool {
        tx >= self.min_x && tx <= self.max_x && ty >= self.min_z && ty <= self.max_z
    }
    /// Containment expanded by `margin` tiles on every side.
    pub fn contains_with_margin(&self, tx: i32, ty: i32, margin: i32) -> bool {
        tx >= self.min_x - margin
            && tx <= self.max_x + margin
            && ty >= self.min_z - margin
            && ty <= self.max_z + margin
    }
}

/// Optional warm point light attached to a building (e.g. forge hearth).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlacementLight {
    pub color: u32,
    pub intensity: f64,
    pub distance: f64,
    pub offset: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldModelPlacement {
    pub id: String,
    /// Path under public/ (e.g. '/models/blacksmith.glb').
    pub url: String,
    /// World tile X coordinate (center of footprint).
    pub x: f64,
    /// World tile Z coordinate (row/Y in tile map).
    pub z: f64,
    /// Vertical offset above y = 0 ground plane.
    #[serde(default, rename = "yOffset", skip_serializing_if = "Option::is_none")]
    pub y_offset: Option<f64>,
    /// Y-axis rotation in radians.
    #[serde(default, rename = "rotationY", skip_serializing_if = "Option::is_none")]
    pub rotation_y: Option<f64>,
    /// Desired footprint width in world tile units (auto-scales uniformly).
    #[serde(rename = "targetWidthTiles")]
    pub target_width_tiles: f64,
    /// Optional tile bounding box whose procedural wall/door meshes are
    /// hidden once this .glb mounts.
    #[serde(default, rename = "hideTerrainFootprint", skip_serializing_if = "Option::is_none")]
    pub hide_terrain_footprint: Option<TerrainFootprint>,
    /// Optional warm point light attached to this building.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<PlacementLight>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LotWorldAnchor {
    pub lot: BlacksmithLot,
    pub x: f64,
    pub z: f64,
    #[serde(rename = "rotationY")]
    pub rotation_y: f64,
    pub label: String,
    /// Tile bounding box cleared beneath this lot so the rebuilt Blacksmith
    /// never clips into cottages, stalls, or trees.
    #[serde(rename = "hideTerrainFootprint")]
    pub hide_terrain_footprint: TerrainFootprint,
}

/// Single source of truth for one Oakhaven Blacksmith lot anchor in world
/// tile coordinates (matching createOakhavenMap's 28x17 layout in world).
pub fn oakhaven_lot_anchor(lot: BlacksmithLot) -> LotWorldAnchor {
    match lot {
        BlacksmithLot::Pondside => LotWorldAnchor {
            lot,
            // Between the breach and the spring, door toward the pond road.
            x: 19.0,
            z: 9.0,
            rotation_y: PI * 0.5,
            label: "Pondside Lot".to_string(),
            hide_terrain_footprint: TerrainFootprint::new(18, 20, 8, 10),
        },
        BlacksmithLot::Market => LotWorldAnchor {
            lot,
            // The north green behind the stalls, facing the trestle road.
            x: 15.0,
            z: 3.0,
            rotation_y: 0.0,
            label: "Market Green Lot".to_string(),
            hide_terrain_footprint: TerrainFootprint::new(14, 16, 2, 4),
        },
        BlacksmithLot::WestGate => LotWorldAnchor {
            lot,
            // The green just inside the gate, door toward the west road.
            x: 3.0,
            z: 10.0,
            rotation_y: PI * 0.5,
            label: "West Gate Lot".to_string(),
            hide_terrain_footprint: TerrainFootprint::new(2, 4, 9, 11),
        },
    }
}

/// All 3 lot anchors (the Rust stand-in for the TS `OAKHAVEN_LOT_ANCHORS`
/// record, which has no const-map equivalent).
pub fn oakhaven_lot_anchors() -> [LotWorldAnchor; 3] {
    [
        oakhaven_lot_anchor(BlacksmithLot::Pondside),
        oakhaven_lot_anchor(BlacksmithLot::Market),
        oakhaven_lot_anchor(BlacksmithLot::WestGate),
    ]
}

/// True when tile (tx, ty) is inside or adjacent (<= 1 tile outside) to the
/// active rebuilt Blacksmith lot footprint in Oakhaven, allowing the player
/// to physically interact with the rebuilt Blacksmith annex in-world.
pub fn is_near_blacksmith_lot(tx: i32, ty: i32, town: Option<&TownState>) -> bool {
    let Some(town) = town else { return false };
    if !town.blacksmith_complete {
        return false;
    }
    let Some(lot) = town.blacksmith_lot else { return false };
    let anchor = oakhaven_lot_anchor(lot);
    anchor.hide_terrain_footprint.contains_with_margin(tx, ty, 1)
}

/// The lot the player stands on or beside, if any.
pub fn lot_beside(tx: i32, ty: i32) -> Option<BlacksmithLot> {
    oakhaven_lot_anchors()
        .into_iter()
        .find(|a| a.hide_terrain_footprint.contains_with_margin(tx, ty, 1))
        .map(|a| a.lot)
}

/// Tiles a standing town building occupies: the rebuilt Blacksmith and the
/// settlers' cottages. Fields and trees stay walkable.
pub fn building_footprints(town: &TownState) -> Vec<TerrainFootprint> {
    get_oakhaven_model_placements(Some(town))
        .iter()
        .filter_map(|p| {
            if p.id.starts_with("major:") {
                p.hide_terrain_footprint
            } else if p.url == "procedural://village_cottage" {
                let (x, z) = (p.x.round() as i32, p.z.round() as i32);
                Some(TerrainFootprint::new(x - 1, x + 1, z - 1, z))
            } else {
                None
            }
        })
        .collect()
}

/// Every terrain footprint hidden by the placements for `town` — the set
/// the renderer's cutaway logic must keep hidden.
pub fn hidden_terrain_footprints(town: Option<&TownState>) -> Vec<TerrainFootprint> {
    get_oakhaven_model_placements(town)
        .iter()
        .filter_map(|p| p.hide_terrain_footprint)
        .collect()
}

/// Whether tile (tx, ty) lies under any mounted .glb footprint for `town`.
pub fn tile_is_under_model_footprint(tx: i32, ty: i32, town: Option<&TownState>) -> bool {
    hidden_terrain_footprints(town).iter().any(|fp| fp.contains(tx, ty))
}

fn placement(
    id: &str,
    url: &str,
    x: f64,
    z: f64,
    rotation_y: f64,
    target_width_tiles: f64,
    hide_terrain_footprint: Option<TerrainFootprint>,
    light: Option<PlacementLight>,
) -> WorldModelPlacement {
    WorldModelPlacement {
        id: id.to_string(),
        url: url.to_string(),
        x,
        z,
        y_offset: None,
        rotation_y: Some(rotation_y),
        target_width_tiles,
        hide_terrain_footprint,
        light,
    }
}

/// Pure derivation of which 3D `.glb` buildings/landmarks should be mounted
/// in Oakhaven for the current town state. Port of TS
/// `getOakhavenModelPlacements`.
pub fn get_oakhaven_model_placements(town: Option<&TownState>) -> Vec<WorldModelPlacement> {
    let mut placements: Vec<WorldModelPlacement> = vec![
        placement(
            "landmark:torin_forge",
            "procedural://cold_forge_ruin",
            7.0,
            2.0,
            0.0,
            4.4,
            Some(TerrainFootprint::new(5, 9, 1, 3)),
            None,
        ),
        placement(
            "landmark:mira_apothecary",
            "/models/mira_apothecary.glb",
            22.0,
            2.0,
            PI * 0.5,
            4.4,
            Some(TerrainFootprint::new(20, 24, 1, 3)),
            Some(PlacementLight { color: 0x7ef29a, intensity: 2.2, distance: 6.0, offset: [0.3, 0.9, 1.0] }),
        ),
        placement(
            "landmark:old_outpost_oak",
            "/models/oakhaven_oak.glb",
            1.0,
            1.0,
            0.4,
            2.8,
            Some(TerrainFootprint::new(1, 1, 1, 1)),
            None,
        ),
        placement(
            "prop:oak_north_west",
            "/models/oakhaven_oak.glb",
            12.0,
            1.0,
            1.8,
            2.3,
            Some(TerrainFootprint::new(12, 12, 1, 1)),
            None,
        ),
        placement(
            "prop:oak_north_east",
            "/models/oakhaven_oak.glb",
            17.0,
            1.0,
            3.3,
            2.2,
            Some(TerrainFootprint::new(17, 17, 1, 1)),
            None,
        ),
        placement(
            "prop:oak_south_green",
            "/models/oakhaven_oak.glb",
            11.0,
            15.0,
            4.7,
            2.3,
            Some(TerrainFootprint::new(11, 11, 15, 15)),
            None,
        ),
        placement(
            "prop:oak_pondside",
            "/models/oakhaven_oak.glb",
            19.0,
            14.0,
            2.5,
            2.1,
            Some(TerrainFootprint::new(19, 19, 14, 14)),
            None,
        ),
    ];

    if let Some(town) = town {
        if town.blacksmith_complete {
            if let Some(lot) = town.blacksmith_lot {
                let anchor = oakhaven_lot_anchor(lot);
                placements.push(WorldModelPlacement {
                    id: format!("major:blacksmith:{}", anchor.lot.as_str()),
                    url: "/models/blacksmith.glb".to_string(),
                    x: anchor.x,
                    z: anchor.z,
                    y_offset: None,
                    rotation_y: Some(anchor.rotation_y),
                    target_width_tiles: 2.8,
                    hide_terrain_footprint: Some(anchor.hide_terrain_footprint),
                    light: Some(PlacementLight {
                        color: 0xff8c2b,
                        intensity: 3.4,
                        distance: 7.5,
                        offset: [0.0, 0.9, 0.8],
                    }),
                });
            }
        }
    }

    // ActRaiser village growth structures mounted automatically as Population Tier rises.
    let tier = town.map_or(0, population_tier);
    if tier >= 1 {
        placements.push(placement(
            "growth:tier1_west_arbor",
            "/models/oakhaven_oak.glb",
            3.0,
            6.0,
            1.1,
            2.2,
            Some(TerrainFootprint::new(3, 3, 6, 6)),
            None,
        ));
        placements.push(placement(
            "growth:tier1_garden_field",
            "procedural://village_field",
            2.5,
            3.0,
            0.0,
            1.8,
            None,
            None,
        ));
    }
    if tier >= 2 {
        // Note: the TS tier-2 platypus placement has NO hideTerrainFootprint.
        placements.push(placement(
            "growth:tier2_pond_totem",
            "/models/oakhaven_platypus.glb",
            20.0,
            11.0,
            -PI * 0.35,
            1.15,
            None,
            None,
        ));
        placements.push(placement(
            "growth:tier2_settler_cottage",
            "procedural://village_cottage",
            9.0,
            14.0,
            0.0,
            2.2,
            None,
            Some(PlacementLight {
                color: 0xffb347,
                intensity: 1.8,
                distance: 4.5,
                offset: [0.0, 0.8, 0.9],
            }),
        ));
    }
    if tier >= 3 {
        placements.push(placement(
            "growth:tier3_east_grove",
            "/models/oakhaven_oak.glb",
            25.0,
            4.0,
            3.9,
            2.3,
            Some(TerrainFootprint::new(25, 25, 4, 4)),
            None,
        ));
        placements.push(placement(
            "growth:tier3_east_cottage",
            "procedural://village_cottage",
            16.0,
            14.0,
            0.0,
            2.2,
            None,
            Some(PlacementLight {
                color: 0xffb347,
                intensity: 1.8,
                distance: 4.5,
                offset: [0.0, 0.8, 0.9],
            }),
        ));
        placements.push(placement(
            "growth:tier3_harvest_field",
            "procedural://village_field",
            25.0,
            14.0,
            0.0,
            1.8,
            None,
            None,
        ));
    }

    placements
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::town::{complete_blacksmith, create_town_state, record_town_item, BLACKSMITH_PIECES, STALL_ITEM_TAG};

    fn completed_town(lot: BlacksmithLot) -> TownState {
        let mut s = create_town_state();
        for p in BLACKSMITH_PIECES {
            s = record_town_item(s, p.tag);
        }
        s = record_town_item(s, STALL_ITEM_TAG);
        complete_blacksmith(s, lot).state
    }

    #[test]
    fn torin_spawns_outside_forge_door() {
        let spawn = torin_spawn();
        assert_eq!((spawn.x, spawn.y), (7, 4));
        // The forge .glb footprint is x 5..=9, z 1..=3: Torin's tile (7,4)
        // is one row south of it — outside the mesh, never inside at (7,2).
        let forge = get_oakhaven_model_placements(None)
            .into_iter()
            .find(|p| p.id == "landmark:torin_forge")
            .unwrap();
        let fp = forge.hide_terrain_footprint.unwrap();
        assert!(!fp.contains(spawn.x, spawn.y));
        assert!(fp.contains(7, 2));
        assert_eq!((forge.x, forge.z), (7.0, 2.0));
    }

    #[test]
    fn all_lots_have_footprints() {
        // Guardrail: all 3 Blacksmith lots carry explicit
        // hide_terrain_footprint rects (HANDOFF §4.2).
        let anchors = oakhaven_lot_anchors();
        assert_eq!(anchors.len(), 3);
        let expected = [
            (BlacksmithLot::Pondside, 19.0, 9.0, TerrainFootprint::new(18, 20, 8, 10)),
            (BlacksmithLot::Market, 15.0, 3.0, TerrainFootprint::new(14, 16, 2, 4)),
            (BlacksmithLot::WestGate, 3.0, 10.0, TerrainFootprint::new(2, 4, 9, 11)),
        ];
        for (anchor, (lot, x, z, fp)) in anchors.iter().zip(expected.iter()) {
            assert_eq!(anchor.lot, *lot);
            assert_eq!((anchor.x, anchor.z), (*x, *z));
            assert_eq!(anchor.hide_terrain_footprint, *fp);
            assert!(fp.min_x <= fp.max_x && fp.min_z <= fp.max_z);
            // The anchor point itself sits inside its cleared footprint.
            assert!(fp.contains(anchor.x.round() as i32, anchor.z.round() as i32) || fp.contains(anchor.x.floor() as i32, anchor.z.floor() as i32));
        }
        // Footprints are pairwise distinct so lots never share a rect.
        for i in 0..3 {
            for j in (i + 1)..3 {
                assert_ne!(anchors[i].hide_terrain_footprint, anchors[j].hide_terrain_footprint);
            }
        }
    }

    #[test]
    fn base_placements_are_the_seven_landmarks() {
        let placements = get_oakhaven_model_placements(None);
        assert_eq!(placements.len(), 7);
        let ids: Vec<&str> = placements.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "landmark:torin_forge",
                "landmark:mira_apothecary",
                "landmark:old_outpost_oak",
                "prop:oak_north_west",
                "prop:oak_north_east",
                "prop:oak_south_green",
                "prop:oak_pondside",
            ]
        );
        // An incomplete town adds no major and no growth structures.
        assert_eq!(get_oakhaven_model_placements(Some(&create_town_state())).len(), 7);
    }

    #[test]
    fn completed_blacksmith_mounts_on_its_lot_footprint() {
        for lot in [BlacksmithLot::Pondside, BlacksmithLot::Market, BlacksmithLot::WestGate] {
            let town = completed_town(lot);
            let placements = get_oakhaven_model_placements(Some(&town));
            let major: Vec<_> = placements.iter().filter(|p| p.id.starts_with("major:blacksmith:")).collect();
            assert_eq!(major.len(), 1);
            let anchor = oakhaven_lot_anchor(lot);
            assert_eq!(major[0].id, format!("major:blacksmith:{}", lot.as_str()));
            assert_eq!((major[0].x, major[0].z), (anchor.x, anchor.z));
            assert_eq!(major[0].hide_terrain_footprint, Some(anchor.hide_terrain_footprint));
            assert_eq!(major[0].target_width_tiles, 2.8);
        }
    }

    #[test]
    fn near_blacksmith_lot_uses_footprint_plus_one_tile_margin() {
        let town = completed_town(BlacksmithLot::Pondside); // fp x 18..=20, z 8..=10
        assert!(is_near_blacksmith_lot(19, 9, Some(&town))); // inside
        assert!(is_near_blacksmith_lot(17, 9, Some(&town))); // 1 tile west
        assert!(is_near_blacksmith_lot(21, 11, Some(&town))); // 1 tile past corner
        assert!(!is_near_blacksmith_lot(22, 9, Some(&town))); // 2 tiles east
        assert!(!is_near_blacksmith_lot(19, 9, Some(&create_town_state()))); // not complete
        assert!(!is_near_blacksmith_lot(19, 9, None));
    }

    #[test]
    fn tier_growth_placements_follow_population_tier() {
        let at = |pop: i32| {
            let town = TownState { population: pop, ..create_town_state() };
            get_oakhaven_model_placements(Some(&town))
        };
        let ids = |ps: &[WorldModelPlacement]| ps.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
        assert!(!ids(&at(19)).iter().any(|i| i.starts_with("growth:")));
        let t1 = ids(&at(20));
        assert!(t1.contains(&"growth:tier1_west_arbor".to_string()));
        assert!(t1.contains(&"growth:tier1_garden_field".to_string()));
        let t2 = at(35);
        let t2_ids = ids(&t2);
        assert!(t2_ids.contains(&"growth:tier2_pond_totem".to_string()));
        assert!(t2_ids.contains(&"growth:tier2_settler_cottage".to_string()));
        // TS pins: the tier-2 platypus carries no terrain footprint.
        let platypus = t2.iter().find(|p| p.id == "growth:tier2_pond_totem").unwrap();
        assert_eq!(platypus.hide_terrain_footprint, None);
        let t3 = at(55);
        let t3_ids = ids(&t3);
        assert!(t3_ids.contains(&"growth:tier3_east_grove".to_string()));
        assert!(t3_ids.contains(&"growth:tier3_east_cottage".to_string()));
        assert!(t3_ids.contains(&"growth:tier3_harvest_field".to_string()));
        assert_eq!(t3.len(), 7 + 7);
    }

    #[test]
    fn footprint_tiles_stay_hidden_under_models() {
        // Cutaway guardrail (data side): tiles under a .glb footprint are
        // enumerable from the placements alone, for every town state.
        let town = completed_town(BlacksmithLot::Market);
        assert!(tile_is_under_model_footprint(15, 3, Some(&town))); // market lot
        assert!(tile_is_under_model_footprint(7, 2, Some(&town))); // forge mesh
        assert!(!tile_is_under_model_footprint(0, 16, Some(&town)));
        assert_eq!(hidden_terrain_footprints(Some(&town)).len(), 8); // 7 base + major
    }

    /// The whole point of re-placing the lots: every one is open ground in the
    /// town as drawn, clear of roads, water, people and every other structure.
    #[test]
    fn lots_and_growth_fit_the_sinkhole_oakhaven() {
        use crate::types::TileKind;
        let map = crate::world::create_oakhaven_map();
        let at = |x: i32, y: i32| &map.tiles[y as usize][x as usize];
        for a in oakhaven_lot_anchors() {
            let f = a.hide_terrain_footprint;
            for y in f.min_z..=f.max_z {
                for x in f.min_x..=f.max_x {
                    assert_eq!(at(x, y).kind, TileKind::Floor, "{:?} covers {:?} at {x},{y}", a.lot, at(x, y).kind);
                    assert!(!map.npcs.iter().any(|n| (n.x, n.y) == (x, y)), "{:?} covers someone at {x},{y}", a.lot);
                    assert_ne!((x, y), (map.spawn_point.x, map.spawn_point.y));
                }
            }
        }
        for lot in [BlacksmithLot::Pondside, BlacksmithLot::Market, BlacksmithLot::WestGate] {
            let mut town = completed_town(lot);
            town.population = 99;
            let mut taken = std::collections::HashSet::new();
            let cottages = building_footprints(&town);
            assert_eq!(cottages.len(), 3, "forge and two cottages");
            for f in cottages {
                for y in f.min_z..=f.max_z {
                    for x in f.min_x..=f.max_x {
                        assert!(taken.insert((x, y)), "two buildings share {x},{y}");
                        assert_eq!(at(x, y).kind, TileKind::Floor, "building on {:?} at {x},{y}", at(x, y).kind);
                        assert!(!map.npcs.iter().any(|n| (n.x, n.y) == (x, y)));
                    }
                }
            }
        }
    }
}
