# CLAUDE.md

Notes for AI agents and future contributors working on this repo.

## Workflow for cross-cutting changes

floo ships in two implementations, `python/` and `typescript/`, that share an
on-disk SQLite registry. They are kept in lockstep through [SPEC.md](./SPEC.md),
which is the single source of truth for:

- the registry location and SQLite schema
- the claim algorithm (including the `BEGIN IMMEDIATE` transaction)
- the port-range policy and bind-check rules
- the CLI surface (command names, flags, output format)
- the agent-setup marker-block format

**Any change that touches behavior visible in SPEC.md must follow this order:**

1. Update `SPEC.md` first. The PR or commit message should describe *why* the
   spec is changing, not just *what*.
2. Update both `python/` and `typescript/` to match. If only one
   implementation is updated, the other is now out of spec; that is a bug.
3. Verify interop by hand: claim something from one impl, list from the
   other, against the same `~/.local/state/floo/registry.db`. They must
   agree.
4. Update `README.md` if any user-visible surface changed.

Implementation-only changes (refactors, performance, internal logging) do
not need a SPEC update, but should not change observable behavior.

## What lives where

- `SPEC.md` - the contract.
- `python/src/floo/` - Python implementation (canonical for features not yet
  ported to TS, currently `gc` and `agent-setup`).
- `python/tests/` - pytest suite. Run with a venv since this machine has no
  system pytest.
- `typescript/src/` - TypeScript implementation. `tsc` builds into
  `typescript/dist/`. Uses Node 22+'s `node:sqlite` (no native deps).
- `README.md` - user-facing.

## Style

- No em dashes (`—`) or en dashes (`–`) in any prose or code in this repo.
  Use ASCII hyphens, commas, periods, or parentheses. Avoid using ASCII
  hyphens as parenthetical-dash substitutes; use commas or parens for that.
- Commit messages: imperative present tense ("Add X", not "Added X").

## Feature parity status

| Command         | Python | TypeScript |
|-----------------|:------:|:----------:|
| `claim`         | yes    | yes        |
| `release`       | yes    | yes        |
| `release --all` | yes    | yes        |
| `list`          | yes    | yes        |
| `version`       | yes    | yes        |
| `gc`            | yes    | no         |
| `agent-setup`   | yes    | no         |

When porting `gc` or `agent-setup` to TypeScript, mirror the Python behavior
exactly (read the Python source, not from memory) and update SPEC.md only if
you find an ambiguity worth nailing down.
