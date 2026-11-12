//! Core of SoulSync: generation 5 data model and save parser.
//!
//! SoulSync reads game state from the outside (save files now, DeSmuME Lua
//! later). Of a ROM it reads the header, to tell which game it is, and the
//! species data a run needs; the ROM itself never leaves the player's machine.

pub mod art;
pub mod bag;
pub mod crypto;
pub mod dex;
pub mod events;
pub mod game;
pub mod lang;
pub mod links;
pub mod live;
pub mod pk5;
pub mod protocol;
pub mod rnqs;
pub mod rom;
pub mod save;
pub mod text;

pub use events::{diff, Event};
pub use game::{read_rom_header, Game, RomInfo, ROM_HEADER_LEN};
pub use lang::{Lang, Names};
pub use links::{links, Link, Side, Status};
pub use live::{Encounters, LiveReport};
pub use pk5::{Ailment, BattleStats, MoveSlot, Place, Pokemon, Stats};
pub use rom::{read_catch_rates, read_dex, read_level_caps, read_map_locations, RomDex, RomError};
pub use save::{parse_b2w2, SaveError, SaveSnapshot, Trainer, PARTY_BLOCK_LEN};
