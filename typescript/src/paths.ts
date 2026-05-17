/**
 * Filesystem locations and repo-root detection.
 *
 * Mirrors python/src/floo/paths.py - see SPEC.md.
 */
import { execSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { resolve, join } from "node:path";

export function stateDir(): string {
  const base = process.env.XDG_STATE_HOME ?? join(homedir(), ".local", "state");
  const d = join(base, "floo");
  mkdirSync(d, { recursive: true });
  return d;
}

export function dbPath(): string {
  return join(stateDir(), "registry.db");
}

export function repoRoot(start: string = process.cwd()): string {
  const cwd = resolve(start);
  try {
    const out = execSync("git rev-parse --show-toplevel", {
      cwd,
      stdio: ["ignore", "pipe", "ignore"],
      encoding: "utf8",
    });
    return resolve(out.trim());
  } catch {
    return cwd;
  }
}
