//! Wild Pokémon: every area gets other species, each old species of an
//! area one new one, and the Pokédex learns where they live now.

use super::tables::LEGENDARIES;
use super::*;

const WILD: &str = "a/1/2/7";
const DEX_AREAS: &str = "a/1/7/6";
/// A file of wild Pokémon has one part of this length, or four of them
/// for the seasons. A part starts with the rates of the seven kinds of
/// encounter (grass, dark grass, shaking grass, surfing, rippling water,
/// fishing, rippling fishing) and holds this many slots for each.
const SEASON: usize = 232;
const SLOTS: [usize; 7] = [12, 12, 12, 5, 5, 5, 5];
const FIRST_SLOT: usize = 8;
/// A species' file of the Pokédex: for each season one byte per area
/// with the kinds of encounter there, after a byte that says "nowhere".
const DEX_AREA_COUNT: usize = 85;
const DEX_AREA_LEN: usize = 345;
/// The area of the Pokédex each file of wild Pokémon belongs to.
const AREA_OF_FILE: [i8; 135] = [
    2, 4, 8, 59, 61, 63, 19, 19, 20, 20, 21, 21, 22, 22, 22, 22, 22, 22, 22, 22, 24, 24, 24, 25,
    25, 25, 25, 26, 26, 26, 26, 76, 27, 27, 27, 27, 27, 70, 70, 70, 70, 70, 29, 35, 71, 71, 72, 72,
    73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 74, 74, 74, 74, 74, 74, 74, 74, 74, 74, 76,
    76, 76, 76, 76, 76, 76, 76, 76, 76, 77, 77, 77, 79, 79, 79, 79, 79, 79, 79, 79, 79, 78, 78, -1,
    55, 57, 58, 37, 38, 39, 30, 30, 40, 40, 41, 42, 31, 31, 31, 43, 32, 32, 32, 32, 44, 33, 45, 46,
    47, 48, 49, 34, 50, 51, 36, 53, 66, 67, 69, 75, 12, 52, 68,
];
/// Files that only one of the two games uses, by area: Route 4, rooms of
/// Victory Road and Reversal Mountain.
const ROUTE_4: (i8, usize, usize) = (40, 104, 105);
const VICTORY_ROAD: (i8, [usize; 2], [usize; 2]) = (76, [71, 73], [78, 79]);
const REVERSAL_MOUNTAIN: (i8, (usize, usize), (usize, usize)) = (73, (49, 54), (55, 60));
/// Forms a player could not keep: Shaymin's and Giratina's other form.
/// Those that only exist through an ability, Castform's and Darmanitan's,
/// are left out too unless abilities are random.
const NOT_WILD: [usize; 2] = [655, 656];
const BY_ABILITY: [usize; 4] = [662, 663, 664, 666];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    /// The species or form, as a number.
    pokemon: usize,
    form: u16,
}

/// The kinds of encounter of one part of a file that take place at all,
/// each with where its slots start and how many there are.
fn kinds(part: &[u8]) -> Vec<(usize, usize, usize)> {
    let mut at = FIRST_SLOT;
    let mut found = Vec::new();
    for (kind, &slots) in SLOTS.iter().enumerate() {
        if part[kind] != 0 {
            found.push((kind, at, slots));
        }
        at += slots * 4;
    }
    found
}

impl<'a> Game<'a> {
    /// The file of the Pokédex for each species, from the wild Pokémon
    /// as they are in `files`.
    fn dex_areas(&self, files: &[Vec<u8>]) -> Vec<Vec<u8>> {
        let white = self.code[2] == b'D';
        let mut dex = vec![vec![0u8; DEX_AREA_LEN]; SPECIES];
        for (number, file) in files.iter().enumerate() {
            let Some(&area) = AREA_OF_FILE.get(number) else {
                continue;
            };
            let other_game = if area == ROUTE_4.0 {
                number == if white { ROUTE_4.1 } else { ROUTE_4.2 }
            } else if area == VICTORY_ROAD.0 {
                let rooms = if white {
                    VICTORY_ROAD.1
                } else {
                    VICTORY_ROAD.2
                };
                rooms.contains(&number)
            } else if area == REVERSAL_MOUNTAIN.0 {
                let (from, to) = if white {
                    REVERSAL_MOUNTAIN.1
                } else {
                    REVERSAL_MOUNTAIN.2
                };
                (from..=to).contains(&number)
            } else {
                false
            };
            if area < 0 || other_game {
                continue;
            }
            for season in 0..4 {
                let start = if file.len() > SEASON {
                    season * SEASON
                } else {
                    0
                };
                let part = &file[start..];
                for (kind, at, slots) in kinds(part) {
                    for slot in 0..slots {
                        let species = usize::from(word(part, at + slot * 4) & 0x3FF);
                        if let Some(entry) = species.checked_sub(1).and_then(|n| dex.get_mut(n)) {
                            entry[season * (DEX_AREA_COUNT + 1) + 2 + area as usize] |= 1 << kind;
                        }
                    }
                }
            }
        }
        for entry in &mut dex {
            let areas = |season: usize| {
                let from = season * (DEX_AREA_COUNT + 1) + 2;
                from..from + DEX_AREA_COUNT
            };
            for season in 0..4 {
                if entry[areas(season)].iter().all(|&b| b == 0) {
                    entry[season * (DEX_AREA_COUNT + 1) + 1] = 1;
                }
            }
            let spring = entry[areas(0)].to_vec();
            if (1..4).all(|season| entry[areas(season)] == spring[..]) {
                entry[0] = 1;
            }
        }
        dex
    }

