//! Filesystem locations and repo-root detection.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pure computation of the XDG state base directory (no IO), given explicit
/// `XDG_STATE_HOME` and `HOME` values (mirroring what `env::var(..).ok()`
/// would yield). Separated from `state_dir_from` so tests can assert on the
/// computed path without mutating process-wide environment variables or
/// touching the real filesystem.
fn base_dir_from(xdg_state_home: Option<String>, home: Option<String>) -> PathBuf {
    xdg_state_home
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = home.expect("HOME must be set");
            Path::new(&home).join(".local").join("state")
        })
        .join("floo")
}

/// Compute the XDG state directory for floo given an explicit
/// `XDG_STATE_HOME` value (mirrors what `env::var("XDG_STATE_HOME").ok()`
/// would yield), creating it if needed.
///
/// This is the seam that lets tests exercise the resolution logic without
/// mutating the process-wide environment.
fn state_dir_from(xdg_state_home: Option<String>) -> std::io::Result<PathBuf> {
    let dir = base_dir_from(xdg_state_home, env::var("HOME").ok());
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Return the XDG state directory for floo, creating it if needed.
pub fn state_dir() -> std::io::Result<PathBuf> {
    state_dir_from(env::var("XDG_STATE_HOME").ok())
}

pub fn db_path() -> std::io::Result<PathBuf> {
    Ok(state_dir()?.join("registry.db"))
}

/// Resolve the repo root starting from `start`.
///
/// Falls back to the (canonicalized) `start` directory if not inside a git
/// repo, or if git is not available. This is the seam the public
/// `repo_root()` delegates to, so tests can point it at a temp directory
/// instead of the real current working directory.
fn repo_root_from(start: &Path) -> std::io::Result<PathBuf> {
    let cwd = start.canonicalize()?;

    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(&cwd)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let trimmed = stdout.trim();
            if !trimmed.is_empty() {
                if let Ok(canon) = Path::new(trimmed).canonicalize() {
                    return Ok(canon);
                }
            }
        }
    }

    Ok(cwd)
}

/// Resolve the repo root for the current working directory.
///
/// Falls back to the (canonicalized) current directory if not inside a git
/// repo, or if git is not available.
pub fn repo_root() -> std::io::Result<PathBuf> {
    repo_root_from(&env::current_dir()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as ProcessCommand;
    use tempfile::tempdir;

    #[test]
    fn test_state_dir_from_explicit_base_creates_floo_subdir() {
        let base = tempdir().unwrap();
        let base_path = base.path().to_path_buf();
        let dir = state_dir_from(Some(base_path.to_string_lossy().into_owned())).unwrap();
        assert_eq!(dir, base_path.join("floo"));
        assert!(dir.is_dir());
    }

    #[test]
    fn test_state_dir_from_none_falls_back_to_home_local_state() {
        // Pure path computation, no filesystem writes and no process-wide
        // env mutation, so this stays hermetic under parallel test threads.
        let dir = base_dir_from(None, Some("/home/someone".to_string()));
        assert!(dir.ends_with(".local/state/floo"));
        assert_eq!(dir, Path::new("/home/someone/.local/state/floo"));
    }

    #[test]
    fn test_repo_root_from_inside_git_repo_returns_toplevel() {
        let dir = tempdir().unwrap();
        let status = ProcessCommand::new("git")
            .arg("init")
            .arg("--quiet")
            .current_dir(dir.path())
            .status()
            .unwrap();
        assert!(status.success());

        let expected = dir.path().canonicalize().unwrap();
        let got = repo_root_from(dir.path()).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn test_repo_root_from_non_git_dir_falls_back_to_itself() {
        let dir = tempdir().unwrap();
        let expected = dir.path().canonicalize().unwrap();
        let got = repo_root_from(dir.path()).unwrap();
        assert_eq!(got, expected);
    }
}
