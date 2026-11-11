//! Core of SoulSync: generation 5 data model and save parser.
//!
//! SoulSync reads game state from the outside (save files now, DeSmuME Lua
//! later) and never touches a ROM.

pub mod crypto;
pub mod dex;
pub mod events;
pub mod lang;
pub mod links;
pub mod pk5;
pub mod protocol;
pub mod save;

pub use events::{diff, Event};
pub use lang::{Lang, Names};
pub use links::{links, Link, Side, Status};
pub use pk5::{BattleStats, Place, Pokemon};
pub use save::{parse_b2w2, SaveError, SaveSnapshot, Trainer};
