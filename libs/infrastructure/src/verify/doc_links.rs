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
        let mut in_fenced_block = false;

        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_fenced_block = !in_fenced_block;
                continue;
            }
            if in_fenced_block {
                continue;
            }

            // Strip inline code spans before scanning for links
            let line_without_code = strip_inline_code(line);
            for cap in link_re.captures_iter(&line_without_code) {
                let link_target = &cap[2];

                // Skip URLs, anchors, and mailto
                if link_target.starts_with("http://")
                    || link_target.starts_with("https://")
                    || link_target.starts_with('#')
                    || link_target.starts_with("mailto:")
                {
                    continue;
                }

                // Strip optional title attribute: [text](path "title") or [text](path 'title')
                // Split on space+quote to avoid truncating apostrophes in filenames.
                let link_target = link_target
                    .split_once(" \"")
                    .or_else(|| link_target.split_once(" '"))
                    .map_or(link_target, |(path, _)| path)
                    .trim();
                // Strip anchor fragment from path
                let path_part = link_target.split('#').next().unwrap_or(link_target);
                if path_part.is_empty() {
                    continue;
                }

                let resolved = if path_part.starts_with('/') {
                    root.join(path_part.trim_start_matches('/'))
                } else {
                    md_dir.join(path_part)
                };

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
/// Remove inline code spans (`` `...` ``) from a line so that link-like
/// syntax inside code is not treated as an actual Markdown link.
fn strip_inline_code(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '`' {
            // Skip until closing backtick
            for inner in chars.by_ref() {
                if inner == '`' {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn collect_md_files(root: &Path) -> Result<Vec<std::path::PathBuf>, std::io::Error> {
    let mut result = Vec::new();
    collect_md_files_recursive(root, &mut result)?;
    Ok(result)
}

fn collect_md_files_recursive(
    dir: &Path,
    out: &mut Vec<std::path::PathBuf>,
) -> Result<(), std::io::Error> {
    const SKIP_EXACT: &[&str] =
        &["target", ".git", "node_modules", "vendor", ".cache", "tmp", "track"];
    const SKIP_PREFIXES: &[&str] = &["target-"];

    let entries = std::fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        if path.is_dir() {
            if SKIP_EXACT.iter().any(|&s| *name == *s)
                || SKIP_PREFIXES.iter().any(|&p| name.starts_with(p))
            {
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
    fn test_dotfile_directories_are_scanned() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), ".claude/rules/test.md", "See [x](nonexistent.md).");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "dotfile directories should be scanned for doc links");
    }

    #[test]
    fn test_git_directory_is_skipped() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), ".git/hooks/broken.md", "See [x](nonexistent.md).");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), ".git directory should be skipped");
    }

    #[test]
    fn test_links_to_tmp_are_detected_as_broken() {
        let tmp = TempDir::new().unwrap();
        // Links to tmp/ (gitignored) should be caught as broken
        write_file(tmp.path(), "knowledge/strategy/plan.md", "See [detail](../../tmp/scratch.md).");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "links to tmp/ should be detected as broken");
    }

    #[test]
    fn test_multiple_broken_links_reported() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "index.md", "See [a](a.md) and [b](b.md).");
        let outcome = verify(tmp.path());
        assert_eq!(outcome.findings().len(), 2, "both broken links should be reported");
    }

    #[test]
    fn test_links_inside_fenced_code_blocks_are_skipped() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "index.md",
            "text\n```\nSee [broken](nonexistent.md)\n```\nmore text",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "links inside fenced code blocks should be skipped");
    }

    #[test]
    fn test_link_with_title_attribute_resolves_path() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "target.md", "# Target");
        write_file(tmp.path(), "index.md", r#"See [t](target.md "hover text")."#);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "link with title attribute should resolve path part");
    }

    #[test]
    fn test_link_after_fenced_block_is_checked() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "index.md", "```\ncode\n```\nSee [broken](nonexistent.md)");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "link after fenced block should be checked");
    }
}
