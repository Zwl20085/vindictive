<p align="center">
  <img src="docs/assets/hero.svg" alt="Vindictive: a dark always-on-top board of flat coloured tiles, one marked NEXT, with a Windows toast in the corner" width="100%">
</p>

<h1 align="center">Vindictive</h1>

<p align="center">
  <strong>An always-on-top tile board that tells you what to do next. Synced with GitHub.</strong>
</p>

<p align="center">
  <a href="https://github.com/Zwl20085/vindictive/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Zwl20085/vindictive/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/Zwl20085/vindictive/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/Zwl20085/vindictive?include_prereleases&label=release"></a>
  <a href="LICENSE"><img alt="License MIT" src="https://img.shields.io/badge/license-MIT-2D89EF"></a>
  <img alt="Built with Tauri 2" src="https://img.shields.io/badge/built%20with-Tauri%202-1E7145">
  <img alt="Windows 11" src="https://img.shields.io/badge/Windows-11-00ABA9">
</p>

---

## Why

**Deadlines hide in calendars.** A conference abstract due Thursday is one line
among forty. Vindictive puts it on the monitor as a red tile with a countdown,
above every other window, until you press Done.

**Experiments do not wait for your to-do app.** When the thermal chamber is at
temperature you have one hand free. `Ctrl+Shift+Space`, type a line, Enter.
The tip is a Markdown file in your GitHub repo two seconds later, visible from
your phone, your laptop, and the board.

**Reading queues rot.** Paste an arXiv id or a DOI and the tile fills itself
with title, authors and year. The queue is a folder of files you can grep,
not a browser tab you will close by accident.

## What it looks like

<p align="center">
  <img src="docs/assets/board.png" alt="The board: a dark 300 px strip of flat tiles, the NEXT tile in crimson at the top" width="300">
  &nbsp;&nbsp;&nbsp;&nbsp;
  <img src="docs/assets/detail.png" alt="A tile flipped open: red deadline header, tags, body text, figure, link, and Done / Snooze / Tomorrow actions" width="300">
</p>

<p align="center">
  <img src="docs/assets/tile-anatomy.svg" alt="Annotated tile: NEXT edge, countdown, kind glyph, title, subtitle" width="720">
</p>

The two screenshots above are the real board rendered with sample tips (left:
the grid, right: a tile flipped open). The hero image at the top and the
annotated tile are design renders. The board is a 290 px wide strip that docks
to a screen edge. Everything is flat: one solid colour per tile, zero radius,
no shadows. The only motion is the flip when a tile opens.

| Kind / state | Colour |
| ------------ | ------ |
| task | blue `#2D89EF` |
| event | teal `#00ABA9` |
| note | purple `#7E3878` |
| reading | orange `#DA532C` |
| deadline, more than 7 days | green `#1E7145` |
| deadline, within 7 days | yellow `#FFC40D` |
| deadline, within 48 hours | red `#EE1111` |
| overdue | crimson `#B91D47` |

## Features

- **Always on top.** A frameless board of Windows 8 style live tiles that stays
  above every window. Drag it by its 20 px strip, or dock it to the left or
  right edge.
- **One thing is next.** Exactly one tile gets the white edge and the `NEXT`
  label. The choice is a fixed, explainable score, not a feed.
- **Deadline countdown.** `3w`, `5d`, `2d 4h`, `45m`, `overdue 2h`. Colour
  escalates green → yellow → red → crimson as the date closes in.
- **Native toasts.** Windows notifications fire at the remind times you wrote
  in the file, once, and when a new tip arrives from the repo.
- **GitHub-synced Markdown.** Each tip is a `.md` file with YAML frontmatter in
  a private repo you own. Edit it in Obsidian, on github.com, from your phone,
  or with `echo >>`. The app polls with conditional requests and writes back
  when you press Done or Snooze.
- **Quick capture.** Global hotkey `Ctrl+Shift+Space` opens a one-line bar:
  `Check coil temp after run 3 #lab !high @tomorrow 09:30 ^"Lab 302"`.
- **arXiv and DOI auto-fill.** `arxiv: 2401.12345` or `doi: 10.1109/...` in a
  tip fetches title, authors, year and venue into the file.
- **Recurring tips.** `repeat: weekly on mon at 10:00`. Pressing Done rolls the
  tip forward instead of closing it.
- **Tray icon, autostart, dark and light themes.**
- **Token stays local.** The GitHub token lives in Windows Credential Manager,
  never in a config file or the repo.

## Quick start

### Download

