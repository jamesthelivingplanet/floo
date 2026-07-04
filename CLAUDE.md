# CLAUDE.md

floo is a single Rust implementation (`rust/`) backed by one on-disk SQLite
registry. [SPEC.md](./SPEC.md) is the single source of truth.

## Cross-cutting changes

For any change visible in SPEC.md (schema, claim algorithm, CLI surface,
marker format, port range):

1. Update `SPEC.md` first.
2. Update `rust/`.
3. Verify interop against the on-disk registry (claim, then list, same DB).
4. Update `README.md` if user-visible.

Implementation-only changes (refactors, perf) don't need a SPEC update.

## Layout

- `SPEC.md` - contract (includes the canonical agent-setup instruction text)
- `rust/` - Rust implementation, uses `rusqlite` with the `bundled` feature,
  no system libsqlite3 dependency

The binary exposes six commands: `claim`, `release`, `list`, `gc`,
`agent-setup`, `version`.

## Style

- No em dashes (`—`), en dashes (`–`), or ASCII hyphens used as parenthetical
  dashes. Use commas, periods, or parens.
- Imperative commit messages.
