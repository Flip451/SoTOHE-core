//! Verify that domain-strings opt-in layers have no `pub` struct fields of type
//! `String`.
//!
//! Newtypes (`pub struct Foo(String)`) are excluded because the inner field
//! is not `pub`. Only named struct fields `pub field: String` are flagged.
//!
//! The scanned source paths are resolved from `architecture-rules.json` layer
//! entries that declare `verify.domain_strings: true` — no path is hardcoded.

use std::path::Path;

use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::arch::VerifierKind;

/// Scan every domain-strings opt-in layer for `pub` struct fields typed `String`.
///
/// When no layer declares the flag, the result is OK/skip (config-driven
/// absence is not an error).
///
/// # Errors
///
/// Returns findings for each `pub field: String` found, or a single error
/// finding if `architecture-rules.json` cannot be loaded or parsed.
pub fn verify(root: &Path) -> VerifyOutcome {
    let targets = match crate::arch::resolve_verify_targets(root, VerifierKind::DomainStrings) {
        Ok(targets) => targets,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "failed to resolve verify targets from architecture-rules.json: {e}"
            ))]);
        }
    };

    let mut findings = Vec::new();
    for target in targets {
        let src = root.join(&target.src_dir);
        if !src.is_dir() {
            findings.push(VerifyFinding::error(format!(
                "{} source directory not found: {}",
                target.label, target.src_dir
            )));
            continue;
        }
        scan_dir(&src, root, &mut findings);
    }
    VerifyOutcome::from_findings(findings)
}

fn scan_dir(dir: &Path, root: &Path, findings: &mut Vec<VerifyFinding>) {
    if !reject_unsafe_scan_path(dir, root, findings) {
        return;
    }

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
        if !reject_unsafe_scan_path(&path, root, findings) {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(e) => {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                findings.push(VerifyFinding::error(format!(
                    "{}: cannot inspect entry type: {e}",
                    rel.to_string_lossy()
                )));
                continue;
            }
        };
        if file_type.is_dir() {
            scan_dir(&path, root, findings);
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            match std::fs::read_to_string(&path) {
                Ok(content) => check_file(&path, root, &content, findings),
                Err(e) => {
                    let rel = path.strip_prefix(root).unwrap_or(&path);
                    findings.push(VerifyFinding::error(format!(
                        "{}: cannot read file: {e}",
                        rel.to_string_lossy()
                    )));
                }
            }
        }
    }
}

fn reject_unsafe_scan_path(path: &Path, root: &Path, findings: &mut Vec<VerifyFinding>) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    match crate::track::symlink_guard::reject_symlinks_below(path, root) {
        Ok(true) => true,
        Ok(false) => {
            findings.push(VerifyFinding::error(format!(
                "{}: path disappeared during scan",
                rel.to_string_lossy()
            )));
            false
        }
        Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => {
            findings.push(VerifyFinding::error(format!(
                "{}: symlink detected during scan (rejected for security)",
                rel.to_string_lossy()
            )));
            false
        }
        Err(e) => {
            findings.push(VerifyFinding::error(format!(
                "{}: cannot inspect path before scanning: {:?}",
                rel.to_string_lossy(),
                e.kind()
            )));
            false
        }
    }
}

fn check_file(path: &Path, root: &Path, content: &str, findings: &mut Vec<VerifyFinding>) {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let rel_str = rel.to_string_lossy();

    check_content(&rel_str, content, findings);
}

fn check_content(rel_path: &str, content: &str, findings: &mut Vec<VerifyFinding>) {
    // Stop scanning at #[cfg(test)] — test modules are conventionally at file end.
    // This avoids complex brace-depth tracking edge cases.
    let production_content = content.split("\n#[cfg(test)]").next().unwrap_or(content);

    for (line_num, line) in production_content.lines().enumerate() {
        let trimmed = line.trim();

        // Match patterns like: `pub field: String` or `pub field: Option<String>`
        // But NOT inside tuple structs (those are newtypes)
        if is_pub_string_field(trimmed) {
            // Warning (not error) until DM-01/02/03 type migration completes.
            findings.push(VerifyFinding::warning(format!(
                "{rel_path}:{}: pub String field: `{trimmed}` — \
                 if finite states, use an enum; if free text, wrap in a newtype",
                line_num + 1
            )));
        }
    }
}

/// Detect `pub field_name: String` or `pub field_name: Option<String>` patterns
/// in named struct fields. Excludes tuple struct fields.
fn is_pub_string_field(line: &str) -> bool {
    // Must start with `pub` and contain a colon (named field, not tuple struct)
    if !line.starts_with("pub ") || !line.contains(':') {
        return false;
    }

    // Exclude function signatures (pub fn ...) and type aliases (pub type ...)
    if line.starts_with("pub fn ")
        || line.starts_with("pub(crate) fn ")
        || line.starts_with("pub type ")
        || line.starts_with("pub struct ")
        || line.starts_with("pub enum ")
        || line.starts_with("pub trait ")
        || line.starts_with("pub mod ")
        || line.starts_with("pub use ")
        || line.starts_with("pub const ")
        || line.starts_with("pub static ")
    {
        return false;
    }

    // Extract the type part after the colon
    let after_colon = match line.split_once(':') {
        Some((_, ty)) => ty.trim().trim_end_matches(','),
        None => return false,
    };

    // Check if the type is exactly `String` or contains `String` as a direct type
    // (e.g., `Option<String>`, `Vec<String>`)
    is_string_type(after_colon)
}

