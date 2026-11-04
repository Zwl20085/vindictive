# Changelog

All notable changes to Vindictive are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) (pre-1.0: a minor
bump may break things). Entries before 0.4.1 were reconstructed from the
conventional-commit history; the version a commit shipped in is the first
`v*` tag that contains it.

## [Unreleased]

### Added

- `npm run bump -- <version>` (`scripts/bump-version.mjs`) sets the version in
  `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` and both
  lockfiles in one go.
- CI: `npm audit` (runtime dependencies), `cargo audit`, `clippy
  --all-targets`, and a debug installer build on every pull request.
  Dependabot for npm, Cargo and GitHub Actions.
- Release: refuses to build unless the tag matches all three version fields
  and the tests pass; fails if the release lacks `latest.json` or `.sig`
  files.
- The installer downloads the WebView2 runtime when it is missing.
- Docs: hotkeys, uninstall and Windows-only build notes in SETUP.md; the
  `paper` block and `color` / `size` values in TIP-FORMAT.md; a new example
  tip and the missing `figures/coil-thermal.png`.
- Parse problems in a tip (`due`, `remind`, `repeat`, `snoozed_until`, an
  unquoted `arxiv` id) are listed in the detail view and marked with `!` on
  the tile, instead of being dropped silently.
- Lenient frontmatter: a bare number as `title`, a scalar where a list is
  expected (`remind: 2026-10-14`), and `kind: Task` all parse.
- Missed reminders: a single digest toast lists reminders that fell due while
  the PC was asleep or the app closed (up to 7 days), instead of dropping
  them; a reminder is recorded as fired only once its toast was shown.
- OneDrive conflict copies (`name-DESKTOP-XXXX.md`, `name (1).md`) are
  flagged on the sync square; they stay on the board.
- Weather returns the last good reading while offline and backs off instead
  of retrying every poll; one shared HTTP client for weather and paper
  metadata.
- Keyboard: Up/Down move by a row of tiles, Alt+Arrow reorders, arrow keys and
  Home/End work inside menus, the sync square is a button, focus survives
  the countdown tick and returns to the tile after a detail closes.
- `Esc` on an edited form asks for a second `Esc` within two seconds.
- Unicode titles (Chinese, umlauts) produce real file-name slugs.

### Changed

- Node 20 or later is required to build (Node 18 is end-of-life); `.nvmrc`
  pins 20.
- `@tauri-apps/api` and `@tauri-apps/cli` 2.12.4, `@tauri-apps/plugin-opener`
  2.7.2. Dropped the `@types/dompurify` stub (DOMPurify ships its own types).
- GitHub Actions are pinned to commit SHAs.

### Fixed

- SETUP.md no longer claims that changing the tips folder copies files.
- Text typed in the add bar or the Settings form was wiped by the 30 s
  countdown tick and by every folder sync.
- A `remind` offset such as `-99999999999d` crashed the app at every start
  (release builds abort on panic); all date arithmetic is now checked.
- A briefly empty tips folder (OneDrive remounting) wiped the board and then
  announced every tip as new; an empty listing now needs two consecutive scans
  before it is applied, and bulk arrivals are not announced.
- An RFC 3339 due time with an offset (`08:30+02:00`, `Z`) was read as local
  wall-clock time; it is now converted to the local zone.
- `monthly on 31` skipped months with fewer days; it now falls on the last
  day. `yearly on 02-29` falls on 28 February in common years.
- A failed Done, Snooze or Delete still closed the detail view.
- `Esc` on a context menu also closed the detail view beneath it.
- Markdown links in a tip body navigated the webview instead of opening the
  browser.
- The detail view was rebuilt on every folder sync, resetting scroll, an armed
  Delete and running animations; tiles are now updated in place, keyed by id.
- Focus ring and the NEXT / overdue edges were white on the light and paper
  themes; amber tiles failed WCAG contrast with light text.
- The hidden face of the board remained reachable by Tab and screen readers.
- Edited images were served from a stale cache for the whole session.
- Opening a tip in an external editor blocked the UI until the editor closed.
- The tip cache was written through a shared temp name without fsync; two
  writers could clobber it.
