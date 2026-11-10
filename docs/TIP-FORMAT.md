# Tip file format

One Markdown file per tip, YAML frontmatter on top. Everything except `title`
is optional. Unknown keys are preserved when the app writes the file back.

```markdown
---
title: Submit ECCE camera-ready
kind: deadline            # task | deadline | note | reading | event | agent
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
highlight: [camera-ready, ORCID] # words or phrases to mark on the tile and in the body
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

## Highlight words

`highlight` is a list of words or phrases (a single value is a one-item
list). Each one is marked with a flat highlight wherever it appears in the
tile's subtitle and in the body. Matching ignores case. The global list in
Settings → **Highlight words** applies to every tip, on top of the tip's own
list. Entries saved from the app are trimmed, duplicates (ignoring case) and
blank entries are dropped, and each must be 2 to 64 characters, at most 32
of them.

```yaml
highlight: [rebuttal, Reviewer 2]
```

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
example body (checklist, figure, links); an agent tip gets no example
body: the captured text is its body (see *Agent tips*). Edit or delete what you do not need. The comments last until the app rewrites the file (Done, snooze, or
when the file is modified); by then they have done their job.

A tip created with the full new-tip form gets no commented reference (the
form already shows every key). Its body is what you typed, or, if you left
it blank, a short skeleton for its kind: a checklist line for a task,
"What is due" and a checklist for a deadline, "Why read it / Key claims /
Notes" for reading, "Agenda / Notes" for an event, and nothing for a note
or an agent tip.

## Keys the app writes

| Key             | Written when                                              |
| --------------- | --------------------------------------------------------- |
| `status`, `done_at` | you press **Done** on a non-recurring tip             |
| `due`, `remind` | you press **Done** on a recurring tip (rolled forward)    |
| `snoozed_until` | you press **Snooze**                                      |
| `paper`         | arXiv / Crossref metadata was fetched                     |
| `created`       | the tip was made with quick capture or the new-tip form   |
| `agent`         | never: only the agent runner writes it, see *Agent tips*  |
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
| `>kind`        | `>task` `>deadline` `>note` `>reading` `>event` `>agent` |
| arXiv id / URL, DOI | becomes a reading tip and fetches metadata      |

## Agent tips

A tip with `kind: agent` is a task for an AI agent. Write the task as the
title and the body; that is all. A separate runner (Claude Graph) scans the
tips folder, runs one agent session per task and writes the outcome back
into the same file. The contract between the two apps is
[AGENT-PROTOCOL.md](AGENT-PROTOCOL.md); this section is the board's side.

```markdown
---
title: Survey hairpin AC loss papers
kind: agent
agent:                      # written by the runner only, never by the board
  v: 1
  state: done               # running | done | question | failed
  hash: sha256:d7e5fb1c...  # hash of the body when the runner wrote
  host: DESKTOP-ABC
  run: 20261109-193100-ab12
  at: 2026-11-09T19:31:00
  results: agent/agent-survey-example/   # relative to the tips folder
---

Survey recent papers on AC copper loss in hairpin windings.

<!-- agent:begin 2026-11-09T07:31:00 question -->
## ❓ Questions · 2026-11-09 07:31
1. Which year range should the survey cover?
<!-- agent:end -->

2020 onwards.
```

- **The `agent` map is read-only for the board.** Every board action (the
  Edit form, Done, reopen, snooze, colour, size, drag, fetching paper
  metadata) keeps it, including sub-keys the board does not know. Its values
  and key order are kept, nulls and empty values included; the quoting style
  is not (the board rewrites the frontmatter as for every other key). The Edit
  form has no field for it. Changing the kind away from `agent` keeps the
  map too (the tip just stops showing a badge); change it back and the
  badge returns. A malformed value (not a map, wrong types) does not
  make the tip unreadable; it is kept as written and the tip shows as waiting.
- **The badge** on the tile is derived, never stored:

  | Badge | State | When |
  | ----- | ----- | ---- |
  | 🕓 | waiting | never handled, or you changed the body since the runner wrote |
  | ⚙ | running | the runner claimed it |
  | ⭐ | finished | the runner left a result and the body is unchanged since |
  | ❓ | question | the agent asked something; answer by typing in the body |
  | ⚠ | failed | the run failed; edit the body to try again |

  A tip with a question is shown at least `md` size unless you set a size.
  `agent.v` above 1 (a newer protocol) shows as a plain agent tip with no badge.
- **Answer or ask for more** by typing anywhere in the body, normally below
  the last block. The body hash changes, the tile turns 🕓 and the runner
  picks it up on its next scan. Changing the title, colour, size or order
  does not re-run it.
- **Blocks** between `<!-- agent:begin ... -->` and `<!-- agent:end -->`
  are the runner's rounds. The comment lines are not shown in the detail view.
- **Results**: the detail view shows the state, host and time, and an
  **Open results** button when `agent.results` is set. The path must be
  an existing folder under `agent/` in the tips folder (relative, no `..`);
  anything else, including a file, is refused.
  The results subfolder is not a tip: the board only lists `*.md` directly in
  the tips folder and in `questions/` (see *Question tips*).
- **Toasts**: when a scan sees an agent tip turn ⭐ or ❓ (written by the
  runner, possibly on another PC), the board raises a toast, gated by the
  same "Toast when a new tip arrives" setting. Not on the first scan after
  start.
- **Retire** a task by marking it Done; the runner skips done tips.
- **Stuck at ⚙** because the machine that claimed it is gone: open the file
  in a text editor and delete the `agent:` lines; the tip is then a new task.
- **Do not share the tips folder for writing** with anyone else: whoever can
  write a `kind: agent` tip there gets an agent session on the runner's machine.
- A new agent tip, from the capture bar (`>agent`) or the new-tip form, has
  no `agent` map and no example body or skeleton: the body is the agent's
  prompt, so it holds only what you typed.
- **Only the prompt is needed.** In the new-tip form (and the Edit form),
  choosing the kind *Agent* puts one large **Prompt** field right after kind
  and priority, with the hint that Claude Graph runs it at its next scan and
  writes the result back into the tip. The title becomes optional. Due date,
  time, location, tags, reminders, repeat, links and figures mean nothing to
  the runner, so they are folded away; **Show all fields** brings them
  back. A field that holds a value is never folded, so nothing is saved
  unseen, and switching the kind away and back keeps everything typed.
  Priority, colour, size and highlight words stay: they shape the tile.
- **Derived title.** With the title left empty, it is taken from the
  prompt: the first line with text, skipping horizontal rules, whole HTML
  comments (also multi-line) and whole runner blocks (`<!-- agent:begin -->`
  to `<!-- agent:end -->`), so it never comes from a comment or a runner
  heading. Markdown markers are removed (`#`, `>`, `-`/`*`/`+` and
  `1.`-style list markers with ASCII digits, task boxes, `**`, `__`, `~~`,
  backticks; links and images keep their text) and a BOM is ignored. Longer
  than 60 characters, it is cut at a word boundary (or at 60), never in the
  middle of an accented letter or emoji, and `…` is added. A title you type
  wins. The
  body is the prompt exactly as typed; nothing is added to it.
