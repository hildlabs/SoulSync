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
    /// Whether DeSmuME has what it needs to run the script.
    lua: Option<LuaSetup>,
    /// Whether DeSmuME loads the script by itself.
    auto: Option<LuaSetup>,
    /// The location the script last reported the player in.
    place: Option<Arc<Mutex<Option<u16>>>>,
    /// Where the sync takes requests to forget an encounter.
    forget: Option<Arc<Mutex<Vec<u32>>>>,
    /// Whether the script reports a party that is not the save's.
    foreign: Option<Arc<Mutex<bool>>>,
    /// A newer build of the app, if one is published.
    update: Option<crate::update::Release>,
    /// How far the installation of it is, once asked for.
    progress: Option<crate::update::Progress>,
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

/// Lua 5.1 for DeSmuME, by the machine type in the header of an `.exe`:
/// 64-bit and 32-bit x86. See `lua51/README.md`.
const LUA_DLLS: [(u16, &[u8]); 2] = [
    (0x8664, include_bytes!("../lua51/x64/lua51.dll")),
    (0x014C, include_bytes!("../lua51/x86/lua51.dll")),
];

/// Whether DeSmuME can run Lua scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum LuaSetup {
    /// `lua51.dll` was already next to DeSmuME.
    Present,
    /// SoulSync put it there.
    Installed,
    /// No DeSmuME near the save, or it could not be set up there.
    Missing,
}

/// The machine type a Windows program is built for.
pub(crate) fn exe_machine(exe: &Path) -> Option<u16> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(exe).ok()?;
    let mut b = [0u8; 4];
    f.seek(SeekFrom::Start(0x3C)).ok()?;
    f.read_exact(&mut b).ok()?;
    f.seek(SeekFrom::Start(u64::from(u32::from_le_bytes(b))))
        .ok()?;
    let mut pe = [0u8; 6];
    f.read_exact(&mut pe).ok()?;
    (&pe[..4] == b"PE\0\0").then(|| u16::from_le_bytes([pe[4], pe[5]]))
}

/// The folder DeSmuME runs from and the machine type it is built for. Looks
/// where the save is: its `Battery` folder sits next to the `.exe`.
fn find_desmume(save: &Path) -> Option<(&Path, u16)> {
    save.ancestors().skip(1).take(2).find_map(|dir| {
        let mut exes: Vec<PathBuf> = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().unwrap_or_default().to_string_lossy();
                let name = name.to_ascii_lowercase();
                name.starts_with("desmume") && name.ends_with(".exe")
            })
            .collect();
        exes.sort();
        Some((dir, exes.iter().find_map(|e| exe_machine(e))?))
    })
}

/// DeSmuME needs `lua51.dll` next to its `.exe` and does not bring it. Adds
/// the DLL if it is missing.
fn install_lua(save: &Path) -> LuaSetup {
    let Some((dir, machine)) = find_desmume(save) else {
        return LuaSetup::Missing;
    };
    let dll = dir.join("lua51.dll");
    if dll.exists() {
        return LuaSetup::Present;
    }
    let ours = LUA_DLLS.iter().find(|(m, _)| *m == machine);
    match ours.map(|(_, bytes)| std::fs::write(&dll, bytes)) {
        Some(Ok(())) => LuaSetup::Installed,
        _ => LuaSetup::Missing,
    }
}

/// First line of the copy of the script that DeSmuME loads by itself, so the
/// app only ever replaces its own file.
const AUTOLOAD_MARK: &str = "-- SoulSync autoload: rewritten by the SoulSync app, do not edit.\n";

