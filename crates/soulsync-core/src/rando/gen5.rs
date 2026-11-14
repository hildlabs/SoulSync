//! The randomizer for Black 2 and White 2.
//!
//! [`Game::load`] reads what the original randomizer reads when it opens
//! a ROM and [`Game::save`] writes what it writes when it saves one. That
//! alone already changes a ROM a little: text files are stored unpacked,
//! held items and evolutions the randomizer has no use for are dropped,
//! and the code it looked at is packed anew.

mod evolutions;
mod items;
mod machines;
mod moves;
mod movesets;
mod statics;
mod tables;
mod trades;
mod trainer_items;
mod trainer_moves;
mod trainers;
mod traits;
mod wild;

pub use evolutions::Evolving;
pub use items::ShopRules;
pub use machines::Taught;
pub use movesets::Movesets;
pub use statics::{StaticRules, Statics};
pub use trainer_items::TrainerItems;
pub use trainers::{Spread, Themes, TrainerRules};
pub use traits::{AbilityRules, Change, Slow, Stats};
pub use wild::{Wild, WildRule, WildRules};

use super::random::JavaRandom;
use super::text::{chars, Text};
use super::{lz, names};
use crate::nds::{narc_pack, narc_unpack, Nds};
use crate::rnqs::Compat;
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

/// The moves a species learns from TMs and HMs: a bit each for 95 TMs,
/// then 6 HMs.
const MACHINES: usize = 40;
const MACHINES_LEN: usize = 13;
const TMS: usize = 95;
const HMS: usize = 6;
/// Where a species' other forms start, and how many forms it has.
const FIRST_FORM: usize = 28;
const FORM_COUNT: usize = 32;
/// The one form the original randomizer treats as differing in looks
/// only: Keldeo's Resolute form.
const LOOKS_ONLY_FORM: usize = 670;
/// Where the three abilities of a species stand in its stats.
const ABILITIES: usize = 24;
/// Species whose forms the original randomizer leaves alone although
/// they only look different, and the one that has such a form with stats
/// of its own.
const NO_LOOKS: [usize; 4] = [421, 493, 585, 586];
const KELDEO: usize = 647;

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
    /// While it knows a certain move.
    pub const LEVEL_WITH_MOVE: u16 = 21;
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
/// Where the three starters stand in the script of the first town.
const STARTER_SCRIPT: usize = 854;
const STARTER_PLACES: [[usize; 3]; 3] = [
    [0x58B, 0x590, 0x595],
    [0x5C0, 0x5C5, 0x5CA],
    [0x5E2, 0x5E7, 0x5EC],
];
/// The place in that script that hands out the Pokédex, and the piece put
/// behind the script so the Pokédex still comes with any starter.
const POKEDEX_MARK: [u8; 6] = [0x28, 0x00, 0xA1, 0x40, 0x04, 0x00];
const POKEDEX_SCRIPT: [u8; 14] = [
    0x28, 0x00, 0xA1, 0x40, 0x04, 0x00, 0xDE, 0x00, 0x00, 0x00, 0xFD, 0x01, 0x05, 0x00,
];
const STARTER_PICTURES: &str = "a/2/0/2";
const PICTURES: &str = "a/0/0/4";
/// Files per species among the pictures; the front picture comes first.
const PICTURES_EACH: usize = 20;
const PICTURE_PALETTE: usize = 18;
/// The overlay with the cries of the three starters, behind this mark.
const STARTER_CRY_OVERLAY: usize = 316;
const STARTER_CRIES: [u8; 8] = [0x08, 0x0A, 0x07, 0x00, 0x00, 0x08, 0x00, 0x00];
/// The story text of the first town: the line of the first starter (the
/// others stand before it) and what the rival says before the fight.
const STARTER_TEXT: usize = 169;
const STARTER_LINE: usize = 37;
const RIVAL_LINE: usize = 60;
/// Where a species' first type stands in its stats.
const FIRST_TYPE: usize = 6;
/// The types as the game numbers them, as the original randomizer names
/// them in the starter text of every language.
const TYPES: [&str; 17] = [
    "Normal", "Fighting", "Flying", "Poison", "Ground", "Rock", "Bug", "Ghost", "Steel", "Fire",
    "Water", "Grass", "Electric", "Psychic", "Ice", "Dragon", "Dark",
];

/// The script that gets the national Pokédex switched on, its length in
/// the games this is built for, and the two instructions put into it.
const DEX_SCRIPT: usize = 854;
const DEX_SCRIPT_LEN: usize = 7996;
const DEX_INSERTS: [(usize, [u8; 2]); 2] = [(1654, [0xC6, 0x01]), (3695, [0, 0])];
/// The jumps in that script that cross one of the two new instructions.
const DEX_JUMPS: [usize; 80] = [
    4, 8, 12, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 72, 76, 80, 88, 92, 96, 100, 104, 108,
    112, 116, 120, 124, 128, 449, 459, 467, 591, 717, 1052, 1085, 1118, 1148, 1179, 1602, 2106,
    2116, 2128, 2155, 2165, 2183, 2193, 2205, 2232, 2242, 2305, 2332, 2342, 2489, 2499, 2529, 2564,
    2597, 2630, 2663, 2704, 2714, 2728, 2752, 2776, 2812, 2824, 2939, 2947, 3190, 3356, 3368, 3401,
    3413, 3446, 3458, 3488, 3500, 3610, 3646, 3660,
];
/// The overlay with the table of what each type does to each other, and
/// how that table starts.
const BATTLE_OVERLAY: usize = 167;
const TYPE_TABLE: [u8; 8] = [4, 4, 4, 4, 4, 2, 4, 0];
const TYPE_COUNT: usize = 17;
const GHOST: usize = 7;
const STEEL: usize = 8;
const DARK: usize = 16;
const NORMAL_DAMAGE: u8 = 4;
/// The highest item number, and the items no random choice may give: key
/// items, items of other games, TMs and HMs, items of the Wonder Launcher.
const LAST_ITEM: usize = 638;
const NO_ITEMS: [(usize, usize); 10] = [
    (428, 76),
    (505, 32),
    (621, 18),
    (574, 1),
    (578, 2),
    (616, 5),
    (113, 3),
    (120, 14),
    (328, 100),
    (592, 24),
];
/// Items of little use, left out where only good ones are wanted.
const BAD_ITEMS: [(usize, usize); 12] = [
    (111, 2),
    (225, 1),
    (236, 1),
    (155, 1),
    (274, 1),
    (575, 1),
    (95, 4),
    (135, 2),
    (137, 12),
    (159, 25),
    (256, 4),
    (260, 5),
];
const LUCKY_EGG: usize = 231;

