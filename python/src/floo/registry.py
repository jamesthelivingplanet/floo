"""SQLite-backed claim registry.

One row = one active claim (a (repo_path, service) pair holding a port).

Key design decisions:
  - Primary key is (repo_path, service): that's the natural lookup pattern.
  - port is UNIQUE: schema-level safety net against double-allocation bugs.
  - Timestamps are ISO 8601 text - SQLite has no native datetime type, but
    its datetime() function understands this format so range queries work.
  - All mutating operations run inside a BEGIN IMMEDIATE transaction. That
    grabs the SQLite write lock up front, so the read-decide-write sequence
    is atomic across agents racing in parallel.
"""
from __future__ import annotations

import sqlite3
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterator, NamedTuple

from floo.paths import db_path
from floo.scanner import is_port_free_on_os

# v1 port range. 3000 is the common dev-server default; staying in 3xxx keeps
# us clear of OS reserved ports and ephemeral ranges. Hit the ceiling -> error.
PORT_MIN = 3000
PORT_MAX = 3999

SCHEMA = """
CREATE TABLE IF NOT EXISTS claims (
    repo_path           TEXT    NOT NULL,
    service             TEXT    NOT NULL,
    port                INTEGER NOT NULL UNIQUE,
    created_at          TEXT    NOT NULL,
    last_seen_listening TEXT,
    PRIMARY KEY (repo_path, service)
);
"""


class Claim(NamedTuple):
    """One row from the claims table."""
    repo_path: str
    service: str
    port: int
    created_at: str
    last_seen_listening: str | None


def _now_iso() -> str:
    """UTC timestamp in ISO 8601. SQLite's datetime() can parse this."""
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _connect(path: Path | None = None) -> sqlite3.Connection:
    path = path or db_path()
    conn = sqlite3.connect(path, isolation_level=None)  # we manage txns manually
    # WAL: lets readers and a single writer run concurrently.
    conn.execute("PRAGMA journal_mode = WAL")
    # Wait up to 5s for the write lock before raising - common in parallel use.
    conn.execute("PRAGMA busy_timeout = 5000")
    conn.row_factory = sqlite3.Row
    conn.executescript(SCHEMA)
    return conn


@contextmanager
def connect(path: Path | None = None) -> Iterator[sqlite3.Connection]:
    """Open a connection, ensure schema, yield it, close on exit."""
    conn = _connect(path)
    try:
        yield conn
    finally:
        conn.close()


def _row_to_claim(row: sqlite3.Row) -> Claim:
    return Claim(
        repo_path=row["repo_path"],
        service=row["service"],
        port=row["port"],
        created_at=row["created_at"],
        last_seen_listening=row["last_seen_listening"],
    )


# ---------------------------------------------------------------------------
# claim
# ---------------------------------------------------------------------------

class ClaimResult(NamedTuple):
    claim: Claim
    was_new: bool  # True if we allocated a new row, False if we returned an existing one.


def claim(
    repo_path: str,
    service: str,
    prefer: int | None = None,
    *,
    conn: sqlite3.Connection | None = None,
) -> ClaimResult:
    """Get or allocate a port for (repo_path, service).

    Idempotent: calling twice returns the same port and was_new=False the
    second time. The agent restart story depends on this.
    """
    owns_conn = conn is None
    if owns_conn:
        conn = _connect()
    try:
        # BEGIN IMMEDIATE acquires the write lock right away (not lazily on the
        # first INSERT), so no other writer can wedge between our SELECT and
        # our INSERT. This is the lock that makes claim() race-free.
        conn.execute("BEGIN IMMEDIATE")

        # 1. Is there already a claim for this (repo, service)?
        existing = conn.execute(
            "SELECT * FROM claims WHERE repo_path = ? AND service = ?",
            (repo_path, service),
        ).fetchone()
        if existing is not None:
            conn.execute("COMMIT")
            return ClaimResult(claim=_row_to_claim(existing), was_new=False)

        # 2. No existing claim. Pick a port.
        port = _pick_port(conn, prefer=prefer)

        # 3. Insert. The UNIQUE constraint on `port` is a backstop here: if
        #    somehow another writer slipped past the lock (logic bug, not
        #    possible with BEGIN IMMEDIATE), SQLite refuses the duplicate.
        now = _now_iso()
        conn.execute(
            "INSERT INTO claims (repo_path, service, port, created_at, last_seen_listening) "
            "VALUES (?, ?, ?, ?, NULL)",
            (repo_path, service, port, now),
        )
        conn.execute("COMMIT")
        return ClaimResult(
            claim=Claim(repo_path=repo_path, service=service, port=port,
                        created_at=now, last_seen_listening=None),
            was_new=True,
        )
    except Exception:
        conn.execute("ROLLBACK")
        raise
    finally:
        if owns_conn:
            conn.close()


