//! `verify` command family — per-context composition root and CliApp shim.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `verify` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct VerifyCompositionRoot;

impl VerifyCompositionRoot {
    /// Create a new `VerifyCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for VerifyCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolves the work machine's home directory for machine-path verification.
///
/// The composition root owns ambient environment access. It resolves
/// `SOTP_MACHINE_HOME`, then `HOME`, then `USERPROFILE`; empty values are
/// skipped. Containerized runs forward the host home through
/// `SOTP_MACHINE_HOME`, avoiding the container-local fallback.
fn machine_home_directory() -> Option<PathBuf> {
    ["SOTP_MACHINE_HOME", "HOME", "USERPROFILE"].into_iter().find_map(|variable| {
        std::env::var_os(variable).filter(|value| !value.is_empty()).map(PathBuf::from)
    })
}

impl VerifyCompositionRoot {
    /// Build a wired [`cli_driver::verify::VerifyDriver`] for the verify family.
    ///
    /// Wire chain: `FsVerifyAdapter` → `VerifyInteractor` → `VerifyDriver`.
    pub fn verify_driver(&self) -> cli_driver::verify::VerifyDriver {
        use infrastructure::FsVerifyAdapter;
        use std::sync::Arc;
        use usecase::verify::{VerifyInteractor, VerifyPort};

        let adapter = Arc::new(FsVerifyAdapter::new(machine_home_directory()));
        let interactor = Arc::new(VerifyInteractor::new(adapter as Arc<dyn VerifyPort>));
        cli_driver::verify::VerifyDriver::new(interactor)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};

    use cli_driver::verify::VerifyInput;

    use super::*;

    fn write_file(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_verify_composition_root_retention_gate_driver_returns_success() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(tmp.path(), "README.md", "# Clean\n");
        let driver = VerifyCompositionRoot::new().verify_driver();

        let outcome =
            driver.handle(VerifyInput::RetentionGate { project_root: tmp.path().to_path_buf() });

        assert_eq!(outcome.exit_code, 0);
    }

    #[test]
    fn test_verify_composition_root_new_verifier_routes_fail_closed_with_controlled_home() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        let machine_home = tempfile::tempdir().unwrap();
        let _home = crate::review_v2::process_guards::EnvGuard::set(
            "HOME",
            machine_home.path().as_os_str().to_os_string(),
        );
        let _userprofile = crate::review_v2::process_guards::EnvGuard::remove("USERPROFILE");
        let driver = VerifyCompositionRoot::new().verify_driver();
        let routes = [
            (
                VerifyInput::SotpVersionTag { project_root: root.path().to_path_buf() },
                "--- verify sotp version tag ---",
            ),
            (
                VerifyInput::MachinePaths { project_root: root.path().to_path_buf() },
                "--- verify machine paths ---",
            ),
            (
                VerifyInput::TemplateRefs { project_root: root.path().to_path_buf() },
                "--- verify template refs ---",
            ),
        ];

        for (input, expected_label) in routes {
            let outcome = driver.handle(input);

            assert_eq!(outcome.exit_code, 1, "missing fixture inputs must fail closed");
            let stdout = outcome.stdout.unwrap_or_default();
            assert!(stdout.contains(expected_label), "{stdout}");
            if expected_label == "--- verify machine paths ---" {
                assert!(
                    !stdout.contains("machine home directory must be explicitly supplied"),
                    "composition must inject the resolved machine home: {stdout}"
                );
            }
        }
    }

    #[test]
    fn test_machine_home_directory_home_takes_precedence() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let _machine_home = crate::review_v2::process_guards::EnvGuard::remove("SOTP_MACHINE_HOME");
        let _home = crate::review_v2::process_guards::EnvGuard::set("HOME", "/work-machine/home");
        let _userprofile = crate::review_v2::process_guards::EnvGuard::set(
            "USERPROFILE",
            "/work-machine/userprofile",
        );

        assert_eq!(machine_home_directory(), Some(PathBuf::from("/work-machine/home")));
    }

    #[test]
    fn test_machine_home_directory_override_takes_precedence() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let _machine_home = crate::review_v2::process_guards::EnvGuard::set(
            "SOTP_MACHINE_HOME",
            "/work-machine/override",
        );
        let _home = crate::review_v2::process_guards::EnvGuard::set("HOME", "/work-machine/home");
        let _userprofile = crate::review_v2::process_guards::EnvGuard::set(
            "USERPROFILE",
            "/work-machine/userprofile",
        );

        assert_eq!(machine_home_directory(), Some(PathBuf::from("/work-machine/override")));
    }
}
