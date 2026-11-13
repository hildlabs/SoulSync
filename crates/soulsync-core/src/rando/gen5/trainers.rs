//! The teams of the trainers: read and written back the way the original
//! randomizer does, and the steps that change them.

use super::tables::{LEGENDARIES, STORY_TRAINERS, THEME_GROUPS};
use super::traits::WONDER_GUARD;
use super::*;
use std::collections::BTreeMap;

const TRAINERS: &str = "a/0/9/1";
const TEAMS: &str = "a/0/9/2";
const LEARNSETS: &str = "a/0/1/8";
/// The teams of the tournament in Driftveil City: 17 trainers, the first
/// two with three Pokémon and the others with six, all at level 25.
const TOURNAMENT: &str = "a/2/5/0";
const TOURNAMENT_TRAINERS: usize = 17;
const TOURNAMENT_LEVEL: u16 = 25;
/// Bits of a trainer's first byte: its Pokémon have moves or items of
/// their own.
const OWN_MOVES: u8 = 1;
const OWN_ITEMS: u8 = 2;
/// The battles against the rival and, as "friend", the two against the
/// other hero: the number of the battle and the first of its three
/// trainers, one for each starter the player may have taken.
const RIVAL_BATTLES: [(u8, usize); 10] = [
    (1, 0xA1),
    (2, 0xA6),
    (3, 0x24C),
    (4, 0x170),
    (5, 0x17A),
    (6, 0x2BD),
    (7, 0x31A),
    (8, 0x2AC),
    (9, 0x2B5),
    (10, 0x2B8),
];
const FRIEND_BATTLES: [(u8, usize); 2] = [(1, 0x168), (1, 0x16B)];
/// Forms that only exist through an ability: Castform's and Darmanitan's.
const CASTFORM: usize = 351;
const DARMANITAN: usize = 555;
/// How many ways a species may evolve at most, for the number that
/// decides between them.
const MOST_SPLIT_EVOLUTIONS: usize = 8;

/// The texts with the names of the trainers, of the trainers of the
/// tournaments and of the faces shown before big battles.
const NAMES: usize = 382;
const TOURNAMENT_NAMES: usize = 409;
const FACE_NAMES: usize = 368;
const LONGEST_NAME: usize = 10;
/// Names many trainers share, who then still get a name each.
const SHARED_NAMES: [&str; 6] = ["GRUNT", "EXECUTIVE", "SHADOW", "ADMIN", "GOON", "EMPLOYEE"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerPokemon {
    pub ivs: u32,
    pub ability: u8,
    gender: u8,
    pub level: u16,
    /// The species, with `form` beside it.
    pub species: usize,
    pub form: u16,
    pub item: u16,
    pub moves: [u16; 4],
    /// The moves are to be the ones it knows at its level.
    pub(super) reset_moves: bool,
}

/// How often the same Pokémon may come up among the trainers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Spread {
    /// As chance has it.
    #[default]
    Free,
    /// All about equally often.
    Even,
    /// About equally often among the trainers of the story.
    EvenInStory,
}

/// Forms that turn back by themselves.
const NOT_KEPT: [usize; 2] = [655, 656];
/// The four of the elite and the champion.
const ELITE: [usize; 5] = [38, 39, 40, 41, 341];
/// The types of the game in the order the original randomizer lists them.
const TYPES_IN_ORDER: [u8; 17] = [0, 1, 2, 11, 10, 9, 5, 4, 13, 6, 15, 12, 7, 3, 14, 8, 16];

/// Which trainers have Pokémon of one type each.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Themes {
    #[default]
    None,
    All,
    /// Only the gyms and the elite.
    GymsAndElite,
}

/// What the random Pokémon of the trainers have to be like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrainerRules {
    pub spread: Spread,
    pub themes: Themes,
    /// Themed trainers get a type as often as Pokémon have it.
    pub type_weights: bool,
    /// About as strong as the ones they replace.
    pub similar_strength: bool,
    pub no_legendaries: bool,
    /// No Pokémon with Wonder Guard below level 20.
    pub no_early_wonder_guard: bool,
    /// The levels rise or fall by so many percent.
    pub levels: i8,
    /// So many of the strongest Pokémon of each of the elite nobody
    /// else has.
    pub elite_unique: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trainer {
    /// The trainer's number; 0 for the trainers of the tournament.
    pub index: usize,
    kind: u8,
    pub team: Vec<TrainerPokemon>,
}

/// A battle with the rival: which one, and for which starter.
type Tag = (u8, usize);

fn tagged(battles: &[(u8, usize)], trainer: &Trainer) -> Option<Tag> {
    let own = |&&(_, first): &&(u8, usize)| (first..first + 3).contains(&trainer.index);
    let &(battle, first) = battles.iter().find(own)?;
    Some((battle, trainer.index - first))
}

