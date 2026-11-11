//! Species names, types and location names (Black 2 / White 2).

use crate::lang::Names;
use std::sync::OnceLock;

/// A Pokédex entry (National Dex 1..=649).
#[derive(Debug, Clone)]
pub struct Species {
    pub id: u16,
    pub name: Names,
    pub types: Vec<Type>,
    /// Shared id of an evolution family.
    pub family: u16,
}

/// The 17 types of generation 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    Normal,
    Fighting,
    Flying,
    Poison,
    Ground,
    Rock,
    Bug,
    Ghost,
    Steel,
    Fire,
    Water,
    Grass,
    Electric,
    Psychic,
    Ice,
    Dragon,
    Dark,
}

impl Type {
    const ALL: [Type; 17] = [
        Type::Normal,
        Type::Fighting,
        Type::Flying,
        Type::Poison,
        Type::Ground,
        Type::Rock,
        Type::Bug,
        Type::Ghost,
        Type::Steel,
        Type::Fire,
        Type::Water,
        Type::Grass,
        Type::Electric,
        Type::Psychic,
        Type::Ice,
        Type::Dragon,
        Type::Dark,
    ];

    fn from_index(i: i64) -> Option<Type> {
        usize::try_from(i)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
    }
}

fn species_table() -> &'static [Species] {
    static T: OnceLock<Vec<Species>> = OnceLock::new();
    T.get_or_init(|| {
        let raw: Vec<(String, String, i64, i64, u16)> =
            serde_json::from_str(include_str!("../data/species.json")).expect("species.json");
        raw.into_iter()
            .zip(1u16..)
            .map(|((en, de, t1, t2, family), id)| Species {
                id,
                name: Names { en, de },
                types: [t1, t2].into_iter().filter_map(Type::from_index).collect(),
                family,
            })
            .collect()
    })
}

fn location_table() -> &'static [Names] {
    static T: OnceLock<Vec<Names>> = OnceLock::new();
    T.get_or_init(|| {
        let raw: Vec<(String, String)> =
            serde_json::from_str(include_str!("../data/locations.json")).expect("locations.json");
        raw.into_iter().map(|(en, de)| Names { en, de }).collect()
    })
}

/// Species by National Dex number.
pub fn species(id: u16) -> Option<&'static Species> {
    id.checked_sub(1)
        .and_then(|i| species_table().get(i as usize))
}

/// Name of a met location or map. Special locations (link trade, egg,
/// event: 30000 and up) and unknown ids give `None`.
pub fn location(id: u16) -> Option<&'static Names> {
    location_table()
        .get(id as usize)
        .filter(|n| !n.en.starts_with('－') && !n.en.contains('\\'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_lookup() {
        let s = species(1).unwrap();
        assert_eq!(s.name.en, "Bulbasaur");
        assert_eq!(s.name.de, "Bisasam");
        assert_eq!(s.types, vec![Type::Grass, Type::Poison]);
        assert_eq!(species(649).unwrap().name.en, "Genesect");
        assert!(species(0).is_none());
        assert!(species(650).is_none());
    }

    #[test]
    fn location_lookup() {
        assert_eq!(location(14).unwrap().en, "Route 1");
        let aspertia = location(117).unwrap();
        assert_eq!(
            (aspertia.en.as_str(), aspertia.de.as_str()),
            ("Aspertia City", "Eventura City")
        );
        assert!(location(0).is_none());
        assert!(location(3).is_none());
        assert!(location(30001).is_none());
    }
}
