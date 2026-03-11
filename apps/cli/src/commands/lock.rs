use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use domain::lock::{AgentId, FileLockManager, FilePath, LockMode};
use infrastructure::lock::FsFileLockManager;

/// Lock management subcommands for agent concurrent file access.
#[derive(Debug, clap::Subcommand)]
pub enum LockCommand {
    /// Acquire a lock on a file.
    Acquire {
        #[arg(long)]
        mode: String,
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        agent: String,
        /// Process ID of the lock holder. Defaults to the current process PID.
        #[arg(long)]
        pid: Option<u32>,
        #[arg(long, default_value = "5000")]
        timeout_ms: u64,
    },
    /// Release a lock on a file.
    Release {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        agent: String,
    },
    /// Show current lock status.
    Status {
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Remove stale lock entries.
    Cleanup,
    /// Extend the expiry of a lock.
    Extend {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        agent: String,
        #[arg(long, default_value = "300000")]
        additional_ms: u64,
    },
}

/// Executes a lock subcommand.
///
/// # Errors
/// Returns `ExitCode::FAILURE` on lock operation errors.
pub fn execute(cmd: LockCommand, locks_dir: &str) -> ExitCode {
    let manager = match FsFileLockManager::new(locks_dir) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error": e.to_string()}));
            return ExitCode::FAILURE;
        }
    };

    match cmd {
        LockCommand::Acquire { mode, path, agent, pid, timeout_ms } => {
            let pid = pid.unwrap_or_else(std::process::id);
            execute_acquire(&manager, &mode, &path, &agent, pid, timeout_ms)
        }
        LockCommand::Release { path, agent } => execute_release(&manager, &path, &agent),
        LockCommand::Status { path } => execute_status(&manager, path.as_deref()),
        LockCommand::Cleanup => execute_cleanup(&manager),
        LockCommand::Extend { path, agent, additional_ms } => {
            execute_extend(&manager, &path, &agent, additional_ms)
        }
    }
}

fn execute_acquire(
    manager: &FsFileLockManager,
    mode: &str,
    path: &PathBuf,
    agent: &str,
    pid: u32,
    timeout_ms: u64,
) -> ExitCode {
    let lock_mode = match mode {
        "shared" => LockMode::Shared,
        "exclusive" => LockMode::Exclusive,
        other => {
            eprintln!(
                "{}",
                serde_json::json!({"error": format!("invalid mode: {other}, expected 'shared' or 'exclusive'")})
            );
            return ExitCode::FAILURE;
        }
    };

    let file_path = match FilePath::new(path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error": e.to_string()}));
            return ExitCode::FAILURE;
        }
    };

    let agent_id = AgentId::new(agent);
    let timeout = if timeout_ms > 0 { Some(Duration::from_millis(timeout_ms)) } else { None };

    match manager.acquire(&file_path, lock_mode, &agent_id, pid, timeout) {
        Ok(guard) => {
            // Forget the guard so the release callback does not fire on process exit.
            // The lock will be explicitly released by a PostToolUse hook.
            std::mem::forget(guard);
            println!(
                "{}",
                serde_json::json!({
                    "status": "acquired",
                    "path": path.to_string_lossy(),
                    "mode": mode,
                    "agent": agent,
                    "pid": pid
                })
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error": e.to_string()}));
            ExitCode::FAILURE
        }
    }
}

fn execute_release(manager: &FsFileLockManager, path: &PathBuf, agent: &str) -> ExitCode {
    let file_path = match FilePath::new(path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error": e.to_string()}));
            return ExitCode::FAILURE;
        }
    };

    let agent_id = AgentId::new(agent);
    match manager.release(&file_path, &agent_id) {
        Ok(()) => {
            println!(
                "{}",
                serde_json::json!({
                    "status": "released",
                    "path": path.to_string_lossy(),
                    "agent": agent
                })
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error": e.to_string()}));
            ExitCode::FAILURE
        }
    }
}

fn execute_status(manager: &FsFileLockManager, path: Option<&std::path::Path>) -> ExitCode {
    let file_path = match path {
        Some(p) => match FilePath::new(p) {
            Ok(fp) => Some(fp),
            Err(e) => {
                eprintln!("{}", serde_json::json!({"error": e.to_string()}));
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };

    match manager.query(file_path.as_ref()) {
        Ok(entries) => {
            let json_entries: Vec<serde_json::Value> = entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "path": e.path.as_path().to_string_lossy(),
                        "mode": e.mode.to_string(),
                        "agent": e.agent.as_str(),
                        "pid": e.pid,
                    })
                })
                .collect();
            println!("{}", serde_json::json!({"locks": json_entries}));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error": e.to_string()}));
            ExitCode::FAILURE
        }
    }
}

fn execute_cleanup(manager: &FsFileLockManager) -> ExitCode {
    match manager.cleanup() {
        Ok(count) => {
            println!("{}", serde_json::json!({"status": "cleanup_done", "reaped": count}));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error": e.to_string()}));
            ExitCode::FAILURE
        }
    }
}

fn execute_extend(
    manager: &FsFileLockManager,
    path: &PathBuf,
    agent: &str,
    additional_ms: u64,
) -> ExitCode {
    let file_path = match FilePath::new(path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error": e.to_string()}));
            return ExitCode::FAILURE;
        }
    };

    let agent_id = AgentId::new(agent);
    match manager.extend(&file_path, &agent_id, Duration::from_millis(additional_ms)) {
        Ok(()) => {
            println!(
                "{}",
                serde_json::json!({
                    "status": "extended",
                    "path": path.to_string_lossy(),
                    "agent": agent,
                    "additional_ms": additional_ms
                })
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error": e.to_string()}));
            ExitCode::FAILURE
        }
    }
}