/// The moves of the TMs and HMs stand in the ARM9 behind this mark: 92
/// TMs, the 6 HMs, then the last 3 TMs.
const MACHINE_MOVES: [u8; 4] = [0x87, 0x03, 0x88, 0x03];
const FIRST_TMS: usize = 92;
const MOVES_COUNT: usize = 559;
const STRUGGLE: u16 = 165;
/// Swift never misses.
const SWIFT: usize = 129;
/// The overlay with the move tutors, how many moves they teach, the size
/// of a tutor's entry and where the entries start in each game.
const TUTOR_OVERLAY: usize = 36;
const TUTORS: usize = 60;
const TUTOR_LEN: usize = 12;
const TUTOR_PLACES: [(&[u8; 4], usize); 14] = [
    (b"IREO", 0x51538),
    (b"IRDO", 0x5152C),
    (b"IREF", 0x5152C),
    (b"IRDF", 0x51520),
    (b"IRED", 0x5155C),
    (b"IRDD", 0x51550),
    (b"IREI", 0x51554),
    (b"IRDI", 0x51548),
    (b"IRES", 0x5153C),
    (b"IRDS", 0x51530),
    (b"IREJ", 0x512DC),
    (b"IRDJ", 0x512D0),
    (b"IREK", 0x5160C),
    (b"IRDK", 0x51600),
];

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

/// Which Pokémon the game starts with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Starters {
    #[default]
    Unchanged,
    /// Any three.
    Random,
    /// Three that evolve twice.
    TwoEvolutions,
    /// These three species; 0 stands for any.
    Custom([u16; 3]),
}

/// What the item maniacs buy: four treasures from the Balm Mushroom on,
/// the four relics, and seven rare berries.
const MANIAC_ITEMS: [(usize, usize); 3] = [(580, 4), (587, 4), (206, 7)];
/// The script with the gifts of the two Lucky Eggs, and the kinds of
/// mulch that take their place.
const GIFT_SCRIPT: usize = 676;
const MULCH: [u16; 4] = [95, 96, 97, 98];
/// The overlay of walking around, and what follows the branch that
/// asks for the Running Shoes in it.
const FIELD_CODE: usize = 36;
const SHOES_MARK: [u8; 11] = [
    0x01, 0xD0, 0x01, 0x20, 0x08, 0xBD, 0x00, 0x20, 0x08, 0xBD, 0x63,
];
/// The start of the function that says how hard the game is.
const DIFFICULTY_MARK: [u8; 20] = [
    0x81, 0x6A, 0x40, 0x6B, 0x0B, 0x1C, 0x07, 0x49, 0x00, 0x22, 0x43, 0x40, 0x90, 0x00, 0x08, 0x58,
    0x83, 0x42, 0x01, 0xD1,
];
/// The overlay of battles' music, and the branch taken when no Pokémon
/// is low on HP.
const LOW_HP_OVERLAY: usize = 168;
const LOW_HP_MARK: [u8; 4] = [0x00, 0xD1, 0x01, 0x27];