impl<'a> Game<'a> {
    /// What each species and form learns at which level.
    fn learnsets(&self) -> Result<Vec<Vec<(u16, u16)>>, RomError> {
        let files = Self::narc(&self.nds, LEARNSETS)?;
        let mut all = vec![Vec::new()];
        for number in 1..=SPECIES + FORMS {
            let data = files
                .get(Self::stats_file(number))
                .ok_or(layout(LEARNSETS))?;
            let entries = data.chunks_exact(4).map(|e| (word(e, 0), word(e, 2)));
            all.push(entries.take_while(|&e| e != (0xFFFF, 0xFFFF)).collect());
        }
        Ok(all)
    }

    /// The number of a form of a species, the species itself if it has no
    /// such form with stats of its own.
    pub(super) fn form_of(&self, species: usize, form: u16) -> usize {
        let forms = SPECIES + 1..=SPECIES + FORMS;
        let own = |&n: &usize| self.species[n].base == species && self.species[n].form == form;
        forms.into_iter().find(own).unwrap_or(species)
    }

    pub fn trainers(&self) -> Result<Vec<Trainer>, RomError> {
        let trainers = Self::narc(&self.nds, TRAINERS)?;
        let teams = Self::narc(&self.nds, TEAMS)?;
        let mut all = Vec::with_capacity(trainers.len() + TOURNAMENT_TRAINERS);
        for (index, (trainer, team)) in trainers.iter().zip(&teams).enumerate().skip(1) {
            let kind = *trainer.first().ok_or(layout(TRAINERS))?;
            let count = usize::from(*trainer.get(3).ok_or(layout(TRAINERS))?);
            let each = 8
                + if kind & OWN_ITEMS != 0 { 2 } else { 0 }
                + if kind & OWN_MOVES != 0 { 8 } else { 0 };
            if team.len() < count * each {
                return Err(layout(TEAMS));
            }
            let mut list = Vec::with_capacity(count);
            for data in team.chunks_exact(each).take(count) {
                let mut at = 8;
                let mut pokemon = TrainerPokemon {
                    ivs: u32::from(data[0]) * 31 / 255,
                    ability: data[1] >> 4,
                    gender: data[1] & 15,
                    level: word(data, 2),
                    species: usize::from(word(data, 4)),
                    form: word(data, 6),
                    item: 0,
                    moves: [0; 4],
                    reset_moves: false,
                };
                if kind & OWN_ITEMS != 0 {
                    pokemon.item = word(data, at);
                    at += 2;
                }
                if kind & OWN_MOVES != 0 {
                    for (i, slot) in pokemon.moves.iter_mut().enumerate() {
                        *slot = word(data, at + i * 2);
                    }
                }
                if pokemon.species > SPECIES {
                    return Err(layout(TEAMS));
                }
                list.push(pokemon);
            }
            all.push(Trainer {
                index,
                kind,
                team: list,
            });
        }
        let tournament = Self::narc(&self.nds, TOURNAMENT)?;
        let mut files = tournament.iter().skip(1);
        for trainer in 0..TOURNAMENT_TRAINERS {
            let mut list = Vec::new();
            for _ in 0..if trainer < 2 { 3 } else { 6 } {
                let data = files
                    .next()
                    .filter(|d| d.len() >= 14)
                    .ok_or(layout(TOURNAMENT))?;
                let species = usize::from(word(data, 0));
                if species > SPECIES {
                    return Err(layout(TOURNAMENT));
                }
                list.push(TrainerPokemon {
                    ivs: 31,
                    ability: 0,
                    gender: 0,
                    level: TOURNAMENT_LEVEL,
                    species,
                    form: 0,
                    item: word(data, 12),
                    moves: [word(data, 2), word(data, 4), word(data, 6), word(data, 8)],
                    reset_moves: false,
                });
            }
            all.push(Trainer {
                index: 0,
                kind: OWN_MOVES | OWN_ITEMS,
                team: list,
            });
        }
        Ok(all)
    }

    /// The moves a Pokémon knows at `level` if it always took the newest.
    fn moves_at(learnset: &[(u16, u16)], level: u16) -> [u16; 4] {
        let mut moves = [0; 4];
        let mut count = 0;
        for &(learnt, _) in learnset.iter().take_while(|&&(_, at)| at <= level) {
            if moves[..count].contains(&learnt) {
                continue;
            }
            if count == 4 {
                moves.rotate_left(1);
                moves[3] = learnt;
            } else {
                moves[count] = learnt;
                count += 1;
            }
        }
        moves
    }

