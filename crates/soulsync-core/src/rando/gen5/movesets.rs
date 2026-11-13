//! The moves a species learns by level and from its parents.

use super::*;
use std::collections::BTreeMap;

/// What each species and form learns at which level, and the moves a
/// species can hatch with.
pub(super) const LEARNSETS: &str = "a/0/1/8";
pub(super) const EGG_MOVES: &str = "a/1/2/4";
const CATEGORY: usize = 2;
const POWER: usize = 3;
const PP: usize = 5;
const METRONOME: u16 = 118;
const ACCURACY: usize = 4;
const PHYSICAL: u8 = 1;
const SPECIAL: u8 = 2;
/// Damaging moves nobody is given as their one sure attack: they fail,
/// faint the user, always do the same damage or are too weak.
const NO_ATTACKS: [u16; 25] = [
    120, 138, 153, 173, 206, 248, 252, 264, 353, 364, 387, 389, 132, 99, 205, 301, 485, 492, 255,
    49, 82, 32, 12, 90, 329,
];
/// The power a good attack has at least, all hits together.
const GOOD_POWER: f64 = 50.0;
/// Moves that always do the same damage, which is far too much early on.
pub(super) const BROKEN: [u16; 2] = [49, 82];

/// How the moves a species learns are chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Movesets {
    #[default]
    Unchanged,
    Random,
    /// Random, but more often than not of the species' own types.
    PreferType,
    /// Nobody knows or can be taught anything but Metronome.
    Metronome,
}
/// The place of each type of the game in the original randomizer's list
/// of types, which is the order it goes through them.
pub(super) const TYPE_PLACES: [u8; 17] = [0, 1, 2, 13, 7, 6, 9, 12, 15, 5, 4, 3, 11, 8, 14, 10, 16];

/// What random movesets are made from.
struct Pool {
    /// Every move that can be learnt, and those that are good attacks.
    all: Vec<u16>,
    attacks: Vec<u16>,
    /// The same by type; a type whose moves are much weaker or stronger
    /// than the others' has some of them more than once.
    of_type: BTreeMap<u8, Vec<u16>>,
    attacks_of_type: BTreeMap<u8, Vec<u16>>,
    category: Vec<u8>,
}

/// How a species' new moves are chosen.
#[derive(Clone, Copy)]
struct Taste {
    types: Option<[u8; 2]>,
    /// The share of its attacks that are physical.
    physical: f64,
}

