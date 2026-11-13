//! Better movesets for the trainers' Pokémon: four moves out of all a
//! Pokémon could know at its level, chosen so that they go well with
//! its types, stats and ability and with each other.

use super::movesets::TYPE_PLACES;
use super::trainer_items::hits_one;
use super::trainers::{rank, tags, Rank, TrainerPokemon};
use super::*;

/// Whose stats a move changes, and whether it also does damage.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Changes {
    None,
    User,
    Target,
    HitTarget,
    HitUser,
}

/// Whether a move only causes a status problem or does damage as well.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Inflicts {
    None,
    Only,
    Hit,
}

/// A change of one stat: which, by how many stages, how likely.
type StatChange = (u8, i8, i8);

/// What the choice needs to know about a move.
struct Move {
    power: i32,
    hits: f64,
    accuracy: i32,
    /// The type, as the original randomizer numbers them.
    kind: usize,
    category: u8,
    changes: Changes,
    stats: [StatChange; 3],
    inflicts: Inflicts,
    status: u8,
    status_chance: i32,
    flinch: u8,
    recoil: i32,
    absorb: i32,
    priority: i8,
    contact: bool,
    charge: bool,
    recharge: bool,
    punch: bool,
    trap: bool,
    /// 0 for the usual chance of a critical hit, 1 for a higher one, 2
    /// for a sure one.
    critical: u8,
}

const STATUS: u8 = 0;
const PHYSICAL: u8 = 1;
const SPECIAL: u8 = 2;

// The stats a move changes.
const NO_STAT: u8 = 0;
const ATTACK: u8 = 1;
const SPECIAL_ATTACK: u8 = 3;
const SPEED: u8 = 5;
const ACCURACY: u8 = 6;
const EVASION: u8 = 7;

// The status problems.
const NO_STATUS: u8 = 0;
const SLEEP: u8 = 2;
const POISON: u8 = 5;
const CONFUSION: u8 = 6;
const TOXIC: u8 = 7;
const TOXIC_MOVES: [usize; 2] = [92, 305];

// The types, as the original randomizer numbers them.
const WATER: usize = 4;
const FIRE: usize = 5;
const ROCK: usize = 6;
const ELECTRIC: usize = 11;
const ICE: usize = 14;
const TYPE_COUNT: usize = 17;

const TRAPPING_EFFECT: u16 = 106;
const TRAPPING_STATUS: u16 = 8;
const ABSORBS: u8 = 8;

/// Moves of no use in battle, or that the game does not use well.
const USELESS: [u16; 3] = [150, 100, 513];
/// Moves that are only of use where two fight side by side.
const FOR_DOUBLES: [u16; 7] = [266, 270, 476, 495, 502, 505, 511];
/// Moves that need another one: Spit Up, Swallow, Dream Eater, Nightmare.
const DEPENDENT: [u16; 4] = [255, 256, 138, 171];
/// Moves no stronger one makes pointless, and moves that make none so.
const NEVER_POINTLESS: [u16; 12] = [216, 218, 283, 175, 179, 237, 500, 265, 374, 68, 243, 162];
const NEVER_BETTER: [u16; 3] = [264, 153, 120];

/// The share of the moves of earlier stages, of TMs and tutors and of
/// egg moves that are thought of at all.
const EARLIER_MOVES: f64 = 0.5;
const TAUGHT_MOVES: f64 = 0.6;
const EGG_MOVES_SHARE: f64 = 0.1;
/// From this level on a Pokémon knows the moves it learnt on evolving.
const EVOLUTION_MOVES_FROM: u16 = 30;

