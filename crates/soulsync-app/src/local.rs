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
    read_catch_rates, read_dex, read_level_caps, read_map_locations, read_rom_header, Lang,
    ROM_HEADER_LEN,
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
    /// The unmodified ROM that a fresh start randomizes anew.
    original: Option<PathBuf>,
    /// The randomizer's `.jar`.
    randomizer: Option<PathBuf>,
    /// The randomizer settings, an `.rnqs`.
    preset: Option<PathBuf>,
    /// The save every fresh start begins with, kept with the app's data.
    template: Option<PathBuf>,
    /// Randomizer options changed in SoulSync on top of the `.rnqs`.
    rando: soulsync_core::rnqs::Overrides,
    /// Rare Candies every fresh start puts in the bag; 0 leaves it alone.
    candies: u16,
    /// Every run this app was in, newest first, to go back to.
    runs: Vec<Saved>,
}

/// A run the app was in, with what it takes to play it again.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    code: String,
    /// As in [`Config::server`]: empty for a run kept in this app.
    server: String,
    name: String,
    rom: Option<PathBuf>,
    save: Option<PathBuf>,
    /// The game's name, `{ "en": …, "de": … }`, as the ROM said.
    game: Option<serde_json::Value>,
    /// When it was last played, in milliseconds since 1970.
    at: u64,
}

/// Runs remembered at most.
const SAVED_RUNS: usize = 20;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl Config {
    /// Puts the current run at the top of [`Config::runs`].
    fn remember(&mut self, at: u64) {
        let Some(code) = self.code.clone() else {
            return;
        };
        let game = self
            .rom
            .as_deref()
            .map(rom_info)
            .and_then(|i| i.get("label").filter(|l| !l.is_null()).cloned());
        self.runs
            .retain(|r| !(r.code == code && r.server == self.server));
        self.runs.insert(
            0,
            Saved {
                code,
                server: self.server.clone(),
                name: self.name.clone(),
                rom: self.rom.clone(),
                save: self.save.clone(),
                game,
                at,
            },
        );
        self.runs.truncate(SAVED_RUNS);
    }

    /// Makes a remembered run the current one again.
    fn resume(&mut self, run: &Saved) {
        self.code = Some(run.code.clone());
        self.server = run.server.clone();
        self.name = run.name.clone();
        self.rom = run.rom.clone();
        self.save = run.save.clone();
    }
}

/// Where the encounters of a run are kept. Each run keeps its own, so that
/// switching runs loses none.
fn encounters_path(code: &str) -> Option<PathBuf> {
    let dir = data_dir()?.join("encounters");
    std::fs::create_dir_all(&dir).ok()?;
    let safe: String = code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let path = dir.join(format!("{safe}.json"));
    // Builds before runs could be switched kept one file for all.
    let old = data_dir()?.join("encounters.json");
    if !path.exists() && old.is_file() && std::fs::copy(&old, &path).is_ok() {
        let _ = std::fs::rename(&old, old.with_extension("json.old"));
    }
    Some(path)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    #[default]
    Idle,
    Connecting,
    /// The save does not exist yet: a new game that was not saved so far.
    Waiting,
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
    /// How far a fresh start of the game is, once asked for.
    fresh: Option<crate::fresh::Progress>,
    /// A fresh start without a starting save is running: the first save
    /// of the new game becomes it.
    awaiting_template: bool,
    /// Whether moves, items and abilities could be read from the ROM: how
    /// many moves, or why not.
    dex: Option<Result<usize, String>>,
    /// Sprites and item icons of the ROM they were read from.
    art: Option<(PathBuf, Option<Arc<soulsync_core::art::Art>>)>,
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

/// The DeSmuME that plays `save`, to check on and to start.
fn desmume_exe(save: &Path) -> Option<PathBuf> {
    let (dir, _) = find_desmume(save)?;
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
    exes.into_iter().find(|e| exe_machine(e).is_some())
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

/// Which game a ROM is, from its header.
fn rom_game(path: &Path) -> Option<soulsync_core::Game> {
    use std::io::Read;
    let mut header = [0u8; ROM_HEADER_LEN];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut header))
        .ok()?;
    read_rom_header(&header)?.game
}

