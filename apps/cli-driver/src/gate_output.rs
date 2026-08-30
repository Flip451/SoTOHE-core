//! Primary adapter and summary helpers for the `gate-output` command.
//!
//! Child output is opaque at the use-case boundary. The driver selects
//! presentation-safe failure records for the human-facing summary; all other
//! bytes remain available in the persisted log.

use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;

use usecase::gate_output::{
    GateExitCode, GateLogReservationError, GateLogWriteError, GateLogWriteOutcome,
    GateProcessError, GateRunCommand, GateRunError, GateRunResult, GateRunService,
};

use crate::render::CommandOutcome;

const MAX_FAILURE_EXCERPTS: usize = 8;
const MAX_FAILURE_LINE_BYTES: usize = 240;
const CLI_EXIT_CODE_FALLBACK: u8 = 1;

/// Driving-boundary input for one gate execution.
#[derive(Debug)]
pub struct GateOutputInput {
    name: String,
    command: Vec<OsString>,
}

impl GateOutputInput {
    /// Creates gate input from an opaque label and OS-native child argv.
    #[must_use]
    pub fn new(name: String, command: Vec<OsString>) -> GateOutputInput {
        GateOutputInput { name, command }
    }
}

/// Primary adapter that invokes the gate use case and renders its result.
pub struct GateOutputDriver {
    service: Arc<dyn GateRunService>,
}

impl GateOutputDriver {
    /// Creates a driver from the gate-run application service.
    #[must_use]
    pub fn new(service: Arc<dyn GateRunService>) -> GateOutputDriver {
        GateOutputDriver { service }
    }

    /// Executes one gate and returns the compact human-facing outcome.
    #[must_use]
    pub fn invoke(&self, input: GateOutputInput) -> CommandOutcome {
        let command = match GateRunCommand::try_new(input.name, input.command) {
            Ok(command) => command,
            Err(error) => return CommandOutcome::failure(Some(error.to_string())),
        };
        match self.service.execute(command) {
            Ok(result) => {
                let exit_code = match &result {
                    GateRunResult::ChildExited { exit_code, .. } => process_exit_code(exit_code),
                    GateRunResult::SpawnFailed { .. } => CLI_EXIT_CODE_FALLBACK,
                };
                CommandOutcome { stdout: Some(render_summary(&result)), stderr: None, exit_code }
            }
            Err(GateRunError::Prepare(error)) => CommandOutcome {
                stdout: Some(render_prepare_failure(&error)),
                stderr: None,
                exit_code: CLI_EXIT_CODE_FALLBACK,
            },
        }
    }
}

/// Selects a bounded set of presentation-safe records from child output.
///
/// The child bytes are not decoded or normalized. Only complete records made
/// of ASCII bytes and separated by the summary's declared line separator are
/// considered; non-ASCII and control-containing records are omitted. Known
/// non-failure records are omitted, while otherwise unrecognized non-empty
/// records remain eligible for the summary. At most eight records are returned;
/// the persisted log path in the rendered summary remains the source for any
/// omitted records.
pub fn failure_excerpts(output: &[u8]) -> Vec<String> {
    output
        .split(|byte| *byte == b'\n')
        .filter_map(ascii_failure_record)
        .take(MAX_FAILURE_EXCERPTS)
        .collect()
}

fn ascii_failure_record(record: &[u8]) -> Option<String> {
    if record.is_empty() || !record.is_ascii() {
        return None;
    }

    let record = record.iter().map(|byte| char::from(*byte)).collect::<String>();
    if record.chars().any(char::is_control) {
        return None;
    }

    let trimmed = record.trim();
    is_failure_line(trimmed).then(|| truncate_line(trimmed))
}