impl Move {
    fn read(number: usize, data: &[u8]) -> Option<Self> {
        if data.len() < 36 {
            return None;
        }
        let quality = data[1];
        let target = data[20];
        let status = word(data, 8);
        let flags = u32::from_le_bytes([data[32], data[33], data[34], data[35]]);
        let drain = i32::from(data[18] as i8);
        let known = status < 7;
        Some(Move {
            power: i32::from(data[3]),
            hits: moves::hits(number),
            accuracy: i32::from(data[4]),
            kind: place(data[0]),
            category: data[2],
            changes: match quality {
                2 | 5 if target == 6 || target == 7 => Changes::User,
                2 | 5 => Changes::Target,
                6 => Changes::HitTarget,
                7 => Changes::HitUser,
                _ => Changes::None,
            },
            stats: [0, 1, 2].map(|i| (data[21 + i], data[24 + i] as i8, data[27 + i] as i8)),
            inflicts: match quality {
                1 | 5 if known => Inflicts::Only,
                4 if known => Inflicts::Hit,
                _ => Inflicts::None,
            },
            status: match status {
                5 if TOXIC_MOVES.contains(&number) => TOXIC,
                s if known => s as u8,
                _ => NO_STATUS,
            },
            status_chance: if known { i32::from(data[10]) } else { 0 },
            flinch: data[15],
            recoil: if quality == ABSORBS { 0 } else { -drain },
            absorb: if quality == ABSORBS { drain } else { 0 },
            priority: data[6] as i8,
            contact: flags & 1 != 0,
            charge: flags & 2 != 0,
            recharge: flags & 4 != 0,
            punch: flags & 0x80 != 0,
            trap: word(data, 16) == TRAPPING_EFFECT || status == TRAPPING_STATUS,
            critical: match data[14] {
                0 => 0,
                6 => 2,
                _ => 1,
            },
        })
    }

    fn damage(&self) -> f64 {
        f64::from(self.power) * self.hits
    }

    fn attacks(&self) -> bool {
        self.category != STATUS
    }

    /// Whether the move changes its user's stats.
    fn on_user(&self) -> bool {
        self.changes == Changes::User || self.changes == Changes::HitUser
    }

    fn changes_stat(&self, stat: u8, up: bool) -> bool {
        (self.stats.iter()).any(|&(which, stages, _)| which == stat && (up != (stages < 0)))
    }

    /// Whether the move changes speed in favour of its user.
    fn speeds(&self) -> bool {
        (self.changes == Changes::HitTarget && self.changes_stat(SPEED, false))
            || (self.on_user() && self.changes_stat(SPEED, true))
    }

    /// Whether the move raises a stat sharply, or more than one stat.
    fn raises_much(&self, least: i8) -> bool {
        self.stats[0].1 >= least || self.stats[1].0 != NO_STAT
    }

    fn good_attack(&self) -> bool {
        self.damage() >= 100.0 || (self.damage() >= 50.0 && self.accuracy >= 90)
    }
}

/// The place of a type of the game among the original randomizer's.
fn place(of: u8) -> usize {
    usize::from(TYPE_PLACES.get(usize::from(of)).copied().unwrap_or(0))
}

/// A Pokémon whose moves are chosen.
struct Fighter {
    types: (usize, Option<usize>),
    ability: u8,
    /// HP, Attack, Defense, Speed, Special Attack, Special Defense.
    stats: [f64; 6],
}

impl Fighter {
    fn is(&self, kind: usize) -> bool {
        self.types.0 == kind || self.types.1 == Some(kind)
    }
}

/// Everything the choice of moves is made from.
struct Known {
    moves: Vec<Option<Move>>,
    learnsets: Vec<Vec<(u16, u16)>>,
    eggs: Vec<Vec<u16>>,
    tms: Vec<u16>,
    tutors: Vec<u16>,
    /// What each species evolves from, if anything.
    before: Vec<Option<usize>>,
    /// The accuracy of a move that never misses.
    sure: i32,
    updated: bool,
}

impl Known {
    fn get(&self, number: u16) -> Option<&Move> {
        self.moves.get(usize::from(number))?.as_ref()
    }

    /// The moves of `list` that `is` holds for, each once.
    fn those(&self, list: &[u16], is: impl Fn(u16, &Move) -> bool) -> Vec<u16> {
        let mut found = Vec::new();
        for &number in list {
            let fits = self.get(number).is_some_and(|m| is(number, m));
            if fits && !found.contains(&number) {
                found.push(number);
            }
        }
        found
    }

