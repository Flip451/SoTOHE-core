//! Verify convention docs README index is in sync.
//!
//! Rust port of `convention_docs.verify_index()` from
//! `scripts/convention_docs.py`.

use std::path::Path;

use domain::verify::{Finding, VerifyOutcome};
use regex::Regex;

const INDEX_START: &str = "<!-- convention-docs:start -->";
const INDEX_END: &str = "<!-- convention-docs:end -->";

/// File ordering for convention docs index rendering.
static FILE_ORDER: &[(&str, u32)] = &[
    ("architecture", 10),
    ("domain-model", 20),
    ("data-model", 30),
    ("api-design", 40),
    ("error-handling", 50),
    ("instrumentation", 60),
    ("testing", 70),
    ("naming", 80),
    ("generated-code", 90),
    ("security", 100),
];

/// Verify that the conventions README index is in sync with actual files.
///
/// # Errors
///
/// Returns error findings when the README is missing, markers are absent,
/// or the index block does not match the expected content.
pub fn verify(root: &Path) -> VerifyOutcome {
    let conventions_dir = root.join("project-docs").join("conventions");
    let readme_path = conventions_dir.join("README.md");

    let has_convention_docs = conventions_dir.is_dir()
        && std::fs::read_dir(&conventions_dir).is_ok_and(|entries| {
            entries.flatten().any(|e| {
                let name = e.file_name();
                let name_str = name.to_string_lossy();
                name_str.ends_with(".md") && name_str != "README.md"
            })
        });

    if !readme_path.is_file() {
        if has_convention_docs {
            return VerifyOutcome::from_findings(vec![Finding::error(
                "project-docs/conventions contains convention documents but is missing README.md"
                    .to_owned(),
            )]);
        }
        // No conventions bootstrapped — skip.
        return VerifyOutcome::pass();
    }

    let content = match std::fs::read_to_string(&readme_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "Cannot read project-docs/conventions/README.md: {e}"
            ))]);
        }
    };

    // Check markers exist.
    let marker_re = match Regex::new(&format!(
        "(?s){}.*?{}",
        regex::escape(INDEX_START),
        regex::escape(INDEX_END)
    )) {
        Ok(re) => re,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "Internal regex error: {e}"
            ))]);
        }
    };

    let actual_block = match marker_re.find(&content) {
        Some(m) => m.as_str().to_owned(),
        None => {
            return VerifyOutcome::from_findings(vec![Finding::error(
                "README index markers not found in project-docs/conventions/README.md".to_owned(),
            )]);
        }
    };

    let expected = match render_index_block(&conventions_dir) {
        Ok(block) => block,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(e)]);
        }
    };
    if actual_block != expected {
        return VerifyOutcome::from_findings(vec![Finding::error(
            "Convention README index is out of sync. To fix: run `cargo make conventions-update-index`."
                .to_owned(),
        )]);
    }

    VerifyOutcome::pass()
}

fn render_index_block(conventions_dir: &Path) -> Result<String, String> {
    let mut entries: Vec<(String, String)> = Vec::new();

    if let Ok(read_dir) = std::fs::read_dir(conventions_dir) {
        let mut paths: Vec<_> = read_dir
            .flatten()
            .filter(|e| {
                let name = e.file_name();
                let name_str = name.to_string_lossy();
                name_str.ends_with(".md") && name_str != "README.md"
            })
            .map(|e| e.path())
            .collect();

        paths.sort_by_key(|a| sort_key(a));

        for path in &paths {
            let file_name =
                path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let heading = extract_heading(path)
                .map_err(|e| format!("Cannot read convention doc {}: {e}", path.display()))?;
            entries.push((file_name, heading));
        }
    }

    let body = if entries.is_empty() {
        "- No convention documents yet. Add one with `/conventions:add <name>`.".to_owned()
    } else {
        entries
            .iter()
            .map(|(name, heading)| format!("- `{name}`: {heading}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(format!("{INDEX_START}\n{body}\n{INDEX_END}"))
}

fn extract_heading(path: &Path) -> Result<String, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("# ") {
            return Ok(heading.trim().to_owned());
        }
    }
    Ok(path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default())
}

fn sort_key(path: &Path) -> (u32, String) {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let order =
        FILE_ORDER.iter().find(|(name, _)| *name == stem).map(|(_, ord)| *ord).unwrap_or(100);
    (order, stem)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn setup_conventions(root: &Path, files: &[(&str, &str)], readme_content: &str) {
        let dir = root.join("project-docs").join("conventions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("README.md"), readme_content).unwrap();
        for (name, content) in files {
            std::fs::write(dir.join(name), content).unwrap();
        }
    }

    #[test]
    fn test_no_conventions_dir_passes() {
        let tmp = TempDir::new().unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_synced_index_passes() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("project-docs").join("conventions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("security.md"), "# Security\nRules here.\n").unwrap();

        let expected_block = render_index_block(&dir).unwrap();
        let readme = format!("# Conventions\n\n{expected_block}\n");
        std::fs::write(dir.join("README.md"), &readme).unwrap();

        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_out_of_sync_index_fails() {
        let tmp = TempDir::new().unwrap();
        setup_conventions(
            tmp.path(),
            &[("security.md", "# Security\n")],
            &format!("# Conventions\n\n{INDEX_START}\n- stale entry\n{INDEX_END}\n"),
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_missing_markers_fails() {
        let tmp = TempDir::new().unwrap();
        setup_conventions(
            tmp.path(),
            &[("security.md", "# Security\n")],
            "# Conventions\nNo markers here.\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_convention_docs_without_readme_fails() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("project-docs").join("conventions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("security.md"), "# Security\n").unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }
}