impl Game<'_> {
    /// Whether a move is an attack worth having: strong, or fairly
    /// strong and accurate.
    pub(super) fn good_attack(&self, number: u16) -> bool {
        let data = &self.moves[usize::from(number)];
        let power = f64::from(data[POWER]) * moves::hits(usize::from(number));
        let accurate = data[ACCURACY] >= 90;
        let good = power >= 2.0 * GOOD_POWER || (power >= GOOD_POWER && accurate);
        good && !NO_ATTACKS.contains(&number)
    }

    fn move_pool(&mut self, no_broken: bool) -> Result<Pool, RomError> {
        let (_, hms) = self.machine_moves().ok_or(layout("moves of the TMs"))?;
        let strength = |game: &Self, m: u16| {
            f64::from(game.moves[usize::from(m)][POWER]) * moves::hits(usize::from(m))
        };
        let mut pool = Pool {
            all: Vec::new(),
            attacks: Vec::new(),
            of_type: BTreeMap::new(),
            attacks_of_type: BTreeMap::new(),
            category: self.moves.iter().map(|m| m[CATEGORY]).collect(),
        };
        let banned =
            |m: &u16| *m == STRUGGLE || hms.contains(m) || (no_broken && BROKEN.contains(m));
        for number in (1..=MOVES_COUNT as u16).filter(|m| !banned(m)) {
            let data = &self.moves[usize::from(number)];
            let place = TYPE_PLACES.get(usize::from(data[0])).copied().unwrap_or(0);
            pool.all.push(number);
            pool.of_type.entry(place).or_default().push(number);
            if self.good_attack(number) {
                pool.attacks.push(number);
                pool.attacks_of_type.entry(place).or_default().push(number);
            }
        }

        // The average power of each type's moves, and of all types.
        let mut averages = BTreeMap::new();
        for (&place, moves) in &pool.of_type {
            let mut sum = 0i32;
            for &m in moves {
                if self.moves[usize::from(m)][POWER] > 0 {
                    sum = (f64::from(sum) + strength(self, m)) as i32;
                }
            }
            averages.insert(place, f64::from(sum) / moves.len() as f64);
        }
        let overall = averages.values().sum::<f64>() / averages.len() as f64;
        let (least, most) = (overall * 0.75, overall * 1.25);
        // A type outside of that gets stronger or weaker moves of its own
        // once more, until it is inside.
        for (place, mut average) in averages {
            let moves = pool.of_type.get_mut(&place).ok_or(layout("moves"))?;
            let mut picked: Vec<u16> = Vec::new();
            for stronger in [true, false] {
                for _ in 0..10000 {
                    if (stronger && average >= least) || (!stronger && average <= most) {
                        break;
                    }
                    let mut wanted: Vec<u16> = moves
                        .iter()
                        .copied()
                        .filter(|&m| {
                            let power = strength(self, m);
                            (stronger && power > average) || (!stronger && power < average)
                        })
                        .collect();
                    if wanted.is_empty() {
                        break;
                    }
                    if wanted.iter().all(|m| picked.contains(m)) {
                        picked.clear();
                    } else {
                        wanted.retain(|m| !picked.contains(m));
                    }
                    let extra = wanted[self.random.index(wanted.len())];
                    let count = moves.len() as f64;
                    average = (average * count + strength(self, extra)) / (count + 1.0);
                    moves.push(extra);
                    picked.push(extra);
                }
            }
        }
        Ok(pool)
    }

    fn taste(&self, number: usize, by_type: bool) -> Taste {
        let stats = &self.stats[Self::stats_file(number)];
        let place = |at: usize| {
            TYPE_PLACES
                .get(usize::from(stats[at]))
                .copied()
                .unwrap_or(0)
        };
        let (attack, special) = (f64::from(stats[1]), f64::from(stats[4]));
        Taste {
            types: by_type.then(|| [place(FIRST_TYPE), place(FIRST_TYPE + 1)]),
            physical: attack / (attack + special),
        }
    }

    /// One more move for a species that has `learnt` already; an attack
    /// if `attack` says so and one is left.
    fn pick_move(&mut self, pool: &Pool, taste: Taste, learnt: &[u16], attack: bool) -> u16 {
        const NORMAL: u8 = 0;
        // A move of one of its own types, more often than not.
        let kind = taste.types.and_then(|[first, second]| {
            let picked = self.random.double();
            if first != second && (first == NORMAL || second == NORMAL) {
                let other = if first == NORMAL { second } else { first };
                if picked < 0.1 {
                    Some(NORMAL)
                } else {
                    (picked < 0.4).then_some(other)
                }
            } else if first != second {
                if picked < 0.2 {
                    Some(first)
                } else {
                    (picked < 0.4).then_some(second)
                }
            } else {
                (picked < 0.4).then_some(first)
            }
        });
        let unused = |list: &[u16]| list.iter().any(|m| !learnt.contains(m));
        fn of<'a>(
            map: &'a BTreeMap<u8, Vec<u16>>,
            kind: Option<u8>,
            learnt: &[u16],
        ) -> Option<&'a [u16]> {
            let list = map.get(&kind?)?;
            list.iter()
                .any(|m| !learnt.contains(m))
                .then_some(list.as_slice())
        }
        let mut list: &[u16] = &pool.all;
        let narrowed: Vec<u16>;
        if attack {
            if let Some(own) = of(&pool.attacks_of_type, kind, learnt) {
                list = own;
            } else if unused(&pool.attacks) {
                list = &pool.attacks;
            }
            // Physical or special, as the species itself is.
            let wanted = if self.random.double() < taste.physical {
                PHYSICAL
            } else {
                SPECIAL
            };
            narrowed = list
                .iter()
                .copied()
                .filter(|&m| pool.category[usize::from(m)] == wanted)
                .collect();
            if unused(&narrowed) {
                list = &narrowed;
            }
        } else if let Some(own) = of(&pool.of_type, kind, learnt) {
            list = own;
        }
        loop {
            let chosen = list[self.random.index(list.len())];
            if !learnt.contains(&chosen) {
                return chosen;
            }
        }
    }

    /// Every species learns random moves at the levels it learnt moves
    /// at, its first one an attack. With `by_type` more of them have its
    /// own types; `at_start` is how many moves it knows at level 1 at
    /// least (0 for as many as it had); `attacks` is the share of good
    /// attacks among them in percent; `no_broken` leaves out the moves
    /// that always do the same damage.
    pub fn randomize_movesets(
        &mut self,
        by_type: bool,
        at_start: usize,
        attacks: u8,
        no_broken: bool,
    ) -> Result<(), RomError> {
        let mut files = Self::narc(&self.nds, LEARNSETS)?;
        let pool = self.move_pool(no_broken)?;
        let mut all: Vec<Vec<(u16, u16)>> = vec![Vec::new()];
        for number in 1..=SPECIES + FORMS {
            let data = files
                .get(Self::stats_file(number))
                .ok_or(layout(LEARNSETS))?;
            let entries = data.chunks_exact(4).map(|e| (word(e, 0), word(e, 2)));
            let mut set: Vec<(u16, u16)> = entries.take_while(|&e| e != (0xFFFF, 0xFFFF)).collect();
            let known = set.iter().filter(|&&(_, level)| level == 1).count();
            for _ in known..at_start {
                set.insert(0, (0, 1));
            }
            if set.is_empty() {
                return Err(layout(LEARNSETS));
            }
            let (base, looks_only) = (self.species[number].base, self.species[number].looks_only);
            if looks_only && base != 0 {
                // A form that only looks different learns what its
                // species learns.
                for (i, entry) in set.iter_mut().enumerate() {
                    entry.0 = all[base].get(i).ok_or(layout(LEARNSETS))?.0;
                }
                all.push(set);
                continue;
            }
            // The last of the moves known from the start is the attack.
            let mut first = usize::from(set[0].1 != 1);
            while first < set.len() && set[first].1 == 1 {
                first += 1;
            }
            first = first.saturating_sub(1);
            let taste = self.taste(number, by_type);
            let share = f64::from(attacks) / 100.0 * set.len() as f64;
            let mut attacks_left = traits::round(share) as i64;
            let mut learnt: Vec<u16> = Vec::with_capacity(set.len());
            let mut attack = 0;
            for i in 0..set.len() {
                let chosen = self.pick_move(&pool, taste, &learnt, i == first || attacks_left > 0);
                if i == first {
                    attack = chosen;
                } else {
                    attacks_left -= 1;
                }
                learnt.push(chosen);
            }
            self.random.shuffle(&mut learnt);
            if learnt.get(first) != Some(&attack) {
                if let Some(at) = learnt.iter().position(|&m| m == attack) {
                    learnt.swap(at, first);
                }
            }
            for (entry, chosen) in set.iter_mut().zip(learnt) {
                entry.0 = chosen;
            }
            if let Some(entry) = set.get_mut(first) {
                entry.1 = 1;
            }
            all.push(set);
        }
        for (number, set) in all.iter().enumerate().skip(1) {
            let mut data = Vec::with_capacity(set.len() * 4 + 4);
            for &(learnt, level) in set {
                data.extend(learnt.to_le_bytes());
                data.extend(level.to_le_bytes());
            }
            data.extend([0xFF; 4]);
            files[Self::stats_file(number)] = data;
        }
        self.put_narc(LEARNSETS, &files)?;

        // And it hatches with random moves.
        let mut eggs = Self::narc(&self.nds, EGG_MOVES)?;
        let pool = self.move_pool(no_broken)?;
        for number in 1..=SPECIES {
            let data = eggs.get_mut(number).ok_or(layout(EGG_MOVES))?;
            let count = if data.len() >= 2 {
                usize::from(word(data, 0))
            } else {
                0
            };
            if data.len() < 2 + count * 2 {
                return Err(layout(EGG_MOVES));
            }
            let taste = self.taste(number, by_type);
            let mut attacks_left = traits::round(f64::from(attacks) / 100.0 * count as f64) as i64;
            let mut learnt: Vec<u16> = Vec::with_capacity(count);
            for _ in 0..count {
                let chosen = self.pick_move(&pool, taste, &learnt, attacks_left > 0);
                attacks_left -= 1;
                learnt.push(chosen);
            }
            self.random.shuffle(&mut learnt);
            for (i, chosen) in learnt.into_iter().enumerate() {
                set_word(data, 2 + i * 2, chosen);
            }
        }
        self.put_narc(EGG_MOVES, &eggs)
    }

    /// Every species knows Metronome and nothing else, trainers' Pokémon
    /// too; TMs and tutors teach it, and the HM moves cannot be used in
    /// battle.
    pub fn metronome_only(&mut self) -> Result<(), RomError> {
        let mut files = Self::narc(&self.nds, LEARNSETS)?;
        for number in 1..=SPECIES + FORMS {
            let data = files
                .get_mut(Self::stats_file(number))
                .ok_or(layout(LEARNSETS))?;
            if data.len() >= 4 && data[..4] != [0xFF; 4] {
                *data = vec![METRONOME as u8, 0, 1, 0, 0xFF, 0xFF, 0xFF, 0xFF];
            }
        }
        self.put_narc(LEARNSETS, &files)?;
        let mut trainers = self.trainers()?;
        for pokemon in trainers.iter_mut().flat_map(|t| &mut t.team) {
            pokemon.reset_moves = true;
        }
        self.set_trainers(&trainers)?;
        self.set_tm_moves(&[METRONOME; TMS])?;
        self.set_tutor_moves(&[METRONOME; TUTORS])?;
        let (_, hms) = self.machine_moves().ok_or(layout("moves of the TMs"))?;
        self.moves[usize::from(METRONOME)][PP] = 40;
        for hm in hms {
            self.moves[usize::from(hm)][PP] = 0;
        }
        Ok(())
    }

    /// The attacks of every species change places among themselves, so
    /// that it learns the weaker ones first.
    pub fn order_moves_by_damage(&mut self) -> Result<(), RomError> {
        let mut files = Self::narc(&self.nds, LEARNSETS)?;
        for number in 1..=SPECIES + FORMS {
            let data = files
                .get_mut(Self::stats_file(number))
                .ok_or(layout(LEARNSETS))?;
            let mut places = Vec::new();
            let mut attacks = Vec::new();
            for (i, entry) in data.chunks_exact(4).enumerate() {
                let (learnt, level) = (word(entry, 0), word(entry, 2));
                if (learnt, level) == (0xFFFF, 0xFFFF) {
                    break;
                }
                let power = self.moves.get(usize::from(learnt)).map_or(0, |m| m[POWER]);
                if level != 0 && power > 1 {
                    places.push(i * 4);
                    attacks.push(learnt);
                }
            }
            // Equally strong ones in random order.
            self.random.shuffle(&mut attacks);
            let strength = |m: &u16| {
                f64::from(self.moves[usize::from(*m)][POWER]) * moves::hits(usize::from(*m))
            };
            attacks.sort_by(|a, b| strength(a).total_cmp(&strength(b)));
            for (at, learnt) in places.into_iter().zip(attacks) {
                set_word(data, at, learnt);
            }
        }
        self.put_narc(LEARNSETS, &files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_has_its_place() {
        let mut places = TYPE_PLACES;
        places.sort_unstable();
        assert_eq!(places.to_vec(), (0..17).collect::<Vec<u8>>());
        // Grass is the fourth type the original randomizer lists.
        assert_eq!(TYPE_PLACES[11], 3);
    }
}
