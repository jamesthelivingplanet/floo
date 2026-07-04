//! Floo CLI entry point.

mod agent_setup;
mod paths;
mod registry;
mod scanner;

use std::process::ExitCode;

use registry::FlooError;
use serde::Serialize;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// JSON shape emitted by `claim --json`: the claim record plus `was_new`.
#[derive(Serialize)]
struct ClaimJson<'a> {
    #[serde(flatten)]
    claim: &'a registry::Claim,
    was_new: bool,
}

/// JSON shape emitted by `list --json`: each claim record plus live
/// `listening` status observed at print time.
#[derive(Serialize)]
struct ListEntryJson<'a> {
    #[serde(flatten)]
    claim: &'a registry::Claim,
    listening: bool,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(run(&args) as u8)
}

fn run(args: &[String]) -> i32 {
    if args.is_empty() {
        print_help();
        return 0;
    }

    match args[0].as_str() {
        "-h" | "--help" => {
            print_help();
            0
        }
        "--version" | "-V" => {
            println!("floo {VERSION}");
            0
        }
        "version" => {
            println!("floo {VERSION}");
            0
        }
        "list" => cmd_list(&args[1..]),
        "claim" => cmd_claim(&args[1..]),
        "release" => cmd_release(&args[1..]),
        "gc" => cmd_gc(&args[1..]),
        "agent-setup" => cmd_agent_setup(),
        other => {
            eprintln!("Unknown command: {other}");
            2
        }
    }
}

fn print_help() {
    println!(
        "usage: floo <command> [options]

commands:
  version                Print floo version
  list                   Show all claims and listening status
                           --json
  claim <service>        Claim (or fetch) a port for a service
                           --prefer <port>
                           --json
  release <service>      Release a claim
  release --all          Release every claim
  gc                     Reclaim stale claims
                           --older-than <duration> (default '-7 days')
                           --dry-run
  agent-setup            Write the floo instruction into ~/.claude/CLAUDE.md

options:
  --version, -V          Print version and exit
  -h, --help             Show this help"
    );
}

fn open_db() -> Result<rusqlite::Connection, FlooError> {
    let path = paths::db_path()?;
    registry::connect(&path)
}

fn current_repo_path() -> Result<String, FlooError> {
    let p = paths::repo_root()?;
    Ok(p.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------------------
// command handlers
// ---------------------------------------------------------------------------

fn cmd_list(args: &[String]) -> i32 {
    let json = args.iter().any(|a| a == "--json");
    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };
    let claims = match registry::list_claims(&conn) {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };
    if json {
        let entries: Vec<ListEntryJson> = claims
            .iter()
            .map(|c| ListEntryJson {
                claim: c,
                listening: !scanner::is_port_free_on_os(c.port),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries).unwrap());
        return 0;
    }
    if claims.is_empty() {
        println!("No claims yet. Run `floo claim <service>` in a repo to make one.");
        return 0;
    }
    println!("{:<6} {:<10} {:<14} REPO", "PORT", "LISTENING", "SERVICE");
    for c in &claims {
        let listening = if scanner::is_port_free_on_os(c.port) {
            "no"
        } else {
            "yes"
        };
        println!(
            "{:<6} {:<10} {:<14} {}",
            c.port, listening, c.service, c.repo_path
        );
    }
    0
}

struct ClaimArgs {
    service: Option<String>,
    prefer: Option<u16>,
    json: bool,
}

fn parse_claim_args(args: &[String]) -> ClaimArgs {
    let mut service = None;
    let mut prefer = None;
    let mut json = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--prefer" => {
                i += 1;
                prefer = args.get(i).and_then(|s| s.parse::<u16>().ok());
            }
            "--json" => json = true,
            other => {
                if service.is_none() {
                    service = Some(other.to_string());
                }
            }
        }
        i += 1;
    }
    ClaimArgs {
        service,
        prefer,
        json,
    }
}

fn cmd_claim(raw_args: &[String]) -> i32 {
    let args = parse_claim_args(raw_args);
    if args.service.is_none() {
        return print_claim_usage_with_state();
    }
    let service = args.service.unwrap();

    let rp = match current_repo_path() {
        Ok(rp) => rp,
        Err(e) => return fail(&e),
    };
    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };
    match registry::claim(&conn, &rp, &service, args.prefer) {
        Ok(result) => {
            if args.json {
                let out = ClaimJson {
                    claim: &result.claim,
                    was_new: result.was_new,
                };
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            } else {
                println!("{}", result.claim.port);
            }
            0
        }
        Err(e) => fail(&e),
    }
}

