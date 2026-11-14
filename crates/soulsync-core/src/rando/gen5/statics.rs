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
/// Which of the fixed Pokémon is the egg.
const EGG: usize = 29;
/// Forms a player could not keep: Shaymin's and Giratina's other form.
/// Those that only exist through an ability, Castform's and Darmanitan's,
/// are left out too unless abilities are random.
const NOT_FIXED: [usize; 2] = [655, 656];
const BY_ABILITY: [usize; 4] = [662, 663, 664, 666];
/// The legendary Pokémon of the story: Cobalion, Terrakion and Virizion.
const MAIN_LEGENDARIES: [usize; 3] = [638, 639, 640];

/// How the new fixed Pokémon are chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Statics {
    /// Legendary ones for legendary ones, the others for others.
    #[default]
    Matching,
    /// Any for any.
    Random,
    /// About as strong as the old ones.
    SimilarStrength,
}

/// How fixed Pokémon are made random; the levels in it change without
/// random ones too.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StaticRules {
    pub mode: Statics,
    /// With similar strength: the story's legendary Pokémon become no
    /// stronger than they were.
    pub limit_main_legendaries: bool,
    /// With similar strength: the strongest become any Pokémon.
    pub limit_600: bool,
    /// Other forms are chosen too.
    pub forms: bool,
    /// Per cent by which the levels rise or fall.
    pub levels: i8,
}

