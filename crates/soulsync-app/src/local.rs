//! The local API behind the dashboard's Settings tab: which ROM and save,
//! which run, and the sync task that keeps the save in step with the run.

use axum::extract::{Request, State};
use axum::http::{header, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use soulsync_client::{create_run, load, session, ws_url, End, Live, Update};
use soulsync_core::protocol::RunSettings;
use soulsync_core::{
    read_catch_rates, read_level_caps, read_map_locations, read_rom_header, Lang, ROM_HEADER_LEN,
};
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
    /// The Lua script for DeSmuME and the file it writes, once installed.
    live: Option<(PathBuf, PathBuf)>,
    /// The location the script last reported the player in.
    place: Option<Arc<Mutex<Option<u16>>>>,
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

/// The Lua script that reports the live party from DeSmuME.
const LIVE_SCRIPT: &str = include_str!("soulsync.lua");
/// Seconds after which a silent script counts as stopped.
const LIVE_TIMEOUT: u64 = 15;

fn live_script_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("soulsync.lua"))
}

/// The file the Lua script writes the party to.
fn live_party_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("live-party.bin"))
}

fn live_script(out: &Path) -> String {
    LIVE_SCRIPT.replace("@OUT@", &out.to_string_lossy())
}

/// Puts the Lua script where the player can load it from. Returns the script
/// and the file it will write.
fn install_live_script() -> Option<(PathBuf, PathBuf)> {
    let (script, out) = (live_script_path()?, live_party_path()?);
    std::fs::create_dir_all(script.parent()?).ok()?;
    let text = live_script(&out);
    if std::fs::read_to_string(&script).ok().as_deref() != Some(text.as_str()) {
        std::fs::write(&script, text).ok()?;
    }
    // Whatever an earlier session left behind says nothing about the game now.
    let _ = std::fs::remove_file(&out);
    Some((script, out))
}

