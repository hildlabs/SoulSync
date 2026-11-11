//! Artnamen, Typen und Ortsnamen (Schwarz 2 / Weiß 2).

use std::sync::OnceLock;

/// Ein Eintrag im Pokédex (nationale Nummer 1..=649).
#[derive(Debug, Clone)]
pub struct Species {
    pub id: u16,
    pub name_de: String,
    pub name_en: String,
    pub types: Vec<Type>,
    /// Gemeinsame Kennung einer Entwicklungsreihe.
    pub family: u16,
}

/// Die 17 Typen der fünften Generation.
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

    /// Deutscher Typname.
    pub fn name_de(self) -> &'static str {
        [
            "Normal", "Kampf", "Flug", "Gift", "Boden", "Gestein", "Käfer", "Geist", "Stahl",
            "Feuer", "Wasser", "Pflanze", "Elektro", "Psycho", "Eis", "Drache", "Unlicht",
        ][self as usize]
    }
}

fn species_table() -> &'static Vec<Species> {
    static T: OnceLock<Vec<Species>> = OnceLock::new();
    T.get_or_init(|| {
        let raw: Vec<(String, String, i64, i64, u16)> =
            serde_json::from_str(include_str!("../data/species.json")).expect("species.json");
        raw.into_iter()
            .enumerate()
            .map(|(i, (de, en, t1, t2, fam))| Species {
                id: i as u16 + 1,
                name_de: de,
                name_en: en,
                types: [t1, t2].into_iter().filter_map(Type::from_index).collect(),
                family: fam,
            })
            .collect()
    })
}

fn location_table() -> &'static Vec<String> {
    static T: OnceLock<Vec<String>> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../data/locations_de.json")).expect("locations_de.json")
    })
}

/// Art zur nationalen Pokédex-Nummer.
pub fn species(id: u16) -> Option<&'static Species> {
    id.checked_sub(1)
        .and_then(|i| species_table().get(i as usize))
}

/// Deutscher Name eines Fang- oder Kartenorts. Sonderorte (Tausch, Ei,
/// Event ab 30000) und unbekannte Nummern ergeben `None`.
pub fn location_name_de(id: u16) -> Option<&'static str> {
    location_table()
        .get(id as usize)
        .map(String::as_str)
        .filter(|s| !s.starts_with('－') && !s.contains('\\'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_lookup() {
        let s = species(1).unwrap();
        assert_eq!(s.name_de, "Bisasam");
        assert_eq!(s.types, vec![Type::Grass, Type::Poison]);
        assert_eq!(species(649).unwrap().name_en, "Genesect");
        assert!(species(0).is_none());
        assert!(species(650).is_none());
    }

    #[test]
    fn location_lookup() {
        assert_eq!(location_name_de(14), Some("Route 1"));
        assert_eq!(location_name_de(0), None);
        assert_eq!(location_name_de(3), None);
        assert_eq!(location_name_de(30001), None);
    }
}
