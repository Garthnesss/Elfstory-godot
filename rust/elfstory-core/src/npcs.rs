//! NPC definitions, quest definitions, lore entries and merchant stock,
//! ported from `elfstoryreborn/src/game/npcs.ts`.
//!
//! All dialogue text, choice labels, quest summaries and lore are verbatim
//! from the TS source (comedy included — do not edit wording). Structural
//! notes:
//! - TS `NPCS` is a `Record<string, NpcDef>`; here [`npc_defs`] returns the
//!   defs in TS source order and [`npc_def`] looks one up by id.
//! - TS `portraitUrl` imports (bundled .jpg assets) map to
//!   [`NpcDef::portrait_file`] as the asset basename; NPCs without a
//!   `portraitUrl` in TS get `None`.
//! - TS dialogue conditions spell "quest not started yet" as
//!   `questStatus: undefined` alongside a `questId`; that maps to
//!   `quest_id: Some(..)` with `quest_status: None`.
//! - TS `createMerchantStock(merchantId, town?)` draws from the module
//!   RNG; here `gen` / `rng` are threaded explicitly, in TS draw order.
//!   The TS fall-through default (Torin's stock) serves merchant id
//!   'torin' AND any unknown id, exactly as in TS.

use crate::loot::{create_consumable, generate_procedural_item, ItemIdGen, ProceduralOptions};
use crate::rng::Rng;
use crate::town::{TownState, TownSupplyTag};
use crate::types::{
    AttributeKey, ConsumableKind, DialogueAction, DialogueChoice, DialogueCondition, DialogueNode,
    DialogueSkillCheck, Item, ItemRarity, ItemSlot, NpcDef, PortraitLine,
    QuestProgress, QuestStatus,
};
#[cfg(test)]
use crate::types::ItemCategory;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Small constructors (keep the data tables below readable)
// ---------------------------------------------------------------------------

fn pl(text: &str, color: &str) -> PortraitLine {
    PortraitLine {
        text: text.to_string(),
        color: color.to_string(),
    }
}

fn portrait(lines: &[(&str, &str)]) -> Vec<PortraitLine> {
    lines.iter().map(|(t, c)| pl(t, c)).collect()
}

/// Base choice with every optional field unset (TS object with only
/// id/label); fill the rest via struct-update syntax.
fn ch(id: &str, label: &str) -> DialogueChoice {
    DialogueChoice {
        id: id.to_string(),
        label: label.to_string(),
        next_node_id: None,
        check: None,
        fail_node_id: None,
        condition: None,
        action: None,
        extra_actions: Vec::new(),
    }
}

fn node(id: &str, text: &str, choices: Vec<DialogueChoice>) -> DialogueNode {
    DialogueNode {
        id: id.to_string(),
        speaker_name: None,
        text: text.to_string(),
        choices,
    }
}

fn node_map(list: Vec<DialogueNode>) -> HashMap<String, DialogueNode> {
    list.into_iter().map(|n| (n.id.clone(), n)).collect()
}

/// Base condition with every field unset.
fn cond() -> DialogueCondition {
    DialogueCondition {
        flag: None,
        flag_equals: None,
        quest_id: None,
        quest_status: None,
        quest_min_stage: None,
        quest_max_stage: None,
        has_item_tag: None,
        min_gold: None,
        not_flag: None,
    }
}

/// TS `{ questId, questStatus: undefined }` — quest not started yet.
fn quest_not_started(quest_id: &str) -> DialogueCondition {
    DialogueCondition {
        quest_id: Some(quest_id.to_string()),
        ..cond()
    }
}

/// TS `{ questId, questStatus: 'active', ... }`.
fn quest_active(quest_id: &str) -> DialogueCondition {
    DialogueCondition {
        quest_id: Some(quest_id.to_string()),
        quest_status: Some(QuestStatus::Active),
        ..cond()
    }
}

fn check(attribute: AttributeKey, threshold: i32) -> DialogueSkillCheck {
    DialogueSkillCheck {
        attribute,
        threshold,
    }
}

fn open_merchant(merchant_id: &str) -> DialogueAction {
    DialogueAction::OpenMerchant {
        merchant_id: merchant_id.to_string(),
    }
}

fn close_dialogue() -> DialogueAction {
    DialogueAction::CloseDialogue
}

fn start_quest(quest_id: &str, log: &str) -> DialogueAction {
    DialogueAction::StartQuest {
        quest_id: quest_id.to_string(),
        log: log.to_string(),
    }
}

fn set_flag_str(flag: &str, value: &str) -> DialogueAction {
    DialogueAction::SetFlag {
        flag: flag.to_string(),
        value: serde_json::Value::String(value.to_string()),
    }
}

fn set_flag_bool(flag: &str, value: bool) -> DialogueAction {
    DialogueAction::SetFlag {
        flag: flag.to_string(),
        value: serde_json::Value::Bool(value),
    }
}

fn set_flag_num(flag: &str, value: i64) -> DialogueAction {
    DialogueAction::SetFlag {
        flag: flag.to_string(),
        value: serde_json::Value::from(value),
    }
}

fn consume(tag: &str) -> DialogueAction {
    DialogueAction::ConsumeQuestItem {
        tag: tag.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn complete_quest(
    quest_id: &str,
    outcome_label: &str,
    log: &str,
    reward_gold: Option<i32>,
    reward_xp: Option<i32>,
    reward_skill_point: Option<i32>,
    reward_attr_point: Option<i32>,
    reward_item_tag: Option<&str>,
) -> DialogueAction {
    DialogueAction::CompleteQuest {
        quest_id: quest_id.to_string(),
        outcome_label: outcome_label.to_string(),
        log: log.to_string(),
        reward_gold,
        reward_xp,
        reward_skill_point,
        reward_attr_point,
        reward_item_tag: reward_item_tag.map(|s| s.to_string()),
    }
}

#[allow(clippy::too_many_arguments)]
fn npc(
    id: &str,
    name: &str,
    title: &str,
    color: &str,
    zone: &str,
    portrait_file: Option<&str>,
    merchant_id: Option<&str>,
    portrait_lines: Vec<PortraitLine>,
    nodes: HashMap<String, DialogueNode>,
) -> NpcDef {
    NpcDef {
        id: id.to_string(),
        name: name.to_string(),
        title: title.to_string(),
        glyph: "@".to_string(),
        color: color.to_string(),
        zone: zone.to_string(),
        portrait_file: portrait_file.map(|s| s.to_string()),
        portrait: portrait_lines,
        merchant_id: merchant_id.map(|s| s.to_string()),
        root_node_id: "root".to_string(),
        nodes,
    }
}

// ---------------------------------------------------------------------------
// Quest definitions (TS QUEST_DEFS)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct QuestDef {
    pub id: &'static str,
    pub title: &'static str,
    pub giver: &'static str,
    pub location: &'static str,
    pub summary: &'static str,
}

pub fn quest_defs() -> Vec<QuestDef> {
    vec![
        QuestDef {
            id: "shadows_beneath",
            title: "Shadows Beneath Oakhaven",
            giver: "Warden Vance / Vex",
            location: "Oakhaven & Floor 1",
            summary: "Retrieve the Sealed Contraband Ledger from Floor 1 of the Sunken Reliquary and decide whether to hand it to Warden Vance or Smuggler Vex.",
        },
        QuestDef {
            id: "herbalist_blight",
            title: "The Herbalist's Blight",
            giver: "Herbalist Mira",
            location: "Oakhaven & Floors 1–2",
            summary: "Collect Pale Nightbloom from the upper crypts, or commune with the Bone Cantor using your Resolve or Arcana to lift the miasma.",
        },
        QuestDef {
            id: "whispering_bell",
            title: "The Whispering Bell",
            giver: "Archivist Lyra / Corvus",
            location: "Highspire Citadel & Floor 3",
            summary: "Recover the resonant bronze bell from the Ember Vaults (Floor 3) and choose its fate: attune it with Lyra, sell it to Corvus, or shatter it yourself.",
        },
        QuestDef {
            id: "sovereign_cinder",
            title: "The Sovereign of Cinder",
            giver: "Guildmaster Kaelen",
            location: "Highspire & Floor 5",
            summary: "Descend to the Obsidian Sanctum on Floor 5 and end the reign of Malakor, Sovereign of Cinder.",
        },
        QuestDef {
            id: "outrider_oath",
            title: "Oath of the Silver Bough",
            giver: "Archivist Lyra / Elven Waystones",
            location: "Highspire & Floors 1–4",
            summary: "Attune 2 ancient Elven Waystones (◈) hidden in the sunken vaults to rekindle the Outrider beacon and claim an elven Starlight Relic.",
        },
        QuestDef {
            id: "silverwood_road",
            title: "The Silverbridge Toll",
            giver: "Rowan the Wayfarer",
            location: "Silverwood Vale Overworld",
            summary: "Cross Silverbridge in the Silverwood Vale Overworld, rout the brigands holding the eastern watchtower, and reclaim Rowan’s Stolen Satchel.",
        },
        QuestDef {
            id: "spores_of_bog",
            title: "Spores of the Bog",
            giver: "Elder Sylva",
            location: "Thornwick Fen & Fungal Hollow",
            summary: "Venture east of Thornwick Fen into Fungal Hollow (2 floors), slay Spore-Matron Vexis, and bring Elder Sylva the Bioluminescent Fungal Heart.",
        },
        QuestDef {
            id: "barrow_purge",
            title: "Wights of the Barrow",
            giver: "Captain Darek",
            location: "Cinderwatch & Whispering Barrow",
            summary: "Descend into Whispering Barrow in northern Silverwood Vale (2 floors) and bring Captain Darek Arch-Wight Morvath’s Barrow Seal.",
        },
        QuestDef {
            id: "cinder_and_frost",
            title: "Heart of the Caldera",
            giver: "Forgemistress Halda",
            location: "Cinderwatch & Frostpeak Cavern",
            summary: "Conquer the 3 depths of Frostpeak & Ashen Caldera northeast of Cinderwatch and return Ignis-Warden Kaelthar’s Ashen Ore Heart to Halda.",
        },
        QuestDef {
            id: "deep_survey",
            title: "Charting the Deep Roads",
            giver: "Rowan the Wayfarer",
            location: "Silverwood Vale & the Sunken Vaults",
            summary: "No blade required: transcribe 6 carved steles (‡) scattered across towns, the Vale, and the vaults to finish Rowan’s survey of the deep roads.",
        },
    ]
}

pub fn quest_def(id: &str) -> Option<QuestDef> {
    quest_defs().into_iter().find(|q| q.id == id)
}

// ---------------------------------------------------------------------------
// Lore entries (TS LORE_ENTRIES)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct LoreEntry {
    pub id: &'static str,
    pub title: &'static str,
    pub text: &'static str,
}

