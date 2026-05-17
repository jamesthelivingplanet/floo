# floo

Sticky port assignments for parallel coding-agent dev servers.

## The problem

I work on multiple tickets at the same time with multiple Claude Code instances
running in parallel. Each agent runs its own dev server. The agents pick "next
free port" independently, so ports drift constantly: a server that was on 3002
disappears, restarts on 3000 next time, and now my mental model — and my
browser tabs, my `.env` files, my notes — is wrong.

`floo` makes the assignment **sticky**. The same `(repo, service)` pair always
gets the same port. Restart your dev server, switch branches, reboot — the port
comes back.

## How it works

You give `floo` a service label. It returns a port. The mapping is keyed on
**`(absolute repo path, service label)`** and stored in a local SQLite registry
at `~/.local/state/floo/registry.db`.

```sh
$ cd ~/dev/myapp
$ floo claim web
3001

$ cd ~/dev/myapp        # same repo, same service
$ floo claim web
3001                    # same port, every time

$ floo claim storybook  # same repo, different service
3002

$ cd ~/dev/myapp-billing # git worktree of the same repo for parallel work
$ floo claim web
3003                    # different path → different port automatically
```

Branch renames don't affect anything (we don't key on branch). Git worktrees
just work (different directory = different key). Moving the directory breaks
the assignment, which is an accepted tradeoff.

## Install

Requires Python 3.10+.

```sh
# with pipx (recommended)
pipx install git+https://gitlab.com/ajlebaron/floo.git

# or with a venv
python3 -m venv ~/.venvs/floo
~/.venvs/floo/bin/pip install git+https://gitlab.com/ajlebaron/floo.git
ln -s ~/.venvs/floo/bin/floo ~/.local/bin/floo
```

Then tell your coding agent about it:

```sh
floo agent-setup
```

This appends a marker block to `~/.claude/CLAUDE.md` explaining when to call
`floo claim`. The block is idempotent — running it again updates in place
rather than duplicating.

## Commands

```sh
floo claim <service> [--prefer <port>]   # idempotent: same input → same port
floo list                                # show all claims + listening status
floo release <service>                   # release one
floo release --all                       # nuke everything
floo gc [--older-than -7d] [--dry-run]   # reclaim stale claims
floo agent-setup                         # write the instruction into ~/.claude/CLAUDE.md
floo version
```

Bare `floo claim` or `floo release` print usage plus the current claims in
your repo, so an agent (or you) can see what's available without a separate
command.

## Design notes

- **Storage**: one SQLite file at `~/.local/state/floo/registry.db`. No daemon.
  The CLI opens the DB, does its work, closes. Survives reboots.
- **Race safety**: every mutating call wraps its read-decide-write in a
  `BEGIN IMMEDIATE` transaction, so two agents claiming a port at the same
  instant can't both grab the same number. The `UNIQUE` constraint on `port`
  is a schema-level backstop.
- **Port range**: 3000–3999. Errors loudly past the ceiling.
- **OS bind check**: before handing out a port, `floo` actually tries to bind
  it. If an untracked process (Docker, a random script) holds it, `floo`
  skips that port without registering it. We never adopt something we didn't
  claim.
- **Reclamation**: claims are sticky by default. `floo gc` reclaims rows that
  haven't been seen listening for the grace window (default 7 days), with an
  extra safety check that re-probes the port at gc time — if a server is
  actively listening right now, the row stays. Run it whenever, or schedule
  it via cron:

  ```cron
  0 3 * * * /usr/local/bin/floo gc >/dev/null 2>&1
  ```

## Status

Alpha. Built for my own workflow. The agent-setup target is hard-coded to
Claude Code (`~/.claude/CLAUDE.md`) for now. Multi-agent support
(Cursor, Aider, Codex, etc.) is deferred until I actually need it.

## Why "floo"

Harry Potter's Floo Network teleports you to a *fixed destination*. Same idea
here: every time you ask for `web` in your repo, you arrive at the same port.
