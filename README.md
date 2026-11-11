# SoulSync

Track Pokémon Soul Link runs live, straight from your emulator.

SoulSync reads both players' game state and shows it on one shared dashboard: linked pairs, encounters per route, deaths, level caps and your house rules. No manual bookkeeping. The goal is a single app you send to your partner: enter the run code, pick your own ROM, play.

> **Status:** early development. Black 2 / White 2 saves sync to a shared live dashboard every time a player saves in-game. Live emulator support (no saving needed) and the desktop app are next, see the [roadmap](docs/ROADMAP.md).

## Quick start

```sh
# 1. Start a server (or use one someone else runs)
soulsync-server                      # http://localhost:3000, set PORT to change

# 2. Create a run and share the code with your partner
soulsync new --server http://localhost:3000

# 3. Both players sync their DeSmuME battery file
soulsync sync Battery/SoulLink.dsv --code ZORUA-4821 --name Red --server http://localhost:3000
```

Open `http://localhost:3000/run/ZORUA-4821` to watch the run. The dashboard pairs both players' Pokémon by met location, marks links as linked, waiting or dead, and lists every catch, faint, evolution, level-up and badge.

## Other commands

```sh
# Show party, boxes and badges from a DeSmuME battery file
soulsync inspect Battery/SoulLink.dsv

# Watch the save: every in-game save reports catches, faints, evolutions, level-ups and badges
soulsync watch Battery/SoulLink.dsv

# Machine-readable output
soulsync inspect Battery/SoulLink.dsv --json
```

```
Trainer Red · 0 badges

Party (1)
  Party 1      Metang · Lv 5 · HP 21/21 · met at Aspertia City
```

Output is available in English and German. Use `--lang de`, or set `SOULSYNC_LANG=de`.

## Building

Requires [Rust](https://rustup.rs) 1.75 or newer.

```sh
cargo build --release
cargo test
```

## Project layout

| Crate | Purpose |
| --- | --- |
| [`soulsync-core`](crates/soulsync-core) | Generation 5 data model: PK5 encryption, save parsing, species/types/locations, events, Soul Link pairs, sync protocol |
| [`soulsync-server`](crates/soulsync-server) | Run server: run codes, WebSocket sync, web dashboard |
| [`soulsync-cli`](crates/soulsync-cli) | The `soulsync` command-line tool |

## License

[GPL-3.0-or-later](LICENSE). SoulSync does not include or distribute ROMs; every player uses their own.

Pokémon is a trademark of Nintendo, Creatures and GAME FREAK. SoulSync is an unofficial fan project.