pub fn lore_entries() -> Vec<LoreEntry> {
    vec![
        LoreEntry {
            id: "lore_oakhaven_post",
            title: "Oakhaven Gate Ledger",
            text: "Founded beside the sunken seal after the First Ember, Oakhaven stands watch over the descent stairs. Those who enter the Reliquary rarely return empty-handed—if they return at all.",
        },
        LoreEntry {
            id: "lore_highspire_archive",
            title: "Marble Edict of Highspire",
            text: "Only bearers of the Citadel Seal or proved delve-wardens may walk Highspire’s upper galleries. Beneath our marble fountains, three tiers of basalt vaults hold the Cinder Sovereign’s crown.",
        },
        LoreEntry {
            id: "lore_floor_1",
            title: "Etched Wall Warning",
            text: "\"Listen before stepping through dark arches. The skeletons wind their heavy blades before swinging; a nimble step or timely shield bash breaks their rhythm.\"",
        },
        LoreEntry {
            id: "lore_floor_2",
            title: "Cantor Vaelen’s Dirge",
            text: "\"We did not turn to bone out of malice. We were bound to guard the lower bell until someone spoke the old words of release.\"",
        },
        LoreEntry {
            id: "lore_floor_3",
            title: "Inquisitor Tablet",
            text: "\"Stone golems cannot be rushed headlong. Watch where their shadow falls before the ground slams—or shatter their plating with ember lances.\"",
        },
        LoreEntry {
            id: "lore_floor_4",
            title: "Crucible Inscription",
            text: "\"Three masteries anchor the old order: Steel to hold the breach, Shadow to strike the unseen seam, and Rune to bend the ember’s breath.\"",
        },
        LoreEntry {
            id: "lore_floor_5",
            title: "Sovereign’s Epitaph",
            text: "\"Malakor forged the Sunken Citadel not to hoard gold, but to outlast the fading of his own name. Speak firmly before drawing steel.\"",
        },
        LoreEntry {
            id: "lore_waystone",
            title: "Outrider Waystone Hymn",
            text: "\"Before Highspire raised marble walls, elven Outriders set starlight stones along these subterranean roads so no lost sentinel would walk in utter dark.\"",
        },
        LoreEntry {
            id: "lore_forge",
            title: "Anvil Ledger of Torin’s Kin",
            text: "\"Deep-vault coal burns hotter than surface oak. Even a cracked blade can drink deep-vault heat here and emerge keen enough to cleave obsidian.\"",
        },
        LoreEntry {
            id: "lore_altar",
            title: "Edict of the Scarlet Pact",
            text: "\"Power purchased in blood lingers in steel. Those of steady Resolve may cleanse the altar instead—turning malice into quiet clarity.\"",
        },
        LoreEntry {
            id: "lore_overworld_vale",
            title: "Silverbridge Milestone",
            text: "\"West to Oakhaven’s timber palisade; north past Whispering Barrow to Cinderwatch Bastion and Frostpeak; southeast through the fen to Thornwick and the Fungal Hollow.\"",
        },
        LoreEntry {
            id: "lore_thornwick_post",
            title: "Thornwick Stilt-Post Chronicle",
            text: "\"Built on ironwood pilings above rising bog-water, Thornwick endures only while the druid wards hold back the Fungal Hollow’s spore-bloom.\"",
        },
        LoreEntry {
            id: "lore_cinderwatch_post",
            title: "Cinderwatch Ridge Tablet",
            text: "\"Where glacial aquifers meet the Caldera’s magma vents, meteoric cinder-iron forms—fiercer than any steel smelted in Highspire.\"",
        },
        LoreEntry {
            id: "lore_barrow",
            title: "Whispering Barrow Epitaph",
            text: "\"Do not speak Morvath’s true title aloud in the crypts. The wight-lord hears steel draw three halls away.\"",
        },
        LoreEntry {
            id: "lore_mire",
            title: "Mycelial Ward Note",
            text: "\"Blight pools sap breath and marrow alike, yet those quick of step or rich in arcana can harvest gold from the glowing spores.\"",
        },
        LoreEntry {
            id: "lore_frost",
            title: "Frostpeak Miner’s Warning",
            text: "\"Ice above, ember below. Carry healing draughts and clear the rime vents carefully before facing the Caldera Warden.\"",
        },
        LoreEntry {
            id: "lore_roadside_shrine",
            title: "Roadside Shrine of the Vale",
            text: "\"For the road that watches back. Travelers leave a copper, a crust, or a confession — the Vale accepts all three currencies.\" Weathered offerings crowd the shrine’s base: bent nails, bird skulls, one very small boot.",
        },
    ]
}

pub fn lore_entry(id: &str) -> Option<LoreEntry> {
    lore_entries().into_iter().find(|l| l.id == id)
}

// ---------------------------------------------------------------------------
// NPC definitions (TS NPCS)
// ---------------------------------------------------------------------------

fn torin_def() -> NpcDef {
    npc(
        "torin",
        "Torin Ironhand",
        "Blacksmith & Forgemaster",
        "#F59E0B",
        "oakhaven",
        Some("portrait_torin_1794537782658.jpg"),
        Some("torin"),
        portrait(&[
            (r"   .=======.   ", "#94A3B8"),
            (r"  /  _   _  \  ", "#E2E8F0"),
            (r" |  (o) (o)  | ", "#F59E0B"),
            (r" |   \___/   | ", "#CBD5E1"),
            (r"  \__#####__/  ", "#78716C"),
            (r"  /[=======]\  ", "#D97706"),
        ]),
        node_map(vec![
            node(
                "root",
                "Sparks drift from Torin’s anvil as he sets down a heavy hammer. \"Elfstrom. You’re back, and still carrying that sword like it owes you money. Whatever you haul up from the Reliquary, my forge can temper it into something worthy of the lower vaults. Can’t do anything about the rest of you.\"",
                vec![
                    DialogueChoice {
                        action: Some(open_merchant("torin")),
                        ..ch("trade", "Browse Armory & Arcane Forge")
                    },
                    DialogueChoice {
                        check: Some(check(AttributeKey::Might, 3)),
                        next_node_id: Some("craft_pass".to_string()),
                        fail_node_id: Some("craft_fail".to_string()),
                        ..ch(
                            "ask_craft",
                            "[Might 3] “Show me the anvil secrets. I’ll pretend I already knew them.”",
                        )
                    },
                    DialogueChoice {
                        action: Some(close_dialogue()),
                        ..ch("leave", "“I’ll let you get back to it. You’re welcome for the visit.”")
                    },
                ],
            ),
            node(
                "craft_pass",
                "Torin eyes your stance and nods in genuine approval. \"You’ve got a warrior’s grip. Salvage dull trinkets in your pack for gold—bring it to my Temper tab and I’ll push any weapon or armor past its limits.\"",
                vec![
                    DialogueChoice {
                        action: Some(open_merchant("torin")),
                        ..ch("open_shop", "Open Forge & Armory")
                    },
                    DialogueChoice {
                        next_node_id: Some("root".to_string()),
                        ..ch("back", "Thank him and return.")
                    },
                ],
            ),
            node(
                "craft_fail",
                "Torin chuckles softly. \"Build your strength first, traveler. Until then, let my bellows do the heavy lifting—bring gold whenever you need an edge.\"",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("back", "Back")
                }],
            ),
        ]),
    )
}

fn mira_def() -> NpcDef {
    npc(
        "mira",
        "Herbalist Mira",
        "Oakhaven Apothecary",
        "#4EA885",
        "oakhaven",
        Some("portrait_mira_1794537782634.jpg"),
        Some("mira"),
        portrait(&[
            (r"    .~~~~~.", "#34D399"),
            (r"   / .   . \", "#D1FAE5"),
            (r"  |  ^   ^  |", "#6EE7B7"),
            (r"  |   \_/   |", "#A7F3D0"),
            (r"   \_______/", "#10B981"),
            (r"  {✿} [!] {✿}", "#F59E0B"),
        ]),
        node_map(vec![
            node(
                "root",
                "Glass phials chime gently along Mira’s shelves. \"Oh, it’s you, dear. Sit, sit — no, not on the mandrake. The Reliquary air carries a creeping chill lately. Need draughts brewed, wounds dressed, or word on the blight below? Or just someone to tell your story to? Again?\"",
                vec![
                    DialogueChoice {
                        action: Some(open_merchant("mira")),
                        ..ch("buy_potions", "Browse Draughts & Scrolls")
                    },
                    DialogueChoice {
                        action: Some(DialogueAction::HealFull),
                        next_node_id: Some("healed".to_string()),
                        ..ch(
                            "heal_free",
                            "Rest by her hearth to mend wounds (Restore Full HP & MP)",
                        )
                    },
                    DialogueChoice {
                        condition: Some(quest_not_started("herbalist_blight")),
                        next_node_id: Some("blight_intro".to_string()),
                        ..ch("quest_offer", "Ask about the creeping blight in the crypts.")
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            has_item_tag: Some("pale_nightbloom".to_string()),
                            ..quest_active("herbalist_blight")
                        }),
                        next_node_id: Some("blight_herb_done".to_string()),
                        ..ch("turn_in_herb", "Deliver the Pale Nightblooms.")
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            flag: Some("cantorDefeated".to_string()),
                            flag_equals: Some(serde_json::Value::Bool(true)),
                            ..quest_active("herbalist_blight")
                        }),
                        next_node_id: Some("blight_cantor_done".to_string()),
                        ..ch(
                            "turn_in_cantor",
                            "Report that the Bone Cantor has been appeased.",
                        )
                    },
                    DialogueChoice {
                        action: Some(close_dialogue()),
                        ..ch("leave", "Leave the apothecary.")
                    },
                ],
            ),
            node(
                "healed",
                "Warm sage steam clears your lungs. Your wounds knit cleanly and your mana steadies.",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("ok", "Thank Mira.")
                }],
            ),
            node(
                "blight_intro",
                "\"Pale Nightblooms grow on the first two crypt floors—bring me a sprig and I can distill stronger healing draughts. Though... rumor says the Bone Cantor on Floor 2 is the true source. If you can reach him, perhaps he can be quelled.\"",
                vec![
                    DialogueChoice {
                        action: Some(start_quest(
                            "herbalist_blight",
                            "Agreed to aid Herbalist Mira with the Reliquary blight.",
                        )),
                        next_node_id: Some("blight_accepted".to_string()),
                        ..ch(
                            "accept_blight",
                            "I will bring back Pale Nightbloom or quell the Cantor.",
                        )
                    },
                    DialogueChoice {
                        next_node_id: Some("root".to_string()),
                        ..ch("decline", "Maybe later.")
                    },
                ],
            ),
            node(
                "blight_accepted",
                "\"Watch your footing near the damp alcoves—and take a Crimson Draught with my blessing.\"",
                vec![DialogueChoice {
                    action: Some(close_dialogue()),
                    ..ch("done", "Head back out.")
                }],
            ),
            node(
                "blight_herb_done",
                "Mira’s eyes brighten as she lifts the luminous petals. \"Flawless! With these spores distilled, every Crimson Draught you drink will mend far deeper wounds.\"",
                vec![DialogueChoice {
                    action: Some(consume("pale_nightbloom")),
                    extra_actions: vec![
                        set_flag_str("blightChoice", "herbs_gathered"),
                        set_flag_num("miraPotionTier", 2),
                        complete_quest(
                            "herbalist_blight",
                            "Distilled Pale Nightbloom",
                            "Delivered Pale Nightbloom to Mira; potions permanently upgraded.",
                            Some(55),
                            Some(60),
                            None,
                            None,
                            None,
                        ),
                    ],
                    next_node_id: Some("root".to_string()),
                    ..ch(
                        "finish_herb",
                        "Accept her reward (+55g, +60 XP, Stronger Potions)",
                    )
                }],
            ),
            node(
                "blight_cantor_done",
                "Mira exhales in relief as the cool breeze shifts outside. \"You silenced the Cantor’s dirge! Even the stones feel lighter. Take this gold and my deepest thanks.\"",
                vec![DialogueChoice {
                    action: Some(set_flag_str("blightChoice", "cantor_appeased")),
                    extra_actions: vec![
                        set_flag_num("miraPotionTier", 2),
                        complete_quest(
                            "herbalist_blight",
                            "Silenced the Bone Cantor",
                            "Ended the Cantor’s miasma on Floor 2; earned Mira’s mastery reward.",
                            Some(85),
                            Some(85),
                            Some(1),
                            None,
                            None,
                        ),
                    ],
                    next_node_id: Some("root".to_string()),
                    ..ch(
                        "finish_cantor",
                        "Accept her reward (+85g, +85 XP, +1 Skill Point)",
                    )
                }],
            ),
        ]),
    )
}

