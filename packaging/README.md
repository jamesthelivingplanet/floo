# Packaging

Package-manager distribution for floo. Both packages build from the immutable
`floo-ports` source tarball published on crates.io, so they can ship before the
prebuilt-binary (`-bin` / bottle) variants exist.

- `homebrew/floo.rb` is a source Homebrew formula for macOS.
- `aur/PKGBUILD` (+ `.SRCINFO`) is a source AUR package for Arch Linux.

Both are pinned to `floo-ports` 0.0.2. When you cut a new release, bump the
version and refresh the checksum in both places (see below).

## Homebrew tap

Homebrew formulae outside of homebrew-core are distributed through a tap, which
is just a git repo whose name starts with `homebrew-`. To publish:

1. Create a repo named `homebrew-floo` under the same account, for example
   `https://gitlab.com/ajlebaron/homebrew-floo`.
2. Copy `homebrew/floo.rb` into that repo at `Formula/floo.rb`.
3. Commit and push.

Users then install with the instructions in the top-level README. The formula
builds from source and needs the Xcode command line tools (for the C compiler
that compiles bundled SQLite) plus the `rust` build dependency, which Homebrew
installs automatically.

## AUR package

The AUR hosts one git repo per package. To publish:

1. Clone the AUR repo (creating it on first push):
   `git clone ssh://aur@aur.archlinux.org/floo.git`
2. Copy `aur/PKGBUILD` and `aur/.SRCINFO` into it.
3. Commit and push. The AUR requires `.SRCINFO` to be committed alongside
   `PKGBUILD` and kept in sync.

`options=(!lto)` is required: rusqlite compiles bundled SQLite from C, and
makepkg's default LTO leaves those symbols unresolvable at the Rust link step.

## Bumping the version

When `floo-ports` publishes a new version on crates.io:

Homebrew (`homebrew/floo.rb`):

- Update the version in the `url`.
- Update `sha256` to the new tarball's checksum.

AUR (`aur/PKGBUILD` and `aur/.SRCINFO`):

- Update `pkgver` (reset `pkgrel` to 1).
- Refresh the checksum: run `updpkgsums` in `aur/`, or compute it with
  `curl -sL https://static.crates.io/crates/floo-ports/floo-ports-<version>.crate | sha256sum`.
- Regenerate `.SRCINFO`: `makepkg --printsrcinfo > .SRCINFO`.
- Rebuild locally to confirm: `makepkg -f`.

The checksum for a `floo-ports` tarball is the sha256 of the file at
`https://static.crates.io/crates/floo-ports/floo-ports-<version>.crate`.
