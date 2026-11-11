//! State of one run, independent of the network layer.

use soulsync_core::protocol::{LogEntry, PlayerState, RunSettings, RunState, MAX_PLAYERS};
use soulsync_core::{diff, links, SaveSnapshot};
use std::collections::VecDeque;

/// Log entries kept per run.
const LOG_LIMIT: usize = 200;

#[derive(Debug)]
struct Player {
    name: String,
    connections: usize,
    snapshot: Option<SaveSnapshot>,
    updated_at: Option<u64>,
}

#[derive(Debug)]
pub struct Run {
    pub code: String,
    pub seed: u64,
    pub settings: RunSettings,
    players: Vec<Player>,
    log: VecDeque<LogEntry>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum JoinError {
    Full,
    BadName,
}

impl std::fmt::Display for JoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JoinError::Full => write!(f, "this run already has {MAX_PLAYERS} players"),
            JoinError::BadName => write!(f, "player names must be 1 to 24 characters"),
        }
    }
}

impl Run {
    pub fn new(code: String, seed: u64, settings: RunSettings) -> Self {
        Run {
            code,
            seed,
            settings,
            players: Vec::new(),
            log: VecDeque::new(),
        }
    }

    fn player_mut(&mut self, name: &str) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.name == name)
    }

    /// Registers a connection for `name`. A known name reconnects to its seat.
    pub fn join(&mut self, name: &str) -> Result<(), JoinError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 24 {
            return Err(JoinError::BadName);
        }
        if let Some(p) = self.player_mut(name) {
            p.connections += 1;
            return Ok(());
        }
        if self.players.len() >= MAX_PLAYERS {
            return Err(JoinError::Full);
        }
        self.players.push(Player {
            name: name.to_owned(),
            connections: 1,
            snapshot: None,
            updated_at: None,
        });
        Ok(())
    }

    pub fn leave(&mut self, name: &str) {
        if let Some(p) = self.player_mut(name.trim()) {
            p.connections = p.connections.saturating_sub(1);
        }
    }

    /// Stores a new snapshot and logs what changed. The first snapshot of a
    /// player is stored without events.
    pub fn update(&mut self, name: &str, snapshot: SaveSnapshot, now: u64) -> usize {
        let name = name.trim();
        let Some(p) = self.player_mut(name) else {
            return 0;
        };
        let events = p
            .snapshot
            .as_ref()
            .map(|old| diff(old, &snapshot))
            .unwrap_or_default();
        p.snapshot = Some(snapshot);
        p.updated_at = Some(now);
        let n = events.len();
        for event in events {
            self.log.push_front(LogEntry {
                at: now,
                player: name.to_owned(),
                event,
            });
        }
        self.log.truncate(LOG_LIMIT);
        n
    }

    pub fn state(&self) -> RunState {
        let snaps: Vec<Option<&SaveSnapshot>> =
            self.players.iter().map(|p| p.snapshot.as_ref()).collect();
        RunState {
            code: self.code.clone(),
            seed: self.seed,
            settings: self.settings.clone(),
            players: self
                .players
                .iter()
                .map(|p| PlayerState {
                    name: p.name.clone(),
                    connected: p.connections > 0,
                    snapshot: p.snapshot.clone(),
                    updated_at: p.updated_at,
                })
                .collect(),
            links: links(&snaps),
            log: self.log.iter().cloned().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soulsync_core::pk5::{build_raw, parse, BattleStats, Place};
    use soulsync_core::{Event, Trainer};

    fn snap(hp: u16) -> SaveSnapshot {
        let b = BattleStats {
            level: 5,
            hp,
            max_hp: 20,
        };
        let p = parse(
            &build_raw(1, 375, "", 117, 5, Some(&b)),
            Place::Party { slot: 0 },
        )
        .unwrap();
        SaveSnapshot {
            trainer: Trainer {
                name: "x".into(),
                badges: 0,
            },
            party: vec![p],
            boxes: vec![],
        }
    }

    #[test]
    fn seats_and_reconnects() {
        let mut r = Run::new("ZORUA-1234".into(), 7, RunSettings::default());
        assert_eq!(r.join(" "), Err(JoinError::BadName));
        r.join("Hannes").unwrap();
        r.join("Buddy").unwrap();
        assert_eq!(r.join("Third"), Err(JoinError::Full));
        r.join("Hannes").unwrap();
        r.leave("Hannes");
        assert!(r.state().players[0].connected);
        r.leave("Hannes");
        assert!(!r.state().players[0].connected);
    }

    #[test]
    fn updates_log_events_and_links() {
        let mut r = Run::new("ZORUA-1234".into(), 7, RunSettings::default());
        r.join("Hannes").unwrap();
        r.join("Buddy").unwrap();
        assert_eq!(r.update("Hannes", snap(20), 1), 0);
        assert_eq!(r.update("Hannes", snap(0), 2), 1);
        assert_eq!(r.update("Nobody", snap(0), 3), 0);
        let s = r.state();
        assert!(matches!(s.log[0].event, Event::Fainted { .. }));
        assert_eq!(s.log[0].player, "Hannes");
        assert_eq!(s.links.len(), 1);
        assert_eq!(s.links[0].sides.len(), 2);
        assert!(s.links[0].sides[1].is_none());
        assert_eq!(s.players[0].updated_at, Some(2));
    }
}
