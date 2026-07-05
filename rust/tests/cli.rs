//! Integration tests for the floo CLI surface and exit codes.
//!
//! Every invocation isolates itself with a fresh XDG_STATE_HOME and HOME so
//! tests never touch the real registry or the real ~/.claude/CLAUDE.md.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

/// Build a `floo` Command isolated to the given state/home directories, with
/// its working directory set to `cwd` (a temp dir is not a git repo, so
/// repo_root falls back to it, which keeps repo-root detection deterministic).
fn floo_cmd(xdg_state_home: &Path, home: &Path, cwd: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_floo"));
    cmd.env("XDG_STATE_HOME", xdg_state_home)
        .env("HOME", home)
        .current_dir(cwd);
    cmd
}

/// Run `floo` with the given args in a brand-new, isolated state/home/cwd.
fn run_isolated(args: &[&str]) -> Output {
    let state = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    floo_cmd(state.path(), home.path(), state.path())
        .args(args)
        .output()
        .unwrap()
}

fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn test_version_command() {
    let output = run_isolated(&["version"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout_str(&output),
        format!("floo {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn test_version_flag_long() {
    let output = run_isolated(&["--version"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout_str(&output),
        format!("floo {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn test_version_flag_short() {
    let output = run_isolated(&["-V"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout_str(&output),
        format!("floo {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn test_no_args_prints_help() {
    let output = run_isolated(&[]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout_str(&output).contains("usage: floo"));
}

#[test]
fn test_help_flag_short() {
    let output = run_isolated(&["-h"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout_str(&output).contains("usage: floo"));
}

#[test]
fn test_help_flag_long() {
    let output = run_isolated(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout_str(&output).contains("usage: floo"));
}

#[test]
fn test_unknown_command() {
    let output = run_isolated(&["frobnicate"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr_str(&output).contains("Unknown command: frobnicate"));
}

#[test]
fn test_claim_prints_bare_port_in_range() {
    let output = run_isolated(&["claim", "web"]);
    assert_eq!(output.status.code(), Some(0));
    let out = stdout_str(&output);
    assert!(out.ends_with('\n'));
    let port: u16 = out
        .trim_end()
        .parse()
        .expect("stdout should be a bare port number");
    assert!((3000..=3999).contains(&port));
    assert_eq!(
        out.matches('\n').count(),
        1,
        "stdout should be exactly the port and a newline"
    );
}

#[test]
fn test_claim_is_idempotent_across_invocations() {
    let state = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();

    let first = floo_cmd(state.path(), home.path(), state.path())
        .args(["claim", "web"])
        .output()
        .unwrap();
    let second = floo_cmd(state.path(), home.path(), state.path())
        .args(["claim", "web"])
        .output()
        .unwrap();

    assert_eq!(first.status.code(), Some(0));
    assert_eq!(second.status.code(), Some(0));
    assert_eq!(stdout_str(&first), stdout_str(&second));
}

#[test]
fn test_claim_reuse_prints_stderr_note_but_bare_port_on_stdout() {
    let state = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();

    let first = floo_cmd(state.path(), home.path(), state.path())
        .args(["claim", "web"])
        .output()
        .unwrap();
    let second = floo_cmd(state.path(), home.path(), state.path())
        .args(["claim", "web"])
        .output()
        .unwrap();

    assert_eq!(first.status.code(), Some(0));
    assert_eq!(second.status.code(), Some(0));

    let port: u16 = stdout_str(&first)
        .trim_end()
        .parse()
        .expect("stdout should be a bare port number");
    assert!((3000..=3999).contains(&port));
    assert_eq!(
        stdout_str(&first),
        stdout_str(&second),
        "reuse must not change stdout"
    );

    assert!(
        !stderr_str(&first).contains("reusing existing claim"),
        "a fresh claim must not print a reuse note"
    );
    let err2 = stderr_str(&second);
    assert!(
        err2.contains("reusing existing claim"),
        "expected reuse note on stderr, got: {err2}"
    );
    assert!(err2.contains(&port.to_string()));
}

#[test]
fn test_url_prints_localhost_url_matching_claim() {
    let state = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();

    let claim = floo_cmd(state.path(), home.path(), state.path())
        .args(["claim", "web"])
        .output()
        .unwrap();
    let url = floo_cmd(state.path(), home.path(), state.path())
        .args(["url", "web"])
        .output()
        .unwrap();

    assert_eq!(claim.status.code(), Some(0));
    assert_eq!(url.status.code(), Some(0));

    let port = stdout_str(&claim);
    let port = port.trim_end();
    assert_eq!(stdout_str(&url), format!("http://localhost:{port}\n"));
}

#[test]
fn test_release_missing_claim() {
    let output = run_isolated(&["release", "nope"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr_str(&output).contains("No claim for service 'nope' in this repo."));
}

#[test]
fn test_list_empty_registry() {
    let output = run_isolated(&["list"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout_str(&output).contains("No claims yet"));
}

#[test]
fn test_db_flag_overrides_registry_location() {
    let state = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    // Nested, not-yet-existing path, to exercise parent-dir creation too.
    let custom_db = state
        .path()
        .join("custom")
        .join("nested")
        .join("registry.db");

    let claim = floo_cmd(state.path(), home.path(), state.path())
        .args(["claim", "web", "--db", custom_db.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(claim.status.code(), Some(0));
    assert!(
        custom_db.is_file(),
        "custom db file should have been created"
    );

    let xdg_default = state.path().join("floo").join("registry.db");
    assert!(
        !xdg_default.exists(),
        "the XDG default registry should not have been touched"
    );

    let list = floo_cmd(state.path(), home.path(), state.path())
        .args(["list", "--db", custom_db.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert_eq!(list.status.code(), Some(0));
    let list_out = stdout_str(&list);
    assert!(list_out.contains("\"service\": \"web\""));
}

#[test]
fn test_floo_db_env_overrides_registry_location() {
    let state = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let custom_db = state
        .path()
        .join("envdir")
        .join("nested")
        .join("registry.db");

    let claim = floo_cmd(state.path(), home.path(), state.path())
        .env("FLOO_DB", &custom_db)
        .args(["claim", "web"])
        .output()
        .unwrap();
    assert_eq!(claim.status.code(), Some(0));
    assert!(
        custom_db.is_file(),
        "custom db file should have been created"
    );

    let xdg_default = state.path().join("floo").join("registry.db");
    assert!(
        !xdg_default.exists(),
        "the XDG default registry should not have been touched"
    );

    let list = floo_cmd(state.path(), home.path(), state.path())
        .env("FLOO_DB", &custom_db)
        .args(["list", "--json"])
        .output()
        .unwrap();
    assert_eq!(list.status.code(), Some(0));
    let list_out = stdout_str(&list);
    assert!(list_out.contains("\"service\": \"web\""));
}

#[test]
fn test_claim_help_is_command_specific() {
    let output = run_isolated(&["claim", "--help"]);
    assert_eq!(output.status.code(), Some(0));
    let out = stdout_str(&output);
    assert!(out.contains("--prefer"));
    assert!(out.contains("--json"));
}

#[test]
fn test_gc_help_is_command_specific() {
    let output = run_isolated(&["gc", "--help"]);
    assert_eq!(output.status.code(), Some(0));
    let out = stdout_str(&output);
    assert!(out.contains("--older-than"));
    assert!(out.contains("--dry-run"));
}

#[test]
fn test_completions_bash() {
    let output = run_isolated(&["completions", "bash"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout_str(&output).contains("floo"));
}

#[test]
fn test_completions_zsh() {
    let output = run_isolated(&["completions", "zsh"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout_str(&output).contains("floo"));
}

#[test]
fn test_completions_fish() {
    let output = run_isolated(&["completions", "fish"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout_str(&output).contains("floo"));
}

#[test]
fn test_completions_invalid_shell() {
    let output = run_isolated(&["completions", "notashell"]);
    assert_ne!(output.status.code(), Some(0));
}
