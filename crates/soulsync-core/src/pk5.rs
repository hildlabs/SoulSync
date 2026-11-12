//! Readable view of a decrypted generation 5 Pokémon record.

use crate::crypto::{self, PARTY_SIZE, STORED_SIZE};
use crate::dex::{self, Type};
use crate::lang::{Lang, Names};
use serde::{Deserialize, Serialize};

/// Where a Pokémon sits in the save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Place {
    /// Party slot 0..6.
    Party { slot: u8 },
    /// Box 1..=24, slot 0..30.
    Box { r#box: u8, slot: u8 },
}

/// Battle stats, stored for party Pokémon only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleStats {
    pub level: u8,
    pub hp: u16,
    pub max_hp: u16,
}

/// One of a Pokémon's four moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveSlot {
    /// Move number, as in the game's move table.
    pub id: u16,
    /// PP left.
    pub pp: u8,
    /// PP Ups used, 0 to 3.
    pub pp_ups: u8,
}

/// The stats besides HP, as the game computed them for a party Pokémon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Stats {
    pub attack: u16,
    pub defense: u16,
    pub speed: u16,
    pub sp_attack: u16,
    pub sp_defense: u16,
}

/// A status condition, as far as it matters for catching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ailment {
    Sleep,
    Poison,
    Burn,
    Freeze,
    Paralysis,
}

impl Ailment {
    /// From the status word of a party Pokémon.
    fn from_bits(status: u32) -> Option<Self> {
        Some(match status {
            s if s & 0x07 != 0 => Ailment::Sleep,
            s if s & 0x88 != 0 => Ailment::Poison,
            s if s & 0x10 != 0 => Ailment::Burn,
            s if s & 0x20 != 0 => Ailment::Freeze,
            s if s & 0x40 != 0 => Ailment::Paralysis,
            _ => return None,
        })
    }
}

/// A Pokémon from a save file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pokemon {
    /// Personality value, unique per Pokémon. SoulSync uses it to recognise a
    /// Pokémon across evolutions and nickname changes.
    pub pid: u32,
    pub species: u16,
    pub species_name: Option<Names>,
    pub types: Vec<Type>,
    pub nickname: String,
    pub is_nicknamed: bool,
    pub is_egg: bool,
    pub met_location: u16,
    pub met_location_name: Option<Names>,
    pub met_level: u8,
    pub place: Place,
    /// Present for party Pokémon only.
    pub battle: Option<BattleStats>,
    /// Status condition of a party Pokémon.
    #[serde(default)]
    pub ailment: Option<Ailment>,
    /// Its moves, empty slots left out.
    #[serde(default)]
    pub moves: Vec<MoveSlot>,
    /// Held item number; 0 is none.
    #[serde(default)]
    pub item: u16,
    /// Ability number.
    #[serde(default)]
    pub ability: u16,
    /// Nature, 0 (Hardy) to 24 (Quirky).
    #[serde(default)]
    pub nature: u8,
    /// Present for party Pokémon only.
    #[serde(default)]
    pub stats: Option<Stats>,
}

impl Pokemon {
    /// `true` if a party Pokémon has 0 HP (dead, in Nuzlocke terms).
    pub fn is_fainted(&self) -> bool {
        self.battle.as_ref().is_some_and(|b| b.hp == 0)
    }

    /// Species name in the given language.
    pub fn species_label(&self, lang: Lang) -> String {
        self.species_name
            .as_ref()
            .map_or_else(|| format!("#{}", self.species), |n| n.get(lang).to_owned())
    }

    /// Nickname if the player gave one, otherwise the species name.
    pub fn label(&self, lang: Lang) -> String {
        if self.is_nicknamed && !self.nickname.is_empty() {
            self.nickname.clone()
        } else {
            self.species_label(lang)
        }
    }
}

fn u16_at(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}

fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

