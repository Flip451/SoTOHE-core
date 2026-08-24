//! System adapter for the active-track catalogue-lint port.

use std::path::Path;
use std::sync::Arc;

use domain::TrackId;
use domain::tddd::catalogue_linter::TypeRefPathExtractorPort;
use usecase::catalogue_lint_workflow::{
    RunCatalogueLint, RunCatalogueLintCommand, RunCatalogueLintError, RunCatalogueLintInteractor,
};
use usecase::track_lifecycle::TrackCataloguePath;
use usecase::track_lifecycle::tddd::catalogue_lint_active::{
    TrackCatalogueLintActiveCommand, TrackCatalogueLintActiveError, TrackCatalogueLintActivePort,
    TrackCatalogueLintActiveResult, TrackCatalogueLintLayerResult,
};

use crate::tddd::contract_map_adapter::FsCatalogueLoader;
use crate::tddd::fs_lint_config_loader::FsLintConfigLoader;
use crate::tddd::syn_primitive_occurrence_scanner::SynPrimitiveOccurrenceScanner;
use crate::verify::tddd_layers::{LoadTdddLayersError, load_tddd_layers};

/// System-backed adapter for active-track catalogue linting.
pub struct SystemTrackCatalogueLintActiveAdapter {
    extractor: Arc<dyn TypeRefPathExtractorPort>,
}

impl SystemTrackCatalogueLintActiveAdapter {
    /// Creates the active-track adapter with the parser-authoritative extractor.
    #[must_use]
    pub fn new(extractor: Arc<dyn TypeRefPathExtractorPort>) -> Self {
        Self { extractor }
    }
}

impl TrackCatalogueLintActivePort for SystemTrackCatalogueLintActiveAdapter {
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackCatalogueLintActiveCommand,
    ) -> Result<TrackCatalogueLintActiveResult, TrackCatalogueLintActiveError> {
        let workspace_root = command.workspace_root.as_path();
        let rules_path = workspace_root.join("architecture-rules.json");
        let bindings = load_tddd_layers(&rules_path, workspace_root).map_err(|error| {
            execution_failed(format!("layer bindings load failed: {}", layer_bindings_error(error)))
        })?;
        if bindings.is_empty() {
            return Err(execution_failed(
                "no tddd.enabled layers found in architecture-rules.json; nothing to lint",
            ));
        }

        let items_dir = workspace_root.join("track/items");
        let track_dir = items_dir.join(track_id.as_ref());
        let config_path = crate::track_lifecycle::tddd::lint::resolve_config_path(
            workspace_root,
            command.rules_file.as_ref(),
        )
        .map_err(execution_failed)?;
        ensure_config_file(&config_path)?;

        for binding in &bindings {
            let catalogue_path = track_dir.join(binding.catalogue_file());
            match catalogue_path.symlink_metadata() {
                Ok(metadata) if metadata.file_type().is_file() => {}
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let layer = domain::tddd::LayerId::try_new(binding.layer_id().to_owned())
                        .map_err(|error| {
                            execution_failed(format!("invalid TDDD layer id: {error}"))
                        })?;
                    let path = TrackCataloguePath::try_new(catalogue_path)
                        .map_err(|error| execution_failed(error.to_string()))?;
                    return Ok(TrackCatalogueLintActiveResult::Skipped { layer, path });
                }
                Err(error) => {
                    return Err(execution_failed(format!(
                        "cannot stat catalogue '{}' for layer '{}': {error}",
                        catalogue_path.display(),
                        binding.layer_id(),
                    )));
                }
            }
        }

        let loader = FsCatalogueLoader::new(items_dir, rules_path, workspace_root.to_path_buf());
        let config_loader = FsLintConfigLoader::new(config_path);
        let interactor = RunCatalogueLintInteractor::new(
            loader,
            config_loader,
            SynPrimitiveOccurrenceScanner,
            self.extractor.clone(),
        );
        let runner: &dyn RunCatalogueLint = &interactor;
        let mut layers = Vec::new();

        for binding in &bindings {
            let violations = match runner.execute(RunCatalogueLintCommand {
                track_id: track_id.as_ref().to_owned(),
                layer_id: binding.layer_id().to_owned(),
                rules: Vec::new(),
            }) {
                Ok(violations) => violations,
                Err(RunCatalogueLintError::ConfigMissing { path }) => {
                    return Err(execution_failed(lint_config_missing_message(&path)));
                }
                Err(error) => {
                    return Err(execution_failed(format!(
                        "catalogue lint failed for layer '{}': {error}",
                        binding.layer_id()
                    )));
                }
            };
            let layer = domain::tddd::LayerId::try_new(binding.layer_id().to_owned())
                .map_err(|error| execution_failed(format!("invalid TDDD layer id: {error}")))?;
            layers.push(TrackCatalogueLintLayerResult { layer, violations });
        }

        Ok(TrackCatalogueLintActiveResult::Checked { layers })
    }
}

