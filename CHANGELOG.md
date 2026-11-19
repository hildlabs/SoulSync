# Changelog

What changed for players in each version. A release takes its patch notes
(on GitHub and in the Discord's #announcements) from the section with its
version number, or else from **Unreleased**. If both are empty, it lists the
titles of the pull requests merged since the last version instead.

Write entries for players, not for developers: what they notice in the app.

## Unreleased

<!-- One line per change, newest first, for example:
### New
- Catch odds now count the Dusk Ball at night.
### Fixed
- The run list no longer jumps when a partner joins.
-->

### New
- The dashboard is now also available in Spanish (ES button next to EN, DE and FR). Pokémon and place names fall back to English for now.

## 0.3.8

### New
- Only the players of a run can change it now. Someone who knows the code, for example from a stream, can still watch the dashboard, but can no longer start a new attempt, end the run or play under your name. Your app keeps its secret in its settings, so play each run from the same computer.
- The dashboard is now also available in French (FR button next to EN and DE). Pokémon and place names fall back to English for now.
- The command-line tool now speaks Spanish: use `--lang es`, or set `SOULSYNC_LANG=es`.

### Fixed
- Losing with your whole party now ends the attempt. Only the first fight against your rival may be lost with just the starter.

## 0.3.7

### New
- A new game is in the run from the start: you show as online when the game opens, and your name, game and place appear as soon as you can walk around, before the starter and without saving.
- The 999 Rare Candies are in your bag right after the intro. Saving and restarting is no longer needed.
- In a wild battle, the card says "Do not catch" when the area is used up for you or the link there is already lost.

### Fixed
- Dashboard accessibility: Pokémon buttons, option lists, the language buttons and dialogs now have names for screen readers, secondary text has more contrast, and the jump to the run settings no longer scrolls smoothly with reduced motion.
- Running from a wild Pokémon now counts as a missed encounter right away, not only after the next battle.
- A starting save made in the other version of the game no longer shows the wrong game in the run.
- The empty save file DeSmuME creates for a new game no longer shows up as a nameless player or as an error.