struct ReleaseArgs {
    service: Option<String>,
    all: bool,
}

fn parse_release_args(args: &[String]) -> ReleaseArgs {
    let mut service = None;
    let mut all = false;
    for a in args {
        match a.as_str() {
            "--all" => all = true,
            other => {
                if service.is_none() {
                    service = Some(other.to_string());
                }
            }
        }
    }
    ReleaseArgs { service, all }
}

fn cmd_release(raw_args: &[String]) -> i32 {
    let args = parse_release_args(raw_args);

    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };

    if args.all {
        return match registry::release_all(&conn) {
            Ok(n) => {
                println!("Released {n} claim(s).");
                0
            }
            Err(e) => fail(&e),
        };
    }

    let Some(service) = args.service else {
        return print_release_usage_with_state();
    };

    let rp = match current_repo_path() {
        Ok(rp) => rp,
        Err(e) => return fail(&e),
    };

    match registry::release(&conn, &rp, &service) {
        Ok(true) => {
            println!("Released {service}.");
            0
        }
        Ok(false) => {
            eprintln!("No claim for service '{service}' in this repo.");
            1
        }
        Err(e) => fail(&e),
    }
}

struct GcArgs {
    older_than: String,
    dry_run: bool,
}

fn parse_gc_args(args: &[String]) -> GcArgs {
    let mut older_than = "-7 days".to_string();
    let mut dry_run = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--older-than" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    older_than = v.clone();
                }
            }
            "--dry-run" => dry_run = true,
            _ => {}
        }
        i += 1;
    }
    GcArgs {
        older_than,
        dry_run,
    }
}

fn cmd_gc(raw_args: &[String]) -> i32 {
    let args = parse_gc_args(raw_args);
    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };
    let cands = match registry::gc(&conn, &args.older_than, args.dry_run) {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };
    if cands.is_empty() {
        println!("Nothing to reclaim.");
        return 0;
    }
    let verb = if args.dry_run {
        "Would reclaim"
    } else {
        "Reclaimed"
    };
    for c in &cands {
        println!(
            "{verb}: port {} ({} @ {}) - {}",
            c.claim.port, c.claim.service, c.claim.repo_path, c.reason
        );
    }
    0
}

fn cmd_agent_setup() -> i32 {
    let target = match agent_setup::claude_md_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    match agent_setup::install(&target) {
        Ok((path, action)) => {
            let mut chars = action.chars();
            let capitalized = match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => action,
            };
            println!("{capitalized} floo block in {}", path.display());
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn print_claim_usage_with_state() -> i32 {
    println!("usage: floo claim <service> [--prefer PORT]");
    let rp = match current_repo_path() {
        Ok(rp) => rp,
        Err(e) => return fail(&e),
    };
    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };
    let existing: Vec<registry::Claim> = match registry::list_claims(&conn) {
        Ok(c) => c.into_iter().filter(|c| c.repo_path == rp).collect(),
        Err(e) => return fail(&e),
    };
    if existing.is_empty() {
        println!("\nNo claims yet in {rp}.");
    } else {
        println!("\nExisting claims in {rp}:");
        for c in &existing {
            println!("  {:<14} port {}", c.service, c.port);
        }
    }
    0
}

fn print_release_usage_with_state() -> i32 {
    println!("usage: floo release <service> | --all");
    let rp = match current_repo_path() {
        Ok(rp) => rp,
        Err(e) => return fail(&e),
    };
    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };
    let existing: Vec<registry::Claim> = match registry::list_claims(&conn) {
        Ok(c) => c.into_iter().filter(|c| c.repo_path == rp).collect(),
        Err(e) => return fail(&e),
    };
    if existing.is_empty() {
        println!("\nNo claims to release in {rp}.");
    } else {
        println!("\nReleasable services in {rp}:");
        for c in &existing {
            println!("  {:<14} port {}", c.service, c.port);
        }
    }
    0
}

