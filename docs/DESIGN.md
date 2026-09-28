# Vindictive — Design Notes

Vindictive is an always-on-top tile board that tells a researcher what to do
next. Tips live as Markdown files in a private GitHub repository; the app
polls them, renders each as a Windows 8 style live tile, and raises native
Windows toasts at remind times.

## Principles

1. **Glanceable.** The board is read from two metres away in under a second.
   One tile is highlighted as *next up*; everything else is context.
2. **Flat.** Metro, not Fluent. Solid colour blocks, no shadows, no
   gradients, no rounded corners, no borders. The one non-Microsoft thing
   is the typography: titles and the clock are set in a heavy Mincho-like
   serif, an Evangelion title-card gesture. Everything else is Segoe UI.
3. **Owned data.** Every tip is a plain `.md` file the user can edit anywhere.
   The app never invents a format that Obsidian or `cat` cannot read.
4. **Quiet.** Toasts fire only at times the user wrote down. New remote tips
   produce a single toast. Motion happens in response to the user (press,
   flip, done) or once on arrival; nothing loops except the sync dot while
   syncing.
5. **Bilingual.** Every UI string exists in English and Simplified Chinese;
   the font stack falls through to Microsoft YaHei / PingFang so CJK titles
   render without tofu.

## Visual language

| Token            | Value                                   |
| ---------------- | --------------------------------------- |
| UI typeface      | `Segoe UI Variable Text`, `Segoe UI`, `Microsoft YaHei UI`, `PingFang SC`, system-ui |
| Title typeface   | `Sitka Display`, Georgia, `Yu Mincho`, `SimSun`, serif at 700: tile titles, detail title, clock, capture input |
| Weights          | 700 titles, 400 body, 600 for the NEXT label |
| Board background | dark `#111113`, light `#F0EFEC`         |
| Text             | dark `#E8E6E1`, muted `#8F8D88`         |
| Edge             | white: next-up left edge, overdue top edge, focus ring |
| Tile unit        | `max(64px, (window width - padding - gaps) / columns)`: tiles fill the window |
| Tile sizes       | `sm` 1x1, `md` 2x2, `wide` 4x2          |
| Type scale       | every size is a `clamp()` on `vw` (panel, detail) or `cqh` (inside tiles), so text grows with the window |
| Title fit        | titles start at the `xl` tier (34 % of tile height, 2 lines) and step down to `lg` / default (3 lines) only if they overflow |
| Corner radius    | 0                                       |
| Motion           | see *Motion* below                      |

### Tile colours (NERV palette)

Desaturated on purpose: the board sits on the monitor all day. The original
Metro colours were too loud next to real work.

| Kind / state | Colour    | Reference |
| ------------ | --------- | --------- |
| task         | `#4A3B6B` | EVA-01 purple |
| event        | `#2C5F58` | deep teal |
| note         | `#3A4356` | slate |
| reading      | `#8C4A22` | burnt orange |
| deadline, later    | `#3F6B3A` | EVA-01 green |
| deadline, soon     | `#A87B1F` | amber |
| deadline, critical | `#9E2F2A` | EVA-02 red |
| any, overdue       | `#6E1B2B` with a 1 px orange top edge | |
| done         | `#26272B`, title struck through (only when "show done" is on) | |

A tip may override its colour with the `color` frontmatter key. Text is
white or near-black by luminance.

### Motion

| Moment | Effect |
| ------ | ------ |
| Tiles arrive (first render, new tip) | Metro entrance: slide in 28 px from the right while fading, 360 ms, staggered 30 ms per tile. Tiles already seen do not replay on the 30 s tick. |
| Tile pressed | Metro tilt: the tile leans up to 9 degrees toward the pointer (`--rx` / `--ry` from the press position) at scale 0.97, springs back on release, then the board flips. |
| Board flips | 240 ms `rotateY`; the back face's content slides in from the right. |
| Done pressed | strike-through draws across the title, panel slides out left, then the board flips back. |
| Figure tapped | lightbox fades in over the window (200 ms); Esc / click closes. |
| Menus, capture bar | fade + 6 px slide, 160-200 ms. |
| Syncing | the sync square pulses; weather glyph pulses while loading. |

All easing is Metro's exponential ease-out (`cubic-bezier(0.1, 0.9, 0.2, 1)`).

Everything honours `prefers-reduced-motion`.

### Tile anatomy

```
┌──────────────────────┐
│ ◷ 2d 4h        ▲     │  top row: countdown (deadlines) · kind glyph
│                      │
│                      │
│ Submit ECCE          │  bottom-left: title, 300 weight, up to 3 lines
│ camera-ready         │
└──────────────────────┘
```

- **Size rule.** `wide` when it is next-up; `md` for high priority or
  deadlines within 7 days; `sm` otherwise. Grid uses `grid-auto-flow: dense`.
- **Next-up** tile gets a 2 px white left edge and the label "NEXT" in the
  top row.
- **Countdown** text: `3w`, `5d`, `2d 4h`, `6h`, `45m`, `overdue 2h`
  (Chinese: `3周`, `2天4时`, `已逾期 2时`).
- **Figure ghost.** `md` and `wide` tiles show the tip's first `images`
  entry as a monochrome ghost in the right half, screen-blended into the
  tile colour, so a plot is recognisable from across the room.

