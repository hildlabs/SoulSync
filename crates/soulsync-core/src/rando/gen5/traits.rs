//! What a species is like: its base stats, its types and its abilities.

use super::*;

/// How base stats change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Stats {
    #[default]
    Unchanged,
    /// Every species' six stats change places.
    Shuffle,
    /// Six new stats with the same sum.
    Random,
}

/// How types or abilities change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Change {
    #[default]
    Unchanged,
    /// Anew for every species.
    Random,
    /// Anew for every family: an evolution keeps what it evolved from has.
    FollowEvolutions,
}

/// The stats in the stats file, in the order the original randomizer
/// lists them: HP, Attack, Defense, Sp. Attack, Sp. Defense, Speed.
const STAT_BYTES: [usize; 6] = [0, 1, 2, 4, 5, 3];
/// The original randomizer's list of types as the game numbers them. It
/// goes on with eight types generation 5 does not have.
const TYPE_BYTES: [u8; 17] = [0, 1, 2, 11, 10, 9, 5, 4, 13, 6, 15, 12, 7, 3, 14, 8, 16];
const LISTED_TYPES: usize = 25;
const SHEDINJA: usize = 292;
/// Makes a second Pokémon of its kind when it evolves, as Nincada does.
const LEVEL_WITH_EXTRA: u16 = 14;
const WONDER_GUARD: u8 = 25;
const LAST_ABILITY: usize = 164;
/// Abilities that do nothing on another species: Forecast, Multitype,
/// Flower Gift and Zen Mode.
const NO_ABILITIES: [u8; 4] = [59, 121, 122, 161];

/// One step of going through every family from its first stage up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// A species nothing evolves into, or one treated as such.
    First(usize),
    Evolved {
        from: usize,
        to: usize,
        /// Whether `to` is the end of its family.
        last: bool,
        /// Whether this is one of several evolutions of `from`.
        split: bool,
    },
}

/// `Math.round` of Java.
fn round(x: f64) -> f64 {
    let floor = x.floor();
    if x - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    }
}

fn six(stats: &[u8]) -> [i32; 6] {
    STAT_BYTES.map(|at| i32::from(stats[at]))
}

fn put_six(stats: &mut [u8], six: [i32; 6]) {
    for (at, value) in STAT_BYTES.into_iter().zip(six) {
        stats[at] = value as u8;
    }
}

/// Six random shares of `sum`, at least 1 each.
fn shares<const N: usize>(random: &mut JavaRandom, sum: i32) -> [i32; N] {
    let weights: [f64; N] = std::array::from_fn(|_| random.double());
    let total: f64 = weights.iter().sum();
    weights.map(|w| round(w / total * f64::from(sum)).max(1.0) as i32)
}

