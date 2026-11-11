//! The SoulSync run server: hosts runs, relays save snapshots between players
//! and serves the live dashboard.
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
use soulsync_core::protocol::{ClientMsg, NewRun, RunSettings, ServerMsg};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

/// The dashboard page.
pub const INDEX: &str = include_str!("../web/index.html");
const MAX_MESSAGE: usize = 4 * 1024 * 1024;

struct Entry {
    run: Run,
    tx: broadcast::Sender<String>,
}

/// Where the dashboard loads sprites from. `{id}` is the National Dex number.
/// SoulSync ships no game graphics; set `SOULSYNC_SPRITES=off` to show none.
const DEFAULT_SPRITES: &str =
    "https://raw.githubusercontent.com/PokeAPI/sprites/master/sprites/pokemon/versions/generation-v/black-white/{id}.png";

#[derive(Clone, Default)]
pub struct AppState {
    runs: Arc<Mutex<HashMap<String, Entry>>>,
    sprites: Option<String>,
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
        .route("/demo", get(index))
        .route("/assets/demo.js", get(demo))
        .route("/assets/icon.svg", get(icon))
        .route("/assets/fonts/{file}", get(font))
        .route("/api/config", get(config))
        .route("/api/runs", post(create_run))
        .route("/api/runs/{code}", get(get_run))
        .route("/api/runs/{code}/species", get(species))
        .route("/api/runs/{code}/attempt", post(new_attempt))
        .route("/api/runs/{code}/ws", get(ws))
        .layer(axum::middleware::map_response(allow_any_origin))
        .with_state(state)
}

/// A dashboard served by the desktop app reads runs from another server.
/// Everything here is public and takes no credentials.
async fn allow_any_origin(mut res: Response) -> Response {
    res.headers_mut().insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        axum::http::HeaderValue::from_static("*"),
    );
    res
}

async fn index() -> Html<&'static str> {
    Html(INDEX)
}

/// Scripted run for `/demo`, so the dashboard can be seen without a save.
async fn demo() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/javascript")],
        include_str!("../web/demo.js"),
    )
}

async fn icon() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
        include_str!("../../../assets/soulsync-icon.svg"),
    )
}

async fn font(Path(file): Path<String>) -> Response {
    let bytes: &'static [u8] = match file.as_str() {
        "Geist.woff2" => include_bytes!("../web/fonts/Geist.woff2"),
        "GeistMono.woff2" => include_bytes!("../web/fonts/GeistMono.woff2"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            (axum::http::header::CONTENT_TYPE, "font/woff2"),
            (
                axum::http::header::CACHE_CONTROL,
                "public, max-age=31536000, immutable",
            ),
        ],
        bytes,
    )
        .into_response()
}

async fn config(State(app): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "sprites": app.sprites }))
}

async fn create_run(State(app): State<AppState>, body: axum::body::Bytes) -> Response {
    let settings: RunSettings = if body.iter().all(u8::is_ascii_whitespace) {
        RunSettings::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(s) => s,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("invalid settings: {e}")).into_response()
            }
        }
    };
    match app.create(settings) {
        Ok(run) => Json(run).into_response(),
        Err(why) => (StatusCode::BAD_REQUEST, why).into_response(),
    }
}

async fn get_run(State(app): State<AppState>, Path(code): Path<String>) -> Response {
    let runs = app.runs.lock().unwrap();
    match runs.get(&normalize(&code)) {
        Some(e) => Json(e.run.state()).into_response(),
        None => (StatusCode::NOT_FOUND, "run not found").into_response(),
    }
}

/// Names and catch rates of all species, for the catch calculator.
async fn species(State(app): State<AppState>, Path(code): Path<String>) -> Response {
    let runs = app.runs.lock().unwrap();
    let Some(e) = runs.get(&normalize(&code)) else {
        return (StatusCode::NOT_FOUND, "run not found").into_response();
    };
    let list: Vec<_> = (1..=run::SPECIES as u16)
        .filter_map(|id| {
            let s = soulsync_core::dex::species(id)?;
            Some((&s.name.en, &s.name.de, e.run.catch_rate(id)?))
        })
        .collect();
    Json(serde_json::json!({ "from_rom": e.run.has_rom_data(), "species": list })).into_response()
}

/// The players wiped and start over under the same code.
async fn new_attempt(State(app): State<AppState>, Path(code): Path<String>) -> Response {
    let code = normalize(&code);
    let attempt = {
        let mut runs = app.runs.lock().unwrap();
        let Some(e) = runs.get_mut(&code) else {
            return (StatusCode::NOT_FOUND, "run not found").into_response();
        };
        e.run.restart();
        e.run.attempt
    };
    tracing::info!(%code, attempt, "new attempt");
    app.broadcast(&code);
    Json(serde_json::json!({ "attempt": attempt })).into_response()
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
                    Ok(ClientMsg::CatchRates { rates }) => {
                        let changed = {
                            let mut runs = app.runs.lock().unwrap();
                            runs.get_mut(&code).is_some_and(|e| e.run.set_catch_rates(rates))
                        };
                        if changed {
                            tracing::info!(%code, player = %name, "catch rates from ROM");
                            app.broadcast(&code);
                        }
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

impl AppState {
    /// State for a server whose dashboard loads sprites from `sprites`.
    pub fn new(sprites: Option<String>) -> Self {
        AppState {
            sprites,
            ..AppState::default()
        }
    }

    /// Reads `SOULSYNC_SPRITES`: a URL with `{id}`, or `off`.
    pub fn from_env() -> Self {
        Self::new(match std::env::var("SOULSYNC_SPRITES") {
            Ok(v) if v.eq_ignore_ascii_case("off") || v.is_empty() => None,
            Ok(v) => Some(v),
            Err(_) => Some(DEFAULT_SPRITES.to_owned()),
        })
    }

    pub fn has_run(&self, code: &str) -> bool {
        self.runs.lock().unwrap().contains_key(&normalize(code))
    }

    /// Creates a run and returns its code and seed.
    pub fn create(&self, settings: RunSettings) -> Result<NewRun, &'static str> {
        settings.validate()?;
        let mut runs = self.runs.lock().unwrap();
        let code = new_code(&runs);
        let seed = rand::rng().random();
        let (tx, _) = broadcast::channel(64);
        runs.insert(
            code.clone(),
            Entry {
                run: Run::new(code.clone(), seed, settings),
                tx,
            },
        );
        tracing::info!(%code, "run created");
        Ok(NewRun { code, seed })
    }
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
                game: None,
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
            b"POST /api/runs HTTP/1.1\r\nHost: x\r\nContent-Length: 16\r\nConnection: close\r\n\r\n{\"caps\":[13,18]}",
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
        let first = next_state(&mut viewer).await;
        assert!(first.players.is_empty());
        assert_eq!(first.settings.caps, vec![13, 18]);

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
