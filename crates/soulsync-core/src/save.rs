//! Reading Pokémon Black 2 and White 2 save files.
//!
//! Accepts raw 512 KiB saves (`.sav`) and DeSmuME battery files (`.dsv`,
//! the save followed by a 122-byte DeSmuME footer).

use crate::crypto::{PARTY_SIZE, STORED_SIZE};
use crate::game::Game;
use crate::pk5::{self, Place, Pokemon};
use serde::{Deserialize, Serialize};
use std::fmt;

const SAVE_SIZE: usize = 0x80000;
const BOX_START: usize = 0x400;
const BOX_STRIDE: usize = 0x1000;
const BOX_COUNT: usize = 24;
const BOX_SLOTS: usize = 30;
const PARTY_BLOCK: usize = 0x18E00;
const TRAINER_BLOCK: usize = 0x19400;
const BADGE_FLAGS: usize = 0x21104;
/// Offset of the game version byte inside the trainer block.
const GAME_VERSION: usize = 0x1F;
/// Length of the party block: an eight-byte header and six Pokémon.
pub const PARTY_BLOCK_LEN: usize = 8 + 6 * PARTY_SIZE;

/// Why a save file could not be read.
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
                "the file is {n} bytes, too small for a Black 2 / White 2 save; \
                 use the .dsv from DeSmuME's Battery folder"
            ),
            SaveError::NotB2W2 => write!(f, "this is not a Black 2 / White 2 save"),
        }
    }
}

impl std::error::Error for SaveError {}

/// Trainer data from the save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trainer {
    pub name: String,
    /// Trainer ID and secret ID. A new game has a new one.
    #[serde(default)]
    pub id: u32,
    pub badges: u8,
    /// Which game wrote the save, if it says so.
    #[serde(default)]
    pub game: Option<Game>,
}

/// Everything SoulSync reads from a save file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveSnapshot {
    pub trainer: Trainer,
    pub party: Vec<Pokemon>,
    pub boxes: Vec<Pokemon>,
    /// Wild Pokémon the player met first in a location, seen live in the
    /// emulator, with that location as `met_location`. Those the player does
    /// not own were not caught. A save file alone has none.
    #[serde(default)]
    pub encounters: Vec<Pokemon>,
    /// Pokémon that fainted at some point, as they were then. Fainted stays
    /// fainted in a Soul Link, whatever a Pokémon Center does. A save file
    /// alone has none.
    #[serde(default)]
    pub fallen: Vec<Pokemon>,
}

impl SaveSnapshot {
    /// All Pokémon except eggs, party first.
    pub fn all_pokemon(&self) -> impl Iterator<Item = &Pokemon> {
        self.party.iter().chain(&self.boxes).filter(|p| !p.is_egg)
    }

    /// Whether the player has this Pokémon, eggs included.
    pub fn owns(&self, pid: u32) -> bool {
        self.party.iter().chain(&self.boxes).any(|p| p.pid == pid)
    }
}

impl SaveSnapshot {
    /// The same save with the party an emulator has in memory right now.
    ///
    /// `block` is the party block as the game keeps it in RAM, in the layout
    /// of the save file: four bytes, the number of Pokémon, then six
    /// party-format Pokémon. Returns `None` if it is not a valid party.
    ///
    /// A Pokémon that was in the saved party but is neither in the live party
    /// nor in a saved box was deposited since the last save; see [`Self::keep`].
    pub fn with_live_party(&self, block: &[u8]) -> Option<SaveSnapshot> {
        if block.len() != PARTY_BLOCK_LEN {
            return None;
        }
        let count = u32::from_le_bytes([block[4], block[5], block[6], block[7]]) as usize;
        if !(1..=6).contains(&count) {
            return None;
        }
        let party = (0..count)
            .map(|i| {
                let o = 8 + i * PARTY_SIZE;
                pk5::parse(&block[o..o + PARTY_SIZE], Place::Party { slot: i as u8 })
            })
            .collect::<Option<Vec<_>>>()?;
        let in_party = |pid: u32| party.iter().any(|p| p.pid == pid);
        let boxes: Vec<Pokemon> = self
            .boxes
            .iter()
            .filter(|p| !in_party(p.pid))
            .cloned()
            .collect();
        let mut live = SaveSnapshot {
            trainer: self.trainer.clone(),
            party,
            boxes,
            encounters: self.encounters.clone(),
            fallen: self.fallen.clone(),
        };
        for old in &self.party {
            live.keep(old);
        }
        Some(live)
    }

