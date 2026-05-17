# CLAUDE.md

floo has two implementations (`python/`, `typescript/`) sharing one on-disk
SQLite registry. [SPEC.md](./SPEC.md) is the single source of truth.

## Cross-cutting changes

For any change visible in SPEC.md (schema, claim algorithm, CLI surface,
marker format, port range):

1. Update `SPEC.md` first.
2. Update both `python/` and `typescript/`.
3. Verify interop: claim from one impl, list from the other, same DB.
4. Update `README.md` if user-visible.

Implementation-only changes (refactors, perf) don't need a SPEC update.

## Layout

- `SPEC.md` - contract (includes the canonical agent-setup instruction text)
- `python/` - Python implementation
- `typescript/` - TypeScript implementation, uses Node 22+'s `node:sqlite`,
  no native deps

Both implementations expose the same six commands at full parity:
`claim`, `release`, `list`, `gc`, `agent-setup`, `version`.

## Style

- No em dashes (`—`), en dashes (`–`), or ASCII hyphens used as parenthetical
  dashes. Use commas, periods, or parens.
- Imperative commit messages.
