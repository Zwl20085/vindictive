# Vindictive — Design Notes

Vindictive is an always-on-top tile board that tells a researcher what to do
next. Tips live as Markdown files in a private GitHub repository; the app
polls them, renders each as a Windows 8 style live tile, and raises native
Windows toasts at remind times.

## Principles

1. **Glanceable.** The board is read from two metres away in under a second.
   One tile is highlighted as *next up*; everything else is context.
2. **Flat.** Metro, not Fluent. Solid colour blocks, no shadows, no gradients,
   no rounded corners, no borders. Motion is the only decoration: tiles flip.
3. **Owned data.** Every tip is a plain `.md` file the user can edit anywhere.
   The app never invents a format that Obsidian or `cat` cannot read.
4. **Quiet.** Toasts fire only at times the user wrote down. New remote tips
   produce a single toast. Nothing pulses, bounces or nags.

## Visual language

| Token            | Value                                   |
| ---------------- | --------------------------------------- |
| Typeface         | `Segoe UI`, `Segoe UI Variable`, system-ui |
| Weights          | 300 for titles on tiles, 400 body, 600 for the next-up label |
| Board background | dark `#111214`, light `#F3F3F3`         |
| Tile unit        | 64 px square, 6 px gap                  |
| Tile sizes       | `sm` 1×1, `md` 2×2, `wide` 4×2          |
| Corner radius    | 0                                       |
| Motion           | 220 ms flip (`rotateY`) on open, 120 ms press scale 0.97 |

### Tile colours (Metro palette)

| Kind / state | Colour    |
| ------------ | --------- |
| task         | `#2D89EF` |
| event        | `#00ABA9` |
| note         | `#7E3878` |
| reading      | `#DA532C` |
| deadline, later    | `#1E7145` |
| deadline, soon     | `#FFC40D` (dark text) |
| deadline, critical | `#EE1111` |
| any, overdue       | `#B91D47` with a thin white top edge |
| done         | `#3A3A3D` (only when "show done" is on) |

A tip may override its colour with the `color` frontmatter key.

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
- **Countdown** text: `3w`, `5d`, `2d 4h`, `6h`, `45m`, `overdue 2h`.

### Detail view (tile flipped open)

The whole board flips to a single detail panel in the tile's colour:

1. Header block (tile colour): kind, title, countdown or due.
2. Meta rows: 📍 location · 🔁 repeat · tags.
3. Body: rendered Markdown (sanitised). Relative images resolved through
   `fetch_image`.
4. Links list. arXiv / DOI show the fetched paper title, authors, year.
5. Actions (flat text buttons): **Done** · **Snooze 1h** · **Tomorrow** ·
   **Edit on GitHub** · **Back**.

### Board chrome

- 20 px top strip, draggable (`data-tauri-drag-region`), contains a 6 px
  sync dot (green ok, amber syncing, red error) and the last-sync time on hover.
- Right-click on the strip → context menu: Sync now, Settings, Show done,
  Dock left / right / free, Quit.
- Settings is an overlay in the same window: owner, repo, branch, dir, token
  (password field, stored in Windows Credential Manager), poll interval,
  hotkey, columns, theme, autostart, notify on new tips.

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
src-tauri/src/sync   GitHub Contents API, arXiv, Crossref
src-tauri/src/app    Tauri state, commands, scheduler, tray, windows
```

The backend owns the truth. Every mutation goes through a command that
returns the new `BoardState` and emits `board-updated`.
