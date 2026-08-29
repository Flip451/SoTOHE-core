//! Primary adapter and summary helpers for the `gate-output` command.
//!
//! Child output is an opaque byte stream at the use-case boundary. The driver
//! applies these presentation policies before interpreting it: at most 64 KiB
//! is inspected (the head and tail are retained when the stream is larger),
//! bytes are decoded as UTF-8 with replacement for an invalid sequence, and
//! LF, CRLF, and lone CR delimit diagnostic lines. The line classifier accepts
//! only the documented, line-oriented gate formats below; unknown encodings
//! and unknown formats remain available in the full persisted log but are not
//! copied to stdout.

use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;

use usecase::gate_output::{
    GateExitCode, GateProcessError, GateRunCommand, GateRunResult, GateRunService,
};

use crate::render::CommandOutcome;

const MAX_FAILURE_EXCERPT_LINES: usize = 8;
const MAX_FAILURE_LINE_BYTES: usize = 240;
const MAX_FAILURE_INPUT_BYTES: usize = 64 * 1024;
const FAILURE_HEAD_BYTES: usize = MAX_FAILURE_INPUT_BYTES / 2;
const FAILURE_TAIL_BYTES: usize = MAX_FAILURE_INPUT_BYTES - FAILURE_HEAD_BYTES;
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
            Err(error) => CommandOutcome::failure(Some(error.to_string())),
        }
    }
}

/// Selects bounded failure lines from opaque child output.
///
/// The inspected input budget is limited to `MAX_FAILURE_INPUT_BYTES` bytes
/// before decoding. A larger stream contributes equal head and tail windows;
/// its middle is intentionally omitted so late stderr diagnostics remain
/// discoverable without making processing depend on total output size. UTF-8
/// is the supported presentation encoding; malformed sequences are replaced
/// with `U+FFFD`. Both Unix-style and Windows-style line endings, as well as
/// lone carriage returns, are accepted.
pub fn failure_excerpts(output: &[u8]) -> Vec<String> {
    if output.len() <= MAX_FAILURE_INPUT_BYTES {
        return failure_excerpts_in_window(output);
    }

    let head = output.get(..FAILURE_HEAD_BYTES).unwrap_or_default();
    let tail_start = output.len().saturating_sub(FAILURE_TAIL_BYTES);
    let tail = output.get(tail_start..).unwrap_or_default();
    let mut excerpts = failure_excerpts_in_window(head);
    if excerpts.len() < MAX_FAILURE_EXCERPT_LINES {
        excerpts.extend(
            failure_excerpts_in_window(tail)
                .into_iter()
                .take(MAX_FAILURE_EXCERPT_LINES - excerpts.len()),
        );
    }
    excerpts
}

fn failure_excerpts_in_window(output: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(output)
        .split(['\n', '\r'])
        .filter_map(|line| {
            let trimmed = line.trim();
            if is_failure_line(trimmed) { Some(truncate_line(trimmed)) } else { None }
        })
        .take(MAX_FAILURE_EXCERPT_LINES)
        .collect()
}

