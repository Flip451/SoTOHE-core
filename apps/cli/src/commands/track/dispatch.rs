//! Command dispatch for track subcommands.
//!
//! Extracted from `mod.rs` to keep the module within the 700-line production
//! code limit (see `knowledge/conventions/impl-delegation-arch-guard.md`).

use std::path::PathBuf;
use std::process::ExitCode;

use super::{
    TrackCommand, archive, branch_ops, resolve, set_commit_hash, signals, state_ops, tddd,
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
/// This variant exists so that `execute_track_with_telemetry` in `main.rs` can
/// populate `NonZeroExit.error_chain` (IN-03) without a second dispatch round.
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
pub fn execute(cmd: TrackCommand) -> ExitCode {
    execute_with_error_chain(cmd).0
}

/// Performs the actual command dispatch, returning `Result<ExitCode, CliError>`.
///
/// Extracted from `execute` / `execute_with_error_chain` so the dispatch
/// logic is written once and the two public entry points share it.
#[allow(clippy::too_many_lines)]
fn dispatch_track_cmd(cmd: TrackCommand) -> Result<ExitCode, crate::CliError> {
    use crate::CliError;

    match cmd {
        TrackCommand::Archive { items_dir, track_id } => {
            resolve_track_id_for_write(track_id, &items_dir)
                .map_err(CliError::Message)
                .and_then(|tid| archive::execute_archive(items_dir, tid))
        }
        TrackCommand::Transition { items_dir, track_id, task_id, target_status, commit_hash } => {
            resolve_track_id_for_write(track_id, &items_dir).map_err(CliError::Message).and_then(
                |tid| {
                    transition::execute_transition(
                        items_dir,
                        tid,
                        task_id,
                        target_status,
                        commit_hash,
                    )
                },
            )
        }
        TrackCommand::Branch { action } => branch_ops::execute_branch(action),
        TrackCommand::Resolve(args) => resolve::execute_resolve(args),
        TrackCommand::Views { action } => views::execute_views(action),
        TrackCommand::AddTask { items_dir, track_id, description, section, after } => {
            resolve_track_id_for_write(track_id, &items_dir).map_err(CliError::Message).and_then(
                |tid| state_ops::execute_add_task(items_dir, tid, description, section, after),
            )
        }
        TrackCommand::SetOverride { items_dir, track_id, status, reason } => {
            resolve_track_id_for_write(track_id, &items_dir)
                .map_err(CliError::Message)
                .and_then(|tid| state_ops::execute_set_override(items_dir, tid, status, reason))
        }
        TrackCommand::ClearOverride { items_dir, track_id } => {
            resolve_track_id_for_write(track_id, &items_dir)
                .map_err(CliError::Message)
                .and_then(|tid| state_ops::execute_clear_override(items_dir, tid))
        }
        TrackCommand::NextTask { items_dir, track_id } => resolve_track_id(track_id, &items_dir)
            .map_err(CliError::Message)
            .and_then(|tid| state_ops::execute_next_task(items_dir, tid)),
        TrackCommand::TaskCounts { items_dir, track_id } => resolve_track_id(track_id, &items_dir)
            .map_err(CliError::Message)
            .and_then(|tid| state_ops::execute_task_counts(items_dir, tid)),
        TrackCommand::Signals { items_dir, track_id } => {
            resolve_track_id_for_write(track_id, &items_dir)
                .map_err(CliError::Message)
                .and_then(|tid| signals::execute_signals(items_dir, tid))
        }
        TrackCommand::TypeSignals { track_id, workspace_root, layer } => {
            let resolved = resolve_track_id_from_root_for_write(track_id, &workspace_root)
                .map_err(CliError::Message);
            resolved.and_then(|tid| tddd::signals::execute_type_signals(tid, workspace_root, layer))
        }
        TrackCommand::TypeGraph {
            items_dir,
            track_id,
            workspace_root,
            layer,
            cluster_depth,
            edges,
        } => {
            let resolved =
                resolve_track_id_from_root(track_id, &workspace_root).map_err(CliError::Message);
            resolved.and_then(|tid| {
                tddd::graph::execute_type_graph(
                    items_dir,
                    tid,
                    workspace_root,
                    layer,
                    cluster_depth,
                    edges,
                )
            })
        }
        TrackCommand::BaselineGraph { items_dir, track_id, workspace_root, layers } => {
            let resolved = resolve_track_id_from_root_for_write(track_id, &workspace_root)
                .map_err(CliError::Message);
            resolved.and_then(|tid| {
                tddd::baseline_graph::execute_baseline_graph(items_dir, tid, workspace_root, layers)
            })
        }
        TrackCommand::ContractMap { items_dir, track_id, workspace_root, layers } => {
            let resolved = resolve_track_id_from_root_for_write(track_id, &workspace_root)
                .map_err(CliError::Message);
            resolved.and_then(|tid| {
                tddd::contract_map::execute_contract_map(items_dir, tid, workspace_root, layers)
            })
        }
        TrackCommand::SpecElementHash { items_dir, track_id, anchor } => {
            resolve_track_id(track_id, &items_dir).map_err(CliError::Message).and_then(|tid| {
                tddd::spec_element_hash::execute_spec_element_hash(items_dir, tid, anchor)
            })
        }
        TrackCommand::BaselineCapture { track_id, workspace_root, source_workspace, layer } => {
            let resolved = resolve_track_id_from_root_for_write(track_id, &workspace_root)
                .map_err(CliError::Message);
            resolved.and_then(|tid| {
                tddd::baseline::execute_baseline_capture(
                    tid,
                    workspace_root,
                    source_workspace,
                    layer,
                )
            })
        }
        TrackCommand::CatalogueSpecSignals { items_dir, track_id, workspace_root, layer } => {
            let resolved = resolve_track_id_from_root_for_write(track_id, &workspace_root)
                .map_err(CliError::Message);
            resolved.and_then(|tid| {
                tddd::catalogue_spec_signals::execute_catalogue_spec_signals(
                    items_dir,
                    tid,
                    workspace_root,
                    layer,
                )
            })
        }
        TrackCommand::Lint { track_id, layer_id, workspace_root, rules_file } => {
            resolve_track_id_from_root(track_id, &workspace_root)
                .map_err(CliError::Message)
                .and_then(|tid| tddd::lint::execute_lint(workspace_root, tid, layer_id, rules_file))
        }
        TrackCommand::CatalogueImplSignals { track_id, workspace_root, layer } => {
            let resolved =
                resolve_track_id_from_root(track_id, &workspace_root).map_err(CliError::Message);
            resolved.and_then(|tid| {
                tddd::catalogue_impl_signals::execute_catalogue_impl_signals(
                    tid,
                    workspace_root,
                    layer,
                )
            })
        }
        TrackCommand::SetCommitHash(args) => {
            resolve_track_id_from_root_for_write(args.track_id, &PathBuf::from("."))
                .map_err(CliError::Message)
                .and_then(set_commit_hash::execute_set_commit_hash)
        }
    }
}
