# floo

Sticky port assignments for parallel coding-agent dev servers.

## The problem

I work on multiple tickets at the same time with multiple Claude Code instances
running in parallel. Each agent runs its own dev server. The agents pick "next
free port" independently, so ports drift constantly: a server that was on 3002
disappears, restarts on 3000 next time, and now my mental model (and my
browser tabs, my `.env` files, my notes) is wrong.

`floo` makes the assignment **sticky**. The same `(repo, service)` pair always
gets the same port. Restart your dev server, switch branches, reboot. The port
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

floo ships in two implementations that share the same on-disk registry at
`~/.local/state/floo/registry.db`. You can install either, both, or mix and
match on the same machine. The shared contract lives in [SPEC.md](./SPEC.md).

### Python (Python 3.10+)

```sh
pipx install git+https://gitlab.com/ajlebaron/floo.git#subdirectory=python
```

### TypeScript (Node 22+)

Pick one:

```sh
# from npm
pnpm add -g @ajlebaron/floo
# or
npm install -g @ajlebaron/floo

# from git, no registry needed
pnpm add -g git+https://gitlab.com/ajlebaron/floo.git#path:typescript
# or
npm install -g git+https://gitlab.com/ajlebaron/floo.git#path:typescript

# from a local checkout
cd typescript && npm install && npm run build && npm link
```

The TS implementation uses Node 22+'s built-in `node:sqlite` (no native
deps). Node emits an `ExperimentalWarning` for `node:sqlite` today; the CLI
suppresses just that one warning so your stdout stays clean for
`PORT=$(floo claim web)` and friends.

Then tell your coding agent about it:

```sh
floo agent-setup
```

This appends a marker block to `~/.claude/CLAUDE.md` explaining when to call
`floo claim`. The block is idempotent - running it again updates in place
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

## Examples (Next.js)

### Basic dev launch

```sh
cd ~/dev/myapp
PORT=$(floo claim web)
next dev -p $PORT
```

Next time, same repo, same command, same port. Restart, switch branches,
reboot, still the same port.

### As package.json scripts

```json
{
  "scripts": {
    "dev": "next dev -p $(floo claim web)",
    "storybook": "storybook dev -p $(floo claim storybook)",
    "ports": "floo list"
  }
}
```

```sh
npm run dev         # next on its sticky port
npm run storybook   # storybook on a different sticky port
npm run ports       # show all claims and listening status
```

### Parallel work via git worktrees

```sh
cd ~/dev/myapp                            # feature-auth branch
npm run dev                               # port 3001

git worktree add ../myapp-billing feature-billing
cd ../myapp-billing
npm run dev                               # port 3003, automatically
```

Two Next.js dev servers, same repo, side by side. Distinct directories give
distinct ports without any configuration.

### Inspecting state

```sh
$ floo list
PORT   LISTENING  SERVICE        REPO
3001   yes        web            /home/me/dev/myapp
3002   no         storybook      /home/me/dev/myapp
3003   yes        web            /home/me/dev/myapp-billing
```

`LISTENING=no` means the row is claimed but the server is not running right
now. The reservation persists.

### Capturing the port for env vars

If you need the value elsewhere (an absolute URL in `NEXT_PUBLIC_APP_URL`, a
log line, a helper script), capture it once and export:

```sh
# .envrc, if you use direnv
export PORT=$(floo claim web)
export NEXT_PUBLIC_APP_URL="http://localhost:$PORT"
```

### What an agent does

After `floo agent-setup`, Claude Code sees the instruction in
`~/.claude/CLAUDE.md` and the typical sequence becomes:

```
> floo claim web
3001
> next dev -p 3001
```

The agent does not have to remember which port "this feature" was on. Same
claim, same answer, every time.

## Design notes

- **Storage**: one SQLite file at `~/.local/state/floo/registry.db`. No daemon.
  The CLI opens the DB, does its work, closes. Survives reboots.
- **Race safety**: every mutating call wraps its read-decide-write in a
  `BEGIN IMMEDIATE` transaction, so two agents claiming a port at the same
  instant can't both grab the same number. The `UNIQUE` constraint on `port`
  is a schema-level backstop.
- **Port range**: 3000-3999. Errors loudly past the ceiling.
- **OS bind check**: before handing out a port, `floo` actually tries to bind
  it. If an untracked process (Docker, a random script) holds it, `floo`
  skips that port without registering it. We never adopt something we didn't
  claim.
- **Reclamation**: claims are sticky by default. `floo gc` reclaims rows that
  haven't been seen listening for the grace window (default 7 days), with an
  extra safety check that re-probes the port at gc time - if a server is
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