/// Reads a generation 5 string (UTF-16LE, ends at 0xFFFF or 0).
pub(crate) fn read_string(d: &[u8], max_chars: usize) -> String {
    let units: Vec<u16> = (0..max_chars)
        .map(|i| u16_at(d, i * 2))
        .take_while(|&c| c != 0xFFFF && c != 0)
        .map(|c| match c {
            0x246D => 0x2642, // ♂
            0x246E => 0x2640, // ♀
            other => other,
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// Reads a Pokémon found in the game's memory and tells whether it is a wild
/// one. Trainers' Pokémon have the same value in all six IVs, which a wild
/// Pokémon practically never has; eggs are not wild either.
///
/// The game gives a wild Pokémon the player's trainer ID before the battle,
/// so that it is theirs once caught. The Pokémon of a scripted battle the
/// player only watches, like the catching lesson on Route 19, have none:
/// seen so in a real memory dump, where the lesson's two Pokémon had ID 0
/// and every wild one the player's.
pub fn parse_wild(raw: &[u8]) -> Option<Pokemon> {
    let d = crypto::decrypt(raw)?;
    let iv = u32_at(&d, 0x38);
    let first = iv & 31;
    let trainer = (1..6).all(|i| (iv >> (5 * i)) & 31 == first);
    let watched = u32_at(&d, 0x0C) == 0;
    parse(raw, Place::Party { slot: 0 }).filter(|p| !trainer && !watched && !p.is_egg)
}

/// Decrypts and reads a record. Empty slots, corrupt data (bad checksum) and
/// invalid species give `None`.
pub fn parse(raw: &[u8], place: Place) -> Option<Pokemon> {
    let pid = u32_at(raw, 0);
    let chk = u16_at(raw, 6);
    if pid == 0 && chk == 0 {
        return None;
    }
    let d = crypto::decrypt(raw)?;
    if crypto::checksum(&d[8..STORED_SIZE]) != chk {
        return None;
    }
    let species = u16_at(&d, 0x08);
    if species == 0 || species > 649 {
        return None;
    }
    let iv = u32_at(&d, 0x38);
    let met_location = u16_at(&d, 0x80);
    let battle = (d.len() == PARTY_SIZE).then(|| BattleStats {
        level: d[0x8C],
        hp: u16_at(&d, 0x8E),
        max_hp: u16_at(&d, 0x90),
    });
    Some(Pokemon {
        pid,
        species,
        species_name: dex::species(species).map(|s| s.name.clone()),
        types: dex::species(species)
            .map(|s| s.types.clone())
            .unwrap_or_default(),
        nickname: read_string(&d[0x48..], 11),
        is_nicknamed: (iv >> 31) & 1 == 1,
        is_egg: (iv >> 30) & 1 == 1,
        met_location,
        met_location_name: dex::location(met_location).cloned(),
        met_level: d[0x84] & 0x7F,
        place,
        ailment: battle
            .as_ref()
            .and_then(|_| Ailment::from_bits(u32_at(&d, 0x88))),
        moves: (0..4)
            .map(|i| MoveSlot {
                id: u16_at(&d, 0x28 + 2 * i),
                pp: d[0x30 + i],
                pp_ups: d[0x34 + i].min(3),
            })
            .filter(|m| m.id != 0)
            .collect(),
        item: u16_at(&d, 0x0A),
        ability: u16::from(d[0x15]),
        nature: d[0x41] % 25,
        stats: (d.len() == PARTY_SIZE).then(|| Stats {
            attack: u16_at(&d, 0x92),
            defense: u16_at(&d, 0x94),
            speed: u16_at(&d, 0x96),
            sp_attack: u16_at(&d, 0x98),
            sp_defense: u16_at(&d, 0x9A),
        }),
        battle,
    })
}

/// Builds an encrypted record. Used by tests and, later, the save editor.
#[doc(hidden)]
pub fn build_raw(
    pid: u32,
    species: u16,
    nickname: &str,
    met_location: u16,
    met_level: u8,
    battle: Option<&BattleStats>,
) -> Vec<u8> {
    build_raw_ivs(pid, species, nickname, met_location, met_level, battle, 0)
}

/// Like [`build_raw`], with the six IVs (five bits each, HP first).
#[doc(hidden)]
pub fn build_raw_ivs(
    pid: u32,
    species: u16,
    nickname: &str,
    met_location: u16,
    met_level: u8,
    battle: Option<&BattleStats>,
    ivs: u32,
) -> Vec<u8> {
    build_raw_ot(
        pid,
        species,
        nickname,
        met_location,
        met_level,
        battle,
        ivs,
        1,
    )
}

/// Like [`build_raw_ivs`], with the trainer ID (and secret ID) it carries.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn build_raw_ot(
    pid: u32,
    species: u16,
    nickname: &str,
    met_location: u16,
    met_level: u8,
    battle: Option<&BattleStats>,
    ivs: u32,
    ot: u32,
) -> Vec<u8> {
    let mut p = vec![
        0u8;
        if battle.is_some() {
            PARTY_SIZE
        } else {
            STORED_SIZE
        }
    ];
    p[0..4].copy_from_slice(&pid.to_le_bytes());
    p[0x08..0x0A].copy_from_slice(&species.to_le_bytes());
    p[0x0C..0x10].copy_from_slice(&ot.to_le_bytes());
    p[0x38..0x3C].copy_from_slice(&(ivs & 0x3FFF_FFFF).to_le_bytes());
    if !nickname.is_empty() {
        p[0x3B] |= 0x80; // nickname flag (bit 31 of the IV word)
    }
    let units: Vec<u16> = nickname.encode_utf16().take(10).chain([0xFFFF]).collect();
    for (i, u) in units.iter().enumerate() {
        p[0x48 + i * 2..0x4A + i * 2].copy_from_slice(&u.to_le_bytes());
    }
    p[0x80..0x82].copy_from_slice(&met_location.to_le_bytes());
    p[0x84] = met_level & 0x7F;
    if let Some(b) = battle {
        p[0x8C] = b.level;
        p[0x8E..0x90].copy_from_slice(&b.hp.to_le_bytes());
        p[0x90..0x92].copy_from_slice(&b.max_hp.to_le_bytes());
    }
    crypto::encrypt(&p).expect("valid size")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_party_mon() {
        let b = BattleStats {
            level: 23,
            hp: 0,
            max_hp: 71,
        };
        let raw = build_raw(0xDEAD_BEEF, 488, "Mondi", 14, 5, Some(&b));
        let m = parse(&raw, Place::Party { slot: 2 }).unwrap();
        assert_eq!(m.species_label(Lang::En), "Cresselia");
        assert_eq!(m.label(Lang::En), "Mondi");
        assert!(m.is_nicknamed);
        assert_eq!(m.met_location_name.as_ref().unwrap().en, "Route 1");
        assert_eq!(m.met_level, 5);
        assert_eq!(m.battle, Some(b));
        assert!(m.is_fainted());
    }

    #[test]
    fn reads_moves_item_ability_and_stats() {
        let b = BattleStats {
            level: 30,
            hp: 80,
            max_hp: 90,
        };
        let raw = build_raw(0x1234_5678, 488, "", 14, 5, Some(&b));
        let mut d = crypto::decrypt(&raw).unwrap();
        d[0x0A..0x0C].copy_from_slice(&234u16.to_le_bytes()); // Leftovers
        d[0x15] = 26; // Levitate
        d[0x28..0x2A].copy_from_slice(&94u16.to_le_bytes()); // Psychic
        d[0x2C..0x2E].copy_from_slice(&248u16.to_le_bytes()); // a gap in slot 2
        d[0x30] = 7;
        d[0x32] = 15;
        d[0x34] = 2;
        d[0x41] = 15; // Modest
        d[0x92..0x94].copy_from_slice(&55u16.to_le_bytes());
        d[0x98..0x9A].copy_from_slice(&66u16.to_le_bytes());
        let chk = crypto::checksum(&d[8..STORED_SIZE]);
        d[6..8].copy_from_slice(&chk.to_le_bytes());
        let m = parse(&crypto::encrypt(&d).unwrap(), Place::Party { slot: 0 }).unwrap();
        assert_eq!(
            m.moves,
            [
                MoveSlot {
                    id: 94,
                    pp: 7,
                    pp_ups: 2
                },
                MoveSlot {
                    id: 248,
                    pp: 15,
                    pp_ups: 0
                },
            ]
        );
        assert_eq!((m.item, m.ability, m.nature), (234, 26, 15));
        let s = m.stats.unwrap();
        assert_eq!((s.attack, s.sp_attack), (55, 66));
    }

    #[test]
    fn empty_and_corrupt_slots() {
        assert!(parse(&[0u8; STORED_SIZE], Place::Box { r#box: 1, slot: 0 }).is_none());
        let mut raw = build_raw(7, 25, "Pika", 14, 3, None);
        raw[40] ^= 0xFF;
        assert!(parse(&raw, Place::Box { r#box: 1, slot: 0 }).is_none());
    }
}
