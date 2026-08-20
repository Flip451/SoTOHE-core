//! Command dispatch for track subcommands.
//!
//! Extracted from `mod.rs` to keep the module within the production-code line
//! limit declared by `architecture-rules.json` (`module_limits.max_lines`).

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::track::TrackInput;

use super::state_ops::track_driver_outcome_to_result;
use super::{
    TrackCommand, archive, branch_ops, fixpoint_resolve, resolve, set_commit_hash, state_ops, tddd,
    transition, views,
};
use crate::commands::track::{
    resolve_track_id, resolve_track_id_for_write, resolve_track_id_from_root,
    resolve_track_id_from_root_for_write,
};

/// Dispatches `cmd` and returns `(ExitCode, Option<String>)`.
///
/// The `Option<String>` is `Some(error_message)` when the dispatch produced a
/// `CliError`, and `None` on success.  The error message is also printed to
/// stderr so user-visible output is unchanged from `execute`.
///
#[allow(clippy::too_many_lines)]
pub fn execute_with_error_chain(cmd: TrackCommand) -> (ExitCode, Option<String>) {
    use crate::CliError;

    let result: Result<ExitCode, CliError> = dispatch_track_cmd(cmd);
    match result {
        Ok(code) => (code, None),
        Err(err) => {
            let msg = err.to_string();
            eprintln!("{msg}");
            (err.exit_code(), Some(msg))
        }
    }
}

/// Public entry point for callers that do not need the error chain string.
#[allow(dead_code)]
pub fn execute(cmd: TrackCommand) -> ExitCode {
    execute_with_error_chain(cmd).0
}

/// Performs the actual command dispatch, returning `Result<ExitCode, CliError>`.
///
/// Extracted from `execute` / `execute_with_error_chain` so the dispatch
/// logic is written once and the two public entry points share it.
#[allow(clippy::too_many_lines)]
fn dispatch_track_cmd(cmd: TrackCommand) -> Result<ExitCode, crate::CliError> {
    dispatch_track_cmd_with_dependencies(cmd, |input| {
        TrackCompositionRoot::new().track_driver().handle_base_merge(input)
    })
}

