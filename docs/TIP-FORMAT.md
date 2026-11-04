# Tip file format

One Markdown file per tip, YAML frontmatter on top. Everything except `title`
is optional. Unknown keys are preserved when the app writes the file back.

```markdown
---
title: Submit ECCE camera-ready
kind: deadline            # task | deadline | note | reading | event
priority: high            # low | normal | high
status: open              # open | done
due: 2026-10-15           # date → 23:59, or "2026-10-15 17:00"
remind:
  - -1d                   # relative to due: -30m -2h -1d -1w
  - 2026-10-13 09:00      # or absolute
location: Lab 302, bench 4
links:
  - https://ecce.org/authors
tags: [ecce, paper]
images:
  - figures/coil-thermal.png     # relative to the tips folder
  - figures/ac-loss-sweep.svg    # png, jpg, gif, webp, svg, or an https URL
repeat: weekly on mon at 10:00   # daily | weekdays | weekly on mon,thu | monthly on 1 | yearly on 03-15
color: "#FF0097"                 # optional tile colour override
size: md                         # optional tile size override: sm | md | wide
order: 2.5                       # optional manual position, written by drag and drop
arxiv: '2401.12345'              # quoted, see below; or
doi: 10.1109/TIE.2024.1234567
---

Body in **Markdown**. Checklists, images, links all work.

- [ ] regenerate Fig. 4 with the 12 kHz data
- [ ] update author ORCID

![hotspot](figures/coil-thermal.png)

<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40" viewBox="0 0 120 40">
  <polyline fill="none" stroke="#E0762B" stroke-width="2" points="4,36 40,28 80,14 116,6"/>
</svg>
```

## Quote arXiv ids

Write `arxiv: '2401.12345'`, with quotes. Unquoted, YAML reads `2401.10000`
as the number 2401.1 and drops the trailing zeros; the app rebuilds the id
when it can, but it adds a warning asking you to quote it. Ids with a prefix
or version (`arXiv:2401.10000v2`) are text already and need no quotes.

## Warnings

Problems the app finds while reading a tip, such as a `due` or `remind` that
does not parse, a `repeat` rule it does not understand, or an unquoted arXiv
id, are collected as a `warnings` list and shown in the tile's detail view.
The tip still loads with the bad value ignored. Warnings are derived on every
read and never written to the file; fix the frontmatter and they disappear.

## Tile colour and size

- `color` takes a hex colour, `#RGB` or `#RRGGBB` (case-insensitive, quote it
  in YAML because `#` starts a comment). Anything else, such as `red` or
  `rgb(…)`, is kept in the file but ignored, and the tile uses its kind or
  deadline colour. Text is light or dark, whichever contrasts more with the
  tile (WCAG contrast ratio).
- `size` takes `sm` (1x1), `md` (2x2) or `wide` (4x2 grid units). Any other
  value, or no `size`, means automatic: `wide` for the next-up tip, `md` for
  high priority or a deadline within seven days, `sm` for the rest.
- `order` is a number; tiles with an `order` are placed by it, the rest by
  score.

## The `paper` block

Written by the app after an arXiv or Crossref lookup, but you can write it
by hand too (for a paper with no arXiv id or DOI). `title` is required
inside the block; the rest is optional.

```yaml
paper:
  title: "Attention Is All You Need"
  authors: [Ashish Vaswani, Noam Shazeer]   # list of names
  year: 2017                                 # integer
  venue: NeurIPS                             # journal or conference
  url: https://arxiv.org/abs/1706.03762
```

## Figures

- `images` entries appear as a thumbnail strip in the tile's detail view and
  the first one is ghosted onto the tile itself when it is `md` or `wide`.
  Click a thumbnail to view it full-window.
- Body images (`![alt](path)`) are resolved the same way and also open the
  viewer. Relative paths are resolved from the tips folder. Only `https:`,
  relative and `data:` sources load; `http:`, `//host/...`, UNC paths
  (`\\server\share`) and `srcset` are stripped.
- Inline `<svg>` in the body is rendered as-is after sanitising: scripts,
  event handlers, `<style>` and `<foreignObject>` are removed.

## New tips

A tip created from the capture bar or the "+" tile starts as a template:
the keys you typed, a commented reference of every optional key, and an
example body (checklist, figure, links). Edit or delete what you do not
need. The comments last until the app rewrites the file (Done, snooze, or
when the file is modified); by then they have done their job.

## Keys the app writes

| Key             | Written when                                              |
| --------------- | --------------------------------------------------------- |
| `status`, `done_at` | you press **Done** on a non-recurring tip             |
| `due`, `remind` | you press **Done** on a recurring tip (rolled forward)    |
| `snoozed_until` | you press **Snooze**                                      |
| `paper`         | arXiv / Crossref metadata was fetched                     |
| `created`       | the tip was made with quick capture                       |
| `color`, `size` | you pick a swatch or size in the tile's detail view       |
| `order`         | you drag a tile to a new place on the board               |

## Quick-capture syntax

```
Check coil temperature after run 3 #lab !high @tomorrow 09:30 ^"Lab 302"
```

| Token          | Meaning                                              |
| -------------- | ---------------------------------------------------- |
| `#tag`         | add a tag                                            |
| `!high` `!low` | priority                                             |
| `@today` `@tomorrow` `@nextweek` `@2026-10-15` | due date, optional `HH:MM` after it |
| `^location`    | location, quote for spaces                           |
| `>kind`        | `>task` `>deadline` `>note` `>reading` `>event`      |
| arXiv id / URL, DOI | becomes a reading tip and fetches metadata      |

## Time semantics

All times are local wall-clock times. A bare date as `due` means 23:59 that
day; a bare date as `remind` means 09:00.