    /// Moves that an ability makes better.
    fn for_ability(&self, list: &[u16], who: &Fighter) -> Vec<u16> {
        let sure = self.sure;
        self.those(list, |n, m| {
            let one_of = |numbers: &[u16]| numbers.contains(&n);
            match who.ability {
                2 => one_of(&[87, 542]) || (n == 311 && who.is(WATER)),
                3 => one_of(&[226, 500]),
                5 => one_of(&[283, 68, 243, 175, 179]),
                8 | 146 => n == 201,
                9 => one_of(&[265, 506]),
                14 => m.accuracy > 0 && m.accuracy <= 80,
                20 | 77 => one_of(&[80, 37, 200]),
                23 | 71 | 43 => n == 195,
                38 => one_of(&[474, 506]),
                27 | 49 => n == 506,
                32 => {
                    let hits = m.changes == Changes::HitTarget || m.changes == Changes::HitUser;
                    (hits && m.stats[0].2 < 100)
                        || (m.inflicts == Inflicts::Hit && m.status_chance < 100)
                        || m.flinch > 0
                }
                33 | 44 | 87 | 93 => n == 240,
                34 | 139 | 102 => n == 241,
                45 => n == 311 && who.is(ROCK),
                48 | 61 => n == 156,
                54 => n == 144,
                55 => {
                    (m.on_user() && m.changes_stat(ACCURACY, true))
                        || (m.category == PHYSICAL && m.accuracy == sure)
                }
                62 => n == 263,
                69 => m.recoil > 0,
                70 => one_of(&[76, 234, 235, 236, 74]) || (n == 311 && who.is(FIRE)),
                89 => m.punch,
                81 | 115 => n == 258,
                84 => one_of(&[374, 512]),
                86 => (m.on_user() && m.stats[0].1 > 0) || n == 367,
                91 => m.attacks() && who.is(m.kind),
                92 => m.hits >= 3.0,
                97 => m.critical > 0,
                98 => m.recoil > 0,
                100 => one_of(&[368, 371]),
                105 => m.critical == 1,
                148 => m.power > 0 && m.priority < 0,
                99 => m.accuracy > 0 && m.accuracy <= 70,
                101 => (40..=60).contains(&m.power) || m.hits > 1.0,
                112 => one_of(&[144, 182, 197, 19, 91, 340, 291]),
                117 => n == 59 || (n == 311 && who.is(ICE)),
                120 => m.recoil > 0 || one_of(&[26, 136]),
                123 => m.status == SLEEP,
                125 => {
                    m.changes == Changes::HitTarget
                        || (m.changes == Changes::HitUser && m.stats[0].1 > 0)
                        || m.inflicts == Inflicts::Hit
                        || m.flinch > 0
                }
                126 => m.changes == Changes::HitUser && m.stats[0].1 < 0,
                134 => one_of(&[535, 484]),
                141 => n == 500,
                143 => m.contact,
                144 => one_of(&[369, 521]),
                158 => m.inflicts == Inflicts::Only || one_of(&[194, 227, 115, 113, 288, 220, 164]),
                _ => false,
            }
        })
    }

    /// Moves that an ability makes a little better: those of the type of
    /// the weather it brings.
    fn for_weather(&self, list: &[u16], ability: u8, against: bool) -> Vec<u16> {
        self.those(list, |_, m| {
            let kind = match (ability, against) {
                (2, false) | (70, true) => WATER,
                (70, false) | (2, true) => FIRE,
                _ => return false,
            };
            m.kind == kind && m.attacks()
        })
    }

    /// Moves that an ability makes pointless.
    fn not_for_ability(&self, list: &[u16], ability: u8) -> Vec<u16> {
        self.those(list, |n, m| {
            let one_of = |numbers: &[u16]| numbers.contains(&n);
            match ability {
                2 | 45 | 117 => one_of(&[76, 234, 235, 236, 240, 241, 258, 201]),
                3 => {
                    (m.changes == Changes::User && m.stats[0].0 == SPEED && m.stats[0].1 > 0)
                        || one_of(&[244, 114])
                }
                70 => one_of(&[87, 542, 240, 241, 258, 201]),
                99 => {
                    one_of(&[199, 170])
                        || m.changes_stat(ACCURACY, true)
                        || m.changes_stat(EVASION, true)
                        || m.changes_stat(EVASION, false)
                }
                6 => one_of(&[120, 153]),
                15 | 72 => n == 156,
                76 | 13 => one_of(&[240, 241, 201, 258]),
                86 => m.on_user() && m.stats[0].1 < 0,
                126 => (m.on_user() && m.stats[0].1 > 0) || n == 504,
                135 => one_of(&[535, 484]),
                _ => false,
            }
        })
    }

