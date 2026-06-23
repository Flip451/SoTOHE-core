//! Auxiliary application service ports for the `review` command family.
//!
//! Covers operations that do not fit the primary `RunReviewService` /
//! `RunReviewFixService` / `ReviewCheckApprovedService` pattern:
//! - `ReviewResultsService` — render review results output string
//! - `ReviewValidateScopeService` — validate a scope name
//! - `ReviewGetBriefingService` — get briefing path for a scope
//! - `ReviewRunLocalService` — run the provider-auto-resolved reviewer
//!
//! All interactors use the function-pointer pattern (mirroring `RunReviewInteractor`)
//! so that `cli_composition` injects the infrastructure wiring without violating
//! the hexagonal boundary.

use std::path::PathBuf;
use std::sync::Arc;

// ── ReviewClassifyService ─────────────────────────────────────────────────────

/// Application service (primary port) for `sotp review classify`.
pub trait ReviewClassifyService: Send + Sync {
    /// Classify each path string into its review scope(s).
    ///
    /// Returns one `(path, scopes_csv)` pair per input path.
    fn classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<(String, String)>, String>;
}

/// Function-pointer interactor implementing [`ReviewClassifyService`].
pub struct ReviewClassifyInteractor {
    #[allow(clippy::type_complexity)]
    run_fn: Arc<
        dyn Fn(Vec<String>, Option<String>, PathBuf) -> Result<Vec<(String, String)>, String>
            + Send
            + Sync,
    >,
}