    pub fn set_trainers(&mut self, all: &[Trainer]) -> Result<(), RomError> {
        let mut trainers = Self::narc(&self.nds, TRAINERS)?;
        let learnsets = self.learnsets()?;
        let mut teams = vec![vec![0; 8]];
        let mut list = all.iter();
        for data in trainers.iter_mut().skip(1) {
            let trainer = list.next().ok_or(layout(TRAINERS))?;
            data[0] = trainer.kind;
            data[3] = trainer.team.len() as u8;
            let mut team = Vec::new();
            for pokemon in &trainer.team {
                let strength = (1 + pokemon.ivs * 255 / 31).min(255) as u8;
                team.extend([strength, pokemon.ability << 4 | pokemon.gender]);
                team.extend(pokemon.level.to_le_bytes());
                team.extend((pokemon.species as u16).to_le_bytes());
                team.extend(pokemon.form.to_le_bytes());
                if trainer.kind & OWN_ITEMS != 0 {
                    team.extend(pokemon.item.to_le_bytes());
                }
                if trainer.kind & OWN_MOVES != 0 {
                    let moves = if pokemon.reset_moves {
                        let number = self.form_of(pokemon.species, pokemon.form);
                        Self::moves_at(&learnsets[number], pokemon.level)
                    } else {
                        pokemon.moves
                    };
                    team.extend(moves.iter().flat_map(|m| m.to_le_bytes()));
                }
            }
            teams.push(team);
        }
        self.put_narc(TRAINERS, &trainers)?;
        self.put_narc(TEAMS, &teams)?;

        let mut tournament = Self::narc(&self.nds, TOURNAMENT)?;
        let mut files = tournament.iter_mut().skip(1);
        for _ in 0..TOURNAMENT_TRAINERS {
            let trainer = list.next().ok_or(layout(TOURNAMENT))?;
            for pokemon in &trainer.team {
                let data = files.next().ok_or(layout(TOURNAMENT))?;
                set_word(data, 0, pokemon.species as u16);
                set_word(data, 12, pokemon.item);
                let moves = if pokemon.reset_moves {
                    Self::moves_at(&learnsets[pokemon.species], pokemon.level)
                } else {
                    pokemon.moves
                };
                for (i, chosen) in moves.iter().enumerate() {
                    set_word(data, 2 + i * 2, *chosen);
                }
            }
        }
        self.put_narc(TOURNAMENT, &tournament)
    }

    /// Makes `pokemon` the species or form with number `chosen`; a species
    /// whose forms only look different gets one of them at random.
    fn become_form(&mut self, pokemon: &mut TrainerPokemon, chosen: usize) {
        let entry = &self.species[chosen];
        pokemon.species = chosen;
        pokemon.form = 0;
        if entry.form > 0 {
            pokemon.form = entry.form;
            pokemon.species = entry.base;
        } else if entry.looks > 0 {
            pokemon.form = self.random.index(usize::from(entry.looks)) as u16;
        }
    }

    /// One of the three ability slots of a species, an empty one never.
    fn random_ability(&mut self, number: usize) -> u8 {
        loop {
            let slot = self.random.index(3);
            if self.species[number].abilities[slot] != 0 {
                return slot as u8 + 1;
            }
        }
    }

    /// The levels of a trainer's Pokémon rise or fall by `percent`.
    fn change_levels(trainer: &mut Trainer, percent: i8) {
        if percent != 0 {
            for pokemon in &mut trainer.team {
                let level = f64::from(pokemon.level) * (1.0 + f64::from(percent) / 100.0);
                pokemon.level = (traits::round(level) as u16).min(100);
            }
        }
    }

    /// Only the levels of the trainers' Pokémon change.
    pub fn change_trainer_levels(&mut self, percent: i8) -> Result<(), RomError> {
        let mut all = self.trainers()?;
        for trainer in &mut all {
            Self::change_levels(trainer, percent);
        }
        self.set_trainers(&all)
    }

