//! State of one run, independent of the network layer.

use serde::{Deserialize, Serialize};
use soulsync_core::protocol::{
    player_seed, AttemptMon, AttemptPlayer, AttemptRecord, LogEntry, Outcome, PlayerState,
    RunSettings, RunState, MAX_PLAYERS,
};
use soulsync_core::{diff, links, Event, RomDex, SaveSnapshot};
use std::collections::{HashSet, VecDeque};

/// Log entries kept per run.
const LOG_LIMIT: usize = 200;
/// Finished attempts kept per run.
const HISTORY_LIMIT: usize = 50;
/// A gap between two updates longer than this is a break and does not count
/// as time played.
const BREAK_MS: u64 = 10 * 60 * 1000;
/// Species in generation 5.
pub const SPECIES: usize = 649;

#[derive(Debug, Serialize, Deserialize)]
struct Player {
    name: String,
    /// Open connections; none after a restart of the server.
    #[serde(skip)]
    connections: usize,
    snapshot: Option<SaveSnapshot>,
    updated_at: Option<u64>,
    /// PIDs of the wild encounters already logged. A client that restarts
    /// sends its save before its encounters, which must not log them again.
    #[serde(default)]
    met: HashSet<u32>,
    /// Moves, abilities, items and learnsets of this player's ROM.
    #[serde(default)]
    dex: Option<Box<RomDex>>,
}

