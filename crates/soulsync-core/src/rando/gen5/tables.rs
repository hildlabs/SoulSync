//! Places in the games the original randomizer knows by number, taken
//! from the tables of Universal Pokémon Randomizer ZX.

/// A Pokémon the game hands out or lets the player meet at a fixed spot:
/// the places of its species, levels and form, each as a file of the
/// scripts (for the levels of some, of the maps) and an offset in it.
pub struct Static {
    pub species: &'static [(usize, usize)],
    pub levels: &'static [(usize, usize)],
    pub forms: &'static [(usize, usize)],
}

/// The species that count as legendary.
pub const LEGENDARIES: [u16; 48] = [
    144, 145, 146, 150, 151, 243, 244, 245, 249, 250, 251, 377, 378, 379, 380, 381, 382, 383, 384,
    385, 386, 480, 481, 482, 483, 484, 485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 638, 639,
    640, 641, 642, 643, 644, 645, 646, 647, 648, 649,
];

/// The newer species that may hide in a Hidden Grotto; all older ones may.
pub const GROTTO_UNOVA: [u16; 53] = [
    505, 507, 510, 511, 513, 515, 519, 523, 525, 527, 529, 531, 533, 535, 538, 539, 542, 545, 546,
    548, 550, 553, 556, 558, 559, 561, 564, 569, 572, 575, 578, 580, 583, 587, 588, 594, 596, 601,
    605, 607, 610, 613, 616, 618, 619, 621, 622, 624, 626, 628, 630, 631, 632,
];

