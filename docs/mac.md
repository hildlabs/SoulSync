# SoulSync on a Mac

SoulSync runs on macOS 11 or later, on Apple Silicon and Intel Macs alike.

**Status: not yet tested on a real Mac.** The app is built and signed by CI on
macOS, but nobody has played a run with it yet. Please
[report what breaks](https://github.com/hildlabs/SoulSync/issues).

## Why DeSmuME runs through Wine

The live sync needs DeSmuME's Lua scripting, and only the Windows build of
DeSmuME has it; the macOS build does not. On a Mac, SoulSync therefore starts
the Windows DeSmuME through [Wine](https://www.winehq.org). Everything else
works as on Windows: SoulSync puts `lua51.dll` next to DeSmuME, has DeSmuME
load the script by itself and opens your game with *Continue*.

Without Wine you can still play in any Mac emulator and pick its save
(`.dsv` or `.sav`) in SoulSync. The dashboard then only follows your in-game
saves: no live party, no encounters while you are in a battle, and *Make a
new game* is not available.

## Setting up

1. **Wine.** Download the latest `wine-stable-…-osx64.tar.xz` from
   [Gcenx/macOS_Wine_builds](https://github.com/Gcenx/macOS_Wine_builds/releases)
   (the WineHQ builds for macOS), unpack it and move *Wine Stable* to
   *Applications*. Then, in Terminal:

   ```sh
   xattr -dr com.apple.quarantine "/Applications/Wine Stable.app"
   # Apple Silicon only: Wine is an Intel program and needs Rosetta.
   softwareupdate --install-rosetta --agree-to-license
   ```

   SoulSync looks for Wine in *Applications* (Stable, Devel or Staging), in
   Homebrew's folders and on the `PATH`. `SOULSYNC_WINE` overrides that.
2. **DeSmuME for Windows.** Download the 64-bit Windows build from
   [desmume.org](https://desmume.org/download/) and unpack it into a folder of
   your own, such as `~/DeSmuME` (not *Applications*: DeSmuME writes its
   `Roms`, `Battery` and settings next to itself).
3. **SoulSync.** Download `SoulSync-mac.zip` from the
   [latest release](https://github.com/hildlabs/SoulSync/releases/latest),
   unpack it and move *SoulSync* to *Applications*.

   The app is not notarized by Apple, so macOS blocks it the first time.
   Double-click it once, close the warning, then open *System Settings ›
   Privacy & Security* and choose *Open Anyway* next to SoulSync. Or, in
   Terminal: `xattr -dr com.apple.quarantine /Applications/SoulSync.app`.
4. In SoulSync, choose *Make a new game*, pick your original ROM and, under
   *DeSmuME*, the `DeSmuME….exe` in your DeSmuME folder.

The first start of Wine sets itself up and takes a minute.

## Not there yet on a Mac

- The app does not update itself; download the new `SoulSync-mac.zip`.
- The original randomizer, used only for settings SoulSync's own cannot
  make yet, needs Java installed (SoulSync downloads Java only on Windows).
