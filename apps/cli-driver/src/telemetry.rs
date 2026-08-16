//! `telemetry` command family — primary adapter driver.
//!
//! `TelemetryDriver` holds the report/emission aggregate and handles only
//! data-bearing telemetry inputs at the primary-adapter boundary.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use usecase::TelemetryAggregateService;
use usecase::capability_exec::ReasoningEffort;
use usecase::telemetry::TelemetryReportOutput;
use usecase::telemetry::review_yield::ReviewYieldValue;

use crate::render::CommandOutcome;

/// Resolves the track-items directory from the common path options without
/// coupling the driver to the CLI's command DTO enum. Command identity is
/// normalized by the binary from clap's parsed subcommand tree; only the path
/// extraction belongs at this driver boundary.
///
/// Explicit `--items-dir` always wins. Commands that use `--workspace-root` as
/// their repository root derive the canonical `<root>/track/items` path from
/// it. Commands that expose an independent `--items-dir` (for example
/// `dry write` and track graph renderers) keep that option's default when only
/// `--workspace-root` is present. A project root is used as the same canonical
/// repository anchor when no workspace root is supplied.
#[must_use]
pub fn items_dir_from_args(args: &[OsString]) -> PathBuf {
    option_path(args, "--items-dir")
        .or_else(|| {
            let has_independent_items_dir = (args.get(1).is_some_and(|arg| arg == "dry")
                && args.get(2).is_some_and(|arg| arg == "write"))
                || (args.get(1).is_some_and(|arg| arg == "track")
                    && matches!(
                        args.get(2).and_then(|arg| arg.to_str()),
                        Some("type-graph" | "baseline-graph" | "contract-map")
                    ));
            (!has_independent_items_dir)
                .then(|| option_path(args, "--workspace-root"))
                .flatten()
                .map(|root| root.join("track/items"))
        })
        .or_else(|| option_path(args, "--project-root").map(|root| root.join("track/items")))
        .unwrap_or_else(|| PathBuf::from("track/items"))
}

fn option_path(args: &[OsString], name: &str) -> Option<PathBuf> {
    let equals_prefix = format!("{name}=");
    for (index, arg) in args.iter().enumerate().skip(1) {
        let text = arg.to_string_lossy();
        // Clap treats everything after `--` as positional payload.  Do not
        // reinterpret option-looking task descriptions as telemetry routing
        // options, or the completion sink could target a different project
        // than the command itself.
        if text == "--" {
            break;
        }
        #[cfg(unix)]
        let equals_value = {
            use std::os::unix::ffi::{OsStrExt, OsStringExt};

            arg.as_bytes()
                .strip_prefix(equals_prefix.as_bytes())
                .map(|value| PathBuf::from(OsString::from_vec(value.to_vec())))
        };
        #[cfg(windows)]
        let equals_value = {
            use std::os::windows::ffi::{OsStrExt, OsStringExt};

            let arg_units = arg.encode_wide().collect::<Vec<_>>();
            let prefix_units = equals_prefix.encode_utf16().collect::<Vec<_>>();
            arg_units
                .strip_prefix(&prefix_units)
                .map(|value| PathBuf::from(OsString::from_wide(value)))
        };
        #[cfg(not(any(unix, windows)))]
        let equals_value = text.strip_prefix(&equals_prefix).map(PathBuf::from);
        if let Some(value) = equals_value {
            return Some(value);
        }
        if text == name {
            return args.get(index + 1).map(PathBuf::from);
        }
    }
    None
}

/// Converts a process exit result to the canonical telemetry integer while
/// preserving all representable non-zero `u8` values.
#[must_use]
pub fn exit_code_value(exit_code: ExitCode) -> i32 {
    if exit_code == ExitCode::SUCCESS {
        return 0;
    }
    (1..=u8::MAX).find(|code| exit_code == ExitCode::from(*code)).map_or(1, i32::from)
}

