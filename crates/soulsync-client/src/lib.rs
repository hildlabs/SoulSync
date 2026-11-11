//! Client side of a run: creates runs and keeps a save in sync with a run
//! server. Used by the command line and by the desktop app.

use futures_util::{SinkExt, StreamExt};
use soulsync_core::protocol::{ClientMsg, NewRun, RunSettings, ServerMsg};
use soulsync_core::{diff, parse_b2w2, Event, SaveSnapshot};
use std::path::Path;
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

/// What to show for the save `base`: with the party DeSmuME has in memory,
/// as the SoulSync Lua script wrote it to `live`, unless the save is newer.
/// `None` while the script is in the middle of writing; try again.
fn live_view(base: &SaveSnapshot, save: &Path, live: &Path) -> Option<SaveSnapshot> {
    match (modified(live), modified(save)) {
        (Some(l), Some(s)) if l >= s => base.with_live_party(&std::fs::read(live).ok()?),
        _ => Some(base.clone()),
    }
}

/// One connection to a run: sends the save at `path` now and again whenever
/// the file changes, until the connection ends.
///
/// With `live`, the party the emulator has in memory replaces the saved one
/// as soon as the Lua script reports it, so nobody has to save in-game.
pub async fn session(
    url: &str,
    path: &Path,
    live: Option<&Path>,
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
                let live_now = live.and_then(modified);
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
                let snap = match live {
                    Some(live) => live_view(&base, path, live),
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
    fn live_party_counts_until_the_next_save() {
        use soulsync_core::pk5::{build_raw, BattleStats};
        use soulsync_core::PARTY_BLOCK_LEN;

        let dir = std::env::temp_dir().join(format!("soulsync-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (save, live) = (dir.join("game.dsv"), dir.join("live-party.bin"));
        std::fs::write(&save, vec![0u8; 0x80000]).unwrap();
        let base = load(&save).unwrap();
        assert_eq!(live_view(&base, &save, &live), Some(base.clone()));

        let stats = BattleStats {
            level: 5,
            hp: 20,
            max_hp: 20,
        };
        let mut block = vec![0u8; PARTY_BLOCK_LEN];
        block[4] = 1;
        block[8..228].copy_from_slice(&build_raw(7, 495, "", 14, 5, Some(&stats)));
        std::fs::write(&live, &block).unwrap();
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
        at(&live, 1_005);
        assert_eq!(live_view(&base, &save, &live).unwrap().party.len(), 1);
        // Saved after the script last wrote: the save is the truth.
        at(&save, 1_010);
        assert_eq!(live_view(&base, &save, &live), Some(base.clone()));
        // A half-written file is ignored.
        std::fs::write(&live, &block[..500]).unwrap();
        at(&live, 1_020);
        assert_eq!(live_view(&base, &save, &live), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
