# Contributing

Thanks for helping. Vindictive is small on purpose; keep it that way.

## Workflow

1. Open an issue first for anything bigger than a typo, so we agree on scope.
2. Branch from `main`. Write the test before the code (RED → GREEN → refactor).
3. Run everything before pushing:

   ```sh
   npm test
   cargo test   --manifest-path src-tauri/Cargo.toml
   cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
   npm run build
   ```

4. Open a pull request against `main`. CI runs the same commands on Windows.

## Commit messages

Conventional commits, one change per commit:

```
feat: add yearly recurrence rule
fix: keep unknown frontmatter keys on save
docs: describe fine-grained token setup
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`.

## Code

- Domain logic goes in `src-tauri/src/core` and takes `now` as a parameter.
  No clock, no network, no Tauri there, so it stays unit-testable.
- Prefer returning new values over mutating; see `Tip::complete` for the pattern.
- Frontend is vanilla TypeScript. No framework, no CSS-in-JS.
- Design changes should agree with `docs/DESIGN.md`. Flat, zero radius, no shadows.
