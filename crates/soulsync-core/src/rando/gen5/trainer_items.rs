//! Random held items for the trainers' Pokémon.

use super::movesets::TYPE_PLACES;
use super::trainers::{rank, tags, Rank, TrainerPokemon};
use super::*;

/// Which trainers' Pokémon hold random items, and which items.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrainerItems {
    pub boss: bool,
    pub important: bool,
    pub regular: bool,
    /// Only items that are used up in battle.
    pub consumable: bool,
    /// Only items that are of use to the Pokémon holding them.
    pub sensible: bool,
    /// Only the trainer's Pokémon of the highest level holds one.
    pub strongest_only: bool,
}

impl TrainerItems {
    pub fn any(&self) -> bool {
        self.boss || self.important || self.regular
    }
}

/// Where a move's data says whether it is physical or special.
const CATEGORY: usize = 2;
const PHYSICAL: u8 = 1;
const SPECIAL: u8 = 2;
const LEVITATE: u8 = 26;
/// Eviolite is too good early on; nobody holds it below this level.
const EVIOLITE_FROM: u16 = 20;

const LIECHI_BERRY: u16 = 201;
const PETAYA_BERRY: u16 = 204;
const CHOICE_BAND: u16 = 220;
const MUSCLE_BAND: u16 = 266;
const WISE_GLASSES: u16 = 267;
const CHOICE_SPECS: u16 = 297;
const CHILAN_BERRY: u16 = 200;
const SHUCA_BERRY: u16 = 191;
const AIR_BALLOON: u16 = 541;
const BLACK_SLUDGE: u16 = 281;
const EVIOLITE: u16 = 538;

/// Everything used up in battle.
const CONSUMABLE: [u16; 71] = [
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 184, 185, 186, 187,
    188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206,
    207, 208, 209, 210, 211, 212, 43, 214, 219, 271, 275, 541, 542, 545, 546, 547, 548, 549, 550,
    551, 552, 553, 554, 555, 556, 557, 558, 559, 560, 561, 562, 563, 564,
];
/// What is held for good, and comes before `CONSUMABLE` among all items.
const LASTING: [u16; 74] = [
    213, 217, 220, 221, 222, 230, 232, 233, 234, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246,
    247, 248, 249, 250, 251, 253, 254, 255, 265, 266, 267, 268, 269, 270, 272, 273, 276, 277, 278,
    279, 280, 281, 282, 283, 284, 285, 286, 287, 288, 295, 296, 297, 298, 299, 300, 301, 302, 303,
    304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315, 316, 317, 318, 326, 327,
];
/// What only the fifth generation has, after all the others.
const NEWEST: [u16; 5] = [538, 539, 540, 543, 544];

/// Of use to every Pokémon: what is used up, and what lasts.
const GENERAL_CONSUMABLE: [u16; 28] = [
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 202, 203, 205, 206, 207, 208, 209, 210, 211,
    212, 43, 214, 219, 275, 542, 545, 546, 547,
];
const GENERAL: [u16; 18] = [
    213, 217, 221, 230, 232, 234, 253, 255, 265, 268, 270, 276, 280, 295, 326, 327, 539, 540,
];

/// For each type, in the original randomizer's order of types: the gem
/// and the lasting items that strengthen its moves, and the berry
/// against it (none against Normal, which nothing is weak to).
const BY_TYPE: [(u16, &[u16], u16); 17] = [
    (564, &[251], 0),
    (553, &[241, 303], 189),
    (556, &[244, 306], 192),
    (551, &[239, 301, 318], 187),
    (549, &[243, 254, 299, 317], 185),
    (548, &[249, 298], 184),
    (559, &[238, 309, 315], 195),
    (555, &[237, 305], 191),
    (557, &[248, 307, 314], 193),
    (558, &[222, 308], 194),
    (561, &[250, 311], 197),
    (550, &[242, 300], 186),
    (560, &[247, 310], 196),
    (554, &[245, 304], 190),
    (552, &[246, 302], 188),
    (563, &[233, 313], 199),
    (562, &[240, 312], 198),
];
const GROUND: usize = 7;
/// Poison as the game numbers its types.
const POISON_TYPE: u8 = 3;
const GHOST: usize = 12;
const STEEL: usize = 15;
const DARK: usize = 16;

