//! State of one run, independent of the network layer.

use soulsync_core::protocol::{LogEntry, PlayerState, RunSettings, RunState, MAX_PLAYERS};
use soulsync_core::{diff, links, SaveSnapshot};
use std::collections::VecDeque;

/// Log entries kept per run.
const LOG_LIMIT: usize = 200;
/// Species in generation 5.
pub const SPECIES: usize = 649;

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
    pub attempt: u32,
    /// Catch rates from a player's ROM, by National Dex number minus one.
    catch_rates: Option<Vec<u8>>,
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
            attempt: 1,
            catch_rates: None,
            players: Vec::new(),
            log: VecDeque::new(),
        }
    }

    /// Starts over: counts a new attempt and forgets saves and log. Players
    /// keep their seats.
    pub fn restart(&mut self) {
        self.attempt += 1;
        self.log.clear();
        for p in &mut self.players {
            p.snapshot = None;
            p.updated_at = None;
        }
    }

    /// Takes catch rates read from a ROM. Returns whether they changed anything.
    pub fn set_catch_rates(&mut self, rates: Vec<u8>) -> bool {
        if rates.len() != SPECIES || self.catch_rates.as_ref() == Some(&rates) {
            return false;
        }
        self.catch_rates = Some(rates);
        true
    }

    /// Catch rate of a species in this run.
    pub fn catch_rate(&self, species: u16) -> Option<u8> {
        let from_rom = self
            .catch_rates
            .as_ref()
            .and_then(|r| r.get(usize::from(species).checked_sub(1)?).copied());
        from_rom.or_else(|| soulsync_core::dex::species(species).map(|s| s.catch_rate))
    }

    pub fn has_rom_data(&self) -> bool {
        self.catch_rates.is_some()
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
            attempt: self.attempt,
            settings: self.settings.clone(),
            rom_data: self.has_rom_data(),
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
                id: 0,
                badges: 0,
                game: None,
            },
            party: vec![p],
            boxes: vec![],
            encounters: vec![],
            fallen: vec![],
        }
    }

    #[test]
    fn restart_counts_attempts() {
        let mut r = Run::new("ZORUA-1234".into(), 7, RunSettings::default());
        r.join("Hannes").unwrap();
        r.update("Hannes", snap(20), 1);
        r.restart();
        let s = r.state();
        assert_eq!(s.attempt, 2);
        assert_eq!(s.players.len(), 1);
        assert!(s.players[0].snapshot.is_none() && s.links.is_empty() && s.log.is_empty());
    }

    #[test]
    fn catch_rates_come_from_the_rom_when_given() {
        let mut r = Run::new("ZORUA-1234".into(), 7, RunSettings::default());
        assert_eq!(r.catch_rate(1), Some(45));
        assert!(!r.state().rom_data);
        assert!(!r.set_catch_rates(vec![3; 10]));
        assert!(r.set_catch_rates(vec![190; SPECIES]));
        assert!(!r.set_catch_rates(vec![190; SPECIES]));
        assert_eq!(r.catch_rate(1), Some(190));
        assert_eq!(r.catch_rate(650), None);
        r.restart();
        assert!(r.state().rom_data);
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
