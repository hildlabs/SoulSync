//! Starting a game over: a freshly randomized ROM and the save the player
//! prepared once, so that a failed run costs a click instead of the whole
//! setup. SoulSync randomizes the game itself where it can do all the
//! settings ask (see [`own`]). Otherwise the randomizer is Universal Pokémon
//! Randomizer ZX, run without its window: fetched on first use (see
//! [`crate::tools`]) or the player's own.
//!
//! SoulSync does not ship anything of the game: the player points it at
//! their own ROM.

use serde::Serialize;
use soulsync_core::rando::gen5;
use soulsync_core::rnqs::{Overrides, Preset};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Everything one fresh start needs.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Whether SoulSync's own randomizer makes the game if it can; without
    /// it the original randomizer always does.
    pub own: bool,
    /// The program that runs the randomizer, `java` unless a test says so.
    pub java: OsString,
    /// `PokeRandoZX.jar`.
    pub randomizer: PathBuf,
    /// The `.rnqs` with the player's randomizer settings.
    pub preset: PathBuf,
    /// Options changed in SoulSync on top of the `.rnqs`.
    pub overrides: Overrides,
    /// The unmodified ROM.
    pub original: PathBuf,
    /// The ROM that is played; replaced.
    pub rom: PathBuf,
    /// The save that is played; replaced.
    pub save: PathBuf,
    /// The save every attempt starts from. Without one the game starts
    /// with no save, at its very beginning.
    pub template: Option<PathBuf>,
    /// Rare Candies to put in the bag of the new save; 0 leaves the bag
    /// as the starting save has it.
    pub candies: u16,
    /// The live encounters of the game so far; they end with it.
    pub encounters: Option<PathBuf>,
    /// The emulator, to be sure it does not have the game open.
    pub emulator: Option<PathBuf>,
}

/// How far a fresh start is, for the Settings tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Progress {
    Rolling,
    Done,
    Failed { why: Why, detail: Option<String> },
}

/// What stands in the way of a fresh start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    /// A file of the setup is missing; the detail says which.
    Missing,
    /// The emulator is still open and would write its save over the new one.
    Running,
    /// Java could not be started.
    Java,
    /// The randomizer or Java could not be fetched.
    Download,
    /// The randomizer ran and left no ROM.
    Randomizer,
    /// The randomizer settings could not be read.
    Preset,
    /// The starting save is not a Black 2 / White 2 save.
    Save,
    /// A file could not be written.
    Write,
    /// A Windows emulator needs Wine, which is not installed.
    Wine,
    /// The emulator did not start.
    Emulator,
}

pub type Failure = (Why, Option<String>);

#[cfg(windows)]
pub fn quiet(command: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW: no console flashing up next to the app.
    command.creation_flags(0x0800_0000)
}

#[cfg(not(windows))]
pub fn quiet(command: &mut Command) -> &mut Command {
    command
}

/// Whether the program at `exe` is running. On Windows the task list is
/// asked. Elsewhere only a Windows emulator run through Wine matters: it
/// writes its save on exit like on Windows, and its process names the
/// `.exe`. Other emulators are left alone.
pub fn running(exe: &Path) -> bool {
    let Some(name) = exe.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !cfg!(windows) {
        return needs_wine(exe)
            && Command::new("pgrep")
                .args(["-i", "-f", name])
                .output()
                .is_ok_and(|o| o.status.success());
    }
    quiet(Command::new("tasklist").args(["/NH", "/FO", "CSV", "/FI"]))
        .arg(format!("IMAGENAME eq {name}"))
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .to_ascii_lowercase()
                .contains(&name.to_ascii_lowercase())
        })
        .unwrap_or(false)
}

/// Whether `emulator` is a Windows program on a system that runs it through
/// Wine: DeSmuME's Lua scripting, which the live sync needs, exists only in
/// its Windows build, so on a Mac that build runs in Wine.
pub fn needs_wine(emulator: &Path) -> bool {
    !cfg!(windows)
        && emulator
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
}

/// Where Wine may be: the WineHQ builds for macOS install as an app
/// (Gcenx/macOS_Wine_builds), Homebrew and Linux put `wine` on the PATH. An
/// app started from the Finder does not get the shell's PATH, so the usual
/// folders are tried by name.
fn wine_candidates() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::env::var_os("SOULSYNC_WINE")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    for app in ["Wine Stable", "Wine Devel", "Wine Staging"] {
        let inside = format!("{app}.app/Contents/Resources/wine/bin/wine");
        found.push(Path::new("/Applications").join(&inside));
        if let Some(home) = &home {
            found.push(home.join("Applications").join(&inside));
        }
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path)
        .chain(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from))
    {
        found.push(dir.join("wine"));
    }
    found
}

