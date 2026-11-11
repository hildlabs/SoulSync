//! `soulsync`: read and watch Pokémon save files.

mod i18n;

use i18n::{text, Text};
use soulsync_core::{diff, parse_b2w2, Event, Lang, Place, Pokemon, SaveSnapshot};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

struct Opts {
    json: bool,
    lang: Lang,
}

fn env_lang() -> Lang {
    ["SOULSYNC_LANG", "LC_ALL", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .map_or(Lang::En, |v| {
            v.parse().unwrap_or_else(|_| Lang::from_locale(&v))
        })
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut pos = Vec::new();
    let mut opts = Opts {
        json: false,
        lang: env_lang(),
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--json" => opts.json = true,
            "--lang" => match args.next().map(|v| v.parse::<Lang>()) {
                Some(Ok(l)) => opts.lang = l,
                Some(Err(e)) => {
                    eprintln!("{}: {e}", text(opts.lang).error);
                    return ExitCode::from(2);
                }
                None => pos.push(a),
            },
            _ => pos.push(a),
        }
    }
    let t = text(opts.lang);
    let result = match (pos.first().map(String::as_str), pos.get(1)) {
        (Some("inspect"), Some(p)) => inspect(Path::new(p), &opts),
        (Some("watch"), Some(p)) => watch(PathBuf::from(p), &opts),
        _ => {
            eprintln!("{}", t.usage);
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}: {e}", t.error);
            ExitCode::FAILURE
        }
    }
}

type Res<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn load(path: &Path) -> Res<SaveSnapshot> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(parse_b2w2(&data)?)
}

fn inspect(path: &Path, o: &Opts) -> Res {
    let snap = load(path)?;
    if o.json {
        println!("{}", serde_json::to_string_pretty(&snap)?);
        return Ok(());
    }
    let t = text(o.lang);
    println!(
        "{} {} · {} {}",
        t.trainer, snap.trainer.name, snap.trainer.badges, t.badges
    );
    println!("\n{} ({})", t.party, snap.party.len());
    for p in &snap.party {
        println!("  {}", line(p, o.lang, t));
    }
    println!("\n{} ({})", t.boxes, snap.boxes.len());
    for p in &snap.boxes {
        println!("  {}", line(p, o.lang, t));
    }
    Ok(())
}

fn line(p: &Pokemon, lang: Lang, t: &Text) -> String {
    let species = p.species_label(lang);
    let name = if p.is_nicknamed {
        format!("{} ({species})", p.nickname)
    } else {
        species
    };
    let place = match p.place {
        Place::Party { slot } => format!("{} {}", t.slot_party, slot + 1),
        Place::Box { r#box, slot } => format!("{} {box} / {}", t.slot_box, slot + 1),
    };
    let stats = p
        .battle
        .as_ref()
        .map(|b| {
            format!(
                " · {} {} · {} {}/{}",
                t.level, b.level, t.hp, b.hp, b.max_hp
            )
        })
        .unwrap_or_default();
    let met = p.met_location_name.as_ref().map_or_else(
        || format!("{} {}", t.location, p.met_location),
        |n| n.get(lang).to_owned(),
    );
    let egg = if p.is_egg {
        format!(" · {}", t.egg)
    } else {
        String::new()
    };
    format!("{place:<12} {name}{stats} · {} {met}{egg}", t.met)
}

fn describe(e: &Event, lang: Lang, t: &Text) -> String {
    match e {
        Event::Caught { pokemon } => format!("{}: {}", t.ev_caught, line(pokemon, lang, t)),
        Event::Fainted { pokemon } => format!("{}: {}", t.ev_fainted, pokemon.label(lang)),
        Event::Evolved { pokemon, from } => format!(
            "{}: {} → {}",
            t.ev_evolved,
            soulsync_core::dex::species(*from).map_or("?", |s| s.name.get(lang)),
            pokemon.species_label(lang)
        ),
        Event::LevelUp { pokemon } => format!(
            "{} {} {}",
            pokemon.label(lang),
            t.ev_level,
            pokemon.battle.as_ref().map_or(0, |b| b.level)
        ),
        Event::Moved { pokemon } => match pokemon.place {
            Place::Party { .. } => format!("{} {}", pokemon.label(lang), t.ev_to_party),
            Place::Box { r#box, .. } => format!("{} {} {box}", pokemon.label(lang), t.ev_to_box),
        },
        Event::Gone { pokemon } => format!("{}: {}", t.ev_gone, pokemon.label(lang)),
        Event::Badges { count } => format!("{}: {count}", t.ev_badges),
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn watch(path: PathBuf, o: &Opts) -> Res {
    let t = text(o.lang);
    let mut last = load(&path)?;
    let mut stamp = modified(&path);
    eprintln!(
        "{}",
        t.watching
            .replace("{path}", &path.display().to_string())
            .replace("{party}", &last.party.len().to_string())
            .replace("{boxes}", &last.boxes.len().to_string())
    );
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let now = modified(&path);
        if now == stamp {
            continue;
        }
        stamp = now;
        // DeSmuME does not write the file atomically: give it a moment, and
        // if the read still fails, try again on the next tick.
        std::thread::sleep(Duration::from_millis(300));
        let Ok(snap) = load(&path) else {
            stamp = None;
            continue;
        };
        for e in diff(&last, &snap) {
            if o.json {
                println!("{}", serde_json::to_string(&e)?);
            } else {
                println!("{}", describe(&e, o.lang, t));
            }
        }
        last = snap;
    }
}