    /// A random Pokémon of `pool` in place of `current`. `used` counts
    /// how often each was chosen, for those who shall come up alike.
    fn trainer_pokemon(
        &mut self,
        pool: &[usize],
        current: usize,
        how: &TrainerRules,
        wonder_guard: bool,
        used: Option<&[u32]>,
    ) -> usize {
        // How often a chosen one was chosen on average.
        let average = |used: &[u32]| {
            let chosen = used.iter().filter(|&&times| times > 0).count();
            if chosen == 0 {
                0.0
            } else {
                f64::from(used.iter().sum::<u32>()) / chosen as f64
            }
        };
        let rare: Vec<usize>;
        let mut pool = pool;
        if let Some(used) = used {
            let limit = average(used) * 2.0;
            rare = (pool.iter().copied())
                .filter(|&n| f64::from(used[n]) < limit)
                .collect();
            if !rare.is_empty() {
                pool = &rare;
            }
        }
        let guarded = |game: &Self, n: usize| game.species[n].abilities.contains(&WONDER_GUARD);
        if !how.similar_strength {
            loop {
                let chosen = pool[self.random.index(pool.len())];
                if wonder_guard || !guarded(self, chosen) {
                    return chosen;
                }
            }
        }
        // Within a tenth of its strength, and wider until some are found.
        let strength = self.strength(current);
        let (mut least, mut most) = (strength - strength / 10, strength + strength / 10);
        let mut fit: Vec<usize> = Vec::new();
        let mut rounds = 0;
        while fit.is_empty() || (fit.len() < 3 && rounds < 2) {
            for &n in pool {
                if (least..=most).contains(&self.strength(n)) && (wonder_guard || !guarded(self, n))
                {
                    fit.push(n);
                }
            }
            least -= strength / 20;
            most += strength / 20;
            rounds += 1;
        }
        let chosen = fit[self.random.index(fit.len())];
        let Some(used) = used else {
            return chosen;
        };
        // The original randomizer then chooses again among the rarer.
        let limit = average(used);
        let rarer: Vec<usize> = (fit.iter().copied())
            .filter(|&n| f64::from(used[n]) < limit)
            .collect();
        let list = if rarer.is_empty() { &fit } else { &rarer };
        list[self.random.index(list.len())]
    }

    /// The three Pokémon the game hands out at the start.
    fn current_starters(&self) -> Result<[usize; 3], RomError> {
        let script = self.scripts.get(STARTER_SCRIPT).ok_or(layout("script"))?;
        if script.len() < STARTER_PLACES[2][0] + 2 {
            return Err(layout("script"));
        }
        let starters = STARTER_PLACES.map(|places| usize::from(word(script, places[0])));
        if starters.iter().any(|s| !(1..=SPECIES).contains(s)) {
            return Err(layout("starters"));
        }
        Ok(starters)
    }

    /// A random type for themed trainers: every type alike, or with
    /// `weights` as often as the Pokémon of the pool have it.
    fn theme(&mut self, weights: Option<&[(u8, usize)]>) -> u8 {
        let Some(weights) = weights else {
            return self.random_type();
        };
        let total: usize = weights.iter().map(|&(_, count)| count).sum();
        let picked = self.random.index(total);
        let mut passed = 0;
        for &(kind, count) in weights {
            if passed + count > picked {
                return kind;
            }
            passed += count;
        }
        weights[0].0
    }

