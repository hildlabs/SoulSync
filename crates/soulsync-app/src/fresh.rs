//! Starting a game over: a freshly randomized ROM and the save the player
//! prepared once, so that a failed run costs a click instead of the whole
//! setup. The randomizer is the player's own copy of Universal Pokémon
//! Randomizer ZX, run without its window.

use serde::Serialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Everything one fresh start needs.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The program that runs the randomizer, `java` unless a test says so.
    pub java: OsString,
    /// `PokeRandoZX.jar`.
    pub randomizer: PathBuf,
    /// The `.rnqs` with the player's randomizer settings.
    pub preset: PathBuf,
    /// The unmodified ROM.
    pub original: PathBuf,
    /// The ROM that is played; replaced.
    pub rom: PathBuf,
    /// The save that is played; replaced.
    pub save: PathBuf,
    /// The save every attempt starts from.
    pub template: PathBuf,
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
    /// The randomizer ran and left no ROM.
    Randomizer,
    /// A file could not be written.
    Write,
}

pub type Failure = (Why, Option<String>);

#[cfg(windows)]
fn quiet(command: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW: no console flashing up next to the app.
    command.creation_flags(0x0800_0000)
}

#[cfg(not(windows))]
fn quiet(command: &mut Command) -> &mut Command {
    command
}

/// Whether the program at `exe` is running. Only Windows is asked; elsewhere
/// an open emulator does not lock or overwrite anything on exit.
fn running(exe: &Path) -> bool {
    let Some(name) = exe.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !cfg!(windows) {
        return false;
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

/// Randomizes the ROM anew and puts the starting save in place.
pub fn run(plan: &Plan) -> Result<(), Failure> {
    let files = [
        ("original", &plan.original),
        ("randomizer", &plan.randomizer),
        ("preset", &plan.preset),
        ("template", &plan.template),
    ];
    for (what, path) in files {
        if !path.is_file() {
            return Err((Why::Missing, Some(what.to_owned())));
        }
    }
    if plan.original == plan.rom {
        return Err((Why::Missing, Some("original".to_owned())));
    }
    if plan.emulator.as_deref().is_some_and(running) {
        return Err((Why::Running, None));
    }

    // The randomizer writes next to the ROM first: a run that fails leaves
    // the game that is there untouched.
    let fresh = plan.rom.with_extension("new.nds");
    let _ = std::fs::remove_file(&fresh);
    let mut command = Command::new(&plan.java);
    command.arg("-Xmx4096M").arg("-jar").arg(&plan.randomizer);
    command.arg("cli").arg("-s").arg(&plan.preset);
    command.arg("-i").arg(&plan.original);
    command.arg("-o").arg(&fresh).arg("-l");
    if let Some(dir) = plan.randomizer.parent().filter(|d| d.is_dir()) {
        command.current_dir(dir);
    }
    let output = quiet(&mut command)
        .output()
        .map_err(|e| (Why::Java, Some(e.to_string())))?;
    if !fresh.is_file() {
        return Err((Why::Randomizer, tail(&output)));
    }

    let write = |e: std::io::Error| (Why::Write, Some(e.to_string()));
    std::fs::rename(&fresh, &plan.rom).map_err(write)?;
    // The randomizer's log tells what it did; keep it next to the ROM as
    // the randomizer itself would.
    let _ = std::fs::rename(with_suffix(&fresh, ".log"), with_suffix(&plan.rom, ".log"));
    if let Some(dir) = plan.save.parent() {
        std::fs::create_dir_all(dir).map_err(write)?;
    }
    std::fs::copy(&plan.template, &plan.save).map_err(write)?;
    if let Some(encounters) = &plan.encounters {
        let _ = std::fs::remove_file(encounters);
    }
    Ok(())
}

/// Opens the game in the emulator.
pub fn play(emulator: &Path, rom: &Path) {
    let mut command = Command::new(emulator);
    command.arg(rom);
    if let Some(dir) = emulator.parent() {
        command.current_dir(dir);
    }
    let _ = command.spawn();
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A stand-in for `java` that writes what it was asked to.
    fn fake_java(dir: &Path, body: &str) -> OsString {
        let path = dir.join("java");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
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
            java: "java".into(),
            randomizer: dir.join("rando.jar"),
            preset: dir.join("mine.rnqs"),
            original: dir.join("original.nds"),
            rom: dir.join("game.nds"),
            save: dir.join("Battery/game.dsv"),
            template: dir.join("start.dsv"),
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

        std::fs::remove_file(&plan.template).unwrap();
        assert_eq!(
            run(&plan).unwrap_err(),
            (Why::Missing, Some("template".to_owned()))
        );
    }
}
