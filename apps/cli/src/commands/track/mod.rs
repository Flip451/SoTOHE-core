//! CLI subcommand for track operations using FsTrackStore.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Args, Subcommand};
use infrastructure::git_cli::{GitRepository, SystemGitRepo};
use infrastructure::track::fs_store::FsTrackStore;
use infrastructure::track::render;

mod branch_ops;
mod resolve;
mod signals;
mod state_ops;
pub(crate) mod tddd;
mod transition;
mod views;

/// Validates a track ID string (lowercase slug: `[a-z0-9]([a-z0-9-]*[a-z0-9])?`).
///
/// Mirrors the validation performed by `domain::TrackId::try_new` without
/// importing domain types.
///
/// # Errors
///
/// Returns an error string describing the failure.
pub(crate) fn validate_track_id_str(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("invalid track id: '{value}' (must not be empty)"));
    }
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => {
            return Err(format!(
                "invalid track id: '{value}' (must start with lowercase letter or digit)"
            ));
        }
    }
    let mut previous_was_hyphen = false;
    for ch in chars {
        let is_valid = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-';
        if !is_valid {
            return Err(format!("invalid track id: '{value}' (invalid character '{ch}')"));
        }
        if ch == '-' && previous_was_hyphen {
            return Err(format!("invalid track id: '{value}' (double hyphen not allowed)"));
        }
        previous_was_hyphen = ch == '-';
    }
    if previous_was_hyphen {
        return Err(format!("invalid track id: '{value}' (must not end with hyphen)"));
    }
    Ok(())
}

/// Validates a track branch name string (`track/<valid-track-id>`).
///
/// Mirrors the validation performed by `domain::TrackBranch::try_new` without
/// importing domain types.
///
/// # Errors
///
/// Returns an error string describing the failure.
pub(crate) fn validate_track_branch_str(value: &str) -> Result<(), String> {
    match value.strip_prefix("track/") {
        Some(slug) => validate_track_id_str(slug)
            .map_err(|_| format!("invalid track branch: '{value}' (slug part is invalid)")),
        None => Err(format!("invalid track branch: '{value}' (must be in 'track/<id>' form)")),
    }
}