fn ensure_config_file(path: &Path) -> Result<(), TrackCatalogueLintActiveError> {
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(execution_failed(format!(
            "refusing to load a symlinked lint config: {}",
            path.display()
        ))),
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => {
            Err(execution_failed(format!("lint config is not a regular file: {}", path.display())))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(execution_failed(lint_config_missing_message(path)))
        }
        Err(error) => {
            Err(execution_failed(format!("cannot stat lint config '{}': {error}", path.display())))
        }
    }
}

fn lint_config_missing_message(path: &Path) -> String {
    format!(
        "lint config not found at {}. Copy `.harness/catalogue-lint/presets/ddd-strict.json` to that location to enable linting.",
        path.display()
    )
}

fn layer_bindings_error(error: LoadTdddLayersError) -> String {
    match error {
        LoadTdddLayersError::Io { path, source } => format!("{}: {source}", path.display()),
        LoadTdddLayersError::Parse(error) => error.to_string(),
    }
}

fn execution_failed(message: impl Into<String>) -> TrackCatalogueLintActiveError {
    TrackCatalogueLintActiveError::ExecutionFailed(usecase::git_workflow::DiagnosticText::new(
        message,
    ))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::tddd::type_ref_parser::SynTypeRefPathExtractorAdapter;
    use usecase::track_lifecycle::tddd::lint::TrackLintPort;
    use usecase::track_lifecycle::{TrackSelection, TrackWorkspaceRoot};

    fn command(root: &std::path::Path) -> TrackCatalogueLintActiveCommand {
        TrackCatalogueLintActiveCommand {
            track: TrackSelection::Explicit(
                TrackId::try_new("lint-track").expect("track id is valid"),
            ),
            workspace_root: TrackWorkspaceRoot::try_new(root.to_path_buf())
                .expect("workspace root is valid"),
            rules_file: None,
        }
    }

    fn write_rules(root: &std::path::Path) {
        fs::write(
            root.join("architecture-rules.json"),
            r#"{
              "version": 2,
              "layers": [{
                "crate": "domain",
                "tddd": {
                  "enabled": true,
                  "catalogue_file": "domain-types.json"
                }
              }]
            }"#,
        )
        .expect("architecture rules are written");
    }

    fn write_config(root: &std::path::Path) {
        let config = root.join(".harness/catalogue-lint");
        fs::create_dir_all(&config).expect("lint config directory exists");
        fs::write(config.join("config.json"), r#"{"schema_version":1,"rules":[]}"#)
            .expect("lint config is written");
    }

    #[test]
    fn test_system_catalogue_lint_adapters_forward_injected_extractor_and_identity_rules() {
        struct RecordingExtractor {
            calls: Arc<AtomicUsize>,
            parser: SynTypeRefPathExtractorAdapter,
        }

        impl domain::tddd::catalogue_linter::TypeRefPathExtractorPort for RecordingExtractor {
            fn extract(
                &self,
                type_ref: &domain::tddd::catalogue_v2::identifiers::TypeRef,
                type_parameters: &[domain::tddd::catalogue_v2::identifiers::ParamName],
                lifetime_parameters: &[domain::tddd::catalogue_v2::identifiers::ParamName],
                const_parameters: &[domain::tddd::catalogue_v2::identifiers::ParamName],
            ) -> Result<
                Vec<domain::tddd::catalogue_linter::ExtractedTypeRefPath>,
                domain::tddd::catalogue_linter::TypeRefPathExtractionError,
            > {
                self.calls.fetch_add(1, Ordering::SeqCst);
                self.parser.extract(
                    type_ref,
                    type_parameters,
                    lifetime_parameters,
                    const_parameters,
                )
            }
        }

        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let root = workspace.path();
        fs::write(
            root.join("architecture-rules.json"),
            r#"{
              "version": 2,
              "layers": [{
                "crate": "domain",
                "tddd": {"enabled": true, "catalogue_file": "domain-types.json"}
              }]
            }"#,
        )
        .expect("architecture rules are written");
        fs::create_dir_all(root.join(".harness/catalogue-lint"))
            .expect("lint config directory exists");
        fs::write(
            root.join(".harness/catalogue-lint/config.json"),
            r#"{
              "schema_version": 1,
              "rules": [
                {
                  "target_roles": ["UseCase"],
                  "kind": {"ReferencedRoleConstraint": {
                    "target_field": "handles",
                    "expected_role": "DomainEvent"
                  }}
                },
                {
                  "target_roles": ["AggregateRoot"],
                  "kind": {"FieldElementUniqueAcrossEntries": {
                    "target_field": "exclusive_members"
                  }}
                },
                {
                  "target_roles": ["AggregateRoot"],
                  "kind": {"NoExternalReferenceInMethods": {
                    "target_field": "exclusive_members"
                  }}
                }
              ]
            }"#,
        )
        .expect("lint config is written");
        fs::create_dir_all(root.join("track/items/lint-track")).expect("track directory exists");
        fs::write(
            root.join("track/items/lint-track/domain-types.json"),
            r#"{
              "schema_version": 5,
              "crate_name": "domain",
              "layer": "domain",
              "types": {
                "domain::alpha::Event": {
                  "action": "add",
                  "role": {"DomainEvent": {}},
                  "kind": {"kind": "struct", "shape": {"kind": "plain"}},
                  "methods": [],
                  "module_path": "alpha",
                  "spec_refs": [],
                  "informal_grounds": []
                },
                "domain::beta::Event": {
                  "action": "add",
                  "role": {"ValueObject": {}},
                  "kind": {"kind": "struct", "shape": {"kind": "plain"}},
                  "methods": [],
                  "module_path": "beta",
                  "spec_refs": [],
                  "informal_grounds": []
                },
                "domain::alpha::Entity": {
                  "action": "add",
                  "role": {"ValueObject": {}},
                  "kind": {"kind": "struct", "shape": {"kind": "plain"}},
                  "methods": [],
                  "module_path": "alpha",
                  "spec_refs": [],
                  "informal_grounds": []
                },
                "domain::beta::Entity": {
                  "action": "add",
                  "role": {"ValueObject": {}},
                  "kind": {"kind": "struct", "shape": {"kind": "plain"}},
                  "methods": [],
                  "module_path": "beta",
                  "spec_refs": [],
                  "informal_grounds": []
                },
                "domain::alpha::AggregateA": {
                  "action": "add",
                  "role": {"AggregateRoot": {
                    "identity": {"method_name": "id"},
                    "invariants": [],
                    "exclusive_members": [
                      "std::not_a_real_wrapper<&'static domain::alpha::Entity>"
                    ],
                    "shared_value_objects": [],
                    "emits": []
                  }},
                  "kind": {"kind": "struct", "shape": {"kind": "plain"}},
                  "methods": [],
                  "module_path": "alpha",
                  "spec_refs": [],
                  "informal_grounds": []
                },
                "domain::beta::AggregateB": {
                  "action": "add",
                  "role": {"AggregateRoot": {
                    "identity": {"method_name": "id"},
                    "invariants": [],
                    "exclusive_members": [
                      "std::not_a_real_wrapper<*const domain::beta::Entity>"
                    ],
                    "shared_value_objects": [],
                    "emits": []
                  }},
                  "kind": {"kind": "struct", "shape": {"kind": "plain"}},
                  "methods": [],
                  "module_path": "beta",
                  "spec_refs": [],
                  "informal_grounds": []
                },
                "domain::alpha::AggregateAmbiguous": {
                  "action": "add",
                  "role": {"AggregateRoot": {
                    "identity": {"method_name": "id"},
                    "invariants": [],
                    "exclusive_members": ["Entity"],
                    "shared_value_objects": [],
                    "emits": []
                  }},
                  "kind": {"kind": "struct", "shape": {"kind": "plain"}},
                  "methods": [],
                  "module_path": "alpha",
                  "spec_refs": [],
                  "informal_grounds": []
                },
                "domain::alpha::ExternalService": {
                  "action": "add",
                  "role": {"DomainService": {"emits": []}},
                  "kind": {"kind": "struct", "shape": {"kind": "plain"}},
                  "methods": [{
                    "action": "add",
                    "has_default_impl": false,
                    "is_async": false,
                    "name": "read_entity",
                    "params": [],
                    "receiver": "&self",
                    "returns": "std::not_a_real_wrapper<&'static domain::alpha::Entity>"
                  }],
                  "module_path": "alpha",
                  "spec_refs": [],
                  "informal_grounds": []
                },
                "domain::alpha::UseCaseValid": {
                  "action": "add",
                  "role": {"UseCase": {"handles": [
                    "std::not_a_real_wrapper<&'static domain::alpha::Event>"
                  ]}},
                  "kind": {"kind": "struct", "shape": {"kind": "plain"}},
                  "methods": [],
                  "module_path": "alpha",
                  "spec_refs": [],
                  "informal_grounds": []
                },
                "domain::alpha::UseCaseAmbiguous": {
                  "action": "add",
                  "role": {"UseCase": {"handles": ["Event"]}},
                  "kind": {"kind": "struct", "shape": {"kind": "plain"}},
                  "methods": [],
                  "module_path": "alpha",
                  "spec_refs": [],
                  "informal_grounds": []
                },
                "domain::alpha::UseCaseUnresolved": {
                  "action": "add",
                  "role": {"UseCase": {"handles": ["domain::missing::Event"]}},
                  "kind": {"kind": "struct", "shape": {"kind": "plain"}},
                  "methods": [],
                  "module_path": "alpha",
                  "spec_refs": [],
                  "informal_grounds": []
                }
              },
              "traits": {},
              "functions": {}
            }"#,
        )
        .expect("catalogue fixture is written");

        let calls = Arc::new(AtomicUsize::new(0));
        let extractor: Arc<dyn domain::tddd::catalogue_linter::TypeRefPathExtractorPort> =
            Arc::new(RecordingExtractor {
                calls: calls.clone(),
                parser: SynTypeRefPathExtractorAdapter,
            });
        let active_adapter = SystemTrackCatalogueLintActiveAdapter::new(extractor.clone());
        let active_result = active_adapter
            .execute(TrackId::try_new("lint-track").expect("track id is valid"), command(root))
            .expect("active-track lint execution succeeds");
        let active_layers = match active_result {
            TrackCatalogueLintActiveResult::Checked { layers } => layers,
            _ => panic!("expected checked active-track result"),
        };
        assert_eq!(active_layers.len(), 1);
        let active_violations = &active_layers.first().unwrap().violations;
        assert_eq!(active_violations.len(), 5);
        assert_eq!(
            active_violations
                .iter()
                .filter(|violation| violation.rule_kind() == "ReferencedRoleConstraint")
                .count(),
            2
        );
        assert_eq!(
            active_violations
                .iter()
                .filter(|violation| violation.rule_kind() == "FieldElementUniqueAcrossEntries")
                .count(),
            1
        );
        assert_eq!(
            active_violations
                .iter()
                .filter(|violation| violation.rule_kind() == "NoExternalReferenceInMethods")
                .count(),
            2
        );
        assert!(active_violations.iter().any(|violation| {
            violation.message().contains("ambiguous identifier")
                && violation.message().contains("Event")
        }));
        let ambiguous_message = active_violations
            .iter()
            .find(|violation| violation.message().contains("ambiguous identifier"))
            .unwrap()
            .message();
        assert!(ambiguous_message.contains("FullyQualifiedItemPath"));
        assert!(ambiguous_message.contains("Identifier(\"alpha\")"));
        assert!(ambiguous_message.contains("Identifier(\"beta\")"));
        assert!(active_violations.iter().any(|violation| {
            violation.message().contains("unresolved identifier")
                && violation.message().contains("domain::missing::Event")
        }));
        assert!(active_violations.iter().any(|violation| {
            violation.rule_kind() == "NoExternalReferenceInMethods"
                && violation.message().contains("domain::alpha::Entity")
                && violation.message().contains("domain::alpha::ExternalService")
        }));
        assert!(
            !active_violations
                .iter()
                .any(|violation| violation.entry_name() == "domain::alpha::UseCaseValid")
        );

        let single_adapter =
            crate::track_lifecycle::tddd::lint::SystemTrackLintAdapter::new(extractor);
        let single_command = usecase::track_lifecycle::tddd::lint::TrackLintCommand {
            track: usecase::track_lifecycle::TrackSelection::Explicit(
                TrackId::try_new("lint-track").expect("track id is valid"),
            ),
            workspace_root: usecase::track_lifecycle::TrackWorkspaceRoot::try_new(
                root.to_path_buf(),
            )
            .expect("workspace root is valid"),
            layer: domain::tddd::LayerId::try_new("domain".to_owned()).expect("layer is valid"),
            rules_file: None,
        };
        let single_result = single_adapter
            .execute(TrackId::try_new("lint-track").expect("track id is valid"), single_command)
            .expect("single-layer lint execution succeeds");
        assert_eq!(single_result.violations.len(), 5);
        assert_eq!(
            single_result
                .violations
                .iter()
                .filter(|violation| violation.rule_kind() == "FieldElementUniqueAcrossEntries")
                .count(),
            1
        );
        assert_eq!(
            single_result
                .violations
                .iter()
                .filter(|violation| violation.rule_kind() == "NoExternalReferenceInMethods")
                .count(),
            2
        );
        assert!(
            single_result
                .violations
                .iter()
                .any(|violation| { violation.message().contains("ambiguous identifier") })
        );
        assert!(
            single_result
                .violations
                .iter()
                .any(|violation| { violation.message().contains("unresolved identifier") })
        );
        let single_ambiguous_message = single_result
            .violations
            .iter()
            .find(|violation| violation.message().contains("ambiguous identifier"))
            .unwrap()
            .message();
        assert!(single_ambiguous_message.contains("FullyQualifiedItemPath"));
        assert!(single_ambiguous_message.contains("Identifier(\"alpha\")"));
        assert!(single_ambiguous_message.contains("Identifier(\"beta\")"));
        assert!(calls.load(Ordering::SeqCst) >= 6);
    }

    #[test]
    fn test_system_track_catalogue_lint_active_adapter_missing_rules_returns_execution_error() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let error = match SystemTrackCatalogueLintActiveAdapter::new(Arc::new(
            SynTypeRefPathExtractorAdapter,
        ))
        .execute(
            TrackId::try_new("lint-track").expect("track id is valid"),
            command(workspace.path()),
        ) {
            Ok(_) => panic!("missing architecture rules must fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("layer bindings load failed"));
    }

    #[test]
    fn test_system_track_catalogue_lint_active_adapter_absent_catalogue_returns_skipped_layer() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        fs::create_dir_all(workspace.path().join("track/items/lint-track"))
            .expect("track directory exists");
        write_rules(workspace.path());
        write_config(workspace.path());

        let result =
            SystemTrackCatalogueLintActiveAdapter::new(Arc::new(SynTypeRefPathExtractorAdapter))
                .execute(
                    TrackId::try_new("lint-track").expect("track id is valid"),
                    command(workspace.path()),
                )
                .expect("absent catalogue is skipped");

        assert!(matches!(
            result,
            TrackCatalogueLintActiveResult::Skipped { layer, path }
                if layer.as_ref() == "domain" && path.as_path().ends_with("domain-types.json")
        ));
    }

    #[test]
    fn test_system_track_catalogue_lint_active_adapter_missing_config_returns_execution_error() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        fs::create_dir_all(workspace.path().join("track/items/lint-track"))
            .expect("track directory exists");
        write_rules(workspace.path());

        let error = match SystemTrackCatalogueLintActiveAdapter::new(Arc::new(
            SynTypeRefPathExtractorAdapter,
        ))
        .execute(
            TrackId::try_new("lint-track").expect("track id is valid"),
            command(workspace.path()),
        ) {
            Ok(_) => panic!("missing config must fail closed"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("lint config not found"));
    }

    #[test]
    fn test_system_track_catalogue_lint_active_adapter_rules_file_outside_workspace_fails_closed() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let outside = tempfile::tempdir().expect("outside config exists");
        write_rules(workspace.path());
        let rules = outside.path().join("rules.json");
        fs::write(&rules, r#"{"schema_version":1,"rules":[]}"#)
            .expect("outside rules file is written");
        let mut command = command(workspace.path());
        command.rules_file = Some(
            usecase::track_lifecycle::tddd::lint::TrackLintRulesFile::try_new(rules)
                .expect("rules file is valid"),
        );

        let error = match SystemTrackCatalogueLintActiveAdapter::new(Arc::new(
            SynTypeRefPathExtractorAdapter,
        ))
        .execute(TrackId::try_new("lint-track").expect("track id is valid"), command)
        {
            Ok(_) => panic!("outside rules file must fail closed"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("outside the workspace trusted root"),
            "outside rules file must fail closed: {error}"
        );
    }

    #[test]
    fn test_system_track_catalogue_lint_active_adapter_directory_config_fails_when_catalogue_absent()
     {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        fs::create_dir_all(workspace.path().join("track/items/lint-track"))
            .expect("track directory exists");
        write_rules(workspace.path());
        fs::create_dir_all(workspace.path().join(".harness/catalogue-lint/config.json"))
            .expect("directory occupies the lint config path");

        let error = match SystemTrackCatalogueLintActiveAdapter::new(Arc::new(
            SynTypeRefPathExtractorAdapter,
        ))
        .execute(
            TrackId::try_new("lint-track").expect("track id is valid"),
            command(workspace.path()),
        ) {
            Ok(_) => panic!("directory config must fail closed"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("lint config is not a regular file"),
            "directory config must fail closed: {error}"
        );
    }

    #[test]
    fn test_system_track_catalogue_lint_active_adapter_symlink_config_fails_when_catalogue_absent()
    {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        fs::create_dir_all(workspace.path().join("track/items/lint-track"))
            .expect("track directory exists");
        write_rules(workspace.path());
        let config_dir = workspace.path().join(".harness/catalogue-lint");
        fs::create_dir_all(&config_dir).expect("lint config directory exists");
        let target = config_dir.join("target.json");
        fs::write(&target, r#"{"schema_version":1,"rules":[]}"#)
            .expect("symlink target is written");
        std::os::unix::fs::symlink(&target, config_dir.join("config.json"))
            .expect("lint config symlink is created");

        let error = match SystemTrackCatalogueLintActiveAdapter::new(Arc::new(
            SynTypeRefPathExtractorAdapter,
        ))
        .execute(
            TrackId::try_new("lint-track").expect("track id is valid"),
            command(workspace.path()),
        ) {
            Ok(_) => panic!("symlinked config must fail closed"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("refusing to load a symlinked lint config"),
            "symlinked config must fail closed: {error}"
        );
    }
}
