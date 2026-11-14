//! Settings files of Universal Pokémon Randomizer ZX (`.rnqs`).
//!
//! A file is the randomizer's version (`u32`, big-endian), the length of the
//! settings string and the string itself: Base64 of 51 bytes of options, the
//! ROM name, a CRC-32 of both and a checksum of the custom names file. The
//! randomizer refuses a string whose CRC-32 does not match, so changing an
//! option means writing it anew.
//!
//! SoulSync changes only the options it offers and keeps everything else as
//! the player set it in the randomizer. The byte layout follows
//! `Settings.toString` of the randomizer (GPL-3.0, as SoulSync).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Option bytes before the ROM name.
const OPTION_BYTES: usize = 51;
/// TM moves and TM/HM compatibility.
const TMS: usize = 18;
/// Full HM compatibility, TMs and tutors follow evolutions.
const TMS2: usize = 19;
/// Move tutor moves and compatibility.
const TUTORS: usize = 21;

/// Why a settings file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetError {
    /// Not a settings file of the randomizer.
    Malformed,
    /// Its checksum does not match: the randomizer would refuse it too.
    Checksum,
}

impl fmt::Display for PresetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PresetError::Malformed => write!(f, "not a randomizer settings file (.rnqs)"),
            PresetError::Checksum => write!(f, "the settings file is damaged"),
        }
    }
}

impl std::error::Error for PresetError {}

/// Which Pokémon can learn which TM, or which move tutor's move.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Compat {
    /// As in the original game.
    #[default]
    Unchanged,
    /// At random, more likely for moves of the Pokémon's own type.
    RandomPreferType,
    /// At random.
    Random,
    /// Every Pokémon learns every one.
    Full,
}

/// The value of one option: a switch, or the name of one of its choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Flag(bool),
    Choice(String),
}

/// The options changed in SoulSync before randomizing, by the `id` of
/// [`OPTIONS`]. One that is missing or `null` keeps what the settings file
/// says, and so does one the table does not know.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Overrides(pub BTreeMap<String, Option<Value>>);

impl Overrides {
    /// Whether nothing is changed.
    pub fn is_empty(&self) -> bool {
        self.0.values().all(Option::is_none)
    }

    pub fn set(&mut self, id: &str, value: Value) {
        self.0.insert(id.to_owned(), Some(value));
    }
}

/// Where an option lives in the option bytes.
#[derive(Debug, Clone, Copy)]
pub enum Kind {
    /// One bit.
    Flag(usize, u8),
    /// Exactly one bit of several is set: the choice's name, byte and bit.
    Choice(&'static [(&'static str, usize, u8)]),
}

/// An option of the randomizer SoulSync can show and change.
#[derive(Debug, Clone, Copy)]
pub struct Opt {
    pub id: &'static str,
    pub kind: Kind,
}

const fn flag(id: &'static str, byte: usize, bit: u8) -> Opt {
    Opt {
        id,
        kind: Kind::Flag(byte, bit),
    }
}

const fn choice(id: &'static str, of: &'static [(&'static str, usize, u8)]) -> Opt {
    Opt {
        id,
        kind: Kind::Choice(of),
    }
}

const COMPAT_TM: &[(&str, usize, u8)] = &[
    ("unchanged", TMS, 2),
    ("random_prefer_type", TMS, 1),
    ("random", TMS, 0),
    ("full", TMS, 7),
];
const COMPAT_TUTOR: &[(&str, usize, u8)] = &[
    ("unchanged", TUTORS, 2),
    ("random_prefer_type", TUTORS, 1),
    ("random", TUTORS, 0),
    ("full", TUTORS, 7),
];

/// The options SoulSync knows, as `Settings.toString` lays them out. Those
/// of other generations (Mega Evolutions, Totem Pokémon) and the ones that
/// take a number or a list are left to the settings file.
pub const OPTIONS: &[Opt] = &[
    // Pokémon traits
    choice(
        "base_stats",
        &[("unchanged", 1, 3), ("shuffle", 1, 2), ("random", 1, 1)],
    ),
    flag("base_stats_follow_evolutions", 1, 0),
    flag("update_base_stats", 1, 5),
    choice(
        "types",
        &[
            ("unchanged", 2, 2),
            ("random_follow_evolutions", 2, 0),
            ("random", 2, 1),
        ],
    ),
    choice("abilities", &[("unchanged", 3, 0), ("random", 3, 1)]),
    flag("abilities_follow_evolutions", 3, 3),
    flag("allow_wonder_guard", 3, 2),
    choice(
        "evolutions",
        &[
            ("unchanged", 26, 0),
            ("random", 26, 1),
            ("random_every_level", 26, 7),
        ],
    ),
    flag("change_impossible_evolutions", 0, 0),
    flag("easier_evolutions", 0, 5),
    flag("no_time_evolutions", 0, 6),
    // Starters, static Pokémon and trades
    choice(
        "starters",
        &[
            ("unchanged", 4, 2),
            ("random", 4, 1),
            ("random_two_evolutions", 4, 3),
        ],
    ),
    choice(
        "statics",
        &[
            ("unchanged", 17, 0),
            ("swap", 17, 1),
            ("random", 17, 2),
            ("similar_strength", 17, 3),
        ],
    ),
    choice(
        "trades",
        &[("unchanged", 23, 6), ("given", 23, 1), ("both", 23, 0)],
    ),
    // Moves
    flag("update_moves", 0, 1),
    flag("random_move_power", 25, 0),
    flag("random_move_accuracy", 25, 1),
    flag("random_move_pp", 25, 2),
    flag("random_move_types", 25, 3),
    choice(
        "movesets",
        &[
            ("unchanged", 11, 2),
            ("random_prefer_type", 11, 1),
            ("random", 11, 0),
            ("metronome", 11, 3),
        ],
    ),
    // Trainers
    choice(
        "trainers",
        &[
            ("unchanged", 13, 0),
            ("random", 13, 1),
            ("distributed", 13, 2),
            ("distributed_main", 13, 3),
            ("type_themed", 13, 4),
            ("type_themed_gyms", 13, 5),
        ],
    ),
    flag("rival_keeps_starter", 27, 1),
    flag("trainers_similar_strength", 27, 0),
    flag("trainers_no_legendaries", 27, 3),
    flag("trainers_fully_evolved", 14, 7),
    flag("random_trainer_names", 0, 3),
    flag("random_trainer_classes", 0, 4),
    // Wild Pokémon
    choice(
        "wild",
        &[
            ("unchanged", 15, 6),
            ("random", 15, 5),
            ("area_mapping", 15, 1),
            ("global_mapping", 15, 4),
        ],
    ),
    choice(
        "wild_rule",
        &[
            ("none", 15, 2),
            ("similar_strength", 16, 2),
            ("catch_em_all", 15, 0),
            ("type_themed", 15, 3),
        ],
    ),
    flag("wild_no_legendaries", 16, 1),
    flag("wild_held_items", 16, 3),
    // TMs, HMs and move tutors
    choice("tm_moves", &[("unchanged", TMS, 4), ("random", TMS, 3)]),
    choice("tm", COMPAT_TM),
    flag("full_hm", TMS2, 0),
    choice(
        "tutor_moves",
        &[("unchanged", TUTORS, 4), ("random", TUTORS, 3)],
    ),
    choice("tutor", COMPAT_TUTOR),
    // Items
    choice(
        "field_items",
        &[
            ("unchanged", 24, 2),
            ("shuffle", 24, 1),
            ("random", 24, 0),
            ("random_even", 24, 4),
        ],
    ),
    choice(
        "shops",
        &[("unchanged", 37, 2), ("shuffle", 37, 1), ("random", 37, 0)],
    ),
];

fn opt(id: &str) -> Option<&'static Opt> {
    OPTIONS.iter().find(|o| o.id == id)
}

/// A randomizer settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    /// The randomizer version that wrote it; it updates older ones itself.
    version: u32,
    /// The decoded settings string.
    data: Vec<u8>,
}

