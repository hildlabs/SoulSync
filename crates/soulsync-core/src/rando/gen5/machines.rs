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
/// Moves of use outside of battle: Cut, Fly, Surf, Strength, Flash, Dig,
/// Teleport, Waterfall, Sweet Scent and Dive.
const FIELD_MOVES: [u16; 10] = [15, 19, 57, 70, 148, 91, 100, 127, 230, 291];

/// How the moves of the TMs or the move tutors are chosen, and who can
/// be taught them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Taught {
    /// No moves of fixed damage.
    pub no_broken: bool,
    /// Moves of use outside of battle stay where they are.
    pub keep_field: bool,
    /// The share of good attacks at least, in percent.
    pub good_attacks: u8,
    /// What a species learns by itself, it can be taught too.
    pub keep_learnt: bool,
    /// An evolution can be taught what the species before it can, where
    /// who learns what is random, and with `keep_learnt`.
    pub follow_evolutions: bool,
}

/// The type a move's data starts with.
const NORMAL: u8 = 0;

impl Game<'_> {
    /// New moves in place of `old`, none of `taken`, as `rules` say.
    fn pick_taught(&mut self, old: &[u16], taken: &[u16], rules: &Taught) -> Vec<u16> {
        let kept = |m: &u16| rules.keep_field && FIELD_MOVES.contains(m);
        let mut usable: Vec<u16> = (1..=MOVES_COUNT as u16)
            .filter(|m| {
                *m != STRUGGLE
                    && !taken.contains(m)
                    && !(rules.no_broken && movesets::BROKEN.contains(m))
                    && !(kept(m) && old.contains(m))
            })
            .collect();
        let mut attacks: Vec<u16> = (usable.iter().copied())
            .filter(|&m| self.good_attack(m))
            .collect();
        let count = old.iter().filter(|m| !kept(m)).count();
        let share = f64::from(rules.good_attacks) / 100.0;
        let mut attacks_left = traits::round(share * count as f64) as i64;
        let mut picked = Vec::with_capacity(count);
        for _ in 0..count {
            let chosen = if attacks_left > 0 && !attacks.is_empty() {
                attacks[self.random.index(attacks.len())]
            } else {
                usable[self.random.index(usable.len())]
            };
            usable.retain(|m| *m != chosen);
            attacks.retain(|m| *m != chosen);
            attacks_left -= 1;
            picked.push(chosen);
        }
        // Else the good attacks would all come first.
        self.random.shuffle(&mut picked);
        let mut picked = picked.into_iter();
        (old.iter())
            .map(|m| if kept(m) { Some(*m) } else { picked.next() })
            .map(|m| m.unwrap_or(STRUGGLE))
            .collect()
    }

    /// Every TM gets a random move no HM teaches.
    pub fn randomize_tm_moves(&mut self, rules: &Taught) -> Result<(), RomError> {
        let (old, hms) = self.machine_moves().ok_or(layout("moves of the TMs"))?;
        let picked = self.pick_taught(&old, &hms, rules);
        self.set_tm_moves(&picked)
    }

    /// The move tutors teach random moves no TM or HM teaches.
    pub fn randomize_tutor_moves(&mut self, rules: &Taught) -> Result<(), RomError> {
        let (mut taken, hms) = self.machine_moves().unwrap_or_default();
        taken.extend(hms);
        let old = self.tutor_moves()?;
        let picked = self.pick_taught(&old, &taken, rules);
        self.set_tutor_moves(&picked)
    }

    /// A species that learns one of `moves` by itself can also be taught
    /// it: sets the bit `place` gives for the move. With
    /// `follow_evolutions` an evolution can then be taught what the
    /// species before it can, for all of the first `handed_on` bits.
    fn keep_learnt(
        &mut self,
        moves: &[u16],
        handed_on: usize,
        place: &dyn Fn(usize) -> usize,
        follow_evolutions: bool,
    ) -> Result<(), RomError> {
        let learnsets = self.learnsets()?;
        for (number, learnset) in learnsets.iter().enumerate().skip(1) {
            let stats = &mut self.stats[Self::stats_file(number)];
            for &(learnt, _) in learnset {
                // The first of two machines with the same move.
                if let Some(at) = moves.iter().position(|&m| m == learnt) {
                    let bit = place(at);
                    stats[bit / 8] |= 1 << (bit % 8);
                }
            }
        }
        if follow_evolutions {
            for step in self.family_steps(true) {
                if let traits::Step::Evolved { from, to, .. } = step {
                    for bit in (0..handed_on).map(place) {
                        let learns = self.stats[Self::stats_file(from)][bit / 8] & 1 << (bit % 8);
                        self.stats[Self::stats_file(to)][bit / 8] |= learns;
                    }
                }
            }
        }
        Ok(())
    }

    /// What a species learns by itself, it can learn from a TM too. The
    /// original does not look at the HMs here, but hands them on.
    pub fn keep_learnt_machines(&mut self, follow_evolutions: bool) -> Result<(), RomError> {
        let (tms, hms) = self.machine_moves().ok_or(layout("moves of the TMs"))?;
        let place = |at: usize| MACHINES * 8 + at;
        let all = tms.len() + hms.len();
        self.keep_learnt(&tms, all, &place, follow_evolutions)?;
        self.copy_to_looks_only(MACHINES..MACHINES + MACHINES_LEN);
        Ok(())
    }

    /// What a species learns by itself, a move tutor can teach it too.
    pub fn keep_learnt_tutors(&mut self, follow_evolutions: bool) -> Result<(), RomError> {
        let moves = self.tutor_moves()?;
        let place = |at: usize| {
            let block = TUTOR_BLOCKS
                .iter()
                .position(|&(count, first)| (first..first + count).contains(&at));
            let block = block.unwrap_or(0);
            (TUTOR_FLAGS + block * 4) * 8 + at - TUTOR_BLOCKS[block].1
        };
        self.keep_learnt(&moves, moves.len(), &place, follow_evolutions)?;
        self.copy_to_looks_only(TUTOR_FLAGS..TUTOR_FLAGS + 16);
        Ok(())
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

    /// Whether an evolution learns each of `moves`: all that the species
    /// before it does, as `before` says, and now and then one more, most
    /// of all with `prefer_type` for a type it has newly got.
    fn learns_more(
        &mut self,
        (from, to): (usize, usize),
        moves: &[u16],
        before: &[bool],
        prefer_type: bool,
    ) -> Vec<bool> {
        let types = |game: &Self, number: usize| {
            let stats = &game.stats[Self::stats_file(number)];
            [stats[FIRST_TYPE], stats[FIRST_TYPE + 1]]
        };
        let (old, new) = (types(self, from), types(self, to));
        (moves.iter().zip(before))
            .map(|(&m, &before)| {
                if before {
                    return true;
                }
                let kind = self.moves.get(usize::from(m)).map_or(NORMAL, |m| m[0]);
                let chance = match prefer_type {
                    true if new.contains(&kind) && !old.contains(&kind) => 0.9,
                    true => 0.1,
                    false => 0.25,
                };
                self.random.double() < chance
            })
            .collect()
    }

    /// Who learns which of `moves`, decided anew as `how` says, for
    /// every species and form by its number. With `follow_evolutions`
    /// an evolution learns what the species before it does.
    fn new_learners(
        &mut self,
        moves: &[u16],
        how: Compat,
        follow_evolutions: bool,
    ) -> Vec<Vec<bool>> {
        let mut all = vec![Vec::new(); SPECIES + FORMS + 1];
        if follow_evolutions && how != Compat::Full {
            for step in self.family_steps(true) {
                match step {
                    traits::Step::First(number) => all[number] = self.learns(number, moves, how),
                    traits::Step::Evolved { from, to, .. } => {
                        let before = all[from].clone();
                        let prefer_type = how == Compat::RandomPreferType;
                        all[to] = self.learns_more((from, to), moves, &before, prefer_type);
                    }
                }
            }
        } else {
            for (number, learns) in all.iter_mut().enumerate().skip(1) {
                *learns = self.learns(number, moves, how);
            }
        }
        all
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
    pub fn change_machine_compatibility(
        &mut self,
        how: Compat,
        follow_evolutions: bool,
    ) -> Result<(), RomError> {
        let (mut moves, hms) = self.machine_moves().ok_or(layout("moves of the TMs"))?;
        moves.extend(hms);
        let all = self.new_learners(&moves, how, follow_evolutions);
        for (number, learns) in all.iter().enumerate().skip(1) {
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
    pub fn change_tutor_compatibility(
        &mut self,
        how: Compat,
        follow_evolutions: bool,
    ) -> Result<(), RomError> {
        let moves = self.tutor_moves()?;
        let all = self.new_learners(&moves, how, follow_evolutions);
        for (number, learns) in all.iter().enumerate().skip(1) {
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
