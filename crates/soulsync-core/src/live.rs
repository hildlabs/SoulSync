//! What the SoulSync Lua script reports from a running emulator.
//!
//! The script writes one file: the magic `SSL3`, then [`WINDOW`] bytes of the
//! game's memory starting at the party block, then any number of Pokémon that
//! turned up in memory while it ran and are not in the party. Those are
//! mostly the opponents of battles, wild and trainers'. Each is one byte, 1
//! while the Pokémon is still where it was found, and its 220 bytes as they
//! are now, or were last.
//!
//! The memory after the party block holds the trainer and the player's
//! position, as in the save file. Where exactly is checked against the save,
//! not assumed.

use crate::crypto::PARTY_SIZE;
use crate::dex;
use crate::pk5::{self, Pokemon};
use crate::save::{SaveSnapshot, Trainer, PARTY_BLOCK_LEN};
use serde::{Deserialize, Serialize};

const MAGIC: &[u8] = b"SSL3";
/// A reported Pokémon: a flag, then the Pokémon.
const ENTRY: usize = 1 + PARTY_SIZE;
/// Bytes of memory the script copies, starting at the party block.
pub const WINDOW: usize = 0x800;
/// Where the trainer block follows the party block in a save file.
const TRAINER_IN_SAVE: usize = 0x600;
/// Size of the trainer block.
const TRAINER_LEN: usize = 0xB0;
/// Where the report has the map the player is on. The script puts it there:
/// the save data's own maps do not keep up, the one at this place changes
/// only with doors and the one at 0x80 only when the game is saved. 0xFFFF
/// while the script does not know the map.
const MAP: usize = 0x04;
/// Remembered PIDs, so that a Pokémon is judged once.
const SEEN_LIMIT: usize = 2000;

/// A trainer's name as the game keeps it at the start of the trainer block,
/// 4 bytes in: UTF-16 with an end mark, 16 bytes at most.
fn trainer_name(trainer: &Trainer) -> Vec<u8> {
    let mut name: Vec<u8> = trainer
        .name
        .encode_utf16()
        .chain([0xFFFF])
        .flat_map(u16::to_le_bytes)
        .collect();
    name.truncate(0x10);
    name
}

/// What the app tells the Lua script about the trainer whose party to look
/// for: the ID as a decimal number, then the name's bytes in hex. The script
/// and [`LiveReport::is_of`] look for the same thing, so a party the script
/// settles on is one the app accepts.
pub fn trainer_note(trainer: &Trainer) -> String {
    let name: String = trainer_name(trainer)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{}\n{name}\n", trainer.id)
}

/// One report of the Lua script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveReport<'a> {
    window: &'a [u8],
    /// Pokémon that appeared in memory, newest last.
    pub seen: Vec<&'a [u8]>,
    /// For each of them, whether it is still in memory.
    present: Vec<bool>,
}

impl<'a> LiveReport<'a> {
    /// Reads a report. `None` if the file is not one or was cut off.
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        let rest = data.strip_prefix(MAGIC)?;
        if rest.len() < WINDOW || (rest.len() - WINDOW) % ENTRY != 0 {
            return None;
        }
        let (window, seen) = rest.split_at(WINDOW);
        Some(LiveReport {
            window,
            seen: seen.chunks(ENTRY).map(|e| &e[1..]).collect(),
            present: seen.chunks(ENTRY).map(|e| e[0] == 1).collect(),
        })
    }

    /// The wild Pokémon the player is facing: the newest one still in memory
    /// that is not theirs and has HP left.
    pub fn wild(&self, snapshot: &SaveSnapshot) -> Option<Pokemon> {
        let newest = self.seen.iter().zip(&self.present).rev();
        newest
            .filter(|(_, present)| **present)
            .find_map(|(raw, _)| pk5::parse_wild(raw))
            .filter(|w| !snapshot.owns(w.pid) && !w.is_fainted())
    }

    /// The party block, for [`SaveSnapshot::with_live_party`].
    pub fn party(&self) -> &'a [u8] {
        &self.window[..PARTY_BLOCK_LEN]
    }

    /// Where the trainer block of `trainer` follows the party in the window.
    fn trainer_at(&self, trainer: &Trainer) -> Option<usize> {
        let name = trainer_name(trainer);
        let id = trainer.id.to_le_bytes();
        let w = self.window;
        (PARTY_BLOCK_LEN..w.len().saturating_sub(0x18))
            .step_by(4)
            .find(|&t| w[t + 4..].starts_with(&name) && w[t + 0x14..t + 0x18] == id)
    }

    /// Whether this is the party of `trainer`. An opponent's party looks the
    /// same in memory, but only the player's own is followed by their
    /// trainer data.
    pub fn is_of(&self, trainer: &Trainer) -> bool {
        self.trainer_at(trainer).is_some()
    }

    /// The map the player is on. `None` unless the trainer of the save is
    /// found behind the party, which shows that the memory is laid out as
    /// expected.
    pub fn map(&self, trainer: &Trainer) -> Option<u16> {
        let w = self.window;
        let at = self.trainer_at(trainer)?;
        // In a save file blocks start at multiples of 0x100.
        let position = if at == TRAINER_IN_SAVE {
            at + 0x100
        } else {
            at + TRAINER_LEN
        };
        let map = w.get(position + MAP..position + MAP + 2)?;
        Some(u16::from_le_bytes([map[0], map[1]]))
    }
}