def _pick_port(conn: sqlite3.Connection, prefer: int | None) -> int:
    """Inside an open transaction, find an unused port.

    A port is "free" iff: not in the registry AND not bound by an OS process.
    """
    taken = {row["port"] for row in conn.execute("SELECT port FROM claims")}

    # Preferred port: try it first if given and in range. If it's taken, fall
    # through to the normal scan - caller treated --prefer as a hint, not a hard
    # requirement.
    if prefer is not None and PORT_MIN <= prefer <= PORT_MAX:
        if prefer not in taken and is_port_free_on_os(prefer):
            return prefer

    for candidate in range(PORT_MIN, PORT_MAX + 1):
        if candidate in taken:
            continue
        if not is_port_free_on_os(candidate):
            continue  # untracked process is bound - skip, don't adopt.
        return candidate

    raise RuntimeError(
        f"No free port in {PORT_MIN}-{PORT_MAX}. Run `floo gc` to reclaim stale claims."
    )


# ---------------------------------------------------------------------------
# release
# ---------------------------------------------------------------------------

def release(repo_path: str, service: str, *, conn: sqlite3.Connection | None = None) -> bool:
    """Release a single claim. Returns True if a row was removed."""
    owns_conn = conn is None
    if owns_conn:
        conn = _connect()
    try:
        cur = conn.execute(
            "DELETE FROM claims WHERE repo_path = ? AND service = ?",
            (repo_path, service),
        )
        return cur.rowcount > 0
    finally:
        if owns_conn:
            conn.close()


def release_all(*, conn: sqlite3.Connection | None = None) -> int:
    """Release every claim. Returns number removed."""
    owns_conn = conn is None
    if owns_conn:
        conn = _connect()
    try:
        cur = conn.execute("DELETE FROM claims")
        return cur.rowcount
    finally:
        if owns_conn:
            conn.close()


# ---------------------------------------------------------------------------
# list
# ---------------------------------------------------------------------------

def list_claims(*, conn: sqlite3.Connection | None = None) -> list[Claim]:
    """Return all claims, ordered by port. Pure read - never mutates."""
    owns_conn = conn is None
    if owns_conn:
        conn = _connect()
    try:
        rows = conn.execute("SELECT * FROM claims ORDER BY port").fetchall()
        return [_row_to_claim(r) for r in rows]
    finally:
        if owns_conn:
            conn.close()


# ---------------------------------------------------------------------------
# gc
# ---------------------------------------------------------------------------

class GcCandidate(NamedTuple):
    claim: Claim
    reason: str  # human-readable: why it's eligible


def find_gc_candidates(
    older_than: str = "-7 days",
    *,
    conn: sqlite3.Connection | None = None,
) -> list[GcCandidate]:
    """Find claims that are eligible for reclamation.

    A claim is eligible if it has not been observed listening recently. The
    `older_than` argument is SQLite-modifier syntax (e.g., '-7 days', '-1 hour').

    Three eligibility paths:
      1. last_seen_listening is older than the threshold.
      2. last_seen_listening is NULL *and* created_at is older than the
         threshold - i.e., claimed long ago but a server was never observed.
      3. The port is not currently in use by anyone - we re-probe at gc time
         to confirm before recommending reclamation. (Liveness vs. claim
         decoupling: a row may show "not seen" only because we never asked.)
    """
    owns_conn = conn is None
    if owns_conn:
        conn = _connect()
    try:
        # Wrap stored timestamps in datetime() too - our ISO 8601 with 'T' and
        # 'Z' is valid for SQLite's parser but does NOT sort-compare cleanly
        # against datetime('now', ...) which uses 'YYYY-MM-DD HH:MM:SS'.
        # datetime() on both sides normalizes them.
        rows = conn.execute(
            "SELECT * FROM claims WHERE "
            "(last_seen_listening IS NOT NULL "
            " AND datetime(last_seen_listening) < datetime('now', ?)) "
            "OR (last_seen_listening IS NULL "
            " AND datetime(created_at) < datetime('now', ?))",
            (older_than, older_than),
        ).fetchall()

        out: list[GcCandidate] = []
        for r in rows:
            c = _row_to_claim(r)
            # Re-probe at gc time. If a server is actively listening *right
            # now*, refuse to reclaim regardless of the stale timestamp - the
            # user clearly still cares about this claim.
            if not is_port_free_on_os(c.port):
                # Port is in use - update last_seen_listening so we don't
                # keep flagging it on every gc run.
                conn.execute(
                    "UPDATE claims SET last_seen_listening = ? "
                    "WHERE repo_path = ? AND service = ?",
                    (_now_iso(), c.repo_path, c.service),
                )
                continue
            reason = (
                "never seen listening" if c.last_seen_listening is None
                else f"last listening at {c.last_seen_listening}"
            )
            out.append(GcCandidate(claim=c, reason=reason))
        return out
    finally:
        if owns_conn:
            conn.close()


def gc(older_than: str = "-7 days", dry_run: bool = False) -> list[GcCandidate]:
    """Reclaim stale claims. Returns the list of (would-be-)reclaimed rows."""
    with connect() as conn:
        conn.execute("BEGIN IMMEDIATE")
        try:
            candidates = find_gc_candidates(older_than, conn=conn)
            if not dry_run:
                for cand in candidates:
                    conn.execute(
                        "DELETE FROM claims WHERE repo_path = ? AND service = ?",
                        (cand.claim.repo_path, cand.claim.service),
                    )
            conn.execute("COMMIT")
            return candidates
        except Exception:
            conn.execute("ROLLBACK")
            raise