/// What the Settings tab shows about a ROM.
fn rom_info(path: &Path) -> serde_json::Value {
    let game = rom_game(path);
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

/// What a randomizer settings file says about the options SoulSync offers.
fn preset_info(path: &Path) -> serde_json::Value {
    use soulsync_core::rnqs::Preset;
    let read = std::fs::read(path)
        .map_err(|e| e.to_string())
        .and_then(|f| Preset::read(&f).map_err(|e| e.to_string()));
    match read {
        Ok(p) => serde_json::json!({
            "ok": true,
            "game": p.rom_name(),
            "tm": p.tm(),
            "tutor": p.tutor(),
            "full_hm": p.full_hm(),
            "random_tm_moves": p.random_tm_moves(),
        }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    }
}

/// The randomizer and its settings usually sit next to the original ROM,
/// as they come in the randomizer's download; found there, nobody has to
/// pick them.
fn find_randomizer(config: &mut Config) {
    let Some(original) = config.original.clone() else {
        return;
    };
    let dirs: Vec<PathBuf> = [original.parent(), original.parent().and_then(Path::parent)]
        .into_iter()
        .flatten()
        .map(Path::to_path_buf)
        .collect();
    let find = |want: &dyn Fn(&str) -> bool| {
        dirs.iter().find_map(|d| {
            let mut found: Vec<PathBuf> = std::fs::read_dir(d)
                .ok()?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| want(&n.to_ascii_lowercase()))
                })
                .collect();
            found.sort();
            found.into_iter().next()
        })
    };
    if config.randomizer.as_deref().map_or(true, |p| !p.is_file()) {
        if let Some(jar) = find(&|n| n.starts_with("pokerandozx") && n.ends_with(".jar")) {
            config.randomizer = Some(jar);
        }
    }
    if config.preset.as_deref().map_or(true, |p| !p.is_file()) {
        if let Some(rnqs) = find(&|n| n.ends_with(".rnqs") && !n.contains(".soulsync.")) {
            config.preset = Some(rnqs);
        }
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
            "runs": c.runs.iter().map(|r| serde_json::json!({
                "code": r.code,
                "server": r.server,
                "name": r.name,
                "game": r.game,
                "at": r.at,
                "current": c.code.as_deref() == Some(r.code.as_str()) && c.server == r.server,
            })).collect::<Vec<_>>(),
            "phase": inner.phase,
            "message": inner.message,
            "art": c.rom.is_some() || c.original.is_some(),
            "play": c.rom.as_deref().is_some_and(Path::exists)
                && c.save.as_deref().and_then(desmume_exe).is_some(),
            "dex": inner.dex.as_ref().map(|d| match d {
                Ok(moves) => serde_json::json!({ "moves": moves }),
                Err(why) => serde_json::json!({ "error": why }),
            }),
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
            "fresh": serde_json::json!({
                "original": c.original.as_deref().map(rom_info),
                "randomizer": c.randomizer,
                "preset": c.preset,
                "options": c.preset.as_deref().map(preset_info),
                "rando": c.rando,
                "candies": c.candies,
                "template": c.template.as_deref().filter(|t| t.is_file()).map(save_info),
                "awaiting": inner.awaiting_template,
                "progress": inner.fresh,
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
        let mut rom = config
            .rom
            .as_ref()
            .and_then(|p| std::fs::File::open(p).ok());
        let rates = rom.as_mut().and_then(|f| read_catch_rates(f).ok());
        let game = config.rom.as_deref().and_then(rom_game);
        let dex = match (rom.as_mut(), game) {
            (Some(f), Some(g)) => read_dex(f, g).map_err(|e| e.to_string()),
            (None, _) => Err("no ROM".into()),
            (_, None) => Err("not a supported game".into()),
        };
        self.inner.lock().unwrap().dex =
            Some(dex.as_ref().map(|d| d.moves.len()).map_err(Clone::clone));
        let dex = dex.ok();
        // Without them encounters cannot be placed; the party still syncs.
        let locations = rom
            .as_mut()
            .and_then(|f| read_map_locations(f).ok())
            .unwrap_or_default();
        let installed = install_live_script();
        let mut live = installed.as_ref().and_then(|(_, out)| {
            let store = encounters_path(&code)?;
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
            // A new game has no save until the player saves for the first
            // time. Until then the script must not look for the party of the
            // game before, whose trainer it was told about.
            let trainer_note = live_party_path().map(|out| out.with_file_name("live-trainer.txt"));
            let mut last = loop {
                match load(&save) {
                    Ok(s) => break s,
                    Err(_) if !save.exists() => {
                        if let Some(note) = &trainer_note {
                            let _ = std::fs::remove_file(note);
                        }
                        me.set_phase(Phase::Waiting, None);
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                    Err(e) => return me.set_phase(Phase::Error, Some(e.to_string())),
                }
            };
            me.set_phase(Phase::Connecting, None);
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
                    dex.as_ref(),
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

#[derive(Default, Deserialize)]
struct Set {
    name: Option<String>,
    rom: Option<String>,
    save: Option<String>,
    server: Option<String>,
    original: Option<String>,
    randomizer: Option<String>,
    preset: Option<String>,
    /// Replaces all randomizer options SoulSync changes.
    rando: Option<soulsync_core::rnqs::Overrides>,
    candies: Option<u16>,
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
    if let Some(original) = set.original {
        config.original = path(original);
        find_randomizer(config);
    }
    if let Some(rando) = set.rando {
        config.rando = rando;
    }
    if let Some(candies) = set.candies {
        config.candies = candies.min(soulsync_core::bag::MAX_COUNT);
    }
    if let Some(randomizer) = set.randomizer {
        config.randomizer = path(randomizer);
    }
    if let Some(preset) = set.preset {
        config.preset = path(preset);
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

/// A file the Settings can ask for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Rom,
    Save,
    Original,
    Randomizer,
    Preset,
}

#[cfg(any(windows, target_os = "macos"))]
fn dialog(kind: Kind, start: Option<PathBuf>) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new();
    d = match kind {
        Kind::Rom | Kind::Original => d.add_filter("Nintendo DS ROM", &["nds"]),
        Kind::Save => d.add_filter("Save", &["dsv", "sav"]),
        Kind::Randomizer => d.add_filter("Universal Pokémon Randomizer ZX", &["jar"]),
        Kind::Preset => d.add_filter("Randomizer settings", &["rnqs"]),
    };
    if let Some(dir) = start.as_deref().and_then(Path::parent) {
        d = d.set_directory(dir);
    }
    d.pick_file()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn dialog(_kind: Kind, _start: Option<PathBuf>) -> Option<PathBuf> {
    None
}

/// Opens the system's file dialog and stores the chosen file.
async fn pick(State(local): State<Local>, Json(pick): Json<Pick>) -> Reply {
    let kind = match pick.kind.as_str() {
        "rom" => Kind::Rom,
        "save" => Kind::Save,
        "original" => Kind::Original,
        "randomizer" => Kind::Randomizer,
        "preset" => Kind::Preset,
        _ => return Err(bad("kind")),
    };
    let start = {
        let inner = local.inner.lock().unwrap();
        let c = &inner.config;
        let rom = || c.rom.clone();
        match kind {
            Kind::Rom => rom(),
            Kind::Save => c.save.clone().or_else(rom),
            Kind::Original => c.original.clone().or_else(rom),
            Kind::Randomizer => c.randomizer.clone().or_else(rom),
            Kind::Preset => c.preset.clone().or(c.randomizer.clone()).or_else(rom),
        }
    };
    let chosen = tokio::task::spawn_blocking(move || dialog(kind, start))
        .await
        .ok()
        .flatten();
    if let Some(path) = chosen {
        let path = Some(path.to_string_lossy().into_owned());
        let mut set = Set::default();
        match kind {
            Kind::Rom => set.rom = path,
            Kind::Save => set.save = path,
            Kind::Original => set.original = path,
            Kind::Randomizer => set.randomizer = path,
            Kind::Preset => set.preset = path,
        }
        let mut inner = local.inner.lock().unwrap();
        apply(&mut inner.config, set);
        write_config(&inner.config);
    }
    Ok(Json(local.view()))
}

/// Keeps the save as it is now as the one every fresh start begins with.
async fn template(State(local): State<Local>) -> Reply {
    let save = local.inner.lock().unwrap().config.save.clone();
    let save = save
        .filter(|s| load(s).is_ok())
        .ok_or_else(|| bad("save"))?;
    let kept = data_dir()
        .ok_or_else(|| bad("data"))?
        .join("start-save.dsv");
    if let Some(dir) = kept.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::copy(&save, &kept).map_err(|e| bad(e.to_string()))?;
    {
        let mut inner = local.inner.lock().unwrap();
        inner.config.template = Some(kept);
        write_config(&inner.config);
    }
    Ok(Json(local.view()))
}

/// Forgets the starting save: the next fresh start begins a new game from
/// the very beginning, and its first save becomes the starting save again.
async fn untemplate(State(local): State<Local>) -> Reply {
    {
        let mut inner = local.inner.lock().unwrap();
        if let Some(kept) = inner.config.template.take() {
            // Only the copy SoulSync keeps, never a save the player chose.
            if data_dir().is_some_and(|d| kept.starts_with(d)) {
                let _ = std::fs::remove_file(&kept);
            }
        }
        write_config(&inner.config);
    }
    Ok(Json(local.view()))
}

/// Starts the game over: randomizes the ROM anew, puts the starting save in
/// place and opens the game. The page follows the progress in the state.
async fn fresh(State(local): State<Local>) -> Reply {
    use crate::fresh::{Plan, Progress};
    let plan = {
        let mut inner = local.inner.lock().unwrap();
        if inner.fresh == Some(Progress::Rolling) {
            None
        } else {
            let c = &inner.config;
            let need = |p: &Option<PathBuf>, what| p.clone().ok_or_else(|| bad(what));
            let save = need(&c.save, "save")?;
            let plan = Plan {
                java: "java".into(),
                randomizer: need(&c.randomizer, "randomizer")?,
                preset: need(&c.preset, "preset")?,
                overrides: c.rando,
                original: need(&c.original, "original")?,
                rom: need(&c.rom, "rom")?,
                template: c.template.clone().filter(|t| t.is_file()),
                candies: c.candies,
                // The new game's encounters start empty; the run's are the
                // ones of the game that ended.
                encounters: c.code.as_deref().and_then(encounters_path),
                emulator: desmume_exe(&save),
                save,
            };
            inner.fresh = Some(Progress::Rolling);
            inner.awaiting_template = false;
            Some(plan)
        }
    };
    let Some(plan) = plan else {
        return Ok(Json(local.view()));
    };
    // The sync reads the save and the ROM; it starts again on the new ones.
    let syncing = local.inner.lock().unwrap().task.is_some();
    local.stop();
    let worker = local.clone();
    tokio::spawn(async move {
        let work = plan.clone();
        let done = tokio::task::spawn_blocking(move || crate::fresh::run(&work)).await;
        let progress = match done {
            Ok(Ok(())) => {
                if let Some(emulator) = &plan.emulator {
                    crate::fresh::play(emulator, &plan.rom);
                }
                if plan.template.is_none() {
                    tokio::spawn(await_template(worker.clone(), plan.save.clone()));
                }
                Progress::Done
            }
            Ok(Err((why, detail))) => Progress::Failed { why, detail },
            Err(e) => Progress::Failed {
                why: crate::fresh::Why::Write,
                detail: Some(e.to_string()),
            },
        };
        if syncing {
            worker.start();
        }
        worker.inner.lock().unwrap().fresh = Some(progress);
    });
    Ok(Json(local.view()))
}

/// After a fresh start without a starting save: the player plays the
/// intro once, sets the options and saves. That save, still without a
/// Pokémon, becomes the one every later fresh start begins with. Saved
/// again before the first Pokémon, it is taken again.
async fn await_template(local: Local, save: PathBuf) {
    local.inner.lock().unwrap().awaiting_template = true;
    let Some(kept) = data_dir().map(|d| d.join("start-save.dsv")) else {
        return;
    };
    let mut seen = None;
    // Long enough for the intro, however slowly it is played.
    for _ in 0..(3 * 60 * 60 / 3) {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        if !local.inner.lock().unwrap().awaiting_template {
            return;
        }
        let modified = std::fs::metadata(&save).and_then(|m| m.modified()).ok();
        if modified.is_none() || modified == seen {
            continue;
        }
        seen = modified;
        let Ok(snap) = load(&save) else { continue };
        if snap.all_pokemon().next().is_some() {
            break;
        }
        if snap.trainer.name.is_empty() || std::fs::copy(&save, &kept).is_err() {
            continue;
        }
        let mut inner = local.inner.lock().unwrap();
        inner.config.template = Some(kept.clone());
        write_config(&inner.config);
    }
    local.inner.lock().unwrap().awaiting_template = false;
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
        inner.config.remember(now_ms());
        write_config(&inner.config);
    }
    // Reading the ROM takes a moment.
    let starter = local.clone();
    let _ = tokio::task::spawn_blocking(move || starter.start()).await;
    Ok(Json(local.view()))
}

#[derive(Deserialize)]
struct RunRef {
    code: String,
    #[serde(default)]
    server: String,
}

/// Goes back to a remembered run, leaving the current one paused. A run its
/// server no longer has is forgotten.
async fn resume_run(State(local): State<Local>, Json(req): Json<RunRef>) -> Reply {
    let run = {
        let inner = local.inner.lock().unwrap();
        inner
            .config
            .runs
            .iter()
            .find(|r| r.code == req.code && r.server == req.server)
            .cloned()
            .ok_or_else(|| bad("unknown run"))?
    };
    let found = if run.server.trim().is_empty() {
        Some(local.server.has_run(&run.code))
    } else {
        let url = format!("{}/api/runs/{}", run.server, run.code);
        tokio::task::spawn_blocking(move || run_exists(&url))
            .await
            .unwrap_or(None)
    };
    match found {
        Some(true) => {}
        // Only a server that answers can say a run is gone.
        None => return Err(bad("offline")),
        Some(false) => {
            let mut inner = local.inner.lock().unwrap();
            inner.config.runs.retain(|r| r != &run);
            write_config(&inner.config);
            return Err(bad("missing"));
        }
    }
    local.stop();
    {
        let mut inner = local.inner.lock().unwrap();
        inner.config.resume(&run);
        inner.config.remember(now_ms());
        write_config(&inner.config);
    }
    local.start();
    Ok(Json(local.view()))
}

/// Takes a run off the list. The run itself stays on its server.
async fn drop_run(State(local): State<Local>, Json(req): Json<RunRef>) -> Json<serde_json::Value> {
    {
        let mut inner = local.inner.lock().unwrap();
        let c = &mut inner.config;
        let current = c.code.as_deref() == Some(req.code.as_str()) && c.server == req.server;
        if !current {
            c.runs
                .retain(|r| !(r.code == req.code && r.server == req.server));
            write_config(c);
        }
    }
    Json(local.view())
}

/// Whether the server has the run at `url`; `None` if it cannot be asked.
fn run_exists(url: &str) -> Option<bool> {
    match ureq::get(url).call() {
        Ok(_) => Some(true),
        Err(ureq::Error::StatusCode(404)) => Some(false),
        Err(_) => None,
    }
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

/// How often a running app looks for a newer build.
const UPDATE_EVERY: Duration = Duration::from_secs(10 * 60);

/// Looks for a newer build in the background, at start and then every few
/// minutes, so an app left open hears of updates too. Only the Windows
/// program is published, and only numbered builds know what is newer.
fn look_for_update(local: &Local) {
    let Some(own) = crate::update::build().filter(|_| cfg!(windows)) else {
        return;
    };
    let local = local.clone();
    std::thread::spawn(move || loop {
        let busy = local.inner.lock().unwrap().progress.is_some();
        if !busy {
            if let Some(release) = crate::update::latest().filter(|r| r.build > own) {
                let mut inner = local.inner.lock().unwrap();
                if inner.progress.is_none() {
                    inner.update = Some(release);
                }
            }
        }
        std::thread::sleep(UPDATE_EVERY);
    });
}

/// Set once the new program has its window up and this one may quit.
static HANDED_OVER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Called by the new program after an update once its window shows: this
/// one stores its runs and quits, which frees the port for the new one.
async fn handoff(State(local): State<Local>) -> StatusCode {
    let installing = matches!(
        local.inner.lock().unwrap().progress,
        Some(crate::update::Progress::Restart)
    );
    if !installing {
        return StatusCode::CONFLICT;
    }
    crate::update::log("the new program took over");
    HANDED_OVER.store(true, std::sync::atomic::Ordering::SeqCst);
    StatusCode::NO_CONTENT
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
            Some(Progress::Download { .. } | Progress::Start | Progress::Restart)
        );
        (!busy).then(|| {
            inner.progress = Some(Progress::Download {
                percent: None,
                done: 0,
                size: None,
            });
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
            // This window stays until the new one is up, or for half a
            // minute if the new program never says so.
            Ok(()) => {
                for _ in 0..120 {
                    if HANDED_OVER.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(250));
                }
                worker.stop();
                worker.server.save();
                crate::update::log("old program quits");
                std::process::exit(0);
            }
            Err(_) => tell(Progress::Failed),
        }
    });
    Ok(Json(local.view()))
}

/// Leaves the run. It stays on its server and in the list of runs, to be
/// resumed later.
async fn leave(State(local): State<Local>) -> Json<serde_json::Value> {
    local.stop();
    {
        let mut inner = local.inner.lock().unwrap();
        inner.config.code = None;
        write_config(&inner.config);
    }
    Json(local.view())
}

/// Opens the game being played in DeSmuME.
async fn play(State(local): State<Local>) -> Reply {
    let (rom, save) = {
        let inner = local.inner.lock().unwrap();
        (inner.config.rom.clone(), inner.config.save.clone())
    };
    let rom = rom.filter(|r| r.exists()).ok_or_else(|| bad("rom"))?;
    let exe = save
        .as_deref()
        .and_then(desmume_exe)
        .ok_or_else(|| bad("emulator"))?;
    crate::fresh::play(&exe, &rom);
    Ok(Json(local.view()))
}

/// The sprites and icons of the ROM being played, or of the original ROM:
/// both have the same pictures. Read once per ROM.
fn art(local: &Local) -> Option<Arc<soulsync_core::art::Art>> {
    let path = {
        let inner = local.inner.lock().unwrap();
        let path = inner
            .config
            .rom
            .clone()
            .or_else(|| inner.config.original.clone())?;
        if let Some((p, art)) = &inner.art {
            if *p == path {
                return art.clone();
            }
        }
        path
    };
    let art = std::fs::File::open(&path)
        .ok()
        .and_then(|mut f| soulsync_core::art::Art::read(&mut f).ok())
        .map(Arc::new);
    local.inner.lock().unwrap().art = Some((path, art.clone()));
    art
}

async fn picture(local: Local, id: u16, item: bool) -> Response {
    let png = tokio::task::spawn_blocking(move || {
        let art = art(&local)?;
        if item {
            art.item_icon(id)
        } else {
            art.sprite(id)
        }
    })
    .await
    .ok()
    .flatten();
    match png {
        Some(png) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "max-age=86400"),
            ],
            png,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn sprite(
    State(local): State<Local>,
    axum::extract::Path(id): axum::extract::Path<u16>,
) -> Response {
    if id == 0 || id > 649 {
        return StatusCode::NOT_FOUND.into_response();
    }
    picture(local, id, false).await
}

async fn item_icon(
    State(local): State<Local>,
    axum::extract::Path(id): axum::extract::Path<u16>,
) -> Response {
    if id == 0 || id > 1000 {
        return StatusCode::NOT_FOUND.into_response();
    }
    picture(local, id, true).await
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
    // Runs kept in this app are stored with it now, but one may be gone.
    let resume = {
        let mut inner = local.inner.lock().unwrap();
        let c = &mut inner.config;
        if c.server.is_empty() && !c.code.as_deref().is_some_and(|k| local.server.has_run(k)) {
            c.code = None;
        }
        // Builds before the list of runs knew only the current one.
        if c.runs.is_empty() && c.code.is_some() {
            c.remember(now_ms());
            write_config(c);
        }
        c.code.is_some()
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
        .route("/api/local/resume", post(resume_run))
        .route("/api/local/drop", post(drop_run))
        .route("/api/local/update", post(update))
        .route("/api/local/handoff", post(handoff))
        .route("/api/local/forget", post(forget))
        .route("/api/local/template", post(template))
        .route("/api/local/untemplate", post(untemplate))
        .route("/api/local/fresh", post(fresh))
        .route("/api/local/play", post(play))
        .route("/api/local/sprite/{id}", get(sprite))
        .route("/api/local/item/{id}", get(item_icon))
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
            ..Set::default()
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
        // The save data says map 7, the one of the last door. The game's own
        // record of where the player is, with a copy of the trainer behind
        // it, says map 3.
        ram[at + 0x704] = 7;
        let live = 0x9000;
        ram[live] = 3;
        ram[live + 4..live + 8].copy_from_slice(&0x0055_8000u32.to_le_bytes());
        ram[live + 12..live + 16].copy_from_slice(&0x02B1_8000u32.to_le_bytes());
        ram[live + 0x20..live + 0x26].copy_from_slice(&[b'H', 0, b'i', 0, 0xFF, 0xFF]);
        ram[live + 0x30..live + 0x34].copy_from_slice(&trainer_id.to_le_bytes());
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
        // The game's working copy of B in the battle: it points to B and has
        // the species, the most HP and the HP now, 3, then the level.
        let mut battle = vec![0u8; 0x20];
        battle[..4].copy_from_slice(&0x0200_C000u32.to_le_bytes());
        battle[0x0C..0x0E].copy_from_slice(&506u16.to_le_bytes());
        battle[0x0E..0x10].copy_from_slice(&60u16.to_le_bytes());
        battle[0x10..0x12].copy_from_slice(&3u16.to_le_bytes());
        battle[0x18] = 4;
        // Poisoned in the battle; B itself says asleep.
        battle = [battle, vec![0u8; 0x20]].concat();
        battle[0x30] = 1;
        globals
            .set("BATTLE", lua.create_string(&battle).unwrap())
            .unwrap();
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
              if frames == 650 then put(0xC000, B2) put(0xE000, BATTLE) end
              if frames > 1000 then error("enough frames") end
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
        // The newest wild one, as the battle has it at the end.
        let facing = report.wild(&live).unwrap();
        assert_eq!(facing.species, 506);
        // The HP of the battle's copy, not the 6 of the Pokémon itself.
        assert_eq!(facing.battle.as_ref().unwrap().hp, 3);
        assert_eq!(facing.battle.as_ref().unwrap().max_hp, 60);
        assert_eq!(facing.ailment, Some(soulsync_core::Ailment::Poison));
        let mut encounters = Encounters::default();
        assert!(encounters.observe(&live, &report.seen, Some(20)));
        let list = encounters.of(&save.trainer);
        assert_eq!(list.len(), 1);
        assert_eq!((list[0].species, list[0].met_location), (504, 20));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn remembers_runs_to_switch_between() {
        let mut c = Config {
            name: "Hannes".into(),
            save: Some("duo.dsv".into()),
            code: Some("ZORUA-0001".into()),
            ..Config::default()
        };
        c.remember(1);
        // A 4-player run on a server, with another save.
        c.server = "https://soulsync.example".into();
        c.save = Some("quad.dsv".into());
        c.code = Some("HYPNO-0002".into());
        c.remember(2);
        assert_eq!(c.runs.len(), 2);
        assert_eq!(c.runs[0].code, "HYPNO-0002");

        let duo = c.runs[1].clone();
        c.resume(&duo);
        c.remember(3);
        assert_eq!(c.code.as_deref(), Some("ZORUA-0001"));
        assert_eq!(c.server, "");
        assert_eq!(c.save.as_deref(), Some(Path::new("duo.dsv")));
        assert_eq!(
            c.runs
                .iter()
                .map(|r| (r.code.as_str(), r.at))
                .collect::<Vec<_>>(),
            [("ZORUA-0001", 3), ("HYPNO-0002", 2)]
        );
    }
}