    /// Lists a Pokémon that is known to be the player's but is in no place
    /// this snapshot knows, in the last box, until the next save says where
    /// it went. Does nothing if the player has it.
    pub fn keep(&mut self, pokemon: &Pokemon) {
        if self.owns(pokemon.pid) {
            return;
        }
        let last_box = BOX_COUNT as u8;
        let free = (0..BOX_SLOTS as u8).find(|&slot| {
            let place = Place::Box {
                r#box: last_box,
                slot,
            };
            !self.boxes.iter().any(|p| p.place == place)
        });
        if let Some(slot) = free {
            let mut kept = pokemon.clone();
            kept.place = Place::Box {
                r#box: last_box,
                slot,
            };
            kept.battle = None;
            self.boxes.push(kept);
        }
    }
}

/// Reads a save. `data` may be a `.sav` or `.dsv` file.
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
        id: u32::from_le_bytes([
            b[TRAINER_BLOCK + 0x14],
            b[TRAINER_BLOCK + 0x15],
            b[TRAINER_BLOCK + 0x16],
            b[TRAINER_BLOCK + 0x17],
        ]),
        badges: b[BADGE_FLAGS].count_ones() as u8,
        game: Game::from_save_version(b[TRAINER_BLOCK + GAME_VERSION]),
    };
    Ok(SaveSnapshot {
        trainer,
        party,
        boxes,
        encounters: Vec::new(),
        fallen: Vec::new(),
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
        s[TRAINER_BLOCK + 0x14..TRAINER_BLOCK + 0x18]
            .copy_from_slice(&0x1234_5678u32.to_le_bytes());
        s[BADGE_FLAGS] = 0b0000_0011;
        s[TRAINER_BLOCK + GAME_VERSION] = 23;
        s
    }

    #[test]
    fn reads_synthetic_save() {
        let snap = parse_b2w2(&synthetic_save()).unwrap();
        assert_eq!(
            snap.trainer,
            Trainer {
                name: "Hannes".into(),
                id: 0x1234_5678,
                badges: 2,
                game: Some(Game::Black2),
            }
        );
        assert_eq!(snap.party.len(), 2);
        assert!(snap.party[1].is_fainted());
        assert_eq!(snap.boxes.len(), 1);
        assert_eq!(snap.boxes[0].place, Place::Box { r#box: 3, slot: 4 });
        assert_eq!(snap.boxes[0].label(crate::Lang::En), "Trick");
        assert_eq!(snap.boxes[0].species_label(crate::Lang::De), "Zorua");
        assert_eq!(snap.all_pokemon().count(), 3);
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(parse_b2w2(&[0u8; 100]), Err(SaveError::TooSmall(100)));
        let mut s = vec![0u8; SAVE_SIZE];
        s[PARTY_BLOCK + 4] = 9;
        assert_eq!(parse_b2w2(&s), Err(SaveError::NotB2W2));
    }

    #[test]
    fn live_party_replaces_the_saved_one() {
        let save = synthetic_save();
        let snap = parse_b2w2(&save).unwrap();
        // In memory: Blubb went to the PC, Trick came out of its box and a
        // new catch joined.
        let mons = [
            build_raw(
                0x1111_2222,
                488,
                "Mondi",
                14,
                5,
                Some(&BattleStats {
                    level: 24,
                    hp: 0,
                    max_hp: 74,
                }),
            ),
            build_raw(
                0x5555_6666,
                570,
                "Trick",
                17,
                18,
                Some(&BattleStats {
                    level: 18,
                    hp: 40,
                    max_hp: 40,
                }),
            ),
            build_raw(
                0x7777_8888,
                519,
                "",
                20,
                12,
                Some(&BattleStats {
                    level: 12,
                    hp: 30,
                    max_hp: 30,
                }),
            ),
        ];
        let mut block = vec![0u8; PARTY_BLOCK_LEN];
        block[4] = mons.len() as u8;
        for (i, raw) in mons.iter().enumerate() {
            block[8 + i * PARTY_SIZE..8 + (i + 1) * PARTY_SIZE].copy_from_slice(raw);
        }
        let live = snap.with_live_party(&block).unwrap();
        assert_eq!(live.trainer, snap.trainer);
        assert_eq!(live.party.len(), 3);
        assert!(live.party[0].is_fainted());
        assert_eq!(live.party[1].place, Place::Party { slot: 1 });
        assert_eq!(live.boxes.len(), 1);
        assert_eq!(live.boxes[0].pid, 0x3333_4444);
        assert_eq!(live.boxes[0].place, Place::Box { r#box: 24, slot: 0 });
        assert_eq!(live.all_pokemon().count(), 4);

        // The block as the save has it changes nothing.
        let same = &save[PARTY_BLOCK..PARTY_BLOCK + PARTY_BLOCK_LEN];
        assert_eq!(snap.with_live_party(same), Some(snap.clone()));

        // Garbage is refused.
        block[8 + 20] ^= 0xFF;
        assert_eq!(snap.with_live_party(&block), None);
        assert_eq!(snap.with_live_party(&[0u8; PARTY_BLOCK_LEN]), None);
        assert_eq!(snap.with_live_party(&block[..100]), None);
    }
}