/// Wine, to run a Windows emulator with, if it is installed.
pub fn wine() -> Option<PathBuf> {
    if cfg!(windows) {
        return None;
    }
    wine_candidates().into_iter().find(|p| p.is_file())
}

/// `path` as a program in Wine sees it: Wine's drive Z: is the root of the
/// system.
pub fn dos_path(path: &Path) -> String {
    format!("Z:{}", path.to_string_lossy().replace('/', "\\"))
}

/// `path` with `.suffix` after its whole name.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// The last lines a program printed, as the reason it failed.
fn tail(output: &std::process::Output) -> Option<String> {
    let text = [&output.stdout[..], &output.stderr[..]].concat();
    let text = String::from_utf8_lossy(&text);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let from = lines.len().saturating_sub(4);
    (!lines.is_empty()).then(|| lines[from..].join("\n"))
}

/// The settings of the plan with SoulSync's options in them.
fn settings(plan: &Plan) -> Result<Preset, Failure> {
    let file = std::fs::read(&plan.preset).map_err(|e| (Why::Preset, Some(e.to_string())))?;
    let preset = Preset::read(&file).map_err(|e| (Why::Preset, Some(e.to_string())))?;
    Ok(preset.with(&plan.overrides))
}

/// What SoulSync's own randomizer is to do, if it can make this game: it
/// has built everything the settings ask for, and the original ROM is a
/// game its result was checked on. Neither Java nor the original
/// randomizer is needed then.
pub fn own(plan: &Plan) -> Option<gen5::Options> {
    if !plan.own {
        return None;
    }
    own_with(&settings(plan).ok()?, &plan.original)
}

/// The same for settings and an original ROM, as [`own`] looks at them.
pub fn own_with(settings: &Preset, original: &Path) -> Option<gen5::Options> {
    use std::io::Read;
    let options = settings.native()?;
    let mut header = [0u8; 16];
    std::fs::File::open(original)
        .and_then(|mut f| f.read_exact(&mut header))
        .ok()?;
    gen5::verified(&header).then_some(options)
}

/// A seed no one chose, 48 bits as the original randomizer picks them.
fn seed() -> i64 {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    (hasher.finish() & 0xFFFF_FFFF_FFFF) as i64
}

/// Randomizes the original ROM into `fresh` with SoulSync's own randomizer.
fn roll(plan: &Plan, options: &gen5::Options, fresh: &Path) -> Result<(), Failure> {
    let original = std::fs::read(&plan.original).map_err(|e| (Why::Write, Some(e.to_string())))?;
    let rom = gen5::randomize(&original, options, seed())
        .map_err(|e| (Why::Randomizer, Some(e.to_string())))?;
    std::fs::write(fresh, rom).map_err(|e| (Why::Write, Some(e.to_string())))
}

/// Randomizes the original ROM into `fresh` with the original randomizer.
fn java(plan: &Plan, fresh: &Path) -> Result<(), Failure> {
    // The settings with SoulSync's options, in a file of their own: the
    // player's `.rnqs` stays as it is.
    let preset = if plan.overrides.is_empty() {
        None
    } else {
        let path = plan.rom.with_extension("soulsync.rnqs");
        std::fs::write(&path, settings(plan)?.write())
            .map_err(|e| (Why::Write, Some(e.to_string())))?;
        Some(path)
    };
    let mut command = Command::new(&plan.java);
    command.arg("-Xmx4096M").arg("-jar").arg(&plan.randomizer);
    command
        .arg("cli")
        .arg("-s")
        .arg(preset.as_ref().unwrap_or(&plan.preset));
    command.arg("-i").arg(&plan.original);
    command.arg("-o").arg(fresh).arg("-l");
    if let Some(dir) = plan.randomizer.parent().filter(|d| d.is_dir()) {
        command.current_dir(dir);
    }
    let output = quiet(&mut command).output();
    if let Some(preset) = &preset {
        let _ = std::fs::remove_file(preset);
    }
    let output = output.map_err(|e| (Why::Java, Some(e.to_string())))?;
    if !fresh.is_file() {
        return Err((Why::Randomizer, tail(&output)));
    }
    Ok(())
}

