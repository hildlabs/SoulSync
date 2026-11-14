//! What the moves are like: their power, accuracy, PP and type.

use super::*;

/// Where a move's data has its type, power, accuracy and PP.
const TYPE: usize = 0;
const POWER: usize = 3;
const ACCURACY: usize = 4;
const PP: usize = 5;
/// Whether a move is an attack, and which stats it works with.
const CATEGORY: usize = 2;
const STATUS: u8 = 0;
const PHYSICAL: u8 = 1;
const SPECIAL: u8 = 2;
/// Moves that hit two to five times, those that hit twice, and Triple
/// Kick: their power is for one hit.
const SEVERAL_HITS: [usize; 13] = [292, 140, 198, 331, 4, 3, 31, 154, 333, 42, 350, 131, 541];
const TWO_HITS: [usize; 6] = [155, 458, 24, 530, 544, 41];
const TRIPLE_KICK: usize = 167;

/// How often a move hits on average.
pub(super) fn hits(number: usize) -> f64 {
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

/// What later games changed about moves: the generation, the move, and
/// its power, accuracy and PP where they changed. Accuracy 101 is a move
/// that never misses.
type Newer = (u8, u16, Option<u8>, Option<u8>, Option<u8>);
const NEWER_MOVES: [Newer; 82] = [
    (6, 14, None, None, Some(20)),
    (6, 18, None, Some(101), None),
    (6, 22, Some(45), None, Some(25)),
    (6, 42, Some(25), Some(95), None),
    (6, 53, Some(90), None, None),
    (6, 56, Some(110), None, None),
    (6, 57, Some(90), None, None),
    (6, 58, Some(90), None, None),
    (6, 59, Some(110), None, None),
    (6, 74, None, None, Some(20)),
    (6, 85, Some(90), None, None),
    (6, 87, Some(110), None, None),
    (6, 107, None, None, Some(10)),
    (6, 112, None, None, Some(20)),
    (6, 122, Some(30), None, None),
    (6, 123, Some(30), None, None),
    (6, 126, Some(110), None, None),
    (6, 130, Some(130), None, Some(10)),
    (6, 137, None, Some(100), None),
    (6, 139, None, Some(90), None),
    (6, 145, Some(40), None, None),
    (6, 149, None, Some(100), None),
    (6, 151, None, None, Some(20)),
    (6, 152, Some(100), None, None),
    (6, 168, Some(60), None, Some(25)),
    (6, 173, Some(50), None, None),
    (6, 210, Some(40), None, None),
    (6, 248, Some(120), None, None),
    (6, 257, Some(95), None, None),
    (6, 261, None, Some(85), None),
    (6, 265, Some(70), None, None),
    (6, 282, Some(65), None, None),
    (6, 309, Some(90), Some(90), None),
    (6, 314, Some(60), None, None),
    (6, 315, Some(130), None, None),
    (6, 317, Some(60), Some(95), Some(15)),
    (6, 326, None, None, Some(20)),
    (6, 330, Some(90), None, None),
    (6, 343, None, None, Some(25)),
    (6, 358, Some(70), None, None),
    (6, 366, None, None, Some(15)),
    (6, 372, Some(60), None, None),
    (6, 375, None, Some(100), None),
    (6, 396, Some(80), None, None),
    (6, 403, None, None, Some(15)),
    (6, 406, Some(85), None, None),
    (6, 408, Some(80), None, None),
    (6, 412, Some(90), None, None),
    (6, 434, Some(130), None, None),
    (6, 437, Some(130), None, None),
    (6, 441, None, Some(80), None),
    (6, 448, Some(65), None, None),
    (6, 463, Some(100), None, None),
    (6, 480, Some(45), None, None),
    (6, 485, Some(120), None, None),
    (6, 490, Some(65), None, None),
    (6, 506, Some(65), None, None),
    (6, 510, Some(60), None, None),
    (6, 518, Some(80), None, None),
    (6, 519, Some(80), None, None),
    (6, 520, Some(80), None, None),
    (6, 522, Some(50), None, None),
    (6, 524, Some(45), None, None),
    (6, 533, None, None, Some(15)),
    (6, 542, Some(110), None, None),
    (6, 546, Some(120), None, None),
    (7, 33, Some(40), None, None),
    (7, 66, None, None, Some(20)),
    (7, 86, None, Some(90), None),
    (7, 141, Some(80), None, Some(10)),
    (7, 207, None, Some(85), None),
    (7, 389, Some(70), None, None),
    (7, 464, None, Some(50), None),
    (8, 229, Some(50), None, None),
    (9, 105, None, None, Some(5)),
    (9, 135, None, None, Some(5)),
    (9, 156, None, None, Some(5)),
    (9, 208, None, None, Some(5)),
    (9, 295, Some(95), None, None),
    (9, 296, Some(95), None, None),
    (9, 303, None, None, Some(5)),
    (9, 355, None, None, Some(5)),
];

impl Game<'_> {
    /// The moves become what they are in the games up to `generation`.
    pub fn update_moves(&mut self, generation: u8) {
        for (from, number, power, accuracy, pp) in NEWER_MOVES {
            if from > generation {
                continue;
            }
            let data = &mut self.moves[usize::from(number)];
            if let Some(power) = power {
                data[POWER] = power;
            }
            if let Some(accuracy) = accuracy {
                data[ACCURACY] = if accuracy == 101 {
                    self.sure_accuracy
                } else {
                    accuracy
                };
            }
            if let Some(pp) = pp {
                data[PP] = pp;
            }
        }
    }

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

    /// Every attack is physical or special by the toss of a coin.
    pub fn randomize_move_categories(&mut self) {
        for number in (1..=MOVES_COUNT).filter(|&n| n != usize::from(STRUGGLE)) {
            let category = self.moves[number][CATEGORY];
            if category != STATUS && self.random.index(2) == 0 {
                self.moves[number][CATEGORY] = if category == PHYSICAL {
                    SPECIAL
                } else {
                    PHYSICAL
                };
            }
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

    #[test]
    fn newer_moves_come_in_the_order_of_the_games() {
        assert!(NEWER_MOVES.is_sorted_by_key(|m| (m.0, m.1)));
        assert!(NEWER_MOVES.iter().all(|m| (6..=9).contains(&m.0)));
        assert!(NEWER_MOVES.iter().all(|m| usize::from(m.1) <= MOVES_COUNT));
        // Thunderbolt lost power in the sixth generation, Tackle in the
        // seventh; Whirlwind never misses since the sixth.
        assert!(NEWER_MOVES.contains(&(6, 85, Some(90), None, None)));
        assert!(NEWER_MOVES.contains(&(7, 33, Some(40), None, None)));
        assert!(NEWER_MOVES.contains(&(6, 18, None, Some(101), None)));
    }
}