fn vance_def() -> NpcDef {
    npc(
        "vance",
        "Warden Vance",
        "Oakhaven Outpost Captain",
        "#60A5FA",
        "oakhaven",
        Some("portrait_vance_1794537783424.jpg"),
        None,
        portrait(&[
            (r"   .-[===]-.   ", "#60A5FA"),
            (r"  |  |   |  |  ", "#93C5FD"),
            (r"  | (•) (•) |  ", "#E2E8F0"),
            (r"  |   ===   |  ", "#94A3B8"),
            (r"   \_______/   ", "#475569"),
            (r"  [⛨=======⛨]  ", "#3B82F6"),
        ]),
        node_map(vec![
            node(
                "root",
                "Captain Vance studies a weathered map pinned with brass tacks. \"Someone has been smuggling sealed relics up from Floor 1 while my patrols are stretched thin. And now the realm has sent me… you.\" He looks you up and down. \"Speak plainly — and briefly. What brings you to the gate?\"",
                vec![
                    DialogueChoice {
                        condition: Some(quest_not_started("shadows_beneath")),
                        next_node_id: Some("shadows_offer".to_string()),
                        ..ch(
                            "start_shadows",
                            "“Smugglers? Say no more. I’ll find your cache on Floor 1.”",
                        )
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            has_item_tag: Some("smuggler_ledger".to_string()),
                            ..quest_active("shadows_beneath")
                        }),
                        next_node_id: Some("ledger_vance".to_string()),
                        ..ch(
                            "turn_in_ledger",
                            "“One Sealed Contraband Ledger. You’re welcome. Applause optional.”",
                        )
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            flag: Some("citadelAccess".to_string()),
                            flag_equals: Some(serde_json::Value::Bool(false)),
                            ..cond()
                        }),
                        next_node_id: Some("citadel_info".to_string()),
                        ..ch(
                            "citadel_ask",
                            "“Highspire. Big castle. How do I get in? Asking for a hero.”",
                        )
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            flag: Some("citadelAccess".to_string()),
                            flag_equals: Some(serde_json::Value::Bool(false)),
                            ..cond()
                        }),
                        check: Some(check(AttributeKey::Resolve, 3)),
                        next_node_id: Some("citadel_unlocked_early".to_string()),
                        fail_node_id: Some("citadel_denied".to_string()),
                        ..ch(
                            "persuade_gate",
                            "[Resolve 3] “Give me a Citadel Seal. Oakhaven needs reinforcements, and Highspire needs me.”",
                        )
                    },
                    DialogueChoice {
                        check: Some(check(AttributeKey::Agility, 2)),
                        next_node_id: Some("outrider_vance_pass".to_string()),
                        fail_node_id: Some("outrider_vance_fail".to_string()),
                        ..ch(
                            "outrider_heritage",
                            "[Agility 2] Ask about earlier elven scouts in the Reliquary.",
                        )
                    },
                    DialogueChoice {
                        action: Some(DialogueAction::SwitchCompanion),
                        next_node_id: Some("root".to_string()),
                        ..ch(
                            "vance_companion",
                            "Swap Spirit Companion (Silverwind Lynx <-> Starlight Owl)",
                        )
                    },
                    DialogueChoice {
                        action: Some(close_dialogue()),
                        ..ch("leave", "Dismiss yourself.")
                    },
                ],
            ),
            node(
                "outrider_vance_pass",
                "Vance nods with respect at your elven cloak. \"Two Outriders ventured below before the seal fractured. Look for glowing Elven Waystones (◈) down there—attuning them will steady your spirit and point Archivist Lyra toward their legacy.\"",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("back", "Keep watch.")
                }],
            ),
            node(
                "outrider_vance_fail",
                "\"Outriders moved too swift for our garrison to track. Sharpen your Agility and watch for blue starlight stones in the crypts.\"",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("back", "Back")
                }],
            ),
            node(
                "shadows_offer",
                "\"Good. Check the eastern chambers on Floor 1 for a Sealed Contraband Ledger. Bring it straight to me—and don’t let that hooded broker Vex talk you into turning traitor.\"",
                vec![DialogueChoice {
                    action: Some(start_quest(
                        "shadows_beneath",
                        "Warden Vance asked you to recover the Sealed Contraband Ledger on Floor 1.",
                    )),
                    next_node_id: Some("root".to_string()),
                    ..ch("accept", "I will investigate Floor 1.")
                }],
            ),
            node(
                "ledger_vance",
                "Vance breaks the wax seal and his jaw tightens. \"Highspire names... just as I feared. You’ve done Oakhaven a true service. Take my Guard Oathplate, gold, and the Citadel Seal—use the West Gate anytime.\"",
                vec![DialogueChoice {
                    action: Some(consume("smuggler_ledger")),
                    extra_actions: vec![
                        set_flag_str("caravanChoice", "arrested_vex"),
                        DialogueAction::UnlockHighspire,
                        complete_quest(
                            "shadows_beneath",
                            "Sided with Warden Vance",
                            "Delivered the Contraband Ledger to Vance; earned Warden’s Oathplate & Citadel Access.",
                            Some(80),
                            Some(90),
                            None,
                            None,
                            Some("warden_oathplate"),
                        ),
                    ],
                    next_node_id: Some("root".to_string()),
                    ..ch(
                        "claim_vance",
                        "Accept Warden’s Oathplate & Citadel Access (+80g, +90 XP)",
                    )
                }],
            ),
            node(
                "citadel_info",
                "\"Highspire’s road is sealed to drifters. Either prove your loyalty with the Floor 1 ledger, strike your own way through the underground, or convince me you have the spine of a true warden.\"",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("back", "Understood.")
                }],
            ),
            node(
                "citadel_unlocked_early",
                "Vance meets your gaze for three long seconds, then tosses an iron passkey onto the table. \"Fair enough. Highspire needs steel more than paperwork. The West Gate is open to you.\"",
                vec![DialogueChoice {
                    action: Some(DialogueAction::UnlockHighspire),
                    next_node_id: Some("root".to_string()),
                    ..ch("take_seal", "Take the Citadel Passkey")
                }],
            ),
            node(
                "citadel_denied",
                "\"Words are cheap in frontier mud, traveler. Prove yourself in the Reliquary first.\"",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("back", "Back")
                }],
            ),
        ]),
    )
}

fn vex_def() -> NpcDef {
    npc(
        "vex",
        "Vex the Quiet",
        "Shadow Broker",
        "#C084FC",
        "oakhaven",
        Some("portrait_vex_1794537783530.jpg"),
        Some("vex"),
        portrait(&[
            (r"    .-------.  ", "#7E22CE"),
            (r"   /  #####  \ ", "#A855F7"),
            (r"  |  (•) (•)  |", "#FDE047"),
            (r"  |   \___/   |", "#C084FC"),
            (r"   \_________/ ", "#581C87"),
            (r"   //  †††  \\ ", "#9333EA"),
        ]),
        node_map(vec![
            node(
                "root",
                "Vex leans out of the alley shade, flipping a tarnished silver coin across her knuckles. \"Psst. Over here. No — quieter. Quieter than that. Do you know what quiet is?\" She sighs. \"Vance marches in circles while real treasure rots below. Looking to trade rare curios — or make an arrangement?\"",
                vec![
                    DialogueChoice {
                        action: Some(open_merchant("vex")),
                        ..ch("black_market", "Inspect Hidden Stock")
                    },
                    DialogueChoice {
                        condition: Some(quest_not_started("shadows_beneath")),
                        next_node_id: Some("vex_offer".to_string()),
                        ..ch("start_vex_quest", "Ask what \"arrangement\" she has in mind.")
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            has_item_tag: Some("smuggler_ledger".to_string()),
                            ..quest_active("shadows_beneath")
                        }),
                        next_node_id: Some("ledger_vex".to_string()),
                        ..ch(
                            "give_ledger_vex",
                            "Slip her the Sealed Contraband Ledger instead of Vance.",
                        )
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            not_flag: Some("vexTipClaimed".to_string()),
                            ..cond()
                        }),
                        check: Some(check(AttributeKey::Agility, 3)),
                        next_node_id: Some("shortcut_pass".to_string()),
                        fail_node_id: Some("shortcut_fail".to_string()),
                        ..ch(
                            "cunning_tip",
                            "[Agility 3] Press her for smuggler shortcuts into the deeper vaults.",
                        )
                    },
                    DialogueChoice {
                        action: Some(close_dialogue()),
                        ..ch("leave", "Step out of the alley.")
                    },
                ],
            ),
            node(
                "vex_offer",
                "\"One of my runners dropped a Sealed Contraband Ledger on Floor 1. Vance wants it to hang half of Highspire. Bring it to *me*, and I’ll hand you my Whisperblade plus a forged Citadel pass.\"",
                vec![DialogueChoice {
                    action: Some(start_quest(
                        "shadows_beneath",
                        "Vex offered a generous bounty if you bring the Floor 1 ledger to her.",
                    )),
                    next_node_id: Some("root".to_string()),
                    ..ch("accept_vex", "Deal. I’ll look for the ledger.")
                }],
            ),
            node(
                "ledger_vex",
                "Vex grins sharply as the ledger vanishes inside her cloak. \"Smart choice. Here—my Whisperblade, forged Highspire papers, and a secret stairwell shortcut straight down to the Ember Vaults.\"",
                vec![DialogueChoice {
                    action: Some(consume("smuggler_ledger")),
                    extra_actions: vec![
                        set_flag_str("caravanChoice", "smuggler_pact"),
                        DialogueAction::UnlockHighspire,
                        set_flag_bool("floor3Shortcut", true),
                        complete_quest(
                            "shadows_beneath",
                            "Struck Pact with Vex",
                            "Delivered the Contraband Ledger to Vex; gained Whisperblade & Floor 3 Shortcut.",
                            Some(110),
                            Some(90),
                            None,
                            None,
                            Some("vex_whisperblade"),
                        ),
                    ],
                    next_node_id: Some("root".to_string()),
                    ..ch(
                        "claim_vex",
                        "Accept Vex's Whisperblade & Citadel Access (+110g, +90 XP)",
                    )
                }],
            ),
            node(
                "shortcut_pass",
                "\"Quick fingers, quick eyes. Remember: press [Space] or click a wall trap only after spotting it, and always keep a Scroll of Phase for enclosed boss rooms.\" She slips you 15 gold for the banter.",
                vec![DialogueChoice {
                    action: Some(DialogueAction::GiveGold { amount: 15 }),
                    extra_actions: vec![set_flag_bool("vexTipClaimed", true)],
                    next_node_id: Some("root".to_string()),
                    ..ch("take_tip", "Pocket the advice (+15g)")
                }],
            ),
            node(
                "shortcut_fail",
                "\"Information isn’t charity, friend. Sharpen your reflexes first.\"",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("back", "Back")
                }],
            ),
        ]),
    )
}