impl Preset {
    /// Reads the contents of an `.rnqs` file.
    pub fn read(file: &[u8]) -> Result<Preset, PresetError> {
        if file.len() < 8 {
            return Err(PresetError::Malformed);
        }
        let version = u32::from_be_bytes([file[0], file[1], file[2], file[3]]);
        let len = u32::from_be_bytes([file[4], file[5], file[6], file[7]]) as usize;
        let text = file.get(8..8 + len).ok_or(PresetError::Malformed)?;
        let data = base64_decode(text).ok_or(PresetError::Malformed)?;
        let name = *data.get(OPTION_BYTES).ok_or(PresetError::Malformed)? as usize;
        if data.len() != OPTION_BYTES + 1 + name + 8 {
            return Err(PresetError::Malformed);
        }
        let preset = Preset { version, data };
        if preset.crc() != preset.stored_crc() {
            return Err(PresetError::Checksum);
        }
        Ok(preset)
    }

    /// The contents of an `.rnqs` file with these settings.
    pub fn write(&self) -> Vec<u8> {
        let text = base64_encode(&self.data);
        let mut out = Vec::with_capacity(8 + text.len());
        out.extend_from_slice(&self.version.to_be_bytes());
        out.extend_from_slice(&(text.len() as u32).to_be_bytes());
        out.extend_from_slice(text.as_bytes());
        out
    }

    /// The name of the game the settings were made for, as the randomizer
    /// writes it, e.g. `Pokemon Black 2 (G)`.
    pub fn rom_name(&self) -> String {
        let len = self.data[OPTION_BYTES] as usize;
        String::from_utf8_lossy(&self.data[OPTION_BYTES + 1..OPTION_BYTES + 1 + len]).into_owned()
    }

    fn checked(&self) -> usize {
        self.data.len() - 8
    }

    fn crc(&self) -> u32 {
        crc32(&self.data[..self.checked()])
    }

    fn stored_crc(&self) -> u32 {
        let o = self.checked();
        u32::from_be_bytes([
            self.data[o],
            self.data[o + 1],
            self.data[o + 2],
            self.data[o + 3],
        ])
    }

    fn bit(&self, byte: usize, bit: u8) -> bool {
        self.data[byte] & (1 << bit) != 0
    }

    fn set_bit(&mut self, byte: usize, bit: u8, on: bool) {
        if on {
            self.data[byte] |= 1 << bit;
        } else {
            self.data[byte] &= !(1 << bit);
        }
    }

    /// What the file says for an option of [`OPTIONS`]. A choice none of
    /// whose bits is set, as files of another version may have it, is `None`.
    pub fn get(&self, id: &str) -> Option<Value> {
        match opt(id)?.kind {
            Kind::Flag(byte, bit) => Some(Value::Flag(self.bit(byte, bit))),
            Kind::Choice(of) => of
                .iter()
                .find(|&&(_, byte, bit)| self.bit(byte, bit))
                .map(|&(name, _, _)| Value::Choice(name.to_owned())),
        }
    }

    /// Sets an option; `false` if the table has no such option or choice,
    /// and then nothing is changed.
    fn put(&mut self, id: &str, value: &Value) -> bool {
        match (opt(id).map(|o| o.kind), value) {
            (Some(Kind::Flag(byte, bit)), Value::Flag(on)) => self.set_bit(byte, bit, *on),
            (Some(Kind::Choice(of)), Value::Choice(name))
                if of.iter().any(|&(n, _, _)| n == name) =>
            {
                for &(n, byte, bit) in of {
                    self.set_bit(byte, bit, n == name);
                }
            }
            _ => return false,
        }
        true
    }

    fn compat(&self, id: &str) -> Compat {
        match self.get(id) {
            Some(Value::Choice(c)) if c == "full" => Compat::Full,
            Some(Value::Choice(c)) if c == "random" => Compat::Random,
            Some(Value::Choice(c)) if c == "random_prefer_type" => Compat::RandomPreferType,
            _ => Compat::Unchanged,
        }
    }