    /// Moves that go well, or with `against` badly, with a Pokémon's stats.
    fn for_stats(&self, list: &[u16], who: &Fighter, against: bool) -> Vec<u16> {
        let [hp, attack, defense, speed, special_attack, special_defense] = who.stats;
        let total: f64 = who.stats.iter().sum();
        let attacks = (attack + special_attack) / total;
        let defenses = (defense + special_defense) / total;
        let mut fit: Vec<u16> = Vec::new();
        let mut when = |holds: bool, numbers: &[u16]| {
            if holds {
                fit.extend(numbers);
            }
        };
        if against {
            when(hp / total >= 1.0 / 4.0, &[220, 283]);
            when(defense * 2.0 < attack, &[379]);
            when(attacks >= 1.0 / 3.0, &[471]);
            when(defenses >= 1.0 / 3.0, &[470]);
            when(speed / total >= 1.0 / 4.0, &[360]);
            when(speed / total < 1.0 / 8.0, &[486]);
        } else {
            when(hp / total < 1.0 / 8.0, &[220, 283]);
            when(hp / total >= 1.0 / 4.0, &[323, 284, 68, 243]);
            when(attack * 2.0 < defense, &[379]);
            when(attacks < 1.0 / 4.0, &[471]);
            when(defenses < 1.0 / 4.0, &[470]);
            when(speed / total < 1.0 / 8.0, &[360]);
            when(speed / total >= 1.0 / 4.0, &[486]);
        }
        self.those(list, |n, _| fit.contains(&n))
    }

    /// Moves that go very well with the move `with`.
    fn for_move(&self, list: &[u16], with: u16) -> Vec<u16> {
        let Some(first) = self.get(with) else {
            return Vec::new();
        };
        self.those(list, |n, m| {
            let one_of = |numbers: &[u16]| numbers.contains(&n);
            (first.speeds() && m.flinch > 0 && m.priority == 0)
                || (first.flinch > 0 && first.priority == 0 && m.speeds())
                || (first.raises_much(2) && one_of(&[226, 500]))
                || (first.status == SLEEP && one_of(&[138, 171, 506]))
                || match with {
                    92 => one_of(&[182, 197, 91, 19, 340, 291, 474, 506]) || m.trap,
                    77 | 139 => one_of(&[474, 506]),
                    474 => m.category == STATUS && (m.status == POISON || m.status == TOXIC),
                    182 | 197 => one_of(&[92, 73, 261]),
                    226 => m.raises_much(2) || n == 504,
                    261 => n == 506,
                    199 | 170 => m.accuracy <= 50,
                    241 => n == 76,
                    240 => one_of(&[87, 542]),
                    384 => {
                        m.changes == Changes::HitUser
                            && (m.changes_stat(ATTACK, false)
                                || m.changes_stat(SPECIAL_ATTACK, false))
                    }
                    203 => one_of(&[179, 175, 283]),
                    283 => m.attacks() && m.priority > 0,
                    86 | 137 | 78 => {
                        m.flinch > 0 || (m.category == STATUS && m.status == CONFUSION) || n == 506
                    }
                    258 => n == 59,
                    254 => one_of(&[255, 256]),
                    255 | 256 => n == 254,
                    73 | 195 => m.trap,
                    191 | 446 | 390 => one_of(&[46, 18, 525, 509]),
                    156 => n == 214,
                    116 => m.critical == 1,
                    264 | 138 | 171 => m.inflicts == Inflicts::Only && m.status == SLEEP,
                    259 => n == 227,
                    227 => n == 259,
                    506 => m.inflicts == Inflicts::Only && m.status != CONFUSION,
                    500 => m.raises_much(2) || one_of(&[367, 504]),
                    207 => n == 386,
                    386 => n == 207,
                    504 => n == 500,
                    _ => false,
                }
        })
    }

    /// Moves that go a little better with the move `with`: attacks that
    /// hit hard what resists it.
    fn beside_move(&self, list: &[u16], with: u16) -> Vec<u16> {
        let Some(first) = self.get(with) else {
            return Vec::new();
        };
        let mut covers = [false; TYPE_COUNT];
        if first.attacks() {
            for resists in (0..TYPE_COUNT).filter(|&t| hits_one(first.kind, t, self.updated) < 2) {
                for (attack, cover) in covers.iter_mut().enumerate() {
                    *cover |= hits_one(attack, resists, self.updated) == 4;
                }
            }
        }
        self.those(list, |_, m| {
            (m.attacks() && covers[m.kind])
                || match with {
                    14 => m.category == PHYSICAL,
                    417 | 294 => m.category == SPECIAL,
                    _ => false,
                }
        })
    }

