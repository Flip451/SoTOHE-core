use std::time::Duration;

use usecase::ref_verify::RefVerifyError;

/// Backoff schedule used by [`run_process_retryable`] when a transient
/// verifier-subprocess failure is detected.
///
/// The slice length determines the maximum number of *retries*, so total
/// attempts is `DEFAULT_TRANSIENT_BACKOFFS.len() + 1` (3). Values are the
/// wait before attempts 2 and 3 respectively; conservative because the
/// documented failure modes (provider "at capacity", codex launcher race)
/// need seconds of clearance, not sub-second jitter.
pub(super) const DEFAULT_TRANSIENT_BACKOFFS: &[Duration] =
    &[Duration::from_secs(2), Duration::from_secs(8)];

/// Substrings that classify a `VerifierPort` error message as transient and
/// therefore retriable. Matched case-insensitively against the combined
/// exit-status + stderr-tail message.
///
/// Conservative allowlist per the ops brief: only errors that come from
/// provider capacity, rate limiting, network flakes, or the codex launcher
/// race qualify. Auth failures, unsupported-model errors, schema/parse
/// rejections, sandbox denials, and empty-output-on-success cases are
/// intentionally excluded so a persistent bug does not silently retry.
const TRANSIENT_MARKERS: &[&str] = &[
    "at capacity",
    "rate limit",
    "429",
    "too many requests",
    "overloaded",
    "timeout",
    "timed out",
    "connection",
    "network",
    "no executable codex found for current version",
];

/// True when `message` contains any of [`TRANSIENT_MARKERS`], case-insensitive.
fn is_transient_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    TRANSIENT_MARKERS.iter().any(|marker| lower.contains(marker))
}

/// True when `err` is a `VerifierPort` whose message matches a transient
/// marker; every other variant / non-matching message is treated as fatal.
fn is_transient_verifier_error(err: &RefVerifyError) -> bool {
    match err {
        RefVerifyError::VerifierPort { message } => is_transient_message(message),
        _ => false,
    }
}

/// Append `"(after {attempts} attempts)"` to a `VerifierPort` message so the
/// exhausted-retry diagnostic tells the operator how many times we tried.
fn annotate_verifier_error_with_attempts(err: RefVerifyError, attempts: usize) -> RefVerifyError {
    match err {
        RefVerifyError::VerifierPort { message } => RefVerifyError::VerifierPort {
            message: format!("{message} (after {attempts} attempts)"),
        },
        other => other,
    }
}

