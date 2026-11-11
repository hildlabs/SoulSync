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
    /// Ids of the house rules the players agreed on, for the dashboard to
    /// show. Empty means the dashboard's default set.
    #[serde(default)]
    pub rules: Vec<String>,
    /// Both players randomize with the same seed and so get the same world.
    /// Usually each player has their own.
    #[serde(default)]
    pub shared_seed: bool,
}

impl RunSettings {
    /// Checks limits before a server accepts the settings.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.caps.len() > 16 || self.caps.iter().any(|&c| c == 0 || c > 100) {
            return Err("caps must be 1 to 100, at most 16 of them");
        }
        let id = |r: &String| {
            !r.is_empty() && r.len() <= 24 && r.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        };
        if self.rules.len() > 32 || !self.rules.iter().all(id) {
            return Err("rules must be short lowercase ids, at most 32 of them");
        }
        Ok(())
    }

    /// The level cap for a player with `badges` badges, if any.
    pub fn cap(&self, badges: u8) -> Option<u8> {
        self.caps.get(badges as usize).or(self.caps.last()).copied()
    }
}

/// Response to creating a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewRun {
    pub code: String,
    /// The run's seed, from which each player's randomizer seed comes; see
    /// [`player_seed`].
    pub seed: u64,
}

/// The randomizer seed of the player in seat `seat` (0 or 1). With a shared
/// seed it is the run's seed for everyone. Seeds stay below 2^53 so that
/// dashboards, which read them as JavaScript numbers, show them exactly.
pub fn player_seed(run_seed: u64, seat: usize, shared: bool) -> u64 {
    if shared || seat == 0 {
        return run_seed;
    }
    // SplitMix64, one step per seat.
    let mut z = run_seed.wrapping_add(0x9E37_79B9_7F4A_7C15u64.wrapping_mul(seat as u64));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) >> 11
}

/// Everything a dashboard needs to show a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunState {
    pub code: String,
    pub seed: u64,
    /// Each seat's randomizer seed, in join order; see [`player_seed`].
    #[serde(default)]
    pub seeds: Vec<u64>,
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
            ..RunSettings::default()
        };
        assert_eq!(s.cap(0), Some(13));
        assert_eq!(s.cap(2), Some(24));
        assert_eq!(s.cap(8), Some(24));
        assert_eq!(RunSettings::default().cap(0), None);
    }

    #[test]
    fn players_get_their_own_seed_unless_shared() {
        let run = 1 << 52;
        assert_eq!(player_seed(run, 0, false), run);
        let second = player_seed(run, 1, false);
        assert_ne!(second, run);
        assert!(second < 1 << 53);
        assert_eq!(player_seed(run, 1, false), second);
        assert_eq!(player_seed(run, 1, true), run);
    }
}
