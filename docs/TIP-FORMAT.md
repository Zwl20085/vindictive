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
arxiv: 2401.12345                # or
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

## Figures

- `images` entries appear as a thumbnail strip in the tile's detail view and
  the first one is ghosted onto the tile itself when it is `md` or `wide`.
  Click a thumbnail to view it full-window.
- Body images (`![alt](path)`) are resolved the same way and also open the
  viewer. Relative paths are resolved from the tips folder.
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
