//! Verify that canonical module ownership rules are not violated.
//!
//! Scans Rust source files for forbidden patterns that indicate
//! reimplementation of functionality owned by a canonical module.
//! Rules are declared in `architecture-rules.json` under
//! `canonical_modules`.

use std::path::Path;

use domain::verify::{Finding, VerifyOutcome};
use regex::Regex;
use serde::Deserialize;

const ARCH_RULES_FILE: &str = "architecture-rules.json";

/// Raw deserialization target for `architecture-rules.json`.
/// Invalid data (missing fields, wrong types) causes a serde error
/// rather than silent data loss.
#[derive(Debug, Deserialize)]
struct ArchitectureRules {
    #[serde(default)]
    canonical_modules: Vec<RawCanonicalRule>,
}

/// A single canonical module rule as stored in JSON.
#[derive(Debug, Deserialize)]
struct RawCanonicalRule {
    concern: String,
    forbidden_patterns: Vec<String>,
    allowed_in: Vec<String>,
    #[serde(default)]
    convention: String,
}

/// A parsed canonical module rule with compiled regexes.
#[derive(Debug)]
struct CanonicalRule {
    concern: String,
    forbidden_patterns: Vec<Regex>,
    allowed_in: Vec<String>,
    convention: String,
}

/// Verify that no Rust source file outside `allowed_in` directories contains
/// forbidden patterns declared in `canonical_modules`.
///
/// # Errors
///
/// Returns findings when the rules file is missing, malformed, or when
/// forbidden patterns are found outside their canonical module.
pub fn verify(root: &Path) -> VerifyOutcome {
    let rules_path = root.join(ARCH_RULES_FILE);
    let content = match std::fs::read_to_string(&rules_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "cannot read {ARCH_RULES_FILE}: {e}"
            ))]);
        }
    };

    let arch_rules: ArchitectureRules = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "cannot parse {ARCH_RULES_FILE}: {e}"
            ))]);
        }
    };

    let rules = match compile_canonical_rules(&arch_rules.canonical_modules) {
        Ok(r) => r,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(e)]);
        }
    };

    if rules.is_empty() {
        return VerifyOutcome::pass();
    }

    let mut findings = Vec::new();
    scan_rust_files(root, &rules, &mut findings);

    if findings.is_empty() { VerifyOutcome::pass() } else { VerifyOutcome::from_findings(findings) }
}

/// Compiles raw deserialized rules into `CanonicalRule` with compiled regexes.
fn compile_canonical_rules(raw_rules: &[RawCanonicalRule]) -> Result<Vec<CanonicalRule>, String> {
    let mut rules = Vec::new();
    for entry in raw_rules {
        let mut forbidden_patterns = Vec::new();
        for s in &entry.forbidden_patterns {
            let re = Regex::new(s).map_err(|e| format!("invalid regex '{s}': {e}"))?;
            forbidden_patterns.push(re);
        }

        rules.push(CanonicalRule {
            concern: entry.concern.clone(),
            forbidden_patterns,
            allowed_in: entry.allowed_in.clone(),
            convention: entry.convention.clone(),
        });
    }

    Ok(rules)
}

fn scan_rust_files(root: &Path, rules: &[CanonicalRule], findings: &mut Vec<Finding>) {
    // Walk libs/ and apps/ directories for .rs files
    for dir_name in &["libs", "apps"] {
        let dir = root.join(dir_name);
        if dir.is_dir() {
            walk_dir(&dir, root, rules, findings);
        }
    }
}

fn walk_dir(dir: &Path, root: &Path, rules: &[CanonicalRule], findings: &mut Vec<Finding>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            findings.push(Finding::error(format!("cannot read directory {}: {e}", dir.display())));
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                findings
                    .push(Finding::error(format!("cannot read entry in {}: {e}", dir.display())));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, root, rules, findings);
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("rs") {
            continue;
        }

        check_file(&path, root, rules, findings);
    }
}

