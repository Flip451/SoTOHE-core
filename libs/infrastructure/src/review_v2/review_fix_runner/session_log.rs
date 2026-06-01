use std::path::{Path, PathBuf};

/// Names of environment variables that carry authentication credentials and are
/// intentionally passed through to the nested Codex run via `build_safe_env`.
/// Any non-empty value for these vars must be redacted before writing to a
/// persistent log file (`.claude/rules/06-security.md`).
pub(super) const CREDENTIAL_VARS: &[&str] =
    &["OPENAI_API_KEY", "CODEX_API_KEY", "OPENAI_ORG_ID", "OPENAI_BASE_URL"];

/// Replaces every non-empty credential value found in `text` with a
/// `[REDACTED:<VAR_NAME>]` placeholder.  Empty values are never replaced —
/// replacing an empty string would corrupt the entire log.
pub(super) fn redact_credentials(text: &str) -> String {
    let values =
        CREDENTIAL_VARS.iter().filter_map(|&var| std::env::var(var).ok().map(|val| (var, val)));
    redact_credential_values(text, values)
}

pub(super) fn redact_credential_values<'a>(
    text: &str,
    values: impl IntoIterator<Item = (&'a str, String)>,
) -> String {
    let mut values: Vec<(&str, String)> =
        values.into_iter().filter(|(_, val)| !val.is_empty()).collect();
    values.sort_by(|(var_a, val_a), (var_b, val_b)| {
        val_b.len().cmp(&val_a.len()).then_with(|| var_a.cmp(var_b))
    });
    let mut result = text.to_owned();
    for (var, val) in values {
        let placeholder = format!("[REDACTED:{var}]");
        result = result.replace(&val, &placeholder);
    }
    result
}

pub(super) fn write_session_log(
    log_path: &Path,
    bin: &std::ffi::OsStr,
    exit_status: &str,
    stdout: &str,
    stderr: &str,
) {
    let bin_display = bin.to_string_lossy();
    let redacted_stdout = redact_credentials(stdout);
    let redacted_stderr = redact_credentials(stderr);
    let log_content = format!(
        "=== codex fixer session log ===\nbin: {bin_display}\nexit_status: {exit_status}\n\n\
         === STDOUT ===\n{redacted_stdout}\n\
         === STDERR ===\n{redacted_stderr}"
    );
    if let Err(e) = std::fs::write(log_path, &log_content) {
        eprintln!(
            "[review-fix-runner] warning: failed to write session log {}: {e}",
            log_path.display()
        );
    }
}

/// Deletes the session log file on drop unless told to keep it.
///
/// Default behaviour is to remove the log when the guard is dropped (clean
/// successful run).  Call [`keep_for_diagnosis`] on the failure path so the
/// log survives for the caller to inspect.
///
/// [`keep_for_diagnosis`]: SessionLogCleanup::keep_for_diagnosis
pub(super) struct SessionLogCleanup {
    path: PathBuf,
    /// When `true` (the default), drop removes the file.
    /// Set to `false` via `keep_for_diagnosis` to retain the file.
    remove_on_drop: bool,
}

impl SessionLogCleanup {
    pub(super) fn new(path: PathBuf) -> Self {
        Self { path, remove_on_drop: true }
    }

    /// Prevents the log from being deleted on drop so it can be used for diagnosis.
    pub(super) fn keep_for_diagnosis(mut self) {
        self.remove_on_drop = false;
    }
}

impl Drop for SessionLogCleanup {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // ── redact_credentials ────────────────────────────────────────────────────

    /// Run `redact_credentials` in isolation without mutating the real env
    /// (Rust 2024 forbids `std::env::set_var` inside tests due to
    /// `#![forbid(unsafe_code)]`).  We test the pure helper directly by
    /// constructing a list of `(name, value)` pairs and applying the same
    /// replacement logic.
    fn apply_redaction_with(text: &str, pairs: &[(&str, &str)]) -> String {
        redact_credential_values(text, pairs.iter().map(|(var, val)| (*var, (*val).to_owned())))
    }

