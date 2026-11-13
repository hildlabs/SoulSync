//! The trades people in the game offer.

use super::*;

const TRADES: &str = "a/1/6/3";
/// The text with, for each trade, the nickname of the Pokémon given and
/// the name of its first trainer.
const TRADE_NAMES: usize = 37;
/// Places in a trade's file, each four bytes.
const GIVEN: usize = 4;
const GIVEN_FORM: usize = 8;
const IVS: usize = 0x10;
const NATURE: usize = 0x2C;
const ITEM: usize = 0x4C;
const WANTED: usize = 0x5C;
const TRADE_LEN: usize = 0x60;
const ANY_NATURE: u32 = 0xFF;
/// The trade file each game leaves unused.
const UNUSED_BLACK: usize = 25;
const UNUSED_WHITE: usize = 24;
/// The story texts of the people offering the trades from the 25th on,
/// which name the Pokémon.
const FIRST_SPOKEN: usize = 24;
const SPOKEN_BLACK: [usize; 6] = [529, 555, 193, 594, 628, 628];
const SPOKEN_WHITE: [usize; 6] = [537, 555, 193, 594, 628, 628];
const LONGEST_NICKNAME: usize = 10;

struct Trade {
    file: usize,
    given: usize,
    wanted: usize,
    nickname: Vec<u16>,
    ivs: [u32; 6],
    item: u32,
}