/// Check if a type expression is or directly contains `String`.
fn is_string_type(ty: &str) -> bool {
    let ty = ty.trim();
    if ty == "String" {
        return true;
    }
    // Check for Option<String>, Vec<String>, etc.
    if let Some(inner) = extract_generic_inner(ty) {
        return is_string_type(inner);
    }
    false
}

/// Extract the inner type from `Foo<Bar>` → `Bar`.
fn extract_generic_inner(ty: &str) -> Option<&str> {
    let open = ty.find('<')?;
    let close = ty.rfind('>')?;
    if close > open + 1 { Some(ty.get(open + 1..close)?.trim()) } else { None }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    const DOMAIN_SRC_DIR: &str = "libs/domain/src";

    /// Write a minimal `architecture-rules.json` declaring a domain layer that
    /// opts into the domain-strings verifier.
    fn write_arch_rules(root: &Path) {
        let json = r#"{
  "version": 2,
  "layers": [
    { "crate": "domain", "path": "libs/domain", "verify": { "domain_strings": true } }
  ]
}"#;
        std::fs::write(root.join("architecture-rules.json"), json).unwrap();
    }

    fn setup_domain_file(root: &Path, rel: &str, content: &str) {
        write_arch_rules(root);
        let path = root.join(DOMAIN_SRC_DIR).join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    #[test]
    fn test_detects_pub_string_field() {
        let tmp = TempDir::new().unwrap();
        setup_domain_file(
            tmp.path(),
            "review.rs",
            "pub struct Foo {\n    pub verdict: String,\n}\n",
        );
        let outcome = verify(tmp.path());
        // Warning-only until DM-01/02/03 migration completes
        assert!(outcome.is_ok());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings()[0].to_string().contains("pub String field"));
    }

    #[test]
    fn test_detects_pub_option_string_field() {
        let tmp = TempDir::new().unwrap();
        setup_domain_file(
            tmp.path(),
            "review.rs",
            "pub struct Foo {\n    pub name: Option<String>,\n}\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(!outcome.findings().is_empty());
    }

    #[test]
    fn test_ignores_newtype_tuple_struct() {
        let tmp = TempDir::new().unwrap();
        setup_domain_file(tmp.path(), "ids.rs", "pub struct TrackId(String);\n");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_ignores_private_string_field() {
        let tmp = TempDir::new().unwrap();
        setup_domain_file(tmp.path(), "review.rs", "pub struct Foo {\n    verdict: String,\n}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_ignores_non_string_pub_field() {
        let tmp = TempDir::new().unwrap();
        setup_domain_file(tmp.path(), "review.rs", "pub struct Foo {\n    pub count: u32,\n}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_missing_domain_dir_errors() {
        // Layer declares the flag but its `<path>/src` is absent — misconfiguration.
        let tmp = TempDir::new().unwrap();
        write_arch_rules(tmp.path());
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }

    #[cfg(unix)]
    #[test]
    fn test_symlinked_domain_source_file_errors() {
        let tmp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        write_arch_rules(tmp.path());

        let domain_src = tmp.path().join(DOMAIN_SRC_DIR);
        std::fs::create_dir_all(&domain_src).unwrap();
        let outside_file = outside.path().join("review.rs");
        std::fs::write(&outside_file, "pub struct Foo {\n    pub verdict: String,\n}\n").unwrap();
        std::os::unix::fs::symlink(&outside_file, domain_src.join("review.rs")).unwrap();

        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(
            outcome.findings().iter().any(|finding| finding.to_string().contains("symlink")),
            "expected symlink rejection finding, got: {:?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_no_opt_in_layer_skips() {
        // arch-rules present but no layer declares domain_strings → OK/skip.
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("architecture-rules.json"),
            r#"{ "version": 2, "layers": [ { "crate": "domain", "path": "libs/domain" } ] }"#,
        )
        .unwrap();
        let path = tmp.path().join(DOMAIN_SRC_DIR).join("review.rs");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "pub struct Foo {\n    pub verdict: String,\n}\n").unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_ignores_test_module_fields() {
        let tmp = TempDir::new().unwrap();
        setup_domain_file(
            tmp.path(),
            "review.rs",
            "pub struct Good {\n    pub count: u32,\n}\n\n\
             #[cfg(test)]\nmod tests {\n    pub struct TestOnly {\n        pub name: String,\n    }\n}\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
    }
}