    #[test]
    fn test_redact_credentials_replaces_non_empty_value_in_captured_output() {
        let fake_key = "sk-FAKE-SECRET-123456";
        let captured = format!("Running codex...\nAuthorization: Bearer {fake_key}\nDone.");

        let redacted = apply_redaction_with(&captured, &[("OPENAI_API_KEY", fake_key)]);

        assert!(
            !redacted.contains(fake_key),
            "redacted output must not contain the original secret value"
        );
        assert!(
            redacted.contains("[REDACTED:OPENAI_API_KEY]"),
            "redacted output must contain the placeholder"
        );
    }

    #[test]
    fn test_redact_credentials_with_empty_value_does_not_corrupt_output() {
        let captured = "Running codex...\nNo secret here.\nDone.";

        // Empty value: must be skipped to avoid replacing every empty-string match.
        let redacted = apply_redaction_with(captured, &[("OPENAI_API_KEY", "")]);

        assert_eq!(
            redacted, captured,
            "empty credential value must leave the output completely unchanged"
        );
    }

    #[test]
    fn test_redact_credentials_handles_multiple_vars_independently() {
        let key_val = "sk-FAKE-OPENAI-KEY";
        let codex_val = "ck-FAKE-CODEX-KEY";
        let org_val = "org-FAKE-ORG";
        let base_url_val = "https://token@example.invalid/v1";
        let captured = format!(
            "key={key_val} codex={codex_val} org={org_val} base={base_url_val} other=plaintext"
        );

        let redacted = apply_redaction_with(
            &captured,
            &[
                ("OPENAI_API_KEY", key_val),
                ("CODEX_API_KEY", codex_val),
                ("OPENAI_ORG_ID", org_val),
                ("OPENAI_BASE_URL", base_url_val),
            ],
        );

        assert!(!redacted.contains(key_val), "OPENAI_API_KEY value must be redacted");
        assert!(!redacted.contains(codex_val), "CODEX_API_KEY value must be redacted");
        assert!(!redacted.contains(org_val), "OPENAI_ORG_ID value must be redacted");
        assert!(!redacted.contains(base_url_val), "OPENAI_BASE_URL value must be redacted");
        assert!(redacted.contains("other=plaintext"), "non-credential content must be preserved");
        assert!(redacted.contains("[REDACTED:OPENAI_API_KEY]"));
        assert!(redacted.contains("[REDACTED:CODEX_API_KEY]"));
        assert!(redacted.contains("[REDACTED:OPENAI_ORG_ID]"));
        assert!(redacted.contains("[REDACTED:OPENAI_BASE_URL]"));
    }

    #[test]
    fn test_redact_credentials_replaces_longest_overlapping_value_first() {
        let short_val = "sk-overlap";
        let long_val = "sk-overlap-secret";
        let captured = format!("short={short_val} long={long_val}");

        let redacted = apply_redaction_with(
            &captured,
            &[("OPENAI_API_KEY", short_val), ("CODEX_API_KEY", long_val)],
        );

        assert!(!redacted.contains(short_val), "short credential value must be redacted");
        assert!(!redacted.contains(long_val), "long credential value must be redacted");
        assert!(
            !redacted.contains("-secret"),
            "suffix of overlapping credential value must not leak"
        );
        assert!(redacted.contains("[REDACTED:OPENAI_API_KEY]"));
        assert!(redacted.contains("[REDACTED:CODEX_API_KEY]"));
    }

    #[test]
    fn test_credential_vars_include_all_auth_safe_vars() {
        assert!(CREDENTIAL_VARS.contains(&"OPENAI_API_KEY"));
        assert!(CREDENTIAL_VARS.contains(&"CODEX_API_KEY"));
        assert!(CREDENTIAL_VARS.contains(&"OPENAI_ORG_ID"));
        assert!(CREDENTIAL_VARS.contains(&"OPENAI_BASE_URL"));
    }
}
