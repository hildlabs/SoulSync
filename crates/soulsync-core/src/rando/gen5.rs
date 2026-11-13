//! The randomizer for Black 2 and White 2.
//!
//! [`Game::load`] reads what the original randomizer reads when it opens
//! a ROM and [`Game::save`] writes what it writes when it saves one. That
//! alone already changes a ROM a little: text files are stored unpacked,
//! held items and evolutions the randomizer has no use for are dropped,
//! and the code it looked at is packed anew.

use super::random::JavaRandom;
use super::text::Text;
use crate::nds::{narc_pack, narc_unpack, Nds};
use crate::rom::RomError;

const SPECIES: usize = 649;
/// Other forms with stats of their own, and where their entries start
/// after those of the species.
const FORMS: usize = 24;
const FORM_OFFSET: usize = 35;

const STATS: &str = "a/0/1/6";
const EVOLUTIONS: &str = "a/0/1/9";
const MOVES: &str = "a/0/2/1";
const STRINGS: &str = "a/0/0/2";
const STORY: &str = "a/0/0/3";
const SCRIPTS: &str = "a/0/5/6";
/// The text file with the name of every species.
const SPECIES_NAMES: usize = 90;

/// Overlays the original randomizer reads on opening a ROM, to check them;
/// whatever it has read it packs anew on saving.
const CHECKED_OVERLAYS: [usize; 8] = [36, 162, 166, 167, 168, 284, 294, 316];
/// The overlay with the Pokémon shown in the game's introduction, and the
/// bytes before its species number there.
const INTRO_OVERLAY: usize = 294;
const INTRO_BEFORE: [u8; 15] = [0x5A, 0, 0, 1, 0, 0, 0, 0x17, 0, 0, 0, 1, 0, 0, 0];

/// The overlay that plays that Pokémon's cry, and the bytes its species
/// number there starts with in the original game.
const INTRO_CRY_OVERLAY: usize = 162;
const INTRO_CRY: [u8; 8] = [0x3D, 0x02, 0, 0, 0xF8, 0xB5, 0x1C, 0x1C];

/// Offsets in a species' stats.
const COMMON_ITEM: usize = 12;
const RARE_ITEM: usize = 14;
const DARK_GRASS_ITEM: usize = 16;

const EVOLUTION_LEN: usize = 6;
const EVOLUTION_SLOTS: usize = 7;
const EVOLUTION_METHODS: u16 = 27;

/// The ways to evolve, as the game numbers them.
mod method {
    pub const HAPPINESS_DAY: u16 = 2;
    pub const HAPPINESS_NIGHT: u16 = 3;
    pub const LEVEL: u16 = 4;
    pub const TRADE: u16 = 5;
    pub const TRADE_ITEM: u16 = 6;
    /// Karrablast and Shelmet, traded for each other.
    pub const TRADE_SPECIAL: u16 = 7;
    pub const STONE: u16 = 8;
    /// Feebas' evolution by beauty, which generation 5 has no way to reach.
    pub const BEAUTY: u16 = 16;
    pub const LEVEL_ITEM_DAY: u16 = 19;
    pub const LEVEL_ITEM_NIGHT: u16 = 20;
    pub const LEVEL_WITH_OTHER: u16 = 22;
    /// Those that happen at a level.
    pub const AT_LEVEL: [u16; 10] = [LEVEL, 9, 10, 11, 12, 13, 14, 15, 23, 24];

    /// The place of a method in the original randomizer's list of them,
    /// which decides the order of evolutions it adds.
    pub fn rank(method: u16) -> u32 {
        const RANKS: [u32; 28] = [
            0, 4, 5, 6, 0, 2, 3, 26, 1, 7, 9, 8, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
            22, 23, 24, 25,
        ];
        RANKS.get(usize::from(method)).copied().unwrap_or(0)
    }
}

