# floo-ports

Sticky port assignments for parallel coding-agent dev servers. The same
`(repo, service)` pair always gets the same port, so restarting a dev
server, switching branches, or rebooting doesn't shuffle your ports around.

The mapping is backed by a local SQLite registry at
`~/.local/state/floo/registry.db`.

## Install

```sh
cargo install floo-ports
```

This installs a binary named `floo` (the package on crates.io is
`floo-ports`, but the command you run is `floo`).

## Usage

```sh
$ cd ~/dev/myapp
$ floo claim web
3001

$ floo claim web
3001              # same repo, same service, same port every time
```

Other commands:

```sh
floo claim <service> [--prefer <port>]
floo list
floo release <service>
floo release --all
floo gc [--older-than '-7 days'] [--dry-run]
floo agent-setup
floo version
```

## Use it in a Next.js / React app

```json
{
  "scripts": {
    "dev": "next dev -p $(floo claim web)",
    "storybook": "storybook dev -p $(floo claim storybook)"
  }
}
```

`floo claim web` returns the same port every time for this repo, so
`npm run dev` always binds the same port across restarts, branch switches,
and reboots. A git worktree of the same repo gets a different port
automatically, so parallel work never collides.

The same pattern works for any framework that takes a port flag (Vite,
Remix, plain node), not just Next.js.

## Documentation

This page is the crates.io summary. For the full picture, including the
problem floo solves, worked examples, and design notes, see the
[project README](https://gitlab.com/ajlebaron/floo/-/blob/main/README.md).

For the exact on-disk schema and CLI contract, see
[SPEC.md](https://gitlab.com/ajlebaron/floo/-/blob/main/SPEC.md).

## Status

Alpha. Linux and macOS only.