/// Classifies the supported test, obligation, and aggregate gate diagnostics.
///
/// The accepted grammar is deliberately line-oriented: project markers
/// (`[FAIL]`, `[ERROR]`, `[BLOCKED]`), the aggregate summary verdict (`FAIL`)
/// and its summary bullets, the stable failure prefixes and suffixes emitted by
/// the supported Cargo/nextest/rustc commands, and the project-owned
/// `test-obligation`/`cargo-make` failure prefixes are recognized explicitly.
/// Known success and internal-record lines are excluded. Unrecognized
/// non-empty lines remain eligible so a new gate-output format cannot silently
/// disappear from the failure summary.
pub fn is_failure_line(line: &str) -> bool {
    if line.chars().any(char::is_control) {
        return false;
    }

    let trimmed = line.trim();
    let prefix = trimmed;
    let suffix = trimmed;
    let diagnostic_prefix = prefix.strip_prefix("- ").unwrap_or(prefix);
    let diagnostic_suffix = suffix.strip_prefix("- ").unwrap_or(suffix);
    let aggregate_summary_failure = diagnostic_prefix.eq_ignore_ascii_case("FAIL")
        && diagnostic_suffix.eq_ignore_ascii_case("FAIL");
    let aggregate_summary_failure_detail =
        starts_with_ascii_case_insensitive(diagnostic_prefix, "child exited with status ")
            || starts_with_ascii_case_insensitive(
                diagnostic_prefix,
                "could not start child command: ",
            );
    let marked_failure = starts_with_ascii_case_insensitive(diagnostic_prefix, "[fail]")
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "[error]")
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "[blocked]");
    let cargo_test_failure = starts_with_ascii_case_insensitive(diagnostic_prefix, "test ")
        && ends_with_ascii_case_insensitive(diagnostic_suffix, " ... failed");
    let cargo_test_result =
        starts_with_ascii_case_insensitive(diagnostic_prefix, "test result: failed");
    let nextest_failure = starts_with_ascii_case_insensitive(diagnostic_prefix, "fail [");
    let nextest_summary = starts_with_ascii_case_insensitive(diagnostic_prefix, "summary ")
        && diagnostic_prefix.split_ascii_whitespace().any(|word| {
            word.eq_ignore_ascii_case("failed")
                || word
                    .get(.."failed,".len())
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case("failed,"))
        });
    let panic_line = starts_with_ascii_case_insensitive(diagnostic_prefix, "thread '")
        && contains_ascii_case_insensitive(diagnostic_prefix, "' panicked at ");
    let obligation_failure =
        starts_with_ascii_case_insensitive(diagnostic_prefix, "test-obligation check failed:")
            || starts_with_ascii_case_insensitive(
                diagnostic_prefix,
                "test-obligation evaluate failed:",
            );
    let cargo_make_failure = starts_with_ascii_case_insensitive(
        diagnostic_prefix,
        "error while executing command, exit code:",
    );
    let compiler_failure = starts_with_ascii_case_insensitive(diagnostic_prefix, "error:")
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "error[")
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "error ");
    let known_non_failure = diagnostic_prefix.eq_ignore_ascii_case("PASS")
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "[pass]")
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "[ok]")
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "[skip]")
        || (starts_with_ascii_case_insensitive(diagnostic_prefix, "--- ")
            && (ends_with_ascii_case_insensitive(diagnostic_suffix, " PASSED ---")
                || ends_with_ascii_case_insensitive(diagnostic_suffix, " SKIPPED ---")))
        || (starts_with_ascii_case_insensitive(diagnostic_prefix, "test ")
            && ends_with_ascii_case_insensitive(diagnostic_suffix, " ... ok"))
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "test result: ok")
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "log: ")
        || diagnostic_prefix.eq_ignore_ascii_case("failures:")
        || diagnostic_prefix.eq_ignore_ascii_case("--- stderr ---")
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "internalrecord")
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "debugrecord")
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "obligationrecord")
        || starts_with_ascii_case_insensitive(diagnostic_prefix, "somerecord");
    aggregate_summary_failure
        || aggregate_summary_failure_detail
        || marked_failure
        || cargo_test_failure
        || cargo_test_result
        || nextest_failure
        || nextest_summary
        || panic_line
        || obligation_failure
        || cargo_make_failure
        || compiler_failure
        || (!known_non_failure && !diagnostic_prefix.is_empty())
}

/// Renders the compact summary from the typed application result.
pub fn render_summary(result: &GateRunResult) -> String {
    match result {
        GateRunResult::ChildExited { exit_code, output, log } => {
            let verdict = if exit_code.is_success() { "PASS" } else { "FAIL" };
            let mut lines = vec![verdict.to_owned(), render_log_outcome(log)];
            if !exit_code.is_success() && matches!(log, GateLogWriteOutcome::Persisted(_)) {
                lines.push("failures:".to_owned());
                let excerpts = failure_excerpts(output);
                if excerpts.is_empty() {
                    lines.push(format!("- child exited with status {}", exit_code.value()));
                } else {
                    lines.extend(excerpts.into_iter().map(|excerpt| format!("- {excerpt}")));
                }
            }
            lines.join("\n")
        }
        GateRunResult::SpawnFailed { error, log } => {
            format!("FAIL\n{}\nfailures:\n{}", render_log_outcome(log), render_spawn_failure(error),)
        }
    }
}

fn render_prepare_failure(error: &GateLogReservationError) -> String {
    format!("FAIL\nlog unavailable: {}", compact_reason(&render_reservation_error_reason(error)))
}

fn render_log_outcome(log: &GateLogWriteOutcome) -> String {
    match log {
        GateLogWriteOutcome::Persisted(path) => {
            format!("log: {}", render_log_path(path.as_path()))
        }
        GateLogWriteOutcome::Unavailable(error) => {
            format!("log unavailable: {}", compact_reason(&render_write_error_reason(error)))
        }
    }
}

fn render_reservation_error_reason(error: &GateLogReservationError) -> String {
    match error {
        GateLogReservationError::OutsideRoot(_) => {
            "gate log path is outside the trusted root".to_owned()
        }
        GateLogReservationError::SymlinkComponent(_) => {
            "gate log path contains a symlink component".to_owned()
        }
        GateLogReservationError::Clock(error) => error.to_string(),
        GateLogReservationError::CreateDirectory(reason)
        | GateLogReservationError::CreateFile(reason)
        | GateLogReservationError::EncodedNameTooLong(reason) => reason.to_string(),
    }
}

fn render_write_error_reason(error: &GateLogWriteError) -> String {
    match error {
        GateLogWriteError::OutsideRoot(_) => "gate log path is outside the trusted root".to_owned(),
        GateLogWriteError::SymlinkComponent(_) => {
            "gate log path contains a symlink component".to_owned()
        }
        GateLogWriteError::Write(reason) => format!("could not write the gate log: {reason}"),
    }
}