/// Items that go well with a move.
const FOR_MOVES: [(u16, &[u16]); 39] = [
    (13, &[271]),
    (19, &[271]),
    (20, &[286, 544]),
    (35, &[286, 544]),
    (71, &[296]),
    (72, &[296]),
    (73, &[296]),
    (76, &[271]),
    (83, &[286, 544]),
    (91, &[271]),
    (113, &[269]),
    (115, &[269]),
    (128, &[286, 544]),
    (130, &[271]),
    (138, &[296]),
    (141, &[296]),
    (143, &[271]),
    (200, &[286]),
    (201, &[283]),
    (202, &[296]),
    (240, &[285]),
    (241, &[284]),
    (250, &[286, 544]),
    (253, &[286]),
    (258, &[282]),
    (263, &[272, 273]),
    (271, &[272, 273, 543]),
    (275, &[296]),
    (291, &[271]),
    (328, &[286, 544]),
    (340, &[271]),
    (374, &[272, 273, 278]),
    (375, &[272, 273]),
    (392, &[296]),
    (409, &[296]),
    (415, &[272, 273, 543]),
    (433, &[278]),
    (463, &[286, 544]),
    (532, &[296]),
];
const FOR_ABILITIES: [(u8, &[u16]); 2] = [(62, &[273, 272]), (98, &[288, 270])];
const FOR_SPECIES: [(usize, &[u16]); 11] = [
    (25, &[236]),
    (83, &[259]),
    (104, &[258]),
    (105, &[258]),
    (113, &[256]),
    (132, &[257, 274]),
    (366, &[226, 227]),
    (380, &[225]),
    (381, &[225]),
    (483, &[135]),
    (484, &[136]),
];

/// How hard a move of the type of the row hits a Pokémon of the type of
/// the column, in halves; the types in the original randomizer's order.
const HITS: [[u8; 17]; 17] = [
    [2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 0, 2, 2, 1, 2],
    [4, 2, 1, 2, 2, 2, 4, 2, 2, 1, 2, 2, 0, 1, 4, 4, 4],
    [2, 4, 2, 4, 2, 2, 1, 2, 2, 4, 2, 1, 2, 2, 2, 1, 2],
    [2, 2, 1, 1, 4, 1, 4, 4, 2, 1, 1, 2, 2, 1, 2, 1, 2],
    [2, 2, 2, 1, 1, 4, 4, 4, 2, 2, 1, 2, 2, 2, 2, 2, 2],
    [2, 2, 2, 4, 1, 1, 1, 2, 2, 4, 1, 2, 2, 2, 4, 4, 2],
    [2, 1, 4, 2, 2, 4, 2, 1, 2, 4, 2, 2, 2, 2, 4, 1, 2],
    [2, 2, 0, 1, 2, 4, 4, 2, 2, 1, 2, 4, 2, 4, 2, 4, 2],
    [2, 4, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 4, 2, 1, 0],
    [2, 1, 1, 4, 2, 1, 2, 2, 4, 2, 2, 2, 1, 1, 2, 1, 4],
    [2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 1, 2],
    [2, 2, 4, 1, 4, 2, 2, 0, 2, 2, 1, 1, 2, 2, 2, 2, 2],
    [0, 2, 2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 4, 2, 2, 1, 1],
    [2, 2, 2, 4, 2, 2, 1, 1, 2, 2, 2, 2, 1, 1, 2, 0, 2],
    [2, 2, 4, 4, 1, 1, 2, 4, 2, 2, 4, 2, 2, 2, 1, 1, 2],
    [2, 2, 2, 2, 1, 1, 4, 2, 2, 2, 2, 1, 2, 2, 4, 1, 2],
    [2, 1, 2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 4, 2, 2, 1, 1],
];
/// What `hits_on` gives for a hit of normal, double and fourfold strength.
const NORMAL: u8 = 4;
const DOUBLE: u8 = 8;
const FOURFOLD: u8 = 16;

/// How hard a move of type `attack` hits a Pokémon of the types
/// `first` and `second`, in quarters. With `updated`, Ghost and Dark
/// hit Steel normally.
fn hits_on(attack: usize, first: usize, second: Option<usize>, updated: bool) -> u8 {
    let single = |defence: usize| {
        if updated && defence == STEEL && (attack == GHOST || attack == DARK) {
            2
        } else {
            HITS[attack][defence]
        }
    };
    single(first) * second.map_or(2, single)
}

/// The place of a type of the game among the original randomizer's.
fn place(of: u8) -> usize {
    usize::from(TYPE_PLACES.get(usize::from(of)).copied().unwrap_or(0))
}

/// Whether no two Pokémon of a team hold the same item.
fn all_differ(team: &[TrainerPokemon]) -> bool {
    let held = |p: &&TrainerPokemon| p.item > 0;
    (team.iter().filter(held).enumerate())
        .all(|(i, p)| !team.iter().filter(held).take(i).any(|q| q.item == p.item))
}