/// The value of `key` in `[section]` of an `.ini` file.
fn ini_get(text: &str, section: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[') {
            inside = name.trim_end_matches(']').eq_ignore_ascii_case(section);
        } else if inside {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// Sets `key` in `[section]` of an `.ini` file, leaving everything else.
fn ini_set(text: &str, section: &str, key: &str, value: &str) -> String {
    let entry = format!("{key}={value}");
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let header = lines.iter().position(|l| {
        let l = l.trim();
        l.starts_with('[') && l[1..].trim_end_matches(']').eq_ignore_ascii_case(section)
    });
    match header {
        Some(h) => {
            let end = lines[h + 1..]
                .iter()
                .position(|l| l.trim().starts_with('['))
                .map_or(lines.len(), |n| h + 1 + n);
            let found = (h + 1..end).find(|&i| {
                lines[i]
                    .split_once('=')
                    .is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case(key))
            });
            match found {
                Some(i) => lines[i] = entry,
                None => lines.insert(h + 1, entry),
            }
        }
        None => lines.extend([format!("[{section}]"), entry]),
    }
    lines.join("\r\n") + "\r\n"
}

/// Has DeSmuME load the script by itself whenever the game opens. DeSmuME
/// does that with `AutoLoad=1` under `[Scripting]` in its `desmume.ini`, for
/// a script named like the ROM in its Lua folder. Never touches a script of
/// that name that is not ours.
fn install_autoload(save: &Path, script: &str) -> LuaSetup {
    let done = (|| {
        let (dir, _) = find_desmume(save)?;
        let ini = dir.join("desmume.ini");
        // DeSmuME writes the file in the system's code page. Bytes are kept
        // as they are by reading each one as a character of its own.
        let bytes = std::fs::read(&ini).unwrap_or_default();
        if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
            return None;
        }
        let text: String = bytes.iter().map(|&b| char::from(b)).collect();
        let folder = ini_get(&text, "PathSettings", "Lua").unwrap_or_else(|| "Lua".into());
        if !folder.is_ascii() {
            return None;
        }
        let rooted = folder.starts_with(['\\', '/']) || folder.as_bytes().get(1) == Some(&b':');
        let folder = if rooted {
            PathBuf::from(folder)
        } else {
            folder
                .split(['\\', '/'])
                .filter(|part| !part.is_empty() && *part != ".")
                .fold(dir.to_path_buf(), |path, part| path.join(part))
        };
        let file = folder.join(save.file_stem()?).with_extension("lua");
        let script = format!("{AUTOLOAD_MARK}{script}");
        let mut changed = false;
        match std::fs::read_to_string(&file) {
            Ok(theirs) if !theirs.starts_with(AUTOLOAD_MARK) => return None,
            Ok(ours) if ours == script => {}
            _ => {
                std::fs::create_dir_all(&folder).ok()?;
                std::fs::write(&file, script).ok()?;
                changed = true;
            }
        }
        let on = ini_get(&text, "Scripting", "AutoLoad");
        if !on.is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true")) {
            let text = ini_set(&text, "Scripting", "AutoLoad", "1");
            let bytes: Vec<u8> = text.chars().map(|c| c as u8).collect();
            std::fs::write(&ini, bytes).ok()?;
            changed = true;
        }
        Some(changed)
    })();
    match done {
        Some(true) => LuaSetup::Installed,
        Some(false) => LuaSetup::Present,
        None => LuaSetup::Missing,
    }
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
                "lua": inner.lua,
                "auto": inner.auto,
                "foreign": inner.foreign.as_ref().is_some_and(|f| *f.lock().unwrap()),
                "place": inner
                    .place
                    .as_ref()
                    .and_then(|p| *p.lock().unwrap())
                    .and_then(soulsync_core::dex::location),
            })),
            "update": serde_json::json!({
                "build": crate::update::build(),
                "latest": inner.update.as_ref().map(|r| r.build),
                "progress": inner.progress,
            }),
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
            inner.forget = live.as_ref().map(|l| l.forget.clone());
            inner.foreign = live.as_ref().map(|l| l.foreign.clone());
            inner.lua = Some(install_lua(&save));
            inner.auto = installed
                .as_ref()
                .map(|(_, out)| install_autoload(&save, &live_script(out)));
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

#[derive(Deserialize)]
struct Forget {
    pid: u32,
}

/// Takes back an encounter the app noted by mistake.
async fn forget(State(local): State<Local>, Json(req): Json<Forget>) -> Reply {
    let inner = local.inner.lock().unwrap();
    let queue = inner.forget.as_ref().ok_or_else(|| bad("no run"))?;
    queue.lock().unwrap().push(req.pid);
    drop(inner);
    Ok(Json(local.view()))
}

/// Looks for a newer build once, in the background. Only the Windows program
/// is published, and only numbered builds know what is newer.
fn look_for_update(local: &Local) {
    let Some(own) = crate::update::build().filter(|_| cfg!(windows)) else {
        return;
    };
    let local = local.clone();
    std::thread::spawn(move || {
        if let Some(release) = crate::update::latest().filter(|r| r.build > own) {
            local.inner.lock().unwrap().update = Some(release);
        }
    });
}

