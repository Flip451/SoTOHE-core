//! Verify that Rust source files do not exceed configured line limits.
//!
//! Configuration is read from `architecture-rules.json` → `module_limits`.

use std::path::Path;

use domain::verify::{VerifyFinding, VerifyOutcome};

const ARCH_RULES_FILE: &str = "architecture-rules.json";

/// Check `.rs` file sizes against `module_limits` in architecture-rules.json.
///
/// # Errors
///
/// Returns findings when files exceed the configured thresholds.
pub fn verify(root: &Path) -> VerifyOutcome {
    let rules_path = root.join(ARCH_RULES_FILE);
    let rules_content = match std::fs::read_to_string(&rules_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot read {ARCH_RULES_FILE}: {e}"
            ))]);
        }
    };

    let rules: serde_json::Value = match serde_json::from_str(&rules_content) {
        Ok(v) => v,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "Cannot parse {ARCH_RULES_FILE}: {e}"
            ))]);
        }
    };

    let limits = match rules.get("module_limits") {
        Some(v) => v,
        None => return VerifyOutcome::pass(),
    };

    let max_lines = limits.get("max_lines").and_then(|v| v.as_u64()).unwrap_or(700) as usize;
    let warn_lines = limits.get("warn_lines").and_then(|v| v.as_u64()).unwrap_or(400) as usize;
    let excludes: Vec<&str> = limits
        .get("exclude")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut findings = Vec::new();
    let mut file_sizes = Vec::new();
    collect_rs_files(root, root, &excludes, &mut findings, &mut file_sizes);

    for (rel_path, line_count) in file_sizes {
        if line_count > max_lines {
            // Warning (not error) until Phase 1.5 refactoring reduces existing large files.
            findings.push(VerifyFinding::warning(format!(
                "{rel_path}: {line_count} lines (max {max_lines})"
            )));
        } else if line_count > warn_lines {
            findings.push(VerifyFinding::warning(format!(
                "{rel_path}: {line_count} lines (warn threshold {warn_lines})"
            )));
        }
    }

    VerifyOutcome::from_findings(findings)
}

fn collect_rs_files(
    root: &Path,
    dir: &Path,
    excludes: &[&str],
    findings: &mut Vec<VerifyFinding>,
    results: &mut Vec<(String, usize)>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            let rel = dir.strip_prefix(root).unwrap_or(dir);
            findings.push(VerifyFinding::error(format!(
                "{}: cannot read directory: {e}",
                rel.to_string_lossy()
            )));
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                let rel = dir.strip_prefix(root).unwrap_or(dir);
                findings.push(VerifyFinding::error(format!(
                    "{}: cannot read entry: {e}",
                    rel.to_string_lossy()
                )));
                continue;
            }
        };
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let rel_str = rel.to_string_lossy();

        // Ensure exclude patterns match directory boundaries (vendor/ must not match vendorized/)
        if excludes.iter().any(|exc| {
            let normalized = exc.trim_end_matches('/');
            rel_str.starts_with(&format!("{normalized}/")) || *rel_str == *normalized
        }) {
            continue;
        }

        if path.is_dir() {
            collect_rs_files(root, &path, excludes, findings, results);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let line_count = match std::fs::read_to_string(&path) {
                Ok(content) => content.lines().count(),
                Err(e) => {
                    findings
                        .push(VerifyFinding::error(format!("{rel_str}: cannot read file: {e}")));
                    continue;
                }
            };
            results.push((rel_str.into_owned(), line_count));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn setup_rules(root: &Path, max: usize, warn: usize, excludes: &[&str]) {
        let rules = serde_json::json!({
            "version": 2,
            "module_limits": {
                "max_lines": max,
                "warn_lines": warn,
                "exclude": excludes
            }
        });
        std::fs::write(root.join(ARCH_RULES_FILE), rules.to_string()).unwrap();
    }

    fn write_rs_file(root: &Path, rel: &str, lines: usize) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let content: String = (0..lines).map(|i| format!("// line {i}\n")).collect();
        std::fs::write(&path, content).unwrap();
    }

    #[test]
    fn test_module_size_passes_for_small_files() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        write_rs_file(tmp.path(), "src/small.rs", 100);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_module_size_warns_for_files_above_warn_threshold() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        write_rs_file(tmp.path(), "src/medium.rs", 450);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok()); // warnings don't fail
        assert_eq!(outcome.findings().len(), 1);
        assert!(outcome.findings()[0].to_string().contains("450 lines"));
    }

    #[test]
    fn test_module_size_warns_for_files_above_max_threshold() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &[]);
        write_rs_file(tmp.path(), "src/large.rs", 750);
        let outcome = verify(tmp.path());
        // Warning-only until Phase 1.5 refactoring completes
        assert!(outcome.is_ok());
        assert!(outcome.findings()[0].to_string().contains("750 lines"));
    }

    #[test]
    fn test_module_size_excludes_vendor_directory() {
        let tmp = TempDir::new().unwrap();
        setup_rules(tmp.path(), 700, 400, &["vendor/"]);
        write_rs_file(tmp.path(), "vendor/big-crate/src/lib.rs", 2000);
        write_rs_file(tmp.path(), "src/small.rs", 50);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_module_size_missing_rules_file_errors() {
        let tmp = TempDir::new().unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_module_size_no_module_limits_section_passes() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(ARCH_RULES_FILE), r#"{"version": 2}"#).unwrap();
        write_rs_file(tmp.path(), "src/huge.rs", 9999);
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }
}