const SLOWPOKE: usize = 79;
const EEVEE: usize = 133;
const KARRABLAST: u16 = 588;
const SHELMET: u16 = 616;
const SUN_STONE: u16 = 80;
const MOON_STONE: u16 = 81;
const WATER_STONE: u16 = 84;
/// Before the happiness an evolution needs, three times in the ARM9.
const HAPPINESS_MARK: [u8; 4] = [0xDC, 0x28, 0x2F, 0xD3];

/// Where `mark` stands in `data`, if it does exactly once and not at the
/// very start.
fn find(data: &[u8], mark: &[u8]) -> Option<usize> {
    let mut places = data.windows(mark.len()).enumerate();
    let mut found = places
        .by_ref()
        .filter(|(_, w)| *w == mark)
        .map(|(at, _)| at);
    let first = found.next()?;
    (found.next().is_none() && first > 0).then_some(first)
}

/// What a randomization changes; everything is off by default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// Evolutions by trade become ones a single player can reach.
    pub change_impossible_evolutions: bool,
    /// Lower levels and less happiness for evolutions.
    pub easier_evolutions: bool,
    /// Evolutions bound to day or night work at any time.
    pub no_time_evolutions: bool,
    /// Wild Pokémon are randomized in some way.
    pub random_wild: bool,
}