fn check_file(path: &Path, root: &Path, rules: &[CanonicalRule], findings: &mut Vec<Finding>) {
    let rel = match path.strip_prefix(root) {
        Ok(r) => r.to_string_lossy().to_string(),
        Err(_) => return,
    };

    // Normalize path separators for matching
    let rel_normalized = rel.replace('\\', "/");

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            findings.push(Finding::error(format!("cannot read file {}: {e}", path.display())));
            return;
        }
    };

    for rule in rules {
        // Skip files inside allowed directories
        if rule.allowed_in.iter().any(|allowed| rel_normalized.starts_with(allowed)) {
            continue;
        }

        // Skip test modules — patterns inside #[cfg(test)] are acceptable
        // (test helpers may legitimately construct similar patterns)
        for (line_num, line) in content.lines().enumerate() {
            for pattern in &rule.forbidden_patterns {
                if pattern.is_match(line) {
                    // Check if we're inside a #[cfg(test)] block
                    if is_inside_test_module(&content, line_num) {
                        continue;
                    }

                    let convention_ref = if rule.convention.is_empty() {
                        String::new()
                    } else {
                        format!(" (see {})", rule.convention)
                    };

                    findings.push(Finding::error(format!(
                        "{}:{}: forbidden pattern for '{}' concern: `{}`{}",
                        rel_normalized,
                        line_num + 1,
                        rule.concern,
                        line.trim(),
                        convention_ref,
                    )));
                }
            }
        }
    }
}

