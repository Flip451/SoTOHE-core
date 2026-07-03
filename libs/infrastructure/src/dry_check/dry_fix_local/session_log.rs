use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub(super) fn dry_fix_redaction_values(safe_env: &[(OsString, OsString)]) -> Vec<(String, String)> {
    let mut values = safe_env
        .iter()
        .filter_map(|(key, value)| {
            let key = key.to_str()?.to_owned();
            if !super::DRY_FIX_REDACTED_ENV_VARS.contains(&key.as_str()) {
                return None;
            }
            let value = value.to_string_lossy();
            if value.is_empty() { None } else { Some((key, value.into_owned())) }
        })
        .collect::<Vec<_>>();
    values.sort_by(|(var_a, val_a), (var_b, val_b)| {
        val_b.len().cmp(&val_a.len()).then_with(|| var_a.cmp(var_b))
    });
    values
}

pub(super) fn redact_dry_fix_sensitive_text(text: &str, redactions: &[(String, String)]) -> String {
    let mut redacted = text.to_owned();
    for (var, secret) in redactions {
        let placeholder = format!("[REDACTED:{var}]");
        redacted = redacted.replace(secret, &placeholder);
    }
    redacted
}

pub(super) fn write_dry_fix_log(
    log_path: &Path,
    bin: &OsString,
    status: &str,
    stdout: &str,
    stderr: &str,
) {
    let content = format!(
        "bin: {}\nstatus: {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        bin.to_string_lossy(),
        status,
        stdout,
        stderr
    );
    let _ = std::fs::write(log_path, content);
}

/// Deletes the Codex fixer session log file on drop unless told to keep it.
///
/// Default behaviour is to remove the log when the guard is dropped (clean
/// successful run). Call [`keep_for_diagnosis`] on the failure path so the
/// log survives for the caller to inspect.
///
/// [`keep_for_diagnosis`]: DryFixSessionLogCleanup::keep_for_diagnosis
pub(super) struct DryFixSessionLogCleanup {
    path: PathBuf,
    remove_on_drop: bool,
}

impl DryFixSessionLogCleanup {
    pub(super) fn new(path: PathBuf) -> Self {
        Self { path, remove_on_drop: true }
    }

    pub(super) fn keep_for_diagnosis(mut self) {
        self.remove_on_drop = false;
    }
}

impl Drop for DryFixSessionLogCleanup {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_dry_fix_session_log_cleanup_removes_log_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("session.log");
        std::fs::write(&log_path, "dry fixer output").unwrap();

        {
            let _cleanup = DryFixSessionLogCleanup::new(log_path.clone());
        }

        assert!(!log_path.exists(), "default cleanup must remove successful-run logs");
    }

    #[test]
    fn test_dry_fix_session_log_cleanup_keep_for_diagnosis_preserves_log() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("session.log");
        std::fs::write(&log_path, "dry fixer output").unwrap();

        DryFixSessionLogCleanup::new(log_path.clone()).keep_for_diagnosis();

        assert!(log_path.exists(), "diagnostic cleanup must preserve failed-run logs");
    }
}
