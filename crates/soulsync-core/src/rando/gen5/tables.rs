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
