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
/// Feebas' evolution by beauty, which generation 5 has no way to reach.
const METHOD_BEAUTY: u16 = 16;

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
                && evolution.method != METHOD_BEAUTY
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
        let found = code
            .windows(mark.len())
            .position(|w| w == mark)
            .filter(|&at| at > 0 && at + skip + 2 <= code.len());
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

/// Randomizes `rom` with `seed`. So far nothing but the Pokémon of the
/// introduction is random.
pub fn randomize(rom: &[u8], seed: i64) -> Result<Vec<u8>, RomError> {
    let mut game = Game::load(rom, seed)?;
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
            (METHOD_BEAUTY, 170, 350),
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