fn long(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

fn set_long(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Whether two texts are the same apart from upper and lower case.
fn same_name(a: &[u16], b: &[u16]) -> bool {
    let fold = |c: &u16| {
        let c = char::from_u32(u32::from(*c)).unwrap_or('\u{0}');
        let mut upper = c.to_uppercase();
        let single = upper.next().filter(|_| upper.next().is_none()).unwrap_or(c);
        single.to_lowercase().next().unwrap_or(single)
    };
    a.len() == b.len() && a.iter().map(fold).eq(b.iter().map(fold))
}

/// `line` with every `old` replaced by `new`.
fn replace(line: &[u16], old: &[u16], new: &[u16]) -> Vec<u16> {
    let mut out = Vec::with_capacity(line.len());
    let mut at = 0;
    while at < line.len() {
        if !old.is_empty() && line[at..].starts_with(old) {
            out.extend_from_slice(new);
            at += old.len();
        } else {
            out.push(line[at]);
            at += 1;
        }
    }
    out
}

impl<'a> Game<'a> {
    /// Every trade gives a random Pokémon; with `wanted` it also asks for
    /// one. Nicknames, IVs and held items are random as chosen.
    pub fn randomize_trades(
        &mut self,
        wanted: bool,
        nicknames: bool,
        ivs: bool,
        items: bool,
    ) -> Result<(), RomError> {
        let mut files = Self::narc(&self.nds, TRADES)?;
        let text = |game: &Self, file: usize| {
            let text = game.strings.get(file).and_then(|data| Text::read(data));
            text.ok_or(layout("text of the trades"))
        };
        let mut names = text(self, TRADE_NAMES)?;
        let species_names = text(self, SPECIES_NAMES)?.lines;
        let white = self.code[2] == b'D';
        let unused = if white { UNUSED_WHITE } else { UNUSED_BLACK };

        let mut trades = Vec::new();
        for (file, data) in files.iter().enumerate() {
            if file == unused {
                continue;
            }
            if data.len() < TRADE_LEN || names.lines.len() < file * 2 + 2 {
                return Err(layout(TRADES));
            }
            let (given, wanted) = (long(data, GIVEN) as usize, long(data, WANTED) as usize);
            if !(1..=SPECIES).contains(&given) || !(1..=SPECIES).contains(&wanted) {
                return Err(layout(TRADES));
            }
            let mut trade = Trade {
                file,
                given,
                wanted,
                nickname: names.lines[file * 2].clone(),
                ivs: [0; 6],
                item: long(data, ITEM),
            };
            for (i, iv) in trade.ivs.iter_mut().enumerate() {
                *iv = long(data, IVS + i * 4);
            }
            trades.push(trade);
        }
        let before: Vec<(usize, usize)> = trades.iter().map(|t| (t.given, t.wanted)).collect();

        let mut pool: Vec<Vec<u16>> = Vec::new();
        if nicknames {
            let fitting = names::NICKNAMES
                .iter()
                .filter(|n| n.chars().count() <= LONGEST_NICKNAME);
            for name in fitting.map(|n| chars(n)) {
                if !pool.contains(&name) {
                    pool.push(name);
                }
            }
        }
        let (mut all_given, mut all_wanted, mut used) = (Vec::new(), Vec::new(), Vec::new());
        for trade in &mut trades {
            let old = trade.given;
            let mut given = 1 + self.random.index(SPECIES);
            while all_given.contains(&given) {
                given = 1 + self.random.index(SPECIES);
            }
            all_given.push(given);
            trade.given = given;
            if old == trade.wanted {
                trade.wanted = given;
            } else if wanted {
                let mut asked = 1 + self.random.index(SPECIES);
                while all_wanted.contains(&asked) || asked == given {
                    asked = 1 + self.random.index(SPECIES);
                }
                all_wanted.push(asked);
                trade.wanted = asked;
            }
            if pool.len() > used.len() {
                let mut pick = self.random.index(pool.len());
                while used.contains(&pick) {
                    pick = self.random.index(pool.len());
                }
                used.push(pick);
                trade.nickname = pool[pick].clone();
            } else if same_name(&trade.nickname, &species_names[old]) {
                trade.nickname = species_names[given].clone();
            }
            if ivs {
                for iv in &mut trade.ivs {
                    *iv = self.random.index(32) as u32;
                }
            }
            if items {
                trade.item = u32::from(self.items.random(&mut self.random));
            }
        }

        for trade in &trades {
            let data = &mut files[trade.file];
            names.lines[trade.file * 2] = trade.nickname.clone();
            set_long(data, GIVEN, trade.given as u32);
            set_long(data, GIVEN_FORM, 0);
            for (i, iv) in trade.ivs.iter().enumerate() {
                set_long(data, IVS + i * 4, *iv);
            }
            set_long(data, NATURE, ANY_NATURE);
            set_long(data, ITEM, trade.item);
            set_long(data, WANTED, trade.wanted as u32);
        }
        self.put_narc(TRADES, &files)?;
        let written = names.write(&self.strings[TRADE_NAMES]);
        self.strings[TRADE_NAMES] = written.ok_or(layout("text of the trades"))?;

        // What the people say names both Pokémon. The original counts the
        // trades here as if none were unused, and so does this.
        let spoken = if white { SPOKEN_WHITE } else { SPOKEN_BLACK };
        for (i, file) in spoken.into_iter().enumerate() {
            let at = FIRST_SPOKEN + i;
            if at == unused {
                continue;
            }
            if at >= trades.len() {
                break;
            }
            let (old, new) = (before[at], &trades[at]);
            let mut renamed = vec![(&species_names[old.0], &species_names[new.given])];
            if old.1 != new.wanted {
                let other = (&species_names[old.1], &species_names[new.wanted]);
                match renamed.iter_mut().find(|r| r.0 == other.0) {
                    Some(same) => *same = other,
                    None => renamed.push(other),
                }
            }
            renamed.sort_by(|a, b| a.0.cmp(b.0));
            let data = self.story.get(file).ok_or(layout("text of the trades"))?;
            let mut text = Text::read(data).ok_or(layout("text of the trades"))?;
            for line in &mut text.lines {
                for (old, new) in &renamed {
                    *line = replace(line, old, new);
                }
            }
            let written = text.write(&self.story[file]);
            self.story[file] = written.ok_or(layout("text of the trades"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_replaced_everywhere() {
        let line = chars("Abra? Abra for Mew!");
        assert_eq!(
            replace(&line, &chars("Abra"), &chars("Onix")),
            chars("Onix? Onix for Mew!")
        );
        assert_eq!(
            replace(&line, &chars("Mew!"), &[]),
            chars("Abra? Abra for ")
        );
        assert_eq!(replace(&line, &[], &chars("x")), line);
    }

    #[test]
    fn case_does_not_tell_names_apart() {
        assert!(same_name(&chars("PIKACHU"), &chars("Pikachu")));
        assert!(same_name(&chars("Flabébé"), &chars("FLABÉBÉ")));
        assert!(!same_name(&chars("Pikachu"), &chars("Pikachus")));
        assert!(!same_name(&chars("Mew"), &chars("Mow")));
    }
}