/// Dispatches track commands with injected command-boundary outcomes.
///
/// Keeping this seam local lets command tests exercise the `MergeBase` enum dispatch without
/// reimplementing any guarded merge policy outside the driver and usecase layers.
#[allow(clippy::too_many_lines)]
fn dispatch_track_cmd_with_dependencies(
    cmd: TrackCommand,
    base_merge: impl FnOnce(cli_driver::track::BaseMergeInput) -> cli_driver::CommandOutcome,
) -> Result<ExitCode, crate::CliError> {
    use crate::CliError;

    match cmd {
        TrackCommand::Archive { items_dir, track_id } => {
            resolve_track_id_for_write(track_id, &items_dir)
                .map_err(|e| CliError::Message(e.to_string()))
                .and_then(|tid| archive::execute_archive(items_dir, tid))
        }
        TrackCommand::Transition { items_dir, track_id, task_id, target_status, commit_hash } => {
            resolve_track_id_for_write(track_id, &items_dir)
                .map_err(|e| CliError::Message(e.to_string()))
                .and_then(|tid| {
                    transition::execute_transition(
                        items_dir,
                        tid,
                        task_id,
                        target_status,
                        commit_hash,
                    )
                })
        }
        TrackCommand::Branch { action } => branch_ops::execute_branch(action),
        TrackCommand::Resolve(args) => resolve::execute_resolve(args),
        TrackCommand::Views { action } => views::execute_views(action),
        TrackCommand::AddTask { items_dir, track_id, description, section, after } => {
            resolve_track_id_for_write(track_id, &items_dir)
                .map_err(|e| CliError::Message(e.to_string()))
                .and_then(|tid| {
                    state_ops::execute_add_task(items_dir, tid, description, section, after)
                })
        }
        TrackCommand::SetOverride { items_dir, track_id, status, reason } => {
            resolve_track_id_for_write(track_id, &items_dir)
                .map_err(|e| CliError::Message(e.to_string()))
                .and_then(|tid| state_ops::execute_set_override(items_dir, tid, status, reason))
        }
        TrackCommand::ClearOverride { items_dir, track_id } => {
            resolve_track_id_for_write(track_id, &items_dir)
                .map_err(|e| CliError::Message(e.to_string()))
                .and_then(|tid| state_ops::execute_clear_override(items_dir, tid))
        }
        TrackCommand::NextTask { items_dir, track_id } => resolve_track_id(track_id, &items_dir)
            .map_err(|e| CliError::Message(e.to_string()))
            .and_then(|tid| state_ops::execute_next_task(items_dir, tid)),
        TrackCommand::TaskCounts { items_dir, track_id } => resolve_track_id(track_id, &items_dir)
            .map_err(|e| CliError::Message(e.to_string()))
            .and_then(|tid| state_ops::execute_task_counts(items_dir, tid)),
        TrackCommand::TypeGraph {
            items_dir,
            track_id,
            workspace_root,
            layer,
            cluster_depth,
            edges,
        } => tddd::graph::execute_type_graph(
            items_dir,
            track_id.unwrap_or_else(|| "removed-command".to_owned()),
            workspace_root,
            layer,
            cluster_depth,
            edges,
        ),
        TrackCommand::TypeSignals { track_id, workspace_root, layer } => {
            tddd::type_signals::execute_type_signals(track_id, workspace_root, layer)
        }
        TrackCommand::BaselineGraph { items_dir, track_id, workspace_root, layers } => {
            let resolved = resolve_track_id_from_root_for_write(track_id, &workspace_root)
                .map_err(|e| CliError::Message(e.to_string()));
            resolved.and_then(|tid| {
                tddd::baseline_graph::execute_baseline_graph(items_dir, tid, workspace_root, layers)
            })
        }
        TrackCommand::ContractMap { items_dir, track_id, workspace_root, layers } => {
            let resolved = resolve_track_id_from_root_for_write(track_id, &workspace_root)
                .map_err(|e| CliError::Message(e.to_string()));
            resolved.and_then(|tid| {
                tddd::contract_map::execute_contract_map(items_dir, tid, workspace_root, layers)
            })
        }
        TrackCommand::SpecElementHash { items_dir, track_id, anchor } => {
            resolve_track_id(track_id, &items_dir)
                .map_err(|e| CliError::Message(e.to_string()))
                .and_then(|tid| {
                    tddd::spec_element_hash::execute_spec_element_hash(items_dir, tid, anchor)
                })
        }
        TrackCommand::BaselineCapture { track_id, workspace_root, source_workspace, layer } => {
            let resolved = resolve_track_id_from_root_for_write(track_id, &workspace_root)
                .map_err(|e| CliError::Message(e.to_string()));
            resolved.and_then(|tid| {
                tddd::baseline::execute_baseline_capture(
                    tid,
                    workspace_root,
                    source_workspace,
                    layer,
                )
            })
        }
        TrackCommand::Lint { track_id, layer_id, workspace_root, rules_file } => {
            resolve_track_id_from_root(track_id, &workspace_root)
                .map_err(|e| CliError::Message(e.to_string()))
                .and_then(|tid| tddd::lint::execute_lint(workspace_root, tid, layer_id, rules_file))
        }
        TrackCommand::CatalogueImplSignals { track_id, workspace_root, layer } => {
            let resolved = resolve_track_id_from_root(track_id, &workspace_root)
                .map_err(|e| CliError::Message(e.to_string()));
            resolved.and_then(|tid| {
                tddd::catalogue_impl_signals::execute_catalogue_impl_signals(
                    tid,
                    workspace_root,
                    layer,
                )
            })
        }
        TrackCommand::FixpointResolve(args) => fixpoint_resolve::execute_fixpoint_resolve(args),
        TrackCommand::SetCommitHash(args) => {
            resolve_track_id_from_root_for_write(args.track_id, &PathBuf::from("."))
                .map_err(|e| CliError::Message(e.to_string()))
                .and_then(set_commit_hash::execute_set_commit_hash)
        }
        TrackCommand::SwitchBase { project_root } => {
            let outcome = TrackCompositionRoot::new()
                .track_driver()
                .handle(TrackInput::SwitchBase { project_root });
            track_driver_outcome_to_result(outcome)
        }
        TrackCommand::MergeBase => {
            let outcome = base_merge(cli_driver::track::BaseMergeInput {
                workspace_root: PathBuf::from("."),
            });
            track_driver_outcome_to_result(outcome)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_track_command_merge_base_dispatches_argument_free_workspace_to_completed_outcome() {
        let result = dispatch_track_cmd_with_dependencies(TrackCommand::MergeBase, |input| {
            assert_eq!(input.workspace_root, PathBuf::from("."));
            cli_driver::CommandOutcome::success(Some("base merge completed".to_owned()))
        });

        assert_eq!(result.unwrap(), ExitCode::SUCCESS);
    }

    #[test]
    fn test_track_command_merge_base_dispatches_conflicted_outcome_as_recovery_failure() {
        let result = dispatch_track_cmd_with_dependencies(TrackCommand::MergeBase, |_| {
            cli_driver::CommandOutcome::failure(Some(
                "base merge conflicted; continue with /track:recover".to_owned(),
            ))
        });

        let error = result.unwrap_err().to_string();
        assert!(error.contains("/track:recover"));
        assert!(!error.contains("base merge completed"));
    }

    #[test]
    fn test_track_command_merge_base_dispatches_composition_root_guard_failure_without_merge_success()
     {
        let workspace = tempfile::tempdir().unwrap();
        let result = dispatch_track_cmd_with_dependencies(TrackCommand::MergeBase, |_| {
            TrackCompositionRoot::new().track_driver().handle_base_merge(
                cli_driver::track::BaseMergeInput {
                    workspace_root: workspace.path().to_path_buf(),
                },
            )
        });

        let error = result.unwrap_err().to_string();
        assert!(error.contains("base merge failed"));
        assert!(!error.contains("base merge completed"));
    }

    #[test]
    fn test_execute_type_graph_removed_command_returns_t008_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();
        let argv_items = items_dir.clone();
        let argv_track_id = Some("test-track".to_owned());
        let argv_workspace = dir.path().to_path_buf();
        let argv_layer = Option::<String>::None;
        let argv_cluster_depth = 0usize;
        let argv_edges = "methods".to_owned();

        let result = dispatch_track_cmd(TrackCommand::TypeGraph {
            items_dir: argv_items.clone(),
            track_id: argv_track_id.clone(),
            workspace_root: argv_workspace.clone(),
            layer: argv_layer.clone(),
            cluster_depth: argv_cluster_depth,
            edges: argv_edges.clone(),
        });

        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("T008"), "error must mention T008: {msg}");
        assert!(
            msg.contains("catalogue-impl-signals"),
            "error must mention the replacement command: {msg}"
        );
        assert_eq!(argv_items, items_dir);
        assert_eq!(argv_track_id.as_deref(), Some("test-track"));
        assert_eq!(argv_workspace, dir.path());
        assert_eq!(argv_cluster_depth, 0);
        assert_eq!(argv_edges, "methods");
        assert!(
            !dir.path().join("track/items/test-track").exists(),
            "removed type-graph command must not persist artifacts"
        );
    }

    #[test]
    fn test_execute_type_graph_rejects_invalid_track_id_before_execution() {
        let result = dispatch_track_cmd(TrackCommand::TypeGraph {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("../escape".to_owned()),
            workspace_root: PathBuf::from("workspace"),
            layer: None,
            cluster_depth: 0,
            edges: "methods".to_owned(),
        });

        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("invalid track id"), "got: {msg}");
    }

    #[test]
    fn test_execute_type_graph_omitted_track_id_still_returns_t008_error() {
        let result = dispatch_track_cmd(TrackCommand::TypeGraph {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: None,
            workspace_root: PathBuf::from("workspace"),
            layer: None,
            cluster_depth: 0,
            edges: "methods".to_owned(),
        });

        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("T008"), "error must mention T008: {msg}");
        assert!(
            msg.contains("catalogue-impl-signals"),
            "error must mention the replacement command: {msg}"
        );
    }
}
