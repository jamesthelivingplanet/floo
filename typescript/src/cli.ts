#!/usr/bin/env node
/**
 * Floo CLI - TypeScript variant.
 *
 * Talks to the same SQLite registry as the Python implementation. See
 * SPEC.md for the shared contract.
 */

// Suppress only Node's "ExperimentalWarning: SQLite ..." emit. We use
// node:sqlite intentionally; users of this CLI shouldn't see the noise on
// every invocation. All other warnings still propagate.
const originalEmit = process.emit.bind(process);
process.emit = ((event: string, ...args: unknown[]) => {
  if (event === "warning") {
    const w = args[0] as { name?: string; message?: string } | undefined;
    if (w?.name === "ExperimentalWarning" && /SQLite/i.test(w.message ?? "")) {
      return false;
    }
  }
  return (originalEmit as (e: string, ...a: unknown[]) => boolean)(event, ...args);
}) as typeof process.emit;

import { parseArgs } from "node:util";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { repoRoot } from "./paths.js";
import { claim, release, releaseAll, listClaims, gc } from "./registry.js";
import { isPortFreeOnOs } from "./scanner.js";
import { install as installAgentInstruction } from "./agent_setup.js";

function getVersion(): string {
  try {
    const here = dirname(fileURLToPath(import.meta.url));
    const pkg = JSON.parse(readFileSync(join(here, "..", "package.json"), "utf8"));
    return pkg.version as string;
  } catch {
    return "0.0.0";
  }
}

function printHelp(): void {
  process.stdout.write(
    `usage: floo <command> [options]

commands:
  version                Print floo version
  list                   Show all claims and listening status
  claim <service>        Claim (or fetch) a port for a service
                           --prefer <port>
  release <service>      Release a claim
  release --all          Release every claim
  gc                     Reclaim stale claims
                           --older-than <duration> (default '-7 days')
                           --dry-run
  agent-setup            Write the floo instruction into ~/.claude/CLAUDE.md

options:
  --version, -V          Print version and exit
  -h, --help             Show this help
`,
  );
}

async function cmdList(): Promise<number> {
  const claims = listClaims();
  if (claims.length === 0) {
    process.stdout.write("No claims yet. Run `floo claim <service>` in a repo to make one.\n");
    return 0;
  }
  process.stdout.write(`PORT   LISTENING  SERVICE        REPO\n`);
  for (const c of claims) {
    const listening = (await isPortFreeOnOs(c.port)) ? "no" : "yes";
    const port = String(c.port).padEnd(6);
    const lis = listening.padEnd(10);
    const svc = c.service.padEnd(14);
    process.stdout.write(`${port} ${lis} ${svc} ${c.repo_path}\n`);
  }
  return 0;
}

async function cmdClaim(args: string[], prefer?: number): Promise<number> {
  const service = args[0];
  if (!service) {
    printClaimUsageWithState();
    return 0;
  }
  const rp = repoRoot();
  const result = await claim(rp, service, prefer);
  process.stdout.write(`${result.claim.port}\n`);
  return 0;
}

function cmdRelease(args: string[], all: boolean): number {
  if (all) {
    const n = releaseAll();
    process.stdout.write(`Released ${n} claim(s).\n`);
    return 0;
  }
  const service = args[0];
  if (!service) {
    printReleaseUsageWithState();
    return 0;
  }
  const rp = repoRoot();
  const ok = release(rp, service);
  if (ok) {
    process.stdout.write(`Released ${service}.\n`);
    return 0;
  }
  process.stderr.write(`No claim for service '${service}' in this repo.\n`);
  return 1;
}

async function cmdGc(olderThan: string, dryRun: boolean): Promise<number> {
  const cands = await gc(olderThan, dryRun);
  if (cands.length === 0) {
    process.stdout.write("Nothing to reclaim.\n");
    return 0;
  }
  const verb = dryRun ? "Would reclaim" : "Reclaimed";
  for (const c of cands) {
    process.stdout.write(
      `${verb}: port ${c.claim.port} (${c.claim.service} @ ${c.claim.repo_path}) - ${c.reason}\n`,
    );
  }
  return 0;
}

function cmdAgentSetup(): number {
  const { path, action } = installAgentInstruction();
  process.stdout.write(`${action.charAt(0).toUpperCase() + action.slice(1)} floo block in ${path}\n`);
  return 0;
}

function printClaimUsageWithState(): void {
  process.stdout.write("usage: floo claim <service> [--prefer PORT]\n");
  const rp = repoRoot();
  const here = listClaims().filter((c) => c.repo_path === rp);
  if (here.length === 0) {
    process.stdout.write(`\nNo claims yet in ${rp}.\n`);
    return;
  }
  process.stdout.write(`\nExisting claims in ${rp}:\n`);
  for (const c of here) {
    process.stdout.write(`  ${c.service.padEnd(14)} port ${c.port}\n`);
  }
}

function printReleaseUsageWithState(): void {
  process.stdout.write("usage: floo release <service> | --all\n");
  const rp = repoRoot();
  const here = listClaims().filter((c) => c.repo_path === rp);
  if (here.length === 0) {
    process.stdout.write(`\nNo claims to release in ${rp}.\n`);
    return;
  }
  process.stdout.write(`\nReleasable services in ${rp}:\n`);
  for (const c of here) {
    process.stdout.write(`  ${c.service.padEnd(14)} port ${c.port}\n`);
  }
}

async function main(argv: string[]): Promise<number> {
  if (argv.length === 0 || argv[0] === "-h" || argv[0] === "--help") {
    printHelp();
    return 0;
  }
  if (argv[0] === "--version" || argv[0] === "-V") {
    process.stdout.write(`floo ${getVersion()}\n`);
    return 0;
  }

  const command = argv[0];
  const rest = argv.slice(1);

  if (command === "version") {
    process.stdout.write(`floo ${getVersion()}\n`);
    return 0;
  }
  if (command === "list") return cmdList();
  if (command === "claim") {
    const { values, positionals } = parseArgs({
      args: rest,
      options: { prefer: { type: "string" } },
      allowPositionals: true,
      strict: false,
    });
    const prefer = values.prefer ? Number(values.prefer) : undefined;
    return cmdClaim(positionals, prefer);
  }
  if (command === "release") {
    const { values, positionals } = parseArgs({
      args: rest,
      options: { all: { type: "boolean" } },
      allowPositionals: true,
      strict: false,
    });
    return cmdRelease(positionals, !!values.all);
  }
  if (command === "gc") {
    const { values } = parseArgs({
      args: rest,
      options: {
        "older-than": { type: "string" },
        "dry-run": { type: "boolean" },
      },
      allowPositionals: true,
      strict: false,
    });
    return cmdGc(
      (values["older-than"] as string | undefined) ?? "-7 days",
      !!values["dry-run"],
    );
  }
  if (command === "agent-setup") return cmdAgentSetup();

  process.stderr.write(`Unknown command: ${command}\n`);
  printHelp();
  return 2;
}

main(process.argv.slice(2)).then(
  (code) => process.exit(code),
  (err) => {
    process.stderr.write(`${err?.message ?? err}\n`);
    process.exit(1);
  },
);