    /// Moves that make no sense beside the move `with`.
    fn not_with_move(&self, list: &[u16], with: u16) -> Vec<u16> {
        let Some(first) = self.get(with) else {
            return Vec::new();
        };
        let ails = |m: &Move| m.category == STATUS && m.status != NO_STATUS;
        self.those(list, |n, m| {
            (ails(first)
                && ails(m)
                && (m.status == first.status
                    || (first.status != CONFUSION && m.status != CONFUSION)))
                || match with {
                    182 => n == 197,
                    197 => n == 182,
                    216 => n == 218,
                    218 => n == 216,
                    73 | 195 => [18, 46, 509, 525].contains(&n),
                    _ => false,
                }
                || (first.attacks() && first.kind == FIRE && n == 346)
                || (first.attacks() && first.kind == ELECTRIC && n == 300)
        })
    }

    /// Moves that are worth less beside the move `with`.
    fn less_with_move(&self, list: &[u16], with: u16) -> Vec<u16> {
        let Some(first) = self.get(with) else {
            return Vec::new();
        };
        self.those(list, |_, m| {
            m.attacks()
                && ((first.attacks() && m.kind == first.kind)
                    || (with == 346 && m.kind == FIRE)
                    || (with == 300 && m.kind == ELECTRIC))
        })
    }

    /// The moves of `list` one of which the move `needy` needs.
    fn needed_by(&self, list: &[u16], needy: u16) -> Vec<u16> {
        self.those(list, |n, m| match needy {
            255 | 256 => n == 254,
            138 | 171 => m.category == STATUS && m.status == SLEEP,
            _ => false,
        })
    }

    /// The moves of `list` that another one of it is plainly better than.
    fn pointless(&self, list: &[u16]) -> Vec<u16> {
        let mut pointless = Vec::new();
        let pairs = |n: u16| self.get(n).map(|m| (n, m));
        for (number, m) in list.iter().filter_map(|&n| pairs(n)) {
            if NEVER_BETTER.contains(&number) {
                continue;
            }
            let helps = (m.changes == Changes::HitTarget && m.stats[0].1 < 0)
                || (m.changes == Changes::HitUser && m.stats[0].1 > 0);
            let changing = |m: &Move| -> Vec<StatChange> {
                (m.stats.iter().copied())
                    .filter(|c| c.0 != NO_STAT)
                    .collect()
            };
            for (other, o) in list.iter().filter_map(|&n| pairs(n)) {
                let worse = if m.power > 0 {
                    let same = (m.changes == o.changes && m.stats[0] == o.stats[0])
                        || (o.changes == Changes::None && helps);
                    !NEVER_POINTLESS.contains(&other)
                        && m.kind == o.kind
                        && ((same && m.absorb >= o.absorb && !m.charge && !m.recharge)
                            || o.damage() <= 30.0)
                        && m.accuracy >= o.accuracy
                        && m.category == o.category
                        && m.priority >= o.priority
                        && o.power > 0
                        && m.damage() > o.damage()
                } else if m.changes == Changes::User || m.changes == Changes::Target {
                    let alike = other != number
                        && o.power <= 0
                        && o.changes == m.changes
                        && (o.status == m.status || o.status == NO_STATUS);
                    let mine = changing(m);
                    let mut theirs = changing(o);
                    if !alike || theirs.len() > mine.len() {
                        continue;
                    }
                    let more: Vec<StatChange> = (mine.iter().copied())
                        .filter(|c| !theirs.contains(c))
                        .collect();
                    theirs.retain(|c| !mine.contains(c));
                    let better = if theirs.is_empty() {
                        !more.is_empty()
                    } else {
                        // Every change that differs has to be the
                        // stronger one.
                        let mut better = false;
                        for &(stat, stages, _) in &more {
                            let mut matched = false;
                            for &(_, their_stages, _) in theirs.iter().filter(|c| c.0 == stat) {
                                matched = if m.changes == Changes::User {
                                    stages > their_stages
                                } else {
                                    stages < their_stages
                                };
                                better |= matched;
                            }
                            if !matched {
                                better = false;
                                break;
                            }
                        }
                        better
                    };
                    better && !NEVER_POINTLESS.contains(&other)
                } else {
                    false
                };
                if worse && !pointless.contains(&other) {
                    pointless.push(other);
                }
            }
        }
        pointless
    }

