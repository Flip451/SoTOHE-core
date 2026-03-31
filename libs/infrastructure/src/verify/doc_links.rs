//! Verify that local file links in Markdown documents resolve to existing files.
//!
//! Scans `.md` files under the project root for `[text](path)` links where `path`
//! is a relative local path (not a URL, not an anchor-only reference). Reports an
//! error finding for each link whose target does not exist on disk.

use std::path::Path;
use std::sync::LazyLock;

use domain::verify::{Finding, VerifyOutcome};
use regex::Regex;

static LINK_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").ok());

/// Scan all `.md` files under `root` and verify local links resolve.
///
/// # Errors
///
/// Returns error findings for each broken local link.
pub fn verify(root: &Path) -> VerifyOutcome {
    let link_re = match LINK_RE.as_ref() {
        Some(re) => re,
        None => {
            return VerifyOutcome::from_findings(vec![Finding::error(
                "Failed to compile link regex".to_owned(),
            )]);
        }
    };

    let mut findings = Vec::new();

    let md_files = match collect_md_files(root) {
        Ok(files) => files,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "Failed to collect markdown files: {e}"
            ))]);
        }
    };

    for md_path in &md_files {
        let content = match std::fs::read_to_string(md_path) {
            Ok(c) => c,
            Err(e) => {
                findings.push(Finding::error(format!("Cannot read {}: {e}", md_path.display())));
                continue;
            }
        };

        let md_dir = md_path.parent().unwrap_or(root);

        for (line_num, line) in content.lines().enumerate() {
            // Skip fenced code blocks
            if line.trim_start().starts_with("```") {
                continue;
            }

            for cap in link_re.captures_iter(line) {
                let link_target = &cap[2];

                // Skip URLs, anchors, and mailto
                if link_target.starts_with("http://")
                    || link_target.starts_with("https://")
                    || link_target.starts_with('#')
                    || link_target.starts_with("mailto:")
                {
                    continue;
                }

                // Strip anchor fragment from path
                let path_part = link_target.split('#').next().unwrap_or(link_target);
                if path_part.is_empty() {
                    continue;
                }

                let resolved = md_dir.join(path_part);
                if !resolved.exists() {
                    let rel_md = md_path.strip_prefix(root).unwrap_or(md_path);
                    findings.push(Finding::error(format!(
                        "{}:{}: broken link to '{}'",
                        rel_md.display(),
                        line_num + 1,
                        path_part
                    )));
                }
            }
        }
    }

    VerifyOutcome::from_findings(findings)
}

/// Recursively collect all `.md` files under `root`, skipping hidden directories
/// and common non-doc directories.
fn collect_md_files(root: &Path) -> Result<Vec<std::path::PathBuf>, std::io::Error> {
    let mut result = Vec::new();
    collect_md_files_recursive(root, &mut result)?;
    Ok(result)
}

fn collect_md_files_recursive(
    dir: &Path,
    out: &mut Vec<std::path::PathBuf>,
) -> Result<(), std::io::Error> {
    const SKIP_DIRS: &[&str] =
        &["target", "target-", ".git", "node_modules", "vendor", ".cache", "tmp"];

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        if path.is_dir() {
            // Skip hidden directories
            if name.starts_with('.') {
                continue;
            }
            // Skip known non-doc directories
            if SKIP_DIRS.iter().any(|&s| name == s || name.starts_with(s)) {
                continue;
            }
            collect_md_files_recursive(&path, out)?;
        } else if name.ends_with(".md") {
            out.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn write_file(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    #[test]
    fn test_valid_local_link_passes() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "target.md", "# Target");
        write_file(tmp.path(), "index.md", "See [target](target.md).");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "valid link should pass");
    }

    #[test]
    fn test_broken_local_link_fails() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "index.md", "See [missing](missing.md).");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "broken link should fail");
        let msg = outcome.findings().first().expect("should have at least one finding").message();
        assert!(msg.contains("missing.md"), "finding should name the broken link");
    }

    #[test]
    fn test_url_links_are_skipped() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "index.md", "See [docs](https://example.com).");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "URL links should be skipped");
    }

    #[test]
    fn test_anchor_links_are_skipped() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "index.md", "See [section](#heading).");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "anchor links should be skipped");
    }

    #[test]
    fn test_link_with_anchor_fragment_resolves_path_only() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "target.md", "# Target");
        write_file(tmp.path(), "index.md", "See [target](target.md#section).");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "link with anchor fragment should resolve path part");
    }

    #[test]
    fn test_relative_link_to_subdirectory() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "docs/guide.md", "# Guide");
        write_file(tmp.path(), "index.md", "See [guide](docs/guide.md).");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "relative link to subdirectory should resolve");
    }

    #[test]
    fn test_link_from_subdirectory_to_parent() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "README.md", "# Root");
        write_file(tmp.path(), "docs/index.md", "See [root](../README.md).");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "relative link to parent should resolve");
    }

    #[test]
    fn test_hidden_directories_are_skipped() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), ".hidden/broken.md", "See [x](nonexistent.md).");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "hidden directories should be skipped");
    }

    #[test]
    fn test_multiple_broken_links_reported() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "index.md", "See [a](a.md) and [b](b.md).");
        let outcome = verify(tmp.path());
        assert_eq!(outcome.findings().len(), 2, "both broken links should be reported");
    }
}