/// One run. Stored as JSON by the server, so fields added later need a
/// `#[serde(default)]` to keep older files readable.
#[derive(Debug, Serialize, Deserialize)]
pub struct Run {
    pub code: String,
    pub seed: u64,
    pub settings: RunSettings,
    pub attempt: u32,
    /// Catch rates from a player's ROM, by National Dex number minus one.
    catch_rates: Option<Vec<u8>>,
    players: Vec<Player>,
    log: VecDeque<LogEntry>,
    /// When the run last changed, in milliseconds since 1970; a run that
    /// stays unchanged long enough is removed. 0 for runs stored before
    /// this was kept.
    #[serde(default)]
    pub(crate) touched: u64,
    /// The first update of this attempt.
    #[serde(default)]
    started_at: Option<u64>,
    /// Time played in this attempt; see [`RunState::played_ms`].
    #[serde(default)]
    played_ms: u64,
    /// The last update of this attempt, to tell play from breaks.
    #[serde(default)]
    active_at: Option<u64>,
    #[serde(default)]
    finished: Option<Outcome>,
    #[serde(default)]
    history: Vec<AttemptRecord>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum JoinError {
    Full,
    BadName,
}

impl std::fmt::Display for JoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JoinError::Full => write!(f, "this run already has all its players"),
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
            touched: 0,
            started_at: None,
            played_ms: 0,
            active_at: None,
            finished: None,
            history: Vec::new(),
        }
    }

    /// Ends this attempt as `outcome` and keeps a record of it. Ending it
    /// again changes the outcome of that record.
    pub fn finish(&mut self, outcome: Outcome, now: u64) {
        if self.finished.is_some() {
            if let Some(last) = self.history.last_mut() {
                last.outcome = outcome;
            }
        } else {
            let record = self.record(outcome, now);
            self.history.push(record);
            let extra = self.history.len().saturating_sub(HISTORY_LIMIT);
            self.history.drain(..extra);
        }
        self.finished = Some(outcome);
    }

    /// What is kept of this attempt.
    fn record(&self, outcome: Outcome, now: u64) -> AttemptRecord {
        let snaps: Vec<Option<&SaveSnapshot>> =
            self.players.iter().map(|p| p.snapshot.as_ref()).collect();
        let all = links(&snaps);
        let count = |f: &dyn Fn(&soulsync_core::Link) -> bool| {
            all.iter()
                .filter(|l| f(l))
                .count()
                .min(usize::from(u16::MAX)) as u16
        };
        AttemptRecord {
            attempt: self.attempt,
            outcome,
            started_at: self.started_at,
            ended_at: now,
            played_ms: self.played_ms,
            linked: count(&|l| l.is_complete() && !l.is_dead()),
            lost: count(&|l| l.is_dead()),
            players: self
                .players
                .iter()
                .map(|p| AttemptPlayer {
                    name: p.name.clone(),
                    badges: p.snapshot.as_ref().map_or(0, |s| s.trainer.badges),
                    team: p.snapshot.as_ref().map_or_else(Vec::new, |s| {
                        s.party
                            .iter()
                            .map(|m| AttemptMon {
                                species: m.species,
                                species_name: m.species_name.clone(),
                                nickname: if m.is_nicknamed {
                                    m.nickname.clone()
                                } else {
                                    String::new()
                                },
                                level: m.battle.as_ref().map_or(m.met_level, |b| b.level),
                            })
                            .collect()
                    }),
                })
                .collect(),
        }
    }

    /// Starts over: counts a new attempt and forgets saves and log. Players
    /// keep their seats. An attempt nobody ended counts as lost, if anyone
    /// played in it.
    pub fn restart(&mut self, now: u64) {
        if self.finished.is_none() && self.started_at.is_some() {
            self.finish(Outcome::Lost, now);
        }
        self.finished = None;
        self.started_at = None;
        self.played_ms = 0;
        self.active_at = None;
        self.attempt += 1;
        self.log.clear();
        for p in &mut self.players {
            p.snapshot = None;
            p.updated_at = None;
            p.met.clear();
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

    /// Takes the dex of a player's ROM. Returns whether it changed anything.
    pub fn set_dex(&mut self, name: &str, dex: Box<RomDex>) -> bool {
        let Some(p) = self.player_mut(name.trim()) else {
            return false;
        };
        if p.dex.as_ref() == Some(&dex) {
            return false;
        }
        p.dex = Some(dex);
        true
    }

    /// The dex of a player's ROM, if it sent one.
    pub fn dex(&self, name: &str) -> Option<&RomDex> {
        self.players
            .iter()
            .find(|p| p.name == name.trim())
            .and_then(|p| p.dex.as_deref())
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
        if self.players.len() >= self.settings.players.clamp(2, MAX_PLAYERS) {
            return Err(JoinError::Full);
        }
        self.players.push(Player {
            name: name.to_owned(),
            connections: 1,
            snapshot: None,
            updated_at: None,
            met: HashSet::new(),
            dex: None,
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
        let mut events = p
            .snapshot
            .as_ref()
            .map(|old| diff(old, &snapshot))
            .unwrap_or_default();
        if p.snapshot.is_none() {
            p.met.extend(snapshot.encounters.iter().map(|w| w.pid));
        }
        let met = &mut p.met;
        events.retain(|e| match e {
            Event::Encountered { pokemon } => met.insert(pokemon.pid),
            _ => true,
        });
        p.snapshot = Some(snapshot);
        p.updated_at = Some(now);
        if self.finished.is_none() {
            self.started_at.get_or_insert(now);
            if let Some(last) = self.active_at {
                let gap = now.saturating_sub(last);
                if gap <= BREAK_MS {
                    self.played_ms += gap;
                }
            }
            self.active_at = Some(now);
        }
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
            seeds: (0..self.settings.players.clamp(2, MAX_PLAYERS))
                .map(|seat| player_seed(self.seed, seat, self.settings.shared_seed))
                .collect(),
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
                    dex: p.dex.is_some(),
                })
                .collect(),
            links: links(&snaps),
            log: self.log.iter().cloned().collect(),
            started_at: self.started_at,
            played_ms: self.played_ms,
            finished: self.finished,
            history: self.history.clone(),
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
        // A second Pokémon: with the starter alone a faint would not count.
        let boxed = parse(
            &build_raw(2, 504, "", 120, 3, None),
            Place::Box { r#box: 0, slot: 0 },
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
            boxes: vec![boxed],
            encounters: vec![],
            fallen: vec![],
            wild: None,
            balls: None,
        }
    }

    #[test]
    fn each_player_keeps_the_dex_of_their_own_rom() {
        let mut r = Run::new("ZORUA-1234".into(), 7, RunSettings::default());
        r.join("Hannes").unwrap();
        r.join("Alex").unwrap();
        let dex = RomDex {
            learnsets: vec![vec![(1, 33)]],
            ..RomDex::default()
        };
        assert!(r.set_dex("Hannes", Box::new(dex.clone())));
        assert!(!r.set_dex("Hannes", Box::new(dex.clone())));
        assert!(!r.set_dex("Nobody", Box::new(dex.clone())));
        assert_eq!(r.dex("Hannes"), Some(&dex));
        assert!(r.dex("Alex").is_none());
        let s = r.state();
        assert!(s.players[0].dex && !s.players[1].dex);
    }

    #[test]
    fn restart_counts_attempts() {
        let mut r = Run::new("ZORUA-1234".into(), 7, RunSettings::default());
        r.join("Hannes").unwrap();
        r.update("Hannes", snap(20), 1);
        r.restart(2);
        let s = r.state();
        assert_eq!(s.attempt, 2);
        assert_eq!(s.players.len(), 1);
        assert!(s.players[0].snapshot.is_none() && s.links.is_empty() && s.log.is_empty());
    }

    #[test]
    fn play_time_leaves_out_breaks_and_attempts_go_into_history() {
        const MIN: u64 = 60 * 1000;
        let mut r = Run::new("ZORUA-1234".into(), 7, RunSettings::default());
        r.join("Hannes").unwrap();
        r.join("Mika").unwrap();
        r.update("Hannes", snap(20), 1000);
        r.update("Mika", snap(20), 1000 + 2 * MIN);
        r.update("Hannes", snap(19), 1000 + 5 * MIN);
        // Lunch.
        r.update("Hannes", snap(18), 1000 + 65 * MIN);
        r.update("Mika", snap(18), 1000 + 66 * MIN);
        let s = r.state();
        assert_eq!(s.started_at, Some(1000));
        assert_eq!(s.played_ms, 6 * MIN);

        r.finish(Outcome::Won, 1000 + 70 * MIN);
        r.update("Mika", snap(17), 1000 + 71 * MIN);
        let s = r.state();
        assert_eq!(s.finished, Some(Outcome::Won));
        assert_eq!(s.played_ms, 6 * MIN, "a finished attempt's clock stops");
        assert_eq!(s.history.len(), 1);
        let h = &s.history[0];
        assert_eq!(
            (h.attempt, h.outcome, h.played_ms),
            (1, Outcome::Won, 6 * MIN)
        );
        assert_eq!(h.players.len(), 2);
        assert_eq!(h.players[0].name, "Hannes");
        assert_eq!(h.players[0].team.len(), 1);
        assert_eq!(h.players[0].team[0].species, 375);
        assert_eq!(h.players[0].team[0].level, 5);

        // Changing one's mind changes the record, without a second one.
        r.finish(Outcome::Lost, 1000 + 72 * MIN);
        assert_eq!(r.state().history.len(), 1);
        assert_eq!(r.state().history[0].outcome, Outcome::Lost);

        // Starting over after an ended attempt keeps no second record of it.
        r.restart(1000 + 80 * MIN);
        let s = r.state();
        assert_eq!((s.attempt, s.history.len(), s.played_ms), (2, 1, 0));
        assert_eq!((s.finished, s.started_at), (None, None));

        // An attempt nobody ended counts as lost; one nobody played in leaves no trace.
        r.update("Hannes", snap(20), 1000 + 90 * MIN);
        r.restart(1000 + 95 * MIN);
        r.restart(1000 + 96 * MIN);
        let s = r.state();
        assert_eq!(s.attempt, 4);
        assert_eq!(s.history.len(), 2);
        assert_eq!(
            (s.history[1].attempt, s.history[1].outcome),
            (2, Outcome::Lost)
        );
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
        r.restart(0);
        assert!(r.state().rom_data);
    }

    #[test]
    fn a_run_seats_as_many_as_it_was_made_for() {
        let settings = RunSettings {
            players: 3,
            ..RunSettings::default()
        };
        let mut r = Run::new("ZORUA-1234".into(), 7, settings);
        for name in ["Hannes", "Buddy", "Third"] {
            r.join(name).unwrap();
        }
        assert_eq!(r.join("Fourth"), Err(JoinError::Full));
        let s = r.state();
        assert_eq!(s.seeds.len(), 3);
        assert!(s.seeds[0] != s.seeds[1] && s.seeds[1] != s.seeds[2]);
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
        assert_eq!(s.links.len(), 2);
        assert_eq!(s.links[0].sides.len(), 2);
        assert!(s.links[0].sides[1].is_none());
        assert_eq!(s.players[0].updated_at, Some(2));
    }

    #[test]
    fn a_restarted_client_does_not_log_its_encounters_again() {
        let wild = |pid| {
            parse(
                &build_raw(pid, 485, "", 32, 30, None),
                Place::Box { r#box: 0, slot: 0 },
            )
            .unwrap()
        };
        let with = |pids: &[u32]| {
            let mut s = snap(20);
            s.encounters = pids.iter().map(|&p| wild(p)).collect();
            s
        };
        let mut r = Run::new("ZORUA-1234".into(), 7, RunSettings::default());
        r.join("Hannes").unwrap();
        r.update("Hannes", snap(20), 1);
        assert_eq!(r.update("Hannes", with(&[100]), 2), 1);
        // The app restarts: first the save alone, then the encounters it kept.
        assert_eq!(r.update("Hannes", snap(20), 3), 0);
        assert_eq!(r.update("Hannes", with(&[100]), 4), 0);
        assert_eq!(r.update("Hannes", with(&[100, 101]), 5), 1);
        let met = r
            .state()
            .log
            .iter()
            .filter(|e| matches!(e.event, Event::Encountered { .. }))
            .count();
        assert_eq!(met, 2);
    }
}
