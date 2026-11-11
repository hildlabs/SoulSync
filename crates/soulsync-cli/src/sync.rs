//! `soulsync new` and `soulsync sync`: talk to a run server.

use crate::i18n::Text;
use crate::{describe, load, modified, Res};
use futures_util::{SinkExt, StreamExt};
use soulsync_core::protocol::{ClientMsg, NewRun, ServerMsg};
use soulsync_core::{diff, Lang, SaveSnapshot};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

pub const DEFAULT_SERVER: &str = "http://localhost:3000";

fn base(server: &str) -> &str {
    server.trim_end_matches('/')
}

fn ws_url(server: &str, code: &str, name: &str) -> String {
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

pub fn new_run(server: &str, t: &Text) -> Res {
    let run: NewRun = ureq::post(&format!("{}/api/runs", base(server)))
        .send_empty()?
        .body_mut()
        .read_json()?;
    let server_arg = if base(server) == DEFAULT_SERVER {
        String::new()
    } else {
        format!(" --server {}", base(server))
    };
    println!("{}", t.run_created.replace("{code}", &run.code));
    println!(
        "{}",
        t.run_share
            .replace("{code}", &run.code)
            .replace("{server}", &server_arg)
            .replace("{dashboard}", &format!("{}/run/{}", base(server), run.code))
    );
    Ok(())
}

fn snapshot_msg(snap: &SaveSnapshot) -> Message {
    let text = serde_json::to_string(&ClientMsg::Snapshot {
        snapshot: snap.clone(),
    })
    .expect("serializable");
    Message::Text(text.into())
}

enum End {
    /// Connection dropped; reconnect.
    Lost,
    /// The server refused us; give up.
    Refused(String),
}

pub fn sync(
    path: PathBuf,
    server: &str,
    code: &str,
    name: &str,
    lang: Lang,
    t: &'static Text,
) -> Res {
    let url = ws_url(server, code, name);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        let mut last = load(&path)?;
        let mut backoff = 1u64;
        loop {
            eprintln!("{}", t.connecting.replace("{url}", &url));
            match session(&url, &path, name, &mut last, lang, t).await {
                Ok(End::Refused(msg)) => return Err(format!("{}: {msg}", t.server_error).into()),
                Ok(End::Lost) => backoff = 1,
                Err(e) => eprintln!("{}: {e}", t.error),
            }
            eprintln!("{}", t.disconnected.replace("{secs}", &backoff.to_string()));
            tokio::time::sleep(Duration::from_secs(backoff)).await;
            backoff = (backoff * 2).min(30);
        }
    })
}

async fn session(
    url: &str,
    path: &Path,
    name: &str,
    last: &mut SaveSnapshot,
    lang: Lang,
    t: &Text,
) -> Result<End, Box<dyn std::error::Error>> {
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await?;
    ws.send(snapshot_msg(last)).await?;
    eprintln!(
        "{}",
        t.connected
            .replace("{name}", name)
            .replace("{path}", &path.display().to_string())
    );
    let mut stamp = modified(path);
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let now = modified(path);
                if now == stamp {
                    continue;
                }
                stamp = now;
                // DeSmuME does not write the file atomically.
                tokio::time::sleep(Duration::from_millis(300)).await;
                let Ok(snap) = load(path) else { stamp = None; continue };
                let events = diff(last, &snap);
                for e in &events {
                    println!("{}", describe(e, lang, t));
                }
                ws.send(snapshot_msg(&snap)).await?;
                eprintln!("{}", t.sent);
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
}