### Detail view (tile flipped open)

The whole board flips to a single detail panel in the tile's colour:

1. Header block (tile colour): kind, title, countdown or due.
2. Meta rows: location · repeat · tags · snoozed until.
3. Paper block for arXiv / DOI tips: title, authors, venue, year.
4. Figures: every `images` entry as a framed thumbnail (png, jpg, svg, webp,
   gif; relative paths go through `fetch_image`, URLs load directly). Click
   opens the lightbox.
5. Body: rendered Markdown (sanitised). Inline `<svg>` is allowed (scripts,
   handlers, `foreignObject` and `style` are stripped). Body images also open
   the lightbox.
6. Links list.
7. Actions (flat text buttons): **Done** · **Snooze 1h** · **Tomorrow** ·
   **Edit on GitHub** · **Back**.

### Board chrome

- 20 px top strip, draggable (`data-tauri-drag-region`), contains a 6 px
  sync square (green ok, amber syncing, red error) and the VINDICTIVE mark;
  the last-sync time shows on hover.
- **Panel** (a flat surface block, draggable, `show_panel` setting): clock
  `HH:MM` with small seconds, date line (`2026-09-28 MON` / `2026年9月28日
  周一`), and weather on the right: glyph, temperature, place, high/low and
  description.
  Weather comes from Open-Meteo through the backend (`fetch_weather`), keyed
  by the `weather_location` setting; blank turns it off. The backend caches
  a result for 20 minutes.
- Right-click on the strip → context menu: Sync now, Settings, Show done,
  Dock left / right / free, Quit.
- Right-click on a tile → Done / Reopen, Snooze 1h, Tomorrow, Edit on
  GitHub, Delete… (a second menu confirms). The same actions live in the
  detail view; there Delete arms on the first click and fires on the second.
- The last tile on the board is a quiet "+" tile. It (or `n`, `+`, `Insert`)
  opens an inline new-tip bar above the grid with the quick-capture syntax.
  Enter creates the file; Esc closes the bar.
- Themes: `dark`, `light`, `nerv` (near-black, orange edge), `cobalt` (deep
  blue), `paper` (warm off-white). Themes change only the ground, text and
  edge tokens; tile colours are shared.
- **Drag to reorder.** Tiles are HTML5 draggable. Drop on the left half of a
  tile to land before it, the right half to land after, or on empty grid to
  go last. The moved tip gets `order` = midpoint of its new neighbours' sort
  keys (explicit `order`, else backend rank), so only that one file changes.
  Tips without `order` keep following the score ranking around it.
- **Tile section** in the detail view: size chips (Auto / Small / Medium /
  Wide → `size` key), 28 colour swatches and a native colour picker for any
  other hex (→ `color` key). Each pick is one commit.
- **Window opacity** (`window_opacity`, 20–100 %): the webview is
  transparent and every background (ground, strip, panel, faces, tiles,
  action bars) is mixed toward transparent by that amount with
  `color-mix`. Text, glyphs and edges stay fully opaque.
- **Startup size.** The board is sized to half the work area width and the
  full work area height (minus the 12 px edge margin) on every launch, then
  docked and shown. `tauri.conf.json` creates it hidden so the placeholder
  size never flashes.
- **Fit height** (`fit_height`, on by default): after every board render the
  window height is set to strip + panel + grid, clamped to the monitor's
  work area, and the board re-docks. Beyond that the board scrolls with a
  4 px scrollbar.
- **Edit locally** (detail view and tile menu) writes the tip's Markdown to
  `%APPDATA%\dev.zhangwentao.vindictive\edit\<repo path>`, opens it with
  the `editor_command` setting (`code` by default; blank means the system
  default app) and watches it every 2 s. Saves are collected as *pending*
  (count shown on the strip) and committed, one commit per file, every
  `push_interval_minutes` (default 60; 0 = at once) or when you pick
  **Commit & push local edits now** in the strip menu or the tray. After a
  push the file is rewritten in canonical form. If the tip changes elsewhere
  while the file is untouched, the file is refreshed. Sessions persist in
  `edits.json`, so pending edits survive a restart. A file that does not
  parse (mid-edit frontmatter) stays pending until the next save fixes it.
- Settings is an overlay in the same window, in three groups: GitHub (owner,
  repo, branch, dir, token in Windows Credential Manager, poll interval);
  appearance (language, theme, columns, weather city, panel, show done);
  window (hotkey, dock, always on top (off by default), autostart, notify on
  new tips).

### Capture window

Frameless 480×56 bar, centred on the active monitor, dark, one input, hint
text below showing the syntax. `Enter` creates, `Esc` hides. Shown by the
global hotkey (default `Ctrl+Shift+Space`).

## Data format

See `docs/TIP-FORMAT.md`. One file per tip under `tips/` in the data repo.

## Architecture

```
src/                 web frontend (vanilla TypeScript + Vite)
src-tauri/src/core   pure domain logic, unit-tested, no I/O
src-tauri/src/sync   GitHub Contents API, arXiv, Crossref, Open-Meteo
src-tauri/src/app    Tauri state, commands, scheduler, tray, windows
```

The backend owns the truth. Every mutation goes through a command that
returns the new `BoardState` and emits `board-updated`.