/// Classifies the supported test, obligation, and aggregate gate diagnostics.
///
/// The accepted grammar is deliberately narrow and line-oriented: project
/// markers (`[FAIL]`, `[ERROR]`, `[BLOCKED]`), the stable failure prefixes and
/// suffixes emitted by the supported Cargo/nextest/rustc commands, and the
/// project-owned `test-obligation`/`cargo-make` failure prefixes. This function
/// does not treat arbitrary occurrences of the word `failed` as diagnostics.
pub fn is_failure_line(line: &str) -> bool {
    let prefix = bounded_trimmed_prefix(line);
    let suffix = bounded_trimmed_suffix(line);
    let marked_failure = starts_with_ascii_case_insensitive(prefix, "[fail]")
        || starts_with_ascii_case_insensitive(prefix, "[error]")
        || starts_with_ascii_case_insensitive(prefix, "[blocked]");
    let cargo_test_failure = starts_with_ascii_case_insensitive(prefix, "test ")
        && ends_with_ascii_case_insensitive(suffix, " ... failed");
    let cargo_test_result = starts_with_ascii_case_insensitive(prefix, "test result: failed");
    let nextest_failure = starts_with_ascii_case_insensitive(prefix, "fail [");
    let nextest_summary = starts_with_ascii_case_insensitive(prefix, "summary ")
        && prefix.split_ascii_whitespace().any(|word| {
            word.eq_ignore_ascii_case("failed")
                || word
                    .get(.."failed,".len())
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case("failed,"))
        });
    let panic_line = starts_with_ascii_case_insensitive(prefix, "thread '")
        && contains_ascii_case_insensitive(prefix, "' panicked at ");
    let obligation_failure =
        starts_with_ascii_case_insensitive(prefix, "test-obligation check failed:")
            || starts_with_ascii_case_insensitive(prefix, "test-obligation evaluate failed:");
    let cargo_make_failure =
        starts_with_ascii_case_insensitive(prefix, "error while executing command, exit code:");
    let compiler_failure = starts_with_ascii_case_insensitive(prefix, "error:")
        || starts_with_ascii_case_insensitive(prefix, "error[")
        || starts_with_ascii_case_insensitive(prefix, "error ");

    marked_failure
        || cargo_test_failure
        || cargo_test_result
        || nextest_failure
        || nextest_summary
        || panic_line
        || obligation_failure
        || cargo_make_failure
        || compiler_failure
}

