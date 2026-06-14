//! CLI subcommand for track operations.

use std::path::PathBuf;

use clap::{Args, Subcommand};

mod archive;
mod branch_ops;
mod dispatch;
mod resolve;
pub(crate) mod set_commit_hash;
mod signals;
mod state_ops;
pub(crate) mod tddd;
mod transition;
mod validate;
mod views;

pub use dispatch::{execute, execute_with_error_chain};
pub(crate) use validate::{
    resolve_project_root, resolve_track_id, resolve_track_id_for_write, resolve_track_id_from_root,
    resolve_track_id_from_root_for_write, validate_track_branch_str, validate_track_id_str,
};

#[derive(Debug, Subcommand)]
pub enum TrackCommand {
    /// Archive a completed track: move it from `track/items/<id>/` to
    /// `track/archive/<id>/` via `git mv`, and additionally move any gitignored
    /// `logs/` subdirectory via a filesystem rename so that telemetry is
    /// preserved alongside the archived track (CN-03 / GO-03).
    Archive {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
        items_dir: std::path::PathBuf,

        /// Track ID (directory name under items_dir).
        /// When omitted, resolved from the current git branch (`track/<id>`).
        #[arg(long)]
        track_id: Option<String>,
    },

    /// Transition a task to a new status (atomic read-modify-write).
    Transition {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long, default_value = "track/items")]
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
    /// Always idempotent: if the baseline file already exists it is kept as-is.
    /// To re-capture, delete the baseline file first and then run this command again.
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
    },

    /// Persist the current HEAD SHA to `.commit_hash` for the active track (v2 diff base).
    ///
    /// Writes the HEAD SHA to `track/items/<track-id>/.commit_hash`.
    /// On failure, prints a recovery hint to stderr.
    ///
    /// The track ID is resolved from the current git branch when `--track-id` is omitted.
    SetCommitHash(SetCommitHashArgs),

    /// Run catalogue lint rules against a layer catalogue and report violations.
    ///
    /// Wires `FsCatalogueLoader` + `RunCatalogueLintInteractor` +
    /// `evaluate_catalogue_lint` at the composition root and runs a hardcoded
    /// demo rule set (ADR `knowledge/adr/2026-05-25-0000-tddd-pattern-semantics-extension.md`
    /// §D15 / D17).
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

impl TrackCommand {
    /// Returns the effective `items_dir` for this command as an owned `PathBuf`.
    ///
    /// Most variants carry an explicit `--items-dir` argument (defaulting to
    /// `"track/items"`); this method exposes it so callers such as the telemetry
    /// wiring in `main.rs` can use the actual value instead of a hardcoded
    /// fallback.
    ///
    /// Variants that derive their track-items path from `workspace_root` (i.e.
    /// `TypeSignals`, `BaselineCapture`, `Lint`, `CatalogueImplSignals`) return
    /// `<workspace_root>/track/items`, so that a non-default `--workspace-root`
    /// is honoured and telemetry is written to the correct track tree.
    ///
    /// Variants that derive their path from `project_root` (i.e. `Views`)
    /// return `<project_root>/track/items` for the same reason.
    ///
    /// `SetCommitHash` has no items-dir or workspace-root argument and returns
    /// the default `"track/items"` (relative to CWD), which matches its
    /// internal resolution behaviour.
    pub fn items_dir(&self) -> PathBuf {
        match self {
            TrackCommand::Archive { items_dir, .. }
            | TrackCommand::Transition { items_dir, .. }
            | TrackCommand::AddTask { items_dir, .. }
            | TrackCommand::SetOverride { items_dir, .. }
            | TrackCommand::ClearOverride { items_dir, .. }
            | TrackCommand::NextTask { items_dir, .. }
            | TrackCommand::TaskCounts { items_dir, .. }
            | TrackCommand::Signals { items_dir, .. }
            | TrackCommand::TypeGraph { items_dir, .. }
            | TrackCommand::BaselineGraph { items_dir, .. }
            | TrackCommand::ContractMap { items_dir, .. }
            | TrackCommand::CatalogueSpecSignals { items_dir, .. }
            | TrackCommand::SpecElementHash { items_dir, .. } => items_dir.clone(),

            // Branch sub-variants embed items_dir inside BranchArgs.
            TrackCommand::Branch { action: BranchAction::Create(args) }
            | TrackCommand::Branch { action: BranchAction::Switch(args) } => args.items_dir.clone(),

            // Resolve embeds items_dir inside ResolveArgs.
            TrackCommand::Resolve(args) => args.items_dir.clone(),

            // Views commands use project_root; derive items_dir from it so that
            // a non-default --project-root is honoured for telemetry path resolution.
            TrackCommand::Views { action: ViewAction::Validate { project_root } }
            | TrackCommand::Views { action: ViewAction::Sync { project_root, .. } } => {
                project_root.join("track").join("items")
            }

            // These variants derive their items path from workspace_root
            // internally; return <workspace_root>/track/items so that a
            // non-default --workspace-root is honoured for telemetry path resolution.
            TrackCommand::TypeSignals { workspace_root, .. }
            | TrackCommand::BaselineCapture { workspace_root, .. }
            | TrackCommand::Lint { workspace_root, .. }
            | TrackCommand::CatalogueImplSignals { workspace_root, .. } => {
                workspace_root.join("track").join("items")
            }

            // SetCommitHash has no items-dir or workspace-root argument.
            // Discover the git repository root so telemetry writes land under
            // `<repo-root>/track/items/<id>/logs/telemetry.jsonl` regardless of
            // whether the process is invoked from a subdirectory (e.g. during
            // tests that temporarily change the working directory via
            // `run_in_dir`).  Falls back to the CWD-relative "track/items" when
            // git discovery fails (no git repo, detached state, etc.).
            TrackCommand::SetCommitHash(_) => std::process::Command::new("git")
                .args(["rev-parse", "--show-toplevel"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| PathBuf::from(s.trim()).join("track").join("items"))
                .unwrap_or_else(|| PathBuf::from("track/items")),
        }
    }
}

/// Arguments for `sotp track set-commit-hash`.
#[derive(Debug, Args, Clone)]
pub struct SetCommitHashArgs {
    /// Track ID (directory name under `track/items`).
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub track_id: Option<String>,
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
pub(crate) mod test_support {
    use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
    use std::os::fd::AsRawFd as _;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Mutex;