    /// Every trainer gets random Pokémon, with `forms` also other forms.
    /// From level `evolved_from` on they will be fully evolved later (0
    /// for never), which themed trainers take into account.
    pub fn randomize_trainers(
        &mut self,
        forms: bool,
        how: &TrainerRules,
        evolved_from: u16,
        rival_keeps_starter: bool,
    ) -> Result<(), RomError> {
        let last = if forms { SPECIES + FORMS } else { SPECIES };
        let usable = |game: &Self, n: usize| {
            let entry = &game.species[n];
            let species = if entry.base > 0 { entry.base } else { n };
            let legendary = LEGENDARIES.contains(&(species as u16));
            !entry.looks_only && !(how.no_legendaries && legendary)
        };
        // Everything of a type, and what trainers may get of it.
        let of_type = |game: &Self, kind: u8| -> Vec<usize> {
            let has = |n: usize| {
                let stats = &game.stats[Self::stats_file(n)];
                stats[FIRST_TYPE] == kind || stats[FIRST_TYPE + 1] == kind
            };
            (1..=last).filter(|&n| usable(game, n) && has(n)).collect()
        };
        let allowed = |game: &Self, n: usize| {
            let entry = &game.species[n];
            let by_ability =
                entry.base == CASTFORM || (entry.base == DARMANITAN && entry.form == 1);
            !by_ability || game.abilities_changed
        };
        let pool: Vec<usize> = (1..=last)
            .filter(|&n| usable(self, n) && allowed(self, n))
            .collect();
        let mut all = self.trainers()?;

        // Themed trainers: the groups first, one type each; no two gyms
        // and no two of the elite share one.
        let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        let mut tags: Vec<&str> = vec![""; all.len()];
        for (tag, members) in THEME_GROUPS {
            for &member in members.iter().filter(|&&m| m <= all.len()) {
                tags[member - 1] = tag;
            }
        }
        let weights: Vec<(u8, usize)> = (TYPES_IN_ORDER.iter())
            .map(|&kind| (kind, of_type(self, kind).len()))
            .collect();
        let weights = how.type_weights.then_some(weights.as_slice());
        let mut types: BTreeMap<usize, u8> = BTreeMap::new();
        let mut strongest: Vec<u8> = Vec::new();
        if how.themes != Themes::None {
            for (place, tag) in tags.iter().enumerate() {
                let group = tag.split('-').next().unwrap_or("");
                let whole = how.themes == Themes::All
                    && (group.starts_with("CHAMPION") || group.starts_with("THEMED"));
                if group.starts_with("GYM") || group.starts_with("ELITE") || whole {
                    groups.entry(group).or_default().push(place);
                }
            }
            let (mut gyms, mut elite): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
            for (group, members) in &mut groups {
                self.random.shuffle(members);
                let mut kind = self.theme(weights);
                for (prefix, taken) in [("GYM", &mut gyms), ("ELITE", &mut elite)] {
                    if group.starts_with(prefix) {
                        while taken.contains(&kind) {
                            kind = self.theme(weights);
                        }
                        taken.push(kind);
                    }
                }
                if *group == "CHAMPION" {
                    strongest.push(kind);
                }
                for &member in members.iter() {
                    types.insert(all[member].index, kind);
                }
            }
        }

        let mut order: Vec<usize> = (0..all.len()).collect();
        self.random.shuffle(&mut order);
        let mut used = vec![0u32; SPECIES + FORMS + 1];

        // The elite's strongest Pokémon are theirs alone: nobody else has
        // them, nor what they evolve from or into where that matters.
        let elite = |place: usize| how.elite_unique > 0 && ELITE.contains(&(place + 1));
        let mut before: Vec<Vec<usize>> = vec![Vec::new(); SPECIES + FORMS + 1];
        for from in 1..=SPECIES {
            for evolution in &self.species[from].evolutions {
                before[usize::from(evolution.to)].push(from);
            }
        }
        let after: Vec<Vec<usize>> = (self.species.iter())
            .map(|s| s.evolutions.iter().map(|e| usize::from(e.to)).collect())
            .collect();
        // A species' whole line: what it evolves from and, with `on`,
        // into.
        fn line(
            (before, after): (&[Vec<usize>], &[Vec<usize>]),
            species: usize,
            on: bool,
            list: &mut Vec<usize>,
        ) {
            fn walk(next: &[Vec<usize>], species: usize, list: &mut Vec<usize>) {
                for &other in &next[species] {
                    list.push(other);
                    walk(next, other, list);
                }
            }
            walk(before, species, list);
            if on {
                walk(after, species, list);
            }
        }
        fn last_stages(after: &[Vec<usize>], species: usize, list: &mut Vec<usize>) {
            if after[species].is_empty() {
                list.push(species);
            }
            for &other in &after[species] {
                last_stages(after, other, list);
            }
        }
        let lines = (before.as_slice(), after.as_slice());
        let mut unique: Vec<usize> = Vec::new();
        let mut not_if_evolved: Vec<usize> = Vec::new();
        let mut not_unique: Vec<usize> = Vec::new();
        if how.elite_unique > 0 {
            // Stable, so the others keep their random order.
            order.sort_by_key(|&place| !elite(place));
            if rival_keeps_starter {
                for starter in self.current_starters()? {
                    not_unique.push(starter);
                    line(lines, starter, true, &mut not_unique);
                }
            }
        }
        let mut typed: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
        for trainer in order {
            Self::change_levels(&mut all[trainer], how.levels);
            let index = all[trainer].index;
            let evenly = match how.spread {
                Spread::Free => false,
                Spread::Even => true,
                Spread::EvenInStory => {
                    (STORY_TRAINERS.iter()).any(|&(first, last)| (first..=last).contains(&index))
                }
            };
            let mut kind = types.get(&index).copied();
            if kind.is_none() && how.themes == Themes::All {
                let mut own = self.theme(weights);
                if tags[trainer] == "UBER" {
                    while strongest.contains(&own) {
                        own = self.theme(weights);
                    }
                    strongest.push(own);
                }
                kind = Some(own);
            }
            // What would leave the type when it is fully evolved.
            let mut leaving: Vec<usize> = Vec::new();
            if let Some(kind) = kind {
                let whole = of_type(self, kind);
                for &n in &whole {
                    if !whole.contains(&self.fully_evolved(n, index)) {
                        leaving.push(n);
                    }
                }
                // Themed trainers never have the forms a player's Pokémon
                // cannot keep: Giratina's and Shaymin's.
                typed.entry(kind).or_insert_with(|| {
                    (whole.into_iter())
                        .filter(|&n| allowed(self, n) && !NOT_KEPT.contains(&n))
                        .collect()
                });
            }
            let themed = kind.and_then(|kind| typed.get(&kind)).filter(|_| !evenly);
            // The elite's Pokémon from the last to the first, the
            // strongest ahead; only for choosing.
            let tracked = how.elite_unique > 0 && ELITE.contains(&index);
            let mut team: Vec<usize> = (0..all[trainer].team.len()).collect();
            if tracked {
                team.reverse();
                team.sort_by_key(|&i| std::cmp::Reverse(all[trainer].team[i].level));
            }
            for (turn, member) in team.into_iter().enumerate() {
                let pokemon = &mut all[trainer].team[member];
                let current = self.form_of(pokemon.species, pokemon.form);
                let wonder_guard = !how.no_early_wonder_guard || pokemon.level >= 20;
                let used_so_far = evenly.then_some(used.as_slice());
                let evolves = evolved_from > 0 && pokemon.level >= evolved_from;
                let alone = tracked && usize::from(how.elite_unique) > turn;
                let banned = |n: &usize| {
                    unique.contains(n)
                        || (evolves && (not_if_evolved.contains(n) || leaving.contains(n)))
                        || (alone && not_unique.contains(n))
                };
                let mut list: &[usize] = themed.map_or(&pool, Vec::as_slice);
                let left: Vec<usize> = list.iter().copied().filter(|n| !banned(n)).collect();
                if !left.is_empty() {
                    list = &left;
                }
                let chosen = self.trainer_pokemon(list, current, how, wonder_guard, used_so_far);
                if evenly {
                    used[chosen] += 1;
                }
                let chains = evolved_from > 0;
                if alone {
                    let mut actual = Vec::new();
                    if evolves {
                        last_stages(&after, chosen, &mut actual);
                    } else {
                        actual.push(chosen);
                    }
                    for species in actual {
                        unique.push(species);
                        if chains {
                            line(lines, species, evolves, &mut not_if_evolved);
                        }
                    }
                }
                if tracked {
                    not_unique.push(chosen);
                    if chains {
                        line(lines, chosen, evolves, &mut not_unique);
                    }
                }
                let pokemon = &mut all[trainer].team[member];
                self.become_form(pokemon, chosen);
                pokemon.ability = self.random_ability(chosen);
                pokemon.reset_moves = true;
            }
        }
        self.set_trainers(&all)
    }

