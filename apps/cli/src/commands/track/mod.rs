//! CLI subcommand for track operations using FsTrackStore.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Args, Subcommand};
use infrastructure::git_cli::SystemGitRepo;
use infrastructure::track::fs_store::FsTrackStore;
use infrastructure::track::render;
use usecase::track_resolution::{
    ActiveTrackResolveError, ActiveTrackResolveInteractor, ActiveTrackResolveService as _,
    TrackResolutionError,
};

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

/// Resolves a track ID from an explicit value or from the current git branch.
///
/// Thin wrapper around [`ActiveTrackResolveInteractor::resolve_for_read`].
/// When `explicit_id` is `Some`, returns it directly (CN-02: explicit value
/// takes priority, no branch validation — git discovery is skipped). When
/// `None`, self-resolves from the current git branch via `SystemGitRepo` (D2).
/// Fail-closed on non-track branches: returns an error with a hint to provide
/// an explicit track-id (CN-01, AC-01, AC-02, AC-19).
///
/// # Errors
///
/// Returns a human-readable error string when the track-id cannot be resolved.
pub(crate) fn resolve_track_id(explicit_id: Option<String>) -> Result<String, String> {
    // Short-circuit for explicit id: skip git discovery entirely (CN-02).
    if let Some(id) = explicit_id {
        return Ok(id);
    }
    let repo =
        SystemGitRepo::discover().map_err(|e| format!("cannot discover git repository: {e}"))?;
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
    interactor.resolve_for_read(None).map_err(format_resolve_error)
}

/// Resolves a track ID using git discovery rooted at a workspace path.
///
/// Thin wrapper around [`ActiveTrackResolveInteractor::resolve_for_read`]
/// anchored to `workspace_root`. This is for commands whose target tree is
/// selected by `--workspace-root`. The omitted track-id path must read the
/// branch from that same workspace rather than from the process current
/// directory.
///
/// When `explicit_id` is `Some`, git discovery is skipped entirely (CN-02).
pub(crate) fn resolve_track_id_from_root(
    explicit_id: Option<String>,
    workspace_root: &Path,
) -> Result<String, String> {
    // Short-circuit for explicit id: skip git discovery entirely (CN-02).
    if let Some(id) = explicit_id {
        return Ok(id);
    }
    let repo = SystemGitRepo::discover_from(workspace_root).map_err(|e| {
        format!("cannot discover git repository from {}: {e}", workspace_root.display())
    })?;
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
    interactor.resolve_for_read(None).map_err(format_resolve_error)
}

#[cfg(test)]
fn resolve_track_id_with_branch_reader(
    explicit_id: Option<String>,
    branch_reader: Arc<dyn usecase::track_resolution::BranchReaderPort>,
) -> Result<String, String> {
    // Delegate the full READ semantics (including the Some short-circuit) to the
    // interactor — no git discovery is needed in tests since the stub reader is
    // injected directly.
    let interactor = ActiveTrackResolveInteractor::new(branch_reader);
    interactor.resolve_for_read(explicit_id).map_err(format_resolve_error)
}

/// Resolves a track ID for a WRITE operation from an explicit value or the
/// current git branch, with fail-closed validation when an explicit id is given.
///
/// Thin wrapper around [`ActiveTrackResolveInteractor::resolve_for_write`]
/// (WRITE semantics, D7, AC-18, CN-02, CN-03):
/// - When `explicit_id` is `None`: self-resolves from the git branch rooted at
///   the repository that owns `items_dir`. Fail-closed on non-track branches.
/// - When `explicit_id` is `Some(id)`: reads the current branch via
///   `ActiveTrackResolveInteractor` from the repo owning `items_dir`, and
///   compares the branch-derived id with the explicit id. Returns the explicit
///   id only when they match; otherwise returns a fail-closed error explaining
///   the mismatch.
///
/// Git discovery is anchored to the repository that owns `items_dir` (derived
/// via `resolve_project_root`) rather than the process CWD. This ensures the
/// WRITE guard validates the correct repo even when `--items-dir` points to an
/// absolute path in a different working directory.
///
/// This prevents accidentally writing to a different track than the one the
/// developer is working on (AC-18). READ operations keep the existing
/// `resolve_track_id` override semantics (AC-19).
///
/// # Errors
///
/// Returns a human-readable error string when:
/// - `items_dir` is not in the `<root>/track/items` form.
/// - `explicit_id` is `Some` and the current branch is not a track branch.
/// - `explicit_id` is `Some` and the branch-derived id does not equal it.
/// - `explicit_id` is `None` and the current branch is not a track branch.
pub(crate) fn resolve_track_id_for_write(
    explicit_id: Option<String>,
    items_dir: &std::path::Path,
) -> Result<String, String> {
    let project_root = resolve_project_root(items_dir)?;
    let repo = SystemGitRepo::discover_from(&project_root).map_err(|e| {
        format!("cannot discover git repository from {}: {e}", project_root.display())
    })?;
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
    interactor.resolve_for_write(explicit_id).map_err(|e| format_write_error(&e))
}