    pub fn tm(&self) -> Compat {
        self.compat("tm")
    }

    pub fn tutor(&self) -> Compat {
        self.compat("tutor")
    }

    pub fn full_hm(&self) -> bool {
        self.bit(TMS2, 0)
    }

    /// Whether the moves on the TMs are randomized.
    pub fn random_tm_moves(&self) -> bool {
        self.bit(TMS, 3)
    }

    /// These settings for SoulSync's own randomizer, if it can do exactly
    /// what they ask. Anything it has not built yet, down to a single
    /// switch, is `None`: such settings go to the original randomizer.
    pub fn native(&self) -> Option<crate::rando::gen5::Options> {
        use crate::rando::gen5::{
            AbilityRules, Change, Evolving, Movesets, Options, ShopRules, Slow, Spread, Starters,
            StaticRules, Statics, Stats, Taught, Themes, TrainerItems, TrainerRules, Wild,
            WildRule, WildRules,
        };
        // Files of 4.6.0 and 4.6.1 have the same layout; older ones the
        // randomizer rewrites first.
        if !(321..=322).contains(&self.version) {
            return None;
        }
        let d = &self.data[..OPTION_BYTES];
        let bit = |byte: usize, bit: u8| d[byte] & (1 << bit) != 0;
        // Exactly one of two ways, each a bit of the same byte, and no
        // other bit of `mask`: whether it is the second.
        let either = |byte: usize, mask: u8, off: u8, on: u8| match d[byte] & mask {
            b if b == 1 << off => Some(false),
            b if b == 1 << on => Some(true),
            _ => None,
        };
        const TWEAKS: u32 = 1 << 7 | 1 << 8 | 1 << 12;
        let tweaks = u32::from_be_bytes([d[32], d[33], d[34], d[35]]);
        // Evolutions and moves as they are; nothing added to trainers.
        let untouched = d[19] & !7 == 0
            && d[25] & 0xE0 == 0
            && d[27] & (1 << 5 | 1 << 6) == 0
            && d[41] & 0x40 == 0
            && d[0] & !(1 | 1 << 3 | 1 << 4 | 1 << 5 | 1 << 6) == 0
            && d[16] & (1 << 5 | 1 << 6) == 0
            && tweaks & !TWEAKS == 0;
        if !untouched {
            return None;
        }
        // The trainers' Pokémon: as they are, random, random and about
        // equally often, or of one type for each trainer.
        let (random_trainers, spread, themes) = match d[13] {
            0b00_0001 => (false, Spread::Free, Themes::None),
            0b00_0010 => (true, Spread::Free, Themes::None),
            0b00_0100 => (true, Spread::Even, Themes::None),
            0b00_1000 => (true, Spread::EvenInStory, Themes::None),
            0b01_0000 => (true, Spread::Free, Themes::All),
            0b10_0000 => (true, Spread::Free, Themes::GymsAndElite),
            _ => return None,
        };
        let trainer_rules = TrainerRules {
            spread,
            themes,
            type_weights: bit(27, 2),
            similar_strength: bit(27, 0),
            no_legendaries: bit(27, 3),
            no_early_wonder_guard: bit(27, 4),
            levels: if bit(36, 7) {
                (d[36] & 0x7F) as i8 - 50
            } else {
                0
            },
            elite_unique: d[50] & 7,
        };
        // What the species evolve into; a new one at every level is not
        // built, and other forms the game cannot evolve into.
        let random_evolutions = match d[26] & 0b1100_0011 {
            0b01 => None,
            0b10 => Some(Evolving {
                similar_strength: bit(26, 2),
                same_type: bit(26, 3),
                three_stages: bit(26, 4),
                force_change: bit(26, 5),
                every_level: false,
            }),
            0x80 => Some(Evolving {
                same_type: bit(26, 3),
                force_change: bit(26, 5),
                every_level: true,
                ..Evolving::default()
            }),
            _ => return None,
        };
        // What the species learn: one way of four.
        let movesets = match d[11] & 0x0F {
            0b0001 => Movesets::Random,
            0b0010 => Movesets::PreferType,
            0b0100 => Movesets::Unchanged,
            0b1000 => Movesets::Metronome,
            _ => return None,
        };
        // What the species are like. Updated stats, levelling alike, a
        // limit on the Pokémon and the bans on abilities are not built.
        let stats = match d[1] & 0b0110_1110 {
            0b1000 => Stats::Unchanged,
            0b0100 => Stats::Shuffle,
            0b0010 => Stats::Random,
            _ => return None,
        };
        let types = match d[2] & 0b0111_1111 {
            0b100 => Change::Unchanged,
            0b010 => Change::Random,
            0b001 => Change::FollowEvolutions,
            _ => return None,
        };
        let abilities = match (d[3] & 0b1000_0011, bit(3, 3)) {
            (0b01, _) => Change::Unchanged,
            (0b10, false) => Change::Random,
            (0b10, true) => Change::FollowEvolutions,
            _ => return None,
        };
        // Who learns what: one way of four; then the moves themselves,
        // random or not. Nothing else of these bytes is built.
        let learning = |byte: usize| {
            let how = match d[byte] & 0b1000_0111 {
                0b100 => Compat::Unchanged,
                0b010 => Compat::RandomPreferType,
                0b001 => Compat::Random,
                0x80 => Compat::Full,
                _ => return None,
            };
            match d[byte] & 0b0001_1000 {
                0b1_0000 => Some((how, false)),
                0b0_1000 => Some((how, true)),
                _ => None,
            }
        };
        let (machines, random_tm_moves) = learning(TMS)?;
        let (tutors, random_tutor_moves) = learning(TUTORS)?;
        // The rules for both: the bytes of the two are laid out alike.
        let taught = |byte: usize, attacks: usize, broken: u8, follow: u8| Taught {
            no_broken: bit(39, broken),
            keep_field: bit(byte, 6),
            good_attacks: if bit(attacks, 7) {
                d[attacks] & 0x7F
            } else {
                0
            },
            keep_learnt: bit(byte, 5),
            follow_evolutions: bit(19, follow),
        };
        // Wild Pokémon: as they are, or new ones for each area, each
        // slot or the whole game; then the one rule they follow.
        let wild = match d[15] & 0b0111_0010 {
            0b0100_0000 => None,
            0b0000_0010 => Some(Wild::Areas),
            0b0010_0000 => Some(Wild::Slots),
            0b0001_0000 => Some(Wild::Everywhere),
            _ => return None,
        };
        let wild_rule = match (d[15] & 0b1101, bit(16, 2)) {
            (0b0100, false) => WildRule::None,
            (0b0001, false) => WildRule::CatchThemAll,
            (0b1000, false) => WildRule::TypeThemes,
            (0, true) => WildRule::SimilarStrength,
            _ => return None,
        };
        let random_trades = match d[23] & (1 | 1 << 1 | 1 << 6) {
            0x40 => false,
            1 | 2 => true,
            _ => return None,
        };
        let fully_evolved = match (bit(14, 7), d[14] & 0x7F) {
            (false, _) => 0,
            (true, 0) => return None,
            (true, level) => level as u16,
        };
        // Gaining levels alike: at one of the game's six speeds, and who
        // stays slow.
        let level_alike = match (bit(1, 4), d[39] & 7, d[46]) {
            (false, _, _) => None,
            (true, 0b001, speed @ 0..=5) => Some((Slow::Legendaries, speed)),
            (true, 0b010, speed @ 0..=5) => Some((Slow::StrongLegendaries, speed)),
            (true, 0b100, speed @ 0..=5) => Some((Slow::None, speed)),
            _ => return None,
        };
        // With evolutions at every level the families go in circles, and
        // what follows evolutions never ends, in the original neither.
        let circles = random_evolutions.is_some_and(|e| e.every_level);
        let follows = bit(1, 0) || abilities == Change::FollowEvolutions || d[19] & 6 != 0;
        if circles && follows {
            return None;
        }
        Some(Options {
            random_evolutions,
            movesets,
            moves_at_start: if bit(11, 4) { (d[11] >> 6) + 2 } else { 0 },
            good_attacks: if bit(12, 7) { d[12] & 0x7F } else { 0 },
            no_broken_moves: bit(39, 3),
            order_moves_by_damage: bit(11, 5),
            national_dex: tweaks & 1 << 7 != 0,
            update_type_effectiveness: tweaks & 1 << 8 != 0,
            ban_lucky_egg: tweaks & 1 << 12 != 0,
            random_wild_held_items: bit(16, 3),
            good_wild_held_items: bit(16, 3) && bit(16, 4),
            change_impossible_evolutions: bit(0, 0),
            easier_evolutions: bit(0, 5),
            no_time_evolutions: bit(0, 6),
            random_wild: wild.is_some(),
            wild_rules: WildRules {
                mode: wild.unwrap_or_default(),
                rule: wild_rule,
                seasons: bit(15, 7),
                no_legendaries: bit(16, 1),
                balance_shaking_grass: bit(16, 7),
                levels: if bit(38, 7) {
                    (d[38] & 0x7F) as i8 - 50
                } else {
                    0
                },
            },
            min_catch_rate: match (bit(16, 0), (d[50] >> 3) & 7) {
                (false, _) => 0,
                (true, step @ 0..=4) => step + 1,
                _ => return None,
            },
            wild_forms: bit(39, 7),
            // Held items and other forms for starters are not of
            // these games.
            starters: match d[4] {
                0b0100 => Starters::Unchanged,
                0b0010 => Starters::Random,
                0b1000 => Starters::TwoEvolutions,
                0b0001 => {
                    let wanted = [5, 7, 9].map(|at| u16::from_le_bytes([d[at], d[at + 1]]));
                    // A species the game does not have, the original
                    // swaps for another.
                    if wanted.iter().any(|&w| usize::from(w) > 649) {
                        return None;
                    }
                    Starters::Custom(wanted)
                }
                _ => return None,
            },
            random_move_powers: bit(25, 0),
            random_move_accuracies: bit(25, 1),
            random_move_pps: bit(25, 2),
            random_move_types: bit(25, 3),
            random_move_categories: bit(25, 4),
            level_alike,
            stats,
            stats_follow_evolutions: bit(1, 0),
            stats_grow_at_random: bit(1, 7),
            types,
            two_types: bit(2, 7),
            abilities,
            ability_rules: AbilityRules {
                wonder_guard: bit(3, 2),
                no_trapping: bit(3, 4),
                no_negative: bit(3, 5),
                no_bad: bit(3, 6),
                always_two: bit(48, 6),
                alike_together: bit(40, 7),
            },
            random_tutor_moves,
            random_tm_moves,
            tm_rules: taught(TMS, 20, 4, 1),
            tutor_rules: taught(TUTORS, 22, 5, 2),
            machines,
            tutors,
            random_trainers,
            more_trainer_pokemon: [d[41] & 7, d[40] >> 4 & 7, d[40] >> 1 & 7],
            trainer_rules,
            trainer_forms: bit(39, 6),
            rival_carries_starter: bit(27, 1),
            trainers_fully_evolved_from: fully_evolved,
            double_battles: bit(40, 0),
            better_trainer_moves: bit(27, 7),
            trainer_items: TrainerItems {
                boss: bit(48, 0),
                important: bit(48, 1),
                regular: bit(48, 2),
                consumable: bit(48, 3),
                sensible: bit(48, 4),
                strongest_only: bit(48, 5),
            },
            random_trainer_names: bit(0, 3),
            random_trainer_class_names: bit(0, 4),
            random_trades,
            trades_want_random: random_trades && bit(23, 0),
            random_trade_nicknames: random_trades && bit(23, 4),
            random_trade_trainers: random_trades && bit(23, 5),
            random_trade_ivs: random_trades && bit(23, 3),
            random_trade_items: random_trades && bit(23, 2),
            // Items lying around: as they are, in other places, random,
            // or random and evenly spread.
            random_field_items: match d[24] & 0b1_0111 {
                0b0_0100 | 0b0_0010 => false,
                0b0_0001 | 0b1_0000 => true,
                _ => return None,
            },
            good_field_items: d[24] & 0b1_0001 != 0 && bit(24, 3),
            even_field_items: bit(24, 4),
            shuffle_field_items: bit(24, 1),
            random_shops: match d[37] & 0b111 {
                0b100 | 0b010 => false,
                0b001 => true,
                _ => return None,
            },
            good_shop_items: bit(37, 0) && bit(37, 3),
            shop_rules: ShopRules {
                no_usual_goods: bit(37, 4),
                no_overpowered: bit(37, 5),
                fair_prices: bit(37, 6),
                evolution_items: bit(37, 7),
                battle_items: bit(41, 7),
            },
            shuffle_shops: bit(37, 1),
            random_pickup: either(49, !(1 << 2), 1, 0)?,
            good_pickup_items: bit(49, 0) && bit(49, 2),
            random_statics: !bit(17, 0),
            static_rules: StaticRules {
                // Mega evolutions are not of these games.
                mode: match d[17] & 0x8F {
                    1 | 2 => Statics::Matching,
                    4 => Statics::Random,
                    8 => Statics::SimilarStrength,
                    _ => return None,
                },
                limit_main_legendaries: bit(17, 4),
                limit_600: bit(17, 5),
                forms: bit(17, 6),
                levels: if bit(47, 7) {
                    (d[47] & 0x7F) as i8 - 50
                } else {
                    0
                },
            },
            full_hm: bit(19, 0),
        })
    }

