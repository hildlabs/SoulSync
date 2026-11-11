# Roadmap

SoulSync reads game state **from the outside** (save files, then DeSmuME Lua) instead of reimplementing or forking an emulator. Each stage is useful on its own.

| Stage | Scope | Outcome |
| --- | --- | --- |
| 1. Foundation | Rust workspace, generation 5 save parser, CLI | `soulsync inspect` reads a real save ✅ |
| 2. Sync on save | Run server with run codes and WebSocket, save watcher, web dashboard | Saving in-game updates both players' dashboards |
| 3. Live | DeSmuME Lua script (party, boxes, map, battle state, bag), rule engine | Encounters and deaths show up within seconds |
| 4. Run setup | Randomizer with a shared seed, ROM check, starter-kit save editor, DeSmuME launcher | One click from run code to playing |
| 5. Installer | Desktop app (Tauri) for Windows, then macOS/Linux, public default server | One file to send to your partner |
| 6. Extras | Leaderboard, OBS overlay, more games (Black/White, Platinum, HGSS), optional embedded emulator | |

## Starting a run (target flow)

1. Player A creates a run: game, randomizer settings (`.rnqs`), rules. The server hands out a code like `ZORUA-4821` and a seed.
2. Player B enters the code and picks only their own ROM.
3. SoulSync checks the ROM header, randomizes with the shared settings and seed, and compares a fingerprint of the randomizer log so both play the same world.
4. After the first in-game save, SoulSync adds the starter kit (for example 999 Rare Candies) to the save.
5. SoulSync launches DeSmuME with the Lua script. From here everything updates automatically.

## Open questions

- Does Universal Pokémon Randomizer ZX 4.6.1 accept a seed on the command line? (UPR FVX does, with `-z`.)
- RAM addresses for the Lua script across game languages.
- Hosting for the public server.
