//! Wild Pokémon: every area gets other species, each old species of an
//! area one new one, and the Pokédex learns where they live now.

use super::tables::LEGENDARIES;
use super::*;
use std::collections::BTreeMap;

const WILD: &str = "a/1/2/7";
const DEX_AREAS: &str = "a/1/7/6";
/// A file of wild Pokémon has one part of this length, or four of them
/// for the seasons. A part starts with the rates of the seven kinds of
/// encounter (grass, dark grass, shaking grass, surfing, rippling water,
/// fishing, rippling fishing) and holds this many slots for each.
const SEASON: usize = 232;
const SLOTS: [usize; 7] = [12, 12, 12, 5, 5, 5, 5];
const FIRST_SLOT: usize = 8;
/// A species' file of the Pokédex: for each season one byte per area
/// with the kinds of encounter there, after a byte that says "nowhere".
const DEX_AREA_COUNT: usize = 85;
const DEX_AREA_LEN: usize = 345;
/// The area of the Pokédex each file of wild Pokémon belongs to.
const AREA_OF_FILE: [i8; 135] = [
    2, 4, 8, 59, 61, 63, 19, 19, 20, 20, 21, 21, 22, 22, 22, 22, 22, 22, 22, 22, 24, 24, 24, 25,
    25, 25, 25, 26, 26, 26, 26, 76, 27, 27, 27, 27, 27, 70, 70, 70, 70, 70, 29, 35, 71, 71, 72, 72,
    73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 73, 74, 74, 74, 74, 74, 74, 74, 74, 74, 74, 76,
    76, 76, 76, 76, 76, 76, 76, 76, 76, 77, 77, 77, 79, 79, 79, 79, 79, 79, 79, 79, 79, 78, 78, -1,
    55, 57, 58, 37, 38, 39, 30, 30, 40, 40, 41, 42, 31, 31, 31, 43, 32, 32, 32, 32, 44, 33, 45, 46,
    47, 48, 49, 34, 50, 51, 36, 53, 66, 67, 69, 75, 12, 52, 68,
];
/// Files that only one of the two games uses, by area: Route 4, rooms of
/// Victory Road and Reversal Mountain.
const ROUTE_4: (i8, usize, usize) = (40, 104, 105);
const VICTORY_ROAD: (i8, [usize; 2], [usize; 2]) = (76, [71, 73], [78, 79]);
const REVERSAL_MOUNTAIN: (i8, (usize, usize), (usize, usize)) = (73, (49, 54), (55, 60));
/// Forms a player could not keep: Shaymin's and Giratina's other form.
/// Those that only exist through an ability, Castform's and Darmanitan's,
/// are left out too unless abilities are random.
const NOT_WILD: [usize; 2] = [655, 656];
const BY_ABILITY: [usize; 4] = [662, 663, 664, 666];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    /// The species or form, as a number.
    pokemon: usize,
    form: u16,
    /// The lowest and the highest level.
    levels: [u8; 2],
}

/// One kind of encounter in one place: where its slots stand in which
/// file, and what is in them.
struct Area {
    file: usize,
    at: usize,
    shaking: bool,
    slots: Vec<Slot>,
}

/// The Pokémon to choose from: all of them, and those that are not
/// legendary.
struct Pools {
    all: Vec<usize>,
    ordinary: Vec<usize>,
}

/// How far the new wild Pokémon reach.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Wild {
    /// Each species of an area becomes one other.
    #[default]
    Areas,
    /// Every slot of every area on its own.
    Slots,
    /// Each species becomes one other, all over the game.
    Everywhere,
}

/// What the new wild Pokémon have to be like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WildRule {
    #[default]
    None,
    /// About as strong as the old ones.
    SimilarStrength,
    /// Every species comes up before one comes up twice.
    CatchThemAll,
    /// One type for each area.
    TypeThemes,
}

/// How wild Pokémon are made random; the levels in it change without
/// random ones too.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WildRules {
    pub mode: Wild,
    pub rule: WildRule,
    /// The seasons keep wild Pokémon of their own.
    pub seasons: bool,
    pub no_legendaries: bool,
    /// In shaking grass a Pokémon is no stronger than suits its level.
    pub balance_shaking_grass: bool,
    /// Per cent by which the levels rise or fall.
    pub levels: i8,
}

