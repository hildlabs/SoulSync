//! The SoulSync run server: hosts runs, relays save snapshots between players
//! and serves the live dashboard.
//!
//! Runs live in memory. With a data directory (see [`AppState::with_store`])
//! they are also written there as one JSON file each, a few seconds after
//! they change, and read back on the next start.

mod limits;
mod run;

pub use limits::Limits;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::Extension;
use axum::{Json, Router};
use rand::Rng;
use run::Run;
use serde::Deserialize;
use soulsync_core::protocol::{ClientMsg, NewRun, RunSettings, ServerMsg};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

/// The dashboard page.
pub const INDEX: &str = include_str!("../web/index.html");
const MAX_MESSAGE: usize = 4 * 1024 * 1024;

struct Entry {
    run: Run,
    tx: broadcast::Sender<String>,
    /// Changed since it was last written to the store.
    dirty: bool,
}

impl Entry {
    fn new(run: Run) -> Self {
        let (tx, _) = broadcast::channel(64);
        Entry {
            run,
            tx,
            dirty: true,
        }
    }
}

/// Where the dashboard loads sprites from. `{id}` is the National Dex number.
/// SoulSync ships no game graphics; set `SOULSYNC_SPRITES=off` to show none.
const DEFAULT_SPRITES: &str =
    "https://raw.githubusercontent.com/PokeAPI/sprites/master/sprites/pokemon/versions/generation-v/black-white/{id}.png";

#[derive(Clone, Default)]
pub struct AppState {
    runs: Arc<Mutex<HashMap<String, Entry>>>,
    sprites: Option<String>,
    /// Where runs are kept across restarts, if anywhere.
    store: Option<Arc<PathBuf>>,
    limits: Arc<Limits>,
}

impl AppState {
    /// Sends the run's state to everyone watching it and marks it for the
    /// store: every change to a run ends here.
    fn broadcast(&self, code: &str) {
        let mut runs = self.runs.lock().unwrap();
        if let Some(e) = runs.get_mut(code) {
            e.dirty = true;
            e.run.touched = now_ms();
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
        .route("/api/presets", get(presets))
        .route("/api/runs/{code}", get(get_run))
        .route("/api/runs/{code}/species", get(species))
        .route("/api/runs/{code}/dex", get(dex))
        .route("/api/runs/{code}/attempt", post(new_attempt))
        .route("/api/runs/{code}/candies", post(set_candies))
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
        "Jersey10.woff2" => include_bytes!("../web/fonts/Jersey10.woff2"),
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

/// The address of the player behind a request, where the server knows it.
type Peer = Option<Extension<ConnectInfo<SocketAddr>>>;

async fn create_run(
    State(app): State<AppState>,
    peer: Peer,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let ip = app.limits.client(peer.map(|p| p.0 .0), &headers);
    let max = app.limits.max_runs;
    if max > 0 && app.runs.lock().unwrap().len() >= max {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "this server has as many runs as it keeps; try again later or use another server",
        )
            .into_response();
    }
    if !app.limits.may_create(ip, now_ms()) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            "too many new runs from your address; try again in an hour",
        )
            .into_response();
    }
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

/// Randomizer settings a run can be made with, shipped with SoulSync. They
/// are settings only; every player randomizes their own ROM with them.
/// Id, name in English and German, description in English and German, file.
type Preset = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static [u8],
);

const PRESETS: &[Preset] = &[(
    "soul-link",
    "Soul Link",
    "Soul Link",
    "Random starters and wild Pokémon by area, random trainers, items and shops, \
     easier evolutions; types, stats and abilities as in the game.",
    "Zufällige Starter und wilde Pokémon pro Gebiet, zufällige Trainer, Items und \
     Läden, leichtere Entwicklungen; Typen, Werte und Fähigkeiten wie im Spiel.",
    include_bytes!("../presets/soul-link.rnqs"),
)];