- **Quick capture** `>agent <text>`: the text (without the `#tag`, `!high`,
  `@date` and other tokens, which apply as usual) becomes the whole body,
  and the title is derived from it as above. A short prompt is therefore
  both the title and the body; the runner gives the agent the title and
  the body, so the agent sees it twice, which is harmless. In the add bar,
  Ctrl+Enter opens the form with the text in the Prompt field.

  ```markdown
  ---
  title: Survey hairpin AC loss papers from 2020 on, with a table of…
  kind: agent
  priority: high
  status: open
  tags:
  - lit
  created: 2026-11-10 09:30
  # ---- optional keys (remove the # to use one) ----
  # due: 2026-10-15 17:00            # a bare date means 23:59
  # ... (the other commented keys)
  ---

  Survey hairpin AC loss papers from 2020 on, with a table of measured vs. FE results
  ```

  from `Survey hairpin AC loss papers from 2020 on, with a table of
  measured vs. FE results #lit !high >agent`.

## Question tips

A Claude session that needs decisions from you writes them as a tip in the
`questions/` subfolder of the tips folder; you answer from any device. The
board reads `*.md` files directly in `questions/` (one level: deeper
folders are not read; hidden, `~` and temp files are skipped as in the main
folder) and shows them with a ❓ badge, `md` size by default while open
(a `size:` in the file wins). The
strip menu's **Questions only** shows just them, until you pick it again.

- **It is a normal tip.** Same frontmatter, same body; only where the file
  lives makes it a question. Nothing is written into the file to mark it.
- **Answer** by editing the text, in the Edit form or your editor: change
  `- [ ]` to `- [x]` for the option you pick, or type after `Answer:`;
  then press **Done**. The checkboxes in the detail view are only a
  display and cannot be clicked. Every board action saves back into
  `questions/`.
- **Never an agent task.** `kind: agent` there shows as a plain question,
  with no agent state, toast or results button, and the Edit form shows it
  like any tip (no Prompt layout, title required; the kind is kept as
  written): the runner only scans the top level. To hand a question to the agent, move the file to the top
  level.
- **Identity.** Its id is `questions/<file name without .md>`, so
  `questions/plan.md` and `plan.md` are two different tips. OneDrive
  conflict copies in `questions/` are flagged against the original in
  `questions/`, as in the main folder.
- **The folder** is optional: when it is missing the board shows the main
  folder as before, and it never creates it. If `questions/` (or the main
  folder) suddenly lists no files while the board has tips there, the
  board waits for a second scan that agrees before removing them, so a
  folder that blinks out during a sync does not wipe its tiles. If
  `questions/` cannot be read, the sync dot says so and its tips stay. A `questions` that is a link
  or junction is not followed. New tips from the board always go to the
  main folder. Image paths are still relative to the tips folder, not to
  `questions/`.
- Boards up to 0.6.0 do not see the folder at all.

What a session should write, e.g. `questions/2026-11-10-fe-mesh.md`:

```markdown
---
title: "FE model: two mesh choices to confirm"
kind: note
created: 2026-11-10 09:30
---

Context in one or two lines: what the session is doing and why it asks.

## 1. Air-gap mesh layers

- [ ] 3 layers (faster)
- [ ] 5 layers (smoother torque)

Answer:

## 2. Skin-depth elements in the hairpins

- [ ] 2 per skin depth
- [ ] 3 per skin depth

Answer:
```

One `##` heading per question, the options as task-list lines (`- [ ]`,
which the owner turns into `- [x]` by editing), and an `Answer:` line for
free text. A session reads the answers back from the
file once its `status` is `done` (or earlier, if it polls the body).

## Time semantics

All times are local wall-clock times. A bare date as `due` means 23:59 that
day; a bare date as `remind` means 09:00.
