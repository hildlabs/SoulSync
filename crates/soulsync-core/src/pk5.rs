//! Readable view of a decrypted generation 5 Pokémon record.

use crate::crypto::{self, PARTY_SIZE, STORED_SIZE};
use crate::dex;
use crate::lang::{Lang, Names};
use serde::Serialize;

/// Where a Pokémon sits in the save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Place {
    /// Party slot 0..6.
    Party { slot: u8 },
    /// Box 1..=24, slot 0..30.
    Box { r#box: u8, slot: u8 },
}

/// Battle stats, stored for party Pokémon only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleStats {
    pub level: u8,
    pub hp: u16,
    pub max_hp: u16,
}

/// A Pokémon from a save file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pokemon {
    /// Personality value, unique per Pokémon. SoulSync uses it to recognise a
    /// Pokémon across evolutions and nickname changes.
    pub pid: u32,
    pub species: u16,
    pub species_name: Option<Names>,
    pub nickname: String,
    pub is_nicknamed: bool,
    pub is_egg: bool,
    pub met_location: u16,
    pub met_location_name: Option<Names>,
    pub met_level: u8,
    pub place: Place,
    /// Present for party Pokémon only.
    pub battle: Option<BattleStats>,
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
        nickname: read_string(&d[0x48..], 11),
        is_nicknamed: (iv >> 31) & 1 == 1,
        is_egg: (iv >> 30) & 1 == 1,
        met_location,
        met_location_name: dex::location(met_location).cloned(),
        met_level: d[0x84] & 0x7F,
        place,
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
    fn empty_and_corrupt_slots() {
        assert!(parse(&[0u8; STORED_SIZE], Place::Box { r#box: 1, slot: 0 }).is_none());
        let mut raw = build_raw(7, 25, "Pika", 14, 3, None);
        raw[40] ^= 0xFF;
        assert!(parse(&raw, Place::Box { r#box: 1, slot: 0 }).is_none());
    }
}
