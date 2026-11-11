//! The local API behind the dashboard's Settings tab: which ROM and save,
//! which run, and the sync task that keeps the save in step with the run.

use axum::extract::{Request, State};
use axum::http::{header, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use soulsync_client::{create_run, load, session, ws_url, End, Update};
use soulsync_core::protocol::RunSettings;
use soulsync_core::{read_catch_rates, read_rom_header, Lang, ROM_HEADER_LEN};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What the app remembers between starts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Config {
    name: String,
    rom: Option<PathBuf>,
    save: Option<PathBuf>,
    /// Run server. Empty: runs live in this app, for playing on one computer.
    server: String,
    /// The run this app is in.
    code: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    #[default]
    Idle,
    Connecting,
    Live,
    Error,
}

#[derive(Default)]
struct Inner {
    config: Config,
    phase: Phase,
    /// Why the sync is not running, if it should be.
    message: Option<String>,
    task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Clone)]
struct Local {
    inner: Arc<Mutex<Inner>>,
    server: soulsync_server::AppState,
    /// Address of this app, used as run server when none is set.
    own: String,
}

/// Where the app keeps its settings.
pub fn data_dir() -> Option<PathBuf> {
    std::env::var_os("SOULSYNC_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("SoulSync")))
        .or_else(|| std::env::var_os("HOME").map(|d| PathBuf::from(d).join(".config/soulsync")))
}

fn config_path() -> Option<PathBuf> {
    Some(data_dir()?.join("config.json"))
}

fn read_config() -> Config {
    config_path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|d| serde_json::from_slice(&d).ok())
        .unwrap_or_default()
}

fn write_config(config: &Config) {
    if let Some(path) = config_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(config) {
            let _ = std::fs::write(path, json);
        }
    }
}

/// What the Settings tab shows about a ROM.
fn rom_info(path: &Path) -> serde_json::Value {
    use std::io::Read;
    let mut header = [0u8; ROM_HEADER_LEN];
    let info = std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut header))
        .ok()
        .and_then(|()| read_rom_header(&header));
    let game = info.as_ref().and_then(|i| i.game);
    serde_json::json!({
        "path": path,
        "ok": game.is_some(),
        "game": game,
        "label": game.map(|g| serde_json::json!({ "en": g.label(Lang::En), "de": g.label(Lang::De) })),
        "generation": game.map(|g| g.generation()),
    })
}

/// What the Settings tab shows about a save.
fn save_info(path: &Path) -> serde_json::Value {
    match load(path) {
        Ok(snap) => serde_json::json!({
            "path": path,
            "ok": true,
            "trainer": snap.trainer.name,
            "badges": snap.trainer.badges,
            "pokemon": snap.party.len() + snap.boxes.len(),
        }),
        Err(e) => serde_json::json!({ "path": path, "ok": false, "error": e.to_string() }),
    }
}

/// DeSmuME keeps `Battery/<rom name>.dsv` next to itself; the ROM usually
/// sits in the same folder or one below.
fn guess_save(rom: &Path) -> Option<PathBuf> {
    let stem = rom.file_stem()?;
    let dir = rom.parent()?;
    [Some(dir), dir.parent()]
        .into_iter()
        .flatten()
        .map(|d| d.join("Battery").join(stem).with_extension("dsv"))
        .find(|p| p.is_file())
}

impl Local {
    fn view(&self) -> serde_json::Value {
        let inner = self.inner.lock().unwrap();
        let c = &inner.config;
        serde_json::json!({
            "name": c.name,
            "rom": c.rom.as_deref().map(rom_info),
            "save": c.save.as_deref().map(save_info),
            "server": c.server,
            "code": c.code,
            "phase": inner.phase,
            "message": inner.message,
            "dialogs": cfg!(any(windows, target_os = "macos")),
            "version": env!("CARGO_PKG_VERSION"),
        })
    }

    fn set_phase(&self, phase: Phase, message: Option<String>) {
        let mut inner = self.inner.lock().unwrap();
        inner.phase = phase;
        inner.message = message;
    }

    fn server_url(&self) -> String {
        let server = self.inner.lock().unwrap().config.server.clone();
        if server.trim().is_empty() {
            self.own.clone()
        } else {
            server
        }
    }

