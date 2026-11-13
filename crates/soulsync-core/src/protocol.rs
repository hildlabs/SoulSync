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

/// Most players a run accepts. Soul Link is usually played in pairs; a run
/// says how many seats it has in [`RunSettings::players`].
pub const MAX_PLAYERS: usize = 4;

/// Sent by a player's client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// The player's current save.
    Snapshot { snapshot: SaveSnapshot },
    /// Catch rates read from the player's ROM, by National Dex number
    /// (index 0 is species 1).
    CatchRates { rates: Vec<u8> },
    /// Moves, abilities, items and learnsets from the player's ROM, which a
    /// randomizer makes different for every player.
    Dex { dex: Box<crate::rom::RomDex> },
}

/// Sent by the server.
// One state per message: boxing it would buy nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    State { state: RunState },
    Error { message: String },
}

/// Settings chosen when a run is created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSettings {
    /// How many players the run has seats for: 1 (a classic Nuzlocke) to
    /// [`MAX_PLAYERS`].
    #[serde(default = "two")]
    pub players: usize,
    /// Level cap before each badge: `caps[n]` applies while a player has `n`
    /// badges. Empty means no caps.
    #[serde(default)]
    pub caps: Vec<u8>,
    /// Ids of the house rules the players agreed on, for the dashboard to
    /// show. Empty means the dashboard's default set.
    #[serde(default)]
    pub rules: Vec<String>,
    /// All players randomize with the same seed and so get the same world.
    /// Usually each player has their own.
    #[serde(default)]
    pub shared_seed: bool,
    /// The randomizer settings every player's game is made with, so that
    /// joining a run builds the same kind of game. Without them each player
    /// randomizes as they like.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub randomizer: Option<RunRandomizer>,
}

/// How the games of a run are made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRandomizer {
    /// What the creator called the settings, e.g. the preset's name.
    pub name: String,
    /// The `.rnqs` file, Base64, with every option already applied.
    pub preset: String,
    /// Rare Candies each player gets in the bag; 0 for none.
    #[serde(default)]
    pub candies: u16,
}

impl RunRandomizer {
    /// The settings file to give the randomizer.
    pub fn file(&self) -> Option<Vec<u8>> {
        let file = crate::rnqs::decode(&self.preset)?;
        crate::rnqs::Preset::read(&file).ok()?;
        Some(file)
    }
}

fn two() -> usize {
    2
}

impl Default for RunSettings {
    fn default() -> Self {
        Self {
            players: 2,
            caps: Vec::new(),
            rules: Vec::new(),
            shared_seed: false,
            randomizer: None,
        }
    }
}

impl RunSettings {
    /// Checks limits before a server accepts the settings.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(1..=MAX_PLAYERS).contains(&self.players) {
            return Err("a run has 1 to 4 players");
        }
        if self.caps.len() > 16 || self.caps.iter().any(|&c| c == 0 || c > 100) {
            return Err("caps must be 1 to 100, at most 16 of them");
        }
        let id = |r: &String| {
            !r.is_empty() && r.len() <= 24 && r.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        };
        if self.rules.len() > 32 || !self.rules.iter().all(id) {
            return Err("rules must be short lowercase ids, at most 32 of them");
        }
        if let Some(r) = &self.randomizer {
            if r.name.chars().count() > 48 || r.preset.len() > 2048 || r.file().is_none() {
                return Err("the randomizer settings are not a valid .rnqs");
            }
            if r.candies > crate::bag::MAX_COUNT {
                return Err("at most 999 Rare Candies");
            }
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

/// The randomizer seed of the player in seat `seat` (from 0). With a shared
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
    /// When this attempt began: the first save anyone sent in it, in Unix
    /// milliseconds.
    #[serde(default)]
    pub started_at: Option<u64>,
    /// Real time played in this attempt, in milliseconds: the time between
    /// updates from the players, leaving out breaks. Not the game's own
    /// clock, which emulator speed-up would distort.
    #[serde(default)]
    pub played_ms: u64,
    /// Set once the players ended this attempt.
    #[serde(default)]
    pub finished: Option<Outcome>,
    /// Attempts before this one, oldest first, and this one once finished.
    #[serde(default)]
    pub history: Vec<AttemptRecord>,
}

/// How an attempt ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The players beat the game, or reached the goal they set themselves.
    Won,
    /// The run is over: wiped, or given up.
    Lost,
}

/// What is kept of an attempt once it is over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptRecord {
    pub attempt: u32,
    pub outcome: Outcome,
    /// Unix milliseconds; `None` if nobody sent a save in it.
    pub started_at: Option<u64>,
    pub ended_at: u64,
    pub played_ms: u64,
    /// Links with every side caught and none lost.
    pub linked: u16,
    /// Links lost to a faint or a missed catch.
    pub lost: u16,
    /// In join order.
    pub players: Vec<AttemptPlayer>,
}

/// One player at the end of an attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptPlayer {
    pub name: String,
    pub badges: u8,
    /// The party as it was at the end.
    pub team: Vec<AttemptMon>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptMon {
    pub species: u16,
    #[serde(default)]
    pub species_name: Option<crate::lang::Names>,
    /// Empty without a nickname.
    pub nickname: String,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    pub name: String,
    pub connected: bool,
    pub snapshot: Option<SaveSnapshot>,
    /// Unix time in milliseconds of the last snapshot.
    pub updated_at: Option<u64>,
    /// Whether the player's ROM supplied a dex, served at
    /// `/api/runs/<code>/dex?player=<name>`.
    #[serde(default)]
    pub dex: bool,
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
    fn runs_have_one_to_four_players() {
        let old: RunSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(old.players, 2);
        for (players, ok) in [(0, false), (1, true), (2, true), (4, true), (5, false)] {
            let s = RunSettings {
                players,
                ..RunSettings::default()
            };
            assert_eq!(s.validate().is_ok(), ok);
        }
    }

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
