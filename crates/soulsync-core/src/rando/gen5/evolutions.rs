//! Random evolutions: what every species evolves into.

use super::*;

/// Where a species' data says how fast it gains levels.
const GROWTH: usize = 21;
const SHEDINJA: usize = 292;

/// What a random evolution has to be like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Evolving {
    /// About as strong as what the species evolved into before.
    pub similar_strength: bool,
    /// Sharing a type with the species.
    pub same_type: bool,
    /// No family grows beyond three stages.
    pub three_stages: bool,
    /// Never what the species evolved into before.
    pub force_change: bool,
}

/// The new evolutions while they are being made: what each species
/// evolves into and from.
struct Families {
    into: Vec<Vec<usize>>,
    from: Vec<Vec<usize>>,
    /// Whether a species had evolutions before.
    evolved: Vec<bool>,
}

impl Families {
    /// Whether `to` evolves into `from` in the end, so that the two
    /// would go round in a circle.
    fn circle(&self, from: usize, to: usize) -> bool {
        let mut seen = vec![false; self.into.len()];
        let mut left = vec![to];
        while let Some(species) = left.pop() {
            if species == from {
                return true;
            }
            if !std::mem::replace(&mut seen[species], true) {
                left.extend(&self.into[species]);
            }
        }
        false
    }

    /// The stages before a species, counted up to `limit`.
    fn stages_before(&self, species: usize, depth: usize, limit: usize) -> usize {
        if self.from[species].is_empty() {
            0
        } else if depth == limit - 1 {
            1
        } else {
            let before = self.from[species].iter();
            let longest = before.map(|&b| self.stages_before(b, depth + 1, limit) + 1);
            longest.max().unwrap_or(0)
        }
    }

    /// Whether a family would have more than `limit` stages, now or once
    /// all its members have their evolutions, if `from` evolved into `to`.
    fn too_long(&mut self, from: usize, to: usize, limit: usize) -> bool {
        self.into[from].push(to);
        self.from[to].push(from);
        let mut family = vec![from];
        let mut seen = vec![false; self.into.len()];
        seen[from] = true;
        let mut next = 0;
        while let Some(&species) = family.get(next) {
            next += 1;
            for &other in self.into[species].iter().chain(&self.from[species]) {
                if !std::mem::replace(&mut seen[other], true) {
                    family.push(other);
                }
            }
        }
        let exceeded = family.iter().any(|&species| {
            let before = self.stages_before(species, 0, limit);
            before >= limit
                || (before == limit - 1 && self.into[species].is_empty() && self.evolved[species])
        });
        self.into[from].pop();
        self.from[to].pop();
        exceeded
    }
}

