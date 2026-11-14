# Dashboard design

The dashboard (`crates/soulsync-server/web/index.html`) looks like a game
launcher: a pixel-art night scene behind frosted glass panels, two player
colours, and big pixel numbers. These rules keep new features in that style.

## Principles

1. **One screen, no hunting.** The overview answers "where are we?" without a
   click: attempt, badges, level cap, who is playing what, links, feed.
2. **Two players, two colours.** Player A is orange (`--a`), player B is blue
   (`--b`). Players 3 and 4 use `--p3` and `--p4`. Anything that belongs to a
   player carries its colour as `--c` on the element.
3. **Glass over the scene.** Content sits on `.glass` panels. Never put text
   straight onto the scene.
4. **Numbers are pixels.** Levels, counts, percentages and the badge total use
   `--pix` (Jersey 10). Codes and commands use `--mono`. Everything else uses
   `--sans` (Geist).
5. **Motion explains state.** Things move when they change (a catch, a lost
   link, a badge), not to decorate. Every animation respects
   `prefers-reduced-motion`.
6. **No game art in the repo.** Sprites and item icons come from the player's
   own ROM at runtime (`/api/local/sprite/{id}`, `/api/local/item/{id}`). The
   web build falls back to the public PokéAPI sprites.

## Tokens

All colours, radii and curves are custom properties on `:root`. Components use
only tokens, never raw colours, so the light theme works for free.

| Group | Tokens | Use |
|---|---|---|
| Surfaces | `--glass`, `--glass-hi`, `--solid`, `--dlg-bg`, `--toast` | Panels, selected panel, opaque fills, dialog, tooltips |
| Overlays | `--ov-0` … `--ov-3` | Tints on glass, from faint to strong (rows, chips, pressed) |
| Borders | `--edge`, `--edge-hi` | Hairlines, hover and selected borders |
| Text | `--text`, `--muted`, `--faint` | Body, secondary, labels and timestamps |
| Players | `--a`, `--b`, `--p3`, `--p4` | Player colour, passed on as `--c` |
| Status | `--ok`, `--warn`, `--danger`, `--up`, `--dn` | HP bars, catch chance, nature arrows, errors |
| Scene | `--sky1…4`, `--hill1…3`, `--ground`, `--moon`, `--stars`, `--shade` | Background only |
| Type | `--sans`, `--mono`, `--pix` | See principle 4 |
| Radius | `--r-sm` 9, `--r-md` 13, `--r-lg` 18 | Inputs and small chips, buttons and cards, panels |
| Control height | `--ctl` 44 | Everything in the top bar: tabs, chips, toggles, avatars, icon buttons |
| Motion | `--out` (ease out), `--spring` (overshoot) | Entrances and hovers, pops and pills |

Dark is the default. Light is defined twice with the same values: under
`@media (prefers-color-scheme: light)` for `:root:not([data-theme=dark])`, and
under `:root[data-theme=light]`. When you add a token, add it to all three
blocks. The choice is stored in `localStorage` under `soulsync.theme`.

## Layout

- `.shell`: sidebar (runs, new run, source link) and `.main`.
- `.main`: `.top` (tabs, status, code, avatars, language, theme, settings),
  then one `.view` per tab.
- The overview is `.hero` (title, badges card) above `.hpanel`, a glass panel
  split into columns `.col.live`, `.col.links`, `.col.feedc`.
- Columns scroll on their own at desktop sizes; the page itself never scrolls
  above 1040 px.

## Components

Every control in the top bar is exactly `--ctl` high (and icon buttons
`--ctl` wide), so the bar lines up. A new top-bar control uses `--ctl` too.