/// What a randomization changes; everything is off by default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// The national Pokédex from the start.
    pub national_dex: bool,
    /// Ghost and Dark hit Steel normally, as in later games.
    pub update_type_effectiveness: bool,
    /// No random choice gives a Lucky Egg.
    pub ban_lucky_egg: bool,
    /// Nor what the item maniacs pay fortunes for.
    pub ban_maniac_items: bool,
    /// The Lucky Eggs the game gives away become mulch.
    pub no_free_lucky_egg: bool,
    /// Running works from the start.
    pub run_without_shoes: bool,
    /// Always Challenge Mode.
    pub force_challenge_mode: bool,
    pub no_low_hp_music: bool,
    /// Wild Pokémon hold random items, with `good_wild_held_items` only
    /// useful ones.
    pub random_wild_held_items: bool,
    pub good_wild_held_items: bool,
    /// Evolutions by trade become ones a single player can reach.
    pub change_impossible_evolutions: bool,
    /// Lower levels and less happiness for evolutions.
    pub easier_evolutions: bool,
    /// Evolutions bound to day or night work at any time.
    pub no_time_evolutions: bool,
    /// Wild Pokémon are random: each species of an area becomes one
    /// other, with `wild_forms` also other forms.
    pub random_wild: bool,
    pub wild_forms: bool,
    /// How they are made random; the levels in it change anyway.
    pub wild_rules: WildRules,
    /// Pokémon get easier to catch, in steps from 1 to 5; 0 leaves them.
    pub min_catch_rate: u8,
    /// The three Pokémon to choose the first one from.
    pub starters: Starters,
    /// The move tutors teach random moves.
    pub random_tutor_moves: bool,
    /// Trainers have random Pokémon, with `trainer_forms` also other
    /// forms of them.
    pub random_trainers: bool,
    pub trainer_forms: bool,
    /// So many more Pokémon for the regular trainers, the important
    /// ones and the bosses.
    pub more_trainer_pokemon: [u8; 3],
    /// What the random ones have to be like; the levels in it change
    /// without random trainers too.
    pub trainer_rules: TrainerRules,
    /// The rival fights with the starter the player left for them.
    pub rival_carries_starter: bool,
    /// Trainers' Pokémon from this level on are fully evolved; 0 for off.
    pub trainers_fully_evolved_from: u16,
    /// Every trainer fights double battles.
    pub double_battles: bool,
    /// Trainers' Pokémon know four moves that suit them.
    pub better_trainer_moves: bool,
    /// Trainers' Pokémon hold random items.
    pub trainer_items: TrainerItems,
    /// Trainers get random names.
    pub random_trainer_names: bool,
    /// The kinds of trainers get random names.
    pub random_trainer_class_names: bool,
    /// Trades give random Pokémon; with `trades_want_random` they ask
    /// for random ones too. Nicknames, IVs and held items as chosen.
    pub random_trades: bool,
    pub trades_want_random: bool,
    pub random_trade_nicknames: bool,
    pub random_trade_trainers: bool,
    pub random_trade_ivs: bool,
    pub random_trade_items: bool,
    /// Random items lie around, with `good_field_items` only useful ones.
    pub random_field_items: bool,
    pub good_field_items: bool,
    /// With random ones: none much more often than the others.
    pub even_field_items: bool,
    /// The items lying around only change places.
    pub shuffle_field_items: bool,
    /// The special shops sell random goods, with `good_shop_items` only
    /// useful ones.
    pub random_shops: bool,
    pub good_shop_items: bool,
    pub shop_rules: ShopRules,
    /// The goods of the special shops only change places.
    pub shuffle_shops: bool,
    /// Pickup finds random items, with `good_pickup_items` only useful
    /// ones.
    pub random_pickup: bool,
    pub good_pickup_items: bool,
    /// Pokémon at fixed spots are swapped, as `static_rules` says.
    pub random_statics: bool,
    pub static_rules: StaticRules,
    /// Every Pokémon can learn every HM.
    pub full_hm: bool,
    /// The base stats of every species, and whether an evolution takes
    /// after what it evolved from: with `stats_grow_at_random` what it
    /// gains goes to any stat, else all stats rise alike.
    pub stats: Stats,
    pub stats_follow_evolutions: bool,
    pub stats_grow_at_random: bool,
    /// The types of every species; with `two_types` always two.
    pub types: Change,
    pub two_types: bool,
    /// The abilities of every species, and which ones may come up.
    pub abilities: Change,
    pub ability_rules: AbilityRules,
    /// The moves get random power, accuracy, PP or types.
    pub random_move_powers: bool,
    pub random_move_accuracies: bool,
    pub random_move_pps: bool,
    pub random_move_types: bool,
    pub random_move_categories: bool,
    /// Every species gains levels at this speed, as the game numbers
    /// them, apart from those that stay slow.
    pub level_alike: Option<(Slow, u8)>,
    /// Every species evolves into random others.
    pub random_evolutions: Option<Evolving>,
    /// The moves every species learns by level and hatches with. With
    /// `moves_at_start` it knows that many at level 1 at least (0 for as
    /// it was); `good_attacks` percent of its moves are good attacks;
    /// `no_broken_moves` leaves out moves of fixed damage.
    pub movesets: Movesets,
    pub moves_at_start: u8,
    pub good_attacks: u8,
    pub no_broken_moves: bool,
    /// Every species learns its weaker attacks first.
    pub order_moves_by_damage: bool,
    /// The TMs hold random moves.
    pub random_tm_moves: bool,
    /// Which Pokémon learn the moves of the TMs and HMs, and which those
    /// of the move tutors.
    pub machines: Compat,
    pub tutors: Compat,
    /// How the moves of the TMs and the tutors are chosen, and what
    /// follows for who learns them.
    pub tm_rules: Taught,
    pub tutor_rules: Taught,
}