impl Game<'_> {
    /// The items that are of use to one Pokémon of a trainer, knowing
    /// `moves`; what is of more use comes up more often.
    fn sensible_items(
        &self,
        pokemon: &TrainerPokemon,
        moves: [u16; 4],
        how: &TrainerItems,
    ) -> Vec<u16> {
        let lasting = !how.consumable;
        let mut items = GENERAL_CONSUMABLE.to_vec();
        // The very good ones come up more often, but not too often.
        let often = if lasting { 8 } else { 6 };
        if lasting {
            items.extend(GENERAL);
        }
        for number in moves {
            let Some(data) = (number > 0)
                .then(|| self.moves.get(usize::from(number)))
                .flatten()
            else {
                continue;
            };
            let (gem, boosts, _) = BY_TYPE[place(data[0])];
            for (category, berry, strong) in [
                (PHYSICAL, LIECHI_BERRY, [CHOICE_BAND, MUSCLE_BAND]),
                (SPECIAL, PETAYA_BERRY, [WISE_GLASSES, CHOICE_SPECS]),
            ] {
                if data[CATEGORY] == category {
                    items.extend([berry, gem]);
                    if lasting {
                        items.extend(boosts);
                        items.extend(strong);
                    }
                }
            }
            if lasting {
                if let Some((_, with)) = FOR_MOVES.iter().find(|(m, _)| *m == number) {
                    items.extend(*with);
                }
            }
        }
        // A form has the types and abilities of its species here.
        let (first, second) = self.types_of(pokemon.species);
        let hit = |attack: usize| {
            hits_on(
                attack,
                place(first),
                second.map(place),
                self.effectiveness_updated,
            )
        };
        // The original goes through the types in an order that differs
        // from run to run; this is the order of its list of types.
        for (attack, &(_, _, berry)) in BY_TYPE.iter().enumerate() {
            match hit(attack) {
                DOUBLE => items.push(berry),
                FOURFOLD => items.extend([berry; 8].iter().take(often)),
                _ => {}
            }
        }
        if hit(0) == NORMAL {
            items.push(CHILAN_BERRY);
        }
        let ability = match pokemon.ability {
            slot @ 1..=3 => self.species[pokemon.species].abilities[usize::from(slot) - 1],
            _ => 0,
        };
        if ability == LEVITATE {
            items.retain(|&item| item != SHUCA_BERRY);
        } else if hit(GROUND) >= DOUBLE {
            items.push(AIR_BALLOON);
        }
        if lasting {
            if let Some((_, with)) = FOR_ABILITIES.iter().find(|(a, _)| *a == ability) {
                items.extend(*with);
            }
            if first == POISON_TYPE || second == Some(POISON_TYPE) {
                items.push(BLACK_SLUDGE);
            }
            if let Some((_, with)) = FOR_SPECIES.iter().find(|(s, _)| *s == pokemon.species) {
                for _ in 0..often {
                    items.extend(*with);
                }
            }
            let evolves = !self.species[pokemon.species].evolutions.is_empty();
            if evolves && pokemon.level >= EVIOLITE_FROM {
                items.push(EVIOLITE);
            }
        }
        items
    }

    /// The Pokémon of the trainers of the chosen ranks hold random
    /// items. The first battles with the rival stay as they are.
    pub fn randomize_trainer_items(&mut self, how: &TrainerItems) -> Result<(), RomError> {
        let mut all = self.trainers()?;
        let learnsets = self.learnsets()?;
        let tags = tags(all.len());
        let everything: Vec<u16> = if how.consumable {
            CONSUMABLE.to_vec()
        } else {
            LASTING
                .iter()
                .chain(&CONSUMABLE)
                .chain(&NEWEST)
                .copied()
                .collect()
        };
        for (place, trainer) in all.iter_mut().enumerate() {
            let wanted = match rank(&tags, place, trainer) {
                Some(Rank::Boss) => how.boss,
                Some(Rank::Important) => how.important,
                Some(Rank::Regular) => how.regular,
                None => false,
            };
            if !wanted {
                continue;
            }
            trainer.hold_items();
            let strongest = (trainer.team.iter().enumerate())
                .rev()
                .max_by_key(|(_, p)| p.level)
                .map(|(i, _)| i);
            // The trainers of the tournament may not have an item twice.
            let unique = trainer.index == 0 && !how.strongest_only;
            for i in 0..trainer.team.len() {
                if how.strongest_only && Some(i) != strongest {
                    continue;
                }
                let pokemon = &trainer.team[i];
                let moves = if pokemon.reset_moves {
                    let number = self.form_of(pokemon.species, pokemon.form);
                    Self::moves_at(&learnsets[number], pokemon.level)
                } else {
                    pokemon.moves
                };
                let choice = if how.sensible {
                    self.sensible_items(pokemon, moves, how)
                } else {
                    everything.clone()
                };
                loop {
                    trainer.team[i].item = choice[self.random.index(choice.len())];
                    if !unique || all_differ(&trainer.team) {
                        break;
                    }
                }
            }
        }
        self.set_trainers(&all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_types_multiply() {
        // Ground on Fire and Rock, on Flying, on Water alone.
        assert_eq!(hits_on(GROUND, 5, Some(6), false), FOURFOLD);
        assert_eq!(hits_on(GROUND, 5, Some(2), false), 0);
        assert_eq!(hits_on(GROUND, 4, None, false), NORMAL);
        assert_eq!(hits_on(GROUND, 5, None, false), DOUBLE);
    }

    #[test]
    fn steel_resists_ghost_until_updated() {
        assert_eq!(hits_on(GHOST, STEEL, None, false), 2);
        assert_eq!(hits_on(GHOST, STEEL, None, true), NORMAL);
        assert_eq!(hits_on(DARK, STEEL, Some(8), true), DOUBLE);
    }

    #[test]
    fn every_held_item_is_listed_once() {
        let mut all: Vec<u16> = LASTING
            .iter()
            .chain(&CONSUMABLE)
            .chain(&NEWEST)
            .copied()
            .collect();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 150);
    }
}