/// The fixed Pokémon of Black 2 and White 2 outside Japan.
pub const STATICS: [Static; 41] = [
    Static {
        species: &[
            (662, 0x1de),
            (662, 0x240),
            (740, 0xcd),
            (740, 0xfc),
            (740, 0x12c),
            (740, 0x14c),
        ],
        levels: &[(740, 0x12e), (740, 0x14e)],
        forms: &[],
    },
    Static {
        species: &[(730, 0x13a), (730, 0x15f), (730, 0x19b), (730, 0x1bb)],
        levels: &[(730, 0x19d), (730, 0x1bd)],
        forms: &[],
    },
    Static {
        species: &[(948, 0x45d), (948, 0x48d), (948, 0x4ad)],
        levels: &[(948, 0x48f), (948, 0x4af)],
        forms: &[],
    },
    Static {
        species: &[
            (426, 0x38a),
            (426, 0x39b),
            (556, 0x367),
            (556, 0x568),
            (556, 0x5e6),
            (556, 0x6e1),
            (1208, 0x3a4),
            (1208, 0xa6a),
            (1208, 0x717),
        ],
        levels: &[(426, 0x39d)],
        forms: &[],
    },
    Static {
        species: &[
            (426, 0x36b),
            (426, 0x37c),
            (556, 0x350),
            (556, 0x551),
            (556, 0x5c7),
            (556, 0x6c3),
            (1208, 0x38d),
            (1208, 0xa53),
            (1208, 0x706),
        ],
        levels: &[(426, 0x37e)],
        forms: &[],
    },
    Static {
        species: &[
            (1112, 0x133),
            (1122, 0x2ba),
            (1122, 0x311),
            (1128, 0x37a),
            (1128, 0x3d1),
            (1208, 0x1b7),
            (1208, 0x1f8),
            (1208, 0x723),
            (1208, 0xf3d),
            (1208, 0xf4e),
        ],
        levels: &[(1208, 0xf50)],
        forms: &[],
    },
    Static {
        species: &[(1208, 0xd8b), (1208, 0xd97)],
        levels: &[(1208, 0xd99)],
        forms: &[(1208, 0xd8d), (1208, 0xd9b)],
    },
    Static {
        species: &[(1208, 0xdb6), (1208, 0xdc2)],
        levels: &[(1208, 0xdc4)],
        forms: &[(1208, 0xdb8), (1208, 0xdc6)],
    },
    Static {
        species: &[
            (304, 0xcc),
            (304, 0x14b),
            (304, 0x1bc),
            (304, 0x237),
            (304, 0x327),
            (304, 0x3e6),
            (304, 0x4a1),
            (304, 0x54a),
            (304, 0x5bd),
            (304, 0x5ce),
        ],
        levels: &[(304, 0x5d0)],
        forms: &[],
    },
    Static {
        species: &[
            (304, 0xb5),
            (304, 0x134),
            (304, 0x1a5),
            (304, 0x220),
            (304, 0x310),
            (304, 0x3cf),
            (304, 0x48a),
            (304, 0x533),
            (304, 0x59e),
            (304, 0x5af),
        ],
        levels: &[(304, 0x5b1)],
        forms: &[],
    },
    Static {
        species: &[(32, 0x247), (32, 0x2b0), (32, 0x2c1), (1034, 0x12a)],
        levels: &[(32, 0x2c3)],
        forms: &[],
    },
    Static {
        species: &[(684, 0x136), (684, 0x1c2), (684, 0x1d3), (1034, 0x169)],
        levels: &[(684, 0x1d5)],
        forms: &[],
    },
    Static {
        species: &[(950, 0xa1), (950, 0x10a), (950, 0x11b), (1034, 0x1be)],
        levels: &[(950, 0x11d)],
        forms: &[],
    },
    Static {
        species: &[(1222, 0x134), (1222, 0x145), (1018, 0x32)],
        levels: &[(1222, 0x147)],
        forms: &[],
    },
    Static {
        species: &[(1224, 0x134), (1224, 0x145), (1018, 0x2c)],
        levels: &[(1224, 0x147)],
        forms: &[],
    },
    Static {
        species: &[(1226, 0x134), (1226, 0x145), (1018, 0x38)],
        levels: &[(1226, 0x147)],
        forms: &[],
    },
    Static {
        species: &[(1018, 0x97), (1018, 0xa8)],
        levels: &[(1018, 0xaa)],
        forms: &[],
    },
    Static {
        species: &[(526, 0x48d), (526, 0x512), (526, 0x523)],
        levels: &[(526, 0x525)],
        forms: &[],
    },
    Static {
        species: &[
            (1068, 0x193),
            (1068, 0x1d6),
            (1068, 0x1e7),
            (1080, 0x193),
            (1080, 0x1d6),
            (1080, 0x1e7),
        ],
        levels: &[(1068, 0x1e9), (1080, 0x1e9)],
        forms: &[],
    },
    Static {
        species: &[(652, 0x5c6), (652, 0x5e9)],
        levels: &[(652, 0x5eb)],
        forms: &[],
    },
    Static {
        species: &[(1102, 0x592), (1102, 0x5b5)],
        levels: &[(1102, 0x5b7)],
        forms: &[],
    },
    Static {
        species: &[(364, 0xe), (364, 0x32), (364, 0x40)],
        levels: &[(364, 0x34), (364, 0x42)],
        forms: &[],
    },
    Static {
        species: &[(1030, 0x290), (1030, 0x2a1)],
        levels: &[(1030, 0x2a3)],
        forms: &[],
    },
    Static {
        species: &[(480, 0xe1), (480, 0x10a), (480, 0x131), (480, 0x15a)],
        levels: &[(480, 0x10c), (480, 0x15c)],
        forms: &[],
    },
    Static {
        species: &[(1168, 0x2c), (1168, 0x4f)],
        levels: &[(1168, 0x51)],
        forms: &[],
    },
    Static {
        species: &[(988, 0x382)],
        levels: &[(988, 0x386)],
        forms: &[],
    },
    Static {
        species: &[(664, 0x3b5), (664, 0x3e2), (664, 0x40f), (664, 0x43c)],
        levels: &[(664, 0x3b9), (664, 0x3e6), (664, 0x413), (664, 0x440)],
        forms: &[(664, 0x3b7), (664, 0x3e4), (664, 0x411), (664, 0x43e)],
    },
    Static {
        species: &[(880, 0xab4), (880, 0xac7)],
        levels: &[(880, 0xab8)],
        forms: &[],
    },
    Static {
        species: &[(880, 0xad3), (880, 0xae6)],
        levels: &[(880, 0xad7)],
        forms: &[],
    },
    Static {
        species: &[(54, 0xdd)],
        levels: &[],
        forms: &[],
    },
    Static {
        species: &[(526, 0x27e)],
        levels: &[(526, 0x282)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x5e0)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x5ff)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x61e)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x63d)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x65c)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x67b)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x69a)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x6b9)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x6d8)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(208, 0x5a6)],
        levels: &[(208, 0x5a8)],
        forms: &[],
    },
];