fn word(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn set_word(d: &mut [u8], at: usize, value: u16) {
    d[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

/// The items a wild Pokémon of a species may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldItems {
    /// An item every one of them holds, or 0.
    pub always: u16,
    pub common: u16,
    pub rare: u16,
    pub dark_grass: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evolution {
    pub method: u16,
    /// A level, an item or whatever else the method asks for.
    pub extra: u16,
    /// The species evolved into.
    pub to: u16,
}

impl HeldItems {
    fn read(stats: &[u8]) -> Self {
        let (common, rare) = (word(stats, COMMON_ITEM), word(stats, RARE_ITEM));
        if common == rare {
            HeldItems {
                always: common,
                common: 0,
                rare: 0,
                dark_grass: 0,
            }
        } else {
            HeldItems {
                always: 0,
                common,
                rare,
                dark_grass: word(stats, DARK_GRASS_ITEM),
            }
        }
    }

    fn write(&self, stats: &mut [u8]) {
        let (common, rare, dark_grass) = if self.always > 0 {
            (self.always, self.always, 0)
        } else {
            (self.common, self.rare, self.dark_grass)
        };
        set_word(stats, COMMON_ITEM, common);
        set_word(stats, RARE_ITEM, rare);
        set_word(stats, DARK_GRASS_ITEM, dark_grass);
    }
}

/// Adds `extra` to `list` in the order the original randomizer's hash set
/// hands them out.
fn add_evolutions(from: usize, list: &mut Vec<Evolution>, mut extra: Vec<Evolution>) {
    let mut unique: Vec<Evolution> = Vec::new();
    for e in extra.drain(..) {
        if !unique.iter().any(|u| u.method == e.method && u.to == e.to) {
            unique.push(e);
        }
    }
    unique.sort_by_key(|e| {
        let hash = 31 * (31 * (31 + from as u32) + u32::from(e.to)) + method::rank(e.method);
        (hash ^ (hash >> 16)) & 15
    });
    list.extend(unique);
}

impl Evolution {
    /// The evolutions of a species the randomizer works with: those with
    /// a method it knows, each target once per method.
    fn read(data: &[u8]) -> Vec<Evolution> {
        let mut list: Vec<Evolution> = Vec::new();
        for slot in 0..EVOLUTION_SLOTS {
            let at = slot * EVOLUTION_LEN;
            let evolution = Evolution {
                method: word(data, at),
                extra: word(data, at + 2),
                to: word(data, at + 4),
            };
            let known = (1..=EVOLUTION_METHODS).contains(&evolution.method)
                && evolution.method != method::BEAUTY
                && evolution.to >= 1;
            let twice = list
                .iter()
                .any(|e| e.method == evolution.method && e.to == evolution.to);
            if known && !twice {
                list.push(evolution);
            }
        }
        list
    }

    fn write(list: &[Evolution], data: &mut [u8]) {
        for slot in 0..EVOLUTION_SLOTS {
            let none = Evolution {
                method: 0,
                extra: 0,
                to: 0,
            };
            let evolution = list.get(slot).unwrap_or(&none);
            let at = slot * EVOLUTION_LEN;
            set_word(data, at, evolution.method);
            set_word(data, at + 2, evolution.extra);
            set_word(data, at + 4, evolution.to);
        }
    }
}

/// A species or one of its other forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Species {
    pub items: HeldItems,
    pub evolutions: Vec<Evolution>,
}

/// Evolutions that need a trade become possible alone.
fn remove_impossible(all: &mut [Species]) {
    for (number, species) in all.iter_mut().enumerate() {
        let mut extra = Vec::new();
        for evolution in &mut species.evolutions {
            match evolution.method {
                method::TRADE => {
                    evolution.method = method::LEVEL;
                    evolution.extra = 37;
                }
                method::TRADE_ITEM if number == SLOWPOKE => {
                    evolution.method = method::STONE;
                    evolution.extra = WATER_STONE;
                }
                method::TRADE_ITEM => {
                    evolution.method = method::LEVEL_ITEM_DAY;
                    extra.push(Evolution {
                        method: method::LEVEL_ITEM_NIGHT,
                        ..*evolution
                    });
                }
                method::TRADE_SPECIAL => {
                    evolution.method = method::LEVEL_WITH_OTHER;
                    evolution.extra = if number == usize::from(KARRABLAST) {
                        SHELMET
                    } else {
                        KARRABLAST
                    };
                }
                _ => {}
            }
        }
        add_evolutions(number, &mut species.evolutions, extra);
    }
}

/// Evolutions at a high level happen earlier, and with wild Pokémon
/// randomized none needs a certain other Pokémon in the party.
fn condense_levels(all: &mut [Species], random_wild: bool) {
    let later: Vec<bool> = (all.iter()).map(|s| !s.evolutions.is_empty()).collect();
    for species in all.iter_mut().take(SPECIES + 1) {
        for evolution in &mut species.evolutions {
            if method::AT_LEVEL.contains(&evolution.method) {
                let again = later.get(usize::from(evolution.to)) == Some(&true);
                if evolution.extra > 30 && again {
                    evolution.extra = 30;
                } else if evolution.extra > 40 {
                    evolution.extra = 40;
                }
            }
        }
    }
    if random_wild {
        // The partner may not be found in the wild any more.
        for evolution in all.iter_mut().flat_map(|s| &mut s.evolutions) {
            if evolution.method == method::LEVEL_WITH_OTHER {
                evolution.method = method::LEVEL;
                evolution.extra = 35;
            }
        }
    }
}

/// Evolutions bound to day or night get a twin for the other half.
fn remove_time_based(all: &mut [Species]) {
    for (number, species) in all.iter_mut().enumerate() {
        let mut extra = Vec::new();
        let has = |list: &[Evolution], method: u16, item: u16| {
            list.iter().any(|e| e.method == method && e.extra == item)
        };
        for i in 0..species.evolutions.len() {
            let evolution = species.evolutions[i];
            let (other, stone) = match evolution.method {
                method::HAPPINESS_DAY => (method::HAPPINESS_NIGHT, SUN_STONE),
                method::HAPPINESS_NIGHT => (method::HAPPINESS_DAY, MOON_STONE),
                method::LEVEL_ITEM_DAY => (method::LEVEL_ITEM_NIGHT, 0),
                method::LEVEL_ITEM_NIGHT => (method::LEVEL_ITEM_DAY, 0),
                _ => continue,
            };
            if stone == 0 {
                if !has(&species.evolutions, other, evolution.extra) {
                    extra.push(Evolution {
                        method: other,
                        ..evolution
                    });
                }
            } else if number == EEVEE {
                species.evolutions[i].method = method::STONE;
                species.evolutions[i].extra = stone;
            } else {
                extra.push(Evolution {
                    method: other,
                    extra: 0,
                    to: evolution.to,
                });
            }
        }
        add_evolutions(number, &mut species.evolutions, extra);
    }
}

/// A ROM opened for randomizing.
pub struct Game<'a> {
    nds: Nds<'a>,
    arm9: Vec<u8>,
    strings: Vec<Vec<u8>>,
    story: Vec<Vec<u8>>,
    scripts: Vec<Vec<u8>>,
    stats: Vec<Vec<u8>>,
    moves: Vec<Vec<u8>>,
    /// By number, from 1; the forms follow the species. Entry 0 is unused.
    pub species: Vec<Species>,
    pub random: JavaRandom,
}

fn layout(what: &'static str) -> RomError {
    RomError::Layout(what)
}

impl<'a> Game<'a> {
    fn narc(nds: &Nds, path: &'static str) -> Result<Vec<Vec<u8>>, RomError> {
        nds.id(path)
            .and_then(|id| nds.file(id))
            .and_then(narc_unpack)
            .ok_or(layout(path))
    }

    fn put_narc(&mut self, path: &'static str, files: &[Vec<u8>]) -> Result<(), RomError> {
        let id = self.nds.id(path).ok_or(layout(path))?;
        self.nds.set_file(id, narc_pack(files));
        Ok(())
    }

    /// Opens a ROM of Black 2 or White 2 in any language.
    pub fn load(rom: &'a [u8], seed: i64) -> Result<Self, RomError> {
        if !matches!(rom.get(12..15), Some(b"IRE" | b"IRD")) {
            return Err(layout("not Black 2 or White 2"));
        }
        let nds = Nds::parse(rom)?;
        let stats = Self::narc(&nds, STATS)?;
        let evolutions = Self::narc(&nds, EVOLUTIONS)?;
        if stats.len() <= SPECIES + FORMS + FORM_OFFSET || evolutions.len() <= SPECIES {
            return Err(layout("species data"));
        }
        let mut species = Vec::with_capacity(SPECIES + FORMS + 1);
        for number in 0..=SPECIES + FORMS {
            let file = if number > SPECIES {
                number + FORM_OFFSET
            } else {
                number
            };
            let data = &stats[file];
            if data.len() < DARK_GRASS_ITEM + 2 {
                return Err(layout("species data"));
            }
            let items = HeldItems::read(data);
            species.push(Species {
                items,
                evolutions: Vec::new(),
            });
        }
        for number in 1..=SPECIES {
            let data = &evolutions[number];
            if data.len() < EVOLUTION_LEN * EVOLUTION_SLOTS {
                return Err(layout("evolutions"));
            }
            species[number].evolutions = Evolution::read(data);
        }
        Ok(Game {
            arm9: nds.arm9()?,
            strings: Self::narc(&nds, STRINGS)?,
            story: Self::narc(&nds, STORY)?,
            scripts: Self::narc(&nds, SCRIPTS)?,
            moves: Self::narc(&nds, MOVES)?,
            stats,
            species,
            random: JavaRandom::new(seed),
            nds,
        })
    }

    /// Evolutions that need a trade become possible alone.
    pub fn remove_impossible_evolutions(&mut self) {
        remove_impossible(&mut self.species);
    }

    /// Lower levels and less happiness for evolutions.
    pub fn make_evolutions_easier(&mut self, random_wild: bool) {
        condense_levels(&mut self.species, random_wild);
        if let Some(at) = find(&self.arm9, &HAPPINESS_MARK) {
            for place in [at, at + 20, at + 38] {
                if self.arm9.get(place) == Some(&220) {
                    self.arm9[place] = 160;
                }
            }
        }
    }

    /// Evolutions bound to day or night get a twin for the other half.
    pub fn remove_time_based_evolutions(&mut self) {
        remove_time_based(&mut self.species);
    }

    /// Writes `value` into overlay `overlay`, `skip` bytes after the first
    /// place where `mark` stands. An overlay without the mark stays as it is.
    fn patch_overlay(
        &mut self,
        overlay: usize,
        mark: &[u8],
        skip: usize,
        value: u16,
    ) -> Result<(), RomError> {
        let mut code = self.nds.overlay(overlay).ok_or(layout("overlay"))?;
        let found = find(&code, mark).filter(|&at| at + skip + 2 <= code.len());
        if let Some(at) = found {
            set_word(&mut code, at + skip, value);
            self.nds.set_overlay(overlay, &code);
        }
        Ok(())
    }

    /// Puts a random Pokémon into the game's introduction.
    pub fn randomize_intro(&mut self) -> Result<(), RomError> {
        let species = 1 + self.random.index(SPECIES) as u16;
        self.patch_overlay(INTRO_OVERLAY, &INTRO_BEFORE, INTRO_BEFORE.len(), species)?;
        self.patch_overlay(INTRO_CRY_OVERLAY, &INTRO_CRY, 0, species)
    }

    /// The ROM with everything as it is now.
    pub fn save(mut self) -> Result<Vec<u8>, RomError> {
        for number in 1..=SPECIES + FORMS {
            let file = if number > SPECIES {
                number + FORM_OFFSET
            } else {
                number
            };
            self.species[number].items.write(&mut self.stats[file]);
        }
        // The names are written back even when none has changed.
        let names = self.strings.get(SPECIES_NAMES).ok_or(layout("text"))?;
        let rewritten = Text::read(names)
            .and_then(|text| text.write(names))
            .ok_or(layout("text"))?;
        self.strings[SPECIES_NAMES] = rewritten;
        let stats = std::mem::take(&mut self.stats);
        self.put_narc(STATS, &stats)?;

        let mut evolutions = Self::narc(&self.nds, EVOLUTIONS)?;
        let species = self.species.iter().zip(&mut evolutions);
        for (species, data) in species.take(SPECIES + 1).skip(1) {
            Evolution::write(&species.evolutions, data);
        }
        self.put_narc(EVOLUTIONS, &evolutions)?;

        let moves = std::mem::take(&mut self.moves);
        self.put_narc(MOVES, &moves)?;
        let arm9 = std::mem::take(&mut self.arm9);
        self.nds.set_arm9(&arm9)?;
        let strings = std::mem::take(&mut self.strings);
        self.put_narc(STRINGS, &strings)?;
        let story = std::mem::take(&mut self.story);
        self.put_narc(STORY, &story)?;
        let scripts = std::mem::take(&mut self.scripts);
        self.put_narc(SCRIPTS, &scripts)?;

        for overlay in CHECKED_OVERLAYS {
            let code = self.nds.overlay(overlay).ok_or(layout("overlay"))?;
            self.nds.set_overlay(overlay, &code);
        }
        Ok(self.nds.build())
    }
}

/// Randomizes `rom` with `seed`, in the original randomizer's order of
/// steps. Only the steps of [`Options`] are built so far.
pub fn randomize(rom: &[u8], options: &Options, seed: i64) -> Result<Vec<u8>, RomError> {
    let mut game = Game::load(rom, seed)?;
    if options.change_impossible_evolutions {
        game.remove_impossible_evolutions();
    }
    if options.easier_evolutions {
        game.make_evolutions_easier(options.random_wild);
    }
    if options.no_time_evolutions {
        game.remove_time_based_evolutions();
    }
    game.randomize_intro()?;
    game.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_items_are_kept_as_the_randomizer_keeps_them() {
        let mut stats = vec![0u8; 20];
        set_word(&mut stats, COMMON_ITEM, 5);
        set_word(&mut stats, RARE_ITEM, 9);
        set_word(&mut stats, DARK_GRASS_ITEM, 7);
        let items = HeldItems::read(&stats);
        assert_eq!((items.always, items.common, items.dark_grass), (0, 5, 7));
        let before = stats.clone();
        items.write(&mut stats);
        assert_eq!(stats, before);

        // The same item twice is one every Pokémon holds; a third is lost.
        set_word(&mut stats, RARE_ITEM, 5);
        let items = HeldItems::read(&stats);
        assert_eq!((items.always, items.common, items.dark_grass), (5, 0, 0));
        items.write(&mut stats);
        assert_eq!(word(&stats, COMMON_ITEM), 5);
        assert_eq!(word(&stats, RARE_ITEM), 5);
        assert_eq!(word(&stats, DARK_GRASS_ITEM), 0);

        // So is the item of a species that has no other.
        set_word(&mut stats, COMMON_ITEM, 0);
        set_word(&mut stats, RARE_ITEM, 0);
        set_word(&mut stats, DARK_GRASS_ITEM, 0x21D);
        HeldItems::read(&stats).write(&mut stats);
        assert_eq!(word(&stats, DARK_GRASS_ITEM), 0);
    }

    #[test]
    fn unknown_and_doubled_evolutions_are_dropped() {
        let mut data = vec![0u8; EVOLUTION_LEN * EVOLUTION_SLOTS];
        let slots = [
            (method::BEAUTY, 170, 350),
            (6, 537, 350),
            (6, 99, 350),
            (28, 1, 2),
            (4, 30, 0),
        ];
        for (slot, (method, extra, to)) in slots.into_iter().enumerate() {
            set_word(&mut data, slot * EVOLUTION_LEN, method);
            set_word(&mut data, slot * EVOLUTION_LEN + 2, extra);
            set_word(&mut data, slot * EVOLUTION_LEN + 4, to);
        }
        let list = Evolution::read(&data);
        assert_eq!(
            list,
            [Evolution {
                method: 6,
                extra: 537,
                to: 350
            }]
        );
        Evolution::write(&list, &mut data);
        assert_eq!(&data[..6], &[6, 0, 0x19, 2, 0x5E, 1]);
        assert!(data[6..].iter().all(|&b| b == 0));
    }

    /// Method, extra and target of an evolution.
    type Evo = (u16, u16, u16);

    fn species(list: &[(usize, &[Evo])]) -> Vec<Species> {
        let none = Species {
            items: HeldItems::read(&[0; 20]),
            evolutions: Vec::new(),
        };
        let mut all = vec![none; SPECIES + FORMS + 1];
        for (number, evolutions) in list {
            all[*number].evolutions = (evolutions.iter())
                .map(|&(method, extra, to)| Evolution { method, extra, to })
                .collect();
        }
        all
    }

    fn of(all: &[Species], number: usize) -> Vec<Evo> {
        (all[number].evolutions.iter())
            .map(|e| (e.method, e.extra, e.to))
            .collect()
    }

    #[test]
    fn trades_become_evolutions_for_one_player() {
        let mut all = species(&[
            (64, &[(method::TRADE, 0, 65)]),
            (
                SLOWPOKE,
                &[(method::LEVEL, 37, 80), (method::TRADE_ITEM, 221, 199)],
            ),
            (
                366,
                &[
                    (method::TRADE_ITEM, 226, 367),
                    (method::TRADE_ITEM, 227, 368),
                ],
            ),
            (588, &[(method::TRADE_SPECIAL, 0, 589)]),
            (616, &[(method::TRADE_SPECIAL, 0, 617)]),
        ]);
        remove_impossible(&mut all);
        assert_eq!(of(&all, 64), [(method::LEVEL, 37, 65)]);
        assert_eq!(of(&all, SLOWPOKE)[1], (method::STONE, WATER_STONE, 199));
        // Holding the item by day, and a twin for the night.
        assert_eq!(
            of(&all, 366),
            [
                (method::LEVEL_ITEM_DAY, 226, 367),
                (method::LEVEL_ITEM_DAY, 227, 368),
                (method::LEVEL_ITEM_NIGHT, 226, 367),
                (method::LEVEL_ITEM_NIGHT, 227, 368),
            ]
        );
        assert_eq!(of(&all, 588), [(method::LEVEL_WITH_OTHER, SHELMET, 589)]);
        assert_eq!(of(&all, 616), [(method::LEVEL_WITH_OTHER, KARRABLAST, 617)]);
    }

    #[test]
    fn high_levels_come_down() {
        let mut all = species(&[
            (633, &[(method::LEVEL, 50, 634)]),
            (634, &[(method::LEVEL, 64, 635)]),
            (147, &[(method::LEVEL, 30, 148)]),
            (588, &[(method::LEVEL_WITH_OTHER, SHELMET, 589)]),
        ]);
        condense_levels(&mut all, false);
        // 30 at most on the way to a further evolution, 40 for the last.
        assert_eq!(of(&all, 633), [(method::LEVEL, 30, 634)]);
        assert_eq!(of(&all, 634), [(method::LEVEL, 40, 635)]);
        assert_eq!(of(&all, 147), [(method::LEVEL, 30, 148)]);
        assert_eq!(of(&all, 588), [(method::LEVEL_WITH_OTHER, SHELMET, 589)]);
        condense_levels(&mut all, true);
        assert_eq!(of(&all, 588), [(method::LEVEL, 35, 589)]);
    }

    #[test]
    fn day_and_night_evolutions_get_a_twin() {
        let mut all = species(&[
            (
                EEVEE,
                &[
                    (method::HAPPINESS_DAY, 0, 196),
                    (method::HAPPINESS_NIGHT, 0, 197),
                ],
            ),
            (447, &[(method::HAPPINESS_DAY, 0, 448)]),
            (207, &[(method::LEVEL_ITEM_NIGHT, 327, 472)]),
            (
                366,
                &[
                    (method::LEVEL_ITEM_DAY, 226, 367),
                    (method::LEVEL_ITEM_NIGHT, 226, 367),
                ],
            ),
        ]);
        remove_time_based(&mut all);
        assert_eq!(
            of(&all, EEVEE),
            [
                (method::STONE, SUN_STONE, 196),
                (method::STONE, MOON_STONE, 197)
            ]
        );
        assert_eq!(
            of(&all, 447),
            [
                (method::HAPPINESS_DAY, 0, 448),
                (method::HAPPINESS_NIGHT, 0, 448)
            ]
        );
        assert_eq!(
            of(&all, 207),
            [
                (method::LEVEL_ITEM_NIGHT, 327, 472),
                (method::LEVEL_ITEM_DAY, 327, 472)
            ]
        );
        // Both halves were there already.
        assert_eq!(of(&all, 366).len(), 2);
    }

    #[test]
    fn a_mark_is_found_only_if_it_stands_once() {
        assert_eq!(find(&[9, 1, 2, 9], &[1, 2]), Some(1));
        assert_eq!(find(&[9, 1, 2, 1, 2], &[1, 2]), None);
        assert_eq!(find(&[1, 2, 9], &[1, 2]), None);
        assert_eq!(find(&[9, 9], &[1, 2]), None);
    }

    #[test]
    fn only_black_2_and_white_2_are_opened() {
        let mut rom = crate::nds::sample(&[("one", b"file")], &[]);
        // The sample has the game code of Black 2 but none of its files.
        assert!(Game::load(&rom, 1).is_err());
        rom[12..16].copy_from_slice(b"IRBO");
        assert!(matches!(
            Game::load(&rom, 1),
            Err(RomError::Layout("not Black 2 or White 2"))
        ));
    }
}
