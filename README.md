# SoulSync

Track Pokémon Soul Link runs live, straight from your emulator.

SoulSync reads both players' game state and shows it on one shared dashboard: linked pairs, encounters per route, deaths, level caps and your house rules. No manual bookkeeping. The goal is a single app you send to your partner: enter the run code, pick your own ROM, play.

> **Status:** early development. The save parser for Black 2 / White 2 and a command-line tool work today. Sync, dashboard and live emulator support are next, see the [roadmap](docs/ROADMAP.md).

## Usage

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
| [`soulsync-core`](crates/soulsync-core) | Generation 5 data model: PK5 encryption, save parsing, species/types/locations, events from comparing two saves |
| [`soulsync-cli`](crates/soulsync-cli) | The `soulsync` command-line tool |

## License

[GPL-3.0-or-later](LICENSE). SoulSync does not include or distribute ROMs; every player uses their own.

Pokémon is a trademark of Nintendo, Creatures and GAME FREAK. SoulSync is an unofficial fan project.