/// What a save file does not tell, kept by the player's client across
/// restarts: the first wild Pokémon met in each location, which Pokémon
/// fainted, and who was in the party since the last save.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Encounters {
    /// The game they belong to; see [`Trainer::id`].
    trainer: u32,
    /// PIDs already judged.
    seen: Vec<u32>,
    list: Vec<Pokemon>,
    /// Pokémon that fainted, as they were then.
    #[serde(default)]
    fallen: Vec<Pokemon>,
    /// The party as last seen live. Whoever leaves it before the next save
    /// went to a box the save does not show yet.
    #[serde(default)]
    unsaved: Vec<Pokemon>,
}

impl Encounters {
    /// The encounters of the game `trainer` plays. A different game starts
    /// with none.
    pub fn of(&mut self, trainer: &Trainer) -> &[Pokemon] {
        if self.trainer != trainer.id {
            *self = Encounters {
                trainer: trainer.id,
                ..Encounters::default()
            };
        }
        &self.list
    }

    /// Takes in the Pokémon of a report. `snapshot` is the player's game
    /// right now and `location` where they are, if known. Returns whether
    /// anything changed.
    ///
    /// A wild Pokémon counts if it is the first in its location: no other
    /// encounter there and no Pokémon from there. Trainers' Pokémon never
    /// count.
    pub fn observe(
        &mut self,
        snapshot: &SaveSnapshot,
        seen: &[&[u8]],
        location: Option<u16>,
    ) -> bool {
        self.of(&snapshot.trainer);
        let location = location.filter(|&l| dex::location(l).is_some());
        let mut changed = false;
        for raw in seen {
            let Some(pid) = raw.get(..4) else { continue };
            let pid = u32::from_le_bytes([pid[0], pid[1], pid[2], pid[3]]);
            if self.seen.contains(&pid) || snapshot.owns(pid) {
                continue;
            }
            let wild = pk5::parse_wild(raw);
            // Without a location a wild Pokémon waits for the next report.
            let (Some(location), Some(mut wild)) = (location, wild.clone()) else {
                if wild.is_none() {
                    self.remember(pid);
                    changed = true;
                }
                continue;
            };
            self.remember(pid);
            changed = true;
            let taken = self.list.iter().any(|p| p.met_location == location)
                || snapshot.all_pokemon().any(|p| p.met_location == location);
            if !taken {
                wild.met_location = location;
                wild.met_location_name = dex::location(location).cloned();
                self.list.push(wild);
            }
        }
        changed
    }

    /// Whether the Pokémon with this PID has been judged, and whether it
    /// counts as an encounter.
    pub fn knows(&self, pid: u32) -> (bool, bool) {
        (
            self.seen.contains(&pid),
            self.list.iter().any(|p| p.pid == pid),
        )
    }

    /// Takes back an encounter that was noted by mistake. Returns whether
    /// there was one. The Pokémon is not noted again.
    pub fn forget(&mut self, pid: u32) -> bool {
        let before = self.list.len();
        self.list.retain(|p| p.pid != pid);
        if !self.seen.contains(&pid) {
            self.remember(pid);
        }
        self.list.len() != before
    }

