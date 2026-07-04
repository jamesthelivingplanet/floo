//! Write the floo instruction into `~/.claude/CLAUDE.md`.
//!
//! We use a marker block so the section can be updated or removed cleanly on
//! future runs. The user's other content in CLAUDE.md is left untouched.

use std::path::{Path, PathBuf};

pub const MARKER_START: &str = "<!-- floo:start -->";
pub const MARKER_END: &str = "<!-- floo:end -->";

pub const INSTRUCTION: &str = "\
## Port allocation (floo)

When you need a port for a dev server, mock API, or any other locally bound
service, call `floo claim <service>` first and use the port it returns.
Examples of service labels: `web`, `storybook`, `api`, `db`. The same label in
the same repo always returns the same port, that's the whole point. Do **not**
guess \"next free port\" yourself; you will collide with other agents.

- `floo claim web` -> prints the port to stdout.
- `floo list` -> shows what's claimed and whether it's listening right now.
- `floo release <service>` -> release one claim. Usually unnecessary.

If `floo` is not installed, fall back to your normal behavior.
";

pub fn claude_md_path() -> std::io::Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set"))?;
    Ok(Path::new(&home).join(".claude").join("CLAUDE.md"))
}

/// The full marker-delimited block we own inside CLAUDE.md.
pub fn build_block() -> String {
    format!("{MARKER_START}\n{INSTRUCTION}{MARKER_END}\n")
}

/// Append/update the floo block in `target`.
///
/// Returns (path, action) where action is one of: "created", "updated",
/// "unchanged".
pub fn install(target: &Path) -> std::io::Result<(PathBuf, String)> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let block = build_block();

    if !target.exists() {
        std::fs::write(target, &block)?;
        return Ok((target.to_path_buf(), "created".to_string()));
    }

    let existing = std::fs::read_to_string(target)?;
    let start = existing.find(MARKER_START);
    let end = existing.find(MARKER_END);

    if let (Some(start), Some(end)) = (start, end) {
        if end > start {
            let end_full = end + MARKER_END.len();
            let trimmed_block = block.trim_end_matches('\n');
            let new = format!(
                "{}{}{}",
                &existing[..start],
                trimmed_block,
                &existing[end_full..]
            );
            if new == existing {
                return Ok((target.to_path_buf(), "unchanged".to_string()));
            }
            std::fs::write(target, &new)?;
            return Ok((target.to_path_buf(), "updated".to_string()));
        }
    }

    // No existing markers, append.
    let sep = if existing.ends_with('\n') { "" } else { "\n" };
    let new = format!("{existing}{sep}\n{block}");
    std::fs::write(target, &new)?;
    Ok((target.to_path_buf(), "updated".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_creates_when_missing() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("CLAUDE.md");
        let (path, action) = install(&target).unwrap();
        assert_eq!(action, "created");
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, build_block());
    }

    #[test]
    fn test_updates_existing_markers() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("CLAUDE.md");
        std::fs::write(
            &target,
            format!("# My notes\n\n{MARKER_START}\nold stuff\n{MARKER_END}\n\nmore notes\n"),
        )
        .unwrap();
        let (_path, action) = install(&target).unwrap();
        assert_eq!(action, "updated");
        let contents = std::fs::read_to_string(&target).unwrap();
        assert!(contents.contains("Port allocation (floo)"));
        assert!(contents.starts_with("# My notes\n\n"));
        assert!(contents.ends_with("more notes\n"));
    }

    #[test]
    fn test_unchanged_when_already_installed() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("CLAUDE.md");
        install(&target).unwrap();
        let (_path, action) = install(&target).unwrap();
        assert_eq!(action, "unchanged");
    }

    #[test]
    fn test_appends_when_no_markers() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("CLAUDE.md");
        std::fs::write(&target, "# My notes\n").unwrap();
        let (_path, action) = install(&target).unwrap();
        assert_eq!(action, "updated");
        let contents = std::fs::read_to_string(&target).unwrap();
        assert!(contents.starts_with("# My notes\n\n"));
        assert!(contents.contains(MARKER_START));
    }
}
