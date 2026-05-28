//! `sotp file` subcommands — file utilities.

use std::path::PathBuf;

use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Atomically write `content` to `path` (tmp + fsync + rename).
    ///
    /// # Errors
    ///
    /// Returns `Err` when the atomic write fails (I/O error or path error).
    pub fn file_write_atomic(
        &self,
        path: PathBuf,
        content: &[u8],
    ) -> Result<CommandOutcome, String> {
        infrastructure::track::atomic_write::atomic_write_file(&path, content)
            .map_err(|e| format!("atomic write failed: {e}"))?;
        Ok(CommandOutcome::success(None))
    }
}