    /// The game was saved: the save now knows where every Pokémon is.
    pub fn saved(&mut self) {
        self.unsaved.clear();
    }

    /// Completes a snapshot with what the save does not tell. `live` says
    /// that its party comes from the emulator's memory. Returns whether
    /// there is something new to keep.
    pub fn complete(&mut self, snapshot: &mut SaveSnapshot, live: bool) -> bool {
        self.of(&snapshot.trainer);
        let mut changed = false;
        if live {
            for p in &snapshot.party {
                match self.unsaved.iter_mut().find(|u| u.pid == p.pid) {
                    Some(u) if u == p => {}
                    Some(u) => {
                        *u = p.clone();
                        changed = true;
                    }
                    None => {
                        self.unsaved.push(p.clone());
                        changed = true;
                    }
                }
            }
            for p in &self.unsaved {
                snapshot.keep(p);
            }
        }
        if snapshot.only_starter() {
            // Before the first catch a faint does not count, and one that was
            // noted before this rule existed is forgotten.
            let before = self.fallen.len();
            self.fallen.retain(|f| !snapshot.owns(f.pid));
            changed |= self.fallen.len() != before;
        } else {
            for p in snapshot.party.iter().filter(|p| p.is_fainted()) {
                if !self.fallen.iter().any(|f| f.pid == p.pid) {
                    self.fallen.push(p.clone());
                    changed = true;
                }
            }
        }
        snapshot.fallen = self.fallen.clone();
        snapshot.encounters = self.list.clone();
        changed
    }

