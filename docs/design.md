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

| Class | What it is |
|---|---|
| `.glass` | Frosted panel. Add it to any new block of content. |
| `.chip` | 38 px pill in the top bar (code, update). |
| `.pair` | Two-option toggle (language, filters). `aria-pressed` marks the choice. |
| `.icon-btn` | 38 px square icon button. `aria-current=page` when its view is open. |
| `.tabbar` + `.pill` | Tabs with a sliding pill; call `movePill()` after layout changes. |
| `.play` | The one big call to action (Weiterspielen). One per screen at most. |
| `.sq` | 58 px square secondary action next to `.play`, with a `.tip`. |
| `.pcard` | A player's live card. `.fight` in battle, `.off` offline. |
| `.row` | A link: two sides, the route label `.loc` in the middle. |
| `.badge` | Octagon gym badge, `.on` when earned, `.pop` when just earned. |
| `.capbar` | Striped level-cap bar with the cap in pixel digits. |
| `.rblock` | A settings or rules section with an `h2`. |
| `.cmd` | A command line with a copy button. |
| `dialog.mon` | The Pokémon overview; one page, no tabs. |

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
| ≤ 1040 | The page scrolls; the sidebar becomes a strip at the top; panels stack |
| ≤ 720 | Phone: tabs scroll on their own row, compact rows and tables |

Check a change at 1920 × 1080, 1440 × 900, 1280 × 720, 1024 and 390 wide, in
both themes. `/demo` plays a scripted run that covers every view.

## Adding a view

1. Add its id to `VIEWS` (and to `TABS` if it gets a tab), plus `tabs.<id>` in
   both languages of `T`.
2. In `mount()`, add `<div class="view" data-view="<id>" hidden>${head("<id>")}…</div>`.
3. Write `update<Id>()` that fills it from `state`, and call it from `render()`.
4. Build its content from `.glass` panels and the components above. Write
   strings in English and German.
5. Check it in `/demo` at the sizes above.