    fn stop(&self) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(task) = inner.task.take() {
            task.abort();
        }
        inner.phase = Phase::Idle;
        inner.message = None;
    }

    /// Starts syncing the configured save with the configured run.
    fn start(&self) {
        self.stop();
        let config = self.inner.lock().unwrap().config.clone();
        let server = self.server_url();
        let (Some(code), Some(save)) = (config.code, config.save) else {
            return;
        };
        let url = ws_url(&server, &code, &config.name);
        let rates = config
            .rom
            .and_then(|p| std::fs::File::open(p).ok())
            .and_then(|mut f| read_catch_rates(&mut f).ok());
        let me = self.clone();
        self.set_phase(Phase::Connecting, None);
        let task = tokio::spawn(async move {
            let mut last = match load(&save) {
                Ok(s) => s,
                Err(e) => return me.set_phase(Phase::Error, Some(e.to_string())),
            };
            let mut backoff = 1u64;
            loop {
                let reporter = me.clone();
                let mut report = move |u: Update| {
                    if matches!(u, Update::Connected) {
                        reporter.set_phase(Phase::Live, None);
                    }
                };
                match session(&url, &save, rates.as_deref(), &mut last, &mut report).await {
                    Ok(End::Refused(msg)) => return me.set_phase(Phase::Error, Some(msg)),
                    Ok(End::Lost) => {
                        backoff = 1;
                        me.set_phase(Phase::Connecting, None);
                    }
                    Err(e) => me.set_phase(Phase::Connecting, Some(e.to_string())),
                }
                tokio::time::sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(30);
            }
        });
        self.inner.lock().unwrap().task = Some(task);
    }
}

type Reply = Result<Json<serde_json::Value>, (StatusCode, String)>;

fn bad(why: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, why.into())
}

async fn state(State(local): State<Local>) -> Json<serde_json::Value> {
    Json(local.view())
}

#[derive(Deserialize)]
struct Set {
    name: Option<String>,
    rom: Option<String>,
    save: Option<String>,
    server: Option<String>,
}

fn apply(config: &mut Config, set: Set) {
    let path = |s: String| {
        let s = s.trim().trim_matches('"').to_owned();
        (!s.is_empty()).then(|| PathBuf::from(s))
    };
    if let Some(name) = set.name {
        config.name = name.trim().chars().take(24).collect();
    }
    if let Some(save) = set.save {
        config.save = path(save);
    }
    if let Some(rom) = set.rom {
        config.rom = path(rom);
        if config.save.is_none() {
            config.save = config.rom.as_deref().and_then(guess_save);
        }
    }
    if let Some(server) = set.server {
        config.server = soulsync_client::base(&server).to_owned();
    }
}

async fn set(State(local): State<Local>, Json(set): Json<Set>) -> Json<serde_json::Value> {
    {
        let mut inner = local.inner.lock().unwrap();
        apply(&mut inner.config, set);
        write_config(&inner.config);
    }
    Json(local.view())
}

#[derive(Deserialize)]
struct Pick {
    /// `rom` or `save`.
    kind: String,
}

#[cfg(any(windows, target_os = "macos"))]
fn dialog(rom: bool, start: Option<PathBuf>) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new();
    d = if rom {
        d.add_filter("Nintendo DS ROM", &["nds"])
    } else {
        d.add_filter("Save", &["dsv", "sav"])
    };
    if let Some(dir) = start.as_deref().and_then(Path::parent) {
        d = d.set_directory(dir);
    }
    d.pick_file()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn dialog(_rom: bool, _start: Option<PathBuf>) -> Option<PathBuf> {
    None
}

/// Opens the system's file dialog and stores the chosen file.
async fn pick(State(local): State<Local>, Json(pick): Json<Pick>) -> Reply {
    let rom = match pick.kind.as_str() {
        "rom" => true,
        "save" => false,
        _ => return Err(bad("kind must be rom or save")),
    };
    let start = {
        let inner = local.inner.lock().unwrap();
        let c = &inner.config;
        if rom {
            c.rom.clone()
        } else {
            c.save.clone().or(c.rom.clone())
        }
    };
    let chosen = tokio::task::spawn_blocking(move || dialog(rom, start))
        .await
        .ok()
        .flatten();
    if let Some(path) = chosen {
        let path = path.to_string_lossy().into_owned();
        let mut inner = local.inner.lock().unwrap();
        let set = Set {
            name: None,
            rom: rom.then(|| path.clone()),
            save: (!rom).then_some(path),
            server: None,
        };
        apply(&mut inner.config, set);
        write_config(&inner.config);
    }
    Ok(Json(local.view()))
}