    /// How often a species still evolves, counted up to two.
    fn evolves(&self, species: usize, deep: bool) -> usize {
        let list = &self.species[species].evolutions;
        match (list.is_empty(), deep) {
            (true, _) => 0,
            (false, true) => 1,
            (false, false) => {
                let further = list
                    .iter()
                    .map(|e| self.evolves(usize::from(e.to), true) + 1);
                further.max().unwrap_or(0)
            }
        }
    }

    fn random_evolution(&mut self, species: usize, evolves_again: bool) -> Result<usize, RomError> {
        let list = &self.species[species].evolutions;
        let fits = |e: &&Evolution| {
            !evolves_again || !self.species[usize::from(e.to)].evolutions.is_empty()
        };
        let choice: Vec<usize> = list
            .iter()
            .filter(fits)
            .map(|e| usize::from(e.to))
            .collect();
        if choice.is_empty() {
            return Err(layout("evolutions of a starter"));
        }
        Ok(choice[self.random.index(choice.len())])
    }

    /// The Pokémon of a team that is taken for the starter: the one with
    /// the highest level, the last one two levels ahead of its own.
    fn starter_place(trainer: &Trainer) -> usize {
        let mut best = 0;
        for (i, pokemon) in trainer.team.iter().enumerate().skip(1) {
            let bonus = if i == trainer.team.len() - 1 { 2 } else { 0 };
            if pokemon.level + bonus > trainer.team[best].level {
                best = i;
            }
        }
        best
    }

    fn rival_takes(
        &mut self,
        all: &mut [Trainer],
        battles: &[(u8, usize)],
        tag: Tag,
        starter: usize,
        ability: u8,
    ) {
        for trainer in all.iter_mut() {
            if tagged(battles, trainer) != Some(tag) || trainer.team.is_empty() {
                continue;
            }
            let place = Self::starter_place(trainer);
            self.become_form(&mut trainer.team[place], starter);
            trainer.team[place].reset_moves = true;
            trainer.team[place].ability = ability;
        }
    }

