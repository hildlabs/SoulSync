//! Messages between SoulSync clients and the run server.
//!
//! Players connect to `/api/runs/{code}/ws?player=<name>` and send
//! [`ClientMsg::Snapshot`] whenever their save changes. Everyone connected to a
//! run, players and dashboards alike, receives [`ServerMsg::State`] after
//! every change.

use crate::events::Event;
use crate::links::Link;
use crate::save::SaveSnapshot;
use serde::{Deserialize, Serialize};

/// Most players a run accepts. Soul Link is played in pairs.
pub const MAX_PLAYERS: usize = 2;

/// Sent by a player's client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// The player's current save.
    Snapshot { snapshot: SaveSnapshot },
    /// Catch rates read from the player's ROM, by National Dex number
    /// (index 0 is species 1).
    CatchRates { rates: Vec<u8> },
}

/// Sent by the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    State { state: RunState },
    Error { message: String },
}

/// Settings chosen when a run is created.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSettings {
    /// Level cap before each badge: `caps[n]` applies while a player has `n`
    /// badges. Empty means no caps.
    #[serde(default)]
    pub caps: Vec<u8>,
}

impl RunSettings {
    /// The level cap for a player with `badges` badges, if any.
    pub fn cap(&self, badges: u8) -> Option<u8> {
        self.caps.get(badges as usize).or(self.caps.last()).copied()
    }
}

/// Response to creating a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewRun {
    pub code: String,
    /// Shared seed for the randomizer, so both players get the same world.
    pub seed: u64,
}

/// Everything a dashboard needs to show a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunState {
    pub code: String,
    pub seed: u64,
    /// Which try this is, starting at 1. Goes up when the players start over.
    pub attempt: u32,
    pub settings: RunSettings,
    /// Whether a player's ROM supplied the catch rates; otherwise they are the
    /// values of the unmodified games.
    #[serde(default)]
    pub rom_data: bool,
    /// In join order.
    pub players: Vec<PlayerState>,
    /// Soul Link pairs, sides in the same order as `players`.
    pub links: Vec<Link>,
    /// Newest first.
    pub log: Vec<LogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    pub name: String,
    pub connected: bool,
    pub snapshot: Option<SaveSnapshot>,
    /// Unix time in milliseconds of the last snapshot.
    pub updated_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unix time in milliseconds.
    pub at: u64,
    pub player: String,
    pub event: Event,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_follow_badges() {
        let s = RunSettings {
            caps: vec![13, 18, 24],
        };
        assert_eq!(s.cap(0), Some(13));
        assert_eq!(s.cap(2), Some(24));
        assert_eq!(s.cap(8), Some(24));
        assert_eq!(RunSettings::default().cap(0), None);
    }
}
