"""Write the floo instruction into ~/.claude/CLAUDE.md.

We use a marker block so the section can be updated or removed cleanly on
future runs. The user's other content in CLAUDE.md is left untouched.
"""
from __future__ import annotations

from pathlib import Path

MARKER_START = "<!-- floo:start -->"
MARKER_END = "<!-- floo:end -->"

INSTRUCTION = """\
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
"""


def claude_md_path() -> Path:
    return Path.home() / ".claude" / "CLAUDE.md"


def build_block() -> str:
    """The full marker-delimited block we own inside CLAUDE.md."""
    return f"{MARKER_START}\n{INSTRUCTION}{MARKER_END}\n"


def install(target: Path | None = None) -> tuple[Path, str]:
    """Append/update the floo block in CLAUDE.md.

    Returns (path, action) where action is one of: 'created', 'updated', 'unchanged'.
    """
    target = target or claude_md_path()
    target.parent.mkdir(parents=True, exist_ok=True)

    block = build_block()

    if not target.exists():
        target.write_text(block)
        return target, "created"

    existing = target.read_text()
    start = existing.find(MARKER_START)
    end = existing.find(MARKER_END)
    if start != -1 and end != -1 and end > start:
        # Replace whatever lives between markers (inclusive).
        end_full = end + len(MARKER_END)
        # Trim trailing newline of old block to avoid drift on repeated installs.
        new = existing[:start] + block.rstrip("\n") + existing[end_full:]
        if new == existing:
            return target, "unchanged"
        target.write_text(new)
        return target, "updated"

    # No existing markers, append.
    sep = "" if existing.endswith("\n") else "\n"
    target.write_text(existing + sep + "\n" + block)
    return target, "updated"
