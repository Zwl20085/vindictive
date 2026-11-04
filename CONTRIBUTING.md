# Contributing

Thanks for helping. Vindictive is small on purpose; keep it that way.

Vindictive is Windows-only: it is developed, tested and released on Windows
10/11 with WebView2, and CI runs on `windows-latest`. You need Rust stable
with the MSVC toolchain and Node 20 or later (see `.nvmrc`).

## Workflow

1. Open an issue first for anything bigger than a typo, so we agree on scope.
2. Branch from `main`. Write the test before the code (RED → GREEN → refactor).
3. Run everything before pushing:

   ```sh
   npm test
   npm run build
   cargo fmt    --manifest-path src-tauri/Cargo.toml --check
   cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
   cargo test   --manifest-path src-tauri/Cargo.toml
   ```

4. Open a pull request against `main`. CI runs the same commands on Windows,
   plus `npm audit`, `cargo audit` and a debug `tauri build`.

## Releasing

1. `npm run bump -- 0.4.1` sets the version in `package.json`,
   `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` and
   `src-tauri/tauri.conf.json`.
2. Move the *Unreleased* notes in `CHANGELOG.md` under the new version.
3. Commit, tag `v0.4.1`, push the tag. The release workflow refuses to build
   if the tag and the three version fields disagree.

## Commit messages

Conventional commits, one change per commit:

```
feat: add yearly recurrence rule
fix: keep unknown frontmatter keys on save
docs: explain how to move the tips folder
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`.

## Code

- Domain logic goes in `src-tauri/src/core` and takes `now` as a parameter.
  No clock, no network, no Tauri there, so it stays unit-testable.
- Prefer returning new values over mutating; see `Tip::complete` for the pattern.
- Frontend is vanilla TypeScript. No framework, no CSS-in-JS.
- Design changes should agree with `docs/DESIGN.md`. Flat, zero radius, no shadows.
