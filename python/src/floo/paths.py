"""Filesystem locations and repo-root detection."""
from __future__ import annotations

import os
import subprocess
from pathlib import Path


def state_dir() -> Path:
    """Return the XDG state directory for floo, creating it if needed."""
    base = os.environ.get("XDG_STATE_HOME") or str(Path.home() / ".local" / "state")
    d = Path(base) / "floo"
    d.mkdir(parents=True, exist_ok=True)
    return d


def db_path() -> Path:
    return state_dir() / "registry.db"


def repo_root(start: Path | None = None) -> Path:
    """Resolve the repo root for the given directory.

    Falls back to the directory itself if not inside a git repo.
    """
    cwd = (start or Path.cwd()).resolve()
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=cwd,
            capture_output=True,
            text=True,
            check=True,
        )
        return Path(out.stdout.strip()).resolve()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return cwd