    /// Each area gets other wild Pokémon: every species of an area
    /// becomes one random other, with `forms` also other forms.
    pub fn randomize_wild(&mut self, forms: bool) -> Result<(), RomError> {
        let mut files = Self::narc(&self.nds, WILD)?;
        // The areas in the order of the files, as places of their slots.
        let mut areas: Vec<(usize, usize, usize)> = Vec::new();
        for (number, file) in files.iter().enumerate() {
            if file.len() < SEASON {
                return Err(layout(WILD));
            }
            areas.extend(
                kinds(file)
                    .into_iter()
                    .map(|(_, at, slots)| (number, at, slots)),
            );
        }
        let read = |files: &[Vec<u8>], (file, at, slots): (usize, usize, usize)| {
            let mut list = Vec::with_capacity(slots);
            for slot in 0..slots {
                let both = word(&files[file], at + slot * 4);
                let (species, form) = (usize::from(both & 0x7FF), both >> 11);
                if !(1..=SPECIES).contains(&species) {
                    return Err(layout(WILD));
                }
                let looks = u16::from(self.species[species].looks);
                let pokemon = if form <= looks || form >= 30 {
                    species
                } else {
                    self.form_of(species, form)
                };
                list.push(Slot { pokemon, form });
            }
            Ok(list)
        };
        let mut slots = Vec::with_capacity(areas.len());
        for &area in &areas {
            slots.push(read(&files, area)?);
        }

        let legendary = |n: usize| {
            let species = match self.species[n].base {
                0 => n,
                base => base,
            };
            LEGENDARIES.contains(&(species as u16))
        };
        let last = if forms { SPECIES + FORMS } else { SPECIES };
        // The original draws from the non-legendary ones too, and then
        // does not use that draw.
        let others = (1..=last).filter(|&n| !legendary(n)).count();
        let mut order: Vec<usize> = (0..areas.len()).collect();
        self.random.shuffle(&mut order);
        for area in order {
            let mut old: Vec<usize> = slots[area].iter().map(|s| s.pokemon).collect();
            old.sort_unstable();
            old.dedup();
            let mut new: Vec<usize> = Vec::with_capacity(old.len());
            for _ in &old {
                let picked = loop {
                    self.random.index(others);
                    let picked = 1 + self.random.index(last);
                    let fits = !self.species[picked].looks_only
                        && !NOT_WILD.contains(&picked)
                        && (self.abilities_changed || !BY_ABILITY.contains(&picked));
                    if fits && !new.contains(&picked) {
                        break picked;
                    }
                };
                new.push(picked);
            }
            for slot in &mut slots[area] {
                let at = old.binary_search(&slot.pokemon).map_err(|_| layout(WILD))?;
                let entry = &self.species[new[at]];
                *slot = if entry.form > 0 {
                    Slot {
                        pokemon: entry.base,
                        form: entry.form,
                    }
                } else if entry.looks > 0 {
                    let form = self.random.index(usize::from(entry.looks)) as u16;
                    Slot {
                        pokemon: new[at],
                        form,
                    }
                } else {
                    Slot {
                        pokemon: new[at],
                        form: 0,
                    }
                };
            }
        }

        for (&(file, at, _), list) in areas.iter().zip(&slots) {
            for (i, slot) in list.iter().enumerate() {
                set_word(
                    &mut files[file],
                    at + i * 4,
                    (slot.form << 11) + slot.pokemon as u16,
                );
            }
        }
        for file in &mut files {
            if file.len() >= SEASON * 4 {
                let (spring, rest) = file.split_at_mut(SEASON);
                for season in rest.chunks_exact_mut(SEASON).take(3) {
                    season.copy_from_slice(spring);
                }
            }
        }
        self.put_narc(WILD, &files)?;

        let mut dex = Self::narc(&self.nds, DEX_AREAS)?;
        if dex.len() < SPECIES {
            return Err(layout(DEX_AREAS));
        }
        for (old, new) in dex.iter_mut().zip(self.dex_areas(&files)) {
            *old = new;
        }
        self.put_narc(DEX_AREAS, &dex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_kinds_with_a_rate_take_place() {
        let mut part = vec![0u8; SEASON];
        part[0] = 30;
        part[3] = 5;
        part[6] = 1;
        assert_eq!(kinds(&part), [(0, 8, 12), (3, 152, 5), (6, 212, 5)]);
        assert_eq!(FIRST_SLOT + SLOTS.iter().sum::<usize>() * 4, SEASON);
    }

    #[test]
    fn the_pokedex_has_room_for_every_area() {
        assert_eq!(4 * (DEX_AREA_COUNT + 1) + 1, DEX_AREA_LEN);
        assert!(AREA_OF_FILE
            .iter()
            .all(|&a| (a as isize) < DEX_AREA_COUNT as isize));
    }
}
