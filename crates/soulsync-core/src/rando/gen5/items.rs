//! Items lying around (in item balls and hidden), the goods of the
//! shops and what the ability Pickup finds.

use super::*;
use std::collections::BTreeMap;

/// The scripts with one small script per item ball and per hidden item.
const ITEM_BALLS: usize = 1240;
const HIDDEN_ITEMS: usize = 1241;
/// The end of the list of scripts at the start of such a file, and the
/// command and variable by which a script names its item.
const LIST_END: u16 = 0xFD13;
const BALL_COMMAND: (u16, u16) = (0x28, 0x800C);
const HIDDEN_COMMAND: (u16, u16) = (0x2A, 0x8000);
/// The items that are TMs: TM01 to TM92 and TM93 to TM95.
const FIRST_TM: u16 = 328;
const TM93: u16 = 618;
const TMS_BEFORE_93: u16 = 92;
/// TMs that have to lie around somewhere for the game to go on.
const NEEDED_TMS: [u16; 39] = [
    1, 2, 3, 5, 6, 12, 13, 19, 22, 26, 28, 29, 30, 36, 39, 41, 46, 47, 50, 52, 53, 56, 58, 61, 63,
    65, 66, 67, 69, 71, 80, 81, 84, 85, 86, 90, 91, 92, 93,
];

/// The goods of each shop, one file per shop. The shops that sell the
/// usual goods and those for TMs keep theirs.
const SHOPS: &str = "a/2/8/2";
const SHOP_COUNT: usize = 32;
const KEPT_SHOPS: [usize; 11] = [0, 1, 2, 3, 4, 5, 7, 10, 12, 17, 24];
/// The overlay with the items Pickup finds, where they start and how
/// many there are.
const PICKUP_OVERLAY: usize = 166;
const PICKUP_MARK: [u8; 6] = [0x19, 0x00, 0x5C, 0x00, 0xDD, 0x00];
const PICKUP_ITEMS: usize = 29;

/// The shops that are met in the course of the story.
const MAIN_SHOPS: [usize; 17] = [
    9, 11, 14, 15, 16, 18, 20, 21, 22, 23, 25, 26, 27, 28, 29, 30, 31,
];
/// What the usual shops sell anyway: balls, medicine and repels.
const USUAL_GOODS: [(usize, usize); 3] = [(2, 3), (17, 12), (76, 4)];
/// Goods that are worth too much or make the game too easy.
const OVERPOWERED: [(usize, usize); 10] = [
    (42, 2),
    (50, 1),
    (54, 1),
    (65, 7),
    (86, 7),
    (106, 1),
    (206, 7),
    (231, 1),
    (571, 1),
    (580, 12),
];
/// The items some Pokémon need to evolve, and those that raise a stat
/// for one battle.
const EVOLUTION_ITEMS: [u16; 24] = [
    80, 81, 82, 83, 84, 85, 107, 108, 109, 110, 221, 226, 227, 233, 235, 252, 321, 322, 323, 324,
    325, 326, 327, 537,
];
const BATTLE_ITEMS: [u16; 8] = [55, 56, 57, 58, 59, 60, 61, 62];
/// What every item costs, a word at the start of its file, and the
/// original randomizer's fairer prices by the number of the item, a
/// tenth of what is paid.
const ITEM_DATA: &str = "a/0/2/4";
#[rustfmt::skip]
const FAIR_PRICES: [u16; 639] = [
    0, 300, 120, 60, 20, 50, 100, 100, 100, 100, 100, 100, 20, 100, 30, 100, 20, 30, 10, 25,
    25, 25, 20, 300, 250, 120, 70, 60, 150, 400, 40, 60, 70, 80, 40, 110, 45, 280, 300, 450,
    1500, 1800, 45, 10, 1000, 980, 980, 980, 980, 980, 1000, 980, 980, 2490, 45, 70, 65, 50, 55, 35,
    95, 35, 35, 100, 100, 2, 2, 2, 2, 2, 2, 2, 40, 40, 40, 40, 50, 70, 55, 35,
    300, 300, 300, 300, 300, 300, 50, 500, 140, 750, 200, 980, 1000, 500, 50, 20, 20, 20, 20, 500,
    500, 500, 500, 800, 500, 500, 1000, 300, 300, 300, 300, 210, 1000, 0, 0, 0, 100, 100, 100, 100,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 15, 1000, 1000, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 20, 25, 10, 25, 25, 300, 5, 20, 50, 50, 10,
    10, 10, 10, 10, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50,
    50, 50, 50, 50, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100,
    100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 300, 100, 300, 600, 450, 100, 100,
    1000, 500, 200, 1500, 100, 20, 300, 300, 20, 20, 300, 1000, 500, 300, 1000, 300, 10, 200, 200, 200,
    200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 300, 600, 200, 300, 1, 1, 50, 20,
    10, 10, 10, 10, 10, 150, 200, 200, 600, 150, 1000, 100, 150, 150, 1, 200, 150, 300, 100, 100,
    150, 500, 20, 20, 20, 20, 150, 1000, 150, 300, 300, 300, 300, 300, 300, 50, 150, 1000, 200, 200,
    200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 100, 200, 200, 1500,
    100, 300, 300, 300, 300, 300, 500, 500, 1000, 1000, 1000, 1000, 1000, 1000, 2000, 1000, 1000, 1000, 2000, 1000,
    1000, 2000, 2000, 2000, 1000, 2000, 1000, 2000, 1000, 1000, 1000, 1000, 2000, 1000, 1000, 1000, 1000, 1000, 1000, 1000,
    2000, 1000, 1000, 1000, 2000, 2000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000,
    1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 2000, 1000, 1000, 1000, 1000,
    1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 2, 2, 2, 2, 2, 2, 2, 30, 30, 30, 30, 30, 30, 30, 30,
    0, 0, 0, 0, 1500, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 300, 1000, 100,
    600, 100, 100, 100, 200, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100,
    100, 100, 100, 100, 100, 300, 300, 300, 300, 300, 300, 20, 500, 500, 0, 20, 100, 100, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 45, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1000, 1000,
    1000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// What the special shops may sell, and for how much.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShopRules {
    /// Nothing the usual shops sell anyway.
    pub no_usual_goods: bool,
    /// Nothing worth too much or too strong.
    pub no_overpowered: bool,
    /// Every item gets a fairer price.
    pub fair_prices: bool,
    /// Every item a Pokémon needs to evolve is sold somewhere.
    pub evolution_items: bool,
    /// Every item that raises a stat for a battle is sold somewhere.
    pub battle_items: bool,
}