/// Starts installing the newer build. The page follows the progress in the
/// state; when the new program is started, this one quits so that it can
/// take over.
async fn update(State(local): State<Local>) -> Reply {
    use crate::update::Progress;
    let release = {
        let mut inner = local.inner.lock().unwrap();
        let release = inner.update.clone().ok_or_else(|| bad("no update"))?;
        let busy = matches!(
            inner.progress,
            Some(Progress::Download { .. } | Progress::Start)
        );
        (!busy).then(|| {
            inner.progress = Some(Progress::Download { percent: None });
            release
        })
    };
    let Some(release) = release else {
        return Ok(Json(local.view()));
    };
    let worker = local.clone();
    std::thread::spawn(move || {
        let tell = |p: Progress| worker.inner.lock().unwrap().progress = Some(p);
        match crate::update::install(&release, &tell) {
            // A moment for the page to hear of it.
            Ok(()) => {
                std::thread::sleep(Duration::from_millis(400));
                std::process::exit(0);
            }
            Err(_) => tell(Progress::Failed),
        }
    });
    Ok(Json(local.view()))
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
    look_for_update(&local);
    Router::new()
        .route("/api/local/state", get(state))
        .route("/api/local/set", post(set))
        .route("/api/local/pick", post(pick))
        .route("/api/local/start", post(start))
        .route("/api/local/leave", post(leave))
        .route("/api/local/update", post(update))
        .route("/api/local/forget", post(forget))
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
    fn puts_lua_next_to_desmume() {
        let dir = std::env::temp_dir().join(format!("soulsync-dll-{}", std::process::id()));
        let save = dir.join("Battery").join("SoulLink.dsv");
        std::fs::create_dir_all(save.parent().unwrap()).unwrap();
        assert_eq!(install_lua(&save), LuaSetup::Missing);
        // A 32-bit program: "PE" header at 0x80, machine 0x014C.
        let mut exe = vec![0u8; 0x90];
        exe[0x3C] = 0x80;
        exe[0x80..0x86].copy_from_slice(b"PE\0\0\x4C\x01");
        std::fs::write(dir.join("DeSmuME_0.9.13_x86.exe"), &exe).unwrap();
        std::fs::write(dir.join("notes.exe"), b"MZ").unwrap();
        assert_eq!(install_lua(&save), LuaSetup::Installed);
        assert_eq!(std::fs::read(dir.join("lua51.dll")).unwrap(), LUA_DLLS[1].1);
        // Never replaces one that is there.
        std::fs::write(dir.join("lua51.dll"), b"theirs").unwrap();
        assert_eq!(install_lua(&save), LuaSetup::Present);
        assert_eq!(std::fs::read(dir.join("lua51.dll")).unwrap(), b"theirs");
        // Both DLLs are Windows libraries for the right machine.
        for (machine, bytes) in LUA_DLLS {
            let file = dir.join("check.dll");
            std::fs::write(&file, bytes).unwrap();
            assert_eq!(exe_machine(&file), Some(machine));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn has_desmume_load_the_script() {
        let dir = std::env::temp_dir().join(format!("soulsync-auto-{}", std::process::id()));
        let save = dir.join("Battery").join("Soul Link.dsv");
        std::fs::create_dir_all(save.parent().unwrap()).unwrap();
        assert_eq!(install_autoload(&save, "print(1)"), LuaSetup::Missing);
        let mut exe = vec![0u8; 0x90];
        exe[0x3C] = 0x80;
        exe[0x80..0x86].copy_from_slice(b"PE\0\0\x64\x86");
        std::fs::write(dir.join("DeSmuME_0.9.13_x64.exe"), &exe).unwrap();
        // A settings file in the system's code page, with a byte that is not UTF-8.
        let ini = dir.join("desmume.ini");
        let theirs = b"[General]\r\nName=J\xF6rg\r\n[Scripting]\r\nRecent Lua Script 1=a.lua\r\n[PathSettings]\r\nLua=.\\Scripts\\Lua\r\n";
        std::fs::write(&ini, theirs).unwrap();
        assert_eq!(install_autoload(&save, "print(1)"), LuaSetup::Installed);
        let script = dir.join("Scripts").join("Lua").join("Soul Link.lua");
        let text = std::fs::read_to_string(&script).unwrap();
        assert_eq!(text, format!("{AUTOLOAD_MARK}print(1)"));
        assert_eq!(
            std::fs::read(&ini).unwrap(),
            b"[General]\r\nName=J\xF6rg\r\n[Scripting]\r\nAutoLoad=1\r\nRecent Lua Script 1=a.lua\r\n[PathSettings]\r\nLua=.\\Scripts\\Lua\r\n"
        );
        // Nothing to do the second time; a new script replaces ours.
        assert_eq!(install_autoload(&save, "print(1)"), LuaSetup::Present);
        assert_eq!(install_autoload(&save, "print(2)"), LuaSetup::Installed);
        // A script of the player's own under that name stays.
        std::fs::write(&script, "print('mine')").unwrap();
        assert_eq!(install_autoload(&save, "print(1)"), LuaSetup::Missing);
        assert_eq!(std::fs::read_to_string(&script).unwrap(), "print('mine')");
        // Without a settings file DeSmuME uses its `Lua` folder.
        std::fs::remove_file(&ini).unwrap();
        assert_eq!(install_autoload(&save, "print(1)"), LuaSetup::Installed);
        assert!(dir.join("Lua").join("Soul Link.lua").exists());
        assert_eq!(
            std::fs::read(&ini).unwrap(),
            b"[Scripting]\r\nAutoLoad=1\r\n"
        );
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
        // An opponent's party, which is laid out like the player's and comes
        // first in memory. Only the player's is followed by their trainer.
        ram[0x404] = 1;
        ram[0x408..0x408 + 220].copy_from_slice(&wild(60, 527));
        let trainer_id = 0xCAFE_1234u32;
        // The player's ID turns up behind it, as in a battle's data, but
        // not their name.
        ram[0x404 + 0x700..0x404 + 0x704].copy_from_slice(&trainer_id.to_le_bytes());
        // The trainer "Hi" and map 3.
        ram[at + 0x604..at + 0x60A].copy_from_slice(&[b'H', 0, b'i', 0, 0xFF, 0xFF]);
        ram[at + 0x614..at + 0x618].copy_from_slice(&trainer_id.to_le_bytes());
        let mut trainer = parse_b2w2(&vec![0u8; 0x80000]).unwrap().trainer;
        trainer.id = trainer_id;
        trainer.name = "Hi".into();
        let note = soulsync_core::live::trainer_note(&trainer);
        std::fs::write(dir.join("live-trainer.txt"), note).unwrap();
        ram[at + 0x704] = 3;
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
        // B a few turns into the battle: 6 HP left, asleep. The status word
        // is at 0x88, encrypted with the rest of the battle stats.
        let mut hurt = soulsync_core::crypto::decrypt(&wild(52, 506)).unwrap();
        hurt[0x88] = 2;
        hurt[0x8E] = 6;
        let hurt = soulsync_core::crypto::encrypt(&hurt).unwrap();
        let later = [
            ("A", wild(51, 504)),
            ("B", wild(52, 506)),
            ("C", foe),
            ("B2", hurt),
        ];
        for (name, raw) in later {
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
              if frames == 650 then put(0xC000, B2) end
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
        let mut save = parse_b2w2(&vec![0u8; 0x80000]).unwrap();
        // Same ID under another name: somebody else.
        save.trainer = trainer.clone();
        save.trainer.name = "Ho".into();
        assert!(!report.is_of(&save.trainer));
        save.trainer = trainer;
        assert!(report.is_of(&save.trainer));
        let live = save.with_live_party(report.party()).unwrap();
        assert_eq!(live.party.len(), 2);
        assert!(live.party[0].is_fainted());
        assert_eq!(live.party[1].pid, 0xFEDC_BA98);
        assert_eq!(report.map(&save.trainer), Some(3));
        // The leftover is not reported, the three new ones are.
        let pids: Vec<u8> = report.seen.iter().map(|raw| raw[0]).collect();
        assert_eq!(pids, [51, 52, 53]);
        // The newest wild one, as it is at the end: hurt and asleep.
        let facing = report.wild(&live).unwrap();
        assert_eq!(facing.species, 506);
        assert_eq!(facing.battle.as_ref().unwrap().hp, 6);
        assert_eq!(facing.ailment, Some(soulsync_core::Ailment::Sleep));
        let mut encounters = Encounters::default();
        assert!(encounters.observe(&live, &report.seen, Some(20)));
        let list = encounters.of(&save.trainer);
        assert_eq!(list.len(), 1);
        assert_eq!((list[0].species, list[0].met_location), (504, 20));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
