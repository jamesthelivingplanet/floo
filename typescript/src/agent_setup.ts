/**
 * Write the floo instruction into ~/.claude/CLAUDE.md.
 *
 * Mirrors python/src/floo/agent_setup.py. Idempotent via marker block. See
 * SPEC.md "Agent setup" for the contract.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";

export const MARKER_START = "<!-- floo:start -->";
export const MARKER_END = "<!-- floo:end -->";

export const INSTRUCTION = `## Port allocation (floo)

When you need a port for a dev server, mock API, or any other locally bound
service, call \`floo claim <service>\` first and use the port it returns.
Examples of service labels: \`web\`, \`storybook\`, \`api\`, \`db\`. The same label in
the same repo always returns the same port, that's the whole point. Do **not**
guess "next free port" yourself; you will collide with other agents.

- \`floo claim web\` -> prints the port to stdout.
- \`floo list\` -> shows what's claimed and whether it's listening right now.
- \`floo release <service>\` -> release one claim. Usually unnecessary.

If \`floo\` is not installed, fall back to your normal behavior.
`;

export function claudeMdPath(): string {
  return join(homedir(), ".claude", "CLAUDE.md");
}

export function buildBlock(): string {
  return `${MARKER_START}\n${INSTRUCTION}${MARKER_END}\n`;
}

export type InstallAction = "created" | "updated" | "unchanged";

export function install(target: string = claudeMdPath()): {
  path: string;
  action: InstallAction;
} {
  mkdirSync(dirname(target), { recursive: true });
  const block = buildBlock();

  if (!existsSync(target)) {
    writeFileSync(target, block);
    return { path: target, action: "created" };
  }

  const existing = readFileSync(target, "utf8");
  const start = existing.indexOf(MARKER_START);
  const end = existing.indexOf(MARKER_END);

  if (start !== -1 && end !== -1 && end > start) {
    const endFull = end + MARKER_END.length;
    // Trim trailing newline of new block so repeated installs don't drift.
    const replaced = existing.slice(0, start) + block.replace(/\n+$/, "") + existing.slice(endFull);
    if (replaced === existing) {
      return { path: target, action: "unchanged" };
    }
    writeFileSync(target, replaced);
    return { path: target, action: "updated" };
  }

  // No markers: append, separated by a blank line.
  const sep = existing.endsWith("\n") ? "" : "\n";
  writeFileSync(target, existing + sep + "\n" + block);
  return { path: target, action: "updated" };
}