const SHAKING_GRASS: usize = 2;
const CATCH_RATE: usize = 8;
const BATTLE_OVERLAY: usize = 167;
/// Stands right behind the branch the game takes when a throw can fail.
const SURE_CATCH_MARK: [u8; 10] = [0x08, 0xDB, 0x00, 0x28, 0x01, 0xD0, 0x01, 0x20, 0x00, 0xE0];

/// The levels of all wild Pokémon rise or fall by `percent`.
fn change_wild_levels(areas: &mut [Area], percent: i8) {
    if percent != 0 {
        for level in areas
            .iter_mut()
            .flat_map(|a| &mut a.slots)
            .flat_map(|s| &mut s.levels)
        {
            let changed = f64::from(*level) * (1.0 + f64::from(percent) / 100.0);
            *level = traits::round(changed).min(100.0) as u8;
        }
    }
}

/// The kinds of encounter of one part of a file that take place at all,
/// each with where its slots start and how many there are.
fn kinds(part: &[u8]) -> Vec<(usize, usize, usize)> {
    let mut at = FIRST_SLOT;
    let mut found = Vec::new();
    for (kind, &slots) in SLOTS.iter().enumerate() {
        if part[kind] != 0 {
            found.push((kind, at, slots));
        }
        at += slots * 4;
    }
    found
}

impl<'a> Game<'a> {
    /// The file of the Pokédex for each species, from the wild Pokémon
    /// as they are in `files`.
    fn dex_areas(&self, files: &[Vec<u8>]) -> Vec<Vec<u8>> {
        let white = self.code[2] == b'D';
        let mut dex = vec![vec![0u8; DEX_AREA_LEN]; SPECIES];
        for (number, file) in files.iter().enumerate() {
            let Some(&area) = AREA_OF_FILE.get(number) else {
                continue;
            };
            let other_game = if area == ROUTE_4.0 {
                number == if white { ROUTE_4.1 } else { ROUTE_4.2 }
            } else if area == VICTORY_ROAD.0 {
                let rooms = if white {
                    VICTORY_ROAD.1
                } else {
                    VICTORY_ROAD.2
                };
                rooms.contains(&number)
            } else if area == REVERSAL_MOUNTAIN.0 {
                let (from, to) = if white {
                    REVERSAL_MOUNTAIN.1
                } else {
                    REVERSAL_MOUNTAIN.2
                };
                (from..=to).contains(&number)
            } else {
                false
            };
            if area < 0 || other_game {
                continue;
            }
            for season in 0..4 {
                let start = if file.len() > SEASON {
                    season * SEASON
                } else {
                    0
                };
                let part = &file[start..];
                for (kind, at, slots) in kinds(part) {
                    for slot in 0..slots {
                        let species = usize::from(word(part, at + slot * 4) & 0x3FF);
                        if let Some(entry) = species.checked_sub(1).and_then(|n| dex.get_mut(n)) {
                            entry[season * (DEX_AREA_COUNT + 1) + 2 + area as usize] |= 1 << kind;
                        }
                    }
                }
            }
        }
        for entry in &mut dex {
            let areas = |season: usize| {
                let from = season * (DEX_AREA_COUNT + 1) + 2;
                from..from + DEX_AREA_COUNT
            };
            for season in 0..4 {
                if entry[areas(season)].iter().all(|&b| b == 0) {
                    entry[season * (DEX_AREA_COUNT + 1) + 1] = 1;
                }
            }
            let spring = entry[areas(0)].to_vec();
            if (1..4).all(|season| entry[areas(season)] == spring[..]) {
                entry[0] = 1;
            }
        }
        dex
    }

    /// The wild Pokémon of all areas as they are in `files`: for each
    /// kind of encounter of a file one area, with `seasons` one for
    /// each season too.
    fn wild_areas(&self, files: &[Vec<u8>], seasons: bool) -> Result<Vec<Area>, RomError> {
        let mut areas = Vec::new();
        for (file, data) in files.iter().enumerate() {
            if data.len() < SEASON {
                return Err(layout(WILD));
            }
            let parts = if seasons && data.len() >= SEASON * 4 {
                4
            } else {
                1
            };
            for start in (0..parts).map(|part| part * SEASON) {
                for (kind, at, count) in kinds(&data[start..]) {
                    let mut slots = Vec::with_capacity(count);
                    for slot in (0..count).map(|slot| start + at + slot * 4) {
                        let both = word(data, slot);
                        let (species, form) = (usize::from(both & 0x7FF), both >> 11);
                        if !(1..=SPECIES).contains(&species) {
                            return Err(layout(WILD));
                        }
                        let looks = u16::from(self.species[species].looks);
                        let pokemon = if form <= looks || form >= 30 {
                            species
                        } else {
                            self.form_of(species, form)
                        };
                        slots.push(Slot {
                            pokemon,
                            form,
                            levels: [data[slot + 2], data[slot + 3]],
                        });
                    }
                    areas.push(Area {
                        file,
                        at: start + at,
                        shaking: kind == SHAKING_GRASS,
                        slots,
                    });
                }
            }
        }
        Ok(areas)
    }