fn lyra_def() -> NpcDef {
    npc(
        "lyra",
        "Archivist Lyra",
        "Keeper of Glyphs",
        "#60A5FA",
        "highspire",
        Some("portrait_lyra_1794537782827.jpg"),
        None,
        portrait(&[
            (r"    .◈◈◈.    ", "#60A5FA"),
            (r"   /  _  \   ", "#DBEAFE"),
            (r"  | (✦)(✦) | ", "#93C5FD"),
            (r"  |  \__/  | ", "#BFDBFE"),
            (r"   \______/  ", "#3B82F6"),
            (r"  [❖======❖] ", "#1D4ED8"),
        ]),
        node_map(vec![
            node(
                "root",
                "Floating chalk runes orbit Lyra’s lectern. \"Welcome to Highspire’s Archive. Please do not lick the scrolls; the last hero did.\" She checks a list. \"The Ember Vaults are waking, and the Whispering Bell on Floor 3 is ringing again. The prophecy mentions a hero. It does not mention a nickname.\"",
                vec![
                    DialogueChoice {
                        condition: Some(quest_not_started("whispering_bell")),
                        next_node_id: Some("bell_intro".to_string()),
                        ..ch("start_bell", "Ask about the Whispering Bell on Floor 3.")
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            has_item_tag: Some("whispering_bell".to_string()),
                            ..quest_active("whispering_bell")
                        }),
                        next_node_id: Some("bell_lyra_complete".to_string()),
                        ..ch(
                            "turn_in_bell_lyra",
                            "Present the Whispering Bell for arcane attunement.",
                        )
                    },
                    DialogueChoice {
                        condition: Some(quest_not_started("outrider_oath")),
                        next_node_id: Some("outrider_intro".to_string()),
                        ..ch(
                            "start_outrider",
                            "Ask about the ancient Elven Waystones below.",
                        )
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            quest_min_stage: Some(3),
                            ..quest_active("outrider_oath")
                        }),
                        next_node_id: Some("outrider_reward".to_string()),
                        ..ch(
                            "complete_outrider",
                            "Report that 2 Elven Waystones are rekindled!",
                        )
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            not_flag: Some("cipherInscribed".to_string()),
                            ..cond()
                        }),
                        check: Some(check(AttributeKey::Arcana, 3)),
                        next_node_id: Some("malakor_cipher".to_string()),
                        fail_node_id: Some("cipher_fail".to_string()),
                        ..ch(
                            "arcana_lore",
                            "[Arcana 3] Study the resonance ciphers protecting Malakor on Floor 5.",
                        )
                    },
                    DialogueChoice {
                        action: Some(close_dialogue()),
                        ..ch("leave", "Close the tome.")
                    },
                ],
            ),
            node(
                "outrider_intro",
                "\"An elven outrider of your line! Two Starlight Waystones remain dormant beneath Highspire. Attune any two glowing Waystones (◈) in the vaults, and I will unseal an Outrider Relic from our vault for you.\"",
                vec![DialogueChoice {
                    action: Some(start_quest(
                        "outrider_oath",
                        "Archivist Lyra asked Garth \"Big Dick\" Elfstrom to attune 2 Elven Waystones (◈) in the sunken vaults.",
                    )),
                    next_node_id: Some("root".to_string()),
                    ..ch("accept_outrider", "I swear the Oath of the Silver Bough.")
                }],
            ),
            node(
                "outrider_reward",
                "Silver dust rises from Lyra’s astrolabe as the subterranean beacons answer in unison. \"The Silver Bough shines again, Garth \"Big Dick\" Elfstrom. Choose which Starlight Relic shall accompany you to the Sanctum.\"",
                vec![
                    DialogueChoice {
                        action: Some(complete_quest(
                            "outrider_oath",
                            "Claimed Sylvanthorn Longblade",
                            "Rekindled the Elven Waystones and claimed the Sylvanthorn Longblade from Archivist Lyra.",
                            Some(100),
                            Some(110),
                            None,
                            None,
                            Some("sylvan_longblade"),
                        )),
                        next_node_id: Some("root".to_string()),
                        ..ch(
                            "take_sylvan_blade",
                            "Claim Sylvanthorn Longblade (+100g, +110 XP)",
                        )
                    },
                    DialogueChoice {
                        action: Some(complete_quest(
                            "outrider_oath",
                            "Claimed Starlight Mantle",
                            "Rekindled the Elven Waystones and claimed the Starlight Outrider Mantle from Archivist Lyra.",
                            Some(100),
                            Some(110),
                            None,
                            None,
                            Some("starlight_mantle"),
                        )),
                        next_node_id: Some("root".to_string()),
                        ..ch(
                            "take_starlight_mantle",
                            "Claim Starlight Outrider Mantle (+100g, +110 XP)",
                        )
                    },
                ],
            ),
            node(
                "bell_intro",
                "\"Bring the Whispering Bell to me from Floor 3 so I can attune it against Malakor’s wards. Beware—Magistrate Corvus wants to auction it to mercenaries instead.\"",
                vec![DialogueChoice {
                    action: Some(start_quest(
                        "whispering_bell",
                        "Archivist Lyra asked you to recover the Whispering Bell from Floor 3.",
                    )),
                    next_node_id: Some("root".to_string()),
                    ..ch("accept_bell", "I will retrieve the Whispering Bell.")
                }],
            ),
            node(
                "bell_lyra_complete",
                "Lyra strikes the bronze rim with a silver rod. A clear chord echoes through the stone, shattering dormant curses. \"Take the Chime of First Ash and a new mastery point—you’ve earned both.\"",
                vec![DialogueChoice {
                    action: Some(consume("whispering_bell")),
                    extra_actions: vec![
                        set_flag_str("bellChoice", "attuned_lyra"),
                        set_flag_bool("malakorWeakened", true),
                        complete_quest(
                            "whispering_bell",
                            "Attuned Bell with Lyra",
                            "Attuned the Whispering Bell with Archivist Lyra; earned Chime of First Ash and weakened Malakor’s ward.",
                            None,
                            Some(120),
                            Some(1),
                            None,
                            Some("chime_first_ash"),
                        ),
                    ],
                    next_node_id: Some("root".to_string()),
                    ..ch(
                        "finish_bell_lyra",
                        "Accept Chime of First Ash (+1 Skill Point, +120 XP)",
                    )
                }],
            ),
            node(
                "malakor_cipher",
                "Lyra traces a sigil onto your palm. \"You grasp the third resonance! When you stand before Malakor on Floor 5, his ward-stone will falter at your approach.\"",
                vec![DialogueChoice {
                    action: Some(set_flag_bool("malakorWeakened", true)),
                    extra_actions: vec![
                        set_flag_bool("cipherInscribed", true),
                        DialogueAction::GrantBuff {
                            text: "Ward-Breaker Cipher inscribed (+8 Max HP).".to_string(),
                            hp_bonus: Some(8),
                        },
                    ],
                    next_node_id: Some("root".to_string()),
                    ..ch("apply_cipher", "Inscribe the Ward-Breaker Cipher")
                }],
            ),
            node(
                "cipher_fail",
                "\"The glyphs slip past untrained eyes. Raise your Arcana to 3 if you wish to pierce Malakor’s ward.\"",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("back", "Back")
                }],
            ),
        ]),
    )
}

fn corvus_def() -> NpcDef {
    npc(
        "corvus",
        "Magistrate Corvus",
        "Citadel Arbiter",
        "#F59E0B",
        "highspire",
        Some("portrait_corvus_1794537785514.jpg"),
        None,
        portrait(&[
            (r"    .▲▲▲.    ", "#FBBF24"),
            (r"   / === \   ", "#FDE68A"),
            (r"  | (•) (•) | ", "#E2E8F0"),
            (r"  |   ---   | ", "#CBD5E1"),
            (r"   \_______/  ", "#D97706"),
            (r"  {█=======█} ", "#92400E"),
        ]),
        node_map(vec![
            node(
                "root",
                "Magistrate Corvus adjusts his gold-trimmed mantle. \"Name? …I’m not writing that down.\" He writes something shorter. \"Highspire pays for results, not dusty superstitions. If you find the Whispering Bell on Floor 3, bring it straight to my vault — not Lyra’s lectern. There will be a form.\"",
                vec![
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            has_item_tag: Some("whispering_bell".to_string()),
                            ..quest_active("whispering_bell")
                        }),
                        next_node_id: Some("bell_corvus_complete".to_string()),
                        ..ch("sell_bell", "Turn over the Whispering Bell to Corvus.")
                    },
                    DialogueChoice {
                        condition: Some(quest_not_started("whispering_bell")),
                        next_node_id: Some("corvus_offer".to_string()),
                        ..ch(
                            "start_bell_corvus",
                            "Agree to recover the Whispering Bell for the Citadel.",
                        )
                    },
                    DialogueChoice {
                        action: Some(close_dialogue()),
                        ..ch("leave", "Take your leave.")
                    },
                ],
            ),
            node(
                "corvus_offer",
                "\"Sensible. Fetch the bell from Floor 3 and you’ll walk away with Citadel gold and my finest Greatspear.\"",
                vec![DialogueChoice {
                    action: Some(start_quest(
                        "whispering_bell",
                        "Magistrate Corvus offered top coin and steel for the Floor 3 Whispering Bell.",
                    )),
                    next_node_id: Some("root".to_string()),
                    ..ch("accept_c", "Understood.")
                }],
            ),
            node(
                "bell_corvus_complete",
                "Corvus locks the bell inside a lead-lined coffer. \"Pragmatism always wins out. Take your coin and the Magistrate Greatspear.\"",
                vec![DialogueChoice {
                    action: Some(consume("whispering_bell")),
                    extra_actions: vec![
                        set_flag_str("bellChoice", "sold_corvus"),
                        complete_quest(
                            "whispering_bell",
                            "Sold Bell to Corvus",
                            "Sold the Whispering Bell to Magistrate Corvus for heavy gold and the Magistrate Greatspear.",
                            Some(180),
                            Some(110),
                            None,
                            None,
                            Some("citadel_halberd"),
                        ),
                    ],
                    next_node_id: Some("root".to_string()),
                    ..ch(
                        "finish_bell_corvus",
                        "Accept Magistrate Greatspear & Bounty (+180g, +110 XP)",
                    )
                }],
            ),
        ]),
    )
}

