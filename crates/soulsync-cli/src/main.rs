//! `soulsync` – Spielstände lesen und beobachten.
//!
//! soulsync inspect <spielstand> [--json]
//! soulsync watch   <spielstand> [--json]

use soulsync_core::{diff, parse_b2w2, Event, Place, Pokemon, SaveSnapshot};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

const USAGE: &str = "\
SoulSync – Soul-Link-Runs live verfolgen

Benutzung:
  soulsync inspect <spielstand> [--json]   Team, Boxen und Orden anzeigen
  soulsync watch   <spielstand> [--json]   Spielstand beobachten und Änderungen melden

<spielstand> ist die .dsv aus DeSmuME (Ordner Battery) oder eine .sav.
Unterstützt: Pokémon Schwarz 2 / Weiß 2.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let result = match (pos.first().map(|s| s.as_str()), pos.get(1)) {
        (Some("inspect"), Some(p)) => inspect(Path::new(p), json),
        (Some("watch"), Some(p)) => watch(PathBuf::from(p), json),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Fehler: {e}");
            ExitCode::FAILURE
        }
    }
}

type Res = Result<(), Box<dyn std::error::Error>>;

fn load(path: &Path) -> Result<SaveSnapshot, Box<dyn std::error::Error>> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(parse_b2w2(&data)?)
}

fn inspect(path: &Path, json: bool) -> Res {
    let snap = load(path)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&snap)?);
        return Ok(());
    }
    println!(
        "Trainer {} · {} Orden",
        snap.trainer.name, snap.trainer.badges
    );
    println!("\nTeam ({})", snap.party.len());
    for p in &snap.party {
        println!("  {}", line(p));
    }
    println!("\nBoxen ({})", snap.boxes.len());
    for p in &snap.boxes {
        println!("  {}", line(p));
    }
    Ok(())
}

fn line(p: &Pokemon) -> String {
    let name = p.species_de.as_deref().unwrap_or("?");
    let nick = if p.is_nicknamed {
        format!("{} ({name})", p.nickname)
    } else {
        name.to_string()
    };
    let place = match p.place {
        Place::Party { slot } => format!("Team {}", slot + 1),
        Place::Box { r#box, slot } => format!("Box {box} / {}", slot + 1),
    };
    let stats = p
        .battle
        .as_ref()
        .map(|b| format!(" · Lv {} · KP {}/{}", b.level, b.hp, b.max_hp))
        .unwrap_or_default();
    let met = p
        .met_location_de
        .as_deref()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Ort {}", p.met_location));
    format!(
        "{place:<12} {nick}{stats} · gefangen: {met}{}",
        if p.is_egg { " · Ei" } else { "" }
    )
}

fn describe(e: &Event) -> String {
    match e {
        Event::Caught { pokemon } => format!("Neu: {}", line(pokemon)),
        Event::Fainted { name, .. } => format!("Besiegt (0 KP): {name}"),
        Event::Evolved { from, to, .. } => format!(
            "Entwicklung: {} → {}",
            soulsync_core::dex::species(*from).map_or("?", |s| s.name_de.as_str()),
            soulsync_core::dex::species(*to).map_or("?", |s| s.name_de.as_str())
        ),
        Event::LevelUp { name, level, .. } => format!("{name} ist jetzt Lv {level}"),
        Event::Moved { name, to, .. } => match to {
            Place::Party { .. } => format!("{name} ist jetzt im Team"),
            Place::Box { r#box, .. } => format!("{name} liegt jetzt in Box {box}"),
        },
        Event::Gone { name, .. } => format!("Nicht mehr im Spielstand: {name}"),
        Event::Badges { count } => format!("Orden: {count}"),
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn watch(path: PathBuf, json: bool) -> Res {
    let mut last = load(&path)?;
    let mut stamp = modified(&path);
    eprintln!(
        "Beobachte {} ({} im Team, {} in Boxen). Speichere im Spiel, um Änderungen zu sehen. Strg+C beendet.",
        path.display(),
        last.party.len(),
        last.boxes.len()
    );
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let now = modified(&path);
        if now == stamp {
            continue;
        }
        stamp = now;
        // DeSmuME schreibt die Datei nicht atomar; kurz warten und bei Fehler beim nächsten Mal erneut lesen.
        std::thread::sleep(Duration::from_millis(300));
        let Ok(snap) = load(&path) else {
            stamp = None;
            continue;
        };
        for e in diff(&last, &snap) {
            if json {
                println!("{}", serde_json::to_string(&e)?);
            } else {
                println!("{}", describe(&e));
            }
        }
        last = snap;
    }
}