    /// Every option of [`OPTIONS`] as this file has it.
    pub fn options(&self) -> Overrides {
        Overrides(
            OPTIONS
                .iter()
                .map(|o| (o.id.to_owned(), self.get(o.id)))
                .collect(),
        )
    }

    /// The same settings with the given options changed.
    pub fn with(&self, overrides: &Overrides) -> Preset {
        let mut out = self.clone();
        for (id, value) in &overrides.0 {
            if let Some(value) = value {
                out.put(id, value);
            }
        }
        let o = out.checked();
        let crc = out.crc().to_be_bytes();
        out.data[o..o + 4].copy_from_slice(&crc);
        out
    }
}

/// A file as Base64, to send it along with a run.
pub fn encode(file: &[u8]) -> String {
    base64_encode(file)
}

/// The file back from [`encode`].
pub fn decode(text: &str) -> Option<Vec<u8>> {
    base64_decode(text.as_bytes())
}

/// CRC-32 as `java.util.zip.CRC32` computes it.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn base64_decode(text: &[u8]) -> Option<Vec<u8>> {
    let text: Vec<u8> = text
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if text.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    for chunk in text.chunks(4) {
        let mut n = 0u32;
        let mut pad = 0;
        for &c in chunk {
            let v = match c {
                b'=' => {
                    pad += 1;
                    0
                }
                _ if pad > 0 => return None,
                _ => ALPHABET.iter().position(|&a| a == c)? as u32,
            };
            n = n << 6 | v;
        }
        if pad > 2 {
            return None;
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..3 - pad]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rando::gen5::{
        AbilityRules, ShopRules, Slow, Starters, StaticRules, Statics, Taught, Wild, WildRule,
        WildRules,
    };

    /// A real settings file for Black 2 (German), saved by randomizer 4.6.0:
    /// TM moves random, TM compatibility unchanged, full HM compatibility.
    const FILE: &str = "AAABQQAAAGxhUWdFQVFMdkFmSUI5UUVFQUFLb0JoZ0NEQUVBREFBZENRQUJBZ0FBQUFBQUFCR0FNZ2t5d1FBSUNUSUdCZ0F5QUFVQUUxQnZhMlZ0YjI0Z1FteGhZMnNnTWlBb1J5bGRsbGNONDhNNGlnPT0=";

    fn file() -> Vec<u8> {
        base64_decode(FILE.as_bytes()).unwrap()
    }

    #[test]
    fn the_soul_link_preset_is_one_for_the_own_randomizer() {
        use crate::rando::gen5::{Change, Evolving, Movesets, Spread, Stats, Themes};
        let file = include_bytes!("../../soulsync-server/presets/soul-link.rnqs");
        let preset = Preset::read(file).unwrap();
        let o = preset.native().expect("every option of it is built");
        let all = [
            o.national_dex,
            o.update_type_effectiveness,
            o.ban_lucky_egg,
            o.random_wild_held_items,
            o.good_wild_held_items,
            o.change_impossible_evolutions,
            o.easier_evolutions,
            o.no_time_evolutions,
            o.random_wild,
            o.wild_forms,
            o.starters == Starters::Random,
            o.random_tutor_moves,
            o.random_trainers,
            o.trainer_forms,
            o.rival_carries_starter,
            o.random_trainer_names,
            o.random_trades,
            o.trades_want_random,
            o.random_trade_nicknames,
            o.random_trade_ivs,
            o.random_trade_items,
            o.random_field_items,
            o.good_field_items,
            o.random_shops,
            o.good_shop_items,
            o.random_pickup,
            o.good_pickup_items,
            o.random_statics,
            o.full_hm,
        ];
        assert!(all.iter().all(|&on| on), "{o:?}");
        assert_eq!(o.trainers_fully_evolved_from, 40);

        // Switched off in SoulSync, an option is off here too.
        let mut off = Overrides::default();
        off.set("wild", Value::Choice("unchanged".into()));
        off.set("random_trainer_names", Value::Flag(false));
        let o = preset.with(&off).native().unwrap();
        assert!(!o.random_wild && !o.random_trainer_names && o.random_trainers);
        assert_eq!(
            (o.machines, o.tutors),
            (Compat::Unchanged, Compat::Unchanged)
        );

        let mut traits = Overrides::default();
        traits.set("base_stats", Value::Choice("shuffle".into()));
        traits.set("base_stats_follow_evolutions", Value::Flag(true));
        traits.set("types", Value::Choice("random_follow_evolutions".into()));
        traits.set("abilities", Value::Choice("random".into()));
        traits.set("allow_wonder_guard", Value::Flag(true));
        let o = preset.with(&traits).native().unwrap();
        assert_eq!(o.stats, Stats::Shuffle);
        assert!(o.stats_follow_evolutions && o.ability_rules.wonder_guard && !o.two_types);
        assert_eq!(
            (o.types, o.abilities),
            (Change::FollowEvolutions, Change::Random)
        );
        traits.set("abilities_follow_evolutions", Value::Flag(true));
        let o = preset.with(&traits).native().unwrap();
        assert_eq!(o.abilities, Change::FollowEvolutions);

        let mut learnt = Overrides::default();
        learnt.set("movesets", Value::Choice("random_prefer_type".into()));
        let mut learnt = preset.with(&learnt);
        let o = learnt.native().unwrap();
        assert_eq!(o.movesets, Movesets::PreferType);
        assert_eq!((o.moves_at_start, o.good_attacks), (0, 0));
        assert!(!o.no_broken_moves && !o.order_moves_by_damage);
        // Four moves from the start, in order of power, 40 percent of
        // them good attacks and none of fixed damage.
        learnt.data[11] |= 1 << 4 | 1 << 5 | 2 << 6;
        learnt.data[12] = 0x80 | 40;
        learnt.data[39] |= 1 << 3;
        let o = learnt.native().unwrap();
        assert_eq!((o.moves_at_start, o.good_attacks), (4, 40));
        assert!(o.no_broken_moves && o.order_moves_by_damage);

        let mut fights = Overrides::default();
        fights.set("trainers", Value::Choice("distributed".into()));
        fights.set("trainers_no_legendaries", Value::Flag(true));
        let mut fights = preset.with(&fights);
        let o = fights.native().unwrap();
        assert!(o.random_trainers && o.trainer_rules.no_legendaries);
        assert_eq!(o.trainer_rules.spread, Spread::Even);
        assert_eq!(o.trainer_rules.levels, 0);
        // A fifth lower, about as strong as before.
        fights.data[36] = 0x80 | 30;
        fights.data[27] |= 1;
        let rules = fights.native().unwrap().trainer_rules;
        assert!(rules.similar_strength && !rules.no_early_wonder_guard);
        assert_eq!(rules.levels, -20);

        let mut themed = Overrides::default();
        themed.set("trainers", Value::Choice("type_themed_gyms".into()));
        let mut themed = preset.with(&themed);
        let rules = themed.native().unwrap().trainer_rules;
        assert_eq!(
            (rules.themes, rules.type_weights),
            (Themes::GymsAndElite, false)
        );
        themed.data[27] |= 1 << 2;
        assert!(themed.native().unwrap().trainer_rules.type_weights);
        // One more Pokémon for regular trainers, two for important
        // ones, three for bosses.
        themed.data[41] |= 1;
        themed.data[40] |= 2 << 4 | 3 << 1;
        assert_eq!(themed.native().unwrap().more_trainer_pokemon, [1, 2, 3]);
        themed.data[50] |= 3;
        assert_eq!(themed.native().unwrap().trainer_rules.elite_unique, 3);
        // Bosses' Pokémon hold items of use to them, the strongest only.
        assert!(!themed.native().unwrap().trainer_items.any());
        themed.data[48] |= 1 | 1 << 4 | 1 << 5;
        let items = themed.native().unwrap().trainer_items;
        assert!(items.boss && items.sensible && items.strongest_only);
        assert!(!items.regular && !items.consumable);
        assert!(!themed.native().unwrap().better_trainer_moves);
        themed.data[27] |= 1 << 7;
        assert!(themed.native().unwrap().better_trainer_moves);
        assert!(!themed.native().unwrap().double_battles);
        themed.data[40] |= 1;
        assert!(themed.native().unwrap().double_battles);

        // Wild Pokémon: slot by slot and all alike in strength, for
        // each season, a fifth higher in level.
        let mut wild = preset.clone();
        assert_eq!(wild.native().unwrap().wild_rules, WildRules::default());
        wild.data[15] = 1 << 5 | 1 << 7;
        wild.data[16] |= 1 << 1 | 1 << 2 | 1 << 7;
        wild.data[38] = 0x80 | 70;
        let rules = wild.native().unwrap().wild_rules;
        assert_eq!(
            (rules.mode, rules.rule),
            (Wild::Slots, WildRule::SimilarStrength)
        );
        assert!(rules.seasons && rules.no_legendaries && rules.balance_shaking_grass);
        assert_eq!(rules.levels, 20);
        // Two rules at once, or none, are no setting of the original.
        wild.data[15] |= 1;
        assert!(wild.native().is_none());
        wild.data[15] = 1 << 4 | 1 << 3;
        wild.data[16] &= !(1 << 2);
        let rules = wild.native().unwrap().wild_rules;
        assert_eq!(
            (rules.mode, rules.rule),
            (Wild::Everywhere, WildRule::TypeThemes)
        );

        // Fixed Pokémon: alike in strength with both limits and other
        // forms, half as high in level again.
        let mut fixed = preset.clone();
        assert_eq!(fixed.native().unwrap().static_rules, StaticRules::default());
        fixed.data[17] = 1 << 3 | 1 << 4 | 1 << 5 | 1 << 6;
        fixed.data[47] = 0x80 | 100;
        let native = fixed.native().unwrap();
        assert!(native.random_statics);
        let rules = native.static_rules;
        assert_eq!(rules.mode, Statics::SimilarStrength);
        assert!(rules.limit_main_legendaries && rules.limit_600 && rules.forms);
        assert_eq!(rules.levels, 50);
        fixed.data[17] = 1 << 2;
        assert_eq!(fixed.native().unwrap().static_rules.mode, Statics::Random);
        // Unchanged, they can still change in level.
        fixed.data[17] = 1;
        let native = fixed.native().unwrap();
        assert!(!native.random_statics);
        assert_eq!(native.static_rules.levels, 50);
        // Two ways at once, or mega evolutions, are not of these games.
        fixed.data[17] = 1 << 1 | 1 << 2;
        assert!(fixed.native().is_none());
        fixed.data[17] = 1 << 1 | 1 << 7;
        assert!(fixed.native().is_none());

        // Easier catching, in the fourth of five steps.
        let mut catching = preset.clone();
        assert_eq!(catching.native().unwrap().min_catch_rate, 0);
        catching.data[16] |= 1;
        catching.data[50] = catching.data[50] & 7 | 3 << 3;
        assert_eq!(catching.native().unwrap().min_catch_rate, 4);
        catching.data[50] |= 7 << 3;
        assert!(catching.native().is_none());

        // Random abilities with every rule there is.
        let mut able = preset.clone();
        assert_eq!(
            able.native().unwrap().ability_rules,
            AbilityRules::default()
        );
        able.data[3] = 1 << 1 | 1 << 2 | 1 << 4 | 1 << 5 | 1 << 6;
        able.data[40] |= 0x80;
        able.data[48] |= 1 << 6;
        let rules = able.native().unwrap().ability_rules;
        assert!(rules.wonder_guard && rules.no_trapping && rules.no_negative && rules.no_bad);
        assert!(rules.always_two && rules.alike_together);

        // Starters: three that evolve twice, or chosen ones, where 0
        // leaves a place to chance.
        let mut first = preset.clone();
        first.data[4] = 1 << 3;
        assert_eq!(first.native().unwrap().starters, Starters::TwoEvolutions);
        first.data[4] = 1;
        first.data[5..11].copy_from_slice(&[25, 0, 0, 0, 0x89, 0x02]);
        let starters = first.native().unwrap().starters;
        assert_eq!(starters, Starters::Custom([25, 0, 649]));
        first.data[9] += 1;
        assert!(first.native().is_none());
        first.data[4] = 1 << 1 | 1 << 6;
        assert!(first.native().is_none());

        // Items: lying around evenly spread or in other places, and the
        // shops with every rule there is.
        let mut items = preset.clone();
        let native = items.native().unwrap();
        assert!(native.random_field_items && !native.even_field_items);
        assert_eq!(native.shop_rules, ShopRules::default());
        items.data[24] = 1 << 4 | 1 << 3;
        let native = items.native().unwrap();
        assert!(native.random_field_items && native.even_field_items && native.good_field_items);
        items.data[24] = 1 << 1;
        let native = items.native().unwrap();
        assert!(native.shuffle_field_items && !native.random_field_items);
        items.data[37] |= 0xF0;
        items.data[41] |= 0x80;
        let rules = items.native().unwrap().shop_rules;
        assert!(rules.no_usual_goods && rules.no_overpowered && rules.fair_prices);
        assert!(rules.evolution_items && rules.battle_items);
        items.data[37] = 1 << 1;
        let native = items.native().unwrap();
        assert!(native.shuffle_shops && !native.random_shops);
        items.data[37] = 1 | 1 << 1;
        assert!(items.native().is_none());

        // Attacks physical or special by chance, all species gaining
        // levels fast apart from the strong legendary ones, and traded
        // Pokémon from random trainers.
        let mut more = preset.clone();
        let native = more.native().unwrap();
        assert!(!native.random_move_categories && !native.random_trade_trainers);
        assert_eq!(native.level_alike, None);
        more.data[25] |= 1 << 4;
        more.data[23] |= 1 << 5;
        more.data[1] |= 1 << 4;
        more.data[39] = more.data[39] & !7 | 0b010;
        more.data[46] = 4;
        let native = more.native().unwrap();
        assert!(native.random_move_categories && native.random_trade_trainers);
        assert_eq!(native.level_alike, Some((Slow::StrongLegendaries, 4)));
        more.data[46] = 6;
        assert!(more.native().is_none());

        // TMs and move tutors with every rule of their own.
        let mut taught = preset.clone();
        let native = taught.native().unwrap();
        assert_eq!(native.tm_rules, Taught::default());
        assert_eq!(native.tutor_rules, Taught::default());
        taught.data[TMS] |= 1 << 5 | 1 << 6;
        taught.data[19] |= 1 << 1;
        taught.data[20] = 0x80 | 40;
        taught.data[39] |= 1 << 4;
        let native = taught.native().unwrap();
        let all = Taught {
            no_broken: true,
            keep_field: true,
            good_attacks: 40,
            keep_learnt: true,
            follow_evolutions: true,
        };
        assert_eq!(native.tm_rules, all);
        assert_eq!(native.tutor_rules, Taught::default());
        taught.data[TUTORS] |= 1 << 5 | 1 << 6;
        taught.data[19] |= 1 << 2;
        taught.data[22] = 0x80 | 40;
        taught.data[39] |= 1 << 5;
        assert_eq!(taught.native().unwrap().tutor_rules, all);

        let mut evolving = Overrides::default();
        evolving.set("evolutions", Value::Choice("random".into()));
        let mut evolving = preset.with(&evolving);
        assert_eq!(
            evolving.native().unwrap().random_evolutions,
            Some(Evolving::default())
        );
        // About as strong as before and three stages at most.
        evolving.data[26] |= 1 << 2 | 1 << 4;
        let how = evolving.native().unwrap().random_evolutions.unwrap();
        assert!(how.similar_strength && how.three_stages && !how.same_type && !how.force_change);
        // Other forms are nothing the game can evolve into.
        evolving.data[26] |= 1 << 6;
        assert_eq!(evolving.native(), None);
        // At every level: only the type and the change count.
        evolving.data[26] = 0x80 | 1 << 3 | 1 << 4;
        let how = evolving.native().unwrap().random_evolutions.unwrap();
        assert!(how.every_level && how.same_type && !how.three_stages);
        // What follows evolutions would go in circles then.
        evolving.data[19] |= 1 << 1;
        assert_eq!(evolving.native(), None);

        let mut tms = Overrides::default();
        tms.set("tm", Value::Choice("full".into()));
        tms.set("tm_moves", Value::Choice("random".into()));
        tms.set("tutor", Value::Choice("random_prefer_type".into()));
        let o = preset.with(&tms).native().unwrap();
        assert!(o.random_tm_moves && o.random_tutor_moves);
        assert_eq!(
            (o.machines, o.tutors),
            (Compat::Full, Compat::RandomPreferType)
        );
    }

    #[test]
    fn what_is_not_built_goes_to_the_original_randomizer() {
        let file = include_bytes!("../../soulsync-server/presets/soul-link.rnqs");
        let preset = Preset::read(file).unwrap();
        for (id, value) in [
            ("update_base_stats", Value::Flag(true)),
            ("update_moves", Value::Flag(true)),
        ] {
            let mut change = Overrides::default();
            change.set(id, value);
            assert_eq!(preset.with(&change).native(), None, "{id}");
        }
        // Every wild Pokémon becomes one other all over the game, and
        // every species comes up before one comes up twice.
        let mut wild = Overrides::default();
        wild.set("wild", Value::Choice("global_mapping".into()));
        wild.set("wild_rule", Value::Choice("catch_em_all".into()));
        wild.set("wild_no_legendaries", Value::Flag(true));
        let rules = preset.with(&wild).native().unwrap().wild_rules;
        assert_eq!(
            (rules.mode, rules.rule, rules.no_legendaries),
            (Wild::Everywhere, WildRule::CatchThemAll, true)
        );
        // The file of the other tests differs in its random TM moves only.
        let other = Preset::read(&super::tests::file()).unwrap().native();
        assert!(other.is_some_and(|o| o.random_tm_moves && o.random_wild));
    }

    #[test]
    fn reads_a_real_file() {
        let p = Preset::read(&file()).unwrap();
        assert_eq!(p.rom_name(), "Pokemon Black 2 (G)");
        assert_eq!(p.tm(), Compat::Unchanged);
        assert!(p.random_tm_moves());
        assert!(p.full_hm());
        assert_eq!(p.tutor(), Compat::Unchanged);
        assert_eq!(p.write(), file());
    }

    #[test]
    fn changes_options_and_keeps_the_rest() {
        let p = Preset::read(&file()).unwrap();
        let mut o = Overrides::default();
        o.set("tm", Value::Choice("random_prefer_type".into()));
        o.set("tutor", Value::Choice("full".into()));
        o.set("full_hm", Value::Flag(false));
        // Unknown options and choices change nothing.
        o.set("no_such_option", Value::Flag(true));
        o.set("starters", Value::Choice("no_such_choice".into()));
        let q = Preset::read(&p.with(&o).write()).unwrap();
        assert_eq!(q.tm(), Compat::RandomPreferType);
        assert_eq!(q.tutor(), Compat::Full);
        assert!(!q.full_hm());
        assert!(q.random_tm_moves(), "TM moves stay random");
        let changed: Vec<usize> = (0..p.data.len())
            .filter(|&i| p.data[i] != q.data[i])
            .collect();
        assert_eq!(changed[..3], [TMS, TMS2, TUTORS]);
        assert!(
            changed[3..].iter().all(|&i| i >= p.checked()),
            "only the CRC else"
        );
        // Back to what it was gives the same file.
        assert_eq!(q.with(&p.options()).write(), file());
        assert_eq!(p.with(&Overrides::default()), p);
    }

    /// Every choice and switch was checked against randomizer 4.6.1
    /// reading the file written here (`Settings.fromString`).
    #[test]
    fn every_option_can_be_set_and_read_back() {
        let p = Preset::read(&file()).unwrap();
        assert_eq!(p.get("starters"), Some(Value::Choice("random".into())));
        assert_eq!(p.get("wild"), Some(Value::Choice("area_mapping".into())));
        assert_eq!(p.get("statics"), Some(Value::Choice("swap".into())));
        assert_eq!(p.get("easier_evolutions"), Some(Value::Flag(true)));
        for o in OPTIONS {
            let values: Vec<Value> = match o.kind {
                Kind::Flag(..) => vec![Value::Flag(true), Value::Flag(false)],
                Kind::Choice(of) => of.iter().map(|c| Value::Choice(c.0.into())).collect(),
            };
            for v in values {
                let mut change = Overrides::default();
                change.set(o.id, v.clone());
                let q = Preset::read(&p.with(&change).write()).unwrap();
                assert_eq!(q.get(o.id), Some(v), "{}", o.id);
                // Nothing else moved.
                for other in OPTIONS.iter().filter(|x| x.id != o.id) {
                    assert_eq!(q.get(other.id), p.get(other.id), "{} by {}", other.id, o.id);
                }
            }
        }
        // As the page sends them: null keeps the file's value.
        let sent: Overrides =
            serde_json::from_str(r#"{"tm":"full","full_hm":false,"wild":null}"#).unwrap();
        let q = p.with(&sent);
        assert_eq!(q.tm(), Compat::Full);
        assert!(!q.full_hm());
        assert_eq!(q.get("wild"), p.get("wild"));
        assert!(!sent.is_empty());
        assert!(serde_json::from_str::<Overrides>(r#"{"wild":null}"#)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn refuses_damaged_files() {
        let mut f = file();
        assert_eq!(Preset::read(&f[..6]), Err(PresetError::Malformed));
        let i = f.len() - 20;
        f[i] = if f[i] == b'A' { b'B' } else { b'A' };
        assert!(Preset::read(&f).is_err());
    }

    #[test]
    fn crc_matches_java() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_decode(b"YWI=").unwrap(), b"ab");
    }
}
