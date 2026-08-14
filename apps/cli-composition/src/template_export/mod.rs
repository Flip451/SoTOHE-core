//! `template` command family — composition root.
//!
//! [`TemplateCompositionRoot`] wires the filesystem boundary-manifest adapter
//! (`FsTemplateBoundaryManifestAdapter`), the filesystem export adapter
//! (`FsTemplateExportAdapter`), the self-binary transplant adapter
//! (`FsSelfBinaryTransplantAdapter`), the use-case interactor
//! (`TemplateExportInteractor`), and [`cli_driver::template_export::TemplateDriver`]
//! for the `sotp template export` subcommand.
//!
//! `handle` accepts the driver input DTO already assembled by the `cli` layer.
//! See IN-01, AC-01, and ADR 2026-07-08-0541 D1.

use std::path::PathBuf;
use std::sync::Arc;

use cli_driver::template_conventions::ConventionShippingCheckDriver;
use cli_driver::template_export::TemplateDriver;

/// Resolves the work machine's home directory for export-output scanning.
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

/// Composition root for the `template` command family.
///
/// Wires `FsTemplateBoundaryManifestAdapter` + `FsTemplateExportAdapter` →
/// `TemplateExportInteractor` → `TemplateDriver`.
#[derive(Debug, Default)]
pub struct TemplateCompositionRoot;

impl TemplateCompositionRoot {
    /// Create a new `TemplateCompositionRoot`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Build a fully-wired [`TemplateDriver`].
    ///
    /// Wire chain: `FsTemplateBoundaryManifestAdapter` +
    /// `FsTemplateExportAdapter` + `FsSelfBinaryTransplantAdapter` →
    /// `TemplateExportInteractor` → `TemplateDriver`.
    #[must_use]
    pub fn template_driver(&self) -> TemplateDriver {
        use infrastructure::template_export::{
            FsSelfBinaryTransplantAdapter, FsTemplateBoundaryManifestAdapter,
            FsTemplateExportAdapter,
        };
        use usecase::template_export::{
            SelfBinaryTransplantPort, TemplateBoundaryManifestPort, TemplateExportInteractor,
            TemplateExportPort, TemplateExportService,
        };

        let manifest_port: Arc<dyn TemplateBoundaryManifestPort> =
            Arc::new(FsTemplateBoundaryManifestAdapter::new());
        let export_port: Arc<dyn TemplateExportPort> =
            Arc::new(FsTemplateExportAdapter::new(machine_home_directory()));
        let transplant_port: Arc<dyn SelfBinaryTransplantPort> =
            Arc::new(FsSelfBinaryTransplantAdapter::new());
        let service: Arc<dyn TemplateExportService> =
            Arc::new(TemplateExportInteractor::new(manifest_port, export_port, transplant_port));
        TemplateDriver::new(service)
    }

