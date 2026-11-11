//! Client side of a run: creates runs and keeps a save in sync with a run
//! server. Used by the command line and by the desktop app.

use futures_util::{SinkExt, StreamExt};
use soulsync_core::protocol::{ClientMsg, NewRun, RunSettings, ServerMsg};
use soulsync_core::{diff, parse_b2w2, Encounters, Event, LiveReport, SaveSnapshot};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio_tungstenite::tungstenite::Message;

pub const DEFAULT_SERVER: &str = "http://localhost:3000";

pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// The server address without a trailing slash.
pub fn base(server: &str) -> &str {
    server.trim().trim_end_matches('/')
}

/// WebSocket address of a player's seat in a run.
pub fn ws_url(server: &str, code: &str, name: &str) -> String {
    let base = base(server);
    let ws = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        format!("ws://{base}")
    };
    let name: String = name
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect();
    format!(
        "{ws}/api/runs/{}/ws?player={name}",
        code.trim().to_ascii_uppercase()
    )
}

/// Asks a server for a new run. Blocks.
pub fn create_run(server: &str, settings: &RunSettings) -> Result<NewRun, Error> {
    Ok(ureq::post(&format!("{}/api/runs", base(server)))
        .send_json(settings)?
        .body_mut()
        .read_json()?)
}

/// Whether a server knows the run at `url`. Blocks.
pub fn exists(url: &str) -> bool {
    ureq::get(url).call().is_ok()
}

/// Reads and parses a save file.
pub fn load(path: &Path) -> Result<SaveSnapshot, Error> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(parse_b2w2(&data)?)
}

pub fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// What a [`session`] reports while it runs.
pub enum Update<'a> {
    Connected,
    /// A changed save went to the server; these are the changes.
    Sent(&'a [Event]),
}

/// Why a [`session`] ended without an error.
pub enum End {
    /// Connection dropped; reconnect.
    Lost,
    /// The server refused us; give up.
    Refused(String),
}

fn text(msg: &ClientMsg) -> Message {
    Message::Text(serde_json::to_string(msg).expect("serializable").into())
}

/// The emulator's side of a sync: the file the SoulSync Lua script writes,
/// and what was learned from it.
pub struct Live {
    /// The script's report.
    file: PathBuf,
    /// Where the encounters are kept between starts.
    store: PathBuf,
    /// Location by map id, from the ROM. Empty: encounters cannot be placed.
    locations: Vec<u16>,
    encounters: Encounters,
    /// When the save was last seen written.
    saved_at: Option<SystemTime>,
    /// Reports in a row that could not be read.
    unreadable: u8,
    /// The location the player is in, for whoever wants to show it.
    pub place: Arc<Mutex<Option<u16>>>,
}

impl Live {
    pub fn new(file: PathBuf, store: PathBuf, locations: Vec<u16>) -> Self {
        let encounters = std::fs::read(&store)
            .ok()
            .and_then(|d| serde_json::from_slice(&d).ok())
            .unwrap_or_default();
        Live {
            file,
            store,
            locations,
            encounters,
            saved_at: None,
            unreadable: 0,
            place: Arc::default(),
        }
    }

    /// What to show for the save `base`: with the party the emulator has in
    /// memory, unless the save is newer, and with the encounters so far.
    /// `None` while the script is in the middle of writing; try again.
    fn view(&mut self, base: &SaveSnapshot, save: &Path) -> Option<SaveSnapshot> {
        let mut snap = base.clone();
        let current =
            matches!((modified(&self.file), modified(save)), (Some(l), Some(s)) if l >= s);
        let data = current.then(|| std::fs::read(&self.file).ok()).flatten();
        let report = data.as_deref().and_then(LiveReport::parse);
        let live = report
            .as_ref()
            .and_then(|r| Some((r, base.with_live_party(r.party())?)));
        let mut changed = false;
        let saved_at = modified(save);
        if self.saved_at != saved_at {
            // Not on the first look: what was unsaved before a restart of the
            // app still is.
            if self.saved_at.is_some() {
                self.encounters.saved();
                changed = true;
            }
            self.saved_at = saved_at;
        }
        let is_live = live.is_some();
        if let Some((report, with_party)) = live {
            self.unreadable = 0;
            snap = with_party;
            let place = report
                .map(&base.trainer)
                .and_then(|m| self.locations.get(usize::from(m)).copied());
            *self.place.lock().unwrap() = place;
            changed |= self.encounters.observe(&snap, &report.seen, place);
            snap.wild = report.wild(&snap).map(Box::new);
        } else if current {
            // Caught mid-write, most likely. A file that stays unreadable is
            // not a report; then the save alone counts.
            self.unreadable = self.unreadable.saturating_add(1);
            if self.unreadable < 3 {
                return None;
            }
        }
        changed |= self.encounters.complete(&mut snap, is_live);
        if changed {
            if let Ok(json) = serde_json::to_vec(&self.encounters) {
                let _ = std::fs::write(&self.store, json);
            }
        }
        Some(snap)
    }
}

