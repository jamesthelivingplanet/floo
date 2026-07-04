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

    /// Extract the canonical agent-setup block from the top-level SPEC.md.
    ///
    /// SPEC.md is the single source of truth for the block text. This finds
    /// the fenced code block that contains the floo markers and returns its
    /// body verbatim, including the markers and the single trailing newline.
    /// The golden test below pins `build_block()` to this exact text so the
    /// Rust source and SPEC.md cannot drift independently.
    fn canonical_block_from_spec() -> String {
        let spec = include_str!("../../SPEC.md");

        let start = spec
            .find(MARKER_START)
            .expect("SPEC.md must contain the floo:start marker");
        // The opening code fence is the last "```" before the start marker.
        let fence_open = spec[..start]
            .rfind("```")
            .expect("SPEC.md must open a code fence before the floo:start marker");
        // Skip the whole opening fence line ("```" plus any language hint).
        let fence_line_end = spec[fence_open..]
            .find('\n')
            .expect("SPEC.md opening fence must end with a newline");
        let content_start = fence_open + fence_line_end + 1;

        let end = spec[content_start..]
            .find(MARKER_END)
            .expect("SPEC.md must contain the floo:end marker");
        let end = content_start + end;
        // Include the single trailing newline after the closing marker, but
        // stop before the closing code fence line.
        let marker_line_end = spec[end..]
            .find('\n')
            .map(|i| end + i + 1)
            .unwrap_or(spec.len());

        spec[content_start..marker_line_end].to_string()
    }

    /// Golden test: the block we generate must be byte-identical to the
    /// canonical text in SPEC.md (markers, body, single trailing newline).
    /// Changing `INSTRUCTION` (or SPEC.md) on one side without the other
    /// fails this test.
    #[test]
    fn build_block_is_byte_identical_to_spec() {
        let expected = canonical_block_from_spec();

        // Structural sanity checks so a SPEC parsing bug fails loudly rather
        // than accidentally matching an unrelated slice of the document.
        assert!(
            expected.starts_with(MARKER_START),
            "parsed SPEC block must start with the start marker; got: {expected:?}"
        );
        assert!(
            expected.ends_with(&format!("{MARKER_END}\n")),
            "parsed SPEC block must end with the end marker and a single \
             newline; got: {expected:?}"
        );

        let actual = build_block();
        assert_eq!(
            actual, expected,
            "build_block() must be byte-identical to the canonical block in \
             SPEC.md. If you changed the instruction text, update SPEC.md too \
             (or vice versa).\n\
             --- expected (from SPEC.md) ---\n{expected}\
             --- actual (from build_block) ---\n{actual}"
        );
    }

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