    /// Build a fully-wired [`ConventionShippingCheckDriver`] (spec IN-11,
    /// AC-18).
    ///
    /// Wire chain: `FsConventionInventoryAdapter` →
    /// `ConventionShippingCheckInteractor` → `ConventionShippingCheckDriver`.
    ///
    /// The adapter is constructed once and injected once. The interactor takes a
    /// single `ConventionInventoryPort` so that both sides of the comparison are
    /// walked by identical rules, and the adapter is a stateless unit struct so
    /// that holds by construction; wiring a second instance, or handing the
    /// interactor anything per-side, would satisfy the same signature and lose
    /// it. A difference this check reports could then be an artefact of the walk
    /// rather than of what ships.
    #[must_use]
    pub fn convention_shipping_check_driver(&self) -> ConventionShippingCheckDriver {
        use infrastructure::template_conventions::FsConventionInventoryAdapter;
        use usecase::template_conventions::{
            ConventionInventoryPort, ConventionShippingCheckInteractor,
            ConventionShippingCheckService,
        };

        let inventory_port: Arc<dyn ConventionInventoryPort> =
            Arc::new(FsConventionInventoryAdapter::new());
        let service: Arc<dyn ConventionShippingCheckService> =
            Arc::new(ConventionShippingCheckInteractor::new(inventory_port));
        ConventionShippingCheckDriver::new(service)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use cli_driver::template_conventions::ConventionShippingCheckInput;
    use cli_driver::template_export::{TemplateDriver, TemplateExportInput, TemplateInput};
    use domain::FreeText;
    use infrastructure::template_export::{
        FsTemplateBoundaryManifestAdapter, FsTemplateExportAdapter,
    };
    use tempfile::TempDir;
    use usecase::template_export::{
        SelfBinaryTransplantError, SelfBinaryTransplantPort, TemplateBoundaryManifestPort,
        TemplateExportInteractor, TemplateExportPort, TemplateExportService,
    };

    use super::{TemplateCompositionRoot, machine_home_directory};

    /// Test-only transplant adapter. The production adapter deliberately keeps
    /// its copy semantics; these in-process tests use a hard link so the test
    /// binary is never materialized again under the scaffold.
    #[derive(Debug)]
    struct HardLinkSelfBinaryTransplantAdapter {
        source: PathBuf,
    }

    impl HardLinkSelfBinaryTransplantAdapter {
        fn new(source: PathBuf) -> Self {
            Self { source }
        }
    }

    fn hard_link_or_copy(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
        match std::fs::hard_link(source, destination) {
            Ok(()) => Ok(()),
            // Cargo normally keeps the test binary and target temp directory on
            // one filesystem. Keep the test runnable for an externally mounted
            // target directory by falling back only for EXDEV.
            Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {
                std::fs::copy(source, destination).map(|_| ())
            }
            Err(error) => Err(error),
        }
    }

    impl SelfBinaryTransplantPort for HardLinkSelfBinaryTransplantAdapter {
        fn transplant(&self, destination: &Path) -> Result<(), SelfBinaryTransplantError> {
            if let Some(parent) = destination.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent).map_err(|error| {
                    SelfBinaryTransplantError::DestinationWriteFailure {
                        path: destination.to_path_buf(),
                        reason: FreeText::new(error.to_string()),
                    }
                })?;
            }

            hard_link_or_copy(&self.source, destination).map_err(|error| {
                SelfBinaryTransplantError::DestinationWriteFailure {
                    path: destination.to_path_buf(),
                    reason: FreeText::new(error.to_string()),
                }
            })
        }
    }

    fn test_template_export_driver() -> TemplateDriver {
        let manifest_port: Arc<dyn TemplateBoundaryManifestPort> =
            Arc::new(FsTemplateBoundaryManifestAdapter::new());
        let export_port: Arc<dyn TemplateExportPort> =
            Arc::new(FsTemplateExportAdapter::new(machine_home_directory()));
        let transplant_port: Arc<dyn SelfBinaryTransplantPort> =
            Arc::new(HardLinkSelfBinaryTransplantAdapter::new(std::env::current_exe().unwrap()));
        let service: Arc<dyn TemplateExportService> =
            Arc::new(TemplateExportInteractor::new(manifest_port, export_port, transplant_port));
        TemplateDriver::new(service)
    }

    fn template_export_temp_parent(
        cargo_target_tmpdir: Option<PathBuf>,
        workspace_root: &Path,
    ) -> PathBuf {
        cargo_target_tmpdir
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| workspace_root.join("target/tmp"))
    }

    fn template_export_tempdir() -> TempDir {
        let workspace_root = crate::test_support::repo_root_for_tests();
        let parent = template_export_temp_parent(
            std::env::var_os("CARGO_TARGET_TMPDIR").map(PathBuf::from),
            &workspace_root,
        );
        std::fs::create_dir_all(&parent).unwrap_or_else(|error| {
            panic!("cannot create template-export temporary parent {}: {error}", parent.display())
        });
        TempDir::new_in(&parent).unwrap_or_else(|error| {
            panic!(
                "cannot create template-export temporary directory in {}: {error}",
                parent.display()
            )
        })
    }

    #[test]
    fn test_template_export_temp_parent_prefers_cargo_target_tmpdir() {
        let configured = PathBuf::from("/cargo/target/tmp");

        assert_eq!(
            template_export_temp_parent(Some(configured.clone()), Path::new("/workspace")),
            configured
        );
    }

    #[test]
    fn test_template_export_temp_parent_falls_back_to_workspace_target_tmp() {
        assert_eq!(
            template_export_temp_parent(None, Path::new("/workspace")),
            PathBuf::from("/workspace/target/tmp")
        );
        assert_eq!(
            template_export_temp_parent(Some(PathBuf::new()), Path::new("/workspace")),
            PathBuf::from("/workspace/target/tmp")
        );
    }

    #[test]
    fn test_machine_home_directory_home_set_returns_home() {
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
    fn test_machine_home_directory_home_empty_returns_userprofile() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let _machine_home = crate::review_v2::process_guards::EnvGuard::remove("SOTP_MACHINE_HOME");
        let _home = crate::review_v2::process_guards::EnvGuard::set("HOME", "");
        let _userprofile = crate::review_v2::process_guards::EnvGuard::set(
            "USERPROFILE",
            "/work-machine/userprofile",
        );

        assert_eq!(machine_home_directory(), Some(PathBuf::from("/work-machine/userprofile")));
    }

    #[test]
    fn test_machine_home_directory_variables_unset_returns_none() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let _machine_home = crate::review_v2::process_guards::EnvGuard::remove("SOTP_MACHINE_HOME");
        let _home = crate::review_v2::process_guards::EnvGuard::remove("HOME");
        let _userprofile = crate::review_v2::process_guards::EnvGuard::remove("USERPROFILE");

        assert_eq!(machine_home_directory(), None);
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

    /// Composition-root smoke test: the wired stack routes an `Export` input all
    /// the way through `FsTemplateBoundaryManifestAdapter` → interactor →
    /// `TemplateDriver`. A missing manifest surfaces as a mapped failure
    /// (`exit_code == 1` with a stderr message), exercising the full wiring
    /// without a filesystem fixture.
    #[test]
    fn export_with_missing_manifest_wires_and_maps_to_exit_1() {
        let root = TemplateCompositionRoot::new();
        let input = TemplateInput::Export(TemplateExportInput {
            workspace_root: PathBuf::from("/nonexistent/ws"),
            manifest_path: PathBuf::from("/nonexistent/ws/boundary.json"),
            overlay_dir: PathBuf::from("/nonexistent/ws/overlay"),
            output_dir: PathBuf::from("/nonexistent/out"),
        });

        let outcome = root.template_driver().handle(input);

        assert_eq!(outcome.exit_code, 1, "missing manifest must map to exit 1: {outcome:?}");
        assert_eq!(outcome.stdout, None, "failure path must not emit stdout");
        assert!(
            outcome.stderr.is_some(),
            "the underlying manifest-read error must be surfaced on stderr"
        );
    }

    #[test]
    fn template_composition_root_is_wiring_only() {
        let source = include_str!("mod.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap();
        assert!(production_source.contains("-> TemplateDriver"));
        assert!(production_source.contains("TemplateDriver::new("));
        assert!(production_source.contains("Arc<dyn TemplateExportService>"));
        for wired_component in [
            "Arc::new(FsTemplateBoundaryManifestAdapter::new())",
            "Arc::new(FsTemplateExportAdapter::new(machine_home_directory()))",
            "Arc::new(FsSelfBinaryTransplantAdapter::new())",
            "TemplateExportInteractor::new(manifest_port, export_port, transplant_port)",
            "TemplateDriver::new(service)",
        ] {
            assert!(
                production_source.contains(wired_component),
                "composition root must wire {wired_component} into the one-way driver path"
            );
        }
        for forbidden in [
            "CommandOutcome",
            ".handle(",
            "std::fs::",
            "std::process::",
            "std::net::",
            "std::io::",
            "println!",
            "eprintln!",
            "print!",
            "eprint!",
            "ServiceImpl",
        ] {
            assert!(
                !production_source.contains(forbidden),
                "composition root must not contain execution or compatibility path {forbidden}"
            );
        }
    }

    /// The shipping check's wire chain is `FsConventionInventoryAdapter` →
    /// `ConventionShippingCheckInteractor` → `ConventionShippingCheckDriver`,
    /// and the adapter is constructed exactly once (spec IN-11, AC-18).
    #[test]
    fn convention_shipping_check_wiring_injects_one_adapter_instance() {
        let source = include_str!("mod.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap();
        for wired_component in [
            "-> ConventionShippingCheckDriver",
            "Arc<dyn ConventionInventoryPort>",
            "Arc::new(FsConventionInventoryAdapter::new())",
            "ConventionShippingCheckInteractor::new(inventory_port)",
            "ConventionShippingCheckDriver::new(service)",
        ] {
            assert!(
                production_source.contains(wired_component),
                "composition root must wire {wired_component} into the check's one-way driver path"
            );
        }
        // One construction site, and the interactor is handed that one binding.
        // The interactor takes a single port so both trees are walked by
        // identical rules; a second adapter here would satisfy the same
        // signature and let a difference in the walk read as a difference in
        // what ships.
        assert_eq!(
            production_source.matches("FsConventionInventoryAdapter::new()").count(),
            1,
            "exactly one inventory adapter is constructed for the check"
        );
        assert_eq!(
            production_source.matches("ConventionShippingCheckInteractor::new(").count(),
            1,
            "the interactor is built once, from that one adapter binding"
        );
    }

    fn write_convention(root: &Path, tree: &str, name: &str) {
        write_file(root, &format!("{tree}/knowledge/conventions/{name}"), "# convention\n");
    }

    /// End-to-end wiring check over real trees (spec IN-11, AC-18): the wired
    /// stack inventories both trees through the injected filesystem adapter and
    /// answers the shipping question about the tree the caller named.
    ///
    /// Two trees rather than one, and real directories rather than a stand-in,
    /// because what this asserts is that the composition root produced a driver
    /// that actually reaches the filesystem walk on both sides.
    #[test]
    fn convention_shipping_check_passes_a_tree_shipping_only_the_overlays_supply() {
        let dir = TempDir::new().unwrap();
        let root_dir = dir.path();
        for name in ["README.md", "coding-principles.md", "testing.md"] {
            write_convention(root_dir, "export", name);
            write_convention(root_dir, "overlay", name);
        }

        let outcome = TemplateCompositionRoot::new().convention_shipping_check_driver().handle(
            ConventionShippingCheckInput {
                exported_root: root_dir.join("export"),
                overlay_dir: root_dir.join("overlay"),
            },
        );

        assert_eq!(outcome.exit_code, 0, "an export shipping the overlay's supply passes");
        assert_eq!(outcome.stderr, None, "a passing check reports no problem: {outcome:?}");
    }

    /// The same wired stack fails, naming every offending document, when the
    /// exported tree ships a source convention the overlay does not supply
    /// (spec IN-11, AC-18) — the shape a tree takes when `knowledge/conventions`
    /// is classified anything other than `overlay`.
    #[test]
    fn convention_shipping_check_names_every_source_convention_the_overlay_does_not_supply() {
        let dir = TempDir::new().unwrap();
        let root_dir = dir.path();
        for name in ["README.md", "coding-principles.md"] {
            write_convention(root_dir, "export", name);
            write_convention(root_dir, "overlay", name);
        }
        let unsupplied = ["dry-check-workflow.md", "language-policy.md", "shell-parsing.md"];
        for name in unsupplied {
            write_convention(root_dir, "export", name);
        }

        let outcome = TemplateCompositionRoot::new().convention_shipping_check_driver().handle(
            ConventionShippingCheckInput {
                exported_root: root_dir.join("export"),
                overlay_dir: root_dir.join("overlay"),
            },
        );

        assert_eq!(outcome.exit_code, 1, "an export shipping an unsupplied convention fails");
        let reported = outcome.stderr.expect("the violation is reported");
        for name in unsupplied {
            assert!(
                reported.contains(&format!("knowledge/conventions/{name}")),
                "every offending document is named rather than counted; {name} is missing from: \
                 {reported}"
            );
        }
        for supplied in ["README.md", "coding-principles.md"] {
            assert!(
                !reported.contains(&format!("knowledge/conventions/{supplied}")),
                "a document the overlay supplies is not an offender: {reported}"
            );
        }
    }

    fn write_file(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    /// End-to-end test-side export regression (ADR D5, spec AC-06): the
    /// hard-link transplant must produce `<output_dir>/bin/sotp` that is
    /// byte-identical to the running test binary without copying its contents.
    /// The production composition-root wiring remains covered by the wiring
    /// test above and continues to select the copy-based adapter.
    #[test]
    fn export_transplants_running_binary_into_bin_sotp() {
        let dir = template_export_tempdir();
        let parent_dir = dir.path();
        let source_root = parent_dir.join("source");
        // Minimal workspace tree: an included subtree, an overlay-classified
        // file with an overlay counterpart, and an excluded directory.
        write_file(&source_root, "libs/domain/src/lib.rs", "// domain\n");
        write_file(&source_root, "Makefile.toml", "# real\n");
        write_file(&source_root, "vendor/blob.bin", "excluded\n");
        write_file(parent_dir, "overlay/Makefile.toml", "# template\n");
        write_file(
            &source_root,
            "boundary.json",
            r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": "boundary.json", "classification": "include" },
    { "pattern": "libs/domain", "classification": "include" },
    { "pattern": "Makefile.toml", "classification": "overlay" },
    { "pattern": "vendor", "classification": "exclude" }
  ]
}"#,
        );

        let output_dir = parent_dir.join("scaffold");
        let input = TemplateInput::Export(TemplateExportInput {
            workspace_root: source_root.clone(),
            manifest_path: source_root.join("boundary.json"),
            overlay_dir: parent_dir.join("overlay"),
            output_dir: output_dir.clone(),
        });

        let outcome = test_template_export_driver().handle(input);
        assert_eq!(outcome.exit_code, 0, "successful export must exit 0: {outcome:?}");

        // AC-01: `bin/sotp` exists and is byte-identical to the running binary.
        let transplanted = output_dir.join("bin/sotp");
        assert!(transplanted.exists(), "bin/sotp must exist under the output tree");
        let source = std::env::current_exe().unwrap();
        assert_eq!(
            std::fs::read(&source).unwrap(),
            std::fs::read(&transplanted).unwrap(),
            "bin/sotp must be byte-identical to the running binary",
        );

        // AC-06: both paths must identify the same inode when the source and
        // target are on the normal shared target filesystem. The adapter's
        // EXDEV fallback is intentionally allowed when those roots differ.
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;

            let source_metadata = std::fs::metadata(&source).unwrap();
            let transplanted_metadata = std::fs::metadata(&transplanted).unwrap();
            if source_metadata.dev() == transplanted_metadata.dev() {
                assert_eq!(
                    source_metadata.ino(),
                    transplanted_metadata.ino(),
                    "test transplant must be a hard link, not a second file",
                );
            }
        }

        // AC-01 unix arm: executable permission preserved.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&transplanted).unwrap().permissions().mode();
            assert!(mode & 0o100 != 0, "bin/sotp must be executable: mode={mode:o}");
        }
    }
}
