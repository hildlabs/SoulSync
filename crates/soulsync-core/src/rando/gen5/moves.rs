//! What the moves are like: their power, accuracy, PP and type.

use super::*;

/// Where a move's data has its type, power, accuracy and PP.
const TYPE: usize = 0;
const POWER: usize = 3;
const ACCURACY: usize = 4;
const PP: usize = 5;
/// Moves that hit two to five times, those that hit twice, and Triple
/// Kick: their power is for one hit.
const SEVERAL_HITS: [usize; 13] = [292, 140, 198, 331, 4, 3, 31, 154, 333, 42, 350, 131, 541];
const TWO_HITS: [usize; 6] = [155, 458, 24, 530, 544, 41];
const TRIPLE_KICK: usize = 167;

/// How often a move hits on average.
fn hits(number: usize) -> f64 {
    if SEVERAL_HITS.contains(&number) {
        19.0 / 6.0
    } else if TWO_HITS.contains(&number) {
        2.0
    } else if number == TRIPLE_KICK {
        2.71
    } else {
        1.0
    }
}

impl Game<'_> {
    /// The moves, Struggle left out, with their numbers.
    fn changeable_moves(&mut self) -> impl Iterator<Item = (usize, &mut Vec<u8>)> {
        let moves = self.moves.iter_mut().enumerate().take(MOVES_COUNT + 1);
        moves.skip(1).filter(|(n, _)| *n != usize::from(STRUGGLE))
    }

    /// Every move that does damage gets a random power: mostly 50 to 100,
    /// sometimes anything from 20 to 150, rarely more.
    pub fn randomize_move_powers(&mut self) {
        let mut random = std::mem::replace(&mut self.random, JavaRandom::new(0));
        for (number, data) in self.changeable_moves() {
            if data[POWER] < 10 {
                continue;
            }
            let mut power = if random.index(3) != 2 {
                random.index(11) * 5 + 50
            } else {
                random.index(27) * 5 + 20
            };
            for _ in 0..2 {
                if random.index(100) == 0 {
                    power += 50;
                }
            }
            let hits = hits(number);
            if hits != 1.0 {
                // The power is for all hits together.
                power = traits::round(power as f64 / hits / 5.0) as usize * 5;
                if power == 0 {
                    power = 5;
                }
            }
            data[POWER] = power as u8;
        }
        self.random = random;
    }

    /// Every move that can miss gets a random accuracy, by how accurate
    /// it was: moves that seldom hit stay that way.
    pub fn randomize_move_accuracies(&mut self) {
        let mut random = std::mem::replace(&mut self.random, JavaRandom::new(0));
        for (_, data) in self.changeable_moves() {
            let old = data[ACCURACY];
            if old < 5 {
                continue;
            }
            let new = if old <= 50 {
                let accuracy = (random.index(7) * 5 + 20) as f64;
                if random.index(10) == 0 {
                    traits::round(accuracy * 3.0 / 2.0 / 5.0 * 5.0)
                } else {
                    accuracy
                }
            } else {
                // Down from 100 in steps of 5, stopping at each with a
                // chance that is higher for moves that were accurate.
                let stop = if old < 90 { 2 } else { 4 };
                let mut accuracy = 100.0;
                while accuracy > 20.0 && random.index(10) >= stop {
                    accuracy -= 5.0;
                }
                accuracy
            };
            data[ACCURACY] = new as u8;
        }
        self.random = random;
    }

    /// Every move gets random PP: mostly 15 to 25, sometimes 5 to 40.
    pub fn randomize_move_pps(&mut self) {
        let mut random = std::mem::replace(&mut self.random, JavaRandom::new(0));
        for (_, data) in self.changeable_moves() {
            data[PP] = if random.index(3) != 2 {
                random.index(3) * 5 + 15
            } else {
                random.index(8) * 5 + 5
            } as u8;
        }
        self.random = random;
    }

    /// Every move gets a random type.
    pub fn randomize_move_types(&mut self) {
        for number in (1..=MOVES_COUNT).filter(|&n| n != usize::from(STRUGGLE)) {
            let kind = self.random_type();
            self.moves[number][TYPE] = kind;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_hit_as_often_as_the_original_counts() {
        assert_eq!(hits(331), 19.0 / 6.0);
        assert_eq!(hits(24), 2.0);
        assert_eq!(hits(TRIPLE_KICK), 2.71);
        assert_eq!(hits(33), 1.0);
    }
}