/// The same for the Japanese games.
pub const STATICS_JAPAN: [Static; 41] = [
    Static {
        species: &[
            (662, 0x1de),
            (662, 0x240),
            (740, 0xcd),
            (740, 0xfc),
            (740, 0x12c),
            (740, 0x14c),
        ],
        levels: &[(740, 0x12e), (740, 0x14e)],
        forms: &[],
    },
    Static {
        species: &[(730, 0x13a), (730, 0x15f), (730, 0x19b), (730, 0x1bb)],
        levels: &[(730, 0x19d), (730, 0x1bd)],
        forms: &[],
    },
    Static {
        species: &[(948, 0x45d), (948, 0x48d), (948, 0x4ad)],
        levels: &[(948, 0x48f), (948, 0x4af)],
        forms: &[],
    },
    Static {
        species: &[
            (426, 0x38a),
            (426, 0x39b),
            (556, 0x367),
            (556, 0x568),
            (556, 0x5e6),
            (556, 0x6e1),
            (1208, 0x3a4),
            (1208, 0xa6a),
            (1208, 0x717),
        ],
        levels: &[(426, 0x39d)],
        forms: &[],
    },
    Static {
        species: &[
            (426, 0x36b),
            (426, 0x37c),
            (556, 0x350),
            (556, 0x551),
            (556, 0x5c7),
            (556, 0x6c3),
            (1208, 0x38d),
            (1208, 0xa53),
            (1208, 0x706),
        ],
        levels: &[(426, 0x37e)],
        forms: &[],
    },
    Static {
        species: &[
            (1112, 0x133),
            (1122, 0x2ba),
            (1122, 0x311),
            (1128, 0x37a),
            (1128, 0x3d1),
            (1208, 0x1b7),
            (1208, 0x1f8),
            (1208, 0x723),
            (1208, 0xf3d),
            (1208, 0xf4e),
        ],
        levels: &[(1208, 0xf50)],
        forms: &[],
    },
    Static {
        species: &[(1208, 0xd8b), (1208, 0xd97)],
        levels: &[(1208, 0xd99)],
        forms: &[(1208, 0xd8d), (1208, 0xd9b)],
    },
    Static {
        species: &[(1208, 0xdb6), (1208, 0xdc2)],
        levels: &[(1208, 0xdc4)],
        forms: &[(1208, 0xdb8), (1208, 0xdc6)],
    },
    Static {
        species: &[
            (304, 0xcc),
            (304, 0x14b),
            (304, 0x1b8),
            (304, 0x22f),
            (304, 0x31b),
            (304, 0x3d6),
            (304, 0x491),
            (304, 0x536),
            (304, 0x5a9),
            (304, 0x5ba),
        ],
        levels: &[(304, 0x5bc)],
        forms: &[],
    },
    Static {
        species: &[
            (304, 0xb5),
            (304, 0x134),
            (304, 0x1a1),
            (304, 0x218),
            (304, 0x304),
            (304, 0x3bf),
            (304, 0x47a),
            (304, 0x51f),
            (304, 0x58a),
            (304, 0x59b),
        ],
        levels: &[(304, 0x59d)],
        forms: &[],
    },
    Static {
        species: &[(32, 0x247), (32, 0x2b0), (32, 0x2c1), (1034, 0x12a)],
        levels: &[(32, 0x2c3)],
        forms: &[],
    },
    Static {
        species: &[(684, 0x136), (684, 0x1c2), (684, 0x1d3), (1034, 0x169)],
        levels: &[(684, 0x1d5)],
        forms: &[],
    },
    Static {
        species: &[(950, 0xa1), (950, 0x10a), (950, 0x11b), (1034, 0x1be)],
        levels: &[(950, 0x11d)],
        forms: &[],
    },
    Static {
        species: &[(1222, 0x134), (1222, 0x145), (1018, 0x32)],
        levels: &[(1222, 0x147)],
        forms: &[],
    },
    Static {
        species: &[(1224, 0x134), (1224, 0x145), (1018, 0x2c)],
        levels: &[(1224, 0x147)],
        forms: &[],
    },
    Static {
        species: &[(1226, 0x134), (1226, 0x145), (1018, 0x38)],
        levels: &[(1226, 0x147)],
        forms: &[],
    },
    Static {
        species: &[(1018, 0x97), (1018, 0xa8)],
        levels: &[(1018, 0xaa)],
        forms: &[],
    },
    Static {
        species: &[(526, 0x48d), (526, 0x512), (526, 0x523)],
        levels: &[(526, 0x525)],
        forms: &[],
    },
    Static {
        species: &[
            (1068, 0x171),
            (1068, 0x1b4),
            (1068, 0x1c5),
            (1080, 0x171),
            (1080, 0x1b4),
            (1080, 0x1c5),
        ],
        levels: &[(1068, 0x1c7), (1080, 0x1c7)],
        forms: &[],
    },
    Static {
        species: &[(652, 0x5c6), (652, 0x5e9)],
        levels: &[(652, 0x5eb)],
        forms: &[],
    },
    Static {
        species: &[(1102, 0x592), (1102, 0x5b5)],
        levels: &[(1102, 0x5b7)],
        forms: &[],
    },
    Static {
        species: &[(364, 0xe), (364, 0x32), (364, 0x40)],
        levels: &[(364, 0x34), (364, 0x42)],
        forms: &[],
    },
    Static {
        species: &[(1030, 0x290), (1030, 0x2a1)],
        levels: &[(1030, 0x2a3)],
        forms: &[],
    },
    Static {
        species: &[(480, 0xe1), (480, 0x10a), (480, 0x131), (480, 0x15a)],
        levels: &[(480, 0x10c), (480, 0x15c)],
        forms: &[],
    },
    Static {
        species: &[(1168, 0x2c), (1168, 0x4f)],
        levels: &[(1168, 0x51)],
        forms: &[],
    },
    Static {
        species: &[(988, 0x382)],
        levels: &[(988, 0x386)],
        forms: &[],
    },
    Static {
        species: &[(664, 0x3b5), (664, 0x3e2), (664, 0x40f), (664, 0x43c)],
        levels: &[(664, 0x3b9), (664, 0x3e6), (664, 0x413), (664, 0x440)],
        forms: &[(664, 0x3b7), (664, 0x3e4), (664, 0x411), (664, 0x43e)],
    },
    Static {
        species: &[(880, 0xab4), (880, 0xac7)],
        levels: &[(880, 0xab8)],
        forms: &[],
    },
    Static {
        species: &[(880, 0xad3), (880, 0xae6)],
        levels: &[(880, 0xad7)],
        forms: &[],
    },
    Static {
        species: &[(54, 0xdd)],
        levels: &[],
        forms: &[],
    },
    Static {
        species: &[(526, 0x27e)],
        levels: &[(526, 0x282)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x5e0)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x5ff)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x61e)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x63d)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x65c)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x67b)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x69a)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x6b9)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(1253, 0x6d8)],
        levels: &[(1253, 0x3d6)],
        forms: &[],
    },
    Static {
        species: &[(208, 0x5a6)],
        levels: &[(208, 0x5a8)],
        forms: &[],
    },
];

