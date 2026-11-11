//! Ereignisse aus dem Vergleich zweier Spielstände.
//!
//! Das ist die Grundlage für den Sync: Statt ganzer Spielstände schickt
//! SoulSync später nur diese Ereignisse an den Run-Server.

use crate::pk5::{Place, Pokemon};
use crate::save::SaveSnapshot;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Ein Pokémon ist neu im Spielstand (gefangen, geschenkt, geschlüpft).
    Caught { pokemon: Pokemon },
    /// Ein Team-Pokémon ist auf 0 KP gefallen.
    Fainted { pid: u32, name: String },
    /// Ein Pokémon hat sich entwickelt.
    Evolved { pid: u32, from: u16, to: u16 },
    /// Ein Team-Pokémon ist im Level gestiegen.
    LevelUp { pid: u32, name: String, level: u8 },
    /// Ein Pokémon ist zwischen Team und Box gewechselt.
    Moved { pid: u32, name: String, to: Place },
    /// Ein Pokémon ist nicht mehr im Spielstand (freigelassen, getauscht).
    Gone { pid: u32, name: String },
    /// Neue Orden.
    Badges { count: u8 },
}

fn label(p: &Pokemon) -> String {
    if p.is_nicknamed && !p.nickname.is_empty() {
        p.nickname.clone()
    } else {
        p.species_de
            .clone()
            .unwrap_or_else(|| format!("#{}", p.species))
    }
}

fn is_party(p: &Pokemon) -> bool {
    matches!(p.place, Place::Party { .. })
}

/// Vergleicht zwei Spielstände desselben Spielers. Pokémon werden über ihre
/// PID wiedererkannt.
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
                pid: p.pid,
                from: o.species,
                to: p.species,
            });
        }
        if is_party(o) != is_party(p) {
            events.push(Event::Moved {
                pid: p.pid,
                name: label(p),
                to: p.place,
            });
        }
        if let (Some(a), Some(b)) = (&o.battle, &p.battle) {
            if b.level > a.level {
                events.push(Event::LevelUp {
                    pid: p.pid,
                    name: label(p),
                    level: b.level,
                });
            }
            if a.hp > 0 && b.hp == 0 {
                events.push(Event::Fainted {
                    pid: p.pid,
                    name: label(p),
                });
            }
        }
    }
    for o in old.all_pokemon() {
        if !after.contains_key(&o.pid) {
            events.push(Event::Gone {
                pid: o.pid,
                name: label(o),
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
                badges,
            },
            party,
            boxes,
        }
    }

    #[test]
    fn detects_changes() {
        let p0 = Place::Party { slot: 0 };
        let p1 = Place::Party { slot: 1 };
        let bx = Place::Box { r#box: 1, slot: 0 };
        let old = snap(
            1,
            vec![mon(1, 570, 10, 30, p0), mon(2, 25, 12, 20, p1)],
            vec![mon(3, 1, 0, 0, bx)],
        );
        let new = snap(
            2,
            vec![mon(1, 571, 30, 30, p0), mon(2, 25, 12, 0, p1)],
            vec![mon(4, 60, 0, 0, bx)],
        );
        let ev = diff(&old, &new);
        assert!(ev.contains(&Event::Evolved {
            pid: 1,
            from: 570,
            to: 571
        }));
        assert!(ev.contains(&Event::LevelUp {
            pid: 1,
            name: "Zoroark".into(),
            level: 30
        }));
        assert!(ev.contains(&Event::Fainted {
            pid: 2,
            name: "Pikachu".into()
        }));
        assert!(ev
            .iter()
            .any(|e| matches!(e, Event::Caught { pokemon } if pokemon.pid == 4)));
        assert!(ev.contains(&Event::Gone {
            pid: 3,
            name: "Bisasam".into()
        }));
        assert!(ev.contains(&Event::Badges { count: 2 }));
        assert!(diff(&new, &new).is_empty());
    }
}
