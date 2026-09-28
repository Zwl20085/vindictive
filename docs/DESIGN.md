# Vindictive — Design Notes

Vindictive is an always-on-top tile board that tells a researcher what to do
next. Tips live as Markdown files in a private GitHub repository; the app
polls them, renders each as a Windows 8 style live tile, and raises native
Windows toasts at remind times.

## Principles

1. **Glanceable.** The board is read from two metres away in under a second.
   One tile is highlighted as *next up*; everything else is context.
2. **Flat.** Solid colour blocks, no shadows, no gradients, no rounded
   corners. The look is NERV, not Fluent: near-black ground, one orange
   accent, thin HUD corner brackets, condensed uppercase labels.
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
| Body typeface    | `Segoe UI Variable Text`, `Segoe UI`, `Microsoft YaHei UI`, `PingFang SC`, system-ui |
| Display typeface | `Bahnschrift SemiCondensed` (labels, clock, NEXT chip, buttons), same CJK fallbacks |
| Weights          | 300 for titles on tiles, 400 body, 600 for the next-up label |
| Board background | dark `#0C0B10` (violet-biased black), light `#ECE8E0` |
| Text             | dark `#E6E1D8` warm off-white, muted `#8A857D` |
| Accent           | NERV orange `#E0762B` (light theme `#C8611C`): next-up edge, strip title, clock, primary action |
| Tile unit        | 64 px square, 6 px gap                  |
| Tile sizes       | `sm` 1×1, `md` 2×2, `wide` 4×2          |
| Corner radius    | 0                                       |
| Decoration       | 7 px corner brackets on every tile at 35 % opacity, faint scanlines over the stage, caution hatch on the strip |
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
| Tiles arrive (first render, new tip) | fade + 8 px rise, 380 ms, staggered 32 ms per tile. Tiles already seen do not replay on the 30 s tick. |
| Tile clicked | 160 ms press: scale 0.96 with a white flash, then the flip. |
| Board flips | 260 ms `rotateY`; one orange scan line sweeps down the back face. |
| Done pressed | strike-through draws across the title, panel slides out left, then the board flips back. |
| Figure tapped | lightbox fades in over the window (180 ms); Esc / click closes. |
| Syncing | the sync square pulses; weather glyph pulses while loading. |

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
- **Next-up** tile gets a 2 px orange left edge and an orange "NEXT" chip
  in the top row.
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
  sync square (green ok, amber syncing, red error), the VINDICTIVE mark in
  orange and a caution hatch; the last-sync time shows on hover.
- **Panel** (52 px, draggable, `show_panel` setting): clock `HH:MM` with
  small seconds, date line (`2026-09-28 MON` / `2026年9月28日 周一`), and
  weather on the right: glyph, temperature, place, high/low and description.
  Weather comes from Open-Meteo through the backend (`fetch_weather`), keyed
  by the `weather_location` setting; blank turns it off. The backend caches
  a result for 20 minutes.
- Right-click on the strip → context menu: Sync now, Settings, Show done,
  Dock left / right / free, Quit.
- Settings is an overlay in the same window, in three groups: GitHub (owner,
  repo, branch, dir, token in Windows Credential Manager, poll interval);
  appearance (language, theme, columns, weather city, panel, show done);
  window (hotkey, dock, always on top, autostart, notify on new tips).

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
