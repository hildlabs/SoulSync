//! `soulsync-server`: hosts runs, relays save snapshots between players and
//! serves the live dashboard.
//!
//! Runs live in memory and are lost on restart.

mod run;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rand::Rng;
use run::Run;
use serde::Deserialize;
use soulsync_core::protocol::{ClientMsg, NewRun, ServerMsg};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

const INDEX: &str = include_str!("../web/index.html");
const MAX_MESSAGE: usize = 4 * 1024 * 1024;

struct Entry {
    run: Run,
    tx: broadcast::Sender<String>,
}

#[derive(Clone, Default)]
pub struct AppState {
    runs: Arc<Mutex<HashMap<String, Entry>>>,
}

impl AppState {
    fn broadcast(&self, code: &str) {
        let runs = self.runs.lock().unwrap();
        if let Some(e) = runs.get(code) {
            let _ = e.tx.send(state_json(&e.run));
        }
    }
}

fn state_json(run: &Run) -> String {
    serde_json::to_string(&ServerMsg::State { state: run.state() }).expect("serializable")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn new_code(taken: &HashMap<String, Entry>) -> String {
    let words: Vec<String> = (1..=649u16)
        .filter_map(soulsync_core::dex::species)
        .map(|s| s.name.en.to_ascii_uppercase())
        .filter(|n| n.len() <= 8 && n.chars().all(|c| c.is_ascii_alphabetic()))
        .collect();
    let mut rng = rand::rng();
    loop {
        let word = &words[rng.random_range(0..words.len())];
        let code = format!("{word}-{:04}", rng.random_range(0..10_000));
        if !taken.contains_key(&code) {
            return code;
        }
    }
}

fn normalize(code: &str) -> String {
    code.trim().to_ascii_uppercase()
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/run/{code}", get(index))
        .route("/api/runs", post(create_run))
        .route("/api/runs/{code}", get(get_run))
        .route("/api/runs/{code}/ws", get(ws))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(INDEX)
}

async fn create_run(State(app): State<AppState>) -> Json<NewRun> {
    let mut runs = app.runs.lock().unwrap();
    let code = new_code(&runs);
    let seed = rand::rng().random();
    let (tx, _) = broadcast::channel(64);
    runs.insert(
        code.clone(),
        Entry {
            run: Run::new(code.clone(), seed),
            tx,
        },
    );
    tracing::info!(%code, "run created");
    Json(NewRun { code, seed })
}

async fn get_run(State(app): State<AppState>, Path(code): Path<String>) -> Response {
    let runs = app.runs.lock().unwrap();
    match runs.get(&normalize(&code)) {
        Some(e) => Json(e.run.state()).into_response(),
        None => (StatusCode::NOT_FOUND, "run not found").into_response(),
    }
}

#[derive(Deserialize)]
struct WsQuery {
    player: Option<String>,
}

async fn ws(
    State(app): State<AppState>,
    Path(code): Path<String>,
    Query(q): Query<WsQuery>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let code = normalize(&code);
    if !app.runs.lock().unwrap().contains_key(&code) {
        return (StatusCode::NOT_FOUND, "run not found").into_response();
    }
    upgrade
        .max_message_size(MAX_MESSAGE)
        .on_upgrade(move |socket| session(app, code, q.player, socket))
}

async fn send(socket: &mut WebSocket, msg: &ServerMsg) -> bool {
    let text = serde_json::to_string(msg).expect("serializable");
    socket.send(Message::Text(text.into())).await.is_ok()
}

/// One WebSocket connection: a player (with `player`) or a read-only viewer.
async fn session(app: AppState, code: String, player: Option<String>, mut socket: WebSocket) {
    let joined = {
        let mut runs = app.runs.lock().unwrap();
        let Some(e) = runs.get_mut(&code) else { return };
        let joined = match &player {
            Some(name) => e.run.join(name).map(|()| true),
            None => Ok(false),
        };
        joined.map(|j| (j, e.tx.subscribe(), state_json(&e.run)))
    };
    let (is_player, mut rx, initial) = match joined {
        Ok(v) => v,
        Err(err) => {
            send(
                &mut socket,
                &ServerMsg::Error {
                    message: err.to_string(),
                },
            )
            .await;
            return;
        }
    };
    let name = player.unwrap_or_default();
    if is_player {
        tracing::info!(%code, player = %name, "player connected");
        app.broadcast(&code);
    } else if socket.send(Message::Text(initial.into())).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(Ok(msg)) = incoming else { break };
                let Message::Text(text) = msg else { continue };
                if !is_player {
                    continue;
                }
                match serde_json::from_str::<ClientMsg>(&text) {
                    Ok(ClientMsg::Snapshot { snapshot }) => {
                        let n = {
                            let mut runs = app.runs.lock().unwrap();
                            runs.get_mut(&code).map_or(0, |e| e.run.update(&name, snapshot, now_ms()))
                        };
                        tracing::info!(%code, player = %name, events = n, "snapshot");
                        app.broadcast(&code);
                    }
                    Err(e) => {
                        let message = format!("invalid message: {e}");
                        if !send(&mut socket, &ServerMsg::Error { message }).await {
                            break;
                        }
                    }
                }
            }
            out = rx.recv() => match out {
                Ok(text) => {
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            },
        }
    }

    if is_player {
        if let Some(e) = app.runs.lock().unwrap().get_mut(&code) {
            e.run.leave(&name);
        }
        tracing::info!(%code, player = %name, "player disconnected");
        app.broadcast(&code);
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "soulsync_server=info".into()),
        )
        .init();
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    tracing::info!("SoulSync server on http://localhost:{port}");
    axum::serve(listener, app(AppState::default()))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("server");
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use soulsync_core::pk5::{build_raw, parse, BattleStats, Place};
    use soulsync_core::protocol::RunState;
    use soulsync_core::{SaveSnapshot, Trainer};
    use tokio_tungstenite::tungstenite::Message as WsMsg;

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

    async fn next_state<S>(ws: &mut S) -> RunState
    where
        S: StreamExt<Item = Result<WsMsg, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        loop {
            let msg = ws.next().await.unwrap().unwrap();
            if let WsMsg::Text(t) = msg {
                match serde_json::from_str::<ServerMsg>(&t).unwrap() {
                    ServerMsg::State { state } => return state,
                    ServerMsg::Error { message } => panic!("{message}"),
                }
            }
        }
    }

    #[tokio::test]
    async fn end_to_end() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app(AppState::default()))
                .await
                .unwrap()
        });

        // Create a run over plain HTTP.
        let mut tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        tcp.write_all(
            b"POST /api/runs HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();
        let mut resp = String::new();
        tcp.read_to_string(&mut resp).await.unwrap();
        let body = resp.split("\r\n\r\n").nth(1).unwrap();
        let run: NewRun = serde_json::from_str(body).unwrap();
        assert!(run.code.contains('-'));

        let url = |q: &str| format!("ws://{addr}/api/runs/{}/ws{q}", run.code.to_lowercase());
        let (mut viewer, _) = tokio_tungstenite::connect_async(url("")).await.unwrap();
        assert!(next_state(&mut viewer).await.players.is_empty());

        let (mut a, _) = tokio_tungstenite::connect_async(url("?player=Hannes"))
            .await
            .unwrap();
        assert_eq!(next_state(&mut viewer).await.players[0].name, "Hannes");

        let send = |s: SaveSnapshot| {
            WsMsg::Text(
                serde_json::to_string(&ClientMsg::Snapshot { snapshot: s })
                    .unwrap()
                    .into(),
            )
        };
        a.send(send(snap(20))).await.unwrap();
        let s = next_state(&mut viewer).await;
        assert_eq!(s.links.len(), 1);
        a.send(send(snap(0))).await.unwrap();
        let s = next_state(&mut viewer).await;
        assert_eq!(s.log.len(), 1);

        let (_b, _) = tokio_tungstenite::connect_async(url("?player=Buddy"))
            .await
            .unwrap();
        assert_eq!(next_state(&mut viewer).await.players.len(), 2);
        let (mut c, _) = tokio_tungstenite::connect_async(url("?player=Third"))
            .await
            .unwrap();
        let WsMsg::Text(t) = c.next().await.unwrap().unwrap() else {
            panic!()
        };
        assert!(matches!(
            serde_json::from_str(&t).unwrap(),
            ServerMsg::Error { .. }
        ));

        drop(a);
        let s = next_state(&mut viewer).await;
        assert!(!s.players[0].connected);
    }
}