async fn presets() -> Json<serde_json::Value> {
    use soulsync_core::rnqs::{encode, Preset};
    let list: Vec<serde_json::Value> = PRESETS
        .iter()
        .filter_map(|(id, en, de, about_en, about_de, file)| {
            let p = Preset::read(file).ok()?;
            Some(serde_json::json!({
                "id": id,
                "name": { "en": en, "de": de },
                "about": { "en": about_en, "de": about_de },
                "preset": encode(file),
                "tm": p.tm(),
                "tutor": p.tutor(),
                "full_hm": p.full_hm(),
                "random_tm_moves": p.random_tm_moves(),
            }))
        })
        .collect();
    Json(serde_json::json!(list))
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

#[derive(Deserialize)]
struct DexQuery {
    player: String,
}

/// Moves, abilities, items and learnsets of one player's ROM.
async fn dex(
    State(app): State<AppState>,
    Path(code): Path<String>,
    Query(q): Query<DexQuery>,
) -> Response {
    let runs = app.runs.lock().unwrap();
    let Some(e) = runs.get(&normalize(&code)) else {
        return (StatusCode::NOT_FOUND, "run not found").into_response();
    };
    match e.run.dex(&q.player) {
        Some(dex) => Json(dex).into_response(),
        None => (StatusCode::NOT_FOUND, "no dex for this player").into_response(),
    }
}

#[derive(Deserialize)]
struct AttemptQuery {
    /// The attempt the player is leaving. When every player of a run
    /// starts over by themselves, only the first one counts.
    from: Option<u32>,
}

/// The players wiped and start over under the same code.
async fn new_attempt(
    State(app): State<AppState>,
    Path(code): Path<String>,
    Query(q): Query<AttemptQuery>,
) -> Response {
    let code = normalize(&code);
    let attempt = {
        let mut runs = app.runs.lock().unwrap();
        let Some(e) = runs.get_mut(&code) else {
            return (StatusCode::NOT_FOUND, "run not found").into_response();
        };
        if q.from.map_or(true, |from| from == e.run.attempt) {
            e.run.restart();
        }
        e.run.attempt
    };
    tracing::info!(%code, attempt, "new attempt");
    app.broadcast(&code);
    Json(serde_json::json!({ "attempt": attempt })).into_response()
}

#[derive(Deserialize)]
struct CandiesQuery {
    n: u16,
}

/// Changes how many Rare Candies the run's new games start with; from the
/// next game each player makes on. A query, not a body, so that a page
/// of another origin can send it without a preflight.
async fn set_candies(
    State(app): State<AppState>,
    Path(code): Path<String>,
    Query(q): Query<CandiesQuery>,
) -> Response {
    let code = normalize(&code);
    let candies = q.n.min(soulsync_core::bag::MAX_COUNT);
    {
        let mut runs = app.runs.lock().unwrap();
        let Some(e) = runs.get_mut(&code) else {
            return (StatusCode::NOT_FOUND, "run not found").into_response();
        };
        let Some(r) = e.run.settings.randomizer.as_mut() else {
            return (StatusCode::CONFLICT, "run has no randomizer settings").into_response();
        };
        r.candies = candies;
    }
    tracing::info!(%code, candies, "rare candies");
    app.broadcast(&code);
    Json(serde_json::json!({ "candies": candies })).into_response()
}

#[derive(Deserialize)]
struct WsQuery {
    player: Option<String>,
}

async fn ws(
    State(app): State<AppState>,
    Path(code): Path<String>,
    Query(q): Query<WsQuery>,
    peer: Peer,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let code = normalize(&code);
    if !app.runs.lock().unwrap().contains_key(&code) {
        return (StatusCode::NOT_FOUND, "run not found").into_response();
    }
    let ip = app.limits.client(peer.map(|p| p.0 .0), &headers);
    let Some(connection) = app.limits.connect(ip) else {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            "too many connections from your address",
        )
            .into_response();
    };
    upgrade
        .max_message_size(MAX_MESSAGE)
        .on_upgrade(move |socket| async move {
            session(app, code, q.player, socket).await;
            drop(connection);
        })
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
                    Ok(ClientMsg::Dex { dex }) => {
                        let changed = {
                            let mut runs = app.runs.lock().unwrap();
                            runs.get_mut(&code).is_some_and(|e| e.run.set_dex(&name, dex))
                        };
                        if changed {
                            tracing::info!(%code, player = %name, "dex from ROM");
                            app.broadcast(&code);
                        }
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

    /// Applies limits for a public server; see [`Limits::from_env`].
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = Arc::new(limits);
        self
    }

    /// Removes runs nobody changed for as long as the limits allow, unless
    /// someone is watching them right now, from memory and from the store.
    /// Returns how many went.
    pub fn prune(&self, now: u64) -> usize {
        let gone: Vec<String> = {
            let mut runs = self.runs.lock().unwrap();
            let idle: Vec<String> = runs
                .iter()
                .filter(|(_, e)| {
                    e.tx.receiver_count() == 0 && self.limits.is_idle(e.run.touched, now)
                })
                .map(|(code, _)| code.clone())
                .collect();
            for code in &idle {
                runs.remove(code);
            }
            idle
        };
        if let Some(dir) = &self.store {
            for code in &gone {
                let _ = std::fs::remove_file(dir.join(format!("{code}.json")));
            }
        }
        for code in &gone {
            tracing::info!(%code, "idle run removed");
        }
        gone.len()
    }

    /// Reads `SOULSYNC_SPRITES`: a URL with `{id}`, or `off`.
    pub fn from_env() -> Self {
        Self::new(match std::env::var("SOULSYNC_SPRITES") {
            Ok(v) if v.eq_ignore_ascii_case("off") || v.is_empty() => None,
            Ok(v) => Some(v),
            Err(_) => Some(DEFAULT_SPRITES.to_owned()),
        })
    }

    /// Keeps runs in `dir`: reads the runs stored there now and writes
    /// changed ones with [`AppState::save`]. Files that do not read as a run
    /// are left alone and logged.
    pub fn with_store(mut self, dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        let mut runs = self.runs.lock().unwrap();
        for file in std::fs::read_dir(&dir)? {
            let path = file?.path();
            if path.extension().map_or(true, |e| e != "json") {
                continue;
            }
            let run = std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| serde_json::from_slice::<Run>(&b).map_err(|e| e.to_string()));
            match run {
                Ok(mut run) => {
                    // Stored before runs kept the time: counted from now.
                    if run.touched == 0 {
                        run.touched = now_ms();
                    }
                    let mut entry = Entry::new(run);
                    entry.dirty = false;
                    runs.insert(entry.run.code.clone(), entry);
                }
                Err(e) => tracing::warn!(file = %path.display(), "not a run: {e}"),
            }
        }
        tracing::info!(runs = runs.len(), dir = %dir.display(), "runs loaded");
        drop(runs);
        self.store = Some(Arc::new(dir));
        Ok(self)
    }

    /// Writes the runs that changed since the last call to the store, if
    /// there is one. Returns how many were written.
    pub fn save(&self) -> usize {
        let Some(dir) = &self.store else { return 0 };
        let changed: Vec<(String, Vec<u8>)> = {
            let mut runs = self.runs.lock().unwrap();
            runs.values_mut()
                .filter(|e| e.dirty)
                .filter_map(|e| {
                    e.dirty = false;
                    Some((e.run.code.clone(), serde_json::to_vec(&e.run).ok()?))
                })
                .collect()
        };
        let mut written = 0;
        for (code, json) in changed {
            match write_atomic(dir, &code, &json) {
                Ok(()) => written += 1,
                Err(e) => {
                    tracing::error!(%code, "could not store the run: {e}");
                    if let Some(e) = self.runs.lock().unwrap().get_mut(&code) {
                        e.dirty = true;
                    }
                }
            }
        }
        written
    }

    pub fn has_run(&self, code: &str) -> bool {
        self.runs.lock().unwrap().contains_key(&normalize(code))
    }

    /// Creates a run and returns its code and seed.
    pub fn create(&self, settings: RunSettings) -> Result<NewRun, &'static str> {
        settings.validate()?;
        let mut runs = self.runs.lock().unwrap();
        let code = new_code(&runs);
        // 53 bits: exact as a JavaScript number.
        let seed = rand::rng().random::<u64>() >> 11;
        let mut run = Run::new(code.clone(), seed, settings);
        run.touched = now_ms();
        runs.insert(code.clone(), Entry::new(run));
        tracing::info!(%code, "run created");
        Ok(NewRun { code, seed })
    }
}

