//! File utility subcommands.

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Subcommand;

use crate::CliError;

/// Maximum stdin size (10 MB) — sufficient for metadata.json, registry, guides.
const MAX_STDIN_BYTES: usize = 10 * 1024 * 1024;

#[derive(Subcommand)]
pub enum FileCommand {
    /// Atomically write stdin content to a file (tmp + fsync + rename).
    WriteAtomic {
        /// Target file path.
        #[arg(long)]
        path: PathBuf,
    },
}

pub fn execute(cmd: FileCommand) -> ExitCode {
    match cmd {
        FileCommand::WriteAtomic { path } => match write_atomic(&path) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
    }
}

fn write_atomic(path: &std::path::Path) -> Result<ExitCode, CliError> {
    let mut buf = Vec::new();
    std::io::stdin()
        .take(MAX_STDIN_BYTES as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|e| CliError::Message(format!("failed to read stdin: {e}")))?;
    if buf.len() > MAX_STDIN_BYTES {
        return Err(CliError::Message(format!(
            "stdin exceeds maximum size of {MAX_STDIN_BYTES} bytes"
        )));
    }

    infrastructure::track::atomic_write::atomic_write_file(path, &buf)
        .map_err(|e| CliError::Message(format!("atomic write failed: {e}")))?;

    Ok(ExitCode::SUCCESS)
}