/// The places of the items lying around, file and offset, and the
/// items there.
type Lying = (Vec<(usize, usize)>, Vec<u16>);
/// The goods of each special shop, with the number of the shop.
type Goods = Vec<(usize, Vec<u16>)>;

fn is_tm(item: u16) -> bool {
    (FIRST_TM..FIRST_TM + TMS_BEFORE_93).contains(&item) || (TM93..TM93 + 3).contains(&item)
}

fn tm_item(tm: u16) -> u16 {
    if tm <= TMS_BEFORE_93 {
        tm + FIRST_TM - 1
    } else {
        tm - TMS_BEFORE_93 + TM93 - 1
    }
}

/// Where the items stand in a file of item scripts.
fn places(script: &[u8], command: (u16, u16)) -> Vec<usize> {
    let mut found = Vec::new();
    let mut at = 0;
    while at + 4 <= script.len() && word(script, at) != LIST_END {
        let jump = i32::from_le_bytes([script[at], script[at + 1], script[at + 2], script[at + 3]]);
        let start = (at as i64 + 4 + i64::from(jump)) as usize;
        at += 4;
        if start > script.len() {
            break;
        }
        if start + 8 <= script.len()
            && (word(script, start + 2), word(script, start + 4)) == command
        {
            found.push(start + 6);
        }
    }
    found
}

impl<'a> Game<'a> {
    /// Where the items lying around stand in the scripts, and which.
    fn field_items(&self) -> Result<Lying, RomError> {
        let mut all: Vec<(usize, usize)> = Vec::new();
        for (file, command) in [(ITEM_BALLS, BALL_COMMAND), (HIDDEN_ITEMS, HIDDEN_COMMAND)] {
            let script = self.scripts.get(file).ok_or(layout("items lying around"))?;
            all.extend(places(script, command).into_iter().map(|at| (file, at)));
        }
        let old = (all.iter())
            .map(|&(file, at)| word(&self.scripts[file], at))
            .collect();
        Ok((all, old))
    }

    /// Puts `new` where plain items lay and `tms`, as items, where TMs
    /// lay; anything else stays.
    fn set_field_items(
        &mut self,
        (all, old): Lying,
        new: Vec<u16>,
        tms: Vec<u16>,
    ) -> Result<(), RomError> {
        let free = Items::new(false);
        let plain = |item: u16| free.allowed.get(usize::from(item)) == Some(&true);
        let (mut new, mut tms) = (new.into_iter(), tms.into_iter());
        for (&(file, at), &item) in all.iter().zip(&old) {
            let chosen = if is_tm(item) {
                tms.next()
            } else if plain(item) {
                new.next()
            } else {
                continue;
            };
            let chosen = chosen.ok_or(layout("items lying around"))?;
            set_word(&mut self.scripts[file], at, chosen);
        }
        Ok(())
    }

