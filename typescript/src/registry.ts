/**
 * SQLite-backed claim registry.
 *
 * Mirrors python/src/floo/registry.py against the same schema and the same
 * on-disk file. See SPEC.md for the contract.
 *
 * Uses node:sqlite (built into Node 22+). Synchronous API matches the access
 * patterns in the Python version, which simplifies the BEGIN IMMEDIATE block.
 */
import { DatabaseSync } from "node:sqlite";
import { dbPath } from "./paths.js";
import { isPortFreeOnOs } from "./scanner.js";

export const PORT_MIN = 3000;
export const PORT_MAX = 3999;

const SCHEMA = `
CREATE TABLE IF NOT EXISTS claims (
    repo_path           TEXT    NOT NULL,
    service             TEXT    NOT NULL,
    port                INTEGER NOT NULL UNIQUE,
    created_at          TEXT    NOT NULL,
    last_seen_listening TEXT,
    PRIMARY KEY (repo_path, service)
);
`;

export interface Claim {
  repo_path: string;
  service: string;
  port: number;
  created_at: string;
  last_seen_listening: string | null;
}

export interface ClaimResult {
  claim: Claim;
  wasNew: boolean;
}

function nowIso(): string {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}

export function open(path: string = dbPath()): DatabaseSync {
  const db = new DatabaseSync(path);
  db.exec("PRAGMA journal_mode = WAL");
  db.exec("PRAGMA busy_timeout = 5000");
  db.exec(SCHEMA);
  return db;
}

export async function claim(
  repoPath: string,
  service: string,
  prefer?: number,
): Promise<ClaimResult> {
  const db = open();
  try {
    db.exec("BEGIN IMMEDIATE");

    const existing = db
      .prepare("SELECT * FROM claims WHERE repo_path = ? AND service = ?")
      .get(repoPath, service) as unknown as Claim | undefined;

    if (existing) {
      db.exec("COMMIT");
      return { claim: existing, wasNew: false };
    }

    const port = await pickPort(db, prefer);
    const now = nowIso();
    db.prepare(
      "INSERT INTO claims (repo_path, service, port, created_at, last_seen_listening) " +
        "VALUES (?, ?, ?, ?, NULL)",
    ).run(repoPath, service, port, now);
    db.exec("COMMIT");

    return {
      claim: { repo_path: repoPath, service, port, created_at: now, last_seen_listening: null },
      wasNew: true,
    };
  } catch (err) {
    try {
      db.exec("ROLLBACK");
    } catch {
      // ignore
    }
    throw err;
  } finally {
    db.close();
  }
}

async function pickPort(db: DatabaseSync, prefer?: number): Promise<number> {
  const rows = db.prepare("SELECT port FROM claims").all() as unknown as { port: number }[];
  const taken = new Set(rows.map((r) => r.port));

  if (prefer !== undefined && prefer >= PORT_MIN && prefer <= PORT_MAX) {
    if (!taken.has(prefer) && (await isPortFreeOnOs(prefer))) {
      return prefer;
    }
  }

  for (let candidate = PORT_MIN; candidate <= PORT_MAX; candidate++) {
    if (taken.has(candidate)) continue;
    if (!(await isPortFreeOnOs(candidate))) continue;
    return candidate;
  }

  throw new Error(
    `No free port in ${PORT_MIN}-${PORT_MAX}. Run \`floo gc\` to reclaim stale claims.`,
  );
}

export function release(repoPath: string, service: string): boolean {
  const db = open();
  try {
    const info = db
      .prepare("DELETE FROM claims WHERE repo_path = ? AND service = ?")
      .run(repoPath, service);
    return info.changes > 0;
  } finally {
    db.close();
  }
}

export function releaseAll(): number {
  const db = open();
  try {
    const info = db.prepare("DELETE FROM claims").run();
    return Number(info.changes);
  } finally {
    db.close();
  }
}

export function listClaims(): Claim[] {
  const db = open();
  try {
    return db.prepare("SELECT * FROM claims ORDER BY port").all() as unknown as Claim[];
  } finally {
    db.close();
  }
}

// ---------------------------------------------------------------------------
// gc
// ---------------------------------------------------------------------------

export interface GcCandidate {
  claim: Claim;
  reason: string;
}

/**
 * Find claims eligible for reclamation, re-probing each candidate.
 *
 * Mirrors python/src/floo/registry.py:find_gc_candidates. olderThan uses
 * SQLite datetime-modifier syntax (e.g., "-7 days", "-1 hour"). Both sides
 * of the timestamp comparison wrap in datetime() so ISO 8601 'T'/'Z'
 * timestamps normalize against `datetime('now', ...)`.
 */
export async function findGcCandidates(
  olderThan = "-7 days",
): Promise<GcCandidate[]> {
  const db = open();
  try {
    const rows = db
      .prepare(
        "SELECT * FROM claims WHERE " +
          "(last_seen_listening IS NOT NULL " +
          " AND datetime(last_seen_listening) < datetime('now', ?)) " +
          "OR (last_seen_listening IS NULL " +
          " AND datetime(created_at) < datetime('now', ?))",
      )
      .all(olderThan, olderThan) as unknown as Claim[];

    const out: GcCandidate[] = [];
    for (const c of rows) {
      // Re-probe at gc time. If something is listening right now, do not
      // reclaim; instead refresh last_seen_listening so this row stops
      // showing up on every gc run.
      if (!(await isPortFreeOnOs(c.port))) {
        db.prepare(
          "UPDATE claims SET last_seen_listening = ? " +
            "WHERE repo_path = ? AND service = ?",
        ).run(nowIso(), c.repo_path, c.service);
        continue;
      }
      out.push({
        claim: c,
        reason:
          c.last_seen_listening === null
            ? "never seen listening"
            : `last listening at ${c.last_seen_listening}`,
      });
    }
    return out;
  } finally {
    db.close();
  }
}

export async function gc(
  olderThan = "-7 days",
  dryRun = false,
): Promise<GcCandidate[]> {
  const candidates = await findGcCandidates(olderThan);
  if (dryRun || candidates.length === 0) return candidates;

  const db = open();
  try {
    db.exec("BEGIN IMMEDIATE");
    const stmt = db.prepare(
      "DELETE FROM claims WHERE repo_path = ? AND service = ?",
    );
    for (const c of candidates) {
      stmt.run(c.claim.repo_path, c.claim.service);
    }
    db.exec("COMMIT");
    return candidates;
  } catch (err) {
    try {
      db.exec("ROLLBACK");
    } catch {
      // ignore
    }
    throw err;
  } finally {
    db.close();
  }
}