/// Randomizes the ROM anew and puts the starting save in place.
pub fn run(plan: &Plan) -> Result<(), Failure> {
    let own = own(plan);
    let files = [
        ("original", Some(&plan.original)),
        ("randomizer", own.is_none().then_some(&plan.randomizer)),
        ("preset", Some(&plan.preset)),
        ("template", plan.template.as_ref()),
    ];
    for (what, path) in files {
        if path.is_some_and(|p| !p.is_file()) {
            return Err((Why::Missing, Some(what.to_owned())));
        }
    }
    if plan.original == plan.rom {
        return Err((Why::Missing, Some("original".to_owned())));
    }
    if plan.emulator.as_deref().is_some_and(running) {
        return Err((Why::Running, None));
    }

    // The starting save, ready before anything is replaced.
    let start = match &plan.template {
        Some(template) => {
            let mut save =
                std::fs::read(template).map_err(|e| (Why::Write, Some(e.to_string())))?;
            if plan.candies > 0 {
                soulsync_core::bag::set_medicine(
                    &mut save,
                    soulsync_core::bag::RARE_CANDY,
                    plan.candies,
                )
                .map_err(|e| (Why::Save, Some(e.to_string())))?;
            }
            Some(save)
        }
        None => None,
    };

    // The new game is written next to the ROM first: a run that fails
    // leaves the game that is there untouched.
    let fresh = plan.rom.with_extension("new.nds");
    let _ = std::fs::remove_file(&fresh);
    match &own {
        Some(options) => roll(plan, options, &fresh)?,
        None => java(plan, &fresh)?,
    }

    let write = |e: std::io::Error| (Why::Write, Some(e.to_string()));
    std::fs::rename(&fresh, &plan.rom).map_err(write)?;
    // The randomizer's log tells what it did; keep it next to the ROM as
    // the randomizer itself would.
    let _ = std::fs::rename(with_suffix(&fresh, ".log"), with_suffix(&plan.rom, ".log"));
    if let Some(dir) = plan.save.parent() {
        std::fs::create_dir_all(dir).map_err(write)?;
    }
    // The last game's save is kept once, in case it is still wanted.
    if plan.save.is_file() {
        let _ = std::fs::rename(&plan.save, with_suffix(&plan.save, ".bak"));
    }
    if let Some(start) = start {
        std::fs::write(&plan.save, start).map_err(write)?;
    }
    if let Some(encounters) = &plan.encounters {
        let _ = std::fs::remove_file(encounters);
    }
    Ok(())
}