    /// Writes the wild Pokémon and tells the Pokédex where they live.
    /// Without `seasons`, every season is like spring.
    fn set_wild_areas(
        &mut self,
        mut files: Vec<Vec<u8>>,
        areas: &[Area],
        seasons: bool,
    ) -> Result<(), RomError> {
        for area in areas {
            for (i, slot) in area.slots.iter().enumerate() {
                let at = area.at + i * 4;
                // A form that was left alone is written as its species.
                let species = match self.species[slot.pokemon].base {
                    0 => slot.pokemon,
                    base => base,
                };
                set_word(
                    &mut files[area.file],
                    at,
                    (slot.form << 11) + species as u16,
                );
                files[area.file][at + 2..at + 4].copy_from_slice(&slot.levels);
            }
        }
        for file in files.iter_mut().filter(|_| !seasons) {
            if file.len() >= SEASON * 4 {
                let (spring, rest) = file.split_at_mut(SEASON);
                for season in rest.chunks_exact_mut(SEASON).take(3) {
                    season.copy_from_slice(spring);
                }
            }
        }
        self.put_narc(WILD, &files)?;

        let mut dex = Self::narc(&self.nds, DEX_AREAS)?;
        if dex.len() < SPECIES {
            return Err(layout(DEX_AREAS));
        }
        for (old, new) in dex.iter_mut().zip(self.dex_areas(&files)) {
            *old = new;
        }
        self.put_narc(DEX_AREAS, &dex)
    }

    /// Puts the Pokémon `picked` into a slot: a form as its species with
    /// the form beside it, a species whose forms only look different in
    /// one of them at random.
    fn fill_slot(&mut self, slot: &mut Slot, picked: usize) {
        let entry = &self.species[picked];
        if entry.form > 0 {
            slot.pokemon = entry.base;
            slot.form = entry.form;
        } else {
            slot.pokemon = picked;
            slot.form = match entry.looks {
                0 => 0,
                looks => self.random.index(usize::from(looks)) as u16,
            };
        }
    }

    /// One of `pool` that is about as strong as `like`, which counts as
    /// no stronger than a Pokémon of level `level` should be. Not
    /// `like` itself with `other`, and none of `used`.
    fn similar_wild(
        &mut self,
        pool: &[usize],
        like: usize,
        other: bool,
        used: &[usize],
        level: i32,
    ) -> usize {
        let strength = self.strength(like).min(level * 10 + 250);
        let (mut least, mut most) = (strength - strength / 10, strength + strength / 10);
        let mut fit: Vec<usize> = Vec::new();
        let mut rounds = 0;
        while fit.is_empty() || (fit.len() < 3 && rounds < 3) {
            for &pokemon in pool {
                if (least..=most).contains(&self.strength(pokemon))
                    && !(other && pokemon == like)
                    && !used.contains(&pokemon)
                    && !fit.contains(&pokemon)
                {
                    fit.push(pokemon);
                }
            }
            least -= strength / 20;
            most += strength / 20;
            rounds += 1;
        }
        fit[self.random.index(fit.len())]
    }

    /// Any Pokémon of `pools` that may live in the wild. The original
    /// draws from both pools and uses one of the draws.
    fn any_wild(&mut self, pools: &Pools, banned: &[usize], no_legendaries: bool) -> usize {
        loop {
            let ordinary = pools.ordinary[self.random.index(pools.ordinary.len())];
            let any = pools.all[self.random.index(pools.all.len())];
            let picked = if no_legendaries { ordinary } else { any };
            if !self.species[picked].looks_only && !banned.contains(&picked) {
                return picked;
            }
        }
    }