fn kaelen_def() -> NpcDef {
    npc(
        "kaelen",
        "Guildmaster Kaelen",
        "Warden of the Descent",
        "#EF4444",
        "highspire",
        Some("portrait_kaelen_1794537786190.jpg"),
        None,
        portrait(&[
            (r"   /▲\_/▲\   ", "#EF4444"),
            (r"  |  ___  |  ", "#FCA5A5"),
            (r"  | (x) (•) | ", "#E2E8F0"),
            (r"  |  \_/  |  ", "#CBD5E1"),
            (r"   \_____/   ", "#991B1B"),
            (r"  [⚔=====⚔]  ", "#DC2626"),
        ]),
        node_map(vec![
            node(
                "root",
                "Guildmaster Kaelen cleans soot from a notched claymore. \"Below these stones lie the Ember Vaults, and beneath them, the Obsidian Sanctum. Only one name remains to settle: Malakor, Sovereign of Cinder.\" He pauses. \"Two names, if we count yours. The Guild has been arguing about yours for a week.\"",
                vec![
                    DialogueChoice {
                        condition: Some(quest_not_started("sovereign_cinder")),
                        next_node_id: Some("final_offer".to_string()),
                        ..ch(
                            "accept_final",
                            "Take the Guild’s commission to slay Malakor on Floor 5.",
                        )
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            min_gold: Some(40),
                            ..cond()
                        }),
                        action: Some(DialogueAction::RespecSkills { cost: 40 }),
                        next_node_id: Some("respec_done".to_string()),
                        ..ch(
                            "respec_ask",
                            "Request Guild Training Reset (Respec all Skills & Attributes · 40g)",
                        )
                    },
                    DialogueChoice {
                        action: Some(DialogueAction::SwitchCompanion),
                        next_node_id: Some("root".to_string()),
                        ..ch(
                            "kaelen_companion",
                            "Swap Spirit Companion (Silverwind Lynx <-> Starlight Owl)",
                        )
                    },
                    DialogueChoice {
                        action: Some(close_dialogue()),
                        ..ch("leave", "Head toward the Deep Stair.")
                    },
                ],
            ),
            node(
                "final_offer",
                "\"Descend through the Ember Vaults, break Inquisitor Serath on Floor 4, and strike down Malakor on Floor 5. Highspire remembers those who finish the climb.\"",
                vec![DialogueChoice {
                    action: Some(start_quest(
                        "sovereign_cinder",
                        "Guildmaster Kaelen commissioned you to defeat Malakor on Floor 5.",
                    )),
                    next_node_id: Some("root".to_string()),
                    ..ch("start_fin", "I will descend to the Obsidian Sanctum.")
                }],
            ),
            node(
                "respec_done",
                "Kaelen walks you through a brisk sparring drill, clearing old habits. All your Attribute and Skill Points have been refunded for a fresh build.",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("ok", "Ready to rebuild.")
                }],
            ),
        ]),
    )
}

fn orin_def() -> NpcDef {
    npc(
        "orin",
        "Relic Appraiser Orin",
        "Master Artificer",
        "#C084FC",
        "highspire",
        Some("portrait_orin_1794537785766.jpg"),
        Some("orin"),
        portrait(&[
            (r"    .⍟⍟⍟.    ", "#C084FC"),
            (r"   / o o \   ", "#F3E8FF"),
            (r"  | [◉|◉] |  ", "#E9D5FF"),
            (r"  |  \_/  |  ", "#D8B4FE"),
            (r"   \_____/   ", "#7E22CE"),
            (r"  {◊=====◊}  ", "#A855F7"),
        ]),
        node_map(vec![node(
            "root",
            "Orin peers through twin brass loupes. \"High-grade relics, enchanted rings, and masterwork forging—name what your build is missing.\"",
            vec![
                DialogueChoice {
                    action: Some(open_merchant("orin")),
                    ..ch("open_orin", "Open Relic Market & Forge")
                },
                DialogueChoice {
                    action: Some(close_dialogue()),
                    ..ch("leave", "Step away.")
                },
            ],
        )]),
    )
}

fn rowan_def() -> NpcDef {
    npc(
        "rowan",
        "Rowan the Wayfarer",
        "Outrider Scout",
        "#34D399",
        "overworld",
        Some("portrait_rowan_1794634591930.jpg"),
        None,
        portrait(&[
            (r"    /^^^^^\    ", "#34D399"),
            (r"   / o   o \   ", "#E2E8F0"),
            (r"  |   \_/   |  ", "#CBD5E1"),
            (r"   \_______/   ", "#166534"),
            (r"  /|__|*|__|\  ", "#D97706"),
            (r"   |  | |  |   ", "#475569"),
        ]),
        node_map(vec![
            node(
                "root",
                "Rowan banks the Silverbridge campfire and nods toward the eastern woods. \"Welcome to Silverwood Vale, outrider. I’ve heard about you.\" Pause. \"From you. In the distance. You were yelling it. North leads to Whispering Barrow and Cinderwatch Bastion; southeast lies Thornwick Fen.\"",
                vec![
                    DialogueChoice {
                        condition: Some(quest_not_started("silverwood_road")),
                        next_node_id: Some("brigand_intro".to_string()),
                        ..ch("ask_roads", "Tell me about the Silverbridge brigands.")
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            has_item_tag: Some("wayfarer_satchel".to_string()),
                            ..quest_active("silverwood_road")
                        }),
                        next_node_id: Some("brigand_complete".to_string()),
                        ..ch(
                            "turn_in_satchel",
                            "Return Rowan’s Stolen Satchel (+75g, +90 XP, Rare Band)",
                        )
                    },
                    DialogueChoice {
                        condition: Some(quest_not_started("deep_survey")),
                        next_node_id: Some("survey_intro".to_string()),
                        ..ch(
                            "ask_survey",
                            "Ask about the carved steles (‡) across the realm.",
                        )
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            quest_min_stage: Some(6),
                            ..quest_active("deep_survey")
                        }),
                        next_node_id: Some("survey_complete".to_string()),
                        ..ch(
                            "turn_in_survey",
                            "Report the completed Deep Roads survey (+60g, +120 XP, Star-Compass)",
                        )
                    },
                    DialogueChoice {
                        check: Some(check(AttributeKey::Agility, 3)),
                        next_node_id: Some("trail_secret".to_string()),
                        fail_node_id: Some("trail_fail".to_string()),
                        ..ch("scout_tip", "[Agility 3] Ask for covert overworld trails.")
                    },
                    DialogueChoice {
                        action: Some(close_dialogue()),
                        ..ch("leave", "Watch the road.")
                    },
                ],
            ),
            node(
                "brigand_intro",
                "\"Harlan and his cutthroats seized the ruined watchtower east across Silverbridge—and lifted my dispatch satchel while I slept. Clear out their tollhouse chest and bring my satchel back, and you can keep my Silverbridge Band.\"",
                vec![
                    DialogueChoice {
                        action: Some(start_quest(
                            "silverwood_road",
                            "Rowan asked you to cross Silverbridge and recover his Stolen Satchel from the eastern brigand watchtower.",
                        )),
                        next_node_id: Some("root".to_string()),
                        ..ch(
                            "accept_silverbridge",
                            "I will reclaim your satchel from the watchtower.",
                        )
                    },
                    DialogueChoice {
                        next_node_id: Some("root".to_string()),
                        ..ch("back", "Back")
                    },
                ],
            ),
            node(
                "brigand_complete",
                "\"My maps and cipher seals—intact!\" Rowan unclasps a silver compass ring from his cloak. \"Wear the Silverbridge Band well on the high roads.\"",
                vec![DialogueChoice {
                    action: Some(consume("wayfarer_satchel")),
                    extra_actions: vec![
                        set_flag_bool("silverbridgeCleared", true),
                        complete_quest(
                            "silverwood_road",
                            "Cleared Silverbridge Toll",
                            "Recovered Rowan’s Stolen Satchel from the eastern watchtower and earned the Wayfarer’s Silverbridge Band.",
                            Some(75),
                            Some(90),
                            None,
                            None,
                            Some("silverbridge_band"),
                        ),
                    ],
                    next_node_id: Some("root".to_string()),
                    ..ch(
                        "finish_silverbridge",
                        "Accept Wayfarer’s Silverbridge Band (+75g, +90 XP)",
                    )
                }],
            ),
            node(
                "survey_intro",
                "\"The old roads are written down, outrider — on carved steles (‡), one in every town and scattered through the vaults. Transcribe six of them for my survey and the deep roads will never lose you. No blood price. Just boot leather and literacy.\"",
                vec![
                    DialogueChoice {
                        action: Some(start_quest(
                            "deep_survey",
                            "Rowan commissioned a survey of the deep roads: transcribe 6 carved steles (‡) across the realm.",
                        )),
                        next_node_id: Some("root".to_string()),
                        ..ch(
                            "accept_survey",
                            "I will chart the deep roads (read 6 carved steles).",
                        )
                    },
                    DialogueChoice {
                        next_node_id: Some("root".to_string()),
                        ..ch("back", "Back")
                    },
                ],
            ),
            node(
                "survey_complete",
                "\"Six steles, faithfully copied!\" Rowan inks your transcriptions into a vellum road-chart, then unhooks the star-compass from his own cloak. \"Take it. Its needle points at the nearest unread stele — a surveyor’s sixth sense.\"",
                vec![DialogueChoice {
                    action: Some(complete_quest(
                        "deep_survey",
                        "Charted the Deep Roads",
                        "Transcribed 6 carved steles across the realm and completed Rowan’s survey of the deep roads.",
                        Some(60),
                        Some(120),
                        None,
                        None,
                        Some("surveyor_compass"),
                    )),
                    next_node_id: Some("root".to_string()),
                    ..ch(
                        "finish_survey",
                        "Accept the Surveyor’s Star-Compass (+60g, +120 XP)",
                    )
                }],
            ),
            node(
                "trail_secret",
                "\"Sharp eyes. North of this camp, beyond the whispering pines, rests an unattuned Elven Waystone—and Frostpeak Cavern in the far northeast holds meteoric ore hotter than Highspire steel.\"",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("back", "Good to know.")
                }],
            ),
            node(
                "trail_fail",
                "\"Stick to the cobbled causeways until your footing is surer. Blight weavers near the southeast fen hit hard if they flank you.\"",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("back", "Back")
                }],
            ),
        ]),
    )
}