fn render_spawn_failure(error: &GateProcessError) -> String {
    let detail = compact_reason(&error.to_string());
    truncate_line(&format!("- could not start child command: {detail}"))
}

fn compact_reason(reason: &str) -> String {
    truncate_line(&reason.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Truncates one presentation line without splitting UTF-8.
pub fn truncate_line(line: &str) -> String {
    if line.len() <= MAX_FAILURE_LINE_BYTES {
        return line.to_owned();
    }

    let mut end = MAX_FAILURE_LINE_BYTES;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", line.get(..end).unwrap_or_default())
}

/// Maps the use-case's OS status to the existing CLI `u8` boundary.
///
/// The existing [`CommandOutcome`] contract supports the inclusive range
/// `0..=255`. Values outside that range are unsupported at this boundary and
/// are deliberately normalized to the generic nonzero failure code. The
/// original `i32` status is retained in the human-readable failure fallback.
fn process_exit_code(code: &GateExitCode) -> u8 {
    u8::try_from(code.value()).unwrap_or(CLI_EXIT_CODE_FALLBACK)
}

/// Renders a persisted path without replacing non-Unicode components.
///
/// A normal path keeps the established display form. An invalid-Unicode path
/// uses `Path`'s escaped `Debug` representation, which is a lossless and
/// unambiguous textual representation suitable for the `String` outcome.
fn render_log_path(path: &Path) -> String {
    match path.to_str() {
        Some(path) if !path.chars().any(char::is_control) => path.to_owned(),
        _ => format!("{path:?}"),
    }
}

fn starts_with_ascii_case_insensitive(value: &str, pattern: &str) -> bool {
    value
        .as_bytes()
        .get(..pattern.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(pattern.as_bytes()))
}

fn ends_with_ascii_case_insensitive(value: &str, pattern: &str) -> bool {
    value
        .as_bytes()
        .get(value.len().saturating_sub(pattern.len())..)
        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(pattern.as_bytes()))
}

fn contains_ascii_case_insensitive(value: &str, pattern: &str) -> bool {
    value
        .as_bytes()
        .windows(pattern.len())
        .any(|candidate| candidate.eq_ignore_ascii_case(pattern.as_bytes()))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use usecase::gate_output::{
        GateAdapterFailureReason, GateLogPath, GateLogPersistencePort, GateLogReservation,
        GateLogWriteError, GateProcessError, GateProcessOutput, GateProcessPort, GateRunError,
        GateRunInteractor,
    };

    struct StubService {
        result: Mutex<Option<Result<GateRunResult, GateRunError>>>,
    }

    impl GateRunService for StubService {
        fn execute(&self, _command: GateRunCommand) -> Result<GateRunResult, GateRunError> {
            self.result
                .lock()
                .expect("service lock should be available")
                .take()
                .expect("service should be invoked once")
        }
    }

    struct RecordingService {
        result: Mutex<Option<Result<GateRunResult, GateRunError>>>,
        seen: Mutex<Option<(String, Vec<OsString>)>>,
    }

    impl GateRunService for RecordingService {
        fn execute(&self, command: GateRunCommand) -> Result<GateRunResult, GateRunError> {
            *self.seen.lock().expect("command lock should be available") =
                Some((command.name().to_owned(), command.command().to_vec()));
            self.result
                .lock()
                .expect("service lock should be available")
                .take()
                .expect("service should be invoked once")
        }
    }

    struct IntegrationRunner {
        result: Mutex<Option<Result<GateProcessOutput, GateProcessError>>>,
    }

    impl GateProcessPort for IntegrationRunner {
        fn run(&self, _command: &GateRunCommand) -> Result<GateProcessOutput, GateProcessError> {
            self.result
                .lock()
                .expect("runner lock should be available")
                .take()
                .expect("runner should be invoked once")
        }
    }

    struct IntegrationLogs {
        path: GateLogPath,
        contents: Mutex<Vec<Vec<u8>>>,
    }

    impl GateLogPersistencePort for IntegrationLogs {
        fn reserve(
            &self,
            _command: &GateRunCommand,
        ) -> Result<GateLogReservation, GateLogReservationError> {
            Ok(GateLogReservation::from_reserved_path(self.path.as_path().to_path_buf()))
        }

        fn persist(
            &self,
            _reservation: GateLogReservation,
            contents: &[u8],
        ) -> Result<GateLogPath, GateLogWriteError> {
            self.contents.lock().expect("log lock should be available").push(contents.to_vec());
            Ok(self.path.clone())
        }
    }

    struct FailingReserveLogs;

    impl GateLogPersistencePort for FailingReserveLogs {
        fn reserve(
            &self,
            _command: &GateRunCommand,
        ) -> Result<GateLogReservation, GateLogReservationError> {
            Err(GateLogReservationError::EncodedNameTooLong(GateAdapterFailureReason::new(
                "encoded name cannot be represented".to_owned(),
            )))
        }

        fn persist(
            &self,
            _reservation: GateLogReservation,
            _contents: &[u8],
        ) -> Result<GateLogPath, GateLogWriteError> {
            panic!("persist must not be called after reservation failure")
        }
    }

    struct FailingWriteLogs;

    impl GateLogPersistencePort for FailingWriteLogs {
        fn reserve(
            &self,
            _command: &GateRunCommand,
        ) -> Result<GateLogReservation, GateLogReservationError> {
            Ok(GateLogReservation::from_reserved_path("tmp/gate/unavailable.log".into()))
        }

        fn persist(
            &self,
            _reservation: GateLogReservation,
            _contents: &[u8],
        ) -> Result<GateLogPath, GateLogWriteError> {
            Err(GateLogWriteError::Write(GateAdapterFailureReason::new("read-only".to_owned())))
        }
    }

    fn integration_driver(
        exit_code: i32,
        output: Vec<u8>,
        log_path: &str,
    ) -> (GateOutputDriver, Arc<IntegrationLogs>) {
        let logs = Arc::new(IntegrationLogs {
            path: GateLogPath::from_persisted_path(log_path.into()),
            contents: Mutex::new(Vec::new()),
        });
        let runner = Arc::new(IntegrationRunner {
            result: Mutex::new(Some(Ok(GateProcessOutput {
                exit_code: GateExitCode::new(exit_code),
                output,
            }))),
        });
        let service: Arc<dyn GateRunService> =
            Arc::new(GateRunInteractor::new(runner, logs.clone()));
        (GateOutputDriver::new(service), logs)
    }

    fn log_path() -> GateLogPath {
        GateLogPath::from_persisted_path("tmp/gate/driver.log".into())
    }

    fn persisted_log() -> GateLogWriteOutcome {
        GateLogWriteOutcome::Persisted(log_path())
    }

    fn driver(result: Result<GateRunResult, GateRunError>) -> GateOutputDriver {
        GateOutputDriver::new(Arc::new(StubService { result: Mutex::new(Some(result)) }))
    }

    fn real_interactor_driver(
        result: Result<GateProcessOutput, GateProcessError>,
    ) -> GateOutputDriver {
        let runner = Arc::new(IntegrationRunner { result: Mutex::new(Some(result)) });
        let service: Arc<dyn GateRunService> =
            Arc::new(GateRunInteractor::new(runner, Arc::new(FailingWriteLogs)));
        GateOutputDriver::new(service)
    }

    #[test]
    fn test_gate_output_driver_rejects_overlong_encoded_name_before_running_child() {
        let runner = Arc::new(IntegrationRunner {
            result: Mutex::new(Some(Ok(GateProcessOutput {
                exit_code: GateExitCode::new(0),
                output: b"child should not run".to_vec(),
            }))),
        });
        let service: Arc<dyn GateRunService> =
            Arc::new(GateRunInteractor::new(runner.clone(), Arc::new(FailingReserveLogs)));

        let outcome = GateOutputDriver::new(service)
            .invoke(GateOutputInput::new("%".repeat(100), vec![OsString::from("check")]));

        assert_eq!(outcome.exit_code, 1);
        let stdout = outcome.stdout.expect("preparation failure should render stdout");
        assert!(stdout.starts_with("FAIL\nlog unavailable: "));
        assert!(stdout.contains("encoded name cannot be represented"));
        assert!(!stdout.contains("log: "));
        assert!(outcome.stderr.is_none());
        assert!(runner.result.lock().expect("runner lock should be available").is_some());
    }

    #[test]
    fn test_gate_output_driver_renders_child_status_when_log_write_is_unavailable() {
        let failure = real_interactor_driver(Ok(GateProcessOutput {
            exit_code: GateExitCode::new(23),
            output: b"[FAIL] item-one: short reason\n".to_vec(),
        }))
        .invoke(GateOutputInput::new(
            "unavailable-failure".to_owned(),
            vec![OsString::from("check")],
        ));

        assert_eq!(failure.exit_code, 23);
        let failure_stdout = failure.stdout.expect("failure should render stdout");
        assert_eq!(
            failure_stdout,
            "FAIL\nlog unavailable: could not write the gate log: read-only"
        );
        assert!(!failure_stdout.contains("failures:"));
        assert!(!failure_stdout.lines().any(|line| line.starts_with("log: ")));

        let success = real_interactor_driver(Ok(GateProcessOutput {
            exit_code: GateExitCode::new(0),
            output: b"success output\n".to_vec(),
        }))
        .invoke(GateOutputInput::new(
            "unavailable-success".to_owned(),
            vec![OsString::from("check")],
        ));

        assert_eq!(success.exit_code, 0);
        let success_stdout = success.stdout.expect("success should render stdout");
        assert!(success_stdout.starts_with("PASS\nlog unavailable: "));
        assert!(!success_stdout.lines().any(|line| line.starts_with("log: ")));

        let spawn = real_interactor_driver(Err(GateProcessError::Spawn(
            GateAdapterFailureReason::new("missing child".to_owned()),
        )))
        .invoke(GateOutputInput::new(
            "unavailable-spawn".to_owned(),
            vec![OsString::from("check")],
        ));

        assert_eq!(spawn.exit_code, 1);
        let spawn_stdout = spawn.stdout.expect("spawn failure should render stdout");
        assert!(spawn_stdout.starts_with("FAIL\nlog unavailable: "));
        assert!(spawn_stdout.contains("failures:\n- could not start child command:"));
        assert!(!spawn_stdout.lines().any(|line| line.starts_with("log: ")));
    }

    #[test]
    fn test_gate_output_driver_suppresses_child_pass_and_debug_records_on_success() {
        let outcome = driver(Ok(GateRunResult::ChildExited {
            exit_code: GateExitCode::new(0),
            output: b"[PASS] item-one\nInternalRecord { Debug: true }\n".to_vec(),
            log: persisted_log(),
        }))
        .invoke(GateOutputInput::new("gate".to_owned(), vec![OsString::from("check")]));

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.expect("success should render stdout");
        assert_eq!(stdout, "PASS\nlog: tmp/gate/driver.log");
        assert!(!stdout.contains("[PASS]"));
        assert!(!stdout.contains("Debug"));
        assert!(outcome.stderr.is_none());
    }

    #[test]
    fn test_render_summary_limits_success_to_verdict_and_log_and_lists_failure_lines() {
        let success = GateRunResult::ChildExited {
            exit_code: GateExitCode::new(0),
            output: b"[PASS] item-one\nInternalRecord { Debug: true }\n".to_vec(),
            log: persisted_log(),
        };
        assert_eq!(render_summary(&success), "PASS\nlog: tmp/gate/driver.log");

        let failure = GateRunResult::ChildExited {
            exit_code: GateExitCode::new(1),
            output: b"[PASS] item-one\n[FAIL] item-two: reason\nInternalRecord { Debug: true }\n"
                .to_vec(),
            log: persisted_log(),
        };
        let rendered = render_summary(&failure);
        assert!(rendered.contains("FAIL\nlog: tmp/gate/driver.log\nfailures:"));
        assert!(rendered.contains("- [FAIL] item-two: reason"));
        assert!(!rendered.contains("item-one"));
        assert!(!rendered.contains("InternalRecord"));
    }

    #[test]
    fn test_render_summary_keeps_child_verdict_when_log_write_is_unavailable() {
        let result = GateRunResult::ChildExited {
            exit_code: GateExitCode::new(0),
            output: Vec::new(),
            log: GateLogWriteOutcome::Unavailable(GateLogWriteError::Write(
                GateAdapterFailureReason::new("read-only".to_owned()),
            )),
        };

        let rendered = render_summary(&result);

        assert_eq!(rendered, "PASS\nlog unavailable: could not write the gate log: read-only");
        assert!(!rendered.lines().any(|line| line.starts_with("log: ")));

        let result = GateRunResult::ChildExited {
            exit_code: GateExitCode::new(23),
            output: b"[FAIL] item-one: short reason\n".to_vec(),
            log: GateLogWriteOutcome::Unavailable(GateLogWriteError::Write(
                GateAdapterFailureReason::new("read-only".to_owned()),
            )),
        };
        let rendered = render_summary(&result);

        assert_eq!(rendered, "FAIL\nlog unavailable: could not write the gate log: read-only");
        assert!(!rendered.contains("failures:"));
        assert!(!rendered.lines().any(|line| line.starts_with("log: ")));

        let result = GateRunResult::SpawnFailed {
            error: GateProcessError::Spawn(GateAdapterFailureReason::new(
                "missing child command".to_owned(),
            )),
            log: GateLogWriteOutcome::Unavailable(GateLogWriteError::Write(
                GateAdapterFailureReason::new("read-only".to_owned()),
            )),
        };
        let rendered = render_summary(&result);

        assert!(
            rendered.starts_with("FAIL\nlog unavailable: could not write the gate log: read-only")
        );
        assert!(rendered.contains("failures:\n- could not start child command: "));
        assert!(!rendered.lines().any(|line| line.starts_with("log: ")));
    }

    #[test]
    fn test_gate_output_driver_preserves_child_status_when_log_write_is_unavailable() {
        let failure = driver(Ok(GateRunResult::ChildExited {
            exit_code: GateExitCode::new(23),
            output: b"[FAIL] item-one: short reason\n".to_vec(),
            log: GateLogWriteOutcome::Unavailable(GateLogWriteError::Write(
                GateAdapterFailureReason::new("read-only".to_owned()),
            )),
        }))
        .invoke(GateOutputInput::new("gate-failure".to_owned(), vec![OsString::from("check")]));

        assert_eq!(failure.exit_code, 23);
        let failure_stdout = failure.stdout.expect("failure should render stdout");
        assert_eq!(
            failure_stdout,
            "FAIL\nlog unavailable: could not write the gate log: read-only"
        );
        assert!(!failure_stdout.contains("failures:"));
        assert!(!failure_stdout.lines().any(|line| line.starts_with("log: ")));

        let success = driver(Ok(GateRunResult::ChildExited {
            exit_code: GateExitCode::new(0),
            output: b"success output\n".to_vec(),
            log: GateLogWriteOutcome::Unavailable(GateLogWriteError::Write(
                GateAdapterFailureReason::new("read-only".to_owned()),
            )),
        }))
        .invoke(GateOutputInput::new("gate-success".to_owned(), vec![OsString::from("check")]));

        assert_eq!(success.exit_code, 0);
        let success_stdout = success.stdout.expect("success should render stdout");
        assert_eq!(
            success_stdout,
            "PASS\nlog unavailable: could not write the gate log: read-only"
        );
        assert!(!success_stdout.lines().any(|line| line.starts_with("log: ")));
    }

    #[test]
    fn test_gate_output_driver_propagates_nonzero_exit_and_renders_all_declared_failure_excerpts() {
        let output = std::iter::once("[PASS] item-pass".to_owned())
            .chain(std::iter::once("[FAIL] item-primary: short reason".to_owned()))
            .chain((0..9).map(|index| format!("[FAIL] item-{index}: {}", "x".repeat(300))))
            .chain(std::iter::once("InternalRecord { Debug: true }".to_owned()))
            .collect::<Vec<_>>()
            .join("\n");
        let outcome = driver(Ok(GateRunResult::ChildExited {
            exit_code: GateExitCode::new(23),
            output: output.into_bytes(),
            log: persisted_log(),
        }))
        .invoke(GateOutputInput::new("gate".to_owned(), vec![OsString::from("check")]));

        assert_eq!(outcome.exit_code, 23);
        let stdout = outcome.stdout.expect("failure should render stdout");
        assert!(stdout.starts_with("FAIL\nlog: tmp/gate/driver.log\nfailures:"));
        assert_eq!(stdout.lines().count(), 3 + MAX_FAILURE_EXCERPTS);
        assert!(stdout.contains("log: tmp/gate/driver.log"));
        assert!(stdout.contains("- [FAIL] item-primary: short reason"));
        let bounded_item = truncate_line(&format!("[FAIL] item-6: {}", "x".repeat(300)));
        assert!(stdout.contains(&format!("- {bounded_item}")));
        assert!(!stdout.contains("item-7"));
        assert!(!stdout.contains("item-8"));
        assert!(
            stdout.lines().skip(3).all(|line| line.len() <= MAX_FAILURE_LINE_BYTES + "…".len() + 2)
        );
        assert!(!stdout.contains("item-pass"));
        assert!(!stdout.contains("InternalRecord"));
        assert!(outcome.stderr.is_none());
    }

    #[test]
    fn test_gate_output_driver_forwards_child_command_and_renders_shared_summary_contract() {
        let service = Arc::new(RecordingService {
            result: Mutex::new(Some(Ok(GateRunResult::ChildExited {
                exit_code: GateExitCode::new(23),
                output: b"[PASS] aggregate-pass\n[FAIL] aggregate-item: short reason\nInternalRecord { Debug: true }\n"
                    .to_vec(),
                log: persisted_log(),
            }))),
            seen: Mutex::new(None),
        });
        let driver = GateOutputDriver::new(service.clone());
        let command = vec![
            OsString::from("/bin/sh"),
            OsString::from("-c"),
            OsString::from("printf '[FAIL] aggregate-item: short reason\\n'; exit 23"),
        ];

        let outcome =
            driver.invoke(GateOutputInput::new("aggregate-check".to_owned(), command.clone()));

        assert_eq!(outcome.exit_code, 23);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "FAIL\nlog: tmp/gate/driver.log\nfailures:\n- [FAIL] aggregate-item: short reason"
            )
        );
        assert!(outcome.stderr.is_none());
        assert_eq!(
            service.seen.lock().expect("command lock should be available").as_ref(),
            Some(&("aggregate-check".to_owned(), command))
        );
    }

    #[test]
    fn test_gate_run_interactor_service_renders_failure_excerpt_from_persisted_output() {
        let output = b"[PASS] item-pass\n[FAIL] item-primary: short reason\nInternalRecord { Debug: true }\n"
            .to_vec();
        let (driver, logs) =
            integration_driver(23, output.clone(), "tmp/gate/integration-failure.log");

        let outcome = driver.invoke(GateOutputInput::new(
            "integration-failure".to_owned(),
            vec![OsString::from("check")],
        ));

        assert_eq!(outcome.exit_code, 23);
        let stdout = outcome.stdout.expect("failure should render stdout");
        assert_eq!(
            stdout,
            "FAIL\nlog: tmp/gate/integration-failure.log\nfailures:\n- [FAIL] item-primary: short reason"
        );
        assert!(!stdout.contains("item-pass"));
        assert!(!stdout.contains("InternalRecord"));
        assert_eq!(
            logs.contents.lock().expect("log lock should be available").as_slice(),
            [output]
        );
    }

    #[test]
    fn test_gate_run_interactor_service_renders_success_summary_without_child_records() {
        let output = b"[PASS] item-pass\nDebugRecord { internal: true }\n".to_vec();
        let (driver, logs) =
            integration_driver(0, output.clone(), "tmp/gate/integration-success.log");

        let outcome = driver.invoke(GateOutputInput::new(
            "integration-success".to_owned(),
            vec![OsString::from("check")],
        ));

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.expect("success should render stdout");
        assert_eq!(stdout, "PASS\nlog: tmp/gate/integration-success.log");
        assert!(!stdout.contains("item-pass"));
        assert!(!stdout.contains("DebugRecord"));
        assert_eq!(
            logs.contents.lock().expect("log lock should be available").as_slice(),
            [output]
        );
    }

    #[test]
    fn test_gate_output_driver_renders_spawn_failure_and_service_failure_separately() {
        let spawn = driver(Ok(GateRunResult::SpawnFailed {
            error: GateProcessError::Spawn(GateAdapterFailureReason::new("missing".to_owned())),
            log: persisted_log(),
        }))
        .invoke(GateOutputInput::new("gate".to_owned(), vec![OsString::from("check")]));
        assert_eq!(spawn.exit_code, 1);
        assert!(spawn.stdout.expect("spawn failure should render stdout").contains("missing"));

        let preparation_failure =
            driver(Err(GateRunError::Prepare(GateLogReservationError::CreateDirectory(
                GateAdapterFailureReason::new("read-only".to_owned()),
            ))))
            .invoke(GateOutputInput::new("gate".to_owned(), vec![OsString::from("check")]));
        assert_eq!(preparation_failure.exit_code, 1);
        assert!(
            preparation_failure
                .stdout
                .expect("preparation failure should use stdout")
                .contains("read-only")
        );
    }

    #[test]
    fn test_spawn_failure_summary_compacts_the_reason_to_one_line() {
        let reason = format!("first line\n{}\nlast line", "x".repeat(300));
        let summary = render_summary(&GateRunResult::SpawnFailed {
            error: GateProcessError::Spawn(GateAdapterFailureReason::new(reason)),
            log: persisted_log(),
        });

        assert_eq!(summary.lines().count(), 4);
        let failure_line = summary.lines().nth(3).expect("spawn failure line should exist");
        assert!(failure_line.starts_with("- could not start child command: "));
        assert!(failure_line.contains("first line"));
        assert!(!failure_line.contains("last line"));
        assert!(failure_line.len() <= MAX_FAILURE_LINE_BYTES + "…".len() + 2);
        assert!(failure_line.ends_with('…'));
    }

    #[test]
    fn test_unrepresentable_exit_code_uses_generic_failure_at_cli_boundary() {
        let outcome = driver(Ok(GateRunResult::ChildExited {
            exit_code: GateExitCode::new(256),
            output: Vec::new(),
            log: persisted_log(),
        }))
        .invoke(GateOutputInput::new("gate".to_owned(), vec![OsString::from("check")]));

        assert_eq!(process_exit_code(&GateExitCode::new(255)), 255);
        assert_eq!(process_exit_code(&GateExitCode::new(256)), 1);
        assert_eq!(process_exit_code(&GateExitCode::new(-1)), 1);
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some("FAIL\nlog: tmp/gate/driver.log\nfailures:\n- child exited with status 256")
        );
    }

    #[test]
    fn test_failure_helpers_accept_gate_formats_and_reject_internal_records() {
        for line in [
            "test cli::tests::broken ... FAILED",
            "test result: FAILED. 1 passed; 1 failed",
            "    FAIL [   0.012s] cli::tests::broken",
            "test-obligation check failed: missing binding",
            "test-obligation evaluate failed: verifier unavailable",
            "[FAIL] pre-commit: check failed",
            "[BLOCKED] review scope has findings",
            "Error while executing command, exit code: 23",
            "thread 'broken' panicked at src/lib.rs:1:1",
            "- child exited with status 256",
            "- could not start child command: missing",
        ] {
            assert!(is_failure_line(line), "expected failure line: {line}");
        }
        for line in [
            "test cli::tests::healthy ... ok",
            "test result: ok. 1 passed; 0 failed",
            "[PASS] item-pass",
            "[OK] All checks passed.",
            "[SKIP] not applicable",
            "--- signal check PASSED ---",
            "--- signal check SKIPPED ---",
            "PASS",
            "SomeRecord { status: failed_in_a_field }",
        ] {
            assert!(!is_failure_line(line), "unexpected failure line: {line}");
        }
    }

    #[test]
    fn test_failure_excerpts_exclude_internal_records_longer_than_diagnostic_prefix() {
        let internal_record = format!("InternalRecord {{ details: {} }}", "x".repeat(240));
        let output = format!("[FAIL] item-one: short reason\n{internal_record}\n");

        assert!(!is_failure_line(&internal_record));
        assert_eq!(
            failure_excerpts(output.as_bytes()),
            ["[FAIL] item-one: short reason".to_owned()]
        );
    }

    #[test]
    fn test_failure_helpers_include_unrecognized_non_empty_lines() {
        for line in [
            "new gate format: item did not complete",
            "a sentence mentions failed output but is not a gate diagnostic",
            "warning: unused variable: `value`",
            "Compiling gate-check v1.0.0",
        ] {
            assert!(is_failure_line(line), "unrecognized line was excluded: {line}");
        }
        assert!(!is_failure_line("   "));
    }

    #[test]
    fn test_render_log_errors_omit_paths_from_prepare_and_unavailable_reasons() {
        let path = std::path::PathBuf::from("tmp/gate/private.log");

        let prepare = render_prepare_failure(&GateLogReservationError::OutsideRoot(path.clone()));
        assert_eq!(prepare, "FAIL\nlog unavailable: gate log path is outside the trusted root");
        assert!(!prepare.contains(path.to_string_lossy().as_ref()));

        let unavailable = render_log_outcome(&GateLogWriteOutcome::Unavailable(
            GateLogWriteError::SymlinkComponent(path.clone()),
        ));
        assert_eq!(unavailable, "log unavailable: gate log path contains a symlink component");
        assert!(!unavailable.contains(path.to_string_lossy().as_ref()));
    }

    #[test]
    fn test_failure_excerpts_reject_control_styling_without_interpretation() {
        let output = b"\x1b[1m\x1b[91merror\x1b[0m: compiler failure\n\x1b[31mtest cli::tests::colored ... FAILED\x1b[0m\n";

        assert!(!is_failure_line("\x1b[1m\x1b[91merror\x1b[0m: compiler failure"));
        assert!(!is_failure_line("\x1b[31mtest cli::tests::colored ... FAILED\x1b[0m"));
        assert!(failure_excerpts(output).is_empty());
    }

    #[test]
    fn test_failure_excerpts_keep_nested_gate_summary_verdict_and_item() {
        let output =
            b"FAIL\nlog: tmp/gate/inner.log\nfailures:\n- FAIL [   0.012s] cli::tests::broken\n";

        assert_eq!(
            failure_excerpts(output),
            ["FAIL".to_owned(), "- FAIL [   0.012s] cli::tests::broken".to_owned(),]
        );
    }

    #[test]
    fn test_failure_excerpts_keep_nested_gate_summary_fallback_reasons() {
        for (line, expected) in [
            ("- child exited with status 256", "- child exited with status 256"),
            (
                "- could not start child command: missing",
                "- could not start child command: missing",
            ),
        ] {
            let output = format!("FAIL\nlog: tmp/gate/inner.log\nfailures:\n{line}\n");

            assert_eq!(
                failure_excerpts(output.as_bytes()),
                ["FAIL".to_owned(), expected.to_owned()]
            );
        }
    }

    #[test]
    fn test_truncate_line_bounds_long_reason_without_splitting_utf8() {
        let line = format!("[FAIL] {}", "あ".repeat(240));

        let truncated = truncate_line(&line);
        let prefix = truncated.strip_suffix('…').expect("long line should be truncated");
        assert!(prefix.len() <= MAX_FAILURE_LINE_BYTES);
        assert!(truncated.len() <= MAX_FAILURE_LINE_BYTES + "…".len());
        assert!(prefix.is_char_boundary(prefix.len()));
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn test_failure_excerpts_bounds_oversized_failure_reason() {
        let line = "[FAIL] aggregate-item: short reason ".to_owned() + &"x".repeat(300);

        let excerpts = failure_excerpts(line.as_bytes());

        assert_eq!(excerpts, [truncate_line(&line)]);
        assert!(excerpts.first().expect("oversized failure excerpt should exist").ends_with('…'));
    }

    #[test]
    fn test_failure_excerpts_selects_failed_items_and_short_reasons() {
        let output = b"[PASS] item-pass\n[FAIL] item-one: short reason\nInternalRecord { Debug: true }\n[FAIL] item-two: another reason\n";

        assert_eq!(
            failure_excerpts(output),
            [
                "[FAIL] item-one: short reason".to_owned(),
                "[FAIL] item-two: another reason".to_owned(),
            ]
        );
    }

    #[test]
    fn test_failure_excerpts_ignore_non_ascii_and_non_lf_records() {
        let output = b"[FAIL] first: reason\r[FAIL] invalid: \xff reason\n[FAIL] third: reason\n";

        assert_eq!(failure_excerpts(output), ["[FAIL] third: reason".to_owned()]);
    }

    #[test]
    fn test_failure_excerpts_apply_only_the_explicit_item_count_cap() {
        let output = (0..(MAX_FAILURE_EXCERPTS + 2))
            .map(|index| format!("[FAIL] item-{index}: reason"))
            .collect::<Vec<_>>()
            .join("\n");

        let excerpts = failure_excerpts(output.as_bytes());

        assert_eq!(excerpts.len(), MAX_FAILURE_EXCERPTS);
        assert!(excerpts.iter().any(|excerpt| excerpt.contains("item-7")));
        assert!(!excerpts.iter().any(|excerpt| excerpt.contains("item-8")));
    }

    #[cfg(unix)]
    #[test]
    fn test_render_summary_uses_escaped_lossless_non_unicode_log_path() {
        use std::os::unix::ffi::OsStringExt;
        use std::path::PathBuf;

        let path = PathBuf::from(OsString::from_vec(b"tmp/gate/log-\xff.log".to_vec()));
        let result = GateRunResult::ChildExited {
            exit_code: GateExitCode::new(0),
            output: Vec::new(),
            log: GateLogWriteOutcome::Persisted(GateLogPath::from_persisted_path(path.clone())),
        };

        assert_eq!(render_log_path(&path), format!("{path:?}"));
        assert_eq!(render_summary(&result), format!("PASS\nlog: {path:?}"));
        assert!(!render_summary(&result).contains('\u{FFFD}'));
    }

    #[test]
    fn test_render_log_path_escapes_control_characters_before_summary_rendering() {
        let path = Path::new("tmp/gate/log\nwith\rcontrol\u{1b}.log");
        let rendered = render_log_path(path);

        assert_eq!(rendered, format!("{path:?}"));
        assert!(!rendered.contains('\n'));
        assert!(!rendered.contains('\r'));
        assert!(!rendered.contains('\u{1b}'));
    }
}
