//! The Pokémon met or handed out at fixed spots, and those of the Hidden
//! Grottoes.

use super::tables::{Static, FAKE_BALLS, GROTTO_UNOVA, LEGENDARIES, STATICS, STATICS_JAPAN};
use super::*;

const MAPS: &str = "a/1/2/6";
const GROTTOES: &str = "a/2/7/3";
/// A grotto's file holds both games' Pokémon, 78 bytes each: three
/// rarities of 26 bytes with four Pokémon each (species, highest level,
/// level, share of females, form).
const GROTTO_GAME: usize = 78;
const GROTTO_RARITY: usize = 26;
const GROTTO_GROUPS: usize = 4;
const GROTTO_MAX_LEVEL: usize = 8;
const GROTTO_LEVEL: usize = 12;
const GROTTO_FEMALES: usize = 16;
const GROTTO_FORM: usize = 20;
/// The species before the newest ones; all of them may hide in a grotto.
const OLDER_SPECIES: usize = 493;

/// A fixed Pokémon as the randomizer sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Fixed {
    /// The species or form, as a number.
    pokemon: usize,
    form: u16,
    level: u8,
    max_level: u8,
    /// Only species a grotto may hold.
    grotto: bool,
    /// Further places with the same Pokémon: their levels.
    linked: Vec<(u8, u8)>,
}

fn byte(files: &[Vec<u8>], (file, at): (usize, usize)) -> Result<u8, RomError> {
    let found = files.get(file).and_then(|f| f.get(at));
    found.copied().ok_or(layout("fixed Pokémon"))
}

fn put(files: &mut [Vec<u8>], (file, at): (usize, usize), bytes: &[u8]) -> Result<(), RomError> {
    let found = files
        .get_mut(file)
        .and_then(|f| f.get_mut(at..at + bytes.len()));
    found.ok_or(layout("fixed Pokémon"))?.copy_from_slice(bytes);
    Ok(())
}