fn sylva_def() -> NpcDef {
    npc(
        "sylva",
        "Elder Sylva",
        "Mire-Druid & Lorekeeper",
        "#34D399",
        "thornwick",
        Some("portrait_sylva_1794634618311.jpg"),
        None,
        portrait(&[
            (r"    \.|.|./    ", "#34D399"),
            (r"   /  ^ ^  \   ", "#ECFDF5"),
            (r"  |  (o o)  |  ", "#2DD4BF"),
            (r"   \  \_/  /   ", "#A7F3D0"),
            (r"  /[=======]\  ", "#115E59"),
        ]),
        node_map(vec![
            node(
                "root",
                "Bioluminescent spores drift from Elder Sylva’s antler crown. \"Thornwick Fen breathes uneasily, child of the Silver Bough. Beneath the eastern marsh, Spore-Matron Vexis swells the Fungal Hollow.\"",
                vec![
                    DialogueChoice {
                        condition: Some(quest_not_started("spores_of_bog")),
                        next_node_id: Some("bog_intro".to_string()),
                        ..ch("ask_bog", "How can I stem the Fungal Hollow blight?")
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            has_item_tag: Some("fungal_heart".to_string()),
                            ..quest_active("spores_of_bog")
                        }),
                        next_node_id: Some("bog_complete".to_string()),
                        ..ch(
                            "turn_in_heart",
                            "Deliver the Bioluminescent Fungal Heart (+120g, +150 XP, +1 Skill, Charm)",
                        )
                    },
                    DialogueChoice {
                        action: Some(DialogueAction::HealFull),
                        next_node_id: Some("healed".to_string()),
                        ..ch(
                            "sylva_mend",
                            "Ask Sylva to purge toxins & restore vigor (Free)",
                        )
                    },
                    DialogueChoice {
                        action: Some(close_dialogue()),
                        ..ch("leave", "Walk in peace.")
                    },
                ],
            ),
            node(
                "bog_intro",
                "\"Enter Fungal Hollow east of Thornwick Gate, descend to its Mycelial Heart on the second floor, and end Spore-Matron Vexis. Bring her Bioluminescent Heart so I may seal the roots.\"",
                vec![
                    DialogueChoice {
                        action: Some(start_quest(
                            "spores_of_bog",
                            "Elder Sylva of Thornwick Fen asked you to slay Spore-Matron Vexis inside Fungal Hollow (2 floors).",
                        )),
                        next_node_id: Some("root".to_string()),
                        ..ch("accept_bog", "I will cleanse the Fungal Hollow.")
                    },
                    DialogueChoice {
                        next_node_id: Some("root".to_string()),
                        ..ch("back", "Back")
                    },
                ],
            ),
            node(
                "bog_complete",
                "Sylva presses the glowing heart into moss-stone, and cool mist sweeps across Thornwick. \"The fen breathes freely once more. Take this charm and my blessing.\"",
                vec![DialogueChoice {
                    action: Some(consume("fungal_heart")),
                    extra_actions: vec![
                        DialogueAction::GrantBuff {
                            text: "Mire-Druid vitality blooms (+10 Max HP).".to_string(),
                            hp_bonus: Some(10),
                        },
                        complete_quest(
                            "spores_of_bog",
                            "Purged Fungal Hollow",
                            "Delivered the Bioluminescent Fungal Heart to Elder Sylva, earning the Sporeguard Charm and +1 Skill Point.",
                            Some(120),
                            Some(150),
                            Some(1),
                            None,
                            Some("sporeguard_amulet"),
                        ),
                    ],
                    next_node_id: Some("root".to_string()),
                    ..ch(
                        "finish_bog",
                        "Claim Sporeguard Charm (+120g, +150 XP, +1 Skill Point, +10 Max HP)",
                    )
                }],
            ),
            node(
                "healed",
                "Warm willow-sap salve seals every wound and clears your lungs. Your HP and Mana are fully restored.",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("back", "Thank you, Elder.")
                }],
            ),
        ]),
    )
}

fn bram_def() -> NpcDef {
    npc(
        "bram",
        "Tinker Bram",
        "Relic Scavenger",
        "#FBBF24",
        "thornwick",
        Some("portrait_bram_1794634617902.jpg"),
        Some("bram"),
        portrait(&[
            (r"    .-[o=o]-.  ", "#FBBF24"),
            (r"   /  _   _  \ ", "#E2E8F0"),
            (r"  |  (◉) (◉) | ", "#FDE68A"),
            (r"   \  \___/  / ", "#CBD5E1"),
            (r"   /[|===|]\   ", "#92400E"),
        ]),
        node_map(vec![node(
            "root",
            "Tinker Bram adjusts his brass loupes over a crate of salvaged runes. \"Runes, blink scrolls, throwing knives, bog-recovered trinkets—all priced for real delvers!\"",
            vec![
                DialogueChoice {
                    action: Some(open_merchant("bram")),
                    ..ch("open_bram", "Browse Bram’s Salvage")
                },
                DialogueChoice {
                    action: Some(close_dialogue()),
                    ..ch("leave", "Maybe later.")
                },
            ],
        )]),
    )
}

fn halda_def() -> NpcDef {
    npc(
        "halda",
        "Forgemistress Halda",
        "Master Runesmith",
        "#FB923C",
        "cinderwatch",
        Some("portrait_halda_1794634618989.jpg"),
        Some("halda"),
        portrait(&[
            (r"   .=======.   ", "#FB923C"),
            (r"  /  \___/  \  ", "#FDBA74"),
            (r" |  (o) (o)  | ", "#F59E0B"),
            (r" |  /=====\  | ", "#9A3412"),
            (r"  \_[=====]_/  ", "#78350F"),
        ]),
        node_map(vec![
            node(
                "root",
                "Sparks cascade off Halda’s runic anvil. \"Highspire polishes parade armor; Cinderwatch forges gear that survives Ashen Caldera. Need volcanic steel, or looking for heavy smithing work?\"",
                vec![
                    DialogueChoice {
                        action: Some(open_merchant("halda")),
                        ..ch("open_halda", "Browse Bastion Armory")
                    },
                    DialogueChoice {
                        condition: Some(quest_not_started("cinder_and_frost")),
                        next_node_id: Some("caldera_intro".to_string()),
                        ..ch("ask_caldera", "Ask about the Ashen Caldera quest.")
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            has_item_tag: Some("ashen_ore_heart".to_string()),
                            ..quest_active("cinder_and_frost")
                        }),
                        next_node_id: Some("caldera_complete".to_string()),
                        ..ch(
                            "turn_in_ore",
                            "Deliver the Ashen Ore Heart (+180g, +200 XP, Cinder Crown Relic)",
                        )
                    },
                    DialogueChoice {
                        action: Some(close_dialogue()),
                        ..ch("leave", "Step away from the forge.")
                    },
                ],
            ),
            node(
                "caldera_intro",
                "\"East of Cinderwatch, Frostpeak Cavern bores three tiers straight down into magma. Slay Ignis-Warden Kaelthar on Floor 3 and haul back the Ashen Ore Heart. I’ll forge you the Cinder Crown Relic.\"",
                vec![
                    DialogueChoice {
                        action: Some(start_quest(
                            "cinder_and_frost",
                            "Forgemistress Halda asked you to descend Frostpeak Cavern (3 floors) and slay Ignis-Warden Kaelthar for the Ashen Ore Heart.",
                        )),
                        next_node_id: Some("root".to_string()),
                        ..ch("accept_caldera", "I will bring back the Ashen Ore Heart.")
                    },
                    DialogueChoice {
                        next_node_id: Some("root".to_string()),
                        ..ch("back", "Back")
                    },
                ],
            ),
            node(
                "caldera_complete",
                "Halda quenches the white-hot core in oil and beats out a blazing circlet in minutes. \"True Caldera work! Wear the Cinder Crown into Malakor’s own Sanctum.\"",
                vec![DialogueChoice {
                    action: Some(consume("ashen_ore_heart")),
                    extra_actions: vec![complete_quest(
                        "cinder_and_frost",
                        "Forged Caldera Relic",
                        "Delivered the Ashen Ore Heart to Forgemistress Halda and claimed the Cinder Crown Relic.",
                        Some(180),
                        Some(200),
                        None,
                        Some(1),
                        Some("cinder_crown"),
                    )],
                    next_node_id: Some("root".to_string()),
                    ..ch(
                        "finish_caldera",
                        "Claim the Cinder Crown Relic (+180g, +200 XP, +1 Attribute Point)",
                    )
                }],
            ),
        ]),
    )
}