/// Formats an [`ActiveTrackResolveError`] into a user-facing CLI error string
/// with a hint for READ operations (non-WRITE path, AC-19).
fn format_resolve_error(e: ActiveTrackResolveError) -> String {
    match &e {
        ActiveTrackResolveError::Resolution(TrackResolutionError::DetachedHead) => {
            format!(
                "{e}\nHint: provide an explicit track-id argument, or switch to a track branch (track/<id>) first."
            )
        }
        ActiveTrackResolveError::BranchRead(_) | ActiveTrackResolveError::Resolution(_) => {
            format!(
                "{e}\nHint: provide an explicit track-id argument, or switch to a track branch (track/<id>) first."
            )
        }
        ActiveTrackResolveError::BranchMismatch { explicit_id, branch_id } => {
            format!(
                "WRITE operation rejected: explicit --track-id '{explicit_id}' does not match \
                 the current branch-derived track id '{branch_id}'.\n\
                 Hint: switch to branch 'track/{explicit_id}' first, or omit --track-id to \
                 operate on the current branch's track."
            )
        }
    }
}

/// Formats an [`ActiveTrackResolveError`] into a user-facing CLI error string
/// for WRITE operations, preserving the existing hint messages (AC-18, D7).
fn format_write_error(e: &ActiveTrackResolveError) -> String {
    match e {
        ActiveTrackResolveError::BranchMismatch { explicit_id, branch_id } => {
            format!(
                "WRITE operation rejected: explicit --track-id '{explicit_id}' does not match \
                 the current branch-derived track id '{branch_id}'.\n\
                 Hint: switch to branch 'track/{explicit_id}' first, or omit --track-id to \
                 operate on the current branch's track."
            )
        }
        ActiveTrackResolveError::Resolution(_) | ActiveTrackResolveError::BranchRead(_) => {
            // For WRITE, a branch resolution failure means we cannot validate the explicit id.
            format!(
                "WRITE operation requires the current branch to be the target track branch, \
                 but branch resolution failed: {e}\n\
                 Hint: switch to a track branch (track/<id>) before passing --track-id."
            )
        }
    }
}

