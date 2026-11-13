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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Compat {
    /// As in the original game.
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

    /// A real settings file for Black 2 (German), saved by randomizer 4.6.0:
    /// TM moves random, TM compatibility unchanged, full HM compatibility.
    const FILE: &str = "AAABQQAAAGxhUWdFQVFMdkFmSUI5UUVFQUFLb0JoZ0NEQUVBREFBZENRQUJBZ0FBQUFBQUFCR0FNZ2t5d1FBSUNUSUdCZ0F5QUFVQUUxQnZhMlZ0YjI0Z1FteGhZMnNnTWlBb1J5bGRsbGNONDhNNGlnPT0=";

    fn file() -> Vec<u8> {
        base64_decode(FILE.as_bytes()).unwrap()
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
