# SoulSync

**Soul-Link-Runs live verfolgen, ohne etwas von Hand einzutragen.**
*Track Pokémon Soul Link runs live, straight from your emulator. Open source, written in Rust.*

SoulSync liest den Spielzustand beider Spieler direkt aus dem Spiel und zeigt ihn in einem gemeinsamen Dashboard: Teams als Link-Paare, Begegnungen pro Route, Tode, Level-Cap und eure Regeln. Ziel ist ein Tool, das man seinem Kumpel schickt: Run-Code eingeben, eigene ROM wählen, losspielen.

> **Stand:** frühe Entwicklung. Aktuell gibt es den Spielstand-Parser für Schwarz 2 / Weiß 2 und eine Kommandozeile. Sync, Dashboard und Live-Anbindung folgen, siehe [Fahrplan](docs/ROADMAP.md).

## Was schon geht

```sh
# Team, Boxen und Orden aus der DeSmuME-Batteriedatei lesen
soulsync inspect "Battery/SoulLink.dsv"

# Spielstand beobachten: jedes Speichern im Spiel meldet Fänge, Tode, Level-ups, Orden
soulsync watch "Battery/SoulLink.dsv"

# Dasselbe maschinenlesbar
soulsync inspect "Battery/SoulLink.dsv" --json
```

Beispielausgabe:

```
Trainer Hannes · 0 Orden

Team (1)
  Team 1       Metang · Lv 5 · KP 21/21 · gefangen: Eventura City
```

## Bauen

Voraussetzung: [Rust](https://rustup.rs) 1.75 oder neuer.

```sh
cargo build --release
cargo test
./target/release/soulsync inspect <spielstand>
```

## Aufbau

| Crate | Aufgabe |
| --- | --- |
| `soulsync-core` | Gen-5-Datenmodell: PK5 ver-/entschlüsseln, Spielstand lesen, Artnamen/Typen/Orte, Ereignisse aus dem Vergleich zweier Spielstände |
| `soulsync-cli` | Kommandozeile `soulsync` |

Geplant: `soulsync-bridge` (DeSmuME-Lua, Datei-Watcher), `soulsync-save` (Startpaket wie 999 Sonderbonbons eintragen), `soulsync-rando` (Universal Pokemon Randomizer mit gemeinsamem Seed), `soulsync-server` (Lobby, Run-Code, Sync, Leaderboard) und eine Tauri-Desktop-App. Details in [docs/ROADMAP.md](docs/ROADMAP.md).

## ROMs und Spielstände

SoulSync enthält und verteilt keine ROMs. Jeder Spieler nutzt seine eigene. ROMs, Spielstände und Randomizer-Logs gehören nicht ins Repo (siehe `.gitignore`) und werden nie auf einen Server geladen.

## Lizenz

[GPL-3.0-or-later](LICENSE). Pokémon ist eine Marke von Nintendo, Creatures und GAME FREAK. SoulSync ist ein inoffizielles Fanprojekt.
