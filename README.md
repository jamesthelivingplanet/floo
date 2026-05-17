# floo

Sticky port assignments for parallel coding-agent dev servers.

Agents working on the same machine independently grab "next free port" and
collide. `floo` gives each `(repo, service)` pair a stable port that survives
restarts, branch switches, and reboots.

## Status

Alpha. Pre-release.

## Install (dev)

```sh
pipx install -e ~/dev/floo
```

## Usage (planned)

```sh
floo claim web                  # returns a port for service "web" in this repo
floo list                       # show all claims + listening status
floo release web                # release one
floo release --all              # release everything
floo gc                         # reclaim stale claims (default: 7d not listening)
floo agent-setup                # writes the floo instruction into ~/.claude/CLAUDE.md
floo version
```
