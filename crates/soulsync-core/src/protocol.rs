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
}

/// Sent by the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    State { state: RunState },
    Error { message: String },
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