impl ReviewClassifyInteractor {
    /// Create with injected function.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn new(
        run_fn: Arc<
            dyn Fn(Vec<String>, Option<String>, PathBuf) -> Result<Vec<(String, String)>, String>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl ReviewClassifyService for ReviewClassifyInteractor {
    fn classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<(String, String)>, String> {
        (self.run_fn)(paths, track_id, items_dir)
    }
}

// ── ReviewFilesService ────────────────────────────────────────────────────────

/// Application service (primary port) for `sotp review files`.
pub trait ReviewFilesService: Send + Sync {
    /// List the diff files belonging to the given scope (one per entry).
    fn files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<String>, String>;
}

/// Function-pointer interactor implementing [`ReviewFilesService`].
pub struct ReviewFilesInteractor {
    #[allow(clippy::type_complexity)]
    run_fn:
        Arc<dyn Fn(String, Option<String>, PathBuf) -> Result<Vec<String>, String> + Send + Sync>,
}

impl ReviewFilesInteractor {
    /// Create with injected function.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn new(
        run_fn: Arc<
            dyn Fn(String, Option<String>, PathBuf) -> Result<Vec<String>, String> + Send + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl ReviewFilesService for ReviewFilesInteractor {
    fn files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<String>, String> {
        (self.run_fn)(scope, track_id, items_dir)
    }
}

// ── ReviewResultsService ──────────────────────────────────────────────────────

/// Application service (primary port) for `sotp review results`.
pub trait ReviewResultsService: Send + Sync {
    /// Render review results output.
    ///
    /// Returns the rendered output string or an error message.
    #[allow(clippy::too_many_arguments)]
    fn results(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
        scope: Option<String>,
        all: bool,
        limit: u32,
        round_type: String,
        no_hint: bool,
    ) -> Result<String, String>;
}

/// Function-pointer interactor implementing [`ReviewResultsService`].
pub struct ReviewResultsInteractor {
    #[allow(clippy::type_complexity)]
    run_fn: Arc<
        dyn Fn(
                Option<String>,
                PathBuf,
                Option<String>,
                bool,
                u32,
                String,
                bool,
            ) -> Result<String, String>
            + Send
            + Sync,
    >,
}

impl ReviewResultsInteractor {
    /// Create with injected function.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn new(
        run_fn: Arc<
            dyn Fn(
                    Option<String>,
                    PathBuf,
                    Option<String>,
                    bool,
                    u32,
                    String,
                    bool,
                ) -> Result<String, String>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl ReviewResultsService for ReviewResultsInteractor {
    #[allow(clippy::too_many_arguments)]
    fn results(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
        scope: Option<String>,
        all: bool,
        limit: u32,
        round_type: String,
        no_hint: bool,
    ) -> Result<String, String> {
        (self.run_fn)(track_id, items_dir, scope, all, limit, round_type, no_hint)
    }
}

// ── ReviewValidateScopeService ────────────────────────────────────────────────

/// Application service (primary port) for `sotp review validate-scope`.
pub trait ReviewValidateScopeService: Send + Sync {
    /// Validate a scope name for the given track.
    ///
    /// Returns `Ok(())` on success or an error message string on failure.
    fn validate_scope(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<(), String>;
}

/// Function-pointer interactor implementing [`ReviewValidateScopeService`].
pub struct ReviewValidateScopeInteractor {
    #[allow(clippy::type_complexity)]
    run_fn: Arc<dyn Fn(String, Option<String>, PathBuf) -> Result<(), String> + Send + Sync>,
}

impl ReviewValidateScopeInteractor {
    /// Create with injected function.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn new(
        run_fn: Arc<dyn Fn(String, Option<String>, PathBuf) -> Result<(), String> + Send + Sync>,
    ) -> Self {
        Self { run_fn }
    }
}

impl ReviewValidateScopeService for ReviewValidateScopeInteractor {
    fn validate_scope(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<(), String> {
        (self.run_fn)(scope, track_id, items_dir)
    }
}

// ── ReviewGetBriefingService ──────────────────────────────────────────────────

/// Application service (primary port) for `sotp review get-briefing`.
pub trait ReviewGetBriefingService: Send + Sync {
    /// Get the briefing file path for the given scope.
    ///
    /// Returns the path string if one exists, or `None`.
    fn get_briefing(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Option<String>, String>;
}

/// Function-pointer interactor implementing [`ReviewGetBriefingService`].
pub struct ReviewGetBriefingInteractor {
    #[allow(clippy::type_complexity)]
    run_fn: Arc<
        dyn Fn(String, Option<String>, PathBuf) -> Result<Option<String>, String> + Send + Sync,
    >,
}

impl ReviewGetBriefingInteractor {
    /// Create with injected function.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn new(
        run_fn: Arc<
            dyn Fn(String, Option<String>, PathBuf) -> Result<Option<String>, String> + Send + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl ReviewGetBriefingService for ReviewGetBriefingInteractor {
    fn get_briefing(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Option<String>, String> {
        (self.run_fn)(scope, track_id, items_dir)
    }
}

// ── ReviewRunLocalService ─────────────────────────────────────────────────────

/// Output DTO from `ReviewRunLocalService`.
///
/// Mirrors the shape of `CodexReviewOutcome` but uses stdlib-only types so
/// that `cli_driver` never imports domain or infrastructure review types.
pub struct ReviewRunLocalOutput {
    /// Human-readable output to print to stdout (the rendered review summary).
    pub stdout: Option<String>,
    /// Human-readable message for stderr (on error or subprocess failure).
    pub stderr: Option<String>,
    /// Process exit code.
    pub exit_code: u8,
}

/// Application service (primary port) for `sotp review local` (provider-resolved).
pub trait ReviewRunLocalService: Send + Sync {
    /// Run the reviewer with the provider auto-resolved from agent-profiles.json.
    #[allow(clippy::too_many_arguments)]
    fn run_local(
        &self,
        model: Option<String>,
        timeout_seconds: u64,
        briefing_file: Option<PathBuf>,
        prompt: Option<String>,
        track_id: Option<String>,
        round_type: String,
        group: String,
        items_dir: PathBuf,
    ) -> ReviewRunLocalOutput;
}

/// Function-pointer interactor implementing [`ReviewRunLocalService`].
pub struct ReviewRunLocalInteractor {
    #[allow(clippy::type_complexity)]
    run_fn: Arc<
        dyn Fn(
                Option<String>,
                u64,
                Option<PathBuf>,
                Option<String>,
                Option<String>,
                String,
                String,
                PathBuf,
            ) -> ReviewRunLocalOutput
            + Send
            + Sync,
    >,
}

impl ReviewRunLocalInteractor {
    /// Create with injected function.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn new(
        run_fn: Arc<
            dyn Fn(
                    Option<String>,
                    u64,
                    Option<PathBuf>,
                    Option<String>,
                    Option<String>,
                    String,
                    String,
                    PathBuf,
                ) -> ReviewRunLocalOutput
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl ReviewRunLocalService for ReviewRunLocalInteractor {
    #[allow(clippy::too_many_arguments)]
    fn run_local(
        &self,
        model: Option<String>,
        timeout_seconds: u64,
        briefing_file: Option<PathBuf>,
        prompt: Option<String>,
        track_id: Option<String>,
        round_type: String,
        group: String,
        items_dir: PathBuf,
    ) -> ReviewRunLocalOutput {
        (self.run_fn)(
            model,
            timeout_seconds,
            briefing_file,
            prompt,
            track_id,
            round_type,
            group,
            items_dir,
        )
    }
}