    fn rival_starters(
        &mut self,
        all: &mut [Trainer],
        battles: &[(u8, usize)],
        shift: usize,
    ) -> Result<(), RomError> {
        let starters = self.current_starters()?;
        let last = battles.iter().map(|b| b.0).max().unwrap_or(0);
        let level = |all: &[Trainer], tag: Tag| {
            let found = all.iter().find(|t| tagged(battles, t) == Some(tag));
            found
                .filter(|t| !t.team.is_empty())
                .map_or(0, |t| t.team[Self::starter_place(t)].level)
        };
        for i in 0..3 {
            let mut starter = starters[(i + shift) % 3];
            let times = self.evolves(starter, false);
            let mut ability = self.random_ability(starter);
            while ability == 3 {
                ability = self.random_ability(starter);
            }
            let keep = |game: &Self, starter: usize| {
                if ability == 2 && game.species[starter].abilities[1] == 0 {
                    1
                } else {
                    ability
                }
            };
            let mut battle = 1;
            // The levels from which the rival's starter has evolved once
            // and twice.
            let stages: &[(u8, u16)] = match times {
                0 => &[],
                1 => &[(last / 2, 30)],
                _ => &[(last, 16), (last, 36)],
            };
            let mut now = ability;
            for (stage, &(until, grown)) in stages.iter().enumerate() {
                while battle <= until && level(all, (battle, i)) < grown {
                    self.rival_takes(all, battles, (battle, i), starter, now);
                    battle += 1;
                }
                let evolves_again = times == 2 && stage == 0;
                starter = self.random_evolution(starter, evolves_again)?;
                now = keep(self, starter);
            }
            while battle <= last {
                self.rival_takes(all, battles, (battle, i), starter, now);
                battle += 1;
            }
        }
        Ok(())
    }

    /// The rival fights with the starter the player left for them, grown
    /// up over the battles.
    pub fn rival_carries_starter(&mut self) -> Result<(), RomError> {
        let mut all = self.trainers()?;
        self.rival_starters(&mut all, &RIVAL_BATTLES, 1)?;
        self.rival_starters(&mut all, &FRIEND_BATTLES, 2)?;
        self.set_trainers(&all)
    }

    /// What `species` ends up as when it evolves all the way; where it
    /// may go more than one way, the trainer decides which.
    fn fully_evolved(&mut self, mut species: usize, trainer: usize) -> usize {
        let turn = match self.evolution_turn {
            Some(turn) => turn,
            None => *self
                .evolution_turn
                .insert(self.random.index(MOST_SPLIT_EVOLUTIONS)),
        };
        let mut seen = vec![species];
        loop {
            let list = &self.species[species].evolutions;
            if list.is_empty() || list.iter().any(|e| seen.contains(&usize::from(e.to))) {
                return species;
            }
            species = usize::from(list[(turn + trainer) % list.len()].to);
            seen.push(species);
        }
    }

    /// Trainers' Pokémon from `level` on are fully evolved.
    pub fn force_fully_evolved(&mut self, level: u16) -> Result<(), RomError> {
        let mut all = self.trainers()?;
        for trainer in &mut all {
            for pokemon in &mut trainer.team {
                if pokemon.level < level {
                    continue;
                }
                let evolved = self.fully_evolved(pokemon.species, trainer.index);
                if evolved != pokemon.species {
                    self.become_form(pokemon, evolved);
                    if pokemon.ability == 2 && self.species[evolved].abilities[1] == 0 {
                        pokemon.ability = 1;
                    }
                    pokemon.reset_moves = true;
                }
            }
        }
        self.set_trainers(&all)
    }
}

/// New names for `current`: the same new name for the same old one, a
/// pair's name for a name with an "&".
fn new_names(current: &[Vec<u16>], random: &mut JavaRandom) -> Vec<Vec<u16>> {
    let fitting = |list: &[&str]| -> Vec<Vec<u16>> {
        let short = list.iter().filter(|n| n.chars().count() <= LONGEST_NAME);
        short.map(|n| chars(n)).collect()
    };
    let lists = [fitting(&names::TRAINERS), fitting(&names::PAIRS)];
    let shared = |name: &[u16]| {
        let upper = String::from_utf16_lossy(name).to_uppercase();
        SHARED_NAMES.contains(&upper.as_str())
    };
    let mut given: Vec<(&[u16], Vec<u16>)> = Vec::new();
    let mut all = Vec::with_capacity(current.len());
    for name in current {
        let known = given.iter().position(|(old, _)| *old == name.as_slice());
        if let Some(known) = known.filter(|_| !shared(name)) {
            all.push(given[known].1.clone());
            continue;
        }
        let list = &lists[usize::from(name.contains(&u16::from(b'&')))];
        let chosen = if list.is_empty() || name.is_empty() {
            name.clone()
        } else {
            list[random.index(list.len())].clone()
        };
        match known {
            Some(known) => given[known].1 = chosen.clone(),
            None => given.push((name, chosen.clone())),
        }
        all.push(chosen);
    }
    all
}