    /// Random items lie around, with `good` only useful ones and with
    /// `even` none much more often than the others. TMs stay TMs, and
    /// those the game needs stay to be found.
    pub fn randomize_field_items(&mut self, good: bool, even: bool) -> Result<(), RomError> {
        let (all, old) = self.field_items()?;
        // The original asks its list of items before any is banned.
        let free = Items::new(false);
        let plain = |item: u16| free.allowed.get(usize::from(item)) == Some(&true);
        let plain_count = old.iter().filter(|&&i| plain(i) && !is_tm(i)).count();
        let tm_count = old.iter().filter(|&&i| is_tm(i)).count();

        let items = if good { &self.good_items } else { &self.items };
        let mut new: Vec<u16> = Vec::with_capacity(plain_count);
        // How often each item lies around so far.
        let mut placed: BTreeMap<u16, u32> = BTreeMap::new();
        for _ in 0..plain_count {
            let mut chosen = items.random(&mut self.random);
            if even {
                // Nothing placed yet is no average at all, as in the
                // original, and lets anything through.
                let average = placed.values().sum::<u32>() as f32 / placed.len() as f32;
                let often = |item: u16| placed.get(&item).copied().unwrap_or(0) as f32;
                let mut tries = 0;
                while often(chosen) > average && tries < 100 {
                    chosen = items.random(&mut self.random);
                    tries += 1;
                }
                *placed.entry(chosen).or_default() += 1;
            }
            new.push(chosen);
        }
        let mut tms = NEEDED_TMS.to_vec();
        for _ in tms.len()..tm_count {
            loop {
                let tm = self.random.index(TMS) as u16 + 1;
                if !tms.contains(&tm) {
                    tms.push(tm);
                    break;
                }
            }
        }
        self.random.shuffle(&mut new);
        self.random.shuffle(&mut tms);
        let tms = tms.into_iter().map(tm_item).collect();
        self.set_field_items((all, old), new, tms)
    }

    /// The items lying around change places: the TMs among themselves,
    /// and all others.
    pub fn shuffle_field_items(&mut self) -> Result<(), RomError> {
        let (all, old) = self.field_items()?;
        let free = Items::new(false);
        let plain = |item: &u16| free.allowed.get(usize::from(*item)) == Some(&true);
        let mut new: Vec<u16> = (old.iter().copied())
            .filter(|i| plain(i) && !is_tm(*i))
            .collect();
        let mut tms: Vec<u16> = old.iter().copied().filter(|&i| is_tm(i)).collect();
        self.random.shuffle(&mut new);
        self.random.shuffle(&mut tms);
        self.set_field_items((all, old), new, tms)
    }

    /// The goods of the special shops, each with the number of its shop.
    fn shops(&self) -> Result<(Vec<Vec<u8>>, Goods), RomError> {
        let shops = Self::narc(&self.nds, SHOPS)?;
        if shops.len() < SHOP_COUNT {
            return Err(layout(SHOPS));
        }
        let goods = (shops.iter().enumerate().take(SHOP_COUNT))
            .filter(|(number, _)| !KEPT_SHOPS.contains(number))
            .map(|(number, shop)| {
                let goods = shop.chunks_exact(2).map(|w| word(w, 0)).collect();
                (number, goods)
            })
            .collect();
        Ok((shops, goods))
    }

    fn set_shops(
        &mut self,
        mut shops: Vec<Vec<u8>>,
        goods: &[(usize, Vec<u16>)],
    ) -> Result<(), RomError> {
        for (number, goods) in goods {
            for (place, item) in shops[*number].chunks_exact_mut(2).zip(goods) {
                place.copy_from_slice(&item.to_le_bytes());
            }
        }
        self.put_narc(SHOPS, &shops)
    }

    /// The goods of the special shops change places.
    pub fn shuffle_shops(&mut self) -> Result<(), RomError> {
        let (shops, mut goods) = self.shops()?;
        let mut all: Vec<u16> = goods.iter().flat_map(|(_, g)| g.iter().copied()).collect();
        self.random.shuffle(&mut all);
        for (place, item) in goods.iter_mut().flat_map(|(_, g)| g).zip(all) {
            *place = item;
        }
        self.set_shops(shops, &goods)
    }