    pub(crate) fn process_env_lock() -> &'static Mutex<()> {
        static LOCK: Mutex<()> = Mutex::new(());
        &LOCK
    }

    pub(crate) fn run_in_dir<T>(path: &Path, run: impl FnOnce() -> T) -> T {
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(path).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
        std::env::set_current_dir(original).unwrap();
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(crate) fn run_git(path: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(path)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .status()
            .unwrap();
        assert!(status.success(), "git {:?} failed with {status}", args);
    }

    pub(crate) fn seed_repo(path: &Path, branch: &str) {
        run_git(path, &["init", "-q"]);
        run_git(path, &["checkout", "-B", branch]);
        run_git(path, &["commit", "--allow-empty", "-m", "init", "--no-gpg-sign"]);
    }

    pub(crate) fn create_track_dir(path: &Path, track_id: &str) -> PathBuf {
        let track_dir = path.join("track").join("items").join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        track_dir
    }

    pub(crate) fn capture_stderr<T>(run: impl FnOnce() -> T) -> (T, String) {
        let mut capture = tempfile::tempfile().unwrap();
        let stderr_fd = std::io::stderr().as_raw_fd();
        let capture_fd = capture.as_raw_fd();
        std::io::stderr().flush().unwrap();

        // Safety: `stderr_fd` is a valid process file descriptor for stderr.
        let saved_fd = unsafe { libc::dup(stderr_fd) };
        assert!(saved_fd >= 0, "dup(stderr) failed");
        // Safety: both descriptors are valid; this redirects stderr to the temp file.
        let redirect_result = unsafe { libc::dup2(capture_fd, stderr_fd) };
        assert_eq!(redirect_result, stderr_fd, "dup2(capture, stderr) failed");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));

        std::io::stderr().flush().unwrap();
        // Safety: `saved_fd` was returned by `dup`; this restores stderr.
        let restore_result = unsafe { libc::dup2(saved_fd, stderr_fd) };
        assert_eq!(restore_result, stderr_fd, "dup2(saved, stderr) failed");
        // Safety: `saved_fd` is no longer needed after restoring stderr.
        let close_result = unsafe { libc::close(saved_fd) };
        assert_eq!(close_result, 0, "close(saved stderr) failed");

        capture.seek(SeekFrom::Start(0)).unwrap();
        let mut output = String::new();
        capture.read_to_string(&mut output).unwrap();