/// Pokémon that look like an item on the ground, with their levels in the map files.
pub const FAKE_BALLS: [Static; 2] = [
    Static {
        species: &[(1273, 0x45)],
        levels: &[
            (500, 0x46e),
            (500, 0x492),
            (500, 0x4b6),
            (506, 0x42a),
            (506, 0x44e),
        ],
        forms: &[],
    },
    Static {
        species: &[(1273, 0xc7)],
        levels: &[
            (534, 0x2f2),
            (534, 0x316),
            (562, 0x3fe),
            (562, 0x422),
            (563, 0x742),
            (563, 0x766),
            (563, 0x78a),
        ],
        forms: &[],
    },
];

/// The trainers a player meets on the way through the story, as first
/// and last numbers of each run of them.
pub const STORY_TRAINERS: [(usize, usize); 31] = [
    (4, 6),
    (133, 139),
    (147, 160),
    (164, 165),
    (169, 182),
    (203, 235),
    (237, 240),
    (242, 245),
    (247, 250),
    (252, 367),
    (372, 377),
    (381, 392),
    (426, 455),
    (461, 486),
    (497, 503),
    (510, 524),
    (537, 570),
    (580, 581),
    (583, 587),
    (592, 615),
    (621, 631),
    (657, 673),
    (679, 683),
    (690, 692),
    (703, 705),
    (712, 738),
    (745, 752),
    (754, 756),
    (763, 776),
    (786, 789),
    (797, 812),
];

