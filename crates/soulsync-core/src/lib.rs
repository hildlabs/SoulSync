//! Kern von SoulSync: Gen-5-Datenmodell und Spielstand-Parser.
//!
//! SoulSync liest den Spielzustand von außen (Spielstand-Datei, später live
//! per DeSmuME-Lua) und fasst nie eine ROM an.

pub mod crypto;
pub mod dex;
pub mod events;
pub mod pk5;
pub mod save;

pub use events::{diff, Event};
pub use pk5::{BattleStats, Place, Pokemon};
pub use save::{parse_b2w2, SaveError, SaveSnapshot, Trainer};
