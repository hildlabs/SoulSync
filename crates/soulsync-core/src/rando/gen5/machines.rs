//! The moves on the TMs, and which Pokémon learn the moves of TMs, HMs
//! and move tutors.

use super::*;

/// The texts that describe the items and the moves.
const ITEM_TEXTS: usize = 63;
const MOVE_TEXTS: usize = 402;
/// The items that are TMs: TM01 to TM92 and TM93 to TM95.
const FIRST_TM_ITEM: usize = 328;
const TM93_ITEM: usize = 618;
/// The pictures of the items stand in the ARM9 from this mark on, four
/// bytes an item, the last two of them its colours.
const ITEM_PICTURES: [u8; 16] = [
    0xFD, 0x03, 0xFE, 0x03, 0x02, 0x00, 0x03, 0x00, 0x04, 0x00, 0x05, 0x00, 0x06, 0x00, 0x07, 0x00,
];
/// The colours of a TM by the type of its move, in the order of [`TYPES`].
const TM_COLOURS: [u16; 17] = [
    402, 398, 413, 403, 410, 412, 610, 411, 408, 406, 400, 405, 409, 401, 404, 399, 407,
];
/// Where a species' stats say which tutor moves it learns: four tutors
/// with four bytes each. The tutors stand in another order there than
/// their moves do in the overlay: how many moves each has in the stats'
/// order, and where its moves start in the overlay's.
pub(super) const TUTOR_FLAGS: usize = 60;
pub(super) const TUTOR_BLOCKS: [(usize, usize); 4] = [(15, 13), (17, 43), (13, 0), (15, 28)];
/// The type a move's data starts with.
const NORMAL: u8 = 0;