    fn remember(&mut self, pid: u32) {
        self.seen.push(pid);
        if self.seen.len() > SEEN_LIMIT {
            self.seen.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pk5::{build_raw, build_raw_ivs, BattleStats, Place};

    const STATS: BattleStats = BattleStats {
        level: 4,
        hp: 15,
        max_hp: 15,
    };
    /// Six different IVs, as a wild Pokémon has.
    const WILD: u32 = 0x1234_5678 & 0x3FFF_FFFF;

    fn trainer() -> Trainer {
        Trainer {
            name: "Hannes".into(),
            id: 0xCAFE_1234,
            badges: 0,
            game: None,
        }
    }

    fn snapshot(party: Vec<Pokemon>) -> SaveSnapshot {
        SaveSnapshot {
            trainer: trainer(),
            party,
            boxes: vec![],
            encounters: vec![],
            fallen: vec![],
            wild: None,
        }
    }

    /// A report whose trainer block sits `at` bytes after the party.
    fn report(at: usize, map: u16, seen: &[Vec<u8>]) -> Vec<u8> {
        let mut w = vec![0u8; WINDOW];
        w[4] = 1;
        w[8..8 + PARTY_SIZE].copy_from_slice(&build_raw(1, 495, "", 4, 5, Some(&STATS)));
        for (i, u) in "Hannes".encode_utf16().chain([0xFFFF]).enumerate() {
            w[at + 4 + i * 2..at + 6 + i * 2].copy_from_slice(&u.to_le_bytes());
        }
        w[at + 0x14..at + 0x18].copy_from_slice(&0xCAFE_1234u32.to_le_bytes());
        let position = if at == 0x600 { 0x700 } else { at + 0xB0 };
        w[position + MAP..position + MAP + 2].copy_from_slice(&map.to_le_bytes());
        let mut data = MAGIC.to_vec();
        data.extend(w);
        for raw in seen {
            data.push(1);
            data.extend(raw);
        }
        data
    }

    #[test]
    fn reads_reports() {
        let wild = build_raw_ivs(9, 504, "", 0, 0, Some(&STATS), WILD);
        let data = report(0x600, 427, std::slice::from_ref(&wild));
        let r = LiveReport::parse(&data).unwrap();
        assert_eq!(r.party().len(), PARTY_BLOCK_LEN);
        assert_eq!(r.seen, vec![&wild[..]]);
        assert_eq!(r.map(&trainer()), Some(427));
        // The battle: who the player faces, until it is over.
        assert_eq!(r.wild(&snapshot(vec![])).unwrap().species, 504);
        let mut over = data.clone();
        over[4 + WINDOW] = 0;
        assert_eq!(
            LiveReport::parse(&over).unwrap().wild(&snapshot(vec![])),
            None
        );
        // Blocks packed without the gaps of the save file.
        let packed = report(0x534, 12, &[]);
        assert_eq!(
            LiveReport::parse(&packed).unwrap().map(&trainer()),
            Some(12)
        );
        // Another trainer's memory, cut-off files, other files.
        let other = Trainer { id: 5, ..trainer() };
        assert_eq!(r.map(&other), None);
        assert_eq!(LiveReport::parse(&data[..data.len() - 1]), None);
        assert_eq!(LiveReport::parse(&data[4..]), None);
    }

    #[test]
    fn keeps_the_first_wild_pokemon_per_location() {
        let own = pk5::parse(
            &build_raw(1, 495, "", 4, 5, Some(&STATS)),
            Place::Party { slot: 0 },
        )
        .unwrap();
        let snap = snapshot(vec![own]);
        let wild = |pid, species| build_raw_ivs(pid, species, "", 0, 0, Some(&STATS), WILD);
        let (a, b, c) = (wild(10, 504), wild(11, 506), wild(12, 519));
        let foe = build_raw_ivs(13, 509, "", 0, 0, Some(&STATS), 0x3FFF_FFFF);
        let mut e = Encounters::default();

        // Where the player is, is not known yet: the wild one waits, the
        // trainer's Pokémon is settled.
        assert!(e.observe(&snap, &[&a, &foe], None));
        assert!(e.of(&trainer()).is_empty());
        assert!(e.observe(&snap, &[&a, &foe], Some(14)));
        assert_eq!(e.of(&trainer()).len(), 1);
        assert_eq!(e.of(&trainer())[0].species, 504);
        assert_eq!(e.of(&trainer())[0].met_location, 14);
        // Seen again, and a second one on the same route: nothing new.
        assert!(!e.observe(&snap, &[&a], Some(15)));
        assert!(e.observe(&snap, &[&a, &b], Some(14)));
        assert_eq!(e.of(&trainer()).len(), 1);
        // Location 4 already has the starter; the player's own Pokémon never count.
        assert!(e.observe(&snap, &[&c], Some(4)));
        let starter = build_raw(1, 495, "", 4, 5, Some(&STATS));
        assert!(!e.observe(&snap, &[&starter], Some(15)));
        assert_eq!(e.of(&trainer()).len(), 1);

        // Fainted stays fainted: healed, boxed after a live catch, released.
        let mon = |pid, hp| {
            let stats = BattleStats {
                level: 8,
                hp,
                max_hp: 20,
            };
            pk5::parse(
                &build_raw(pid, 427, "", 15, 5, Some(&stats)),
                Place::Party { slot: 1 },
            )
            .unwrap()
        };
        let mut live = snapshot(vec![snap.party[0].clone(), mon(30, 0)]);
        assert!(e.complete(&mut live, true));
        assert_eq!(live.fallen.len(), 1);
        assert_eq!(live.encounters.len(), 1);
        let mut healed = snapshot(vec![snap.party[0].clone(), mon(30, 20)]);
        e.complete(&mut healed, true);
        assert_eq!(healed.fallen[0].pid, 30);
        // Put into a box the save does not show yet: still the player's.
        let mut boxed = snapshot(vec![snap.party[0].clone()]);
        e.complete(&mut boxed, true);
        assert!(boxed.owns(30));
        assert_eq!(boxed.boxes[0].place, Place::Box { r#box: 24, slot: 0 });
        // After a save the save decides who is there.
        e.saved();
        let mut saved = snapshot(vec![snap.party[0].clone()]);
        e.complete(&mut saved, false);
        assert!(!saved.owns(30));
        assert_eq!(saved.fallen[0].pid, 30);

        // A faint noted while the starter was all there was is forgotten.
        let mut alone = Encounters::default();
        alone.of(&trainer());
        alone.fallen = vec![mon(30, 0)];
        let mut first = snapshot(vec![mon(30, 0)]);
        assert!(alone.complete(&mut first, true));
        assert!(first.fallen.is_empty());

        // A new game starts over.
        let again = Trainer {
            id: 77,
            ..trainer()
        };
        assert!(e.of(&again).is_empty());
        let mut fresh = SaveSnapshot {
            trainer: again,
            ..snapshot(vec![])
        };
        e.complete(&mut fresh, false);
        assert!(fresh.fallen.is_empty());
    }
}