    /// The special shops sell random goods, none of them twice, with
    /// `good` only useful ones, and as `rules` say.
    pub fn randomize_shops(&mut self, good: bool, rules: &ShopRules) -> Result<(), RomError> {
        let (shops, mut goods) = self.shops()?;
        // What the shops may not sell, nothing later may give either:
        // the original takes it off its list for good.
        let items = if good {
            &mut self.good_items
        } else {
            &mut self.items
        };
        let usual = USUAL_GOODS.iter().filter(|_| rules.no_usual_goods);
        let overpowered = OVERPOWERED.iter().filter(|_| rules.no_overpowered);
        for &(first, count) in usual.chain(overpowered) {
            (first..first + count).for_each(|item| items.ban(item));
        }
        let items = items.clone();

        let mut sure: Vec<u16> = Vec::new();
        if rules.evolution_items {
            sure.extend(EVOLUTION_ITEMS);
        }
        if rules.battle_items {
            sure.extend(BATTLE_ITEMS);
        }
        let count: usize = goods.iter().map(|(_, g)| g.len()).sum();
        let mut sold = sure.clone();
        for _ in sure.len()..count {
            let item = loop {
                let item = items.random(&mut self.random);
                if !sold.contains(&item) {
                    break item;
                }
            };
            sold.push(item);
        }
        if rules.evolution_items || rules.battle_items {
            // The shops off the way get none of the sure goods, so that
            // those are all sold where everybody comes by.
            self.random.shuffle(&mut sold);
            let main = |number: &usize| MAIN_SHOPS.contains(number);
            for (_, goods) in goods.iter_mut().filter(|(number, _)| !main(number)) {
                let mut at = 0;
                for place in goods.iter_mut() {
                    while sold.get(at).is_some_and(|item| sure.contains(item)) {
                        at += 1;
                    }
                    if at >= sold.len() {
                        return Err(layout(SHOPS));
                    }
                    *place = sold.remove(at);
                }
            }
            self.random.shuffle(&mut sold);
            let mut sold = sold.into_iter();
            for (_, goods) in goods.iter_mut().filter(|(number, _)| main(number)) {
                for place in goods.iter_mut() {
                    *place = sold.next().ok_or(layout(SHOPS))?;
                }
            }
        } else {
            for (place, item) in goods.iter_mut().flat_map(|(_, g)| g).zip(sold) {
                *place = item;
            }
        }
        self.set_shops(shops, &goods)?;

        if rules.fair_prices {
            let mut data = Self::narc(&self.nds, ITEM_DATA)?;
            for (item, price) in data.iter_mut().zip(FAIR_PRICES).skip(1) {
                if item.len() < 2 {
                    return Err(layout(ITEM_DATA));
                }
                set_word(item, 0, price);
            }
            self.put_narc(ITEM_DATA, &data)?;
        }
        Ok(())
    }

    /// Pickup finds random items, with `good` only useful ones.
    pub fn randomize_pickup(&mut self, good: bool) -> Result<(), RomError> {
        let mut code = self.nds.overlay(PICKUP_OVERLAY).ok_or(layout("overlay"))?;
        let table = find(&code, &PICKUP_MARK).filter(|at| at + PICKUP_ITEMS * 2 <= code.len());
        let items = if good { &self.good_items } else { &self.items };
        let new: Vec<u16> = (0..PICKUP_ITEMS)
            .map(|_| items.random(&mut self.random))
            .collect();
        if let Some(table) = table {
            for (i, item) in new.iter().enumerate() {
                set_word(&mut code, table + i * 2, *item);
            }
            self.nds.set_overlay(PICKUP_OVERLAY, &code);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sure_goods_fit_into_the_main_shops() {
        assert!(MAIN_SHOPS.iter().all(|shop| !KEPT_SHOPS.contains(shop)));
        assert!(MAIN_SHOPS.iter().all(|&shop| shop < SHOP_COUNT));
        let free = Items::new(false);
        for item in EVOLUTION_ITEMS.iter().chain(&BATTLE_ITEMS) {
            assert!(free.allowed[usize::from(*item)], "{item}");
        }
        for &(first, count) in USUAL_GOODS.iter().chain(&OVERPOWERED) {
            assert!(first + count <= FAIR_PRICES.len());
        }
        // The Master Ball and the Poké Ball.
        assert_eq!((FAIR_PRICES[1], FAIR_PRICES[4]), (300, 20));
    }

    #[test]
    fn tms_are_two_runs_of_items() {
        assert_eq!(
            (tm_item(1), tm_item(92), tm_item(93), tm_item(95)),
            (328, 419, 618, 620)
        );
        assert!(is_tm(328) && is_tm(419) && is_tm(620));
        assert!(!is_tm(327) && !is_tm(420) && !is_tm(621));
        assert!(NEEDED_TMS.iter().all(|&tm| is_tm(tm_item(tm))));
    }

    #[test]
    fn items_are_found_behind_the_list_of_scripts() {
        // Two scripts and the end of the list; the second one sets
        // another variable and is no item.
        let mut script = vec![0u8; 12];
        script[0..4].copy_from_slice(&8i32.to_le_bytes());
        script[4..8].copy_from_slice(&12i32.to_le_bytes());
        script[8..10].copy_from_slice(&LIST_END.to_le_bytes());
        for variable in [0x800Cu16, 0x8001] {
            for value in [0x2E, 0x28, variable, 17] {
                script.extend(value.to_le_bytes());
            }
        }
        assert_eq!(places(&script, BALL_COMMAND), [18]);
        assert!(places(&script, HIDDEN_COMMAND).is_empty());
    }
}