/// One connection to a run: sends the save at `path` now and again whenever
/// the file changes, until the connection ends.
///
/// With `live`, the party the emulator has in memory replaces the saved one
/// as soon as the Lua script reports it, so nobody has to save in-game, and
/// wild encounters are tracked.
pub async fn session(
    url: &str,
    path: &Path,
    mut live: Option<&mut Live>,
    catch_rates: Option<&[u8]>,
    last: &mut SaveSnapshot,
    on: &mut (dyn FnMut(Update) + Send),
) -> Result<End, Error> {
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await?;
    let snapshot = |snap: &SaveSnapshot| {
        text(&ClientMsg::Snapshot {
            snapshot: snap.clone(),
        })
    };
    ws.send(snapshot(last)).await?;
    if let Some(rates) = catch_rates {
        ws.send(text(&ClientMsg::CatchRates {
            rates: rates.to_vec(),
        }))
        .await?;
    }
    on(Update::Connected);
    // What the save file says, without the live party.
    let mut base = load(path).unwrap_or_else(|_| last.clone());
    let mut stamp = None;
    let mut live_stamp = None;
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let now = modified(path);
                let live_now = live.as_ref().and_then(|l| modified(&l.file));
                if now == stamp && live_now == live_stamp {
                    continue;
                }
                if now != stamp {
                    // DeSmuME does not write the file atomically.
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    match load(path) {
                        Ok(snap) => base = snap,
                        Err(_) => continue,
                    }
                    stamp = now;
                }
                let snap = match live.as_mut() {
                    Some(live) => live.view(&base, path),
                    None => Some(base.clone()),
                };
                let Some(snap) = snap else { continue };
                live_stamp = live_now;
                if snap == *last {
                    continue;
                }
                let events = diff(last, &snap);
                ws.send(snapshot(&snap)).await?;
                on(Update::Sent(&events));
                *last = snap;
            }
            msg = ws.next() => match msg {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(ServerMsg::Error { message }) = serde_json::from_str(&text) {
                        return Ok(End::Refused(message));
                    }
                }
                Some(Ok(Message::Close(_))) | None => return Ok(End::Lost),
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e.into()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_ws_urls() {
        assert_eq!(
            ws_url("https://soulsync.example/", "zorua-1234", "Hannes M."),
            "wss://soulsync.example/api/runs/ZORUA-1234/ws?player=Hannes%20M."
        );
        assert_eq!(
            ws_url("http://localhost:3000", "A-1", "Bö"),
            "ws://localhost:3000/api/runs/A-1/ws?player=B%C3%B6"
        );
    }

    #[test]
    fn live_report_counts_until_the_next_save() {
        use soulsync_core::pk5::{build_raw, build_raw_ivs, BattleStats};

        let dir = std::env::temp_dir().join(format!("soulsync-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (save, file) = (dir.join("game.dsv"), dir.join("live.bin"));
        let store = dir.join("encounters.json");
        std::fs::write(&save, vec![0u8; 0x80000]).unwrap();
        let base = load(&save).unwrap();
        // Map 1 is in location 14.
        let mut live = Live::new(file.clone(), store.clone(), vec![0, 14]);
        assert_eq!(live.view(&base, &save), Some(base.clone()));

        let stats = BattleStats {
            level: 5,
            hp: 20,
            max_hp: 20,
        };
        // The party, the trainer of the empty save behind it, and map 1.
        let mut report = b"SSL3".to_vec();
        report.resize(4 + 0x800, 0);
        report[4 + 4] = 1;
        report[4 + 8..4 + 228].copy_from_slice(&build_raw(7, 495, "", 4, 5, Some(&stats)));
        report[4 + 0x604..4 + 0x606].copy_from_slice(&[0xFF, 0xFF]);
        report[4 + 0x780] = 1;
        report.push(1);
        report.extend(build_raw_ivs(9, 504, "", 0, 0, Some(&stats), 0x0123_4567));
        std::fs::write(&file, &report).unwrap();
        let at = |file: &Path, secs| {
            let t = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
            std::fs::File::options()
                .write(true)
                .open(file)
                .unwrap()
                .set_modified(t)
                .unwrap();
        };
        at(&save, 1_000);
        at(&file, 1_005);
        let snap = live.view(&base, &save).unwrap();
        assert_eq!(snap.party.len(), 1);
        assert_eq!(snap.encounters.len(), 1);
        assert_eq!(snap.encounters[0].species, 504);
        assert_eq!(snap.encounters[0].met_location, 14);
        assert_eq!(snap.wild.as_ref().unwrap().species, 504);
        assert_eq!(*live.place.lock().unwrap(), Some(14));
        // Saved after the script last wrote: the save is the truth, the
        // encounters stay.
        at(&save, 1_010);
        let snap = live.view(&base, &save).unwrap();
        assert!(snap.party.is_empty());
        assert_eq!(snap.encounters.len(), 1);
        // They survive a restart of the app.
        let mut again = Live::new(file.clone(), store, vec![0, 14]);
        assert_eq!(again.view(&base, &save).unwrap().encounters.len(), 1);
        // A half-written report is tried again, then given up on.
        std::fs::write(&file, &report[..500]).unwrap();
        at(&file, 1_020);
        assert_eq!(live.view(&base, &save), None);
        assert_eq!(live.view(&base, &save), None);
        assert!(live.view(&base, &save).unwrap().party.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