/// Retry `attempt_fn` on transient `VerifierPort` errors, sleeping via
/// `sleep_fn` between attempts using the supplied backoff schedule.
///
/// Total attempts equal `backoffs.len() + 1`. Non-transient errors and
/// successes short-circuit immediately. When retries are exhausted on a still-
/// transient error, the returned `VerifierPort` message is annotated with the
/// attempt count so the operator can distinguish exhausted-retry failures from
/// a first-shot fatal error.
///
/// `sleep_fn` is a seam so unit tests can drive the loop without wall-clock
/// waiting; the production caller wires it to `std::thread::sleep`.
pub(super) fn retry_transient<T, F, S>(
    backoffs: &[Duration],
    mut attempt_fn: F,
    mut sleep_fn: S,
) -> Result<T, RefVerifyError>
where
    F: FnMut() -> Result<T, RefVerifyError>,
    S: FnMut(Duration),
{
    // Total attempts = `backoffs.len() + 1`. Each element of `backoffs`
    // schedules the sleep AFTER the corresponding attempt fails transiently
    // and BEFORE the next attempt. The final attempt runs without a
    // trailing sleep.
    let total_attempts = backoffs.len() + 1;
    let mut attempts_taken = 0_usize;
    for backoff in backoffs.iter().copied() {
        attempts_taken += 1;
        match attempt_fn() {
            Ok(out) => return Ok(out),
            Err(err) if is_transient_verifier_error(&err) => {
                sleep_fn(backoff);
                continue;
            }
            Err(err) => return Err(err),
        }
    }
    // Final attempt: no more retries queued after this call.
    attempts_taken += 1;
    debug_assert_eq!(attempts_taken, total_attempts);
    match attempt_fn() {
        Ok(out) => Ok(out),
        Err(err) if is_transient_verifier_error(&err) => {
            Err(annotate_verifier_error_with_attempts(err, attempts_taken))
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // ── transient classifier ──────────────────────────────────────────────────

    #[test]
    fn is_transient_message_matches_documented_provider_flakes() {
        // Every documented transient marker must classify positively,
        // regardless of case or surrounding boilerplate.
        let transient_cases = [
            "Selected model is at capacity. Please try a different model.",
            "Error: rate limit exceeded, please slow down",
            "HTTP 429 Too Many Requests",
            "the server responded with too many requests",
            "the model is Overloaded right now",
            "request timeout after 60s",
            "operation timed out",
            "connection reset by peer",
            "Network is unreachable",
            "No executable codex found for current version",
        ];
        for msg in transient_cases {
            assert!(is_transient_message(msg), "expected '{msg}' to classify as transient");
        }
    }

    #[test]
    fn is_transient_message_rejects_fatal_diagnostics() {
        // Fatal categories: auth, unsupported model, schema/parse rejection,
        // sandbox denial, empty output. None of these must trigger a retry.
        let fatal_cases = [
            "authentication failed: invalid API key",
            "unsupported model 'gpt-99'",
            "schema validation error: missing field 'kind'",
            "sandbox: workspace-write denied by policy",
            "codex ref-verifier output-last-message is empty",
            "spawn failed: permission denied",
            "invalid ref-verifier provider 'foo'",
            "",
        ];
        for msg in fatal_cases {
            assert!(
                !is_transient_message(msg),
                "expected '{msg}' to be classified as fatal (non-transient)"
            );
        }
    }

    #[test]
    fn is_transient_verifier_error_only_recognises_verifier_port() {
        // A transient marker embedded in a non-VerifierPort variant must NOT
        // trigger a retry — retry semantics only make sense for adapter-layer
        // subprocess failures.
        assert!(!is_transient_verifier_error(&RefVerifyError::InvalidConfig {
            message: "at capacity".to_owned(),
        }));
        assert!(is_transient_verifier_error(&RefVerifyError::VerifierPort {
            message: "provider at capacity".to_owned(),
        }));
        assert!(!is_transient_verifier_error(&RefVerifyError::VerifierPort {
            message: "unsupported model".to_owned(),
        }));
    }

    // ── retry loop behaviour ──────────────────────────────────────────────────

    /// Build a `VerifierPort` error whose message contains the "at capacity"
    /// marker used by the transient classifier.
    fn transient_err(tag: &str) -> RefVerifyError {
        RefVerifyError::VerifierPort { message: format!("provider at capacity ({tag})") }
    }

    fn fatal_err(tag: &str) -> RefVerifyError {
        RefVerifyError::VerifierPort { message: format!("authentication failed ({tag})") }
    }

    /// Test schedule with two retries but zero real sleep — the injected
    /// `sleep_fn` counts invocations instead of parking the thread.
    const TEST_BACKOFFS: &[Duration] = &[Duration::from_millis(1), Duration::from_millis(1)];

    #[test]
    fn retry_transient_recovers_after_two_transient_failures() {
        // fails transiently twice then succeeds → overall success with 3 invocations
        let attempts = std::cell::Cell::new(0_usize);
        let sleeps = std::cell::Cell::new(0_usize);
        let outcome = retry_transient(
            TEST_BACKOFFS,
            || {
                attempts.set(attempts.get() + 1);
                match attempts.get() {
                    1 | 2 => Err(transient_err("try")),
                    _ => Ok("ok"),
                }
            },
            |_dur| sleeps.set(sleeps.get() + 1),
        )
        .unwrap();
        assert_eq!(outcome, "ok");
        assert_eq!(attempts.get(), 3, "should invoke exactly 3 times");
        assert_eq!(sleeps.get(), 2, "should sleep between attempts, not after final success");
    }

    #[test]
    fn retry_transient_does_not_retry_on_fatal_error() {
        // non-transient error → exactly 1 invocation, no sleep, error is
        // returned verbatim without the attempt-count annotation.
        let attempts = std::cell::Cell::new(0_usize);
        let sleeps = std::cell::Cell::new(0_usize);
        let err = retry_transient(
            TEST_BACKOFFS,
            || -> Result<&'static str, RefVerifyError> {
                attempts.set(attempts.get() + 1);
                Err(fatal_err("first"))
            },
            |_dur| sleeps.set(sleeps.get() + 1),
        )
        .unwrap_err();
        assert_eq!(attempts.get(), 1, "must not retry a fatal error");
        assert_eq!(sleeps.get(), 0, "must not sleep before giving up on fatal error");
        let RefVerifyError::VerifierPort { message } = err else {
            panic!("expected VerifierPort, got {err:?}");
        };
        assert!(
            !message.contains("after"),
            "fatal error must NOT carry attempt-count annotation, got: {message}"
        );
    }

    #[test]
    fn retry_transient_gives_up_after_persistent_transient_error() {
        // transient error persisting → gives up after 3 attempts and appends
        // the attempt count to the final diagnostic.
        let attempts = std::cell::Cell::new(0_usize);
        let sleeps = std::cell::Cell::new(0_usize);
        let err = retry_transient(
            TEST_BACKOFFS,
            || -> Result<&'static str, RefVerifyError> {
                attempts.set(attempts.get() + 1);
                Err(transient_err("persistent"))
            },
            |_dur| sleeps.set(sleeps.get() + 1),
        )
        .unwrap_err();
        assert_eq!(attempts.get(), 3, "must attempt exactly max_attempts times");
        assert_eq!(sleeps.get(), 2, "must sleep once between each retry");
        let RefVerifyError::VerifierPort { message } = err else {
            panic!("expected VerifierPort, got {err:?}");
        };
        assert!(
            message.contains("after 3 attempts"),
            "exhausted-retry error must annotate attempt count, got: {message}"
        );
    }

    #[test]
    fn retry_transient_uses_backoff_schedule_in_order() {
        // Verify that the caller-supplied backoff schedule is consumed in
        // order (attempt N uses backoffs[N-1] before attempt N+1). This
        // guards against off-by-one regressions.
        let recorded: std::cell::RefCell<Vec<Duration>> = std::cell::RefCell::new(Vec::new());
        let schedule = [Duration::from_millis(7), Duration::from_millis(23)];
        let attempts = std::cell::Cell::new(0_usize);
        let _ = retry_transient(
            &schedule,
            || -> Result<&'static str, RefVerifyError> {
                attempts.set(attempts.get() + 1);
                Err(transient_err("always"))
            },
            |dur| recorded.borrow_mut().push(dur),
        )
        .unwrap_err();
        assert_eq!(recorded.into_inner(), schedule.to_vec());
    }

    #[test]
    fn default_transient_backoffs_have_two_entries_totalling_three_attempts() {
        // The team-lead contract says 3 attempts total with increasing
        // backoff. Any change to the schedule must keep both invariants.
        assert_eq!(DEFAULT_TRANSIENT_BACKOFFS.len(), 2, "3 attempts total");
        let mut iter = DEFAULT_TRANSIENT_BACKOFFS.iter();
        let first = *iter.next().unwrap();
        let second = *iter.next().unwrap();
        assert!(first < second, "backoff must strictly increase between retries");
    }
}