    /// Without the moves that are useless, pointless or impossible to
    /// use. Stops as soon as four or fewer are left.
    fn trimmed(&self, mut list: Vec<u16>, ability: u8, doubles: bool) -> Vec<u16> {
        if list.len() <= 4 {
            return list;
        }
        list.retain(|n| !USELESS.contains(n) && (doubles || !FOR_DOUBLES.contains(n)));
        if list.len() <= 4 {
            return list;
        }
        let pointless = self.pointless(&list);
        list.retain(|n| !pointless.contains(n));
        if list.len() <= 4 {
            return list;
        }
        for needy in list.clone().into_iter().filter(|n| DEPENDENT.contains(n)) {
            if self.needed_by(&list, needy).is_empty() {
                remove_one(&mut list, needy);
            }
        }
        if list.len() <= 4 {
            return list;
        }
        let bad = self.not_for_ability(&list, ability);
        if list.iter().any(|n| !bad.contains(n)) {
            list.retain(|n| !bad.contains(n));
        }
        list
    }
}

/// Each move of `list` once, in the order they first come up.
fn distinct(list: &[u16]) -> Vec<u16> {
    let mut once = Vec::new();
    for &number in list {
        if !once.contains(&number) {
            once.push(number);
        }
    }
    once
}

fn remove_one(list: &mut Vec<u16>, number: u16) {
    if let Some(at) = list.iter().position(|&n| n == number) {
        list.remove(at);
    }
}

/// The first four moves of `list`.
fn four(list: &[u16]) -> [u16; 4] {
    [0, 1, 2, 3].map(|i| list.get(i).copied().unwrap_or(0))
}