- A capture whose title clashed with an unsynced file on disk failed; the next
  free name is used.
- An invalid time in a capture (`25:99`) aborted the capture with a misleading
  error; it is kept as text.
- `tip.sha` and other doc comments still described the removed GitHub store.

### Security

- DOMPurify 3.4.17 (GHSA-cg2g-xh64-v4q7, GHSA-mrv2-3pqr-j5jr,
  GHSA-c6gr-qvv4-5v3f).
- rustls 0.23.46, rustls-webpki 0.103.16, webpki-roots 1.0.11.
- The external editor is no longer started through `cmd /C` with the file
  path in the command line; paths that `cmd` would expand (`%VAR%`, `&`) are
  refused on the batch-file path.
- Markdown bodies: inline `style`, `data-*` attributes, `srcset`, `http:`,
  protocol-relative and UNC sources are stripped; `https:`, relative and
  `data:` sources load. Ids are namespaced against DOM clobbering.
- Tip file names containing `:` (alternate data streams) and paths that
  resolve outside the tips folder are rejected.
- The front-end capability set is reduced to what the webview calls.
- Tip titles are no longer written to the persistent log.

## [0.4.0] - 2026-10-02

### Added

- Clawd reacts to the board: celebrates a completed tip, frets while
  something is overdue, hums when nothing is open, sleeps at night.
- Edit a tip in the app: title, kind, priority, due, location, tags and
  notes; other frontmatter keys are kept and the file is never renamed.
- Peek hotkey (`Ctrl+Alt+Shift+Space`, or the tray icon) lifts a bottom-layer
  board above other windows.
- Quiet background update check shortly after start and daily, with one
  toast per new version (`auto_update_check`).

## [0.3.3] - 2026-10-02

### Fixed

- The tips folder is loaded on every start.

## [0.3.2] - 2026-10-02

### Added

- **Colour…** in the tile menu.

### Changed

- Only one copy runs; launching it again brings the board to the front.

## [0.3.1] - 2026-10-02

### Added

- Clawd, a pixel-art companion, paces in the clock panel.

### Changed

- New default settings.

## [0.3.0] - 2026-10-01

### Added

- Always-on-bottom window layer, exclusive with always-on-top.
- Self-update from GitHub Releases via the tray (**Check for updates…**).

### Changed

- **Breaking:** tips live in a local folder (by default `%OneDrive%\Vindictive`
  or `Documents\Vindictive`), synced by OneDrive or any file sync tool.
  To migrate, copy the old repo's `tips/` files into the tips folder.

### Removed

- GitHub sync, the GitHub client and the Credential Manager token storage.

### Fixed

- Says when the latest release has no update file instead of blaming the
  network.

## [0.2.0] - 2026-09-30

### Added

- NERV restyle, animations, Chinese UI, figures, clock and weather panel.
- Metro restyle with serif titles, adaptive type, light and dark themes, tile
  actions and a local editor.
- New-tip template, fit-height window, tile colour and size controls.
- Drag and drop tiles to reorder the board.
- 28 tile colours plus a native colour picker; window opacity setting.
- Opens at half the desktop; always-on-top off by default.
- One-click setup for new users.

### Changed

- Neutral placeholder city in mock data, docs and tests.
- CI checks Rust formatting.

### Fixed

- Find VS Code by install path, or through its registry entry, when `code` is
  not on PATH.
- Mouse-wheel scrolling on the board and in the detail view.
- No lost edits, no stale Done, no broken tips; overdue toasts repeat for
  recurring and rescheduled tips.

## [0.1.0] - 2026-09-28

### Added

- First release: an always-on-top tile board synced with GitHub.

[Unreleased]: https://github.com/Zwl20085/vindictive/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/Zwl20085/vindictive/compare/v0.3.3...v0.4.0
[0.3.3]: https://github.com/Zwl20085/vindictive/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/Zwl20085/vindictive/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/Zwl20085/vindictive/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/Zwl20085/vindictive/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Zwl20085/vindictive/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Zwl20085/vindictive/releases/tag/v0.1.0