/// Renders the compact summary from the typed application result.
pub fn render_summary(result: &GateRunResult) -> String {
    match result {
        GateRunResult::ChildExited { exit_code, output, log_path } => {
            let verdict = if exit_code.is_success() { "PASS" } else { "FAIL" };
            let mut lines =
                vec![verdict.to_owned(), format!("log: {}", render_log_path(log_path.as_path()))];
            if !exit_code.is_success() {
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
        GateRunResult::SpawnFailed { error, log_path } => {
            format!(
                "FAIL\nlog: {}\nfailures:\n{}",
                render_log_path(log_path.as_path()),
                render_spawn_failure(error),
            )
        }
    }
}

fn render_spawn_failure(error: &GateProcessError) -> String {
    let detail = error.to_string().split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_line(&format!("- could not start child command: {detail}"))
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

fn bounded_trimmed_prefix(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut start = 0;
    while start < bytes.len()
        && start < MAX_FAILURE_LINE_BYTES
        && bytes.get(start).is_some_and(|byte| byte.is_ascii_whitespace())
    {
        start += 1;
    }

    let mut end = (start + MAX_FAILURE_LINE_BYTES).min(bytes.len());
    while end > start && !line.is_char_boundary(end) {
        end -= 1;
    }
    line.get(start..end).unwrap_or_default()
}

fn bounded_trimmed_suffix(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut end = bytes.len();
    let mut whitespace_bytes = 0;
    while end > 0
        && whitespace_bytes < MAX_FAILURE_LINE_BYTES
        && bytes.get(end - 1).is_some_and(|byte| byte.is_ascii_whitespace())
    {
        end -= 1;
        whitespace_bytes += 1;
    }

    let mut start = end.saturating_sub(MAX_FAILURE_LINE_BYTES);
    while start < end && !line.is_char_boundary(start) {
        start += 1;
    }
    line.get(start..end).unwrap_or_default()
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
        GateAdapterFailureReason, GateLogPath, GateLogPersistenceError, GateLogPersistencePort,
        GateProcessError, GateProcessOutput, GateProcessPort, GateRunError, GateRunInteractor,
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
        fn persist(
            &self,
            _command: &GateRunCommand,
            contents: &[u8],
        ) -> Result<GateLogPath, GateLogPersistenceError> {
            self.contents.lock().expect("log lock should be available").push(contents.to_vec());
            Ok(self.path.clone())
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

    fn driver(result: Result<GateRunResult, GateRunError>) -> GateOutputDriver {
        GateOutputDriver::new(Arc::new(StubService { result: Mutex::new(Some(result)) }))
    }

    #[test]
    fn test_gate_output_driver_suppresses_child_pass_and_debug_records_on_success() {
        let outcome = driver(Ok(GateRunResult::ChildExited {
            exit_code: GateExitCode::new(0),
            output: b"[PASS] item-one\nInternalRecord { Debug: true }\n".to_vec(),
            log_path: log_path(),
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
            log_path: log_path(),
        };
        assert_eq!(render_summary(&success), "PASS\nlog: tmp/gate/driver.log");

        let failure = GateRunResult::ChildExited {
            exit_code: GateExitCode::new(1),
            output: b"[PASS] item-one\n[FAIL] item-two: reason\nInternalRecord { Debug: true }\n"
                .to_vec(),
            log_path: log_path(),
        };
        let rendered = render_summary(&failure);
        assert!(rendered.contains("FAIL\nlog: tmp/gate/driver.log\nfailures:"));
        assert!(rendered.contains("- [FAIL] item-two: reason"));
        assert!(!rendered.contains("item-one"));
        assert!(!rendered.contains("InternalRecord"));
    }

    #[test]
    fn test_gate_output_driver_propagates_nonzero_exit_and_renders_bounded_failure_excerpt() {
        let output = std::iter::once("[PASS] item-pass".to_owned())
            .chain(std::iter::once("[FAIL] item-primary: short reason".to_owned()))
            .chain(
                (0..(MAX_FAILURE_EXCERPT_LINES + 1))
                    .map(|index| format!("[FAIL] item-{index}: {}", "x".repeat(300))),
            )
            .chain(std::iter::once("unrelated diagnostic detail".to_owned()))
            .chain(std::iter::once("InternalRecord { Debug: true }".to_owned()))
            .collect::<Vec<_>>()
            .join("\n");
        let outcome = driver(Ok(GateRunResult::ChildExited {
            exit_code: GateExitCode::new(23),
            output: output.into_bytes(),
            log_path: log_path(),
        }))
        .invoke(GateOutputInput::new("gate".to_owned(), vec![OsString::from("check")]));

        assert_eq!(outcome.exit_code, 23);
        let stdout = outcome.stdout.expect("failure should render stdout");
        assert!(stdout.starts_with("FAIL\nlog: tmp/gate/driver.log\nfailures:"));
        assert_eq!(stdout.lines().count(), MAX_FAILURE_EXCERPT_LINES + 3);
        assert!(stdout.lines().skip(3).all(|line| line.len() <= MAX_FAILURE_LINE_BYTES + 5));
        assert!(stdout.contains("- [FAIL] item-primary: short reason"));
        assert!(!stdout.contains("item-pass"));
        assert!(!stdout.contains("unrelated diagnostic detail"));
        assert!(!stdout.contains("InternalRecord"));
        assert!(outcome.stderr.is_none());
    }

    #[test]
    fn test_gate_output_driver_forwards_child_command_and_renders_shared_summary_contract() {
        let service = Arc::new(RecordingService {
            result: Mutex::new(Some(Ok(GateRunResult::ChildExited {
                exit_code: GateExitCode::new(23),
                output: b"[PASS] aggregate-pass\n[FAIL] aggregate-item: short reason\nfull diagnostic detail\n"
                    .to_vec(),
                log_path: log_path(),
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
        let output =
            b"[PASS] item-pass\n[FAIL] item-primary: short reason\nfull diagnostic detail\n"
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
        assert!(!stdout.contains("full diagnostic detail"));
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
            log_path: log_path(),
        }))
        .invoke(GateOutputInput::new("gate".to_owned(), vec![OsString::from("check")]));
        assert_eq!(spawn.exit_code, 1);
        assert!(spawn.stdout.expect("spawn failure should render stdout").contains("missing"));

        let service_failure = driver(Err(GateRunError::Persist(
            usecase::gate_output::GateLogPersistenceError::Write(GateAdapterFailureReason::new(
                "read-only".to_owned(),
            )),
        )))
        .invoke(GateOutputInput::new("gate".to_owned(), vec![OsString::from("check")]));
        assert_eq!(service_failure.exit_code, 1);
        assert!(
            service_failure
                .stderr
                .expect("service failure should use stderr")
                .contains("read-only")
        );
    }

    #[test]
    fn test_spawn_failure_summary_is_one_bounded_line() {
        let reason = format!("first line\n{}\nlast line", "x".repeat(MAX_FAILURE_LINE_BYTES));
        let summary = render_summary(&GateRunResult::SpawnFailed {
            error: GateProcessError::Spawn(GateAdapterFailureReason::new(reason)),
            log_path: log_path(),
        });

        assert_eq!(summary.lines().count(), 4);
        let failure_line = summary.lines().nth(3).expect("spawn failure line should exist");
        assert!(failure_line.starts_with("- could not start child command: "));
        assert!(failure_line.contains("first line"));
        assert!(failure_line.len() <= MAX_FAILURE_LINE_BYTES + "…".len());
        assert!(failure_line.ends_with('…'));
    }

    #[test]
    fn test_unrepresentable_exit_code_uses_generic_failure_at_cli_boundary() {
        let outcome = driver(Ok(GateRunResult::ChildExited {
            exit_code: GateExitCode::new(256),
            output: Vec::new(),
            log_path: log_path(),
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
        ] {
            assert!(is_failure_line(line), "expected failure line: {line}");
        }
        for line in [
            "test cli::tests::healthy ... ok",
            "[PASS] item-pass",
            "SomeRecord { status: failed_in_a_field }",
            "a sentence mentions failed output but is not a gate diagnostic",
        ] {
            assert!(!is_failure_line(line), "unexpected failure line: {line}");
        }
    }

    #[test]
    fn test_truncate_line_preserves_utf8_boundary_and_configured_byte_limit() {
        let line = format!("[FAIL] {}", "あ".repeat(MAX_FAILURE_LINE_BYTES));
        let truncated = truncate_line(&line);
        let prefix = truncated.strip_suffix('…').expect("long line should be truncated");
        assert!(prefix.len() <= MAX_FAILURE_LINE_BYTES);
        assert!(truncated.len() <= MAX_FAILURE_LINE_BYTES + "…".len());
        assert!(prefix.is_char_boundary(prefix.len()));
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn test_truncate_line_bounds_failed_item_and_short_reason_excerpt() {
        let line =
            format!("[FAIL] aggregate-item: short reason {}", "あ".repeat(MAX_FAILURE_LINE_BYTES));

        let truncated = truncate_line(&line);
        let excerpts = failure_excerpts(line.as_bytes());

        assert!(truncated.starts_with("[FAIL] aggregate-item: short reason"));
        assert!(truncated.ends_with('…'));
        assert!(truncated.len() <= MAX_FAILURE_LINE_BYTES + "…".len());
        assert_eq!(excerpts, [truncated]);
    }

    #[test]
    fn test_failure_excerpts_selects_failed_items_and_short_reasons() {
        let output = b"[PASS] item-pass\n[FAIL] item-one: short reason\nunrelated detail\n[FAIL] item-two: another reason\nInternalRecord { Debug: true }\n";

        assert_eq!(
            failure_excerpts(output),
            [
                "[FAIL] item-one: short reason".to_owned(),
                "[FAIL] item-two: another reason".to_owned(),
            ]
        );
    }

    #[test]
    fn test_failure_excerpts_apply_utf8_replacement_and_all_supported_line_endings() {
        let output = b"[FAIL] first: reason\r[FAIL] invalid: \xff reason\n[FAIL] third: reason\r\n";

        assert_eq!(
            failure_excerpts(output),
            [
                "[FAIL] first: reason".to_owned(),
                "[FAIL] invalid: � reason".to_owned(),
                "[FAIL] third: reason".to_owned(),
            ]
        );
    }

    #[test]
    fn test_failure_excerpts_use_a_bounded_head_and_tail_budget() {
        let mut output = vec![b' '; MAX_FAILURE_INPUT_BYTES];
        output.extend_from_slice(b"[FAIL] middle-window: reason");
        output.resize(MAX_FAILURE_INPUT_BYTES + FAILURE_TAIL_BYTES, b' ');
        output.extend_from_slice(b"\n[FAIL] tail-window: reason");

        assert_eq!(failure_excerpts(&output), ["[FAIL] tail-window: reason".to_owned()]);
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
            log_path: GateLogPath::from_persisted_path(path.clone()),
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