impl Game<'_> {
    /// The two types of a species; the second is none if it has one only.
    fn types_of(&self, number: usize) -> (u8, Option<u8>) {
        let stats = &self.stats[number];
        let (first, second) = (stats[FIRST_TYPE], stats[FIRST_TYPE + 1]);
        (first, (second != first).then_some(second))
    }

    /// The sum of a species' base stats, as far as it says how strong it
    /// is: Shedinja's single HP does not count against it.
    pub(super) fn strength(&self, number: usize) -> i32 {
        let stats = &self.stats[Self::stats_file(number)];
        let sum = |range: std::ops::Range<usize>| -> i32 {
            stats[range].iter().map(|&s| i32::from(s)).sum()
        };
        if number == SHEDINJA {
            sum(1..6) * 6 / 5
        } else {
            sum(0..6)
        }
    }

    /// One of `choice` that is about as strong as `like`, rather one
    /// nobody evolves into yet.
    fn similar_evolution(&mut self, choice: &[usize], like: usize, taken: &[usize]) -> usize {
        let strength = self.strength(like);
        let (mut least, mut most) = (strength - strength / 10, strength + strength / 10);
        let mut fit: Vec<usize> = Vec::new();
        let mut spare: Vec<usize> = Vec::new();
        let mut rounds = 0;
        while fit.is_empty() || (fit.len() < 3 && rounds < 3) {
            for &species in choice {
                let own = self.strength(species);
                if (least..=most).contains(&own)
                    && !fit.contains(&species)
                    && !spare.contains(&species)
                {
                    if taken.contains(&species) {
                        spare.push(species);
                    } else {
                        fit.push(species);
                    }
                }
            }
            if rounds >= 2 && fit.is_empty() {
                fit.append(&mut spare);
            }
            least -= strength / 20;
            most += strength / 20;
            rounds += 1;
        }
        fit[self.random.index(fit.len())]
    }

    /// Every species evolves into random others, in the ways and as
    /// often as before, with the same speed of gaining levels and never
    /// in a circle.
    pub fn randomize_evolutions(&mut self, how: Evolving) -> Result<(), RomError> {
        let limit = if how.three_stages { 3 } else { 10 };
        let original: Vec<Vec<Evolution>> = (self.species.iter().take(SPECIES + 1))
            .map(|s| s.evolutions.clone())
            .collect();
        let was = |from: usize, to: usize| original[from].iter().any(|e| usize::from(e.to) == to);
        let mut families = Families {
            into: vec![Vec::new(); SPECIES + 1],
            from: vec![Vec::new(); SPECIES + 1],
            evolved: original.iter().map(|list| !list.is_empty()).collect(),
        };
        let mut order: Vec<usize> = (1..=SPECIES).collect();
        self.random.shuffle(&mut order);
        let mut taken: Vec<usize> = Vec::new();
        for &from in &order {
            let types = self.types_of(from);
            for (slot, evolution) in original[from].iter().enumerate() {
                let mut choice: Vec<usize> = Vec::new();
                for to in 1..=SPECIES {
                    if to == from
                        || self.stats[to][GROWTH] != self.stats[from][GROWTH]
                        || families.into[from].contains(&to)
                        || (how.force_change && was(from, to))
                        || families.circle(from, to)
                        || families.too_long(from, to, limit)
                    {
                        continue;
                    }
                    choice.push(to);
                }
                if choice.is_empty() {
                    return Err(RomError::Layout("evolutions"));
                }
                if choice.len() > 1 && how.same_type {
                    // Eevee's evolutions keep the types they had.
                    let old = self.types_of(usize::from(evolution.to)).0;
                    let alike: Vec<usize> = (choice.iter().copied())
                        .filter(|&to| {
                            let (first, second) = self.types_of(to);
                            if from == EEVEE {
                                first == old || second == Some(old)
                            } else {
                                first == types.0
                                    || Some(first) == types.1
                                    || second == Some(types.0)
                                    || (second.is_some() && second == types.1)
                            }
                        })
                        .collect();
                    if !alike.is_empty() {
                        choice = alike;
                    }
                }
                if !how.similar_strength && !choice.iter().all(|to| taken.contains(to)) {
                    choice.retain(|to| !taken.contains(to));
                }
                let picked = if choice.len() == 1 {
                    choice[0]
                } else if how.similar_strength {
                    self.similar_evolution(&choice, usize::from(evolution.to), &taken)
                } else {
                    choice[self.random.index(choice.len())]
                };
                taken.push(picked);
                // The original randomizer chooses a look for it, which
                // the game has no place for.
                let looks = usize::from(self.species[picked].looks);
                if looks > 0 {
                    self.random.index(looks);
                }
                self.species[from].evolutions[slot].to = picked as u16;
                families.into[from].push(picked);
                families.from[picked].push(from);
            }
        }
        self.evolved_in = Some(order);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn families(pairs: &[(usize, usize)]) -> Families {
        let mut families = Families {
            into: vec![Vec::new(); 10],
            from: vec![Vec::new(); 10],
            evolved: vec![false; 10],
        };
        for &(from, to) in pairs {
            families.into[from].push(to);
            families.from[to].push(from);
        }
        families
    }

    #[test]
    fn nobody_evolves_in_a_circle() {
        let all = families(&[(1, 2), (2, 3)]);
        assert!(all.circle(3, 1));
        assert!(all.circle(3, 2));
        assert!(!all.circle(1, 3));
        assert!(!all.circle(4, 1));
    }

    #[test]
    fn a_family_has_three_stages_at_most() {
        let mut all = families(&[(1, 2), (2, 3)]);
        assert!(all.too_long(3, 4, 3));
        assert!(!all.too_long(3, 4, 10));
        assert!(all.too_long(5, 1, 3));
        // A second stage that will still evolve makes three already.
        let mut all = families(&[(1, 2)]);
        assert!(!all.too_long(2, 3, 3));
        all.evolved[3] = true;
        assert!(all.too_long(2, 3, 3));
        assert_eq!(all.into[2], Vec::<usize>::new());
    }
}
