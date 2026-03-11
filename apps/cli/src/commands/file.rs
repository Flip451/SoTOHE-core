//! File utility subcommands.

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Subcommand;

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
        FileCommand::WriteAtomic { path } => write_atomic(&path),
    }
}

fn write_atomic(path: &std::path::Path) -> ExitCode {
    let mut buf = Vec::new();
    if let Err(e) = std::io::stdin().take(MAX_STDIN_BYTES as u64 + 1).read_to_end(&mut buf) {
        eprintln!("failed to read stdin: {e}");
        return ExitCode::FAILURE;
    }
    if buf.len() > MAX_STDIN_BYTES {
        eprintln!("stdin exceeds maximum size of {MAX_STDIN_BYTES} bytes");
        return ExitCode::FAILURE;
    }

    if let Err(e) = infrastructure::track::atomic_write::atomic_write_file(path, &buf) {
        eprintln!("atomic write failed: {e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