fn fail(e: &FlooError) -> i32 {
    eprintln!("{e}");
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_parse_claim_args_bare() {
        let args = parse_claim_args(&strs(&[]));
        assert_eq!(args.service, None);
        assert_eq!(args.prefer, None);
    }

    #[test]
    fn test_parse_claim_args_service_only() {
        let args = parse_claim_args(&strs(&["web"]));
        assert_eq!(args.service, Some("web".to_string()));
        assert_eq!(args.prefer, None);
    }

    #[test]
    fn test_parse_claim_args_service_and_prefer() {
        let args = parse_claim_args(&strs(&["web", "--prefer", "3500"]));
        assert_eq!(args.service, Some("web".to_string()));
        assert_eq!(args.prefer, Some(3500));
    }

    #[test]
    fn test_parse_claim_args_order_independent() {
        let args = parse_claim_args(&strs(&["--prefer", "3500", "web"]));
        assert_eq!(args.service, Some("web".to_string()));
        assert_eq!(args.prefer, Some(3500));
    }

    #[test]
    fn test_parse_claim_args_invalid_prefer_is_ignored() {
        let args = parse_claim_args(&strs(&["web", "--prefer", "notaport"]));
        assert_eq!(args.service, Some("web".to_string()));
        assert_eq!(args.prefer, None);
    }

    #[test]
    fn test_parse_claim_args_json_flag() {
        let args = parse_claim_args(&strs(&["web", "--json"]));
        assert_eq!(args.service, Some("web".to_string()));
        assert!(args.json);
    }

    #[test]
    fn test_parse_claim_args_json_not_treated_as_service() {
        let args = parse_claim_args(&strs(&["--json", "web"]));
        assert_eq!(args.service, Some("web".to_string()));
        assert!(args.json);
    }

    #[test]
    fn test_claim_json_shape() {
        let claim = registry::Claim {
            repo_path: "/repo/A".to_string(),
            service: "web".to_string(),
            port: 3000,
            created_at: "2026-07-04T12:00:00Z".to_string(),
            last_seen_listening: None,
        };
        let out = ClaimJson {
            claim: &claim,
            was_new: true,
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&out).unwrap()).unwrap();
        assert_eq!(v["repo_path"], "/repo/A");
        assert_eq!(v["service"], "web");
        assert_eq!(v["port"], 3000);
        assert_eq!(v["created_at"], "2026-07-04T12:00:00Z");
        assert!(v["last_seen_listening"].is_null());
        assert_eq!(v["was_new"], true);
    }

    #[test]
    fn test_list_entry_json_shape() {
        let claim = registry::Claim {
            repo_path: "/repo/A".to_string(),
            service: "web".to_string(),
            port: 3001,
            created_at: "2026-07-04T12:00:00Z".to_string(),
            last_seen_listening: Some("2026-07-04T13:00:00Z".to_string()),
        };
        let out = ListEntryJson {
            claim: &claim,
            listening: true,
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&out).unwrap()).unwrap();
        assert_eq!(v["port"], 3001);
        assert_eq!(v["last_seen_listening"], "2026-07-04T13:00:00Z");
        assert_eq!(v["listening"], true);
    }

    #[test]
    fn test_parse_release_args_empty() {
        let args = parse_release_args(&strs(&[]));
        assert_eq!(args.service, None);
        assert!(!args.all);
    }

    #[test]
    fn test_parse_release_args_service() {
        let args = parse_release_args(&strs(&["web"]));
        assert_eq!(args.service, Some("web".to_string()));
        assert!(!args.all);
    }

    #[test]
    fn test_parse_release_args_all() {
        let args = parse_release_args(&strs(&["--all"]));
        assert_eq!(args.service, None);
        assert!(args.all);
    }

    #[test]
    fn test_parse_release_args_all_and_service() {
        let args = parse_release_args(&strs(&["--all", "web"]));
        assert_eq!(args.service, Some("web".to_string()));
        assert!(args.all);
    }

    #[test]
    fn test_parse_gc_args_default() {
        let args = parse_gc_args(&strs(&[]));
        assert_eq!(args.older_than, "-7 days");
        assert!(!args.dry_run);
    }

    #[test]
    fn test_parse_gc_args_dry_run() {
        let args = parse_gc_args(&strs(&["--dry-run"]));
        assert_eq!(args.older_than, "-7 days");
        assert!(args.dry_run);
    }

    #[test]
    fn test_parse_gc_args_older_than() {
        let args = parse_gc_args(&strs(&["--older-than", "-1 hour"]));
        assert_eq!(args.older_than, "-1 hour");
        assert!(!args.dry_run);
    }

    #[test]
    fn test_parse_gc_args_older_than_and_dry_run() {
        let args = parse_gc_args(&strs(&["--older-than", "-1 hour", "--dry-run"]));
        assert_eq!(args.older_than, "-1 hour");
        assert!(args.dry_run);
    }
}