Grab the latest `.msi` or `.exe` from
[Releases](https://github.com/Zwl20085/vindictive/releases/latest), install,
and run. Windows 10 21H2 or later with WebView2 (built into Windows 11).

### Build from source

Prerequisites: [Rust stable](https://rustup.rs), Node 18 or later, and the
[WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/)
(already present on Windows 11).

```sh
git clone https://github.com/Zwl20085/vindictive.git
cd vindictive
npm install
npm run tauri dev      # run with hot reload
npm run tauri build    # produce src-tauri/target/release/bundle/
```

## Set up your tips repo

1. Create a **private** repository, e.g. `vindictive-tips`, with a `tips/`
   folder. The [`examples/tips/`](examples/tips) folder is a ready starter set.
2. Create a **fine-grained personal access token** limited to that one repo
   with *Contents: Read and write*. Step-by-step with screenshots-in-words:
   [docs/SETUP.md](docs/SETUP.md).
3. In the app, right-click the top strip → **Settings**, enter owner, repo,
   branch, dir, paste the token, **Test connection**.

## Tip format

One file per tip. Only `title` is required; unknown keys are preserved.

```markdown
---
title: Submit ECCE camera-ready
kind: deadline
priority: high
due: 2026-10-15
remind: [-1w, -1d, -2h]
location: Lab 302, bench 4
links:
  - https://ecce.org/authors
images:
  - figures/coil-thermal.png
tags: [ecce, paper]
---

- [ ] regenerate Fig. 4 with the 12 kHz data
- [ ] update author ORCID
```

| Key | Values | Notes |
| --- | ------ | ----- |
| `title` | text | required |
| `kind` | `task` `deadline` `note` `reading` `event` | default `task` |
| `priority` | `low` `normal` `high` | tile size and score |
| `status` | `open` `done` | written by the app |
| `due` | `2026-10-15` or `2026-10-15 17:00` | bare date means 23:59 |
| `remind` | list of `-30m` `-2h` `-1d` `-1w` or absolute times | relative to `due` |
| `location` | text | shown on the tile and in detail |
| `links`, `images`, `tags` | lists | images relative to the tips dir |
| `repeat` | recurrence rule, see below | Done rolls `due` forward |
| `color` | any CSS colour | overrides the kind colour |
| `arxiv`, `doi` | id | fills the `paper` block |
| `paper`, `snoozed_until`, `done_at`, `created` | | written by the app |

Full reference: [docs/TIP-FORMAT.md](docs/TIP-FORMAT.md).

### Quick-capture syntax

| Token | Meaning |
| ----- | ------- |
| `#tag` | add a tag, repeatable |
| `!high` `!low` | priority |
| `@today` `@tomorrow` `@nextweek` `@2026-10-15` | due date; optional `HH:MM` after it |
| `^location` | location, quote for spaces: `^"Lab 302"` |
| `>task` `>deadline` `>note` `>reading` `>event` | kind |
| arXiv id, arXiv URL, DOI | becomes a reading tip and fetches metadata |

### Recurrence grammar

```
daily [at HH:MM]
weekdays [at HH:MM]
weekly on mon[,thu] [at HH:MM]        every monday [at HH:MM]
monthly on 15 [at HH:MM]
yearly on 03-15 [at HH:MM]
```

Default time is 09:00. Months without the requested day are skipped.

### How "next up" is chosen

Every open, un-snoozed tip gets a score. The highest wins; ties go to the
earlier due date, then the title.

| Component | Points |
| --------- | ------ |
| overdue | 1000 |
| due within 48 h | 500 |
| due within 7 days | 200 |
| due later | 50 |
| no due date | 10 |
| priority high / normal / low | +30 / +10 / +0 |
| kind deadline / event / task / reading / note | +5 / +3 / +2 / +1 / +0 |

An overdue low-priority note therefore outranks a high-priority task due next
month. That is on purpose: the board exists to make the overdue thing
impossible to ignore.

## Architecture

```
vindictive/
├── src/                 web frontend, vanilla TypeScript + Vite
│   ├── types.ts         IPC contract shared with the backend
│   └── ...              tiles, detail view, settings, capture bar
├── src-tauri/src/
│   ├── core/            pure domain logic, no I/O, unit-tested
│   │   ├── tip.rs       Tip model, frontmatter round trip
│   │   ├── recur.rs     recurrence grammar
│   │   ├── nextup.rs    scoring
│   │   ├── deadline.rs  urgency levels
│   │   └── capture.rs   quick-capture parser
│   ├── sync/            GitHub Contents API, arXiv, Crossref
│   └── app/             Tauri state, commands, scheduler, tray, windows
├── docs/                DESIGN.md, TIP-FORMAT.md, SETUP.md
└── examples/tips/       starter tips for your private repo
```

The backend owns the truth. Every mutation is a command that returns the new
board state and emits `board-updated`, so the board and the capture bar never
disagree.

## Roadmap

- [ ] Linux and macOS builds (Tauri makes this mostly a CI matrix change)
- [ ] GitHub Issues as an alternative backend
- [ ] Obsidian plugin that mirrors the tile board in a side pane
- [ ] Calendar `.ics` export of deadlines and events
- [ ] Per-monitor docking and multi-board profiles

## Contributing

Issues and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md):
conventional commits, tests first, and the same commands CI runs.

## License

[MIT](LICENSE) © 2026 Wentao Zhang