fn word(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn set_word(d: &mut [u8], at: usize, value: u16) {
    d[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

/// The items a random choice may give.
#[derive(Debug, Clone)]
struct Items {
    allowed: Vec<bool>,
}

impl Items {
    fn new(good: bool) -> Self {
        let mut allowed = vec![true; LAST_ITEM + 1];
        allowed[0] = false;
        let bad = if good { &BAD_ITEMS[..] } else { &[] };
        for &(first, count) in NO_ITEMS.iter().chain(bad) {
            allowed[first..first + count].fill(false);
        }
        Items { allowed }
    }

    fn ban(&mut self, item: usize) {
        self.allowed[item] = false;
    }

    fn random(&self, random: &mut JavaRandom) -> u16 {
        let mut chosen = 0;
        while !self.allowed[chosen] {
            chosen = random.index(self.allowed.len());
        }
        chosen as u16
    }
}

/// Gives every species new items for its wild Pokémon to hold.
fn random_held_items(all: &mut [Species], items: &Items, random: &mut JavaRandom) {
    for species in all.iter_mut().skip(1) {
        let held = &mut species.items;
        let decision = random.double();
        let mut dark_grass = true;
        let pair = |held: &mut HeldItems, random: &mut JavaRandom| {
            held.common = items.random(random);
            held.rare = items.random(random);
            while held.rare == held.common {
                held.rare = items.random(random);
            }
        };
        if held.always > 0 {
            if decision < 0.9 {
                dark_grass = false;
                held.always = items.random(random);
            } else {
                held.always = 0;
                pair(held, random);
            }
        } else if decision < 0.5 {
            held.common = 0;
            held.rare = 0;
        } else if decision < 0.65 {
            held.common = 0;
            held.rare = items.random(random);
        } else if decision < 0.8 {
            held.common = items.random(random);
            held.rare = 0;
        } else if decision < 0.95 {
            pair(held, random);
        } else {
            dark_grass = false;
            held.always = items.random(random);
            held.common = 0;
            held.rare = 0;
        }
        held.dark_grass = if dark_grass && random.double() < 0.5 {
            items.random(random)
        } else {
            0
        };
    }
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
    pub abilities: [u8; 3],
    /// For a form with stats of its own: its species and which of the
    /// species' forms it is. Both are 0 for a species.
    pub base: usize,
    pub form: u16,
    /// How many forms a species has that only look different.
    pub looks: u8,
    /// A form with stats of its own that still only looks different.
    pub looks_only: bool,
}

/// Evolutions that need a trade become possible alone.
fn remove_impossible(all: &mut [Species], learnt_at: Option<&dyn Fn(usize, u16) -> u16>) {
    for (number, species) in all.iter_mut().enumerate() {
        let mut extra = Vec::new();
        for evolution in &mut species.evolutions {
            if let (method::LEVEL_WITH_MOVE, Some(learnt_at)) = (evolution.method, learnt_at) {
                evolution.method = method::LEVEL;
                evolution.extra = learnt_at(number, evolution.extra);
            }
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

/// Lets a species learn every HM. Bits past the last HM are cleared, as
/// the original randomizer's list of machines ends there.
fn full_hms(stats: &mut [u8]) {
    let machines = &mut stats[MACHINES..MACHINES + MACHINES_LEN];
    for bit in 0..MACHINES_LEN * 8 {
        let mask = 1 << (bit % 8);
        if bit >= TMS + HMS {
            machines[bit / 8] &= !mask;
        } else if bit >= TMS {
            machines[bit / 8] |= mask;
        }
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
    /// The three starters, once they are chosen anew.
    pub starters: Option<[u16; 3]>,
    /// The items random choices may give: all of them, and the good ones.
    items: Items,
    good_items: Items,
    /// Decides between the ways a trainer's Pokémon may evolve; drawn
    /// once when first needed.
    evolution_turn: Option<usize>,
    /// The game code, which tells version and language.
    code: [u8; 4],
    pub random: JavaRandom,
    /// Whether abilities were changed: forms that exist through an ability
    /// are then Pokémon like any other.
    abilities_changed: bool,
    /// Ghost and Dark hit Steel normally by now.
    effectiveness_updated: bool,
    /// The accuracy the game gives a move that never misses.
    sure_accuracy: u8,
    /// The order random evolutions were made in, which is the order of
    /// what a species evolves from since.
    evolved_in: Option<Vec<usize>>,
    /// The random numbers for what changes nothing about the game but
    /// looks and names; the original keeps these apart.
    pub looks_random: JavaRandom,
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
            if data.len() < MACHINES + MACHINES_LEN || data.len() <= FORM_COUNT {
                return Err(layout("species data"));
            }
            let items = HeldItems::read(data);
            let mut looks = 0;
            let mut looks_only = false;
            if data[FORM_COUNT] > 1 {
                if word(data, FIRST_FORM) != 0 {
                    if number == KELDEO {
                        looks = data[FORM_COUNT];
                    }
                } else {
                    if number < SPECIES && !NO_LOOKS.contains(&number) {
                        looks = data[FORM_COUNT];
                    }
                    looks_only = number == LOOKS_ONLY_FORM;
                }
            }
            species.push(Species {
                items,
                evolutions: Vec::new(),
                abilities: [data[ABILITIES], data[ABILITIES + 1], data[ABILITIES + 2]],
                base: 0,
                form: 0,
                looks,
                looks_only,
            });
        }
        for (number, data) in stats.iter().enumerate().take(SPECIES + 1).skip(1) {
            let first = usize::from(word(data, FIRST_FORM));
            if data[FORM_COUNT] > 1 && first != 0 {
                for form in 1..data[FORM_COUNT] {
                    let file = first + usize::from(form) - 1;
                    let entry = file.checked_sub(FORM_OFFSET).filter(|n| *n > SPECIES);
                    let entry = entry.and_then(|n| species.get_mut(n));
                    let entry = entry.ok_or(layout("forms"))?;
                    entry.base = number;
                    entry.form = u16::from(form);
                }
            }
        }
        for number in 1..=SPECIES {
            let data = &evolutions[number];
            if data.len() < EVOLUTION_LEN * EVOLUTION_SLOTS {
                return Err(layout("evolutions"));
            }
            species[number].evolutions = Evolution::read(data);
        }
        let moves = Self::narc(&nds, MOVES)?;
        let swift = moves.get(SWIFT).and_then(|m| m.get(4));
        Ok(Game {
            arm9: nds.arm9()?,
            strings: Self::narc(&nds, STRINGS)?,
            story: Self::narc(&nds, STORY)?,
            scripts: Self::narc(&nds, SCRIPTS)?,
            sure_accuracy: swift.copied().unwrap_or(101),
            moves,
            stats,
            species,
            starters: None,
            items: Items::new(false),
            good_items: Items::new(true),
            evolution_turn: None,
            code: [rom[12], rom[13], rom[14], rom[15]],
            random: JavaRandom::new(seed),
            abilities_changed: false,
            effectiveness_updated: false,
            evolved_in: None,
            looks_random: JavaRandom::new(seed),
            nds,
        })
    }

    /// Keeps the Lucky Egg out of every random choice of items.
    pub fn ban_lucky_egg(&mut self) {
        self.items.ban(LUCKY_EGG);
        self.good_items.ban(LUCKY_EGG);
    }

    /// Keeps what the item maniacs pay fortunes for out of every random
    /// choice of items: the big mushroom and its like, the relics and
    /// the rare berries.
    pub fn ban_maniac_items(&mut self) {
        for (first, count) in MANIAC_ITEMS {
            for item in first..first + count {
                self.items.ban(item);
                self.good_items.ban(item);
            }
        }
    }

    /// The two Lucky Eggs the game gives away become one kind of mulch.
    /// The original looks for them byte by byte through each script of
    /// the file until both are found.
    pub fn remove_free_lucky_eggs(&mut self) -> Result<(), RomError> {
        let mulch = MULCH[self.random.index(MULCH.len())];
        let script = (self.scripts.get_mut(GIFT_SCRIPT)).ok_or(layout("script"))?;
        let (mut list, mut left) = (0, 2);
        while left > 0 && list + 4 <= script.len() && word(script, list) != 0xFD13 {
            let jump = i32::from_le_bytes([
                script[list],
                script[list + 1],
                script[list + 2],
                script[list + 3],
            ]);
            let mut at = (list as i64 + 4 + i64::from(jump)) as usize;
            list += 4;
            if at > script.len() {
                break;
            }
            loop {
                at += 1;
                if at >= script.len() {
                    return Err(layout("script of the Lucky Eggs"));
                }
                if script[at] == 0x2A && at + 6 <= script.len() {
                    let here = [word(script, at), word(script, at + 2), word(script, at + 4)];
                    if here == [0x2A, 0x8000, LUCKY_EGG as u16] {
                        set_word(script, at + 4, mulch);
                        left -= 1;
                    }
                }
                // The start of the next script.
                if script[at] == 0x2E {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Running works without the Running Shoes.
    pub fn run_without_shoes(&mut self) -> Result<(), RomError> {
        let mut code = (self.nds.overlay(FIELD_CODE)).ok_or(layout("overlay"))?;
        let at = find(&code, &SHOES_MARK).ok_or(layout("check for the Running Shoes"))?;
        set_word(&mut code, at, 0);
        self.nds.set_overlay(FIELD_CODE, &code);
        Ok(())
    }

    /// The game says it is played in Challenge Mode whatever was chosen.
    pub fn force_challenge_mode(&mut self) {
        if let Some(at) = find(&self.arm9, &DIFFICULTY_MARK) {
            // mov r0, #2; bx lr
            self.arm9[at..at + 4].copy_from_slice(&[0x02, 0x20, 0x70, 0x47]);
        }
    }

    /// The music does not change when a Pokémon is low on HP.
    pub fn disable_low_hp_music(&mut self) -> Result<(), RomError> {
        let mut code = (self.nds.overlay(LOW_HP_OVERLAY)).ok_or(layout("overlay"))?;
        if let Some(at) = find(&code, &LOW_HP_MARK) {
            // The branch past the change is always taken.
            code[at + 1] = 0xE0;
            self.nds.set_overlay(LOW_HP_OVERLAY, &code);
        }
        Ok(())
    }

    /// Switches the national Pokédex on together with the Pokédex. The
    /// original randomizer does this with a patch that replaces most of
    /// the script; this puts the same two instructions in and moves the
    /// jumps across them, which gives the same script.
    pub fn national_dex_at_start(&mut self) -> Result<(), RomError> {
        let script = self.scripts.get_mut(DEX_SCRIPT).ok_or(layout("script"))?;
        if script.len() != DEX_SCRIPT_LEN {
            return Err(layout("script of the Pokédex"));
        }
        let moved =
            |at: usize| at + DEX_INSERTS.iter().filter(|(place, _)| *place < at).count() * 2;
        for at in DEX_JUMPS {
            let jump =
                i32::from_le_bytes([script[at], script[at + 1], script[at + 2], script[at + 3]]);
            let target = (at as i32 + 4 + jump) as usize;
            let jump = moved(target) as i32 - moved(at) as i32 - 4;
            script[at..at + 4].copy_from_slice(&jump.to_le_bytes());
        }
        for (place, bytes) in DEX_INSERTS.iter().rev() {
            script.splice(place..place, *bytes);
        }
        Ok(())
    }

    /// Ghost and Dark hit Steel normally, as in later games.
    pub fn update_type_effectiveness(&mut self) -> Result<(), RomError> {
        let mut code = self.nds.overlay(BATTLE_OVERLAY).ok_or(layout("overlay"))?;
        let table =
            find(&code, &TYPE_TABLE).filter(|at| at + TYPE_COUNT * TYPE_COUNT <= code.len());
        if let Some(table) = table {
            code[table + GHOST * TYPE_COUNT + STEEL] = NORMAL_DAMAGE;
            code[table + DARK * TYPE_COUNT + STEEL] = NORMAL_DAMAGE;
            self.nds.set_overlay(BATTLE_OVERLAY, &code);
            self.effectiveness_updated = true;
        }
        Ok(())
    }

    /// Wild Pokémon hold random items, with `good` only useful ones.
    pub fn randomize_wild_held_items(&mut self, good: bool) {
        let items = if good { &self.good_items } else { &self.items };
        random_held_items(&mut self.species, items, &mut self.random);
    }

    /// Evolutions by trade become ones a single player can reach. With
    /// `new_movesets` an evolution that needs a certain move, which the
    /// species may never learn then, comes at the level it learnt it.
    pub fn remove_impossible_evolutions(&mut self, new_movesets: bool) -> Result<(), RomError> {
        let learnsets = Self::narc(&self.nds, movesets::LEARNSETS)?;
        let learnt_at = |species: usize, wanted: u16| {
            let entries = learnsets.get(species).map_or(&[][..], Vec::as_slice);
            let mut entries = entries.chunks_exact(4).map(|e| (word(e, 0), word(e, 2)));
            let found = entries.find(|&(learnt, _)| learnt == wanted);
            // Known from the start means relearnt, which is late.
            found.map_or(45, |(_, level)| if level == 1 { 45 } else { level })
        };
        let learnt_at: &dyn Fn(usize, u16) -> u16 = &learnt_at;
        remove_impossible(&mut self.species, new_movesets.then_some(learnt_at));
        Ok(())
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

    /// Chooses the three starters as `how` says; no Pokémon twice
    /// unless it was asked for.
    pub fn choose_starters(&mut self, how: Starters) -> Result<(), RomError> {
        let mut picked: Vec<u16> = Vec::with_capacity(3);
        match how {
            Starters::Unchanged => return Ok(()),
            Starters::Random => {
                while picked.len() < 3 {
                    let species = 1 + self.random.index(SPECIES) as u16;
                    if !picked.contains(&species) {
                        picked.push(species);
                    }
                }
            }
            Starters::TwoEvolutions => {
                // The first stage of a family with at least three.
                let evolves = |n: usize| !self.species[n].evolutions.is_empty();
                let evolved: Vec<u16> = (1..=SPECIES)
                    .flat_map(|n| self.species[n].evolutions.iter().map(|e| e.to))
                    .collect();
                let usable: Vec<u16> = (1..=SPECIES)
                    .filter(|&n| {
                        let twice = |e: &Evolution| evolves(usize::from(e.to));
                        !evolved.contains(&(n as u16))
                            && self.species[n].evolutions.iter().any(twice)
                    })
                    .map(|n| n as u16)
                    .collect();
                while picked.len() < 3 {
                    let species = usable[self.random.index(usable.len())];
                    if !picked.contains(&species) {
                        picked.push(species);
                    }
                }
            }
            Starters::Custom(wanted) => {
                for species in wanted {
                    let mut species = species;
                    // 0 is any that is neither wanted nor picked.
                    while species == 0 || species as usize > SPECIES {
                        let any = 1 + self.random.index(SPECIES) as u16;
                        if !picked.contains(&any) && !wanted.contains(&any) {
                            species = any;
                        }
                    }
                    picked.push(species);
                }
            }
        }
        self.set_starters([picked[0], picked[1], picked[2]])
    }

    /// Puts `starters` where the game hands out the first Pokémon: into
    /// the script, the pictures, the cries and the text of the choice.
    pub fn set_starters(&mut self, starters: [u16; 3]) -> Result<(), RomError> {
        let japanese = self.code[3] == b'J';
        let script = self
            .scripts
            .get_mut(STARTER_SCRIPT)
            .ok_or(layout("script"))?;
        if script.len() < STARTER_PLACES[2][2] + 2 {
            return Err(layout("script"));
        }
        for (places, starter) in STARTER_PLACES.iter().zip(starters) {
            for &place in places {
                set_word(script, place, starter);
            }
        }
        // Without the first starter the game would not hand out the
        // Pokédex, so that part moves behind the script.
        if let Some(at) = find(script, &POKEDEX_MARK) {
            let end = script.len();
            script.extend_from_slice(&POKEDEX_SCRIPT);
            if japanese {
                script[end + 6] -= 4;
            }
            script[at] = 0x1E;
            script[at + 1] = 0;
            let jump = (end as u32).wrapping_sub(at as u32 + 6);
            script[at + 2..at + 6].copy_from_slice(&jump.to_le_bytes());
        }

        let mut shown = Self::narc(&self.nds, STARTER_PICTURES)?;
        let pictures = Self::narc(&self.nds, PICTURES)?;
        if shown.len() < 15 {
            return Err(layout(STARTER_PICTURES));
        }
        for (i, &starter) in starters.iter().enumerate() {
            let first = usize::from(starter) * PICTURES_EACH;
            let palette = pictures.get(first + PICTURE_PALETTE);
            let picture = pictures.get(first).and_then(|packed| lz::decode(packed));
            let (Some(palette), Some(picture)) = (palette, picture) else {
                return Err(layout(PICTURES));
            };
            shown[i * 2] = palette.clone();
            shown[12 + i] = picture;
        }
        self.put_narc(STARTER_PICTURES, &shown)?;

        let mut code = self
            .nds
            .overlay(STARTER_CRY_OVERLAY)
            .ok_or(layout("overlay"))?;
        let cries = find(&code, &STARTER_CRIES).map(|at| at + STARTER_CRIES.len());
        if let Some(at) = cries.filter(|&at| at + 6 <= code.len()) {
            for (i, &starter) in starters.iter().enumerate() {
                set_word(&mut code, at + i * 2, starter);
            }
            self.nds.set_overlay(STARTER_CRY_OVERLAY, &code);
        }

        let names = self
            .strings
            .get(SPECIES_NAMES)
            .and_then(|file| Text::read(file));
        let names = names.ok_or(layout("text"))?;
        let file = self.story.get(STARTER_TEXT).ok_or(layout("text"))?;
        let mut text = Text::read(file).ok_or(layout("text"))?;
        if text.lines.len() <= RIVAL_LINE {
            return Err(layout("text"));
        }
        const NAME: [u16; 3] = [0xF000, 0xBD02, 0];
        const WAIT: [u16; 3] = [0xF000, 0xBE01, 0];
        const BREAK: u16 = 0xFFFE;
        for (i, &starter) in starters.iter().enumerate() {
            let kind = self.stats[usize::from(starter)][FIRST_TYPE];
            let kind = TYPES.get(usize::from(kind)).ok_or(layout("type"))?;
            let name = names
                .lines
                .get(usize::from(starter))
                .ok_or(layout("text"))?;
            let mut line = NAME.to_vec();
            line.extend(chars(&format!("The {kind}-type Pok\u{e9}mon")));
            line.push(BREAK);
            line.extend(NAME);
            line.extend(name);
            text.lines[STARTER_LINE - i] = line;
        }
        let mut line = vec![0xF000, 0x0100, 1, 1];
        line.extend(chars(": Let's see how good"));
        line.push(BREAK);
        line.extend(chars("a Trainer you are!"));
        line.extend(WAIT);
        line.push(BREAK);
        line.extend(chars("I'll use my Pok\u{e9}mon"));
        line.push(BREAK);
        line.extend(chars("that I raised from an Egg!"));
        line.extend(WAIT);
        text.lines[RIVAL_LINE] = line;
        self.story[STARTER_TEXT] = text.write(file).ok_or(layout("text"))?;
        self.starters = Some(starters);
        Ok(())
    }

    /// The moves of the TMs and of the HMs.
    fn machine_moves(&self) -> Option<(Vec<u16>, Vec<u16>)> {
        let at = find(&self.arm9, &MACHINE_MOVES)? + MACHINE_MOVES.len();
        let words = self.arm9.get(at..at + (TMS + HMS) * 2)?;
        let mut moves: Vec<u16> = words.chunks(2).map(|w| word(w, 0)).collect();
        let hms = moves.drain(FIRST_TMS..FIRST_TMS + HMS).collect();
        Some((moves, hms))
    }

    /// Puts `picked` where the move tutors have their moves.
    pub(super) fn set_tutor_moves(&mut self, picked: &[u16]) -> Result<(), RomError> {
        let place = TUTOR_PLACES.iter().find(|(code, _)| **code == self.code);
        let &(_, place) = place.ok_or(layout("move tutors"))?;
        let mut code = self.nds.overlay(TUTOR_OVERLAY).ok_or(layout("overlay"))?;
        if code.len() < place + TUTORS * TUTOR_LEN {
            return Err(layout("move tutors"));
        }
        for (i, &chosen) in picked.iter().take(TUTORS).enumerate() {
            set_word(&mut code, place + i * TUTOR_LEN, chosen);
        }
        self.nds.set_overlay(TUTOR_OVERLAY, &code);
        Ok(())
    }

    /// Every Pokémon can learn every HM.
    pub fn full_hm_compatibility(&mut self) {
        for number in 1..=SPECIES + FORMS {
            full_hms(&mut self.stats[Self::stats_file(number)]);
        }
        self.copy_to_looks_only(MACHINES..MACHINES + MACHINES_LEN);
    }

    /// The file with the stats of a species or form.
    fn stats_file(number: usize) -> usize {
        if number > SPECIES {
            number + FORM_OFFSET
        } else {
            number
        }
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

/// Whether `rom`, or just its first bytes, is a game whose randomization
/// was compared with the original randomizer's, byte for byte. So far that
/// is the German Black 2; the others are built from the same tables but
/// nobody has checked them.
pub fn verified(rom: &[u8]) -> bool {
    const VERIFIED: [&[u8]; 1] = [b"IRED"];
    rom.get(12..16).is_some_and(|code| VERIFIED.contains(&code))
}

/// Randomizes `rom` with `seed`, in the original randomizer's order of
/// steps. Only the steps of [`Options`] are built so far.
pub fn randomize(rom: &[u8], options: &Options, seed: i64) -> Result<Vec<u8>, RomError> {
    let mut game = Game::load(rom, seed)?;
    if options.random_move_powers {
        game.randomize_move_powers();
    }
    if options.random_move_accuracies {
        game.randomize_move_accuracies();
    }
    if options.random_move_pps {
        game.randomize_move_pps();
    }
    if options.random_move_types {
        game.randomize_move_types();
    }
    if options.random_move_categories {
        game.randomize_move_categories();
    }
    if options.ban_lucky_egg {
        game.ban_lucky_egg();
    }
    if options.ban_maniac_items {
        game.ban_maniac_items();
    }
    if options.no_free_lucky_egg {
        game.remove_free_lucky_eggs()?;
    }
    if options.run_without_shoes {
        game.run_without_shoes()?;
    }
    if options.force_challenge_mode {
        game.force_challenge_mode();
    }
    if options.no_low_hp_music {
        game.disable_low_hp_music()?;
    }
    if options.national_dex {
        game.national_dex_at_start()?;
    }
    if options.update_type_effectiveness {
        game.update_type_effectiveness()?;
    }
    if let Some((slow, speed)) = options.level_alike {
        game.level_alike(slow, speed);
    }
    game.randomize_types(options.types, options.two_types);
    if options.random_wild_held_items {
        game.randomize_wild_held_items(options.good_wild_held_items);
    }
    if let Some(how) = options.random_evolutions {
        game.randomize_evolutions(how)?;
    }
    match options.stats {
        Stats::Unchanged => {}
        Stats::Shuffle => game.shuffle_stats(options.stats_follow_evolutions),
        Stats::Random => game.randomize_stats(
            options.stats_follow_evolutions,
            options.stats_grow_at_random,
        ),
    }
    game.randomize_abilities(
        options.abilities,
        &options.ability_rules,
        options.double_battles,
    );
    if options.change_impossible_evolutions {
        game.remove_impossible_evolutions(options.movesets != Movesets::Unchanged)?;
    }
    if options.easier_evolutions {
        game.make_evolutions_easier(options.random_wild);
    }
    if options.no_time_evolutions {
        game.remove_time_based_evolutions();
    }
    game.choose_starters(options.starters)?;
    if matches!(options.movesets, Movesets::Random | Movesets::PreferType) {
        game.randomize_movesets(
            options.movesets == Movesets::PreferType,
            usize::from(options.moves_at_start),
            options.good_attacks,
            options.no_broken_moves,
        )?;
    }
    if options.order_moves_by_damage {
        game.order_moves_by_damage()?;
    }
    if options.random_tm_moves && options.movesets != Movesets::Metronome {
        game.randomize_tm_moves(&options.tm_rules)?;
    }
    if options.machines != Compat::Unchanged {
        game.change_machine_compatibility(options.machines, options.tm_rules.follow_evolutions)?;
    }
    if options.tm_rules.keep_learnt {
        game.keep_learnt_machines(options.tm_rules.follow_evolutions)?;
    }
    if options.full_hm {
        game.full_hm_compatibility();
    }
    if options.random_tutor_moves && options.movesets != Movesets::Metronome {
        game.randomize_tutor_moves(&options.tutor_rules)?;
    }
    if options.tutors != Compat::Unchanged {
        game.change_tutor_compatibility(options.tutors, options.tutor_rules.follow_evolutions)?;
    }
    if options.tutor_rules.keep_learnt {
        game.keep_learnt_tutors(options.tutor_rules.follow_evolutions)?;
    }
    let [regular, important, boss] = options.more_trainer_pokemon;
    if regular > 0 || important > 0 || boss > 0 {
        game.add_trainer_pokemon(regular, important, boss)?;
    }
    if options.double_battles {
        game.double_battles()?;
    }
    if options.random_trainers {
        game.randomize_trainers(
            options.trainer_forms,
            &options.trainer_rules,
            options.trainers_fully_evolved_from,
            options.rival_carries_starter,
        )?;
    } else if options.trainer_rules.levels != 0 {
        game.change_trainer_levels(options.trainer_rules.levels)?;
    }
    if options.rival_carries_starter
        && (options.random_trainers || options.starters != Starters::Unchanged)
    {
        game.rival_carries_starter()?;
    }
    if options.trainers_fully_evolved_from > 0 {
        game.force_fully_evolved(options.trainers_fully_evolved_from)?;
    }
    if options.better_trainer_moves {
        let cyclic = options.random_evolutions.is_some_and(|e| e.every_level);
        game.pick_trainer_moves(cyclic, options.double_battles)?;
    }
    if options.trainer_items.any() {
        game.randomize_trainer_items(&options.trainer_items)?;
    }
    if options.random_trainer_class_names {
        game.randomize_trainer_class_names()?;
    }
    if options.random_trainer_names {
        game.randomize_trainer_names()?;
    }
    if options.movesets == Movesets::Metronome {
        game.metronome_only()?;
    }
    if options.random_statics {
        game.randomize_statics(&options.static_rules)?;
    } else if options.static_rules.levels != 0 {
        game.change_static_levels(options.static_rules.levels)?;
    }
    if options.min_catch_rate != 0 {
        game.raise_catch_rates(options.min_catch_rate)?;
    }
    if options.random_wild {
        game.randomize_wild(options.wild_forms, &options.wild_rules)?;
    } else if options.wild_rules.levels != 0 {
        game.change_wild_levels(options.wild_rules.levels)?;
    }
    if options.random_trades {
        game.randomize_trades(
            options.trades_want_random,
            options.random_trade_nicknames,
            options.random_trade_trainers,
            options.random_trade_ivs,
            options.random_trade_items,
        )?;
    }
    if options.random_field_items {
        game.randomize_field_items(options.good_field_items, options.even_field_items)?;
    } else if options.shuffle_field_items {
        game.shuffle_field_items()?;
    }
    if options.random_shops {
        game.randomize_shops(options.good_shop_items, &options.shop_rules)?;
    } else if options.shuffle_shops {
        game.shuffle_shops()?;
    }
    if options.random_pickup {
        game.randomize_pickup(options.good_pickup_items)?;
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
            abilities: [1, 0, 0],
            base: 0,
            form: 0,
            looks: 0,
            looks_only: false,
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
        remove_impossible(&mut all, None);
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
    fn every_hm_is_learnable_and_nothing_past_them() {
        let mut stats = vec![0u8; 76];
        stats[MACHINES] = 0b101;
        stats[MACHINES + 12] = 0b1110_0000;
        full_hms(&mut stats);
        // TMs stay, HMs 1 to 6 are bits 95 to 100.
        assert_eq!(stats[MACHINES], 0b101);
        assert_eq!(stats[MACHINES + 11], 0b1000_0000);
        assert_eq!(stats[MACHINES + 12], 0b0001_1111);
    }

    #[test]
    fn random_items_leave_out_what_nobody_may_get() {
        let (all, mut good) = (Items::new(false), Items::new(true));
        // No "none", no TM, no key item; the Lucky Egg until it is banned.
        for item in [0, 328, 427, 428, 620, LAST_ITEM] {
            assert!(!all.allowed[item], "{item}");
        }
        assert!(all.allowed[1] && all.allowed[580] && all.allowed[155]);
        assert!(!good.allowed[155] && good.allowed[LUCKY_EGG]);
        good.ban(LUCKY_EGG);
        let mut random = JavaRandom::new(1);
        for _ in 0..2000 {
            let item = usize::from(good.random(&mut random));
            assert!(good.allowed[item] && item != LUCKY_EGG);
        }
    }

    #[test]
    fn held_items_stay_sound() {
        let mut all = species(&[(1, &[]), (2, &[]), (3, &[])]);
        all[2].items.always = 5;
        let mut random = JavaRandom::new(3);
        for _ in 0..200 {
            random_held_items(&mut all, &Items::new(true), &mut random);
            for species in &all[1..] {
                let held = species.items;
                if held.always > 0 {
                    assert_eq!((held.common, held.rare, held.dark_grass), (0, 0, 0));
                } else if held.common > 0 {
                    assert_ne!(held.common, held.rare);
                }
            }
        }
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