    /// The areas get other wild Pokémon, as `how` says, with `forms`
    /// also other forms.
    pub fn randomize_wild(&mut self, forms: bool, how: &WildRules) -> Result<(), RomError> {
        let files = Self::narc(&self.nds, WILD)?;
        let legendary = |n: usize| {
            let species = match self.species[n].base {
                0 => n,
                base => base,
            };
            LEGENDARIES.contains(&(species as u16))
        };
        let all = if forms { &self.pool_forms } else { &self.pool };
        let pools = Pools {
            all: all.clone(),
            ordinary: all.iter().copied().filter(|&n| !legendary(n)).collect(),
        };
        let mut banned = NOT_WILD.to_vec();
        if !self.abilities_changed {
            banned.extend(BY_ABILITY);
        }
        // The original only bans those of them it may choose at all.
        banned.retain(|n| self.pool_forms.contains(n));
        if self.no_irregular_forms {
            banned.extend(IRREGULAR_FORMS);
        }
        // All that can be chosen where the choice is not just any.
        let wanted = if how.no_legendaries {
            &pools.ordinary
        } else {
            &pools.all
        };
        let usable: Vec<usize> = (wanted.iter().copied())
            .filter(|n| !banned.contains(n))
            .collect();
        let real: Vec<usize> = (usable.iter().copied())
            .filter(|&n| !self.species[n].looks_only)
            .collect();
        let of_type = |game: &Self, kind: u8| -> Vec<usize> {
            let has = |&n: &usize| {
                let stats = &game.stats[Self::stats_file(n)];
                stats[FIRST_TYPE] == kind || stats[FIRST_TYPE + 1] == kind
            };
            real.iter().copied().filter(has).collect()
        };

        let mut areas;
        if how.mode == Wild::Everywhere {
            // One new Pokémon for each old one, all over the game.
            let mut left: Vec<usize> = (1..=SPECIES + FORMS)
                .filter(|&n| !self.species[n].looks_only && !banned.contains(&n))
                .collect();
            let mut right = real.clone();
            let mut becomes: Vec<usize> = (0..=SPECIES + FORMS).collect();
            while !left.is_empty() {
                let old = left.remove(self.random.index(left.len()));
                let new = if how.rule == WildRule::SimilarStrength {
                    let new = match right[..] {
                        [only] => only,
                        _ => self.similar_wild(&right, old, true, &[], 100),
                    };
                    right.retain(|&n| n != new);
                    new
                } else {
                    let mut at = self.random.index(right.len());
                    // Drawn before the old one in the original.
                    while right[at] == old && right.len() != 1 {
                        at = self.random.index(right.len());
                    }
                    right.remove(at)
                };
                becomes[old] = new;
                if right.is_empty() {
                    right = real.clone();
                }
            }
            areas = self.wild_areas(&files, how.seasons)?;
            for slot in areas.iter_mut().flat_map(|area| &mut area.slots) {
                let new = becomes[slot.pokemon];
                self.fill_slot(slot, new);
            }
        } else {
            areas = self.wild_areas(&files, how.seasons)?;
            let each = how.mode == Wild::Slots;
            let mut order: Vec<usize> = (0..areas.len()).collect();
            self.random.shuffle(&mut order);
            let mut left = real.clone();
            let mut typed: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
            for area in order {
                let shaking = areas[area].shaking;
                // What to replace: every slot, or every species once.
                let mut old: Vec<usize> = areas[area].slots.iter().map(|s| s.pokemon).collect();
                if !each {
                    old.sort_unstable();
                    old.dedup();
                }
                // In an area of slots each is filled at once; else the
                // new species are chosen first, one for each old one.
                let mut new: Vec<Option<usize>> = Vec::with_capacity(old.len());
                let put = |game: &mut Self, areas: &mut [Area], slot: usize, picked: usize| {
                    let mut filled = areas[area].slots[slot];
                    game.fill_slot(&mut filled, picked);
                    areas[area].slots[slot] = filled;
                };
                let mut choice = Vec::new();
                if how.rule == WildRule::TypeThemes {
                    let least = if each { 1 } else { old.len() };
                    for _ in 0..10000 {
                        let theme = self.random_type();
                        let list = typed.entry(theme).or_insert_with(|| of_type(self, theme));
                        if list.len() >= least {
                            choice = list.clone();
                            break;
                        }
                    }
                    if choice.is_empty() {
                        return Err(layout("wild Pokémon of one type"));
                    }
                }
                let mut used = Vec::new();
                for (slot, &was) in old.iter().enumerate() {
                    let picked = match how.rule {
                        WildRule::CatchThemAll if each && banned.contains(&was) => None,
                        WildRule::CatchThemAll => {
                            let picked = left.remove(self.random.index(left.len()));
                            if left.is_empty() {
                                left = real.clone();
                            }
                            (!banned.contains(&was)).then_some(picked)
                        }
                        WildRule::TypeThemes => {
                            let at = self.random.index(choice.len());
                            Some(if each { choice[at] } else { choice.remove(at) })
                        }
                        WildRule::SimilarStrength => {
                            let levels = areas[area].slots[slot].levels.map(i32::from);
                            let level = if each && how.balance_shaking_grass && shaking {
                                (levels[0] + levels[1]) / 2
                            } else {
                                100
                            };
                            let mut picked = self.similar_wild(&usable, was, false, &used, level);
                            while self.species[picked].looks_only {
                                // Slot by slot, the original's next try
                                // is as strong as this one.
                                let like = if each { picked } else { was };
                                picked = self.similar_wild(&usable, like, false, &used, level);
                            }
                            if !each {
                                used.push(picked);
                            }
                            Some(picked)
                        }
                        WildRule::None => {
                            let mut picked = self.any_wild(&pools, &banned, how.no_legendaries);
                            while !each && new.contains(&Some(picked)) {
                                picked = self.any_wild(&pools, &banned, how.no_legendaries);
                            }
                            Some(picked)
                        }
                    };
                    match picked {
                        Some(picked) if each => put(self, &mut areas, slot, picked),
                        _ => new.push(picked),
                    }
                }
                if !each {
                    for slot in 0..areas[area].slots.len() {
                        let was = areas[area].slots[slot].pokemon;
                        let at = old.binary_search(&was).map_err(|_| layout(WILD))?;
                        if let Some(picked) = new[at] {
                            put(self, &mut areas, slot, picked);
                        }
                    }
                }
            }
        }
        change_wild_levels(&mut areas, how.levels);
        self.set_wild_areas(files, &areas, how.seasons)
    }

