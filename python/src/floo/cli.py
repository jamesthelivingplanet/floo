"""Floo CLI entry point."""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from floo import __version__
from floo import agent_setup, registry
from floo.paths import repo_root


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="floo",
        description="Sticky port assignments for parallel coding-agent dev servers.",
    )
    parser.add_argument("--version", action="version", version=f"floo {__version__}")
    sub = parser.add_subparsers(dest="command", metavar="<command>")

    sub.add_parser("version", help="Print floo version")
    sub.add_parser("list", help="Show all claims and listening status")

    p_claim = sub.add_parser("claim", help="Claim (or fetch) a port for a service")
    p_claim.add_argument("service", nargs="?", help="Service label, e.g. 'web'")
    p_claim.add_argument("--prefer", type=int, help="Preferred port number")

    p_release = sub.add_parser("release", help="Release a claim")
    p_release.add_argument("service", nargs="?", help="Service label to release")
    p_release.add_argument("--all", action="store_true", help="Release every claim")

    p_gc = sub.add_parser("gc", help="Reclaim stale claims")
    p_gc.add_argument("--older-than", default="-7 days",
                      help="SQLite datetime modifier; default '-7 days'")
    p_gc.add_argument("--dry-run", action="store_true", help="Show what would be reclaimed")

    sub.add_parser("agent-setup", help="Write the floo instruction into ~/.claude/CLAUDE.md")

    return parser


# ---------------------------------------------------------------------------
# command handlers
# ---------------------------------------------------------------------------

def cmd_list() -> int:
    claims = registry.list_claims()
    if not claims:
        print("No claims yet. Run `floo claim <service>` in a repo to make one.")
        return 0
    # Probe liveness per claim. Pure read at the registry level - we print
    # listening status without persisting it (gc handles persistence).
    from floo.scanner import is_port_free_on_os
    print(f"{'PORT':<6} {'LISTENING':<10} {'SERVICE':<14} REPO")
    for c in claims:
        listening = "no" if is_port_free_on_os(c.port) else "yes"
        print(f"{c.port:<6} {listening:<10} {c.service:<14} {c.repo_path}")
    return 0


def cmd_claim(service: str | None, prefer: int | None) -> int:
    if not service:
        _print_claim_usage_with_state()
        return 0
    rp = str(repo_root())
    result = registry.claim(rp, service, prefer=prefer)
    print(result.claim.port)
    return 0


def cmd_release(service: str | None, all_: bool) -> int:
    if all_:
        n = registry.release_all()
        print(f"Released {n} claim(s).")
        return 0
    if not service:
        _print_release_usage_with_state()
        return 0
    rp = str(repo_root())
    ok = registry.release(rp, service)
    if ok:
        print(f"Released {service}.")
        return 0
    print(f"No claim for service '{service}' in this repo.", file=sys.stderr)
    return 1


def cmd_gc(older_than: str, dry_run: bool) -> int:
    cands = registry.gc(older_than=older_than, dry_run=dry_run)
    if not cands:
        print("Nothing to reclaim.")
        return 0
    verb = "Would reclaim" if dry_run else "Reclaimed"
    for c in cands:
        print(f"{verb}: port {c.claim.port} ({c.claim.service} @ {c.claim.repo_path}) - {c.reason}")
    return 0


def cmd_agent_setup() -> int:
    path, action = agent_setup.install()
    print(f"{action.title()} floo block in {path}")
    return 0


# ---------------------------------------------------------------------------
# helpers for bare-subcommand usage hints
# ---------------------------------------------------------------------------

def _print_claim_usage_with_state() -> None:
    print("usage: floo claim <service> [--prefer PORT]")
    rp = str(repo_root())
    existing = [c for c in registry.list_claims() if c.repo_path == rp]
    if existing:
        print(f"\nExisting claims in {rp}:")
        for c in existing:
            print(f"  {c.service:<14} port {c.port}")
    else:
        print(f"\nNo claims yet in {rp}.")


def _print_release_usage_with_state() -> None:
    print("usage: floo release <service> | --all")
    rp = str(repo_root())
    existing = [c for c in registry.list_claims() if c.repo_path == rp]
    if existing:
        print(f"\nReleasable services in {rp}:")
        for c in existing:
            print(f"  {c.service:<14} port {c.port}")
    else:
        print(f"\nNo claims to release in {rp}.")


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------

def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command is None:
        parser.print_help()
        return 0
    if args.command == "version":
        print(f"floo {__version__}")
        return 0
    if args.command == "list":
        return cmd_list()
    if args.command == "claim":
        return cmd_claim(args.service, args.prefer)
    if args.command == "release":
        return cmd_release(args.service, args.all)
    if args.command == "gc":
        return cmd_gc(args.older_than, args.dry_run)
    if args.command == "agent-setup":
        return cmd_agent_setup()

    print(f"Unknown command: {args.command}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