/// A level `percent` higher or lower, 100 at most.
fn raised(level: u8, percent: i8) -> u8 {
    if percent == 0 {
        return level;
    }
    let changed = f64::from(level) * (1.0 + f64::from(percent) / 100.0);
    traits::round(changed).min(100.0) as u8
}

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
    /// Handed out as an egg, which keeps its level.
    egg: bool,
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
            egg: false,
            linked: all.iter().skip(1).map(|&l| (l, 0)).collect(),
        })
    }

    /// All fixed Pokémon in the original's order, with the files of the
    /// maps and the grottoes.
    #[allow(clippy::type_complexity)]
    fn fixed_pokemon(&self) -> Result<(Vec<Fixed>, Vec<Vec<u8>>, Vec<Vec<u8>>), RomError> {
        let maps = Self::narc(&self.nds, MAPS)?;
        let grottoes = Self::narc(&self.nds, GROTTOES)?;
        let start = self.grotto_start();

        let mut old = Vec::new();
        for (number, place) in self.statics().iter().enumerate() {
            let mut fixed = self.read_fixed(place, &self.scripts)?;
            fixed.egg = number == EGG;
            old.push(fixed);
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
                            egg: false,
                            linked: Vec::new(),
                        }),
                    }
                }
            }
        }
        Ok((old, maps, grottoes))
    }

    /// Writes the fixed Pokémon back. The scripts only ever get the
    /// species, as in the original, which loses a form there.
    fn set_fixed(
        &mut self,
        new: Vec<Fixed>,
        mut maps: Vec<Vec<u8>>,
        mut grottoes: Vec<Vec<u8>>,
    ) -> Result<(), RomError> {
        let start = self.grotto_start();
        let species = |game: &Self, pokemon: usize| match game.species[pokemon].base {
            0 => pokemon as u16,
            base => base as u16,
        };
        let mut new = new.into_iter();
        let mut next = || new.next().ok_or(layout("fixed Pokémon"));
        for place in self.statics() {
            let fixed = next()?;
            let number = species(self, fixed.pokemon);
            for &at in place.species {
                put(&mut self.scripts, at, &number.to_le_bytes())?;
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
            let number = species(self, fixed.pokemon);
            for &at in place.species {
                put(&mut self.scripts, at, &number.to_le_bytes())?;
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
                    let number = species(self, fixed.pokemon);
                    let females = self.random.index(101) as u8;
                    let places =
                        std::iter::once((fixed.level, fixed.max_level)).chain(fixed.linked);
                    for (level, max_level) in places {
                        if group >= GROTTO_GROUPS {
                            return Err(layout(GROTTOES));
                        }
                        set_word(grotto, at + group * 2, number);
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

    /// The form a fixed Pokémon shows: its own, and any of those that
    /// only look different.
    fn fixed_form(&mut self, pokemon: usize) -> u16 {
        let own = self.species[pokemon].form;
        match self.species[pokemon].looks {
            0 => own,
            looks => own + self.random.index(usize::from(looks)) as u16,
        }
    }

    /// One of `pool` that is about as strong as `like`, or with `weaker`
    /// no stronger; not `like` itself with `other`.
    fn similar_fixed(&mut self, pool: &[usize], like: usize, other: bool, weaker: bool) -> usize {
        let strength = self.strength(like);
        let (mut least, mut most) = if weaker {
            (strength - strength / 5, strength)
        } else {
            (strength - strength / 10, strength + strength / 10)
        };
        let mut fit: Vec<usize> = Vec::new();
        let mut rounds = 0;
        while fit.is_empty() || (fit.len() < 3 && rounds < 3) {
            for &pokemon in pool {
                if (least..=most).contains(&self.strength(pokemon))
                    && !(other && pokemon == like)
                    && !fit.contains(&pokemon)
                {
                    fit.push(pokemon);
                }
            }
            least -= strength / 20;
            most += strength / 20;
            rounds += 1;
        }
        fit[self.random.index(fit.len())]
    }

    /// Every fixed Pokémon is swapped for a random one, as `how` says.
    pub fn randomize_statics(&mut self, how: &StaticRules) -> Result<(), RomError> {
        let (old, maps, grottoes) = self.fixed_pokemon()?;
        // A form is as legendary as its species.
        let species = |game: &Self, pokemon: usize| match game.species[pokemon].base {
            0 => pokemon,
            base => base,
        };
        let legendary = |game: &Self, n: usize| LEGENDARIES.contains(&(species(game, n) as u16));
        let mut banned = NOT_FIXED.to_vec();
        if !self.abilities_changed {
            banned.extend(BY_ABILITY);
        }
        if self.no_irregular_forms {
            banned.extend(IRREGULAR_FORMS);
        }
        let last = if how.forms { SPECIES + FORMS } else { SPECIES };
        let usable: Vec<usize> = (1..=last)
            .filter(|&n| !(how.forms && self.species[n].looks_only) && !banned.contains(&n))
            .collect();
        // Legendary ones for legendary ones, or one pool for all.
        let pools: Vec<Vec<usize>> = if how.mode == Statics::Matching {
            vec![
                (usable.iter().copied())
                    .filter(|&n| !legendary(self, n))
                    .collect(),
                (usable.iter().copied())
                    .filter(|&n| legendary(self, n))
                    .collect(),
            ]
        } else {
            vec![usable]
        };
        let mut left = pools.clone();
        let in_grotto = |n: &usize| *n <= OLDER_SPECIES || GROTTO_UNOVA.contains(&(*n as u16));
        let mut new = old.clone();
        for (fixed, old) in new.iter_mut().zip(&old) {
            let which = match how.mode {
                Statics::Matching => usize::from(legendary(self, old.pokemon)),
                _ => 0,
            };
            // What a grotto may hold of what is left, or of all.
            let mut choice: Vec<usize> = Vec::new();
            if old.grotto {
                choice = left[which].iter().copied().filter(in_grotto).collect();
                if choice.is_empty() {
                    choice = pools[which].iter().copied().filter(in_grotto).collect();
                }
            }
            let alike = how.mode == Statics::SimilarStrength
                && !(how.limit_600 && self.strength(old.pokemon) >= 600);
            let chosen = if alike {
                let weaker = how.limit_main_legendaries
                    && MAIN_LEGENDARIES.contains(&species(self, old.pokemon));
                if old.grotto {
                    self.similar_fixed(&choice, old.pokemon, false, weaker)
                } else {
                    let pool = std::mem::take(&mut left[which]);
                    let chosen = self.similar_fixed(&pool, old.pokemon, true, weaker);
                    left[which] = pool;
                    chosen
                }
            } else if old.grotto {
                choice[self.random.index(choice.len())]
            } else {
                left[which][self.random.index(left[which].len())]
            };
            left[which].retain(|n| *n != chosen);
            fixed.pokemon = chosen;
            fixed.form = self.fixed_form(chosen);
            if left[which].is_empty() {
                left[which] = pools[which].clone();
            }
        }
        for fixed in new.iter_mut().filter(|f| !f.egg) {
            fixed.level = raised(fixed.level, how.levels);
            fixed.max_level = raised(fixed.max_level, how.levels);
            for (level, max_level) in &mut fixed.linked {
                *level = raised(*level, how.levels);
                *max_level = raised(*max_level, how.levels);
            }
        }
        self.set_fixed(new, maps, grottoes)
    }

    /// Only the levels of the fixed Pokémon rise or fall by `percent`.
    /// The original still draws how each one looks, and does not touch
    /// the highest level in a grotto.
    pub fn change_static_levels(&mut self, percent: i8) -> Result<(), RomError> {
        let (mut all, maps, grottoes) = self.fixed_pokemon()?;
        for fixed in &mut all {
            if !fixed.egg {
                fixed.level = raised(fixed.level, percent);
                for (level, _) in &mut fixed.linked {
                    *level = raised(*level, percent);
                }
            }
            fixed.form = self.fixed_form(fixed.pokemon);
        }
        self.set_fixed(all, maps, grottoes)
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
    fn levels_rise_and_fall_as_in_the_original() {
        assert_eq!(raised(25, 0), 25);
        assert_eq!(raised(25, 50), 38);
        assert_eq!(raised(5, -50), 3);
        assert_eq!(raised(70, 50), 100);
        assert_eq!(raised(0, 50), 0);
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
