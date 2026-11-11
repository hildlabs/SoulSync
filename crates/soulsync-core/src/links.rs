//! Soul Link pairs: the two players' Pokémon from the same location.

use crate::dex;
use crate::lang::Names;
use crate::pk5::Pokemon;
use crate::save::SaveSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What happened to one side of a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// In the party and able to fight.
    Party,
    /// In a PC box.
    Boxed,
    /// Fainted, now or earlier. Healing does not bring a link back. A starter
    /// that faints before the player's first catch is not counted.
    Fainted,
    /// The first wild Pokémon of the location, met but not caught. The
    /// location is used up for both players.
    Missed,
}

/// One player's side of a link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Side {
    pub pokemon: Pokemon,
    pub status: Status,
}

/// A location and the Pokémon each player got there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub location: u16,
    pub location_name: Option<Names>,
    /// Indexed like the players passed to [`links`].
    pub sides: Vec<Option<Side>>,
}

impl Link {
    /// Both players have a Pokémon here.
    pub fn is_complete(&self) -> bool {
        self.sides.iter().all(Option::is_some)
    }

    /// A side fainted or missed its encounter, so the whole link is dead.
    pub fn is_dead(&self) -> bool {
        self.sides
            .iter()
            .flatten()
            .any(|s| matches!(s.status, Status::Fainted | Status::Missed))
    }
}

fn status(p: &Pokemon, fallen: &[Pokemon], excused: bool) -> Status {
    if !excused && (p.is_fainted() || fallen.iter().any(|f| f.pid == p.pid)) {
        Status::Fainted
    } else if p.battle.is_some() {
        Status::Party
    } else {
        Status::Boxed
    }
}

