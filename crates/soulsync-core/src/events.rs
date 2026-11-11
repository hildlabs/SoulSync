//! Events derived from comparing two snapshots of the same player's save.
//!
//! This is the basis for syncing: instead of whole saves, SoulSync will send
//! these events to the run server. Each event carries the Pokémon as it is
//! now (or was last seen, for `Gone`), so clients can show it in any language.

use crate::pk5::{Place, Pokemon};
use crate::save::SaveSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// A Pokémon appeared (caught, gifted, hatched).
    Caught { pokemon: Pokemon },
    /// A party Pokémon dropped to 0 HP.
    Fainted { pokemon: Pokemon },
    /// A Pokémon evolved from species `from`.
    Evolved { pokemon: Pokemon, from: u16 },
    /// A party Pokémon gained one or more levels.
    LevelUp { pokemon: Pokemon },
    /// A Pokémon moved between party and box.
    Moved { pokemon: Pokemon },
    /// A Pokémon is no longer in the save (released, traded away).
    Gone { pokemon: Pokemon },
    /// A wild Pokémon was the first the player met in its location. Whether
    /// it gets caught shows later.
    Encountered { pokemon: Pokemon },
    /// The badge count went up.
    Badges { count: u8 },
}

fn is_party(p: &Pokemon) -> bool {
    matches!(p.place, Place::Party { .. })
}

/// Compares two snapshots of the same player's save. Pokémon are matched by PID.
pub fn diff(old: &SaveSnapshot, new: &SaveSnapshot) -> Vec<Event> {
    let before: HashMap<u32, &Pokemon> = old.all_pokemon().map(|p| (p.pid, p)).collect();
    let after: HashMap<u32, &Pokemon> = new.all_pokemon().map(|p| (p.pid, p)).collect();
    let mut events = Vec::new();

    for p in new.all_pokemon() {
        let Some(o) = before.get(&p.pid) else {
            events.push(Event::Caught { pokemon: p.clone() });
            continue;
        };
        if o.species != p.species {
            events.push(Event::Evolved {
                pokemon: p.clone(),
                from: o.species,
            });
        }
        if is_party(o) != is_party(p) {
            events.push(Event::Moved { pokemon: p.clone() });
        }
        if let (Some(a), Some(b)) = (&o.battle, &p.battle) {
            if b.level > a.level {
                events.push(Event::LevelUp { pokemon: p.clone() });
            }
            if a.hp > 0 && b.hp == 0 {
                events.push(Event::Fainted { pokemon: p.clone() });
            }
        }
    }
    for o in old.all_pokemon() {
        if !after.contains_key(&o.pid) {
            events.push(Event::Gone { pokemon: o.clone() });
        }
    }
    for wild in &new.encounters {
        if !old.encounters.iter().any(|w| w.pid == wild.pid) && !new.owns(wild.pid) {
            events.push(Event::Encountered {
                pokemon: wild.clone(),
            });
        }
    }
    if new.trainer.badges > old.trainer.badges {
        events.push(Event::Badges {
            count: new.trainer.badges,
        });
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pk5::{build_raw, parse, BattleStats};
    use crate::save::Trainer;

    fn mon(pid: u32, species: u16, level: u8, hp: u16, place: Place) -> Pokemon {
        let battle = matches!(place, Place::Party { .. }).then_some(BattleStats {
            level,
            hp,
            max_hp: 50,
        });
        parse(&build_raw(pid, species, "", 14, 5, battle.as_ref()), place).unwrap()
    }

    fn snap(badges: u8, party: Vec<Pokemon>, boxes: Vec<Pokemon>) -> SaveSnapshot {
        SaveSnapshot {
            trainer: Trainer {
                name: "Hannes".into(),
                id: 0,
                badges,
                game: None,
            },
            party,
            boxes,
            encounters: vec![],
            fallen: vec![],
            wild: None,
        }
    }

    fn kinds(ev: &[Event]) -> Vec<(&'static str, u32)> {
        ev.iter()
            .map(|e| match e {
                Event::Caught { pokemon } => ("caught", pokemon.pid),
                Event::Fainted { pokemon } => ("fainted", pokemon.pid),
                Event::Evolved { pokemon, .. } => ("evolved", pokemon.pid),
                Event::LevelUp { pokemon } => ("level_up", pokemon.pid),
                Event::Moved { pokemon } => ("moved", pokemon.pid),
                Event::Gone { pokemon } => ("gone", pokemon.pid),
                Event::Encountered { pokemon } => ("encountered", pokemon.pid),
                Event::Badges { count } => ("badges", *count as u32),
            })
            .collect()
    }

    #[test]
    fn detects_changes() {
        let p0 = Place::Party { slot: 0 };
        let p1 = Place::Party { slot: 1 };
        let bx = Place::Box { r#box: 1, slot: 0 };
        let old = snap(
            1,
            vec![mon(1, 570, 10, 30, p0), mon(2, 25, 12, 20, p1)],
            vec![mon(3, 1, 0, 0, bx), mon(5, 4, 0, 0, bx)],
        );
        let new = snap(
            2,
            vec![
                mon(1, 571, 30, 30, p0),
                mon(2, 25, 12, 0, p1),
                mon(5, 4, 9, 30, Place::Party { slot: 2 }),
            ],
            vec![mon(4, 60, 0, 0, bx)],
        );
        let ev = diff(&old, &new);
        let k = kinds(&ev);
        for want in [
            ("evolved", 1),
            ("level_up", 1),
            ("fainted", 2),
            ("moved", 5),
            ("caught", 4),
            ("gone", 3),
            ("badges", 2),
        ] {
            assert!(k.contains(&want), "missing {want:?} in {k:?}");
        }
        assert!(ev.contains(&Event::Evolved {
            pokemon: new.party[0].clone(),
            from: 570
        }));
        assert!(diff(&new, &new).is_empty());
    }
}
