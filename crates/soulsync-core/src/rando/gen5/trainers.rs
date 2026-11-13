//! The teams of the trainers: read and written back the way the original
//! randomizer does, and the steps that change them.

use super::*;

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
    reset_moves: bool,
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
    fn form_of(&self, species: usize, form: u16) -> usize {
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

    /// Every trainer gets random Pokémon, with `forms` also other forms.
    pub fn randomize_trainers(&mut self, forms: bool) -> Result<(), RomError> {
        let last = if forms { SPECIES + FORMS } else { SPECIES };
        let pool: Vec<usize> = (1..=last)
            .filter(|&n| {
                let entry = &self.species[n];
                let by_ability =
                    entry.base == CASTFORM || (entry.base == DARMANITAN && entry.form == 1);
                !entry.looks_only && !by_ability
            })
            .collect();
        let mut all = self.trainers()?;
        let mut order: Vec<usize> = (0..all.len()).collect();
        self.random.shuffle(&mut order);
        for trainer in order {
            for pokemon in &mut all[trainer].team {
                let chosen = pool[self.random.index(pool.len())];
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
        let script = self.scripts.get(STARTER_SCRIPT).ok_or(layout("script"))?;
        if script.len() < STARTER_PLACES[2][0] + 2 {
            return Err(layout("script"));
        }
        let starters = STARTER_PLACES.map(|places| usize::from(word(script, places[0])));
        if starters.iter().any(|s| !(1..=SPECIES).contains(s)) {
            return Err(layout("starters"));
        }
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
    fn rival_battles_come_in_threes() {
        assert_eq!(tagged(&RIVAL_BATTLES, &trainer(0xA1, &[])), Some((1, 0)));
        assert_eq!(tagged(&RIVAL_BATTLES, &trainer(0x2BA, &[])), Some((10, 2)));
        assert_eq!(tagged(&RIVAL_BATTLES, &trainer(0xA4, &[])), None);
        assert_eq!(tagged(&FRIEND_BATTLES, &trainer(0x16C, &[])), Some((1, 1)));
        // The trainers of the tournament have no number.
        assert_eq!(tagged(&RIVAL_BATTLES, &trainer(0, &[])), None);
    }
}