#[derive(Deserialize)]
struct Start {
    /// Join this run; without it a new run is created.
    code: Option<String>,
    #[serde(default)]
    settings: RunSettings,
}

/// Creates or joins a run and starts syncing.
async fn start(State(local): State<Local>, Json(req): Json<Start>) -> Reply {
    let config = local.inner.lock().unwrap().config.clone();
    if config.name.is_empty() {
        return Err(bad("name"));
    }
    match config.save.as_deref().map(load) {
        Some(Ok(_)) => {}
        _ => return Err(bad("save")),
    }
    let own = config.server.trim().is_empty();
    let server = local.server_url();
    let code = match req.code.map(|c| c.trim().to_ascii_uppercase()) {
        Some(code) if !code.is_empty() => {
            let url = format!("{server}/api/runs/{code}");
            let found = if own {
                local.server.has_run(&code)
            } else {
                tokio::task::spawn_blocking(move || ureq_ok(&url))
                    .await
                    .unwrap_or(false)
            };
            if !found {
                return Err(bad("code"));
            }
            code
        }
        _ if own => local.server.create(req.settings).map_err(bad)?.code,
        _ => {
            tokio::task::spawn_blocking(move || create_run(&server, &req.settings))
                .await
                .map_err(|e| bad(e.to_string()))?
                .map_err(|e| bad(format!("server: {e}")))?
                .code
        }
    };
    {
        let mut inner = local.inner.lock().unwrap();
        inner.config.code = Some(code);
        write_config(&inner.config);
    }
    local.start();
    Ok(Json(local.view()))
}

fn ureq_ok(url: &str) -> bool {
    soulsync_client::exists(url)
}

/// Leaves the run. The run itself stays on its server.
async fn leave(State(local): State<Local>) -> Json<serde_json::Value> {
    local.stop();
    {
        let mut inner = local.inner.lock().unwrap();
        inner.config.code = None;
        write_config(&inner.config);
    }
    Json(local.view())
}

/// Keeps other websites away from the local API: a browser sends their
/// requests here too. They cannot set the custom header without a preflight,
/// which this app never answers, and a rebound DNS name fails the Host check.
async fn guard(req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let local_host = host.starts_with("127.0.0.1:") || host.starts_with("localhost:");
    let marked = req.method() == Method::GET || req.headers().contains_key("x-soulsync");
    if local_host && marked {
        next.run(req).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

/// The local API, and the sync task if the app was in a run when it closed.
pub fn routes(server: soulsync_server::AppState, own: String) -> Router {
    let local = Local {
        inner: Arc::new(Mutex::new(Inner {
            config: read_config(),
            ..Inner::default()
        })),
        server,
        own,
    };
    // A run kept in this app is gone after a restart; one on a server is not.
    let resume = {
        let mut inner = local.inner.lock().unwrap();
        if inner.config.server.is_empty() {
            inner.config.code = None;
        }
        inner.config.code.is_some()
    };
    if resume {
        local.start();
    }
    Router::new()
        .route("/api/local/state", get(state))
        .route("/api/local/set", post(set))
        .route("/api/local/pick", post(pick))
        .route("/api/local/start", post(start))
        .route("/api/local/leave", post(leave))
        .layer(axum::middleware::from_fn(guard))
        .with_state(local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_desmume_save() {
        let dir = std::env::temp_dir().join(format!("soulsync-test-{}", std::process::id()));
        let battery = dir.join("Battery");
        std::fs::create_dir_all(dir.join("Roms")).unwrap();
        std::fs::create_dir_all(&battery).unwrap();
        std::fs::write(battery.join("SoulLink.dsv"), b"").unwrap();
        assert_eq!(
            guess_save(&dir.join("Roms/SoulLink.nds")),
            Some(battery.join("SoulLink.dsv"))
        );
        assert_eq!(
            guess_save(&dir.join("SoulLink.nds")),
            Some(battery.join("SoulLink.dsv"))
        );
        assert_eq!(guess_save(&dir.join("Other.nds")), None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn settings_trim_and_guess() {
        let mut c = Config::default();
        let set = Set {
            name: Some("  Hannes  ".into()),
            rom: Some("\"C:\\Games\\SoulLink.nds\"".into()),
            save: None,
            server: Some("https://soulsync.example/".into()),
        };
        apply(&mut c, set);
        assert_eq!(c.name, "Hannes");
        assert_eq!(c.rom, Some(PathBuf::from("C:\\Games\\SoulLink.nds")));
        assert_eq!(c.server, "https://soulsync.example");
    }
}
