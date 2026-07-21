//! Verify that domain-purity opt-in layers contain no forbidden patterns that
//! violate hexagonal architecture purity.
//!
//! The scanned source paths are resolved from `architecture-rules.json` layer
//! entries that declare `verify.domain_purity: true` — no path is hardcoded.
//! Dispatch delegates to the shared `super::usecase_purity` engine.

use std::path::Path;

use domain::verify::VerifyOutcome;

use crate::arch::VerifierKind;

/// Scan every domain-purity opt-in layer for forbidden patterns that violate
/// hexagonal purity.
///
/// # Errors
///
/// Returns findings for each forbidden pattern found.
pub fn verify(root: &Path) -> VerifyOutcome {
    super::usecase_purity::check_arch_layers_purity(root, VerifierKind::DomainPurity)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use std::path::Path;

    use tempfile::TempDir;

    use super::*;

    const DOMAIN_SRC_DIR: &str = "libs/domain/src";

    /// Write a minimal `architecture-rules.json` declaring a domain layer that
    /// opts into the domain-purity verifier.
    fn write_arch_rules(root: &Path) {
        let json = r#"{
  "version": 2,
  "layers": [
    { "crate": "domain", "path": "libs/domain", "verify": { "domain_purity": true } }
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
        // Layer declares the flag but its `<path>/src` is absent — misconfiguration.
        let tmp = TempDir::new().unwrap();
        write_arch_rules(tmp.path());
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_no_opt_in_layer_skips() {
        // arch-rules present but no layer declares domain_purity → OK/skip.
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("architecture-rules.json"),
            r#"{ "version": 2, "layers": [ { "crate": "domain", "path": "libs/domain" } ] }"#,
        )
        .unwrap();
        let path = tmp.path().join(DOMAIN_SRC_DIR).join("lib.rs");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "fn bad() { std::fs::read(\"x\"); }\n").unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }
}