impl<'a> Game<'a> {
    fn statics(&self) -> &'static [Static] {
        if self.code[3] == b'J' {
            &STATICS_JAPAN
        } else {
            &STATICS
        }
    }

    /// Where this game's Pokémon start in a grotto's file. The original
    /// randomizer takes Black 2's also for the Japanese White 2.
    fn grotto_start(&self) -> usize {
        if self.code[2] == b'E' || self.code[3] == b'J' {
            GROTTO_GAME
        } else {
            0
        }
    }

    fn read_fixed(&self, place: &Static, levels: &[Vec<u8>]) -> Result<Fixed, RomError> {
        let first = *place.species.first().ok_or(layout("fixed Pokémon"))?;
        let species = byte(&self.scripts, first)?;
        let species = usize::from(u16::from_le_bytes([
            species,
            byte(&self.scripts, (first.0, first.1 + 1))?,
        ]));
        if !(1..=SPECIES).contains(&species) {
            return Err(layout("fixed Pokémon"));
        }
        let form = match place.forms.first() {
            Some(&at) => u16::from(byte(&self.scripts, at)?),
            None => 0,
        };
        let mut all = Vec::new();
        for &at in place.levels {
            all.push(byte(levels, at)?);
        }
        Ok(Fixed {
            pokemon: self.form_of(species, form),
            form: 0,
            level: all.first().copied().unwrap_or(1),
            max_level: 0,
            grotto: false,
            linked: all.iter().skip(1).map(|&l| (l, 0)).collect(),
        })
    }

    /// Every fixed Pokémon is swapped for a random one: legendary ones
    /// for legendary ones, the others for others.
    pub fn randomize_statics(&mut self) -> Result<(), RomError> {
        let mut maps = Self::narc(&self.nds, MAPS)?;
        let mut grottoes = Self::narc(&self.nds, GROTTOES)?;
        let start = self.grotto_start();

        let mut old = Vec::new();
        for place in self.statics() {
            old.push(self.read_fixed(place, &self.scripts)?);
        }
        for place in &FAKE_BALLS {
            old.push(self.read_fixed(place, &maps)?);
        }
        for grotto in &grottoes {
            if grotto.len() < start + GROTTO_GAME {
                return Err(layout(GROTTOES));
            }
            for rarity in 0..3 {
                let at = start + rarity * GROTTO_RARITY;
                let from = old.len();
                for group in 0..GROTTO_GROUPS {
                    let species = usize::from(word(grotto, at + group * 2));
                    if !(1..=SPECIES).contains(&species) {
                        return Err(layout(GROTTOES));
                    }
                    let pokemon =
                        self.form_of(species, u16::from(grotto[at + GROTTO_FORM + group]));
                    let level = grotto[at + GROTTO_LEVEL + group];
                    let max_level = grotto[at + GROTTO_MAX_LEVEL + group];
                    match old[from..].iter_mut().find(|f| f.pokemon == pokemon) {
                        Some(same) => same.linked.push((level, max_level)),
                        None => old.push(Fixed {
                            pokemon,
                            form: 0,
                            level,
                            max_level,
                            grotto: true,
                            linked: Vec::new(),
                        }),
                    }
                }
            }
        }

        let legendary = |n: usize| LEGENDARIES.contains(&(n as u16));
        let pools: [Vec<usize>; 2] = [
            (1..=SPECIES).filter(|&n| !legendary(n)).collect(),
            (1..=SPECIES).filter(|&n| legendary(n)).collect(),
        ];
        let mut left = pools.clone();
        let in_grotto = |n: &usize| *n <= OLDER_SPECIES || GROTTO_UNOVA.contains(&(*n as u16));
        let mut new = old.clone();
        for (fixed, old) in new.iter_mut().zip(&old) {
            // A form is as legendary as its species.
            let species = match self.species[old.pokemon].base {
                0 => old.pokemon,
                base => base,
            };
            let which = usize::from(legendary(species));
            let chosen = if old.grotto {
                let mut choice: Vec<usize> =
                    left[which].iter().copied().filter(in_grotto).collect();
                if choice.is_empty() {
                    choice = pools[which].iter().copied().filter(in_grotto).collect();
                }
                let chosen = choice[self.random.index(choice.len())];
                left[which].retain(|n| *n != chosen);
                chosen
            } else {
                let at = self.random.index(left[which].len());
                left[which].remove(at)
            };
            fixed.pokemon = chosen;
            let looks = self.species[chosen].looks;
            fixed.form = if looks > 0 {
                self.random.index(usize::from(looks)) as u16
            } else {
                0
            };
            if left[which].is_empty() {
                left[which] = pools[which].clone();
            }
        }

        let mut new = new.into_iter();
        let mut next = || new.next().ok_or(layout("fixed Pokémon"));
        for place in self.statics() {
            let fixed = next()?;
            for &at in place.species {
                put(&mut self.scripts, at, &(fixed.pokemon as u16).to_le_bytes())?;
            }
            for &at in place.forms {
                put(&mut self.scripts, at, &[0])?;
            }
            let levels = std::iter::once(fixed.level).chain(fixed.linked.iter().map(|l| l.0));
            for (&at, level) in place.levels.iter().zip(levels) {
                put(&mut self.scripts, at, &[level])?;
            }
        }
        for place in &FAKE_BALLS {
            let fixed = next()?;
            for &at in place.species {
                put(&mut self.scripts, at, &(fixed.pokemon as u16).to_le_bytes())?;
            }
            let levels = std::iter::once(fixed.level).chain(fixed.linked.iter().map(|l| l.0));
            for (&at, level) in place.levels.iter().zip(levels) {
                put(&mut maps, at, &[level])?;
            }
        }
        self.put_narc(MAPS, &maps)?;
        for grotto in &mut grottoes {
            for rarity in 0..3 {
                let at = start + rarity * GROTTO_RARITY;
                let mut group = 0;
                while group < GROTTO_GROUPS {
                    let fixed = next()?;
                    let females = self.random.index(101) as u8;
                    let places =
                        std::iter::once((fixed.level, fixed.max_level)).chain(fixed.linked);
                    for (level, max_level) in places {
                        if group >= GROTTO_GROUPS {
                            return Err(layout(GROTTOES));
                        }
                        set_word(grotto, at + group * 2, fixed.pokemon as u16);
                        grotto[at + GROTTO_FEMALES + group] = females;
                        grotto[at + GROTTO_FORM + group] = fixed.form as u8;
                        grotto[at + GROTTO_LEVEL + group] = level;
                        grotto[at + GROTTO_MAX_LEVEL + group] = max_level;
                        group += 1;
                    }
                }
            }
        }
        self.put_narc(GROTTOES, &grottoes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_stay_inside_the_games() {
        for place in STATICS.iter().chain(&STATICS_JAPAN).chain(&FAKE_BALLS) {
            assert!(!place.species.is_empty());
        }
        assert_eq!(STATICS.len(), STATICS_JAPAN.len());
        let species = 1..=SPECIES as u16;
        assert!(LEGENDARIES.iter().all(|n| species.contains(n)));
        assert!(GROTTO_UNOVA
            .iter()
            .all(|n| *n as usize > OLDER_SPECIES && species.contains(n)));
    }

    #[test]
    fn bytes_outside_a_file_are_refused() {
        let mut files = vec![vec![1, 2, 3]];
        assert_eq!(byte(&files, (0, 2)).unwrap(), 3);
        assert!(byte(&files, (0, 3)).is_err());
        assert!(byte(&files, (1, 0)).is_err());
        assert!(put(&mut files, (0, 2), &[9, 9]).is_err());
        put(&mut files, (0, 1), &[9, 9]).unwrap();
        assert_eq!(files[0], [1, 9, 9]);
    }
}