/// Heuristic: check if the given line index is inside a `#[cfg(test)]` module.
///
/// Finds the nearest `#[cfg(test)]` attribute followed by a `mod` declaration,
/// then uses brace-depth tracking to determine whether `target_line` falls
/// within that module's braces. This avoids false-negatives where production
/// code after a test module is misclassified as test code.
fn is_inside_test_module(content: &str, target_line: usize) -> bool {
    let lines: Vec<&str> = content.lines().collect();

    // Scan backwards to find the nearest #[cfg(test)] + mod declaration
    let mut cfg_test_line = None;
    for i in (0..=target_line).rev() {
        let Some(line) = lines.get(i) else { continue };
        let trimmed = line.trim();

        // Found a module boundary — check if it's a test module
        if trimmed.starts_with("pub mod ") || trimmed.starts_with("mod ") {
            // Check if this mod's preceding attributes include #[cfg(test)].
            // Walk backwards through blank lines and #[...] attributes only;
            // stop at the first non-attribute, non-blank line.
            let mut is_test_mod = false;
            for j in (0..i).rev() {
                let prev = lines.get(j).map_or("", |l| l.trim());
                if prev.is_empty() {
                    continue;
                }
                if prev.starts_with("#[") {
                    if prev.contains("#[cfg(test)]") {
                        is_test_mod = true;
                        break;
                    }
                    continue;
                }
                // Non-attribute, non-blank line — stop scanning
                break;
            }
            if is_test_mod {
                cfg_test_line = Some(i);
            }
            break;
        }

        // If we directly hit #[cfg(test)] before any mod, it's an inline cfg(test)
        if trimmed.contains("#[cfg(test)]") {
            cfg_test_line = Some(i);
            break;
        }
    }

    let Some(mod_start) = cfg_test_line else {
        return false;
    };

    // Track brace depth from the mod declaration line to determine scope
    let mut depth: i32 = 0;
    for i in mod_start..=target_line {
        let Some(line) = lines.get(i) else { break };
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                // If depth drops to 0, the module has closed
                if depth <= 0 && i < target_line {
                    return false;
                }
            }
        }
    }

    // target_line is inside the test module if depth > 0
    depth > 0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_canonical_modules_passes() {
        let arch: ArchitectureRules = serde_json::from_str(r#"{"version": 2}"#).unwrap();
        let rules = compile_canonical_rules(&arch.canonical_modules).unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_parse_canonical_rule() {
        let arch: ArchitectureRules = serde_json::from_str(
            r#"{
            "canonical_modules": [{
                "concern": "shell-parsing",
                "forbidden_patterns": ["fn\\s+normalize_separators"],
                "allowed_in": ["libs/domain/src/guard/"],
                "convention": "knowledge/conventions/shell-parsing.md",
                "rationale": "test"
            }]
        }"#,
        )
        .unwrap();
        let rules = compile_canonical_rules(&arch.canonical_modules).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].concern, "shell-parsing");
        assert_eq!(rules[0].forbidden_patterns.len(), 1);
    }

    #[test]
    fn test_forbidden_pattern_detected_outside_allowed_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create architecture rules
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(
            root.join("architecture-rules.json"),
            r#"{
                "version": 2,
                "canonical_modules": [{
                    "concern": "test-concern",
                    "forbidden_patterns": ["fn\\s+my_forbidden_fn"],
                    "allowed_in": ["libs/domain/"],
                    "convention": ""
                }]
            }"#,
        )
        .unwrap();

        // Create a file outside allowed_in with the forbidden pattern
        let bad_dir = root.join("libs/usecase/src");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(bad_dir.join("hook.rs"), "fn my_forbidden_fn() {}\n").unwrap();

        let outcome = verify(root);
        assert!(outcome.has_errors(), "should detect forbidden pattern outside allowed_in");
    }

    #[test]
    fn test_forbidden_pattern_allowed_inside_canonical_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(
            root.join("architecture-rules.json"),
            r#"{
                "version": 2,
                "canonical_modules": [{
                    "concern": "test-concern",
                    "forbidden_patterns": ["fn\\s+my_forbidden_fn"],
                    "allowed_in": ["libs/domain/"],
                    "convention": ""
                }]
            }"#,
        )
        .unwrap();

        // Create a file inside allowed_in with the forbidden pattern
        let ok_dir = root.join("libs/domain/src/guard");
        std::fs::create_dir_all(&ok_dir).unwrap();
        std::fs::write(ok_dir.join("parser.rs"), "fn my_forbidden_fn() {}\n").unwrap();

        let outcome = verify(root);
        assert!(!outcome.has_errors(), "should allow pattern inside canonical dir");
    }

    #[test]
    fn test_forbidden_pattern_inside_test_module_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(
            root.join("architecture-rules.json"),
            r#"{
                "version": 2,
                "canonical_modules": [{
                    "concern": "test-concern",
                    "forbidden_patterns": ["fn\\s+my_forbidden_fn"],
                    "allowed_in": ["libs/domain/"],
                    "convention": ""
                }]
            }"#,
        )
        .unwrap();

        let bad_dir = root.join("libs/usecase/src");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(
            bad_dir.join("hook.rs"),
            "pub fn real_code() {}\n\n#[cfg(test)]\nmod tests {\n    fn my_forbidden_fn() {}\n}\n",
        )
        .unwrap();

        let outcome = verify(root);
        assert!(
            !outcome.has_errors(),
            "should ignore forbidden pattern inside #[cfg(test)] module"
        );
    }

    #[test]
    fn test_forbidden_pattern_after_test_module_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(
            root.join("architecture-rules.json"),
            r#"{
                "version": 2,
                "canonical_modules": [{
                    "concern": "test-concern",
                    "forbidden_patterns": ["fn\\s+my_forbidden_fn"],
                    "allowed_in": ["libs/domain/"],
                    "convention": ""
                }]
            }"#,
        )
        .unwrap();

        // Forbidden pattern appears AFTER the test module closes — must be detected
        let bad_dir = root.join("libs/usecase/src");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(
            bad_dir.join("hook.rs"),
            "#[cfg(test)]\nmod tests {\n    fn test_helper() {}\n}\n\nfn my_forbidden_fn() {}\n",
        )
        .unwrap();

        let outcome = verify(root);
        assert!(
            outcome.has_errors(),
            "forbidden pattern after #[cfg(test)] module must be detected"
        );
    }

    #[test]
    fn test_allowed_in_rejects_non_string_entries() {
        let result = serde_json::from_str::<ArchitectureRules>(
            r#"{
            "canonical_modules": [{
                "concern": "test-concern",
                "forbidden_patterns": ["fn\\s+my_forbidden_fn"],
                "allowed_in": ["libs/domain/", 42],
                "convention": ""
            }]
        }"#,
        );
        assert!(result.is_err(), "non-string entry in allowed_in must cause serde error");
    }
}