impl<'a> Game<'a> {
    /// Trainers get random names from the lists of the original.
    pub fn randomize_trainer_names(&mut self) -> Result<(), RomError> {
        let read = |game: &Self, file: usize| {
            let text = game.strings.get(file).and_then(|data| Text::read(data));
            text.ok_or(layout("names of the trainers"))
        };
        let mut trainers = read(self, NAMES)?;
        let mut tournament = read(self, TOURNAMENT_NAMES)?;
        let mut faces = read(self, FACE_NAMES)?;
        let named = |line: &Vec<u16>| line.first().is_some_and(|c| (65..=90).contains(c));
        let mut current: Vec<Vec<u16>> = trainers.lines.iter().skip(1).cloned().collect();
        current.extend(tournament.lines.iter().cloned());
        current.extend(faces.lines.iter().filter(|l| named(l)).cloned());

        let mut fresh = new_names(&current, &mut self.looks_random).into_iter();
        for line in trainers.lines.iter_mut().skip(1) {
            *line = fresh.next().ok_or(layout("names of the trainers"))?;
        }
        for line in tournament.lines.iter_mut() {
            *line = fresh.next().ok_or(layout("names of the trainers"))?;
        }
        for line in faces.lines.iter_mut().filter(|l| named(l)) {
            *line = fresh.next().ok_or(layout("names of the trainers"))?;
        }
        for (file, text) in [
            (NAMES, trainers),
            (TOURNAMENT_NAMES, tournament),
            (FACE_NAMES, faces),
        ] {
            let written = text.write(&self.strings[file]);
            self.strings[file] = written.ok_or(layout("names of the trainers"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pokemon(level: u16) -> TrainerPokemon {
        TrainerPokemon {
            ivs: 0,
            ability: 0,
            gender: 0,
            level,
            species: 1,
            form: 0,
            item: 0,
            moves: [0; 4],
            reset_moves: false,
        }
    }

    fn trainer(index: usize, levels: &[u16]) -> Trainer {
        Trainer {
            index,
            kind: 0,
            team: levels.iter().map(|&l| pokemon(l)).collect(),
        }
    }

    #[test]
    fn the_newest_four_moves_are_known() {
        let learnset = [(1, 1), (2, 1), (2, 5), (3, 7), (4, 9), (5, 12), (6, 30)];
        assert_eq!(Game::moves_at(&learnset, 4), [1, 2, 0, 0]);
        assert_eq!(Game::moves_at(&learnset, 9), [1, 2, 3, 4]);
        assert_eq!(Game::moves_at(&learnset, 29), [2, 3, 4, 5]);
    }

    #[test]
    fn the_starter_is_the_strongest_with_the_last_ahead() {
        assert_eq!(Game::starter_place(&trainer(1, &[10, 13, 11])), 1);
        // The last one wins while it is at most one level behind.
        assert_eq!(Game::starter_place(&trainer(1, &[10, 13, 12])), 2);
        assert_eq!(Game::starter_place(&trainer(1, &[10, 14, 12])), 1);
        assert_eq!(Game::starter_place(&trainer(1, &[10, 11, 12])), 2);
        assert_eq!(Game::starter_place(&trainer(1, &[12, 11, 11])), 2);
        assert_eq!(Game::starter_place(&trainer(1, &[14])), 0);
    }

    #[test]
    fn the_same_trainer_keeps_one_new_name() {
        let old: Vec<Vec<u16>> = ["Anna", "Tim&Tom", "Anna", "", "GRUNT", "GRUNT"]
            .iter()
            .map(|n| chars(n))
            .collect();
        let new = new_names(&old, &mut JavaRandom::new(4));
        let text = |name: &Vec<u16>| String::from_utf16_lossy(name);
        assert_eq!(new[0], new[2]);
        assert!(names::TRAINERS.contains(&text(&new[0]).as_str()));
        assert!(names::PAIRS.contains(&text(&new[1]).as_str()));
        assert!(new[3].is_empty());
        // Shared names are drawn anew each time.
        let mut random = JavaRandom::new(4);
        let grunts = new_names(&vec![chars("Grunt"); 40], &mut random);
        assert!(grunts.iter().any(|g| *g != grunts[0]));
        assert!(new.iter().all(|n| n.len() <= LONGEST_NAME));
    }

    #[test]
    fn rival_battles_come_in_threes() {
        assert_eq!(tagged(&RIVAL_BATTLES, &trainer(0xA1, &[])), Some((1, 0)));
        assert_eq!(tagged(&RIVAL_BATTLES, &trainer(0x2BA, &[])), Some((10, 2)));
        assert_eq!(tagged(&RIVAL_BATTLES, &trainer(0xA4, &[])), None);
        assert_eq!(tagged(&FRIEND_BATTLES, &trainer(0x16C, &[])), Some((1, 1)));
        // The trainers of the tournament have no number.
        assert_eq!(tagged(&RIVAL_BATTLES, &trainer(0, &[])), None);
    }
}