fn darek_def() -> NpcDef {
    npc(
        "darek",
        "Captain Darek",
        "Rift Sentinel",
        "#EF4444",
        "cinderwatch",
        Some("portrait_darek_1794634620253.jpg"),
        None,
        portrait(&[
            (r"    /[===]\    ", "#94A3B8"),
            (r"   | *   * |   ", "#F87171"),
            (r"   |  ---  |   ", "#E2E8F0"),
            (r"  /|_______|\  ", "#7F1D1D"),
            (r"   |  | |  |   ", "#475569"),
        ]),
        node_map(vec![
            node(
                "root",
                "Captain Darek scans the ridge parapets. \"Between Whispering Barrow to the west and Frostpeak east, Cinderwatch holds the whole northern pass. Looking to earn garrison bounty?\"",
                vec![
                    DialogueChoice {
                        condition: Some(quest_not_started("barrow_purge")),
                        next_node_id: Some("barrow_intro".to_string()),
                        ..ch("ask_barrow", "Tell me about Whispering Barrow.")
                    },
                    DialogueChoice {
                        condition: Some(DialogueCondition {
                            has_item_tag: Some("morvath_seal".to_string()),
                            ..quest_active("barrow_purge")
                        }),
                        next_node_id: Some("barrow_complete".to_string()),
                        ..ch(
                            "turn_in_barrow",
                            "Present Morvath’s Barrow Seal (+140g, +150 XP, +1 Attribute, Shroud)",
                        )
                    },
                    DialogueChoice {
                        action: Some(DialogueAction::RespecSkills { cost: 0 }),
                        next_node_id: Some("respec_done".to_string()),
                        ..ch(
                            "darek_respec",
                            "Ask Darek for tactical drills (Respec Attributes & Skills — Free)",
                        )
                    },
                    DialogueChoice {
                        action: Some(close_dialogue()),
                        ..ch("leave", "Carry on, Captain.")
                    },
                ],
            ),
            node(
                "barrow_intro",
                "\"West of here, Whispering Barrow has been spilling wights onto the north trail. Put down Arch-Wight Morvath on Barrow Floor 2 and bring me his iron Barrow Seal.\"",
                vec![
                    DialogueChoice {
                        action: Some(start_quest(
                            "barrow_purge",
                            "Captain Darek placed a garrison bounty on Arch-Wight Morvath on Floor 2 of Whispering Barrow.",
                        )),
                        next_node_id: Some("root".to_string()),
                        ..ch("accept_barrow", "Consider Arch-Wight Morvath dealt with.")
                    },
                    DialogueChoice {
                        next_node_id: Some("root".to_string()),
                        ..ch("back", "Back")
                    },
                ],
            ),
            node(
                "barrow_complete",
                "Darek weighs the cracked crown-seal and salutes sharply. \"Clean work, outrider. Claim your garrison gold, our spare Barrow-Wight Shroud, and veteran conditioning.\"",
                vec![DialogueChoice {
                    action: Some(consume("morvath_seal")),
                    extra_actions: vec![complete_quest(
                        "barrow_purge",
                        "Purged Whispering Barrow",
                        "Slayed Arch-Wight Morvath and turned in the Barrow Seal to Captain Darek.",
                        Some(140),
                        Some(150),
                        None,
                        Some(1),
                        Some("morvath_shroud"),
                    )],
                    next_node_id: Some("root".to_string()),
                    ..ch(
                        "finish_barrow",
                        "Claim Barrow Bounty (+140g, +150 XP, +1 Attribute Point, Shroud)",
                    )
                }],
            ),
            node(
                "respec_done",
                "Darek drills your stance on the basalt ramparts. All Attribute and Skill Points have been refunded for reallocation.",
                vec![DialogueChoice {
                    next_node_id: Some("root".to_string()),
                    ..ch("back", "Ready.")
                }],
            ),
        ]),
    )
}

// Phase 3 ambient townsfolk — no quests, just local colour (TS comment).

fn pell_def() -> NpcDef {
    npc(
        "pell",
        "Pell the Carter",
        "Outpost Carter",
        "#A8A29E",
        "oakhaven",
        None,
        None,
        portrait(&[
            (r"    _______    ", "#A8A29E"),
            (r"   / o   o \   ", "#E2E8F0"),
            (r"  |   ___   |  ", "#CBD5E1"),
            (r"   \_______/   ", "#78716C"),
            (r"  /|==| |==|\  ", "#92400E"),
            (r"   |  | |  |   ", "#475569"),
        ]),
        node_map(vec![node(
            "root",
            "Pell is re-shoeing his mule and cursing the cart axle in three languages. \"Hauls ore up from the gate, stories down from the Vale. Stories pay worse, weigh less. Mind the mule — she bites critics.\"",
            vec![DialogueChoice {
                action: Some(close_dialogue()),
                ..ch("leave", "Leave him to the axle.")
            }],
        )]),
    )
}

fn wren_def() -> NpcDef {
    npc(
        "wren",
        "Sister Wren",
        "Shrine Keeper",
        "#FDE68A",
        "oakhaven",
        None,
        None,
        portrait(&[
            (r"    .-----.    ", "#FDE68A"),
            (r"   / o   o \   ", "#FEF3C7"),
            (r"  |   \_/   |  ", "#FDE68A"),
            (r"   \_______/   ", "#B45309"),
            (r"  /|  |*|  |\  ", "#F59E0B"),
            (r"   |  | |  |   ", "#475569"),
        ]),
        node_map(vec![node(
            "root",
            "Sister Wren tends a row of shrine candles that never quite go out. \"Blessings are free, delver. Candles cost a copper. Ego is extra.\" She eyes you. \"I’ll light one for you anyway. A big one. I hear that’s how you like things.\" She glances at the Old Oak. \"It was old when the Outpost was young. Leave it a kind word when you pass.\"",
            vec![DialogueChoice {
                action: Some(close_dialogue()),
                ..ch("leave", "Bow and go.")
            }],
        )]),
    )
}

fn tomas_def() -> NpcDef {
    npc(
        "tomas",
        "Page Tomas",
        "Citadel Page",
        "#93C5FD",
        "highspire",
        None,
        None,
        portrait(&[
            (r"    /^^^^^\    ", "#93C5FD"),
            (r"   / o   o \   ", "#E2E8F0"),
            (r"  |   \o/   |  ", "#BFDBFE"),
            (r"   \_______/   ", "#2563EB"),
            (r"  /|__|▤|__|\  ", "#D97706"),
            (r"   |  | |  |   ", "#475569"),
        ]),
        node_map(vec![node(
            "root",
            "Page Tomas skids to a halt, scrolls bundled under both arms. \"Messages, maps, and misplaced magistrates — that’s my whole job! I know every stair in Highspire. Fallen down most of them, too. Archivist Lyra’s archive is north, if you’re hunting reading.\"",
            vec![DialogueChoice {
                action: Some(close_dialogue()),
                ..ch("leave", "Let him run.")
            }],
        )]),
    )
}

fn fenwick_def() -> NpcDef {
    npc(
        "fenwick",
        "Fenwick the Fisher",
        "Fen Fisher",
        "#4ADE80",
        "thornwick",
        None,
        None,
        portrait(&[
            (r"   _________   ", "#4ADE80"),
            (r"  /  o   o  \  ", "#DCFCE7"),
            (r"  |   \_/   |  ", "#86EFAC"),
            (r"   \_______/   ", "#166534"),
            (r"  /|  |~|  |\  ", "#0EA5E9"),
            (r"   |  | |  |   ", "#475569"),
        ]),
        node_map(vec![node(
            "root",
            "Fenwick holds up today’s catch: a boot. \"Yesterday: a boot. The fen gives what the fen gives.\" He nods toward the frogs. \"When they sing, the Hollow’s calm. When they stop — that’s when you wade careful.\"",
            vec![DialogueChoice {
                action: Some(close_dialogue()),
                ..ch("leave", "Tight lines.")
            }],
        )]),
    )
}

fn sella_def() -> NpcDef {
    npc(
        "sella",
        "Sella the Quartermaster",
        "Bastion Quartermaster",
        "#FDE68A",
        "cinderwatch",
        None,
        None,
        portrait(&[
            (r"    /=====\    ", "#FDE68A"),
            (r"   / o   o \   ", "#E2E8F0"),
            (r"  |   \_/   |  ", "#FDE68A"),
            (r"   \_______/   ", "#B45309"),
            (r"  /|==|▤|==|\  ", "#92400E"),
            (r"   |  | |  |   ", "#475569"),
        ]),
        node_map(vec![node(
            "root",
            "Sella is chalking inventory onto a basalt slab. \"Rations, rope, rime-wool. If it isn’t on my list, it isn’t in the Bastion. The mountain provides the rest — rudely, and usually on fire.\"",
            vec![DialogueChoice {
                action: Some(close_dialogue()),
                ..ch("leave", "Salute the list.")
            }],
        )]),
    )
}

/// All NPC defs, in TS `NPCS` source order.
pub fn npc_defs() -> Vec<NpcDef> {
    vec![
        torin_def(),
        mira_def(),
        vance_def(),
        vex_def(),
        lyra_def(),
        corvus_def(),
        kaelen_def(),
        orin_def(),
        rowan_def(),
        sylva_def(),
        bram_def(),
        halda_def(),
        darek_def(),
        pell_def(),
        wren_def(),
        tomas_def(),
        fenwick_def(),
        sella_def(),
    ]
}

pub fn npc_def(id: &str) -> Option<NpcDef> {
    npc_defs().into_iter().find(|n| n.id == id)
}

// ---------------------------------------------------------------------------
// Merchant stock (TS createMerchantStock)
// ---------------------------------------------------------------------------

fn proc(
    gen: &mut ItemIdGen,
    rng: &mut Rng,
    depth: i32,
    slot: ItemSlot,
    min_rarity: ItemRarity,
) -> Item {
    generate_procedural_item(
        gen,
        rng,
        depth,
        ProceduralOptions {
            slot: Some(slot),
            min_rarity: Some(min_rarity),
        },
    )
}