| Class | What it is |
|---|---|
| `.glass` | Frosted panel. Add it to any new block of content. |
| `.chip` | Pill in the top bar (code, update). |
| `.pair` | Two-option toggle (language, filters). `aria-pressed` marks the choice. |
| `.icon-btn` | Square icon button. `aria-current=page` when its view is open. |
| `.tabbar` + `.pill` | Tabs with a sliding pill; call `movePill()` after layout changes. |
| `.play` | The one big call to action (Weiterspielen). One per screen at most. |
| `.sq` | 58 px square secondary action next to `.play`, with a `.tip`. |
| `.pcard` | A player's live card. `.fight` in battle, `.off` offline. |
| `.row` | A link: two sides, the route label `.loc` in the middle. |
| `.badge` | Octagon gym badge, `.on` when earned, `.pop` when just earned. |
| `.capbar` | Striped level-cap bar with the cap in pixel digits. |
| `.rblock` | A settings or rules section with an `h2`. |
| `.cmd` | A command line with a copy button. |
| `dialog.rdlg` | The randomizer settings: groups on the left (`.rnav`), the rows of one group in two columns on the right (`.ro` rows in `.rdc`, held together as `.rblk` blocks: an option with what hangs on it, or plain switches in a row), a search over all groups, and a footer. The page itself only shows `randoHtml()`: a button, how much is changed, and the changes as `.rchip`. Options that only matter with another one stand indented under it and show only then. |
| `.sw` | A switch as a button with `role=switch`; looks like the checkboxes of the rules. |
| `dialog.rdlg.rules` | The rules of a new run in the app: the same window, narrow and as high as its list. The setup page only names the rules that are on (`rulesField()`), so that it fits a screen without scrolling; "My runs" is a third tab there, not a card below. |
| `.rn` | A number in a row of the randomizer settings: minus, the value to type over, plus; its unit stands inside, muted. |
| `dialog.mon` | The Pokémon overview, one screen without scrolling: a head (sprite, name, HP, link partners), then three columns: facts, moves as a 2×2 grid of `.mvc` cards with the stats under them, and level-up moves with the next one on its own `.lsn` card. |

Type badges come from `tyChip(type)`, HP from `hpLine(hp, max)`, sprites from
`spriteImg(species)` and held items from `itemImg(item)`. Reuse them.

## Motion

- Entrances: `.reveal` (rise and fade, staggered by `--i`), 0.7 s, `--out`.
- Views fade in with the Web Animations API in `showView()`. Do not give a
  `.view` or any ancestor of a `.glass` panel a CSS animation that stays
  applied: a finished CSS animation stops `backdrop-filter` from blurring.
  `scene()` removes finished `.reveal` animations for the same reason.
- Hovers: lift 1–2 px, 0.2–0.3 s.
- State changes: `pop` for new things, `snap` for losses, `ping` for live dots.
- Keep loops (`sheen`, `shine`, embers) slow and quiet.

## Window sizes

1920 × 1080 is the reference. Breakpoints in the stylesheet:

| Width | Change |
|---|---|
| ≥ 1700 | Wider sidebar and padding |
| ≤ 1499 | Overview in two columns (feed under live); status text and language toggle leave the top bar (language stays in Settings) |
| ≤ 1240 | Hero in one column, dialog header stacks |
| ≤ 1040 | The page scrolls; the sidebar becomes a strip at the top; overview sections become cards in one column; the top bar stays on one line (tabs scroll, live dot hides) |
| ≤ 720 | Phone: tabs scroll on their own row, compact rows and tables |

Check a change at 1920 × 1080, 1440 × 900, 1280 × 720, 1024 and 390 wide, in
both themes. `/demo` plays a scripted run that covers every view.

## Overview sections

Every viewer can fold, hide and reorder the overview's sections (badges,
live, links, feed). "Customise" next to the run chips turns on edit mode.

- A section is an element with `class="blk"` and `data-b="<id>"`. Put
  `${layBar("<id>")}` in its heading. That adds the fold chevron, plus the
  move arrows and the eye button that only show in edit mode (`.laying`).
- `applyLayout()` sets `.folded`, `.off` and `style.order`, and writes the
  grid areas of `.hpanel` from the visible columns. A folded column shrinks
  to its heading. Hidden sections stay visible, dimmed, while editing.
- The choice is stored in `localStorage` under `soulsync.layout` as
  `{order, hidden, folded}`. "Reset" clears it.
- At ≤ 1040 px the hero and panel use `display:contents`, so all sections
  become cards in one column and the order applies to all of them. The
  default order there is links, live, badges, feed. Above that, badges stay
  in the hero and only the three columns move.
- To add a section, add its id to `BLOCKS` and to the narrow default order
  in `layOrder()`.

## Adding a view

1. Add its id to `VIEWS` (and to `TABS` if it gets a tab), plus `tabs.<id>` in
   both languages of `T`.
2. In `mount()`, add `<div class="view" data-view="<id>" hidden>${head("<id>")}…</div>`.
3. Write `update<Id>()` that fills it from `state`, and call it from `render()`.
4. Build its content from `.glass` panels and the components above. Write
   strings in English and German.
5. Check it in `/demo` at the sizes above.
