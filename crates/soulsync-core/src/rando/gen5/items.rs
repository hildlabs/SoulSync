//! Items lying around (in item balls and hidden), the goods of the
//! shops and what the ability Pickup finds.

use super::*;

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
    /// Random items lie around, with `good` only useful ones. TMs stay
    /// TMs, and those the game needs stay to be found.
    pub fn randomize_field_items(&mut self, good: bool) -> Result<(), RomError> {
        let mut all: Vec<(usize, usize)> = Vec::new();
        for (file, command) in [(ITEM_BALLS, BALL_COMMAND), (HIDDEN_ITEMS, HIDDEN_COMMAND)] {
            let script = self.scripts.get(file).ok_or(layout("items lying around"))?;
            all.extend(places(script, command).into_iter().map(|at| (file, at)));
        }
        let old: Vec<u16> = all
            .iter()
            .map(|&(file, at)| word(&self.scripts[file], at))
            .collect();
        // The original asks its list of items before any is banned.
        let free = Items::new(false);
        let plain = |item: u16| free.allowed.get(usize::from(item)) == Some(&true);
        let plain_count = old.iter().filter(|&&i| plain(i) && !is_tm(i)).count();
        let tm_count = old.iter().filter(|&&i| is_tm(i)).count();

        let items = if good { &self.good_items } else { &self.items };
        let mut new: Vec<u16> = (0..plain_count)
            .map(|_| items.random(&mut self.random))
            .collect();
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

        let (mut new, mut tms) = (new.into_iter(), tms.into_iter());
        for (&(file, at), &item) in all.iter().zip(&old) {
            let chosen = if is_tm(item) {
                tms.next().map(tm_item)
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

    /// The special shops sell random goods, none of them twice, with
    /// `good` only useful ones.
    pub fn randomize_shops(&mut self, good: bool) -> Result<(), RomError> {
        let mut shops = Self::narc(&self.nds, SHOPS)?;
        if shops.len() < SHOP_COUNT {
            return Err(layout(SHOPS));
        }
        let items = if good { &self.good_items } else { &self.items };
        let mut sold: Vec<u16> = Vec::new();
        for (number, shop) in shops.iter_mut().enumerate().take(SHOP_COUNT) {
            if KEPT_SHOPS.contains(&number) {
                continue;
            }
            for _ in 0..shop.len().div_ceil(2) {
                let item = loop {
                    let item = items.random(&mut self.random);
                    if !sold.contains(&item) {
                        break item;
                    }
                };
                sold.push(item);
            }
        }
        let mut sold = sold.into_iter();
        for (number, shop) in shops.iter_mut().enumerate().take(SHOP_COUNT) {
            if KEPT_SHOPS.contains(&number) {
                continue;
            }
            for place in shop.chunks_exact_mut(2) {
                let item = sold.next().ok_or(layout(SHOPS))?;
                place.copy_from_slice(&item.to_le_bytes());
            }
        }
        self.put_narc(SHOPS, &shops)
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