impl Game<'_> {
    /// Every TM gets a random move no HM teaches.
    pub fn randomize_tm_moves(&mut self) -> Result<(), RomError> {
        let (_, hms) = self.machine_moves().ok_or(layout("moves of the TMs"))?;
        let mut usable: Vec<u16> = (1..=MOVES_COUNT as u16)
            .filter(|m| *m != STRUGGLE && !hms.contains(m))
            .collect();
        let mut picked = Vec::with_capacity(TMS);
        for _ in 0..TMS {
            picked.push(usable.remove(self.random.index(usable.len())));
        }
        self.random.shuffle(&mut picked);
        self.set_tm_moves(&picked)
    }

    /// Puts `picked` into the TMs, one move each.
    pub(super) fn set_tm_moves(&mut self, picked: &[u16]) -> Result<(), RomError> {
        // The HMs stand between TM92 and TM93, in the ARM9 as in the items.
        let place = |tm: usize| if tm < FIRST_TMS { tm } else { tm + HMS };
        let item = |tm: usize| {
            if tm < FIRST_TMS {
                FIRST_TM_ITEM + tm
            } else {
                TM93_ITEM + tm - FIRST_TMS
            }
        };
        let at = find(&self.arm9, &MACHINE_MOVES).ok_or(layout("moves of the TMs"))?
            + MACHINE_MOVES.len();
        for (tm, &chosen) in picked.iter().enumerate() {
            set_word(&mut self.arm9, at + place(tm) * 2, chosen);
        }

        // A TM is described as its move is.
        let texts = (self.strings.get(ITEM_TEXTS), self.strings.get(MOVE_TEXTS));
        let (Some(items), Some(moves)) = texts else {
            return Err(layout("text of the items"));
        };
        let moves = Text::read(moves).ok_or(layout("text of the moves"))?;
        let mut text = Text::read(items).ok_or(layout("text of the items"))?;
        for (tm, &chosen) in picked.iter().enumerate() {
            let words = moves.lines.get(usize::from(chosen)).cloned();
            let line = text.lines.get_mut(item(tm));
            let (Some(words), Some(line)) = (words, line) else {
                return Err(layout("text of the items"));
            };
            *line = words;
        }
        let written = text.write(&self.strings[ITEM_TEXTS]);
        self.strings[ITEM_TEXTS] = written.ok_or(layout("text of the items"))?;

        // And it has the colour of its move's type.
        if let Some(pictures) = find(&self.arm9, &ITEM_PICTURES) {
            for (tm, &chosen) in picked.iter().enumerate() {
                let kind = self.moves.get(usize::from(chosen)).map_or(NORMAL, |m| m[0]);
                let colours = TM_COLOURS.get(usize::from(kind)).copied().unwrap_or(411);
                let at = pictures + item(tm) * 4 + 2;
                if at + 2 <= self.arm9.len() {
                    set_word(&mut self.arm9, at, colours);
                }
            }
        }
        Ok(())
    }

    /// Whether a species learns each of `moves`, as `how` says: at random,
    /// more likely for moves of its own types, or every one.
    fn learns(&mut self, number: usize, moves: &[u16], how: Compat) -> Vec<bool> {
        let stats = &self.stats[Self::stats_file(number)];
        let types = [stats[FIRST_TYPE], stats[FIRST_TYPE + 1]];
        moves
            .iter()
            .map(|&m| {
                let kind = self.moves.get(usize::from(m)).map_or(NORMAL, |m| m[0]);
                let chance = match how {
                    Compat::Full => return true,
                    Compat::RandomPreferType if types.contains(&kind) => 0.9,
                    Compat::RandomPreferType if kind != NORMAL => 0.25,
                    _ => 0.5,
                };
                self.random.double() < chance
            })
            .collect()
    }

    /// What a form that only looks different learns is what its species
    /// learns: copies `bytes` of the stats over.
    pub(super) fn copy_to_looks_only(&mut self, bytes: std::ops::Range<usize>) {
        let file = Self::stats_file(LOOKS_ONLY_FORM);
        let form = &self.stats[file];
        if form[FORM_COUNT] > 1 && word(form, FIRST_FORM) == 0 {
            let base = (1..=SPECIES).find(|&number| {
                let stats = &self.stats[number];
                let first = usize::from(word(stats, FIRST_FORM));
                let forms = usize::from(stats[FORM_COUNT]);
                first != 0 && (first..first + forms.saturating_sub(1)).contains(&file)
            });
            if let Some(base) = base {
                let learns = self.stats[base][bytes.clone()].to_vec();
                self.stats[file][bytes].copy_from_slice(&learns);
            }
        }
    }

    /// Which Pokémon learn which TM and HM is decided anew.
    pub fn change_machine_compatibility(&mut self, how: Compat) -> Result<(), RomError> {
        let (mut moves, hms) = self.machine_moves().ok_or(layout("moves of the TMs"))?;
        moves.extend(hms);
        for number in 1..=SPECIES + FORMS {
            let learns = self.learns(number, &moves, how);
            let stats = &mut self.stats[Self::stats_file(number)];
            let machines = &mut stats[MACHINES..MACHINES + MACHINES_LEN];
            machines.fill(0);
            for (bit, _) in learns.iter().enumerate().filter(|(_, &on)| on) {
                machines[bit / 8] |= 1 << (bit % 8);
            }
        }
        self.copy_to_looks_only(MACHINES..MACHINES + MACHINES_LEN);
        Ok(())
    }

    /// The moves the move tutors teach, in the order of the game's list.
    pub(super) fn tutor_moves(&self) -> Result<Vec<u16>, RomError> {
        let place = TUTOR_PLACES.iter().find(|(code, _)| **code == self.code);
        let &(_, place) = place.ok_or(layout("move tutors"))?;
        let code = self.nds.overlay(TUTOR_OVERLAY).ok_or(layout("overlay"))?;
        if code.len() < place + TUTORS * TUTOR_LEN {
            return Err(layout("move tutors"));
        }
        Ok((0..TUTORS)
            .map(|i| word(&code, place + i * TUTOR_LEN))
            .collect())
    }

    /// Which Pokémon learn which move of the move tutors is decided anew.
    pub fn change_tutor_compatibility(&mut self, how: Compat) -> Result<(), RomError> {
        let moves = self.tutor_moves()?;
        for number in 1..=SPECIES + FORMS {
            let learns = self.learns(number, &moves, how);
            let stats = &mut self.stats[Self::stats_file(number)];
            for (tutor, (count, first)) in TUTOR_BLOCKS.into_iter().enumerate() {
                let flags = &mut stats[TUTOR_FLAGS + tutor * 4..TUTOR_FLAGS + tutor * 4 + 4];
                flags.fill(0);
                for bit in (0..count).filter(|bit| learns[first + bit]) {
                    flags[bit / 8] |= 1 << (bit % 8);
                }
            }
        }
        self.copy_to_looks_only(TUTOR_FLAGS..TUTOR_FLAGS + 16);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tutors_cover_all_sixty_moves_once() {
        let mut seen = [0; TUTORS];
        for (count, first) in TUTOR_BLOCKS {
            for slot in &mut seen[first..first + count] {
                *slot += 1;
            }
        }
        assert_eq!(seen, [1; TUTORS]);
    }
}
