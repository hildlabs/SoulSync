//! Spielstände von Pokémon Schwarz 2 und Weiß 2 lesen.
//!
//! Unterstützt rohe 512-KiB-Spielstände (`.sav`) und DeSmuME-Batteriedateien
//! (`.dsv`, Spielstand plus 122 Byte DeSmuME-Fußzeile).

use crate::crypto::{PARTY_SIZE, STORED_SIZE};
use crate::pk5::{self, Place, Pokemon};
use serde::Serialize;
use std::fmt;

const SAVE_SIZE: usize = 0x80000;
const BOX_START: usize = 0x400;
const BOX_STRIDE: usize = 0x1000;
const BOX_COUNT: usize = 24;
const BOX_SLOTS: usize = 30;
const PARTY_BLOCK: usize = 0x18E00;
const TRAINER_BLOCK: usize = 0x19400;
const BADGE_FLAGS: usize = 0x21104;

/// Warum ein Spielstand nicht gelesen werden konnte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    TooSmall(usize),
    NotB2W2,
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::TooSmall(n) => write!(
                f,
                "Die Datei ist mit {n} Byte zu klein für einen Schwarz-2-/Weiß-2-Spielstand. \
                 Nimm die .dsv aus dem Battery-Ordner von DeSmuME."
            ),
            SaveError::NotB2W2 => write!(f, "Das ist kein Spielstand von Schwarz 2 oder Weiß 2."),
        }
    }
}

impl std::error::Error for SaveError {}

/// Trainerdaten aus dem Spielstand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Trainer {
    pub name: String,
    pub badges: u8,
}

/// Alles, was SoulSync aus einem Spielstand liest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SaveSnapshot {
    pub trainer: Trainer,
    pub party: Vec<Pokemon>,
    pub boxes: Vec<Pokemon>,
}

impl SaveSnapshot {
    /// Alle Pokémon ohne Eier, Team zuerst.
    pub fn all_pokemon(&self) -> impl Iterator<Item = &Pokemon> {
        self.party.iter().chain(&self.boxes).filter(|p| !p.is_egg)
    }
}

/// Liest einen Spielstand. `data` darf eine `.sav`- oder `.dsv`-Datei sein.
pub fn parse_b2w2(data: &[u8]) -> Result<SaveSnapshot, SaveError> {
    if data.len() < SAVE_SIZE {
        return Err(SaveError::TooSmall(data.len()));
    }
    let b = &data[..SAVE_SIZE];
    let count = b[PARTY_BLOCK + 4] as usize;
    if count > 6 {
        return Err(SaveError::NotB2W2);
    }
    let party = (0..count)
        .filter_map(|i| {
            let o = PARTY_BLOCK + 8 + i * PARTY_SIZE;
            pk5::parse(&b[o..o + PARTY_SIZE], Place::Party { slot: i as u8 })
        })
        .collect();
    let boxes = (0..BOX_COUNT)
        .flat_map(|x| (0..BOX_SLOTS).map(move |s| (x, s)))
        .filter_map(|(x, s)| {
            let o = BOX_START + x * BOX_STRIDE + s * STORED_SIZE;
            pk5::parse(
                &b[o..o + STORED_SIZE],
                Place::Box {
                    r#box: x as u8 + 1,
                    slot: s as u8,
                },
            )
        })
        .collect();
    let trainer = Trainer {
        name: pk5::read_string(&b[TRAINER_BLOCK + 4..], 8),
        badges: b[BADGE_FLAGS].count_ones() as u8,
    };
    Ok(SaveSnapshot {
        trainer,
        party,
        boxes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pk5::{build_raw, BattleStats};

    fn synthetic_save() -> Vec<u8> {
        let mut s = vec![0u8; SAVE_SIZE + 122];
        let party = [
            build_raw(
                0x1111_2222,
                488,
                "Mondi",
                14,
                5,
                Some(&BattleStats {
                    level: 23,
                    hp: 60,
                    max_hp: 71,
                }),
            ),
            build_raw(
                0x3333_4444,
                60,
                "Blubb",
                15,
                4,
                Some(&BattleStats {
                    level: 22,
                    hp: 0,
                    max_hp: 55,
                }),
            ),
        ];
        s[PARTY_BLOCK + 4] = party.len() as u8;
        for (i, raw) in party.iter().enumerate() {
            let o = PARTY_BLOCK + 8 + i * PARTY_SIZE;
            s[o..o + PARTY_SIZE].copy_from_slice(raw);
        }
        let boxed = build_raw(0x5555_6666, 570, "Trick", 17, 18, None);
        let o = BOX_START + 2 * BOX_STRIDE + 4 * STORED_SIZE;
        s[o..o + STORED_SIZE].copy_from_slice(&boxed);
        for (i, u) in "Hannes".encode_utf16().chain([0xFFFF]).enumerate() {
            s[TRAINER_BLOCK + 4 + i * 2..TRAINER_BLOCK + 6 + i * 2]
                .copy_from_slice(&u.to_le_bytes());
        }
        s[BADGE_FLAGS] = 0b0000_0011;
        s
    }

    #[test]
    fn reads_synthetic_save() {
        let snap = parse_b2w2(&synthetic_save()).unwrap();
        assert_eq!(
            snap.trainer,
            Trainer {
                name: "Hannes".into(),
                badges: 2
            }
        );
        assert_eq!(snap.party.len(), 2);
        assert!(snap.party[1].is_fainted());
        assert_eq!(snap.boxes.len(), 1);
        assert_eq!(snap.boxes[0].place, Place::Box { r#box: 3, slot: 4 });
        assert_eq!(snap.boxes[0].species_de.as_deref(), Some("Zorua"));
        assert_eq!(snap.all_pokemon().count(), 3);
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(parse_b2w2(&[0u8; 100]), Err(SaveError::TooSmall(100)));
        let mut s = vec![0u8; SAVE_SIZE];
        s[PARTY_BLOCK + 4] = 9;
        assert_eq!(parse_b2w2(&s), Err(SaveError::NotB2W2));
    }
}