/// Whether the Lua script wrote the party a moment ago.
fn live_active(out: &Path) -> bool {
    soulsync_client::modified(out)
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age.as_secs() < LIVE_TIMEOUT)
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
    // Level caps follow the gym leaders of this very ROM; the unmodified
    // game's levels stand in if they cannot be read.
    let caps = game.map(|g| {
        let read = std::fs::File::open(path)
            .ok()
            .and_then(|mut f| read_level_caps(&mut f, g).ok());
        serde_json::json!({
            "from_rom": read.is_some(),
            "levels": read.unwrap_or_else(|| g.level_caps().to_vec()),
        })
    });
    serde_json::json!({
        "caps": caps,
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
            "live": inner.live.as_ref().map(|(script, out)| serde_json::json!({
                "script": script,
                "active": live_active(out),
                "place": inner
                    .place
                    .as_ref()
                    .and_then(|p| *p.lock().unwrap())
                    .and_then(soulsync_core::dex::location),
            })),
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
        let mut rom = config.rom.and_then(|p| std::fs::File::open(p).ok());
        let rates = rom.as_mut().and_then(|f| read_catch_rates(f).ok());
        // Without them encounters cannot be placed; the party still syncs.
        let locations = rom
            .as_mut()
            .and_then(|f| read_map_locations(f).ok())
            .unwrap_or_default();
        let installed = install_live_script();
        let mut live = installed.as_ref().and_then(|(_, out)| {
            let store = data_dir()?.join("encounters.json");
            Some(Live::new(out.clone(), store, locations))
        });
        {
            let mut inner = self.inner.lock().unwrap();
            inner.place = live.as_ref().map(|l| l.place.clone());
            inner.live = installed;
        }
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
                match session(
                    &url,
                    &save,
                    live.as_mut(),
                    rates.as_deref(),
                    &mut last,
                    &mut report,
                )
                .await
                {
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

    /// Runs the Lua script against a fake emulator: a party, the trainer and
    /// the map behind it, and battles that start while it runs. Checks that
    /// the core understands what it writes.
    #[test]
    fn live_script_reports_party_place_and_battles() {
        use soulsync_core::pk5::{build_raw, build_raw_ivs, BattleStats};
        use soulsync_core::{parse_b2w2, Encounters, LiveReport, PARTY_BLOCK_LEN};

        let dir = std::env::temp_dir().join(format!("soulsync-lua-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stats = |level, hp| BattleStats {
            level,
            hp,
            max_hp: 60,
        };
        let wild =
            |pid, species| build_raw_ivs(pid, species, "", 0, 0, Some(&stats(4, 15)), 0x0123_4567);
        let mons = [
            build_raw(0x1111_2222, 488, "Mondi", 14, 5, Some(&stats(23, 0))),
            build_raw(0xFEDC_BA98, 570, "", 17, 18, Some(&stats(9, 31))),
        ];
        // 64 KiB of memory with decoys before the party at 0x02001234; the
        // rest of the 4 MiB reads as zero.
        let mut ram = vec![0u8; 0x10000];
        ram[0x104] = 3;
        ram[0x1004] = 1;
        ram[0x1008..0x1008 + 220].copy_from_slice(&mons[0]);
        ram[0x1008 + 30] ^= 1;
        let at = 0x1234;
        ram[at] = 6;
        ram[at + 4] = mons.len() as u8;
        for (i, raw) in mons.iter().enumerate() {
            ram[at + 8 + i * 220..at + 8 + (i + 1) * 220].copy_from_slice(raw);
        }
        // The trainer of an empty save (no name, id 0) and map 3.
        ram[at + 0x604..at + 0x606].copy_from_slice(&[0xFF, 0xFF]);
        ram[at + 0x780] = 3;
        // Left over from a battle before the script started.
        ram[0x8000..0x8000 + 220].copy_from_slice(&wild(50, 519));

        let out = dir.join("live.bin");
        // Lua 5.1 is what DeSmuME runs.
        let lua = mlua::Lua::new();
        let globals = lua.globals();
        globals
            .set("RAM", lua.create_string(&ram).unwrap())
            .unwrap();
        // Battles after the script has taken stock: one where the last one
        // was, one somewhere new, and a trainer's Pokémon.
        let foe = build_raw_ivs(53, 509, "", 0, 0, Some(&stats(7, 20)), 0);
        for (name, raw) in [("A", wild(51, 504)), ("B", wild(52, 506)), ("C", foe)] {
            globals.set(name, lua.create_string(&raw).unwrap()).unwrap();
        }
        let harness = r#"
            local ram, later = RAM, {}
            local function byte(a)
              local o = a - 0x02000000
              return later[o] or (o >= 0 and o < #ram and ram:byte(o + 1)) or 0
            end
            local function put(o, data)
              for i = 1, #data do later[o + i - 1] = data:byte(i) end
            end
            memory = {
              readbyte = byte,
              readword = function(a) return byte(a) + byte(a + 1) * 256 end,
              readdword = function(a)
                return byte(a) + byte(a + 1) * 256 + byte(a + 2) * 65536 + byte(a + 3) * 16777216
              end,
            }
            local frames = 0
            emu = { frameadvance = function()
              frames = frames + 1
              if frames == 300 then put(0x8000, A) put(0xC000, B) put(0xD000, C) end
              if frames > 700 then error("enough frames") end
            end }
            print = function() end
        "#;
        lua.load(harness).exec().unwrap();
        let stopped = lua.load(live_script(&out)).exec().unwrap_err();
        assert!(stopped.to_string().contains("enough frames"), "{stopped}");

        let data = std::fs::read(&out).unwrap();
        let report = LiveReport::parse(&data).unwrap();
        assert_eq!(report.party(), &ram[at..at + PARTY_BLOCK_LEN]);
        let save = parse_b2w2(&vec![0u8; 0x80000]).unwrap();
        let live = save.with_live_party(report.party()).unwrap();
        assert_eq!(live.party.len(), 2);
        assert!(live.party[0].is_fainted());
        assert_eq!(live.party[1].pid, 0xFEDC_BA98);
        assert_eq!(report.map(&save.trainer), Some(3));
        // The leftover is not reported, the three new ones are.
        let pids: Vec<u8> = report.seen.iter().map(|raw| raw[0]).collect();
        assert_eq!(pids, [51, 52, 53]);
        let mut encounters = Encounters::default();
        assert!(encounters.observe(&live, &report.seen, Some(20)));
        let list = encounters.of(&save.trainer);
        assert_eq!(list.len(), 1);
        assert_eq!((list[0].species, list[0].met_location), (504, 20));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