/// Resolves a track ID for a WRITE operation from an explicit value or the
/// current git branch, anchored to a workspace root path.
///
/// Thin wrapper around [`ActiveTrackResolveInteractor::resolve_for_write`]
/// (WRITE semantics, D7, AC-18, CN-02, CN-03) for commands that use
/// `--workspace-root` instead of `--items-dir`:
/// - When `explicit_id` is `None`: self-resolves from the git branch of the
///   repository discovered from `workspace_root`. Fail-closed on non-track branches.
/// - When `explicit_id` is `Some(id)`: reads the current branch via
///   `ActiveTrackResolveInteractor` from the repo discovered at `workspace_root`,
///   and compares the branch-derived id with the explicit id. Returns the explicit
///   id only when they match; otherwise returns a fail-closed error explaining
///   the mismatch.
///
/// This prevents accidentally writing to a different track than the one the
/// developer is working on (AC-18). This is the workspace-root counterpart of
/// [`resolve_track_id_for_write`], which is anchored to `items_dir`.
///
/// # Errors
///
/// Returns a human-readable error string when:
/// - `workspace_root` is not inside a git repository.
/// - `explicit_id` is `Some` and the current branch is not a track branch.
/// - `explicit_id` is `Some` and the branch-derived id does not equal it.
/// - `explicit_id` is `None` and the current branch is not a track branch.
pub(crate) fn resolve_track_id_from_root_for_write(
    explicit_id: Option<String>,
    workspace_root: &Path,
) -> Result<String, String> {
    let repo = SystemGitRepo::discover_from(workspace_root).map_err(|e| {
        format!("cannot discover git repository from {}: {e}", workspace_root.display())
    })?;
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
    interactor.resolve_for_write(explicit_id).map_err(|e| format_write_error(&e))
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
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,

        /// Task ID (e.g., T1, T2).
        task_id: String,

        /// Target status: todo, in_progress, done, skipped.
        target_status: String,

        /// Commit hash (required when target_status is "done", optional).
        #[arg(long)]
        commit_hash: Option<String>,
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
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,

        /// Task description.
        description: String,

        /// Target section ID. If omitted, appends to the first section.
        #[arg(long)]
        section: Option<String>,

        /// Insert after this task ID within the section. If omitted or not found, appends to end.
        #[arg(long)]
        after: Option<String>,
    },

    /// Set a status override on a track (blocked/cancelled).
    SetOverride {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,

        /// Override status: blocked or cancelled.
        status: String,

        /// Reason for the override.
        #[arg(long, default_value = "")]
        reason: String,
    },

    /// Clear a status override on a track.
    ClearOverride {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,
    },

    /// Show the next open task for a track (JSON output).
    NextTask {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,
    },

    /// Show task status counts for a track (JSON output).
    TaskCounts {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,
    },

    /// Evaluate spec.md source tags and store results in metadata.json spec_signals.
    Signals {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,
    },

    /// Evaluate domain type signals via rustdoc schema export and store results in domain-types.json.
    TypeSignals {
        /// Track ID (directory name under `workspace_root/track/items`).
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,

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
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,

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
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,

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
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,

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
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,

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
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,

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
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,

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
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,

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
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,

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
        TrackCommand::NextTask { items_dir, track_id } => resolve_track_id(track_id)
            .map_err(CliError::Message)
            .and_then(|tid| state_ops::execute_next_task(items_dir, tid)),
        TrackCommand::TaskCounts { items_dir, track_id } => resolve_track_id(track_id)
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
            resolve_track_id(track_id).map_err(CliError::Message).and_then(|tid| {
                tddd::spec_element_hash::execute_spec_element_hash(items_dir, tid, anchor)
            })
        }
        TrackCommand::BaselineCapture {
            track_id,
            workspace_root,
            source_workspace,
            layer,
            force,
        } => {
            let resolved = resolve_track_id_from_root_for_write(track_id, &workspace_root)
                .map_err(CliError::Message);
            resolved.and_then(|tid| {
                tddd::baseline::execute_baseline_capture(
                    tid,
                    workspace_root,
                    source_workspace,
                    layer,
                    force,
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
        TrackCommand::Lint { track_id, layer_id, workspace_root } => {
            resolve_track_id_from_root(track_id, &workspace_root)
                .map_err(CliError::Message)
                .and_then(|tid| tddd::lint::execute_lint(workspace_root, tid, layer_id))
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
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err}");
            err.exit_code()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use usecase::track_resolution::{BranchReadError, BranchReaderPort};

    #[derive(Debug)]
    struct StubBranchReader {
        branch: Option<String>,
    }

    impl StubBranchReader {
        fn new(branch: Option<&str>) -> Self {
            Self { branch: branch.map(str::to_owned) }
        }
    }

    impl BranchReaderPort for StubBranchReader {
        fn current_branch(&self) -> Result<Option<String>, BranchReadError> {
            Ok(self.branch.clone())
        }
    }

    fn branch_reader(branch: Option<&str>) -> Arc<dyn BranchReaderPort> {
        Arc::new(StubBranchReader::new(branch))
    }

    // --- resolve_track_id ---

    /// AC-03 / CN-02: explicit track-id is returned as-is, regardless of branch.
    #[test]
    fn test_resolve_track_id_with_explicit_value_returns_it_directly() {
        let result = resolve_track_id(Some("my-feature-2026".to_owned()));
        assert_eq!(result.unwrap(), "my-feature-2026");
    }

    /// CN-02: explicit track-id priority is preserved even when on a track branch.
    #[test]
    fn test_resolve_track_id_explicit_value_takes_priority_over_branch() {
        let result = resolve_track_id_with_branch_reader(
            Some("explicit-id".to_owned()),
            branch_reader(Some("track/branch-id")),
        );
        assert_eq!(result.unwrap(), "explicit-id");
    }

    /// AC-01 / CN-01: when track_id is None and on a track branch, the branch
    /// suffix is returned.
    #[test]
    fn test_resolve_track_id_none_on_track_branch_returns_branch_id() {
        let result = resolve_track_id_with_branch_reader(
            None,
            branch_reader(Some("track/active-track-2026")),
        );
        assert!(result.is_ok(), "expected Ok on track branch, got: {result:?}");
        assert_eq!(result.unwrap(), "active-track-2026");
    }

    /// AC-02 / CN-01: when track_id is None and on a non-track branch (e.g. main),
    /// an error is returned with a hint to provide an explicit track-id.
    #[test]
    fn test_resolve_track_id_none_on_non_track_branch_returns_error_with_hint() {
        let result = resolve_track_id_with_branch_reader(None, branch_reader(Some("main")));
        assert!(result.is_err(), "expected Err on non-track branch");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("Hint:") || err_msg.contains("provide an explicit track-id"),
            "error must prompt user to provide explicit track-id, got: {err_msg}"
        );
    }

    /// AC-02 / CN-01: detached HEAD is also fail-closed with the explicit-id hint.
    #[test]
    fn test_resolve_track_id_none_on_detached_head_returns_error_with_hint() {
        let result = resolve_track_id_with_branch_reader(None, branch_reader(Some("HEAD")));
        assert!(result.is_err(), "expected Err on detached HEAD");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("detached HEAD") && err_msg.contains("provide an explicit track-id"),
            "error must mention detached HEAD and explicit track-id hint, got: {err_msg}"
        );
    }

    // ── resolve_track_id_for_write ────────────────────────────────────────────

    /// Helper for resolve_track_id_for_write that injects a stub branch reader
    /// instead of discovering a real git repo, enabling unit testing without a
    /// git repository.
    ///
    /// Now delegates to [`ActiveTrackResolveInteractor::resolve_for_write`] and
    /// maps the error via [`format_write_error`] — mirroring the production wrapper.
    fn resolve_track_id_for_write_with_reader(
        explicit_id: Option<String>,
        branch_reader: Arc<dyn BranchReaderPort>,
    ) -> Result<String, String> {
        let interactor = ActiveTrackResolveInteractor::new(branch_reader);
        interactor.resolve_for_write(explicit_id).map_err(|e| format_write_error(&e))
    }

    /// AC-18 / D7: explicit id matches the branch-derived id → returns the id.
    #[test]
    fn test_resolve_track_id_for_write_matching_explicit_and_branch_returns_id() {
        let result = resolve_track_id_for_write_with_reader(
            Some("my-track-2026".to_owned()),
            branch_reader(Some("track/my-track-2026")),
        );
        assert_eq!(result.unwrap(), "my-track-2026");
    }

    /// AC-18 / D7: explicit id does not match the branch-derived id → fail-closed error.
    #[test]
    fn test_resolve_track_id_for_write_mismatched_explicit_and_branch_returns_error() {
        let result = resolve_track_id_for_write_with_reader(
            Some("other-track-2026".to_owned()),
            branch_reader(Some("track/my-track-2026")),
        );
        assert!(result.is_err(), "expected Err on id mismatch");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("WRITE operation rejected") && err_msg.contains("other-track-2026"),
            "error must explain the mismatch, got: {err_msg}"
        );
    }

    /// AC-18 / D7: non-track branch with explicit id → fail-closed error.
    #[test]
    fn test_resolve_track_id_for_write_non_track_branch_with_explicit_id_returns_error() {
        let result = resolve_track_id_for_write_with_reader(
            Some("my-track-2026".to_owned()),
            branch_reader(Some("main")),
        );
        assert!(result.is_err(), "expected Err on non-track branch");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("WRITE operation") || err_msg.contains("branch"),
            "error must reference the branch issue, got: {err_msg}"
        );
    }

    /// AC-18 / D7: None explicit id on track branch → self-resolves to branch id.
    #[test]
    fn test_resolve_track_id_for_write_none_on_track_branch_self_resolves() {
        let result = resolve_track_id_for_write_with_reader(
            None,
            branch_reader(Some("track/my-track-2026")),
        );
        assert_eq!(result.unwrap(), "my-track-2026");
    }

    /// AC-18 / D7: None explicit id on non-track branch → fail-closed error.
    #[test]
    fn test_resolve_track_id_for_write_none_on_non_track_branch_returns_error() {
        let result = resolve_track_id_for_write_with_reader(None, branch_reader(Some("main")));
        assert!(result.is_err(), "expected Err on non-track branch with no explicit id");
    }

    // ── resolve_track_id_from_root_for_write ─────────────────────────────────

    /// Helper for resolve_track_id_from_root_for_write that injects a stub branch
    /// reader instead of discovering a real git repo, enabling unit testing without
    /// a git repository.
    ///
    /// Delegates to [`ActiveTrackResolveInteractor::resolve_for_write`] and maps
    /// the error via [`format_write_error`] — mirroring the production wrapper.
    fn resolve_track_id_from_root_for_write_with_reader(
        explicit_id: Option<String>,
        branch_reader: Arc<dyn BranchReaderPort>,
    ) -> Result<String, String> {
        let interactor = ActiveTrackResolveInteractor::new(branch_reader);
        interactor.resolve_for_write(explicit_id).map_err(|e| format_write_error(&e))
    }

    /// AC-18 / D7: explicit id matches the branch-derived id → returns the id.
    #[test]
    fn test_resolve_track_id_from_root_for_write_matching_explicit_and_branch_returns_id() {
        let result = resolve_track_id_from_root_for_write_with_reader(
            Some("my-track-2026".to_owned()),
            branch_reader(Some("track/my-track-2026")),
        );
        assert_eq!(result.unwrap(), "my-track-2026");
    }

    /// AC-18 / D7: explicit id does not match the branch-derived id → fail-closed error.
    #[test]
    fn test_resolve_track_id_from_root_for_write_mismatched_explicit_and_branch_returns_error() {
        let result = resolve_track_id_from_root_for_write_with_reader(
            Some("other-track-2026".to_owned()),
            branch_reader(Some("track/my-track-2026")),
        );
        assert!(result.is_err(), "expected Err on id mismatch");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("WRITE operation rejected") && err_msg.contains("other-track-2026"),
            "error must explain the mismatch, got: {err_msg}"
        );
    }

    /// AC-18 / D7: non-track branch with explicit id → fail-closed error.
    #[test]
    fn test_resolve_track_id_from_root_for_write_non_track_branch_with_explicit_id_returns_error() {
        let result = resolve_track_id_from_root_for_write_with_reader(
            Some("my-track-2026".to_owned()),
            branch_reader(Some("main")),
        );
        assert!(result.is_err(), "expected Err on non-track branch");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("WRITE operation") || err_msg.contains("branch"),
            "error must reference the branch issue, got: {err_msg}"
        );
    }

    /// AC-18 / D7: None explicit id on track branch → self-resolves to branch id.
    #[test]
    fn test_resolve_track_id_from_root_for_write_none_on_track_branch_self_resolves() {
        let result = resolve_track_id_from_root_for_write_with_reader(
            None,
            branch_reader(Some("track/my-track-2026")),
        );
        assert_eq!(result.unwrap(), "my-track-2026");
    }

    /// AC-18 / D7: None explicit id on non-track branch → fail-closed error.
    #[test]
    fn test_resolve_track_id_from_root_for_write_none_on_non_track_branch_returns_error() {
        let result =
            resolve_track_id_from_root_for_write_with_reader(None, branch_reader(Some("main")));
        assert!(result.is_err(), "expected Err on non-track branch with no explicit id");
    }
}
