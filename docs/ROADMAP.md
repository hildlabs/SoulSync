# Fahrplan

SoulSync liest den Spielzustand **von außen** (Spielstand-Datei, später DeSmuME-Lua) statt einen Emulator nachzubauen oder zu forken. Jede Stufe ist für sich nutzbar.

| Stufe | Inhalt | Ergebnis |
| --- | --- | --- |
| 1 · Fundament | Rust-Workspace, Gen-5-Parser, CLI | `soulsync inspect` liest einen echten Spielstand ✅ |
| 2 · Sync per Speichern | `soulsync-server` mit Run-Code und WebSocket, Datei-Watcher, Web-Dashboard | Speichern im Spiel aktualisiert das Dashboard beider Spieler |
| 3 · Echt live | Lua-Skript für DeSmuME (Team, Boxen, Karte, Kampfstatus, Bag), Regel-Engine | Begegnungen und Tode erscheinen sekundenaktuell |
| 4 · Run-Assistent | Randomizer-Aufruf mit gemeinsamem Seed, ROM-Prüfung, Startpaket-Editor, DeSmuME-Start | Ein Klick vom Run-Code zum Spielstart |
| 5 · Installer | Tauri-App für Windows, später macOS/Linux, öffentlicher Standard-Server | Eine Datei, die man dem Kumpel schickt |
| 6 · Extras | Leaderboard, OBS-Overlay, weitere Spiele (S/W 1, Platin, HGSS), optional eingebetteter Emulator | |

## Run-Start (Ziel)

1. Spieler A erstellt einen Run: Spiel, Randomizer-Settings (`.rnqs`), Regeln. Der Server vergibt einen Code wie `ZORUA-4821` und einen Seed.
2. Spieler B gibt den Code ein und wählt nur seine eigene ROM.
3. SoulSync prüft den ROM-Header, randomisiert mit Settings und Seed und vergleicht den Fingerabdruck des Randomizer-Logs. Beide spielen dieselbe Welt.
4. Nach dem ersten Speichern trägt SoulSync das Startpaket (z. B. 999 Sonderbonbons) in den Spielstand ein.
5. SoulSync startet DeSmuME mit Lua-Skript. Ab hier läuft alles automatisch.

## Offene Punkte

- Nimmt Universal Pokemon Randomizer ZX 4.6.1 einen Seed per Kommandozeile? (UPR FVX kann es mit `-z`.)
- RAM-Adressen der deutschen und englischen Schwarz 2 / Weiß 2 für das Lua-Skript.
- Hosting des öffentlichen Servers.