/// Writes `json` as `dir/<code>.json` so that a crash leaves either the old
/// file or the new one, never half of one.
fn write_atomic(dir: &FsPath, code: &str, json: &[u8]) -> std::io::Result<()> {
    // Codes are made by `new_code`: letters, digits and one dash.
    if !code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(std::io::Error::other("unexpected run code"));
    }
    let tmp = dir.join(format!("{code}.json.tmp"));
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, dir.join(format!("{code}.json")))
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
        assert_eq!(s.links.len(), 2);
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

    #[tokio::test]
    async fn rare_candies_of_a_run_can_change() {
        let state = AppState::default();
        let settings = RunSettings {
            randomizer: Some(soulsync_core::protocol::RunRandomizer {
                name: "Soul Link".into(),
                preset: soulsync_core::rnqs::encode(PRESETS[0].5),
                candies: 187,
            }),
            ..RunSettings::default()
        };
        let run = state.create(settings).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let served = state.clone();
        tokio::spawn(async move { axum::serve(listener, app(served)).await.unwrap() });
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let post = |path: String| async move {
            let mut tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
            let req = format!(
                "POST {path} HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            tcp.write_all(req.as_bytes()).await.unwrap();
            let mut resp = String::new();
            tcp.read_to_string(&mut resp).await.unwrap();
            resp
        };
        let resp = post(format!("/api/runs/{}/candies?n=5000", run.code)).await;
        assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
        let candies = |s: &AppState| {
            let runs = s.runs.lock().unwrap();
            runs[&run.code]
                .run
                .settings
                .randomizer
                .as_ref()
                .unwrap()
                .candies
        };
        assert_eq!(candies(&state), 999);
        let resp = post("/api/runs/NOPE-0000/candies?n=1".into()).await;
        assert!(resp.starts_with("HTTP/1.1 404"), "{resp}");
    }

    #[test]
    fn runs_outlive_a_restart_with_a_store() {
        let dir = std::env::temp_dir().join(format!("soulsync-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = AppState::default().with_store(&dir).unwrap();
        let code = first.create(RunSettings::default()).unwrap().code;
        {
            let mut runs = first.runs.lock().unwrap();
            let run = &mut runs.get_mut(&code).unwrap().run;
            run.join("Hannes").unwrap();
            run.update("Hannes", snap(20), 1);
            run.update("Hannes", snap(0), 2);
        }
        first.broadcast(&code);
        assert_eq!(first.save(), 1);
        assert_eq!(first.save(), 0, "nothing changed since");
        std::fs::write(dir.join("broken.json"), b"{").unwrap();

        let second = AppState::default().with_store(&dir).unwrap();
        let state = second.runs.lock().unwrap().get(&code).unwrap().run.state();
        assert_eq!(state.players[0].name, "Hannes");
        assert!(!state.players[0].connected);
        assert_eq!(state.log.len(), 1);
        assert!(state.players[0].snapshot.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_public_server_limits_new_runs() {
        let state = AppState::default().with_limits(Limits {
            runs_per_hour: 2,
            max_runs: 3,
            ..Limits::default()
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let service = app(state.clone()).into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move { axum::serve(listener, service).await.unwrap() });
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let post = || async move {
            let mut tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
            let req = "POST /api/runs HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            tcp.write_all(req.as_bytes()).await.unwrap();
            let mut resp = String::new();
            tcp.read_to_string(&mut resp).await.unwrap();
            resp
        };
        for _ in 0..2 {
            let resp = post().await;
            assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
        }
        let resp = post().await;
        assert!(resp.starts_with("HTTP/1.1 429"), "{resp}");
        // Someone else filled the server.
        state.create(RunSettings::default()).unwrap();
        let resp = post().await;
        assert!(resp.starts_with("HTTP/1.1 503"), "{resp}");
    }

    #[test]
    fn idle_runs_go_unless_someone_watches() {
        let dir = std::env::temp_dir().join(format!("soulsync-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let state = AppState::default()
            .with_limits(Limits {
                idle_days: 1,
                ..Limits::default()
            })
            .with_store(&dir)
            .unwrap();
        let idle = state.create(RunSettings::default()).unwrap().code;
        let watched = state.create(RunSettings::default()).unwrap().code;
        assert_eq!(state.save(), 2);
        let _rx = state.runs.lock().unwrap()[&watched].tx.subscribe();
        let now = now_ms();
        assert_eq!(state.prune(now), 0, "not idle yet");
        assert_eq!(state.prune(now + 2 * 24 * 60 * 60 * 1000), 1);
        let runs = state.runs.lock().unwrap();
        assert!(!runs.contains_key(&idle) && runs.contains_key(&watched));
        assert!(!dir.join(format!("{idle}.json")).exists());
        assert!(dir.join(format!("{watched}.json")).exists());
        drop(runs);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod preset_tests {
    #[test]
    fn shipped_presets_are_valid() {
        for p in super::PRESETS {
            assert!(soulsync_core::rnqs::Preset::read(p.5).is_ok(), "{}", p.0);
        }
    }
}