    /// Pokémon get easier to catch, in five steps: no catch rate under
    /// 75, 128, 200 or 255, for legendary ones half of that until the
    /// fourth step, and with the fifth every throw catches.
    pub fn raise_catch_rates(&mut self, step: u8) -> Result<(), RomError> {
        if step >= 5 {
            let mut code = (self.nds.overlay(BATTLE_OVERLAY)).ok_or(layout("overlay"))?;
            // As in the original, a game without the mark stays as it is.
            if let Some(at) = find(&code, &SURE_CATCH_MARK) {
                code[at..at + 2].fill(0);
                self.nds.set_overlay(BATTLE_OVERLAY, &code);
            }
            return Ok(());
        }
        let (ordinary, legendary) = match step {
            2 => (128, 64),
            3 => (200, 100),
            4 => (255, 255),
            _ => (75, 37),
        };
        for number in 1..=SPECIES + FORMS {
            let species = match self.species[number].base {
                0 => number,
                base => base,
            };
            let least = if LEGENDARIES.contains(&(species as u16)) {
                legendary
            } else {
                ordinary
            };
            let rate = &mut self.stats[Self::stats_file(number)][CATCH_RATE];
            *rate = (*rate).max(least);
        }
        Ok(())
    }

    /// Only the levels of the wild Pokémon change.
    pub fn change_wild_levels(&mut self, percent: i8) -> Result<(), RomError> {
        let files = Self::narc(&self.nds, WILD)?;
        let mut areas = self.wild_areas(&files, true)?;
        change_wild_levels(&mut areas, percent);
        self.set_wild_areas(files, &areas, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_kinds_with_a_rate_take_place() {
        let mut part = vec![0u8; SEASON];
        part[0] = 30;
        part[3] = 5;
        part[6] = 1;
        assert_eq!(kinds(&part), [(0, 8, 12), (3, 152, 5), (6, 212, 5)]);
        assert_eq!(FIRST_SLOT + SLOTS.iter().sum::<usize>() * 4, SEASON);
    }

    #[test]
    fn the_pokedex_has_room_for_every_area() {
        assert_eq!(4 * (DEX_AREA_COUNT + 1) + 1, DEX_AREA_LEN);
        assert!(AREA_OF_FILE
            .iter()
            .all(|&a| (a as isize) < DEX_AREA_COUNT as isize));
    }
}