impl Game<'_> {
    fn known(&self) -> Result<Known, RomError> {
        let (tms, _) = self.machine_moves().ok_or(layout("moves of the TMs"))?;
        let eggs = Self::narc(&self.nds, movesets::EGG_MOVES)?;
        let eggs = (eggs.iter().take(SPECIES + 1))
            .map(|data| {
                let count = if data.len() >= 2 {
                    usize::from(word(data, 0))
                } else {
                    0
                };
                let moves = data.get(2..).unwrap_or(&[]).chunks_exact(2);
                moves.take(count).map(|m| word(m, 0)).collect()
            })
            .collect();
        let mut before = vec![None; SPECIES + FORMS + 1];
        let numbers: Vec<usize> = (1..=SPECIES).collect();
        for &from in self.evolved_in.as_ref().unwrap_or(&numbers) {
            for evolution in &self.species[from].evolutions {
                if let Some(first) = before.get_mut(usize::from(evolution.to)) {
                    first.get_or_insert(from);
                }
            }
        }
        Ok(Known {
            moves: (self.moves.iter().enumerate())
                .map(|(number, data)| Move::read(number, data).filter(|_| number > 0))
                .collect(),
            learnsets: self.learnsets()?,
            eggs,
            tms,
            tutors: self.tutor_moves()?,
            before,
            sure: i32::from(self.sure_accuracy),
            updated: self.effectiveness_updated,
        })
    }

    /// Every move a trainer's Pokémon could know: what it and, by
    /// chance, the stages before it learn up to its level, some of what
    /// it can be taught and now and then a move it hatched with.
    fn possible_moves(
        &mut self,
        known: &Known,
        pokemon: &TrainerPokemon,
        cyclic: bool,
    ) -> Vec<u16> {
        let own = self.form_of(pokemon.species, pokemon.form);
        let level = pokemon.level;
        let mut pool: Vec<u16> = Vec::new();
        for &(learnt, at) in &known.learnsets[own] {
            if (at != 0 && at <= level) || (at == 0 && level >= EVOLUTION_MOVES_FROM) {
                pool.push(learnt);
            }
        }
        let mut pool = distinct(&pool);
        let mut first = pokemon.species;
        while let Some(earlier) = known.before[first].filter(|_| !cyclic) {
            first = earlier;
            let mut more = Vec::new();
            for &(learnt, at) in &known.learnsets[earlier] {
                if at <= level && self.random.double() < EARLIER_MOVES {
                    more.push(learnt);
                }
            }
            pool.extend(distinct(&more));
        }

        let stats = self.stats[Self::stats_file(own)].clone();
        let flag = |at: usize, bit: usize| {
            stats
                .get(at + bit / 8)
                .is_some_and(|b| b >> (bit % 8) & 1 != 0)
        };
        let learns_tm = |tm: usize| flag(MACHINES, tm);
        let learns_tutor = |tutor: usize| {
            let blocks = machines::TUTOR_BLOCKS.iter().enumerate();
            let mut fits =
                blocks.filter(|(_, &(count, first))| (first..first + count).contains(&tutor));
            fits.next().is_some_and(|(block, &(_, first))| {
                flag(machines::TUTOR_FLAGS + block * 4, tutor - first)
            })
        };
        let taught = [(&known.tms, true), (&known.tutors, false)];
        for (list, tm) in taught {
            for &teaches in list.iter() {
                let at = list.iter().position(|&m| m == teaches).unwrap_or(0);
                let learns = if tm { learns_tm(at) } else { learns_tutor(at) };
                let Some(m) = known.get(teaches).filter(|_| learns) else {
                    continue;
                };
                let level = i32::from(level);
                let takes = if m.power > 1
                    && f64::from(level * 3) > m.damage()
                    && self.random.double() < TAUGHT_MOVES
                {
                    true
                } else {
                    (m.power <= 1 && self.random.below(100) < level)
                        || self.random.below(200) < level
                };
                if takes {
                    pool.push(teaches);
                }
            }
        }
        if !cyclic {
            for &hatched in known.eggs.get(first).map_or(&[][..], |e| e) {
                if self.random.double() < EGG_MOVES_SHARE {
                    pool.push(hatched);
                }
            }
        }
        pool.retain(|&n| known.get(n).is_some());
        distinct(&pool)
    }

    /// Adds some of `liked`, in random order, to `list` once more; how
    /// many is `bias` times their number.
    fn favour(&mut self, list: &mut Vec<u16>, mut liked: Vec<u16>, bias: f64) {
        self.random.shuffle(&mut liked);
        let mut i = 0;
        while (i as f64) < bias * liked.len() as f64 {
            list.push(liked[i % liked.len()]);
            i += 1;
        }
    }

    /// The four moves for one Pokémon of a trainer of the weight `weight`.
    fn better_moves(
        &mut self,
        known: &Known,
        pokemon: &TrainerPokemon,
        weight: f64,
        cyclic: bool,
        doubles: bool,
    ) -> [u16; 4] {
        let own = self.form_of(pokemon.species, pokemon.form);
        let stats = &self.stats[Self::stats_file(own)];
        let types = (stats[FIRST_TYPE], stats[FIRST_TYPE + 1]);
        let who = Fighter {
            types: (place(types.0), (types.1 != types.0).then(|| place(types.1))),
            // A form has the abilities of its species here.
            ability: match pokemon.ability {
                slot @ 1..=3 => self.species[pokemon.species].abilities[usize::from(slot) - 1],
                _ => 0,
            },
            stats: [0, 1, 2, 3, 4, 5].map(|i| f64::from(stats[i])),
        };
        let pool = self.possible_moves(known, pokemon, cyclic);
        let mut list = known.trimmed(pool, who.ability, doubles);
        if list.len() <= 4 {
            return four(&list);
        }

        let bonus = weight * (list.len() as f64 / 10.0);
        let attack = |n: &u16| known.get(*n).is_some_and(|m| m.attacks());
        let of_type = |list: &[u16], kind: usize| -> Vec<u16> {
            let fits = |n: &&u16| attack(n) && known.get(**n).is_some_and(|m| m.kind == kind);
            list.iter().filter(fits).copied().collect()
        };
        let own_type = of_type(&list, who.types.0);
        self.favour(&mut list, own_type, 0.25 * bonus);
        if let Some(second) = who.types.1 {
            let own_type = of_type(&list, second);
            self.favour(&mut list, own_type, 0.25 * bonus);
        }
        let liked = known.for_ability(&list, &who);
        self.favour(&mut list, liked, bonus);
        let liked = known.for_weather(&list, who.ability, false);
        self.favour(&mut list, liked, 0.5 * bonus);
        // What suits the Pokémon badly counts once less, unless nothing
        // would be left.
        let once_less = |list: &mut Vec<u16>, bad: Vec<u16>| {
            let mut without = list.clone();
            for number in bad {
                remove_one(&mut without, number);
            }
            if !without.is_empty() {
                *list = without;
            }
        };
        let bad = known.for_weather(&list, who.ability, true);
        once_less(&mut list, bad);
        if distinct(&list).len() <= 4 {
            return four(&distinct(&list));
        }
        let liked = known.for_stats(&list, &who, false);
        self.favour(&mut list, liked, 0.5 * bonus);
        let bad = known.for_stats(&list, &who, true);
        once_less(&mut list, bad);
        if distinct(&list).len() <= 4 {
            return four(&distinct(&list));
        }

        // As many physical and special moves as suits its stats.
        let mut ratio = who.stats[1] / who.stats[4];
        ratio *= match who.ability {
            37 | 74 => 2.0,
            55 => 1.5,
            153 => 1.1,
            _ => 1.0,
        };
        let of = |list: &[u16], category: u8| -> Vec<u16> {
            let fits = |n: &&u16| known.get(**n).is_some_and(|m| m.category == category);
            list.iter().filter(fits).copied().collect()
        };
        let (physical, special) = (of(&list, PHYSICAL), of(&list, SPECIAL));
        let (more, fewer) = if ratio < 1.0 && !special.is_empty() {
            ratio = 1.0 / ratio;
            (special, physical.len())
        } else {
            (physical, special.len())
        };
        if !more.is_empty() {
            let wanted = (fewer as f64 * (0.75 * ratio)) as i32 - more.len() as i32;
            for _ in 0..wanted {
                list.push(more[self.random.index(more.len())]);
            }
        }

        let mut picked: Vec<u16> = Vec::new();
        for turn in 1..=4 {
            let good: Vec<u16> = if turn == 1 {
                let good = |n: &&u16| known.get(**n).is_some_and(|m| m.good_attack());
                list.iter().filter(good).copied().collect()
            } else {
                Vec::new()
            };
            if turn == 4 {
                for needy in distinct(&list)
                    .into_iter()
                    .filter(|n| DEPENDENT.contains(n))
                {
                    let needed = known.needed_by(&list, needy);
                    if !needed.iter().any(|n| picked.contains(n)) {
                        list.retain(|&n| n != needy);
                    }
                }
            }
            let from = if good.is_empty() { &list } else { &good };
            if from.is_empty() {
                break;
            }
            let chosen = from[self.random.index(from.len())];
            picked.push(chosen);
            if turn == 4 {
                break;
            }
            let left = 4 - turn;
            list.retain(|&n| n != chosen);
            let bad = known.not_with_move(&list, chosen);
            list.retain(|n| !bad.contains(n));
            if distinct(&list).len() <= left {
                picked.extend(distinct(&list));
                break;
            }
            let liked = known.for_move(&list, chosen);
            self.favour(&mut list, liked, bonus);
            let liked = known.beside_move(&list, chosen);
            self.favour(&mut list, liked, 0.25 * bonus);
            let mut less = known.less_with_move(&list, chosen);
            self.random.shuffle(&mut less);
            let mut i = 0;
            while (i as f64) < 0.5 * less.len() as f64 {
                if distinct(&list).len() <= left {
                    break;
                }
                remove_one(&mut list, less[i % less.len()]);
                i += 1;
            }
            if distinct(&list).len() <= left {
                picked.extend(distinct(&list));
                break;
            }
        }
        four(&picked)
    }

    /// Every trainer's Pokémon gets four moves that suit it, out of all
    /// it could know at its level. `cyclic` is for evolutions that go
    /// round in circles, `doubles` for a game of double battles only.
    pub fn pick_trainer_moves(&mut self, cyclic: bool, doubles: bool) -> Result<(), RomError> {
        let mut all = self.trainers()?;
        let known = self.known()?;
        let tags = tags(all.len());
        for (place, trainer) in all.iter_mut().enumerate() {
            trainer.own_moves();
            let weight = match rank(&tags, place, trainer) {
                Some(Rank::Regular) => 1.0,
                Some(Rank::Boss) => 2.0,
                _ => 1.5,
            };
            for pokemon in &mut trainer.team {
                pokemon.reset_moves = false;
                pokemon.moves = self.better_moves(&known, pokemon, weight, cyclic, doubles);
            }
        }
        self.set_trainers(&all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_move_counts_once() {
        assert_eq!(distinct(&[5, 3, 5, 7, 3]), [5, 3, 7]);
        assert_eq!(four(&[9, 8]), [9, 8, 0, 0]);
        let mut list = vec![1, 2, 1];
        remove_one(&mut list, 1);
        assert_eq!(list, [2, 1]);
    }
}
