//! Verify that `libs/domain/src/` contains no forbidden patterns that violate
//! hexagonal architecture purity.
//!
//! Delegates to the shared `super::usecase_purity::check_layer_purity` engine.

use std::path::Path;

use domain::verify::VerifyOutcome;

const DOMAIN_SRC_DIR: &str = "libs/domain/src";

/// Scan `libs/domain/src/` for forbidden patterns that violate hexagonal purity.
///
/// # Errors
///
/// Returns findings for each forbidden pattern found.
pub fn verify(root: &Path) -> VerifyOutcome {
    super::usecase_purity::check_layer_purity(root, DOMAIN_SRC_DIR, "Domain")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use std::path::Path;

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
    fn test_clean_domain_passes() {
        let tmp = TempDir::new().unwrap();
        setup_domain_file(
            tmp.path(),
            "lib.rs",
            "pub struct Foo;\nimpl Foo { pub fn bar(&self) {} }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_detects_std_fs_in_domain() {
        let tmp = TempDir::new().unwrap();
        setup_domain_file(tmp.path(), "lib.rs", "fn bad() { std::fs::read(\"x\"); }\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings()[0].to_string().contains("std::fs::"));
    }

    #[test]
    fn test_detects_println_in_domain() {
        let tmp = TempDir::new().unwrap();
        setup_domain_file(tmp.path(), "lib.rs", "fn bad() { println!(\"hi\"); }\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
    }

    #[test]
    fn test_ignores_test_module_in_domain() {
        let tmp = TempDir::new().unwrap();
        setup_domain_file(
            tmp.path(),
            "lib.rs",
            "pub fn clean() {}\n\n#[cfg(test)]\nmod tests {\n    fn t() { println!(\"ok\"); }\n}\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_missing_domain_dir_errors() {
        let tmp = TempDir::new().unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }
}