/// Converts an elapsed command duration to the telemetry millisecond field.
#[must_use]
pub fn duration_millis(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

/// Captured command context held across dispatch and completion emission.
///
/// The source track is captured before dispatch so a command that changes
/// branches cannot retarget its completion record. Ineligible commands carry
/// no source context and complete as a diagnostic no-op.
pub struct TelemetryCompletion {
    items_dir: PathBuf,
    source_track_id: Option<String>,
    subcommand: String,
    started: Instant,
    eligible: bool,
    archive_completion_uses_archive_sink: bool,
}

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Input DTO for `sotp telemetry report`.
#[derive(Debug, Clone)]
pub struct TelemetryReportInput {
    /// Track ID whose telemetry log should be aggregated.
    pub track_id: String,
    /// Path to the track items directory (e.g. `track/items`).
    pub items_dir: PathBuf,
}

/// Typed input for the `telemetry` command family.
pub enum TelemetryInput {
    /// Aggregate and format telemetry for a track.
    Report(TelemetryReportInput),
    /// Complete a command whose context was captured before dispatch.
    CompleteCommand {
        /// Branch-bound context and command identity captured before dispatch.
        completion: TelemetryCompletion,
        /// Process exit code returned by the dispatched command.
        exit_code: i32,
        /// Optional error-chain text for a non-zero completion.
        error_chain: Option<String>,
    },
    /// Emit an active-track command completion through the existing telemetry
    /// writer path. Dispatch and timing are captured by the CLI boundary and
    /// arrive here as plain data.
    EmitCompletedCommand {
        /// Path to the track items directory used for repository resolution.
        items_dir: PathBuf,
        /// Track id captured before dispatch; `None` means non-track context.
        source_track_id: Option<String>,
        /// Opaque CLI command identity.
        subcommand: String,
        /// Process exit code returned by the dispatched command.
        exit_code: i32,
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
        /// Optional error-chain text for a non-zero completion.
        error_chain: Option<String>,
    },
    /// Emit a telemetry event for a subcommand dispatched against an archived track.
    EmitArchivedTrackSubcommand {
        /// Path to the track items directory (used to derive project root).
        items_dir: PathBuf,
        /// Track ID identifying the archived track.
        track_id: String,
        /// Opaque CLI subcommand label (e.g. `"track archive"`).
        subcommand: String,
        /// Process exit code.
        exit_code: i32,
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `telemetry` command family.
///
/// Holds the report/emission aggregate and exposes data-only handling.
pub struct TelemetryDriver {
    service: Arc<dyn TelemetryAggregateService>,
    #[allow(clippy::type_complexity)]
    context_resolver: Arc<dyn Fn(&Path) -> Option<String> + Send + Sync>,
    archive_completion_uses_archive_sink: bool,
}

impl TelemetryDriver {
    /// Create a driver with branch context resolution and a pre-resolved
    /// archive-routing policy supplied by infrastructure configuration.
    #[allow(clippy::type_complexity)]
    pub fn new(
        service: Arc<dyn TelemetryAggregateService>,
        context_resolver: Arc<dyn Fn(&Path) -> Option<String> + Send + Sync>,
        archive_completion_uses_archive_sink: bool,
    ) -> Self {
        Self { service, context_resolver, archive_completion_uses_archive_sink }
    }

    /// Capture command completion context before dispatch.
    #[must_use]
    pub fn begin_completion(&self, items_dir: PathBuf, subcommand: String) -> TelemetryCompletion {
        let normalized = subcommand.strip_prefix("sotp ").unwrap_or(&subcommand);
        let eligible = completion_eligible(normalized);
        let source_track_id = eligible.then(|| (self.context_resolver)(&items_dir)).flatten();
        TelemetryCompletion {
            items_dir,
            source_track_id,
            subcommand,
            started: Instant::now(),
            eligible,
            archive_completion_uses_archive_sink: self.archive_completion_uses_archive_sink,
        }
    }

    /// Handle a data-only telemetry command.
    pub fn handle(&self, input: TelemetryInput) -> CommandOutcome {
        match input {
            TelemetryInput::Report(input) => self.telemetry_report(input),
            TelemetryInput::CompleteCommand { completion, exit_code, error_chain } => {
                self.telemetry_complete(completion, exit_code, error_chain)
            }
            TelemetryInput::EmitCompletedCommand {
                items_dir,
                source_track_id,
                subcommand,
                exit_code,
                duration_ms,
                error_chain,
            } => self.telemetry_emit_completed(
                items_dir,
                source_track_id,
                subcommand,
                exit_code,
                duration_ms,
                error_chain,
            ),
            TelemetryInput::EmitArchivedTrackSubcommand {
                items_dir,
                track_id,
                subcommand,
                exit_code,
                duration_ms,
            } => self.telemetry_emit_archived(
                items_dir,
                track_id,
                subcommand,
                exit_code,
                duration_ms,
            ),
        }
    }

    fn telemetry_complete(
        &self,
        completion: TelemetryCompletion,
        exit_code: i32,
        error_chain: Option<String>,
    ) -> CommandOutcome {
        if !completion.eligible {
            return CommandOutcome::success(None);
        }
        let TelemetryCompletion {
            items_dir,
            source_track_id,
            subcommand,
            started,
            archive_completion_uses_archive_sink,
            ..
        } = completion;
        let duration_ms = duration_millis(started);
        let normalized = subcommand.strip_prefix("sotp ").unwrap_or(&subcommand);
        let archived_track_id = (normalized == "track archive" && exit_code == 0)
            .then_some(source_track_id.as_deref())
            .flatten()
            .filter(|_| archive_completion_uses_archive_sink);
        if let Some(track_id) = archived_track_id {
            let _ = self.service.emit_archived(
                &items_dir,
                track_id,
                subcommand,
                exit_code,
                duration_ms,
            );
        } else {
            let _ = self.service.emit_completed(
                &items_dir,
                source_track_id,
                subcommand,
                exit_code,
                duration_ms,
                error_chain,
            );
        }
        CommandOutcome::success(None)
    }

    fn telemetry_report(&self, input: TelemetryReportInput) -> CommandOutcome {
        match self.service.report(&input.track_id, &input.items_dir) {
            Ok(output) => {
                let text = format_report(&input.track_id, &output);
                CommandOutcome::success(Some(text))
            }
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }

    fn telemetry_emit_archived(
        &self,
        items_dir: PathBuf,
        track_id: String,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> CommandOutcome {
        // Archive telemetry is diagnostic-only; preserve the archive command's
        // original result when the diagnostic sink is unavailable.
        match self.service.emit_archived(&items_dir, &track_id, subcommand, exit_code, duration_ms)
        {
            Ok(()) | Err(_) => CommandOutcome::success(None),
        }
    }

    fn telemetry_emit_completed(
        &self,
        items_dir: PathBuf,
        source_track_id: Option<String>,
        subcommand: String,
        exit_code: i32,
        duration_ms: u64,
        error_chain: Option<String>,
    ) -> CommandOutcome {
        match self.service.emit_completed(
            &items_dir,
            source_track_id,
            subcommand,
            exit_code,
            duration_ms,
            error_chain,
        ) {
            Ok(()) => CommandOutcome::success(None),
            // Telemetry is diagnostic-only; preserve a successful driver
            // outcome even when a concrete adapter reports an unavailable
            // sink. The caller's command exit code is never replaced.
            Err(_) => CommandOutcome::success(None),
        }
    }
}

/// Resolve the completion-telemetry admission decision for a normalized
/// command identity (IN-01 / AC-01).
#[must_use]
pub fn completion_eligible(subcommand: &str) -> bool {
    let normalized = subcommand.strip_prefix("sotp ").unwrap_or(subcommand);
    let top_level = normalized.split_whitespace().next().unwrap_or_default();
    if top_level == "track" {
        return !telemetry_track_display_only(normalized);
    }
    if top_level == "verify" {
        return normalized != "verify results";
    }
    if matches!(top_level, "arch" | "hook" | "find-similar" | "telemetry") {
        return false;
    }
    !matches!(
        normalized,
        "conventions resolve"
            | "phase explain"
            | "review results"
            | "review classify"
            | "review files"
            | "dry results"
            | "ref-verify results"
            | "signal report"
            | "test-obligation results"
            | "test-obligation bindings-skeleton"
            | "dup-index measure-quality"
            | "pr status"
            | "pr poll-review"
            | "maintenance cleanup plan"
    )
}

fn telemetry_track_display_only(subcommand: &str) -> bool {
    matches!(
        subcommand,
        "track resolve"
            | "track next-task"
            | "track task-counts"
            | "track views validate"
            | "track spec-element-hash"
            | "track fixpoint-resolve"
            | "track catalogue-impl-signals"
            | "track type-graph"
            | "track contract-map"
    )
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Format a [`TelemetryReportOutput`] DTO into a human-readable report string.
///
/// Pure function — no side effects, no I/O. Returns a `String`; the caller
/// (`TelemetryDriver::telemetry_report`) is responsible for outputting it.
fn format_report(track_id: &str, output: &TelemetryReportOutput) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push(format!("Telemetry report for track: {track_id}"));
    lines.push(String::new());

    lines.push("Phase durations:".to_owned());
    if output.phase_durations.is_empty() {
        lines.push("  (no phase data recorded)".to_owned());
    } else {
        for pd in &output.phase_durations {
            lines.push(format!(
                "  {:<40} {:>8} ms  ({} event(s))",
                pd.phase_name, pd.total_ms, pd.event_count
            ));
        }
    }
    lines.push(String::new());

    lines.push(format!("Errors ({}):", output.errors.len()));
    if output.errors.is_empty() {
        lines.push("  (none)".to_owned());
    } else {
        for err in &output.errors {
            lines.push(format!(
                "  [{}] {} (exit {}): {}",
                err.timestamp, err.command, err.exit_code, err.error_chain
            ));
        }
    }
    lines.push(String::new());

    lines.push(format!("Hook blocks ({}):", output.hook_blocks.len()));
    if output.hook_blocks.is_empty() {
        lines.push("  (none)".to_owned());
    } else {
        for hb in &output.hook_blocks {
            lines.push(format!("  [{}] {}", hb.timestamp, hb.hook_name));
        }
    }
    lines.push(String::new());

    lines.push("Command metrics:".to_owned());
    if output.command_metrics.is_empty() {
        lines.push("  (no command data recorded)".to_owned());
    } else {
        for metric in &output.command_metrics {
            let executions = *metric.executions().as_ref();
            let failures = *metric.failures().as_ref();
            let total_duration = *metric.total_duration().as_ref();
            let failure_rate = metric.failure_rate().value();
            lines.push(format!(
                "  {:<40} {:>8} execution(s) {:>8} ms  ({} failure(s), {}.{:02}% failure rate)",
                metric.command().as_str(),
                executions,
                total_duration,
                failures,
                failure_rate / 100,
                failure_rate % 100,
            ));
        }
    }
    lines.push(String::new());

    lines.push("Review yield metrics:".to_owned());
    if output.review_yield_metrics.is_empty() {
        lines.push("  (no review yield data recorded)".to_owned());
    } else {
        for metric in &output.review_yield_metrics {
            let (axis, value) = review_yield_value_label(&metric.value);
            lines.push(format!(
                "  {axis}={value}: {} execution(s), {} basis points detection rate",
                metric.execution_count, metric.detection_rate
            ));
        }
    }
    lines.push(String::new());

    let skipped_lines = output.skipped_lines.as_ref();
    lines.push(format!("Skipped lines: {skipped_lines}"));
    if *skipped_lines > 0 {
        lines.push(
            "  (parse failure, unknown schema_version, oversized record, or retained-output cap)"
                .to_owned(),
        );
    }
    lines.push(String::new());

    lines.join("\n")
}

fn review_yield_value_label(value: &ReviewYieldValue) -> (&'static str, String) {
    match value {
        ReviewYieldValue::Scope(scope) => ("scope", scope.to_string()),
        ReviewYieldValue::RoundType(round_type) => ("round_type", round_type.to_string()),
        ReviewYieldValue::Provider(provider) => ("provider", provider.to_string()),
        ReviewYieldValue::Model(model) => ("model", model.to_string()),
        ReviewYieldValue::ReasoningEffort(effort) => {
            ("reasoning_effort", reasoning_effort_label(*effort).to_owned())
        }
    }
}

fn reasoning_effort_label(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
        ReasoningEffort::Max => "max",
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::ffi::OsString;
    use std::num::NonZeroU64;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use usecase::capability_exec::{ModelName, ProviderName};
    use usecase::telemetry::{
        TelemetryAggregateServiceError, TelemetryArchivedService, TelemetryEmitService,
        TelemetryReportService,
        command_trace::{
            CommandDurationMillis, CommandExecutionCount, CommandExecutionMetric,
            SotpCommandIdentity, TelemetrySkippedLineCount,
        },
        review_yield::{
            ReviewDetectionRateBasisPoints, ReviewExecutionCount, ReviewYieldMetric,
            ReviewYieldValue,
        },
    };

    use super::*;

    #[test]
    fn test_items_dir_from_args_preserves_explicit_project_root() {
        let args = vec![
            OsString::from("sotp"),
            OsString::from("dry"),
            OsString::from("write"),
            OsString::from("--project-root"),
            OsString::from("/workspace/project"),
        ];

        let items_dir = items_dir_from_args(&args);
        assert_eq!(items_dir, PathBuf::from("/workspace/project/track/items"));
    }

    #[test]
    fn test_items_dir_from_args_ignores_option_looking_positional_after_delimiter() {
        let args = vec![
            OsString::from("sotp"),
            OsString::from("track"),
            OsString::from("add-task"),
            OsString::from("--track-id"),
            OsString::from("current"),
            OsString::from("--"),
            OsString::from("--items-dir=/other/repo/track/items"),
        ];

        assert_eq!(items_dir_from_args(&args), PathBuf::from("track/items"));
    }

    #[test]
    fn test_items_dir_from_args_does_not_treat_workspace_root_as_items_dir() {
        let args = vec![
            OsString::from("sotp"),
            OsString::from("dry"),
            OsString::from("write"),
            OsString::from("--workspace-root"),
            OsString::from("/other/repository"),
        ];

        assert_eq!(items_dir_from_args(&args), PathBuf::from("track/items"));
    }

    #[test]
    fn test_items_dir_from_args_derives_track_root_for_workspace_commands() {
        let baseline_args = vec![
            OsString::from("sotp"),
            OsString::from("track"),
            OsString::from("baseline-capture"),
            OsString::from("--workspace-root"),
            OsString::from("/other/repository"),
        ];
        let lint_args = vec![
            OsString::from("sotp"),
            OsString::from("track"),
            OsString::from("lint"),
            OsString::from("--workspace-root=/other/repository"),
        ];

        assert_eq!(
            items_dir_from_args(&baseline_args),
            PathBuf::from("/other/repository/track/items")
        );
        assert_eq!(items_dir_from_args(&lint_args), PathBuf::from("/other/repository/track/items"));
    }

    #[test]
    fn test_items_dir_from_args_prefers_explicit_items_dir_over_workspace_root() {
        let args = vec![
            OsString::from("sotp"),
            OsString::from("verify"),
            OsString::from("catalogue-spec-refs"),
            OsString::from("--workspace-root"),
            OsString::from("/other/repository"),
            OsString::from("--items-dir"),
            OsString::from("/explicit/items"),
        ];

        assert_eq!(items_dir_from_args(&args), PathBuf::from("/explicit/items"));
    }

    #[test]
    fn test_items_dir_from_args_keeps_independent_default_for_workspace_options() {
        let baseline_graph_args = vec![
            OsString::from("sotp"),
            OsString::from("track"),
            OsString::from("baseline-graph"),
            OsString::from("--workspace-root"),
            OsString::from("/other/repository"),
        ];
        assert_eq!(items_dir_from_args(&baseline_graph_args), PathBuf::from("track/items"));
    }

    #[test]
    fn test_items_dir_from_args_derives_verify_catalogue_items_from_workspace_root() {
        let args = vec![
            OsString::from("sotp"),
            OsString::from("verify"),
            OsString::from("catalogue-spec-refs"),
            OsString::from("--workspace-root=/other/repository"),
        ];

        assert_eq!(items_dir_from_args(&args), PathBuf::from("/other/repository/track/items"));
    }

    #[cfg(unix)]
    #[test]
    fn test_items_dir_from_args_preserves_non_utf8_equals_path_bytes() {
        use std::os::unix::ffi::OsStringExt;

        let mut workspace_arg = OsString::from("--workspace-root=");
        workspace_arg.push(OsString::from_vec(b"/other/\xff/repository".to_vec()));
        let args = vec![
            OsString::from("sotp"),
            OsString::from("track"),
            OsString::from("baseline-capture"),
            workspace_arg,
        ];

        let items_dir = items_dir_from_args(&args);
        assert!(
            items_dir
                .as_os_str()
                .as_encoded_bytes()
                .starts_with(b"/other/\xff/repository/track/items")
        );
    }

    #[test]
    fn test_duration_millis_and_exit_code_value_preserve_completed_command_values() {
        let duration = duration_millis(Instant::now() - Duration::from_millis(1));

        assert!(duration >= 1);
        assert_eq!(exit_code_value(ExitCode::SUCCESS), 0);
        assert_eq!(exit_code_value(ExitCode::from(42)), 42);
    }

    struct MetricsService {
        report: TelemetryReportOutput,
    }

    impl TelemetryReportService for MetricsService {
        fn report(
            &self,
            _track_id: &str,
            _items_dir: &Path,
        ) -> Result<TelemetryReportOutput, TelemetryAggregateServiceError> {
            Ok(self.report.clone())
        }
    }

    impl TelemetryEmitService for MetricsService {
        fn emit_completed(
            &self,
            _items_dir: &Path,
            _source_track_id: Option<String>,
            _subcommand: String,
            _exit_code: i32,
            _duration_ms: u64,
            _error_chain: Option<String>,
        ) -> Result<(), TelemetryAggregateServiceError> {
            Ok(())
        }
    }

    impl TelemetryArchivedService for MetricsService {
        fn emit_archived(
            &self,
            _items_dir: &Path,
            _track_id: &str,
            _subcommand: String,
            _exit_code: i32,
            _duration_ms: u64,
        ) -> Result<(), TelemetryAggregateServiceError> {
            Ok(())
        }
    }

    impl TelemetryAggregateService for MetricsService {}

    struct FailingReportService;

    impl TelemetryReportService for FailingReportService {
        fn report(
            &self,
            _track_id: &str,
            _items_dir: &Path,
        ) -> Result<TelemetryReportOutput, TelemetryAggregateServiceError> {
            Err(TelemetryAggregateServiceError::ReportUnavailable(
                "report fixture unavailable".to_owned(),
            ))
        }
    }

    impl TelemetryEmitService for FailingReportService {
        fn emit_completed(
            &self,
            _items_dir: &Path,
            _source_track_id: Option<String>,
            _subcommand: String,
            _exit_code: i32,
            _duration_ms: u64,
            _error_chain: Option<String>,
        ) -> Result<(), TelemetryAggregateServiceError> {
            Ok(())
        }
    }

    impl TelemetryArchivedService for FailingReportService {
        fn emit_archived(
            &self,
            _items_dir: &Path,
            _track_id: &str,
            _subcommand: String,
            _exit_code: i32,
            _duration_ms: u64,
        ) -> Result<(), TelemetryAggregateServiceError> {
            Ok(())
        }
    }

    impl TelemetryAggregateService for FailingReportService {}

    struct CompletedCommandService {
        calls: std::sync::Mutex<Vec<String>>,
        fail: bool,
    }

    impl TelemetryReportService for CompletedCommandService {
        fn report(
            &self,
            _track_id: &str,
            _items_dir: &Path,
        ) -> Result<TelemetryReportOutput, TelemetryAggregateServiceError> {
            Ok(TelemetryReportOutput {
                phase_durations: Vec::new(),
                errors: Vec::new(),
                hook_blocks: Vec::new(),
                skipped_lines: TelemetrySkippedLineCount::from(0),
                command_metrics: Vec::new(),
                review_yield_metrics: Vec::new(),
            })
        }
    }

    impl TelemetryArchivedService for CompletedCommandService {
        fn emit_archived(
            &self,
            _items_dir: &Path,
            _track_id: &str,
            _subcommand: String,
            _exit_code: i32,
            _duration_ms: u64,
        ) -> Result<(), TelemetryAggregateServiceError> {
            Ok(())
        }
    }

    impl TelemetryEmitService for CompletedCommandService {
        fn emit_completed(
            &self,
            _items_dir: &Path,
            source_track_id: Option<String>,
            subcommand: String,
            exit_code: i32,
            duration_ms: u64,
            error_chain: Option<String>,
        ) -> Result<(), TelemetryAggregateServiceError> {
            self.calls.lock().unwrap().push(format!(
                "{subcommand}|{exit_code}|{duration_ms}|{}|{}",
                error_chain.unwrap_or_default(),
                source_track_id.unwrap_or_default(),
            ));
            if self.fail {
                Err(TelemetryAggregateServiceError::EmitUnavailable(
                    "telemetry append failed".to_owned(),
                ))
            } else {
                Ok(())
            }
        }
    }

    impl TelemetryAggregateService for CompletedCommandService {}

    struct ArchiveCommandService {
        calls: std::sync::Mutex<Vec<String>>,
        fail: bool,
    }

    impl TelemetryReportService for ArchiveCommandService {
        fn report(
            &self,
            _track_id: &str,
            _items_dir: &Path,
        ) -> Result<TelemetryReportOutput, TelemetryAggregateServiceError> {
            Ok(TelemetryReportOutput {
                phase_durations: Vec::new(),
                errors: Vec::new(),
                hook_blocks: Vec::new(),
                skipped_lines: TelemetrySkippedLineCount::from(0),
                command_metrics: Vec::new(),
                review_yield_metrics: Vec::new(),
            })
        }
    }

    impl TelemetryEmitService for ArchiveCommandService {
        fn emit_completed(
            &self,
            _items_dir: &Path,
            _source_track_id: Option<String>,
            _subcommand: String,
            _exit_code: i32,
            _duration_ms: u64,
            _error_chain: Option<String>,
        ) -> Result<(), TelemetryAggregateServiceError> {
            Ok(())
        }
    }

    impl TelemetryArchivedService for ArchiveCommandService {
        fn emit_archived(
            &self,
            _items_dir: &Path,
            track_id: &str,
            subcommand: String,
            exit_code: i32,
            duration_ms: u64,
        ) -> Result<(), TelemetryAggregateServiceError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("{track_id}|{subcommand}|{exit_code}|{duration_ms}"));
            if self.fail {
                Err(TelemetryAggregateServiceError::EmitUnavailable(
                    "archive telemetry unavailable".to_owned(),
                ))
            } else {
                Ok(())
            }
        }
    }

    impl TelemetryAggregateService for ArchiveCommandService {}

    fn build_driver<T>(service: Arc<T>) -> TelemetryDriver
    where
        T: TelemetryAggregateService + 'static,
    {
        TelemetryDriver::new(service, Arc::new(|_| None), false)
    }

    #[test]
    fn test_telemetry_driver_begin_completion_captures_context_and_emits_once() {
        let service_impl = Arc::new(CompletedCommandService {
            calls: std::sync::Mutex::new(Vec::new()),
            fail: false,
        });
        let service: Arc<dyn TelemetryAggregateService> = service_impl.clone();
        let driver = TelemetryDriver::new(
            service,
            Arc::new(|items_dir| {
                assert_eq!(items_dir, Path::new("track/items"));
                Some("active-track".to_owned())
            }),
            false,
        );
        let completion =
            driver.begin_completion(PathBuf::from("track/items"), "sotp dry write".to_owned());

        assert!(completion.eligible);
        assert_eq!(completion.source_track_id.as_deref(), Some("active-track"));
        let outcome = driver.handle(TelemetryInput::CompleteCommand {
            completion,
            exit_code: 17,
            error_chain: Some("command failed".to_owned()),
        });

        assert_eq!(outcome.exit_code, 0, "diagnostic telemetry must preserve command outcome");
        let calls = service_impl.calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "eligible completion must emit exactly once");
        assert!(calls[0].starts_with("sotp dry write|17|"));
        assert!(calls[0].contains("|command failed|active-track"));
    }

    #[test]
    fn test_telemetry_driver_begin_completion_skips_ineligible_context_resolution() {
        let service_impl = Arc::new(CompletedCommandService {
            calls: std::sync::Mutex::new(Vec::new()),
            fail: false,
        });
        let service: Arc<dyn TelemetryAggregateService> = service_impl.clone();
        let resolver_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let resolver_calls_for_callback = Arc::clone(&resolver_calls);
        let driver = TelemetryDriver::new(
            service,
            Arc::new(move |_| {
                resolver_calls_for_callback.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Some("must-not-be-captured".to_owned())
            }),
            false,
        );
        let completion =
            driver.begin_completion(PathBuf::from("track/items"), "sotp track resolve".to_owned());

        assert!(!completion.eligible);
        assert!(completion.source_track_id.is_none());
        let outcome = driver.handle(TelemetryInput::CompleteCommand {
            completion,
            exit_code: 9,
            error_chain: Some("display-only failure".to_owned()),
        });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(resolver_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(service_impl.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_telemetry_driver_begin_completion_routes_archive_to_archived_service() {
        let service_impl = Arc::new(ArchiveCommandService {
            calls: std::sync::Mutex::new(Vec::new()),
            fail: false,
        });
        let service: Arc<dyn TelemetryAggregateService> = service_impl.clone();
        let driver =
            TelemetryDriver::new(service, Arc::new(|_| Some("archived-track".to_owned())), true);
        let completion =
            driver.begin_completion(PathBuf::from("track/items"), "sotp track archive".to_owned());
        let outcome = driver.handle(TelemetryInput::CompleteCommand {
            completion,
            exit_code: 0,
            error_chain: None,
        });

        assert_eq!(outcome.exit_code, 0);
        let calls = service_impl.calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "successful archive must use archived telemetry emission");
        assert!(calls[0].starts_with("archived-track|sotp track archive|0|"));
    }

    #[test]
    fn test_telemetry_driver_complete_command_archive_failure_preserves_outcome() {
        let service_impl = Arc::new(ArchiveCommandService {
            calls: std::sync::Mutex::new(Vec::new()),
            fail: true,
        });
        let service: Arc<dyn TelemetryAggregateService> = service_impl.clone();
        let driver =
            TelemetryDriver::new(service, Arc::new(|_| Some("archived-track".to_owned())), true);
        let completion =
            driver.begin_completion(PathBuf::from("track/items"), "sotp track archive".to_owned());
        let outcome = driver.handle(TelemetryInput::CompleteCommand {
            completion,
            exit_code: 0,
            error_chain: None,
        });

        assert_eq!(outcome.exit_code, 0, "archive write failures are diagnostic-only");
        assert_eq!(
            service_impl.calls.lock().unwrap().len(),
            1,
            "the completion archive route must attempt exactly one emission"
        );
    }

    #[test]
    fn test_telemetry_driver_archived_command_forwards_track_id_exit_and_duration() {
        let service = Arc::new(ArchiveCommandService {
            calls: std::sync::Mutex::new(Vec::new()),
            fail: false,
        });
        let driver = build_driver(Arc::clone(&service));
        let outcome = driver.handle(TelemetryInput::EmitArchivedTrackSubcommand {
            items_dir: PathBuf::from("track/items"),
            track_id: "archived-track".to_owned(),
            subcommand: "sotp track archive".to_owned(),
            exit_code: 3,
            duration_ms: 80,
        });

        assert_eq!(outcome.exit_code, 0, "archive telemetry is diagnostic-only");
        let call = service.calls.lock().unwrap()[0].clone();
        assert!(
            call.starts_with("archived-track|sotp track archive|3|"),
            "archive payload: {call}"
        );
    }

    #[test]
    fn test_telemetry_driver_archived_append_failure_preserves_success_outcome() {
        let service = Arc::new(ArchiveCommandService {
            calls: std::sync::Mutex::new(Vec::new()),
            fail: true,
        });
        let driver = build_driver(Arc::clone(&service));
        let outcome = driver.handle(TelemetryInput::EmitArchivedTrackSubcommand {
            items_dir: PathBuf::from("track/items"),
            track_id: "archived-track".to_owned(),
            subcommand: "sotp track archive".to_owned(),
            exit_code: 0,
            duration_ms: 0,
        });

        assert_eq!(outcome.exit_code, 0, "archive append failure must not replace command outcome");
        assert_eq!(service.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_telemetry_driver_completed_command_forwards_identity_duration_exit_and_error() {
        let service = Arc::new(CompletedCommandService {
            calls: std::sync::Mutex::new(Vec::new()),
            fail: false,
        });
        let driver = build_driver(Arc::clone(&service));
        let outcome = driver.handle(TelemetryInput::EmitCompletedCommand {
            items_dir: PathBuf::from("track/items"),
            subcommand: "sotp dry".to_owned(),
            source_track_id: Some("track-id".to_owned()),
            exit_code: 17,
            duration_ms: 240,
            error_chain: Some("command failed".to_owned()),
        });

        assert_eq!(outcome.exit_code, 0, "telemetry is diagnostic-only");
        assert_eq!(
            service.calls.lock().unwrap().len(),
            1,
            "the completion event must be emitted once"
        );
        let call = service.calls.lock().unwrap()[0].clone();
        assert!(
            call.starts_with("sotp dry|17|"),
            "identity and exit code must be preserved: {call}"
        );
        let duration_ms = call
            .split('|')
            .nth(2)
            .and_then(|value| value.parse::<u64>().ok())
            .expect("completion telemetry must carry duration milliseconds");
        assert!(duration_ms >= 200, "elapsed duration must be forwarded: {call}");
        assert!(
            call.contains("|command failed|track-id"),
            "error chain and source track must be preserved: {call}"
        );
    }

    #[test]
    fn test_telemetry_driver_completed_command_append_failure_preserves_success_outcome() {
        let service = Arc::new(CompletedCommandService {
            calls: std::sync::Mutex::new(Vec::new()),
            fail: true,
        });
        let driver = build_driver(Arc::clone(&service));
        let outcome = driver.handle(TelemetryInput::EmitCompletedCommand {
            items_dir: PathBuf::from("track/items"),
            subcommand: "sotp verify".to_owned(),
            source_track_id: Some("track-id".to_owned()),
            exit_code: 0,
            duration_ms: 0,
            error_chain: None,
        });

        assert_eq!(outcome.exit_code, 0, "append failure must not replace command outcome");
        assert_eq!(service.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_telemetry_report_command_metrics_render_frequency_duration_and_failure_rate()
    -> Result<(), Box<dyn std::error::Error>> {
        let metric = CommandExecutionMetric::new(
            SotpCommandIdentity::try_new("track status".to_owned())?,
            CommandExecutionCount::from(3),
            CommandExecutionCount::from(1),
            CommandDurationMillis::from(540),
        )?;
        let service = Arc::new(MetricsService {
            report: TelemetryReportOutput {
                phase_durations: Vec::new(),
                errors: Vec::new(),
                hook_blocks: Vec::new(),
                skipped_lines: TelemetrySkippedLineCount::from(0),
                command_metrics: vec![metric],
                review_yield_metrics: Vec::new(),
            },
        });
        let driver = build_driver(service);

        let outcome = driver.handle(TelemetryInput::Report(TelemetryReportInput {
            track_id: "test-track".to_owned(),
            items_dir: PathBuf::from("track/items"),
        }));
        let report = outcome.stdout.expect("successful report has stdout");

        assert_eq!(outcome.exit_code, 0);
        assert!(report.contains("Command metrics:"));
        assert!(report.contains("track status"));
        assert!(report.contains("3 execution(s)"));
        assert!(report.contains("540 ms"));
        assert!(report.contains("1 failure(s), 33.33% failure rate"));
        Ok(())
    }

    #[test]
    fn test_telemetry_report_review_yield_metrics_render_each_accessible_axis() {
        let execution_count = ReviewExecutionCount::new(NonZeroU64::new(3).unwrap());
        let detection_rate = ReviewDetectionRateBasisPoints::try_new(6_666).unwrap();
        let service = Arc::new(MetricsService {
            report: TelemetryReportOutput {
                phase_durations: Vec::new(),
                errors: Vec::new(),
                hook_blocks: Vec::new(),
                skipped_lines: TelemetrySkippedLineCount::from(0),
                command_metrics: Vec::new(),
                review_yield_metrics: vec![
                    ReviewYieldMetric {
                        value: ReviewYieldValue::Scope(
                            domain::review_v2::ScopeName::parse("cli_driver").unwrap(),
                        ),
                        execution_count,
                        detection_rate,
                    },
                    ReviewYieldMetric {
                        value: ReviewYieldValue::RoundType(domain::review_v2::RoundType::Final),
                        execution_count,
                        detection_rate,
                    },
                    ReviewYieldMetric {
                        value: ReviewYieldValue::Provider(ProviderName::try_new("codex").unwrap()),
                        execution_count,
                        detection_rate,
                    },
                    ReviewYieldMetric {
                        value: ReviewYieldValue::Model(ModelName::try_new("gpt-5").unwrap()),
                        execution_count,
                        detection_rate,
                    },
                    ReviewYieldMetric {
                        value: ReviewYieldValue::ReasoningEffort(ReasoningEffort::High),
                        execution_count,
                        detection_rate,
                    },
                ],
            },
        });
        let driver = build_driver(service);

        let outcome = driver.handle(TelemetryInput::Report(TelemetryReportInput {
            track_id: "test-track".to_owned(),
            items_dir: PathBuf::from("track/items"),
        }));
        let report = outcome.stdout.expect("successful report has stdout");

        assert!(report.contains("Review yield metrics:"));
        assert!(
            report.contains("scope=cli_driver: 3 execution(s), 6666 basis points detection rate")
        );
        assert!(
            report.contains("round_type=final: 3 execution(s), 6666 basis points detection rate")
        );
        assert!(
            report.contains("provider=codex: 3 execution(s), 6666 basis points detection rate")
        );
        assert!(report.contains("model=gpt-5: 3 execution(s), 6666 basis points detection rate"));
        assert!(
            report.contains(
                "reasoning_effort=high: 3 execution(s), 6666 basis points detection rate"
            )
        );
    }

    #[test]
    fn test_telemetry_report_empty_command_metrics_renders_empty_data_message() {
        let service = Arc::new(MetricsService {
            report: TelemetryReportOutput {
                phase_durations: Vec::new(),
                errors: Vec::new(),
                hook_blocks: Vec::new(),
                skipped_lines: TelemetrySkippedLineCount::from(0),
                command_metrics: Vec::new(),
                review_yield_metrics: Vec::new(),
            },
        });
        let driver = build_driver(service);

        let outcome = driver.handle(TelemetryInput::Report(TelemetryReportInput {
            track_id: "test-track".to_owned(),
            items_dir: PathBuf::from("track/items"),
        }));

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stderr, None);
        assert!(
            outcome
                .stdout
                .as_deref()
                .is_some_and(|report| report.contains("  (no command data recorded)"))
        );
        assert!(
            outcome
                .stdout
                .as_deref()
                .is_some_and(|report| report.contains("  (no review yield data recorded)"))
        );
        assert!(outcome.stdout.as_deref().is_some_and(|report| !report.contains("detection rate")));
    }

    #[test]
    fn test_telemetry_report_nonzero_skipped_lines_explain_projection_cap() {
        let service = Arc::new(MetricsService {
            report: TelemetryReportOutput {
                phase_durations: Vec::new(),
                errors: Vec::new(),
                hook_blocks: Vec::new(),
                skipped_lines: TelemetrySkippedLineCount::from(1),
                command_metrics: Vec::new(),
                review_yield_metrics: Vec::new(),
            },
        });
        let driver = build_driver(service);

        let outcome = driver.handle(TelemetryInput::Report(TelemetryReportInput {
            track_id: "test-track".to_owned(),
            items_dir: PathBuf::from("track/items"),
        }));

        let report = outcome.stdout.expect("successful report has stdout");
        assert!(report.contains("Skipped lines: 1"));
        assert!(report.contains("retained-output cap"));
    }

    #[test]
    fn test_telemetry_report_service_failure_preserves_failure_outcome() {
        let driver = build_driver(Arc::new(FailingReportService));

        let outcome = driver.handle(TelemetryInput::Report(TelemetryReportInput {
            track_id: "test-track".to_owned(),
            items_dir: PathBuf::from("track/items"),
        }));

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stdout, None);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("report unavailable: report fixture unavailable")
        );
    }
}