/// The trainers that belong together and share a type when trainers are
/// themed, by the place each has among all trainers, counted from 1. A
/// later group takes a trainer from an earlier one; the part of a name
/// from the "-" on is not part of the group.
pub const THEME_GROUPS: [(&str, &[usize]); 32] = [
    ("GYM1", &[0xab, 0xac]),
    ("GYM2", &[0xb2, 0xb3]),
    ("GYM3", &[0x2de, 0x2df, 0x2e0, 0x2e1]),
    ("GYM4", &[0x26d, 0x94, 0xcf, 0xd0, 0xd1]),
    ("GYM5", &[0x13f, 0x140, 0x141, 0x142, 0x143, 0x144, 0x145]),
    ("GYM6", &[0x95, 0x96, 0x97, 0x98, 0x14c]),
    ("GYM7", &[0x17d, 0x17e, 0x17f, 0x180, 0x181]),
    ("GYM8", &[0x15e, 0x15f, 0x160, 0x161, 0x162, 0x163]),
    ("GYM1-LEADER", &[0x9c, 0x2fc]),
    ("GYM2-LEADER", &[0x9d, 0x2fd]),
    ("GYM3-LEADER", &[0x9a, 0x2fe]),
    ("GYM4-LEADER", &[0x99, 0x2ff]),
    ("GYM5-LEADER", &[0x9e, 0x300]),
    ("GYM6-LEADER", &[0x9b, 0x301]),
    ("GYM7-LEADER", &[0x9f, 0x302]),
    ("GYM8-LEADER", &[0xa0, 0x303]),
    ("ELITE1", &[0x26, 0x304, 0x8f, 0x309]),
    ("ELITE2", &[0x28, 0x305, 0x91, 0x30a]),
    ("ELITE3", &[0x29, 0x307, 0x92, 0x30c]),
    ("ELITE4", &[0x27, 0x306, 0x90, 0x30b]),
    ("CHAMPION", &[0x155, 0x308, 0x218, 0x30d]),
    // The battles with the rival and the other hero, three trainers
    // each, belong to no group.
    (
        "",
        &[
            0xa1, 0xa2, 0xa3, 0xa6, 0xa7, 0xa8, 0x24c, 0x24d, 0x24e, 0x170, 0x171, 0x172, 0x17a,
            0x17b, 0x17c, 0x2bd, 0x2be, 0x2bf, 0x31a, 0x31b, 0x31c, 0x2ac, 0x2ad, 0x2ae, 0x2b5,
            0x2b6, 0x2b7, 0x2b8, 0x2b9, 0x2ba, 0x168, 0x169, 0x16a, 0x16b, 0x16c, 0x16d,
        ],
    ),
    ("GYM1", &[0x173, 0x278, 0x32e]),
    ("GYM9-LEADER", &[0x1f0]),
    ("GYM10-LEADER", &[0x1ee]),
    ("GYM11-LEADER", &[0x1ef]),
    ("THEMED:ZINZOLIN-STRONG", &[0x2c0, 0x248, 0x15b, 0x1f1]),
    ("THEMED:COLRESS-STRONG", &[0x166, 0x158, 0x32d, 0x32f]),
    ("THEMED:SHADOW1", &[0x247, 0x15c, 0x2af]),
    ("THEMED:SHADOW2", &[0x1f2, 0x2b0]),
    ("THEMED:SHADOW3", &[0x1f3, 0x2b1]),
    // The strongest trainers, each of a type of their own.
    (
        "UBER",
        &[
            0x246, 0x1c8, 0xca, 0xc9, 0x5, 0x6, 0x30e, 0x30f, 0x310, 0x311, 0x159, 0x8c, 0x24f,
        ],
    ),
];
