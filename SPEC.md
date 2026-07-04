# floo registry contract

This document is the source of truth for the on-disk format and CLI behavior
that floo's implementation (currently a single Rust binary) must follow. It
remains the contract for any future implementation, so that any combination
of implementations installed on the same machine interoperates against the
same registry.

## On-disk location

The registry is a SQLite file at:

```
$XDG_STATE_HOME/floo/registry.db
```

Falling back to `~/.local/state/floo/registry.db` if `XDG_STATE_HOME` is unset.
The directory must be created with parents if it does not exist.

## Schema

```sql
CREATE TABLE IF NOT EXISTS claims (
    repo_path           TEXT    NOT NULL,
    service             TEXT    NOT NULL,
    port                INTEGER NOT NULL UNIQUE,
    created_at          TEXT    NOT NULL,
    last_seen_listening TEXT,
    PRIMARY KEY (repo_path, service)
);
```

- `repo_path` is the absolute filesystem path to the repository root (output
  of `git rev-parse --show-toplevel`, or the current working directory if not
  inside a git repo).
- `service` is a free-form label the caller picks (`web`, `storybook`, `api`,
  etc.).
- `port` is in the range 3000-3999 inclusive.
- Timestamps are ISO 8601 in UTC, written as `YYYY-MM-DDTHH:MM:SSZ`. Queries
  that compare timestamps against `datetime('now', ...)` must wrap stored
  values in SQLite's `datetime()` function to normalize.

## Required PRAGMAs

Every connection should set:

```
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
```

WAL allows concurrent reads with one writer; the busy timeout makes the
implicit locking sane under parallel CLI use.

## Port range

- `PORT_MIN = 3000`
- `PORT_MAX = 3999`

Exhausting the range is a hard error. Do not silently fall into ephemeral
port territory.

## Claim algorithm

`claim(repo_path, service, prefer=None)`:

1. Open a write transaction with `BEGIN IMMEDIATE`. This serializes the
   read-decide-write so concurrent writers cannot race.
2. `SELECT` the row for `(repo_path, service)`. If present, commit and return
   it as an existing claim.
3. Otherwise, pick a port:
   - If `prefer` is provided, in range, not in the registry, and not bound on
     the OS, use it.
   - Else scan `PORT_MIN..PORT_MAX` ascending. Skip any port present in the
     registry. Skip any port currently bound by an OS process. Take the
     first one that passes both checks.
   - If the range is exhausted, raise.
4. `INSERT` the row with `created_at = now()` and `last_seen_listening = NULL`.
5. Commit.

The bind check is a point-in-time probe (try to bind to `127.0.0.1:port` with
a fresh socket and immediately close). It is racy by nature; that race is
accepted. Implementations must not "adopt" a bound-but-untracked port into
the registry.

## Release

- `release(repo_path, service)` deletes the matching row. Returns whether a
  row was removed.
- `release_all()` deletes every row. Returns the count.

## List

Pure read. Returns every row, ordered by `port`. Listening status is observed
at print time but never persisted by `list`.

## GC

A claim is eligible for reclamation when *both* hold:

1. The timestamp criterion is satisfied. With grace window `T`:
   - `last_seen_listening` is non-null and earlier than `now - T`, OR
   - `last_seen_listening` is null and `created_at` is earlier than `now - T`.
2. Re-probing the port at gc time confirms nothing is listening. If the port
   *is* listening, the row is not reclaimed; instead, `last_seen_listening`
   is updated to "now" so the row is not re-flagged on the next gc run.

`gc --dry-run` performs the same query and probe but does not delete.

## Agent setup

`agent-setup` writes an instruction block to `~/.claude/CLAUDE.md`,
delimited by literal HTML comment markers, with the **exact** body shown
below. Every implementation must produce byte-identical output so two
implementations on the same machine do not flap the file.

```
<!-- floo:start -->
## Port allocation (floo)

When you need a port for a dev server, mock API, or any other locally bound
service, call `floo claim <service>` first and use the port it returns.
Examples of service labels: `web`, `storybook`, `api`, `db`. The same label in
the same repo always returns the same port, that's the whole point. Do **not**
guess "next free port" yourself; you will collide with other agents.

- `floo claim web` -> prints the port to stdout.
- `floo list` -> shows what's claimed and whether it's listening right now.
- `floo release <service>` -> release one claim. Usually unnecessary.

If `floo` is not installed, fall back to your normal behavior.
<!-- floo:end -->
```

The block ends with a single trailing newline after the closing marker.

Behavior:
- If the file does not exist, create it containing only the block. Return
  action `created`.
- If the file exists and contains the markers, replace whatever sits between
  them with the canonical body. Return `updated` if the file changed,
  `unchanged` otherwise.
- If the file exists but does not contain the markers, append the block
  preceded by a blank line. Return `updated`.

## CLI surface

```
floo claim <service> [--prefer <port>] [--json]
floo release <service>
floo release --all
floo list [--json]
floo gc [--older-than <duration>] [--dry-run]
floo agent-setup
floo version
floo --version
```

Bare `floo claim` and `floo release` print usage plus the current claims in
the active repo. They do not error.

`floo claim <service>` prints the port number to stdout on its own line and
nothing else (so it can be captured with `PORT=$(floo claim web)`).

## JSON output

`claim` and `list` accept a `--json` flag that emits structured JSON to
stdout instead of the human text format. Without the flag, output is
unchanged, so `PORT=$(floo claim web)` still yields a bare number.

`floo claim <service> --json` prints a single object: the claim record plus
`was_new` (true when this invocation allocated a new port, false when it
returned an existing claim).

    {
      "repo_path": "/home/me/dev/myapp",
      "service": "web",
      "port": 3001,
      "created_at": "2026-01-02T15:04:05Z",
      "last_seen_listening": null,
      "was_new": true
    }

`floo list --json` prints an array of objects, one per claim ordered by port,
each being the claim record plus `listening` (the live OS bind probe at print
time). An empty registry prints `[]`.

    [
      {
        "repo_path": "/home/me/dev/myapp",
        "service": "web",
        "port": 3001,
        "created_at": "2026-01-02T15:04:05Z",
        "last_seen_listening": "2026-01-02T15:05:10Z",
        "listening": true
      }
    ]

Field types: `repo_path`, `service`, `created_at` are strings; `port` is an
integer; `last_seen_listening` is a string or null; `was_new` and `listening`
are booleans. Output is pretty-printed JSON followed by a trailing newline.
