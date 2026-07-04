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

floo is a single Rust binary backed by an on-disk SQLite registry at
`~/.local/state/floo/registry.db`. The on-disk contract lives in
[SPEC.md](./SPEC.md).

### Homebrew (macOS)

```sh
brew install ajlebaron/floo/floo
```

If you have not added the tap yet, Homebrew will prompt you to; you can also add
it explicitly first:

```sh
brew tap ajlebaron/floo https://gitlab.com/ajlebaron/homebrew-floo.git
brew install floo
```

The formula builds floo from source (SQLite is bundled, so there is no system
libsqlite3 dependency). The crates.io package is named `floo-ports`, but the
installed binary is named `floo`.

### AUR (Arch Linux)

```sh
paru -S floo        # or: yay -S floo
```

or build it by hand:

```sh
git clone https://aur.archlinux.org/floo.git
cd floo
makepkg -si
```

The AUR package builds floo from the published `floo-ports` source with bundled
SQLite, so it has no system libsqlite3 dependency.

### crates.io

```sh
cargo install floo-ports
```

The package on crates.io is named `floo-ports` (the name `floo` was already
taken), but the installed binary is still named `floo`. If you'd rather
skip compiling from source, `cargo binstall floo-ports` fetches a prebuilt
binary instead, provided you have `cargo-binstall` installed.

### Prebuilt binary (no toolchain required)

Every tagged release publishes prebuilt binaries for Linux and macOS, so you
can run floo without a Rust toolchain or a C compiler. The binaries bundle
SQLite, so there is no system libsqlite3 dependency.

Supported targets:

- `floo-x86_64-unknown-linux-gnu` (Linux, Intel/AMD)
- `floo-aarch64-unknown-linux-gnu` (Linux, ARM64)
- `floo-x86_64-apple-darwin` (macOS, Intel)
- `floo-aarch64-apple-darwin` (macOS, Apple Silicon)

Grab the asset for your platform from the
[Releases page](https://gitlab.com/ajlebaron/floo/-/releases), make it
executable, and put it on your PATH:

```sh
# copy the download URL for your target from the latest release, then:
curl -L -o floo "<release-asset-url-for-your-target>"
chmod +x floo
sudo mv floo /usr/local/bin/floo
floo version
```

On macOS, if Gatekeeper blocks the first run, clear the quarantine attribute
with `xattr -d com.apple.quarantine /usr/local/bin/floo`.

### Rust (a recent stable toolchain)

```sh
cargo install --path rust
```

or clone and build a release binary yourself:

```sh
git clone https://gitlab.com/ajlebaron/floo.git
cd floo/rust
cargo build --release
# binary at target/release/floo
```

`rusqlite` is built with the `bundled` feature, so SQLite is compiled in.
There is no system libsqlite3 dependency, only a C compiler at build time.

Then tell your coding agent about it:

```sh
floo agent-setup
```

This appends a marker block to `~/.claude/CLAUDE.md` explaining when to call
`floo claim`. The block is idempotent, so running it again updates in place
rather than duplicating.

## Commands

```sh
floo claim <service> [--prefer <port>] [--json]  # idempotent: same input → same port
floo list [--json]                       # show all claims + listening status
floo release <service>                   # release one
floo release --all                       # nuke everything
floo gc [--older-than '-7 days'] [--dry-run] # reclaim stale claims
floo agent-setup                         # write the instruction into ~/.claude/CLAUDE.md
floo version
floo --version
```

`--older-than` takes a SQLite datetime modifier (`-7 days`, `-12 hours`,
`-30 minutes`), not a shorthand like `-7d`. The default is `-7 days`.

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

### Machine-readable output

Pass `--json` to `claim` or `list` for structured output that scripts and
agents can parse instead of scraping columns:

```sh
$ floo claim web --json
{
  "repo_path": "/home/me/dev/myapp",
  "service": "web",
  "port": 3001,
  "created_at": "2026-01-02T15:04:05Z",
  "last_seen_listening": null,
  "was_new": true
}

$ floo list --json
[ { "repo_path": "...", "service": "web", "port": 3001, "listening": true, ... } ]
```

Without `--json`, output is unchanged, so `PORT=$(floo claim web)` still
returns a bare number.

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
- **Overriding the registry location**: pass a global `--db <path>` flag
  (works with any subcommand, anywhere in the argument list), or set the
  `FLOO_DB` environment variable. Precedence is `--db` flag, then `FLOO_DB`,
  then the XDG default above. The parent directory is created automatically
  if it doesn't exist, same as the default location.
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
  extra safety check that re-probes the port at gc time. If a server is
  actively listening right now, the row stays. Run it whenever, or schedule
  it via cron:

  ```cron
  0 3 * * * /usr/local/bin/floo gc >/dev/null 2>&1
  ```

## Migration to Rust

floo used to ship as two parallel implementations, Python and TypeScript,
sharing one on-disk registry. Both have been removed and replaced by a
single Rust binary in `rust/`. Nothing changes for the on-disk format or the
CLI surface: `SPEC.md` still documents the exact registry contract, so any
future implementation can interoperate against the same registry file.
The agent-setup target is unchanged too: it remains Claude-Code-only
(`~/.claude/CLAUDE.md`), the same exclusion that existed before this
migration.

## Status

Alpha. Built for my own workflow. The agent-setup target is hard-coded to
Claude Code (`~/.claude/CLAUDE.md`) for now. Multi-agent support
(Cursor, Aider, Codex, etc.) is deferred until I actually need it.

**Platform: Linux and macOS only.** floo is not supported on Windows today.
It resolves the home directory via `HOME`, stores its registry under the
XDG location (`$XDG_STATE_HOME` or `~/.local/state`), and canonicalizes repo
paths in a Unix-oriented way, none of which map cleanly to Windows yet. The
`$(floo claim web)` command-substitution pattern also assumes a POSIX shell,
so it does not work in `cmd.exe` (npm's default shell on Windows). Windows
support would need a SPEC change for the state-dir location plus a Windows CI
runner, and is out of scope for now.

## Why "floo"

Harry Potter's Floo Network teleports you to a *fixed destination*. Same idea
here: every time you ask for `web` in your repo, you arrive at the same port.