pub(super) fn resolve_project_root(items_dir: &std::path::Path) -> Result<PathBuf, String> {
    let items_name = items_dir.file_name().and_then(|name| name.to_str());
    let track_dir = items_dir.parent();
    let track_name = track_dir.and_then(std::path::Path::file_name).and_then(|name| name.to_str());
    let project_root = track_dir.and_then(std::path::Path::parent);

    match (items_name, track_name, project_root) {
        (Some("items"), Some("track"), Some(root)) => {
            // When items_dir is a bare relative path like "track/items", Path::parent()
            // returns an empty path ("") rather than ".".  An empty path passed to
            // Command::current_dir causes ENOENT on spawn (e.g. in render.rs's git
            // branch discovery).  Normalise the empty root to "." so all callers get
            // a usable current-directory path, consistent with how relative joins
            // elsewhere in the render pipeline behave.
            if root.as_os_str().is_empty() {
                Ok(PathBuf::from("."))
            } else {
                Ok(root.to_path_buf())
            }
        }
        _ => Err(format!(
            "--items-dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        )),
    }
}

#[derive(Debug, Subcommand)]
pub enum TrackCommand {
    /// Transition a task to a new status (atomic read-modify-write).
    Transition {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long)]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Task ID (e.g., T1, T2).
        task_id: String,

        /// Target status: todo, in_progress, done, skipped.
        target_status: String,

        /// Commit hash (required when target_status is "done", optional).
        #[arg(long)]
        commit_hash: Option<String>,

        /// Skip branch validation (escape hatch for CI/testing).
        #[arg(long, default_value_t = false)]
        skip_branch_check: bool,
    },

    /// Create or switch to a track branch.
    Branch {
        #[command(subcommand)]
        action: BranchAction,
    },

    /// Resolve the current track phase, next command, and blocker.
    Resolve(ResolveArgs),

    /// Validate track metadata and/or regenerate rendered views from metadata.json.
    Views {
        #[command(subcommand)]
        action: ViewAction,
    },

    /// Add a new task to a track (atomic read-modify-write).
    AddTask {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Task description.
        description: String,

        /// Target section ID. If omitted, appends to the first section.
        #[arg(long)]
        section: Option<String>,

        /// Insert after this task ID within the section. If omitted or not found, appends to end.
        #[arg(long)]
        after: Option<String>,

        /// Skip branch validation (escape hatch for CI/testing).
        #[arg(long, default_value_t = false)]
        skip_branch_check: bool,
    },

    /// Set a status override on a track (blocked/cancelled).
    SetOverride {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Override status: blocked or cancelled.
        status: String,

        /// Reason for the override.
        #[arg(long, default_value = "")]
        reason: String,

        /// Skip branch validation.
        #[arg(long, default_value_t = false)]
        skip_branch_check: bool,
    },

    /// Clear a status override on a track.
    ClearOverride {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Skip branch validation.
        #[arg(long, default_value_t = false)]
        skip_branch_check: bool,
    },

    /// Show the next open task for a track (JSON output).
    NextTask {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,
    },

    /// Show task status counts for a track (JSON output).
    TaskCounts {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,
    },

    /// Evaluate spec.md source tags and store results in metadata.json spec_signals.
    Signals {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,
    },

    /// Evaluate domain type signals via rustdoc schema export and store results in domain-types.json.
    TypeSignals {
        /// Track ID (directory name under `workspace_root/track/items`).
        track_id: String,

        /// Workspace root directory (must contain `Cargo.toml`). Defaults to current directory.
        ///
        /// The track items directory is always derived as
        /// `<workspace_root>/track/items` inside the interactor.
        #[arg(long, default_value = ".")]
        workspace_root: PathBuf,

        /// Optional layer id filter. When omitted all `tddd.enabled` layers
        /// are processed in `architecture-rules.json` order. When supplied,
        /// the specified layer id must be `tddd.enabled=true`; targeting a
        /// disabled layer is fail-closed.
        #[arg(long)]
        layer: Option<String>,
    },

    /// Render a mermaid type graph from rustdoc schema export.
    ///
    /// When `--cluster-depth 0` writes a single flat `<layer>-graph.md` file.
    /// When `--cluster-depth N` (N ≥ 1, default 2) writes a cluster directory
    /// `<layer>-graph/` with `index.md` plus one file per cluster.
    TypeGraph {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Workspace root directory (must contain `Cargo.toml`). Defaults to current directory.
        #[arg(long, default_value = ".")]
        workspace_root: PathBuf,

        /// Optional layer id filter. When omitted all `tddd.enabled` layers
        /// are processed in `architecture-rules.json` order.
        #[arg(long)]
        layer: Option<String>,

        /// Cluster depth for directory layout.  0 = single flat file; ≥ 1 = cluster
        /// directory.  Defaults to `TypeGraphRenderOptions::default()` (currently 2).
        #[arg(long, default_value_t = 2)]
        cluster_depth: usize,

        /// Edge types to include.  Accepted values: methods, fields, impls, all.
        /// Defaults to `methods`.
        #[arg(long, default_value = "methods")]
        edges: String,
    },

    /// Render the rustdoc-input baseline graph (Reality View) for a track
    /// (ADR 2026-05-22-1507-baseline-graph-renderer-rustdoc-adaptation).
    ///
    /// Writes per-layer depth-1 `<layer>-graph-d1/index.md` (overview) and
    /// depth-2 `<layer>-graph-d2/<cluster>.md` (cluster detail) files under
    /// `track/items/<track-id>/`.
    ///
    /// Requires rustdoc JSON baselines captured by `sotp track baseline-capture`.
    /// Style config at `.harness/config/baseline-graph-style.toml` (fail-closed
    /// if absent or invalid, CN-02 / AC-15).
    BaselineGraph {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Workspace root directory (must contain `architecture-rules.json`).
        /// Defaults to current directory.
        #[arg(long, default_value = ".")]
        workspace_root: PathBuf,

        /// Optional comma-separated layer id filter (e.g.
        /// `domain,usecase`). When omitted every `tddd.enabled` layer is
        /// rendered. Unknown layer ids fail closed.
        #[arg(long)]
        layers: Option<String>,
    },

    /// Render the catalogue-input contract map for a track
    /// (ADR 2026-04-17-1528 §D1).
    ///
    /// Writes a single `track/items/<track-id>/contract-map.md` file
    /// containing a mermaid flowchart of every `tddd.enabled` layer's
    /// declared types, edges between method returns and declared types,
    /// and trait-impl edges from `SecondaryAdapter` entries.
    ContractMap {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Workspace root directory (must contain `architecture-rules.json`).
        /// Defaults to current directory.
        #[arg(long, default_value = ".")]
        workspace_root: PathBuf,

        /// Optional comma-separated layer id filter (e.g.
        /// `domain,usecase`). When omitted every `tddd.enabled` layer is
        /// rendered. Unknown layer ids fail closed.
        #[arg(long)]
        layers: Option<String>,
    },

    /// Regenerate `<layer>-catalogue-spec-signals.json` for each
    /// catalogue-spec-enabled layer (SoT Chain ② pre-commit step).
    ///
    /// Reads the LOCAL `<layer>-types.json` (not the origin blob) so
    /// uncommitted changes are reflected. Emits per-entry signals computed
    /// via the informal-priority rule (ADR D1.1) plus the raw-bytes SHA-256
    /// `catalogue_declaration_hash` used by the stale-detection gate.
    CatalogueSpecSignals {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Workspace root directory (must contain `architecture-rules.json`).
        /// Defaults to current directory.
        #[arg(long, default_value = ".")]
        workspace_root: PathBuf,

        /// Optional layer id filter. When omitted all `tddd.enabled` layers
        /// are processed. When supplied, the specified layer id must be
        /// `tddd.enabled=true`.
        #[arg(long)]
        layer: Option<String>,
    },

    /// Emit canonical SHA-256 hashes for spec.json elements (helper for
    /// catalogue Blue promotion: type-designer feeds the printed hash into
    /// `spec_refs[].hash` so `sotp verify catalogue-spec-refs` passes).
    ///
    /// When `--anchor <id>` is supplied, prints only that anchor's hash on
    /// stdout (single 64-hex line). When omitted, prints a JSON object
    /// mapping every element id to its hash, sorted by id.
    SpecElementHash {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Optional single-anchor lookup. When omitted, every element is emitted.
        #[arg(long)]
        anchor: Option<String>,
    },

    /// Capture the current TypeGraph as a baseline snapshot for TDDD reverse signal filtering.
    ///
    /// Idempotent by default: if the baseline file already exists it is kept as-is.
    /// Re-capturing the baseline after implementation has started pollutes the
    /// pre-implementation snapshot. Use `--force` only when explicitly migrating
    /// from an older baseline format (e.g. TypeBaseline JSON v2 → rustdoc JSON).
    ///
    /// `--source-workspace` lets you capture the API from a different Cargo
    /// workspace (e.g. a git worktree at `main`) while writing the baseline files
    /// into the current branch's track directory.
    BaselineCapture {
        /// Track ID (directory name under `workspace_root/track/items`).
        ///
        /// The track items directory is always derived as
        /// `<workspace_root>/track/items` inside the interactor.
        track_id: String,

        /// Workspace root directory used for architecture-rules.json resolution
        /// and the default rustdoc source. Defaults to current directory.
        #[arg(long, default_value = ".")]
        workspace_root: PathBuf,

        /// Optional source workspace for rustdoc export. When supplied,
        /// rustdoc is invoked from this workspace instead of `workspace_root`.
        /// Useful for capturing a baseline from a git worktree at `main`.
        #[arg(long)]
        source_workspace: Option<PathBuf>,

        /// Optional layer id filter. When omitted all `tddd.enabled` layers
        /// are processed in `architecture-rules.json` order. When supplied,
        /// the specified layer id must be `tddd.enabled=true`.
        #[arg(long)]
        layer: Option<String>,

        /// Overwrite existing baseline files. Use only when migrating from an
        /// older baseline format.
        #[arg(long)]
        force: bool,
    },

    /// Run catalogue lint rules against a layer catalogue and report violations.
    ///
    /// Wires `FsCatalogueLoader` + `InMemoryCatalogueLinter` +
    /// `RunCatalogueLintInteractor` at the composition root and runs a hardcoded
    /// demo rule set (ADR `tddd-struct-kind-uniformization-and-catalogue-linter`
    /// §S3 / IN-05 / AC-05).
    ///
    /// Exits with code 1 when any violations are found, 0 when none.
    Lint {
        /// Track ID (directory name under `track/items`).
        #[arg(long)]
        track_id: String,

        /// Layer ID to lint (e.g. `domain`, `usecase`, `infrastructure`).
        #[arg(long)]
        layer_id: String,

        /// Workspace root directory (must contain `architecture-rules.json`).
        /// Defaults to current directory.
        #[arg(long, default_value = ".")]
        workspace_root: PathBuf,
    },

    /// Diagnose SoT Chain ③ (catalogue ↔ implementation) for a track.
    ///
    /// A = ExtendedCrate built from `<layer>-types.json` via `CatalogueToExtendedCrateCodec`.
    /// B = `rustdoc_types::Crate` loaded from `<layer>-types-baseline.json` via
    ///     `BaselineRustdocCodec` (captured at pre-implementation main HEAD).
    /// C = `rustdoc_types::Crate` captured live via `cargo +nightly rustdoc`.
    ///
    /// Processes every `tddd.enabled` layer in `architecture-rules.json`.
    /// Outputs a markdown report to stdout (one table per layer).
    /// Exits with code 1 when any Red signals are found.
    ///
    /// On-demand diagnostic only — no output file, no Makefile wrapper
    /// (ADR 2026-05-11-2330 §D3).
    CatalogueImplSignals {
        /// Track ID (directory name under `track/items`).
        track_id: String,

        /// Workspace root directory (must contain `architecture-rules.json`).
        /// Defaults to current directory.
        ///
        /// The track items directory is derived from this path as
        /// `<workspace_root>/track/items` (canonical layout only).
        #[arg(long, default_value = ".")]
        workspace_root: PathBuf,

        /// Optional layer id filter. When omitted all `tddd.enabled` layers
        /// are processed. When supplied, the specified layer id must be
        /// `tddd.enabled=true`.
        #[arg(long)]
        layer: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum BranchAction {
    /// Create a new branch `track/<track-id>` from `main` and switch to it.
    Create(BranchArgs),

    /// Switch to an existing branch `track/<track-id>`.
    Switch(BranchArgs),
}

#[derive(Debug, Args, Clone)]
pub struct BranchArgs {
    /// Path to the track items root directory (e.g., `track/items`).
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID used to form the branch name `track/<track-id>`.
    track_id: String,
}

#[derive(Debug, Args, Clone)]
pub struct ResolveArgs {
    /// Path to the track items root directory (e.g., `track/items`).
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID. If omitted, auto-detects from the current git branch.
    track_id: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ViewAction {
    /// Validate metadata.json files under the repository.
    Validate {
        /// Project root containing `track/items` and `track/archive`.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },

    /// Render `plan.md` and `registry.md` from metadata.json.
    Sync {
        /// Project root containing `track/items` and `track/archive`.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,

        /// Sync only one active track's `plan.md`.
        #[arg(long)]
        track_id: Option<String>,
    },
}

#[allow(clippy::too_many_lines)]
pub fn execute(cmd: TrackCommand) -> ExitCode {
    use crate::CliError;

    let result: Result<ExitCode, CliError> = match cmd {
        TrackCommand::Transition {
            items_dir,
            track_id,
            task_id,
            target_status,
            commit_hash,
            skip_branch_check,
        } => transition::execute_transition(
            items_dir,
            track_id,
            task_id,
            target_status,
            commit_hash,
            skip_branch_check,
        ),
        TrackCommand::Branch { action } => branch_ops::execute_branch(action),
        TrackCommand::Resolve(args) => resolve::execute_resolve(args),
        TrackCommand::Views { action } => views::execute_views(action),
        TrackCommand::AddTask {
            items_dir,
            track_id,
            description,
            section,
            after,
            skip_branch_check,
        } => state_ops::execute_add_task(
            items_dir,
            track_id,
            description,
            section,
            after,
            skip_branch_check,
        ),
        TrackCommand::SetOverride { items_dir, track_id, status, reason, skip_branch_check } => {
            state_ops::execute_set_override(items_dir, track_id, status, reason, skip_branch_check)
        }
        TrackCommand::ClearOverride { items_dir, track_id, skip_branch_check } => {
            state_ops::execute_clear_override(items_dir, track_id, skip_branch_check)
        }
        TrackCommand::NextTask { items_dir, track_id } => {
            state_ops::execute_next_task(items_dir, track_id)
        }
        TrackCommand::TaskCounts { items_dir, track_id } => {
            state_ops::execute_task_counts(items_dir, track_id)
        }
        TrackCommand::Signals { items_dir, track_id } => {
            signals::execute_signals(items_dir, track_id)
        }
        TrackCommand::TypeSignals { track_id, workspace_root, layer } => {
            tddd::signals::execute_type_signals(track_id, workspace_root, layer)
        }
        TrackCommand::TypeGraph {
            items_dir,
            track_id,
            workspace_root,
            layer,
            cluster_depth,
            edges,
        } => tddd::graph::execute_type_graph(
            items_dir,
            track_id,
            workspace_root,
            layer,
            cluster_depth,
            edges,
        ),
        TrackCommand::BaselineGraph { items_dir, track_id, workspace_root, layers } => {
            tddd::baseline_graph::execute_baseline_graph(
                items_dir,
                track_id,
                workspace_root,
                layers,
            )
        }
        TrackCommand::ContractMap { items_dir, track_id, workspace_root, layers } => {
            tddd::contract_map::execute_contract_map(items_dir, track_id, workspace_root, layers)
        }
        TrackCommand::SpecElementHash { items_dir, track_id, anchor } => {
            tddd::spec_element_hash::execute_spec_element_hash(items_dir, track_id, anchor)
        }
        TrackCommand::BaselineCapture {
            track_id,
            workspace_root,
            source_workspace,
            layer,
            force,
        } => tddd::baseline::execute_baseline_capture(
            track_id,
            workspace_root,
            source_workspace,
            layer,
            force,
        ),
        TrackCommand::CatalogueSpecSignals { items_dir, track_id, workspace_root, layer } => {
            tddd::catalogue_spec_signals::execute_catalogue_spec_signals(
                items_dir,
                track_id,
                workspace_root,
                layer,
            )
        }
        TrackCommand::Lint { track_id, layer_id, workspace_root } => {
            tddd::lint::execute_lint(workspace_root, track_id, layer_id)
        }
        TrackCommand::CatalogueImplSignals { track_id, workspace_root, layer } => {
            tddd::catalogue_impl_signals::execute_catalogue_impl_signals(
                track_id,
                workspace_root,
                layer,
            )
        }
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err}");
            err.exit_code()
        }
    }
}