/// Opens the game in the emulator. Fails only when a Windows emulator needs
/// Wine and there is none, or the emulator does not start.
pub fn play(emulator: &Path, rom: &Path) -> Result<(), Why> {
    let mut command = if needs_wine(emulator) {
        let mut command = Command::new(wine().ok_or(Why::Wine)?);
        command
            .arg(emulator)
            .arg(dos_path(rom))
            // No offer to install Wine's .NET and browser on the first
            // start, and no debug output.
            .env("WINEDLLOVERRIDES", "mscoree,mshtml=")
            .env("WINEDEBUG", "-all");
        command
    } else {
        let mut command = Command::new(emulator);
        command.arg(rom);
        command
    };
    if let Some(dir) = emulator.parent() {
        command.current_dir(dir);
    }
    command.spawn().map(drop).map_err(|_| Why::Emulator)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn runs_windows_emulators_through_wine() {
        assert!(needs_wine(Path::new(
            "/Users/a/DeSmuME/DeSmuME_0.9.13_x64.exe"
        )));
        assert!(needs_wine(Path::new("/Users/a/DeSmuME/DESMUME.EXE")));
        assert!(!needs_wine(Path::new("/Applications/DeSmuME.app")));
        assert_eq!(
            dos_path(Path::new("/Users/a/DeSmuME/Roms/SoulSync-X.nds")),
            r"Z:\Users\a\DeSmuME\Roms\SoulSync-X.nds"
        );
    }

    #[test]
    fn finds_wine_where_the_mac_build_installs_it() {
        let found = wine_candidates();
        assert!(found
            .iter()
            .any(|p| p
                == Path::new("/Applications/Wine Stable.app/Contents/Resources/wine/bin/wine")));
        assert!(found
            .iter()
            .any(|p| p == Path::new("/opt/homebrew/bin/wine")));
    }

    /// A stand-in for `java` that writes what it was asked to.
    fn fake_java(dir: &Path, body: &str) -> OsString {
        let path = dir.join("java");
        let script = format!("#!/bin/sh\n[ -n \"$SOULSYNC_PROBE\" ] && exit 0\n{body}\n");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        // A process another test forks while the file is still open for
        // writing keeps it "busy" until that child execs: wait it out.
        for _ in 0..100 {
            match Command::new(&path).env("SOULSYNC_PROBE", "1").status() {
                Err(e) if e.raw_os_error() == Some(26) => {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                _ => break,
            }
        }
        path.into()
    }

    fn plan(name: &str) -> (PathBuf, Plan) {
        let dir =
            std::env::temp_dir().join(format!("soulsync-fresh-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Battery")).unwrap();
        for (file, text) in [
            ("original.nds", "original"),
            ("rando.jar", "jar"),
            ("mine.rnqs", "preset"),
            ("start.dsv", "start"),
            ("game.nds", "old game"),
            ("Battery/game.dsv", "old save"),
            ("encounters.json", "{}"),
        ] {
            std::fs::write(dir.join(file), text).unwrap();
        }
        let plan = Plan {
            own: false,
            java: "java".into(),
            randomizer: dir.join("rando.jar"),
            preset: dir.join("mine.rnqs"),
            overrides: Overrides::default(),
            original: dir.join("original.nds"),
            rom: dir.join("game.nds"),
            save: dir.join("Battery/game.dsv"),
            template: Some(dir.join("start.dsv")),
            candies: 0,
            encounters: Some(dir.join("encounters.json")),
            emulator: None,
        };
        (dir, plan)
    }

    #[test]
    fn rolls_a_new_game_and_resets_the_save() {
        let (dir, mut plan) = plan("ok");
        // -Xmx -jar <jar> cli -s <preset> -i <original> -o <out> -l
        plan.java = fake_java(
            &dir,
            r#"[ "$2 $4 $5 $7 ${11}" = "-jar cli -s -i -l" ] || exit 1
echo "$3|$6|$8" > "${10}"; echo log > "${10}.log""#,
        );
        run(&plan).unwrap();
        let rom = std::fs::read_to_string(&plan.rom).unwrap();
        assert_eq!(
            rom.trim(),
            format!(
                "{}|{}|{}",
                plan.randomizer.display(),
                plan.preset.display(),
                plan.original.display()
            )
        );
        assert_eq!(std::fs::read_to_string(&plan.save).unwrap(), "start");
        assert_eq!(
            std::fs::read_to_string(dir.join("Battery/game.dsv.bak")).unwrap(),
            "old save"
        );
        assert!(dir.join("game.nds.log").is_file());
        assert!(!dir.join("game.new.nds").exists());
        assert!(!dir.join("encounters.json").exists());
    }

    #[test]
    fn a_failed_roll_leaves_the_game_alone() {
        let (dir, mut plan) = plan("fail");
        plan.java = fake_java(&dir, "echo first; echo 'bad settings' >&2; exit 0");
        let (why, detail) = run(&plan).unwrap_err();
        assert_eq!(why, Why::Randomizer);
        assert!(detail.unwrap().contains("bad settings"));
        assert_eq!(std::fs::read_to_string(&plan.rom).unwrap(), "old game");
        assert_eq!(std::fs::read_to_string(&plan.save).unwrap(), "old save");
        assert!(dir.join("encounters.json").exists());

        plan.java = dir.join("no-such-java").into();
        assert_eq!(run(&plan).unwrap_err().0, Why::Java);

        std::fs::remove_file(plan.template.as_ref().unwrap()).unwrap();
        assert_eq!(
            run(&plan).unwrap_err(),
            (Why::Missing, Some("template".to_owned()))
        );
    }

    /// A real `.rnqs`: TM compatibility unchanged, full HM compatibility.
    fn real_preset(path: &Path) {
        let mut file = vec![0, 0, 1, 0x41, 0, 0, 0, 0x6c];
        file.extend_from_slice(b"aQgEAQLvAfIB9QEEAAKoBhgCDAEADAAdCQABAgAAAAAAABGAMgkywQAICTIGBgAyAAUAE1Bva2Vtb24gQmxhY2sgMiAoRyldllcN48M4ig==");
        std::fs::write(path, file).unwrap();
    }

    #[test]
    fn changed_options_go_to_the_randomizer_in_a_copy() {
        use soulsync_core::rnqs::Compat;
        let (dir, mut plan) = plan("options");
        real_preset(&plan.preset);
        let mine = std::fs::read(&plan.preset).unwrap();
        plan.overrides.set(
            "tm",
            soulsync_core::rnqs::Value::Choice("random_prefer_type".into()),
        );
        plan.java = fake_java(&dir, r#"cp "$6" "${10}""#);
        run(&plan).unwrap();
        // What the randomizer was given is now the "ROM".
        let given = Preset::read(&std::fs::read(&plan.rom).unwrap()).unwrap();
        assert_eq!(given.tm(), Compat::RandomPreferType);
        assert!(given.full_hm());
        assert_eq!(
            std::fs::read(&plan.preset).unwrap(),
            mine,
            "the player's file stays"
        );
        assert!(!dir.join("game.soulsync.rnqs").exists());

        std::fs::write(&plan.preset, "broken").unwrap();
        assert_eq!(run(&plan).unwrap_err().0, Why::Preset);
    }

    #[test]
    fn the_own_randomizer_takes_what_it_can_do() {
        let (dir, mut plan) = plan("own");
        plan.own = true;
        let soul_link = include_bytes!("../../soulsync-server/presets/soul-link.rnqs");
        std::fs::write(&plan.preset, soul_link).unwrap();
        let rom = |code: &[u8]| {
            let mut rom = b"POKEMON B2\0\0".to_vec();
            rom.extend_from_slice(code);
            rom.resize(64, 0);
            std::fs::write(&plan.original, rom).unwrap();
        };
        // A game nobody compared with the original randomizer.
        rom(b"IREO");
        assert_eq!(own(&plan), None);
        rom(b"IRED");
        assert!(own(&plan).is_some_and(|o| o.random_wild && o.full_hm));
        // No Java and no randomizer are asked for; this is no real ROM,
        // though, and what is there stays.
        plan.java = dir.join("no-such-java").into();
        std::fs::remove_file(&plan.randomizer).unwrap();
        assert_eq!(run(&plan).unwrap_err().0, Why::Randomizer);
        assert_eq!(std::fs::read_to_string(&plan.rom).unwrap(), "old game");
        assert_eq!(std::fs::read_to_string(&plan.save).unwrap(), "old save");

        // Without the choice of the own randomizer it is the original.
        plan.own = false;
        assert_eq!(own(&plan), None);
    }

    /// With a real German Black 2 at `SOULSYNC_ROM`:
    /// `cargo test --release -p soulsync-app -- --ignored a_real_game`.
    #[test]
    #[ignore = "needs the player's own ROM"]
    fn a_real_game_is_made_without_java() {
        let (dir, mut plan) = plan("real");
        plan.own = true;
        plan.original = std::env::var_os("SOULSYNC_ROM").unwrap().into();
        plan.java = dir.join("no-such-java").into();
        let soul_link = include_bytes!("../../soulsync-server/presets/soul-link.rnqs");
        std::fs::write(&plan.preset, soul_link).unwrap();
        run(&plan).unwrap();
        let original = std::fs::read(&plan.original).unwrap();
        let first = std::fs::read(&plan.rom).unwrap();
        assert!(gen5::verified(&first));
        assert_ne!(first, original);
        assert_eq!(std::fs::read_to_string(&plan.save).unwrap(), "start");
        // Every game is another one.
        run(&plan).unwrap();
        assert_ne!(std::fs::read(&plan.rom).unwrap(), first);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn seeds_differ() {
        let (a, b) = (seed(), seed());
        assert_ne!(a, b);
        assert!((0..1 << 48).contains(&a));
    }

    #[test]
    fn without_a_starting_save_the_game_begins_anew() {
        let (dir, mut plan) = plan("blank");
        plan.template = None;
        plan.java = fake_java(&dir, r#"echo new > "${10}""#);
        run(&plan).unwrap();
        assert!(!plan.save.exists());
        assert_eq!(
            std::fs::read_to_string(dir.join("Battery/game.dsv.bak")).unwrap(),
            "old save"
        );
    }

    #[test]
    fn rare_candies_need_a_real_save() {
        let (dir, mut plan) = plan("candies");
        plan.candies = 999;
        plan.java = fake_java(&dir, r#"echo new > "${10}""#);
        assert_eq!(run(&plan).unwrap_err().0, Why::Save);
        assert_eq!(std::fs::read_to_string(&plan.rom).unwrap(), "old game");
    }
}