        match result {
            Ok(value) => (value, output),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::commands::track::test_support::{
        create_track_dir, process_env_lock, run_in_dir, seed_repo,
    };
    use std::process::ExitCode;
    use std::sync::Arc;
    use usecase::track_resolution::{
        ActiveTrackResolveError, ActiveTrackResolveInteractor, ActiveTrackResolveService as _,
        BranchReadError, BranchReaderPort, TrackResolutionError,
    };

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
                format!(
                    "WRITE operation requires the current branch to be the target track branch, \
                     but branch resolution failed: {e}\n\
                     Hint: switch to a track branch (track/<id>) before passing --track-id."
                )
            }
        }
    }

    fn resolve_track_id_with_branch_reader(
        explicit_id: Option<String>,
        branch_reader: Arc<dyn BranchReaderPort>,
    ) -> Result<String, String> {
        let interactor = ActiveTrackResolveInteractor::new(branch_reader);
        interactor.resolve_for_read(explicit_id).map_err(format_resolve_error)
    }

    // --- resolve_track_id ---

    /// AC-03 / CN-02: explicit track-id is returned as-is, regardless of branch or items_dir.
    #[test]
    fn test_resolve_track_id_with_explicit_value_returns_it_directly() {
        // items_dir only matters when explicit_id is None (git discovery is needed).
        // With an explicit id, it is short-circuited — any valid-structured path works.
        let dummy_items_dir = std::path::Path::new("track/items");
        let result = resolve_track_id(Some("my-feature-2026".to_owned()), dummy_items_dir);
        assert_eq!(result.unwrap(), "my-feature-2026");
    }

    /// items_dir anchor: invalid structure returns resolve_project_root error.
    ///
    /// When items_dir is not in the `<root>/track/items` form, resolve_track_id
    /// returns the error from resolve_project_root without attempting git discovery.
    /// This validates that the items_dir anchor flows through resolve_track_id_inner.
    #[test]
    fn test_resolve_track_id_none_with_invalid_items_dir_structure_returns_error() {
        let bad_items_dir = std::path::Path::new("not/a/track/items/structure");
        let result = resolve_track_id(None, bad_items_dir);
        assert!(result.is_err(), "expected Err for malformed items_dir, got: {result:?}");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("track/items"),
            "error should mention the expected items_dir form, got: {err_msg}"
        );
    }

    /// items_dir anchor: git discovery is anchored to items_dir, not CWD.
    ///
    /// This is the regression test for the bug fixed in PR #142: `resolve_track_id`
    /// previously discovered git from CWD even when the caller supplied a different
    /// `--items-dir`. The fix routes git discovery through `resolve_project_root(items_dir)`
    /// → `SystemGitRepo::discover_from(project_root)`.
    ///
    /// Proof: create a temp directory (not a git repo) with the required `track/items`
    /// sub-structure. Pass its `track/items` as `items_dir`. The error must reference
    /// the temp directory path, confirming that git discovery ran against the target
    /// tree rather than the process CWD (which IS a git repo and would have succeeded).
    #[test]
    fn test_resolve_track_id_none_anchors_discovery_to_items_dir_not_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("track").join("items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result = resolve_track_id(None, &items_dir);

        // The error must come from git discovery failing at the temp dir, not CWD.
        // If CWD were used, the call would succeed (the process runs inside a git repo).
        assert!(
            result.is_err(),
            "expected Err because temp dir is not a git repo, got: {result:?}"
        );
        let err_msg = result.unwrap_err();
        let expected_root = tmp.path().to_string_lossy();
        assert!(
            err_msg.contains(expected_root.as_ref()),
            "error must reference the target tree root '{}' (not CWD), got: {err_msg}",
            expected_root,
        );
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

    #[test]
    fn test_execute_set_commit_hash_without_track_id_resolves_branch_and_writes_hash() {
        let _guard = process_env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        seed_repo(dir.path(), "track/my-track-2026");
        let track_dir = create_track_dir(dir.path(), "my-track-2026");

        let exit = run_in_dir(dir.path(), || {
            execute(TrackCommand::SetCommitHash(SetCommitHashArgs { track_id: None }))
        });

        assert_eq!(exit, ExitCode::SUCCESS);
        let written = std::fs::read_to_string(track_dir.join(".commit_hash")).unwrap();
        assert_eq!(written.trim().len(), 40, "written SHA must be 40 hex chars");
    }

    #[test]
    fn test_execute_set_commit_hash_from_subdir_resolves_repo_root_and_writes_hash() {
        let _guard = process_env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        seed_repo(dir.path(), "track/my-track-2026");
        let track_dir = create_track_dir(dir.path(), "my-track-2026");
        let subdir = dir.path().join("nested").join("workdir");
        std::fs::create_dir_all(&subdir).unwrap();

        let exit = run_in_dir(&subdir, || {
            execute(TrackCommand::SetCommitHash(SetCommitHashArgs { track_id: None }))
        });

        assert_eq!(exit, ExitCode::SUCCESS);
        let written = std::fs::read_to_string(track_dir.join(".commit_hash")).unwrap();
        assert_eq!(written.trim().len(), 40, "written SHA must be 40 hex chars");
    }

    #[test]
    fn test_execute_set_commit_hash_with_mismatched_track_id_fails_closed_before_write() {
        let _guard = process_env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        seed_repo(dir.path(), "track/my-track-2026");
        create_track_dir(dir.path(), "my-track-2026");
        let other_track_dir = create_track_dir(dir.path(), "other-track-2026");

        let exit = run_in_dir(dir.path(), || {
            execute(TrackCommand::SetCommitHash(SetCommitHashArgs {
                track_id: Some("other-track-2026".to_owned()),
            }))
        });

        assert_eq!(exit, ExitCode::FAILURE);
        assert!(
            !other_track_dir.join(".commit_hash").exists(),
            "branch mismatch must fail before writing the explicit track's .commit_hash"
        );
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

    // ── TrackCommand::items_dir accessor ─────────────────────────────────────

    /// Variants carrying an explicit items_dir field return it via `items_dir()`.
    #[test]
    fn test_items_dir_accessor_returns_explicit_items_dir_for_transition() {
        let custom = PathBuf::from("custom/items");
        let cmd = TrackCommand::Transition {
            items_dir: custom.clone(),
            track_id: None,
            task_id: "T001".to_owned(),
            target_status: "done".to_owned(),
            commit_hash: None,
        };
        assert_eq!(cmd.items_dir(), custom.as_path());
    }

    /// Variant `Transition` with default items_dir returns `"track/items"`.
    #[test]
    fn test_items_dir_accessor_returns_default_for_transition_with_default_path() {
        let cmd = TrackCommand::Transition {
            items_dir: PathBuf::from("track/items"),
            track_id: None,
            task_id: "T001".to_owned(),
            target_status: "done".to_owned(),
            commit_hash: None,
        };
        assert_eq!(cmd.items_dir(), std::path::Path::new("track/items"));
    }

    /// TypeSignals derives items_dir from workspace_root: default workspace_root "."
    /// yields "./track/items".
    #[test]
    fn test_items_dir_accessor_returns_workspace_root_derived_path_for_type_signals() {
        let cmd = TrackCommand::TypeSignals {
            track_id: None,
            workspace_root: PathBuf::from("."),
            layer: None,
        };
        assert_eq!(cmd.items_dir(), PathBuf::from(".").join("track").join("items"));
    }

    /// TypeSignals with a non-default workspace_root derives items_dir correctly.
    #[test]
    fn test_items_dir_accessor_returns_non_default_workspace_root_for_type_signals() {
        let cmd = TrackCommand::TypeSignals {
            track_id: None,
            workspace_root: PathBuf::from("/custom/root"),
            layer: None,
        };
        assert_eq!(cmd.items_dir(), PathBuf::from("/custom/root/track/items"));
    }

    /// SetCommitHash has no items_dir argument; accessor discovers the git repo root and
    /// returns `<repo-root>/track/items`.  When tests run inside the workspace git repo,
    /// the returned path must end with `track/items` (the canonical segment pair) and be
    /// absolute, proving that git discovery succeeded and CWD-relative fallback was not used.
    #[test]
    fn test_items_dir_accessor_returns_absolute_repo_root_for_set_commit_hash() {
        let cmd = TrackCommand::SetCommitHash(SetCommitHashArgs { track_id: None });
        let dir = cmd.items_dir();
        // The path must end with `<anything>/track/items`.
        // Use the PathBuf join-based expectation: suffix `track/items` must match.
        let ends_with_track_items = dir.ends_with("track/items");
        assert!(ends_with_track_items, "items_dir must end with 'track/items', got: {dir:?}");
        // When the process runs inside a git repository, the path must be absolute
        // (git discovery succeeded and provided an absolute root).
        assert!(
            dir.is_absolute(),
            "items_dir for SetCommitHash must be absolute when inside a git repo, got: {dir:?}"
        );
    }

    /// NextTask custom items_dir is propagated correctly by the accessor.
    #[test]
    fn test_items_dir_accessor_returns_custom_items_dir_for_next_task() {
        let custom = PathBuf::from("alt/track/items");
        let cmd = TrackCommand::NextTask { items_dir: custom.clone(), track_id: None };
        assert_eq!(cmd.items_dir(), custom.as_path());
    }
}