impl Game<'_> {
    /// The order in which the original randomizer goes through the
    /// families. Without `whole`, a species that is one of several
    /// evolutions counts as the start of a family of its own.
    fn family_steps(&self, whole: bool) -> Vec<Step> {
        let count = SPECIES + FORMS;
        // What each species evolves from, and whether it takes after it.
        let mut before: Vec<Vec<(usize, bool)>> = vec![Vec::new(); count + 1];
        for from in 1..=SPECIES {
            let evolutions = &self.species[from].evolutions;
            for evolution in evolutions {
                let alone = evolutions.len() == 1 || evolution.method == LEVEL_WITH_EXTRA;
                before[usize::from(evolution.to)].push((from, alone));
            }
        }
        let split = |n: usize| before[n].first().is_some_and(|&(_, alone)| !alone);
        let middle = |n: usize| {
            n <= SPECIES
                && before[n].len() == 1
                && !self.species[n].evolutions.is_empty()
                && (before[n][0].1 || whole)
        };
        let mut done = vec![false; count + 1];
        let mut steps = Vec::with_capacity(count);
        for n in 1..=count {
            if before[n].is_empty() {
                steps.push(Step::First(n));
                done[n] = true;
            }
        }
        if !whole {
            for n in (1..=count).filter(|&n| split(n)) {
                steps.push(Step::First(n));
                done[n] = true;
            }
        }
        for n in 1..=count {
            if done[n] {
                continue;
            }
            // Down to a species that is done, then up again.
            let mut chain = vec![n];
            while !done[before[chain[chain.len() - 1]][0].0] {
                let from = before[chain[chain.len() - 1]][0].0;
                chain.push(from);
            }
            for (i, &to) in chain.iter().rev().enumerate() {
                steps.push(Step::Evolved {
                    from: before[to][0].0,
                    to,
                    last: !middle(to),
                    // The original asks this of the species the walk
                    // started from for all but the first step.
                    split: whole && split(if i == 0 { to } else { n }),
                });
                done[to] = true;
            }
        }
        steps
    }

    fn stats_of(&mut self, number: usize) -> &mut Vec<u8> {
        &mut self.stats[Self::stats_file(number)]
    }

    /// A form that only looks different is like its species in `bytes`
    /// of the stats.
    fn copy_species_to_looks_only(&mut self, bytes: &[usize]) {
        for number in SPECIES + 1..=SPECIES + FORMS {
            let (base, looks_only) = (self.species[number].base, self.species[number].looks_only);
            if looks_only && base != 0 {
                for &at in bytes {
                    let value = self.stats[Self::stats_file(base)][at];
                    self.stats_of(number)[at] = value;
                }
                let stats = &self.stats[Self::stats_file(number)];
                self.species[number].abilities =
                    [stats[ABILITIES], stats[ABILITIES + 1], stats[ABILITIES + 2]];
            }
        }
    }

    /// Every species' stats change places; with `follow`, the same way
    /// through a family.
    pub fn shuffle_stats(&mut self, follow: bool) {
        let count = SPECIES + FORMS;
        let mut orders = vec![[0usize, 1, 2, 3, 4, 5]; count + 1];
        let steps = if follow {
            self.family_steps(false)
        } else {
            (1..=count).map(Step::First).collect()
        };
        for step in steps {
            let number = match step {
                Step::First(n) => {
                    self.random.shuffle(&mut orders[n]);
                    n
                }
                Step::Evolved { from, to, .. } => {
                    orders[to] = orders[from];
                    to
                }
            };
            let stats = self.stats_of(number);
            let old = six(stats);
            put_six(stats, orders[number].map(|i| old[i]));
        }
        self.copy_species_to_looks_only(&STAT_BYTES);
    }

    /// New stats with the sum a species has.
    fn random_stats(&mut self, number: usize) {
        let mut stats = six(self.stats_of(number));
        loop {
            let sum: i32 = stats.iter().sum();
            stats = if number == SHEDINJA {
                // One HP is what makes Shedinja bearable.
                let [a, b, c, d, e] = shares(&mut self.random, sum - 51);
                [1, a + 10, b + 10, c + 10, d + 10, e + 10]
            } else {
                let [hp, a, b, c, d, e] = shares(&mut self.random, sum - 70);
                [hp + 20, a + 10, b + 10, c + 10, d + 10, e + 10]
            };
            // A stat that does not fit a byte: once more, from the sum
            // these have.
            if stats.iter().all(|&s| s <= 255) {
                break;
            }
        }
        put_six(self.stats_of(number), stats);
    }

    /// The stats of what `to` evolved from, and the rise of the sum
    /// spread over them at random.
    fn stats_grown(&mut self, from: usize, to: usize) {
        let (old, own) = (six(self.stats_of(from)), six(self.stats_of(to)));
        let rise = f64::from(own.iter().sum::<i32>()) - f64::from(old.iter().sum::<i32>());
        let weights: [f64; 6] = std::array::from_fn(|_| self.random.double());
        let total: f64 = weights.iter().sum();
        let new = std::array::from_fn(|i| {
            let more = round(weights[i] / total * rise);
            (f64::from(old[i]) + more).clamp(1.0, 255.0) as i32
        });
        put_six(self.stats_of(to), new);
    }

    /// The stats of what `to` evolved from, raised to the sum `to` has.
    fn stats_scaled(&mut self, from: usize, to: usize) {
        let (old, own) = (six(self.stats_of(from)), six(self.stats_of(to)));
        let ratio = f64::from(own.iter().sum::<i32>()) / f64::from(old.iter().sum::<i32>());
        let new = old.map(|s| round(f64::from(s) * ratio).clamp(1.0, 255.0) as i32);
        put_six(self.stats_of(to), new);
    }

    /// Every species gets new stats with the sum it has. With `follow`,
    /// an evolution has the stats of what it evolved from, only higher;
    /// with `anew`, what it gains is spread at random.
    pub fn randomize_stats(&mut self, follow: bool, anew: bool) {
        if follow {
            for step in self.family_steps(true) {
                match step {
                    Step::First(n) => self.random_stats(n),
                    Step::Evolved {
                        from, to, split, ..
                    } => {
                        if split || anew {
                            self.stats_grown(from, to);
                        } else {
                            self.stats_scaled(from, to);
                        }
                    }
                }
            }
        } else {
            for number in 1..=SPECIES + FORMS {
                self.random_stats(number);
            }
        }
        self.copy_species_to_looks_only(&STAT_BYTES);
    }

    /// A type of the game, as the original randomizer draws it.
    fn random_type(&mut self) -> u8 {
        loop {
            if let Some(&kind) = TYPE_BYTES.get(self.random.index(LISTED_TYPES)) {
                return kind;
            }
        }
    }

    /// Gives `number` a second type with `chance`, or surely if `always`.
    fn maybe_second_type(&mut self, number: usize, chance: f64, always: bool) {
        if self.random.double() < chance || always {
            let first = self.stats_of(number)[FIRST_TYPE];
            let mut second = self.random_type();
            while second == first {
                second = self.random_type();
            }
            self.stats_of(number)[FIRST_TYPE + 1] = second;
        }
    }

    /// Every species gets random types; with `dual` always two.
    pub fn randomize_types(&mut self, how: Change, dual: bool) {
        let steps = match how {
            Change::Unchanged => return,
            Change::Random => (1..=SPECIES + FORMS).map(Step::First).collect(),
            Change::FollowEvolutions => self.family_steps(false),
        };
        for step in steps {
            match step {
                Step::First(n) => {
                    let kind = self.random_type();
                    let stats = self.stats_of(n);
                    stats[FIRST_TYPE] = kind;
                    stats[FIRST_TYPE + 1] = kind;
                    // A second type is rarer where an evolution inherits it.
                    let evolutions = &self.species[n].evolutions;
                    let inherited =
                        how == Change::FollowEvolutions && n <= SPECIES && evolutions.len() == 1;
                    self.maybe_second_type(n, if inherited { 0.35 } else { 0.5 }, dual);
                }
                Step::Evolved { from, to, last, .. } => {
                    let types = [FIRST_TYPE, FIRST_TYPE + 1].map(|at| self.stats_of(from)[at]);
                    self.stats_of(to)[FIRST_TYPE..FIRST_TYPE + 2].copy_from_slice(&types);
                    if types[0] == types[1] {
                        self.maybe_second_type(to, if last { 0.25 } else { 0.15 }, dual);
                    }
                }
            }
        }
        self.copy_species_to_looks_only(&[FIRST_TYPE, FIRST_TYPE + 1]);
    }

    /// An ability that is allowed and not one of `taken`.
    fn new_ability(&mut self, wonder_guard: bool, taken: &[u8]) -> u8 {
        loop {
            let ability = self.random.index(LAST_ABILITY) as u8 + 1;
            let banned =
                NO_ABILITIES.contains(&ability) || (ability == WONDER_GUARD && !wonder_guard);
            if !banned && !taken.contains(&ability) {
                return ability;
            }
        }
    }

    fn set_abilities(&mut self, number: usize, abilities: [u8; 3]) {
        self.stats_of(number)[ABILITIES..ABILITIES + 3].copy_from_slice(&abilities);
        self.species[number].abilities = abilities;
    }

    /// Every species gets random abilities. One that has Wonder Guard
    /// keeps what it has; others can get it with `wonder_guard`.
    pub fn randomize_abilities(&mut self, how: Change, wonder_guard: bool) {
        let steps = match how {
            Change::Unchanged => return,
            Change::Random => (1..=SPECIES + FORMS).map(Step::First).collect(),
            Change::FollowEvolutions => self.family_steps(false),
        };
        self.abilities_changed = true;
        for step in steps {
            let (number, from) = match step {
                Step::First(n) => (n, None),
                Step::Evolved { from, to, .. } => (to, Some(from)),
            };
            if self.species[number].abilities.contains(&WONDER_GUARD) {
                continue;
            }
            let abilities = match from {
                Some(from) => self.species[from].abilities,
                None => {
                    let first = self.new_ability(wonder_guard, &[]);
                    let second = if self.random.double() < 0.5 {
                        self.new_ability(wonder_guard, &[first])
                    } else {
                        0
                    };
                    let third = self.new_ability(wonder_guard, &[first, second]);
                    [first, second, third]
                }
            };
            self.set_abilities(number, abilities);
        }
        self.copy_species_to_looks_only(&[ABILITIES, ABILITIES + 1, ABILITIES + 2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_as_java_does() {
        for (x, y) in [
            (0.5, 1.0),
            (1.4, 1.0),
            (-0.5, 0.0),
            (-2.5, -2.0),
            (-2.6, -3.0),
        ] {
            assert_eq!(round(x), y, "{x}");
        }
        assert_eq!(round(0.499_999_999_999_999_94), 0.0);
    }

    #[test]
    fn stats_keep_their_place_in_the_file() {
        let mut stats = vec![1, 2, 3, 4, 5, 6];
        assert_eq!(six(&stats), [1, 2, 3, 5, 6, 4]);
        put_six(&mut stats, [10, 20, 30, 40, 50, 60]);
        assert_eq!(stats, [10, 20, 30, 60, 40, 50]);
    }

    #[test]
    fn shares_add_up_to_about_the_sum() {
        let mut random = JavaRandom::new(1);
        let got: [i32; 6] = shares(&mut random, 300);
        assert!((295..=305).contains(&got.iter().sum::<i32>()), "{got:?}");
        assert!(got.iter().all(|&s| s >= 1));
    }
}