/// Pairs up the players' Pokémon by met location. Per player and location only
/// the first Pokémon counts (party before boxes); eggs are ignored. Links are
/// sorted by location id. Players without a save yet are passed as `None`.
///
/// A player without a Pokémon from a location where they met a wild one has
/// missed that encounter. Neither a faint nor a missed encounter counts
/// while a player has only their starter; see [`SaveSnapshot::only_starter`].
pub fn links(players: &[Option<&SaveSnapshot>]) -> Vec<Link> {
    let mut by_loc: BTreeMap<u16, Vec<Option<Side>>> = BTreeMap::new();
    for (i, snap) in players.iter().enumerate() {
        let Some(snap) = snap else { continue };
        for p in snap.all_pokemon() {
            let sides = by_loc
                .entry(p.met_location)
                .or_insert_with(|| vec![None; players.len()]);
            if sides[i].is_none() {
                sides[i] = Some(Side {
                    pokemon: p.clone(),
                    status: status(p, &snap.fallen, snap.only_starter()),
                });
            }
        }
        // Fainted and then released: the link stays lost.
        for gone in snap.fallen.iter().filter(|f| !snap.owns(f.pid)) {
            let sides = by_loc
                .entry(gone.met_location)
                .or_insert_with(|| vec![None; players.len()]);
            if sides[i].is_none() {
                sides[i] = Some(Side {
                    pokemon: gone.clone(),
                    status: Status::Fainted,
                });
            }
        }
        let begun = !snap.only_starter();
        for wild in snap
            .encounters
            .iter()
            .filter(|w| begun && !snap.owns(w.pid))
        {
            let sides = by_loc
                .entry(wild.met_location)
                .or_insert_with(|| vec![None; players.len()]);
            if sides[i].is_none() {
                sides[i] = Some(Side {
                    pokemon: wild.clone(),
                    status: Status::Missed,
                });
            }
        }
    }
    by_loc
        .into_iter()
        .map(|(location, sides)| Link {
            location,
            location_name: dex::location(location).cloned(),
            sides,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pk5::{build_raw, parse, BattleStats, Place};
    use crate::save::Trainer;

    fn party(pid: u32, species: u16, loc: u16, hp: u16) -> Pokemon {
        let b = BattleStats {
            level: 10,
            hp,
            max_hp: 30,
        };
        parse(
            &build_raw(pid, species, "", loc, 5, Some(&b)),
            Place::Party { slot: 0 },
        )
        .unwrap()
    }

    fn boxed(pid: u32, species: u16, loc: u16) -> Pokemon {
        parse(
            &build_raw(pid, species, "", loc, 5, None),
            Place::Box { r#box: 1, slot: 0 },
        )
        .unwrap()
    }

    fn snap(party: Vec<Pokemon>, boxes: Vec<Pokemon>) -> SaveSnapshot {
        SaveSnapshot {
            trainer: Trainer {
                name: "x".into(),
                id: 0,
                badges: 0,
                game: None,
            },
            party,
            boxes,
            encounters: vec![],
            fallen: vec![],
            wild: None,
        }
    }

    #[test]
    fn pairs_by_location() {
        let a = snap(
            vec![party(1, 375, 117, 20), party(2, 25, 14, 0)],
            vec![boxed(3, 1, 15), boxed(4, 4, 15)],
        );
        let b = snap(vec![party(10, 60, 117, 20)], vec![boxed(11, 7, 14)]);
        let l = links(&[Some(&a), Some(&b)]);
        assert_eq!(
            l.iter().map(|l| l.location).collect::<Vec<_>>(),
            vec![14, 15, 117]
        );

        let r1 = &l[0];
        assert!(r1.is_complete() && r1.is_dead());
        assert_eq!(r1.sides[1].as_ref().unwrap().status, Status::Boxed);

        let r2 = &l[1];
        assert!(!r2.is_complete());
        assert_eq!(
            r2.sides[0].as_ref().unwrap().pokemon.pid,
            3,
            "first Pokémon per location counts"
        );

        let start = &l[2];
        assert!(start.is_complete() && !start.is_dead());
        assert_eq!(start.location_name.as_ref().unwrap().en, "Aspertia City");
    }

    #[test]
    fn missing_player() {
        let a = snap(vec![party(1, 375, 117, 20)], vec![]);
        let l = links(&[Some(&a), None]);
        assert_eq!(l.len(), 1);
        assert!(l[0].sides[1].is_none());
    }

    #[test]
    fn fainted_stays_fainted() {
        let fell = party(1, 427, 15, 0);
        // Healed at a Pokémon Center.
        let mut healed = snap(vec![party(1, 427, 15, 30), party(2, 495, 4, 30)], vec![]);
        healed.fallen = vec![fell.clone()];
        let l = links(&[Some(&healed), None]);
        assert_eq!(l[1].sides[0].as_ref().unwrap().status, Status::Fainted);
        assert!(l[1].is_dead() && !l[0].is_dead());
        // Released afterwards: the location is not open again.
        let mut released = snap(vec![party(2, 495, 4, 30)], vec![]);
        released.fallen = vec![fell];
        let l = links(&[Some(&released), None]);
        assert_eq!(l.len(), 2);
        assert_eq!(l[1].location, 15);
        assert_eq!(l[1].sides[0].as_ref().unwrap().status, Status::Fainted);
    }

    #[test]
    fn the_starter_may_faint_before_the_first_catch() {
        // Lost the first fight against the rival, or noted as fallen earlier.
        let mut first = snap(vec![party(1, 495, 4, 0)], vec![]);
        first.fallen = vec![party(1, 495, 4, 0)];
        let l = links(&[Some(&first), None]);
        assert_eq!(l[0].sides[0].as_ref().unwrap().status, Status::Party);
        assert!(!l[0].is_dead());
        // Nor is a wild Pokémon missed, like the one of the catching lesson.
        first.encounters = vec![party(9, 504, 14, 12)];
        assert_eq!(links(&[Some(&first), None]).len(), 1);
        // With a second Pokémon the run has begun.
        let begun = snap(vec![party(1, 495, 4, 0), party(2, 504, 14, 12)], vec![]);
        let l = links(&[Some(&begun), None]);
        assert_eq!(l[0].sides[0].as_ref().unwrap().status, Status::Fainted);
    }

    #[test]
    fn a_missed_encounter_uses_the_location_up() {
        let mut a = snap(vec![party(2, 495, 4, 30), party(4, 519, 4, 30)], vec![]);
        a.encounters = vec![party(9, 504, 14, 12), party(2, 495, 4, 30)];
        let b = snap(vec![party(3, 498, 14, 30)], vec![]);
        let l = links(&[Some(&a), Some(&b)]);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].sides[0].as_ref().unwrap().status, Status::Party);
        assert_eq!(l[1].sides[0].as_ref().unwrap().status, Status::Missed);
        assert!(l[1].is_dead());
    }
}