pub fn create_merchant_stock(
    gen: &mut ItemIdGen,
    rng: &mut Rng,
    merchant_id: &str,
    town: Option<&TownState>,
) -> Vec<Item> {
    match merchant_id {
        "mira" => vec![
            create_consumable(gen, ConsumableKind::HealPotion, 3),
            create_consumable(gen, ConsumableKind::ManaPotion, 2),
            create_consumable(gen, ConsumableKind::WardTalisman, 2),
            create_consumable(gen, ConsumableKind::ScrollBlink, 1),
            create_consumable(gen, ConsumableKind::HealPotion, 2),
            create_consumable(gen, ConsumableKind::WardTalisman, 1),
        ],
        "vex" => vec![
            proc(gen, rng, 2, ItemSlot::Weapon, ItemRarity::Rare),
            proc(gen, rng, 2, ItemSlot::Ring, ItemRarity::Uncommon),
            create_consumable(gen, ConsumableKind::ScrollBlink, 1),
            create_consumable(gen, ConsumableKind::ThrowingKnife, 3),
            create_consumable(gen, ConsumableKind::ScrollBlink, 2),
        ],
        "bram" => vec![
            proc(gen, rng, 2, ItemSlot::Amulet, ItemRarity::Rare),
            proc(gen, rng, 2, ItemSlot::Ring, ItemRarity::Uncommon),
            create_consumable(gen, ConsumableKind::ScrollBlink, 1),
            create_consumable(gen, ConsumableKind::ThrowingKnife, 2),
            create_consumable(gen, ConsumableKind::ScrollBlink, 2),
            create_consumable(gen, ConsumableKind::ThrowingKnife, 3),
        ],
        "halda" => vec![
            proc(gen, rng, 5, ItemSlot::Weapon, ItemRarity::Epic),
            proc(gen, rng, 5, ItemSlot::Armor, ItemRarity::Rare),
            proc(gen, rng, 5, ItemSlot::Helm, ItemRarity::Rare),
            create_consumable(gen, ConsumableKind::ThrowingKnife, 3),
            create_consumable(gen, ConsumableKind::ManaPotion, 2),
            create_consumable(gen, ConsumableKind::HealPotion, 3),
            create_consumable(gen, ConsumableKind::WardTalisman, 2),
        ],
        "orin" => vec![
            proc(gen, rng, 4, ItemSlot::Weapon, ItemRarity::Rare),
            proc(gen, rng, 4, ItemSlot::Armor, ItemRarity::Rare),
            proc(gen, rng, 4, ItemSlot::Amulet, ItemRarity::Epic),
            create_consumable(gen, ConsumableKind::WardTalisman, 1),
            create_consumable(gen, ConsumableKind::HealPotion, 2),
            create_consumable(gen, ConsumableKind::HealPotion, 2),
            create_consumable(gen, ConsumableKind::ManaPotion, 2),
        ],
        "blacksmith" => {
            // ElfStory Reborn Phase 0/1: the REBUILT Blacksmith's own stock.
            // This merchant only exists once the town project completes
            // (the session gates it on town state). Headline item is
            // town-gated: the Oakhaven-Forged Blade cannot be bought
            // anywhere else. Side-dungeon town supplies unlock signature
            // gear here as well.
            let blade = proc(gen, rng, 3, ItemSlot::Weapon, ItemRarity::Rare);
            let mut stock = vec![
                Item {
                    name: "Oakhaven-Forged Blade".to_string(),
                    description: "Forged in Torin’s rebuilt smithy from Reliquary steel. Sold only by the rebuilt Blacksmith — Oakhaven made this, and Oakhaven keeps the good stuff.".to_string(),
                    ..blade
                },
                proc(gen, rng, 2, ItemSlot::Armor, ItemRarity::Uncommon),
                create_consumable(gen, ConsumableKind::HealPotion, 3),
                create_consumable(gen, ConsumableKind::ThrowingKnife, 2),
            ];
            let supplies: &[TownSupplyTag] =
                town.map(|t| t.supplies_found.as_slice()).unwrap_or(&[]);
            if supplies.contains(&TownSupplyTag::BarrowIron) {
                let hauberk = proc(gen, rng, 3, ItemSlot::Armor, ItemRarity::Rare);
                stock.push(Item {
                    name: "Barrow-Iron Hauberk".to_string(),
                    description: "Forged from Whispering Barrow Iron brought back to Oakhaven. Heavy grave-tempered rings shrug off crypt steel.".to_string(),
                    ..hauberk
                });
            }
            if supplies.contains(&TownSupplyTag::MireTimber) {
                let bow = proc(gen, rng, 3, ItemSlot::Weapon, ItemRarity::Rare);
                stock.push(Item {
                    name: "Mire-Timber Greatbow".to_string(),
                    description: "Bent from Thornwick Ironwood Timber banked in Oakhaven. Rot-proof heartwood hums with bog tension.".to_string(),
                    ..bow
                });
            }
            if supplies.contains(&TownSupplyTag::FrostCoal) {
                let helm = proc(gen, rng, 4, ItemSlot::Helm, ItemRarity::Epic);
                stock.push(Item {
                    name: "Frost-Quenched Greathelm".to_string(),
                    description: "Smelted over Frostpeak White-Coal in Torin’s rebuilt hearth. Cold enough to turn cinder sparks aside.".to_string(),
                    ..helm
                });
            }
            stock
        }
        // Torin default — also the TS fall-through for any unknown id.
        _ => vec![
            proc(gen, rng, 1, ItemSlot::Weapon, ItemRarity::Uncommon),
            proc(gen, rng, 1, ItemSlot::Armor, ItemRarity::Uncommon),
            proc(gen, rng, 2, ItemSlot::Helm, ItemRarity::Uncommon),
            create_consumable(gen, ConsumableKind::ThrowingKnife, 2),
        ],
    }
}

// ---------------------------------------------------------------------------
// Quest helpers (TS isQuestActive)
// ---------------------------------------------------------------------------

pub fn is_quest_active(quests: &HashMap<String, QuestProgress>, quest_id: &str) -> bool {
    quests
        .get(quest_id)
        .map(|q| q.status == QuestStatus::Active)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::town::create_town_state;

    #[test]
    fn quest_defs_count_and_lookup() {
        let defs = quest_defs();
        assert_eq!(defs.len(), 10);
        let shadows = quest_def("shadows_beneath").expect("shadows_beneath exists");
        assert_eq!(shadows.title, "Shadows Beneath Oakhaven");
        assert_eq!(shadows.giver, "Warden Vance / Vex");
        assert!(quest_def("nope").is_none());
    }

    #[test]
    fn lore_count() {
        assert_eq!(lore_entries().len(), 17);
        assert_eq!(
            lore_entry("lore_floor_1").unwrap().title,
            "Etched Wall Warning"
        );
    }

    #[test]
    fn torin_root_trade_and_craft_check() {
        let torin = npc_def("torin").unwrap();
        let root = &torin.nodes["root"];
        let trade = root.choices.iter().find(|c| c.id == "trade").unwrap();
        match &trade.action {
            Some(DialogueAction::OpenMerchant { merchant_id }) => {
                assert_eq!(merchant_id, "torin")
            }
            other => panic!("trade action should open merchant torin, got {other:?}"),
        }
        let craft = root.choices.iter().find(|c| c.id == "ask_craft").unwrap();
        let check = craft.check.as_ref().expect("ask_craft has a skill check");
        assert_eq!(check.attribute, AttributeKey::Might);
        assert_eq!(check.threshold, 3);
        assert_eq!(craft.next_node_id.as_deref(), Some("craft_pass"));
        assert_eq!(craft.fail_node_id.as_deref(), Some("craft_fail"));
    }

    #[test]
    fn mira_root_heal_and_blight_offer() {
        let mira = npc_def("mira").unwrap();
        let root = &mira.nodes["root"];
        let heal = root.choices.iter().find(|c| c.id == "heal_free").unwrap();
        assert!(matches!(heal.action, Some(DialogueAction::HealFull)));
        assert_eq!(heal.next_node_id.as_deref(), Some("healed"));
        let offer = root.choices.iter().find(|c| c.id == "quest_offer").unwrap();
        let condition = offer.condition.as_ref().expect("quest_offer is conditioned");
        assert_eq!(condition.quest_id.as_deref(), Some("herbalist_blight"));
        // TS spells this `questStatus: undefined` — quest not started yet.
        assert_eq!(condition.quest_status, None);
        assert_eq!(offer.next_node_id.as_deref(), Some("blight_intro"));
    }

    #[test]
    fn all_dialogue_graphs_are_internally_consistent() {
        let defs = npc_defs();
        assert_eq!(defs.len(), 18);
        let mut total_nodes = 0;
        let mut total_choices = 0;
        let mut total_portrait_lines = 0;
        for def in &defs {
            assert!(
                def.nodes.contains_key(&def.root_node_id),
                "{}: root node {} missing",
                def.id,
                def.root_node_id
            );
            for (node_id, node) in &def.nodes {
                assert_eq!(&node.id, node_id, "{}: node key/id mismatch", def.id);
                total_nodes += 1;
                for choice in &node.choices {
                    total_choices += 1;
                    if let Some(next) = &choice.next_node_id {
                        assert!(
                            def.nodes.contains_key(next),
                            "{}:{} choice {} -> missing node {}",
                            def.id,
                            node_id,
                            choice.id,
                            next
                        );
                    }
                    if let Some(fail) = &choice.fail_node_id {
                        assert!(
                            def.nodes.contains_key(fail),
                            "{}:{} choice {} fail -> missing node {}",
                            def.id,
                            node_id,
                            choice.id,
                            fail
                        );
                    }
                }
            }
            total_portrait_lines += def.portrait.len();
        }
        // Exhaustiveness pins against the TS source (counted from npcs.ts).
        assert_eq!(total_nodes, 60);
        assert_eq!(total_choices, 111);
        assert_eq!(total_portrait_lines, 104);
    }

    #[test]
    fn mira_stock_is_six_consumables() {
        let mut gen = ItemIdGen::new();
        let mut rng = Rng::new(42);
        let stock = create_merchant_stock(&mut gen, &mut rng, "mira", None);
        assert_eq!(stock.len(), 6);
        assert!(
            stock.iter().all(|i| i.category == ItemCategory::Consumable),
            "mira stock is all consumables"
        );
    }

    #[test]
    fn blacksmith_stock_blade_and_supply_growth() {
        let mut gen = ItemIdGen::new();
        let mut rng = Rng::new(7);
        let base = create_merchant_stock(&mut gen, &mut rng, "blacksmith", None);
        assert_eq!(base.len(), 4);
        assert_eq!(base[0].name, "Oakhaven-Forged Blade");

        let mut town = create_town_state();
        town.supplies_found.push(TownSupplyTag::BarrowIron);
        let mut gen = ItemIdGen::new();
        let mut rng = Rng::new(7);
        let with_iron = create_merchant_stock(&mut gen, &mut rng, "blacksmith", Some(&town));
        assert_eq!(with_iron.len(), 5);
        assert!(with_iron.iter().any(|i| i.name == "Barrow-Iron Hauberk"));

        town.supplies_found.push(TownSupplyTag::MireTimber);
        let mut gen = ItemIdGen::new();
        let mut rng = Rng::new(7);
        let with_timber = create_merchant_stock(&mut gen, &mut rng, "blacksmith", Some(&town));
        assert_eq!(with_timber.len(), 6);
        assert!(with_timber.iter().any(|i| i.name == "Mire-Timber Greatbow"));

        town.supplies_found.push(TownSupplyTag::FrostCoal);
        let mut gen = ItemIdGen::new();
        let mut rng = Rng::new(7);
        let with_coal = create_merchant_stock(&mut gen, &mut rng, "blacksmith", Some(&town));
        assert_eq!(with_coal.len(), 7);
        assert!(with_coal.iter().any(|i| i.name == "Frost-Quenched Greathelm"));
        assert_eq!(with_coal[0].name, "Oakhaven-Forged Blade");
    }

    #[test]
    fn unknown_merchant_gets_torin_default() {
        let mut gen = ItemIdGen::new();
        let mut rng = Rng::new(3);
        let stock = create_merchant_stock(&mut gen, &mut rng, "no_such_merchant", None);
        assert_eq!(stock.len(), 4);
        // Same shape as the explicit 'torin' id (TS fall-through).
        let mut gen = ItemIdGen::new();
        let mut rng = Rng::new(3);
        let torin = create_merchant_stock(&mut gen, &mut rng, "torin", None);
        assert_eq!(stock, torin);
    }

    #[test]
    fn is_quest_active_checks_status() {
        let mut quests: HashMap<String, QuestProgress> = HashMap::new();
        assert!(!is_quest_active(&quests, "shadows_beneath"));
        quests.insert(
            "shadows_beneath".to_string(),
            QuestProgress {
                id: "shadows_beneath".to_string(),
                status: QuestStatus::Active,
                stage: 0,
                outcome_label: None,
                history: Vec::new(),
            },
        );
        assert!(is_quest_active(&quests, "shadows_beneath"));
        quests.get_mut("shadows_beneath").unwrap().status = QuestStatus::Completed;
        assert!(!is_quest_active(&quests, "shadows_beneath"));
    }
}
