//! Verify that `libs/domain/src/` has no `pub` struct fields of type `String`.
//!
//! Newtypes (`pub struct Foo(String)`) are excluded because the inner field
//! is not `pub`. Only named struct fields `pub field: String` are flagged.

use std::path::Path;

use domain::verify::{Finding, VerifyOutcome};

const DOMAIN_SRC_DIR: &str = "libs/domain/src";

/// Scan `libs/domain/src/` for `pub` struct fields typed `String`.
///
/// # Errors
///
/// Returns findings for each `pub field: String` found.
pub fn verify(root: &Path) -> VerifyOutcome {
    let domain_src = root.join(DOMAIN_SRC_DIR);
    if !domain_src.is_dir() {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "Domain source directory not found: {DOMAIN_SRC_DIR}"
        ))]);
    }

    let mut findings = Vec::new();
    scan_dir(&domain_src, root, &mut findings);
    VerifyOutcome::from_findings(findings)
}

fn scan_dir(dir: &Path, root: &Path, findings: &mut Vec<Finding>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            let rel = dir.strip_prefix(root).unwrap_or(dir);
            findings.push(Finding::error(format!(
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
                findings.push(Finding::error(format!(
                    "{}: cannot read entry: {e}",
                    rel.to_string_lossy()
                )));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, root, findings);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            match std::fs::read_to_string(&path) {
                Ok(content) => check_file(&path, root, &content, findings),
                Err(e) => {
                    let rel = path.strip_prefix(root).unwrap_or(&path);
                    findings.push(Finding::error(format!(
                        "{}: cannot read file: {e}",
                        rel.to_string_lossy()
                    )));
                }
            }
        }
    }
}

fn check_file(path: &Path, root: &Path, content: &str, findings: &mut Vec<Finding>) {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let rel_str = rel.to_string_lossy();

    check_content(&rel_str, content, findings);
}

fn check_content(rel_path: &str, content: &str, findings: &mut Vec<Finding>) {
    // Stop scanning at #[cfg(test)] — test modules are conventionally at file end.
    // This avoids complex brace-depth tracking edge cases.
    let production_content = content.split("\n#[cfg(test)]").next().unwrap_or(content);

    for (line_num, line) in production_content.lines().enumerate() {
        let trimmed = line.trim();

        // Match patterns like: `pub field: String` or `pub field: Option<String>`
        // But NOT inside tuple structs (those are newtypes)
        if is_pub_string_field(trimmed) {
            // Warning (not error) until DM-01/02/03 type migration completes.
            findings.push(Finding::warning(format!(
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

    fn setup_domain_file(root: &Path, rel: &str, content: &str) {
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
        let tmp = TempDir::new().unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
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
