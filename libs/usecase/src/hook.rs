//! Hook dispatch use cases (OCP: each hook implements `HookHandler` independently).

use std::time::Duration;

use domain::Decision;
use domain::hook::{HookContext, HookError, HookInput, HookName, HookVerdict};
use domain::lock::{FileLockManager, LockMode};

/// Default timeout for lock acquisition (matches existing lock command default).
const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_millis(5000);

/// Resolves `LockMode` from the tool name.
/// `Read` → `Shared`, `Edit`/`Write` → `Exclusive`.
///
/// # Errors
/// Returns `HookError::Input` for unknown tool names to prevent silent
/// exclusive lock acquisition on unsupported tools.
fn resolve_lock_mode(tool_name: &str) -> Result<LockMode, HookError> {
    match tool_name {
        "Read" => Ok(LockMode::Shared),
        "Edit" | "Write" => Ok(LockMode::Exclusive),
        other => Err(HookError::Input(format!("unsupported tool for lock: {other}"))),
    }
}

/// Port for individual hook logic.
/// Receives framework-free HookInput (converted from HookEnvelope at CLI boundary).
///
/// ## Required Field Validation (fail-closed)
///
/// Each handler MUST validate hook-specific required fields from `HookInput`
/// and return `HookError::Input` if they are missing.
///
/// How the CLI maps `HookError::Input` depends on the hook event type:
/// - PreToolUse (guard, lock-acquire): `HookError::Input` → exit 2 (block, fail-closed)
/// - PostToolUse (lock-release): `HookError::Input` → stderr warning + exit 0 (cannot block)
///
/// | Hook | Required fields | Missing → |
/// |------|----------------|-----------|
/// | `BlockDirectGitOps` | `tool_name` (always present), `command` | `HookError::Input("missing command")` |
/// | `FileLockAcquire` | `tool_name`, `file_path` | `HookError::Input("missing file_path")` |
/// | `FileLockRelease` | `tool_name`, `file_path` | `HookError::Input("missing file_path")` |
///
/// Note: `tool_name` is guaranteed present (required in `HookEnvelope` serde).
/// `command` and `file_path` are `Option` in `HookInput` because different hooks
/// need different fields. The handler validates what it needs.
pub trait HookHandler: Send + Sync {
    /// Processes a hook event and returns a verdict.
    ///
    /// # Errors
    /// Returns `HookError` on invalid input or subsystem failure.
    fn handle(&self, ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError>;
}

/// Guard hook handler: delegates to `domain::guard::policy::check`.
pub struct GuardHookHandler;

impl HookHandler for GuardHookHandler {
    fn handle(&self, _ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError> {
        let command =
            input.command.as_deref().ok_or_else(|| HookError::Input("missing command".into()))?;

        let guard_verdict = domain::guard::policy::check(command);

        if guard_verdict.is_blocked() {
            Ok(HookVerdict::block(guard_verdict.reason))
        } else {
            Ok(HookVerdict::allow())
        }
    }
}

/// Lock-acquire hook handler: delegates to `FileLockManager::acquire`.
pub struct LockAcquireHookHandler<L: FileLockManager> {
    lock_manager: std::sync::Arc<L>,
}

impl<L: FileLockManager> LockAcquireHookHandler<L> {
    /// Creates a new handler with the given lock manager.
    #[must_use]
    pub fn new(lock_manager: std::sync::Arc<L>) -> Self {
        Self { lock_manager }
    }
}

impl<L: FileLockManager> HookHandler for LockAcquireHookHandler<L> {
    fn handle(&self, ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError> {
        let file_path = input
            .file_path
            .as_deref()
            .ok_or_else(|| HookError::Input("missing file_path".into()))?;

        let agent = ctx.agent.as_ref().ok_or_else(|| HookError::Input("missing agent".into()))?;

        let pid = ctx.pid.ok_or_else(|| HookError::Input("missing pid".into()))?;

        let lock_path = domain::lock::FilePath::new(file_path)?;

        // Resolve lock mode from tool_name: Read → Shared, Edit/Write → Exclusive.
        let mode = resolve_lock_mode(&input.tool_name)?;

        match self.lock_manager.acquire(&lock_path, mode, agent, pid, Some(DEFAULT_LOCK_TIMEOUT)) {
            Ok(guard) => {
                // Prevent drop from releasing the lock — PostToolUse will release explicitly.
                std::mem::forget(guard);
                Ok(HookVerdict::allow())
            }
            Err(domain::lock::LockError::ExclusivelyHeld { holder, pid: held_pid }) => {
                Ok(HookVerdict {
                    decision: Decision::Block,
                    reason: Some(format!(
                        "file is exclusively locked by agent {holder} (pid {held_pid})"
                    )),
                    additional_context: None,
                })
            }
            Err(domain::lock::LockError::SharedLockConflict { count }) => Ok(HookVerdict {
                decision: Decision::Block,
                reason: Some(format!(
                    "file has {count} shared lock(s); cannot acquire exclusive lock"
                )),
                additional_context: None,
            }),
            Err(e) => Err(HookError::Lock(e)),
        }
    }
}

/// Lock-release hook handler: delegates to `FileLockManager::release`.
pub struct LockReleaseHookHandler<L: FileLockManager> {
    lock_manager: std::sync::Arc<L>,
}

impl<L: FileLockManager> LockReleaseHookHandler<L> {
    /// Creates a new handler with the given lock manager.
    #[must_use]
    pub fn new(lock_manager: std::sync::Arc<L>) -> Self {
        Self { lock_manager }
    }
}

impl<L: FileLockManager> HookHandler for LockReleaseHookHandler<L> {
    fn handle(&self, ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError> {
        let file_path = input
            .file_path
            .as_deref()
            .ok_or_else(|| HookError::Input("missing file_path".into()))?;

        let agent = ctx.agent.as_ref().ok_or_else(|| HookError::Input("missing agent".into()))?;

        // pid is NOT required for release — FileLockManager::release uses path + agent only.

        let lock_path = domain::lock::FilePath::new(file_path)?;

        self.lock_manager.release(&lock_path, agent)?;

        Ok(HookVerdict::allow())
    }
}

/// Resolves a `HookName` to the appropriate handler and dispatches.
///
/// This function is the single dispatch point (OCP: adding a new hook
/// only requires a new `HookHandler` impl and a match arm here).
///
/// # Errors
/// Returns `HookError` from the handler, or `HookError::Unsupported` for unknown hooks.
pub fn dispatch(
    _name: HookName,
    handler: &dyn HookHandler,
    ctx: &HookContext,
    input: &HookInput,
) -> Result<HookVerdict, HookError> {
    handler.handle(ctx, input)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_guard_handler_allows_safe_command() {
        let handler = GuardHookHandler;
        let ctx = HookContext { project_dir: None, locks_dir: None, agent: None, pid: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("git status".into()),
            file_path: None,
        };

        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_guard_handler_blocks_git_add() {
        let handler = GuardHookHandler;
        let ctx = HookContext { project_dir: None, locks_dir: None, agent: None, pid: None };
        let input = HookInput {
            tool_name: "Bash".into(),
            command: Some("git add .".into()),
            file_path: None,
        };

        let verdict = handler.handle(&ctx, &input).unwrap();
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_guard_handler_returns_error_on_missing_command() {
        let handler = GuardHookHandler;
        let ctx = HookContext { project_dir: None, locks_dir: None, agent: None, pid: None };
        let input = HookInput { tool_name: "Bash".into(), command: None, file_path: None };

        let result = handler.handle(&ctx, &input);
        assert!(matches!(result, Err(HookError::Input(msg)) if msg.contains("missing command")));
    }

    #[test]
    fn test_lock_acquire_returns_error_on_missing_file_path() {
        let handler = LockAcquireHookHandler::new(std::sync::Arc::new(StubLockManager));
        let ctx = HookContext {
            project_dir: None,
            locks_dir: Some(PathBuf::from("/tmp/locks")),
            agent: Some(domain::lock::AgentId::new("pid-1234")),
            pid: Some(1234),
        };
        let input = HookInput { tool_name: "Write".into(), command: None, file_path: None };

        let result = handler.handle(&ctx, &input);
        assert!(matches!(result, Err(HookError::Input(msg)) if msg.contains("missing file_path")));
    }

    #[test]
    fn test_lock_acquire_returns_error_on_missing_agent() {
        let handler = LockAcquireHookHandler::new(std::sync::Arc::new(StubLockManager));
        let ctx = HookContext {
            project_dir: None,
            locks_dir: Some(PathBuf::from("/tmp/locks")),
            agent: None,
            pid: Some(1234),
        };
        let input = HookInput {
            tool_name: "Write".into(),
            command: None,
            file_path: Some(PathBuf::from("/tmp/file.txt")),
        };

        let result = handler.handle(&ctx, &input);
        assert!(matches!(result, Err(HookError::Input(msg)) if msg.contains("missing agent")));
    }

    #[test]
    fn test_lock_acquire_returns_error_on_missing_pid() {
        let handler = LockAcquireHookHandler::new(std::sync::Arc::new(StubLockManager));
        let ctx = HookContext {
            project_dir: None,
            locks_dir: Some(PathBuf::from("/tmp/locks")),
            agent: Some(domain::lock::AgentId::new("pid-1234")),
            pid: None,
        };
        let input = HookInput {
            tool_name: "Write".into(),
            command: None,
            file_path: Some(PathBuf::from("/tmp/file.txt")),
        };

        let result = handler.handle(&ctx, &input);
        assert!(matches!(result, Err(HookError::Input(msg)) if msg.contains("missing pid")));
    }

    #[test]
    fn test_lock_release_returns_error_on_missing_file_path() {
        let handler = LockReleaseHookHandler::new(std::sync::Arc::new(StubLockManager));
        let ctx = HookContext {
            project_dir: None,
            locks_dir: Some(PathBuf::from("/tmp/locks")),
            agent: Some(domain::lock::AgentId::new("pid-1234")),
            pid: None,
        };
        let input = HookInput { tool_name: "Write".into(), command: None, file_path: None };

        let result = handler.handle(&ctx, &input);
        assert!(matches!(result, Err(HookError::Input(msg)) if msg.contains("missing file_path")));
    }

    #[test]
    fn test_lock_release_does_not_require_pid() {
        // lock-release only needs locks_dir + agent, NOT pid.
        // This test verifies pid=None does not cause an error.
        let handler = LockReleaseHookHandler::new(std::sync::Arc::new(StubLockManager));
        let ctx = HookContext {
            project_dir: None,
            locks_dir: Some(PathBuf::from("/tmp/locks")),
            agent: Some(domain::lock::AgentId::new("pid-1234")),
            pid: None,
        };
        let input = HookInput {
            tool_name: "Write".into(),
            command: None,
            // Use a path that will fail canonicalization — that's fine,
            // we're testing pid is not required, not lock success.
            file_path: Some(PathBuf::from("/tmp/file.txt")),
        };

        // Will fail with LockError (path canonicalization) but NOT with HookError::Input.
        let result = handler.handle(&ctx, &input);
        assert!(!matches!(result, Err(HookError::Input(_))), "lock-release should not require pid");
    }

    #[test]
    fn test_resolve_lock_mode_read_is_shared() {
        assert_eq!(resolve_lock_mode("Read").unwrap(), LockMode::Shared);
    }

    #[test]
    fn test_resolve_lock_mode_edit_is_exclusive() {
        assert_eq!(resolve_lock_mode("Edit").unwrap(), LockMode::Exclusive);
    }

    #[test]
    fn test_resolve_lock_mode_write_is_exclusive() {
        assert_eq!(resolve_lock_mode("Write").unwrap(), LockMode::Exclusive);
    }

    #[test]
    fn test_resolve_lock_mode_unknown_tool_returns_error() {
        let result = resolve_lock_mode("Bash");
        assert!(matches!(result, Err(HookError::Input(msg)) if msg.contains("unsupported tool")));
    }

    /// Stub lock manager for unit tests (validates field presence, not lock logic).
    struct StubLockManager;

    impl FileLockManager for StubLockManager {
        fn acquire(
            &self,
            _path: &domain::lock::FilePath,
            _mode: domain::lock::LockMode,
            _agent: &domain::lock::AgentId,
            _pid: u32,
            _timeout: Option<std::time::Duration>,
        ) -> Result<domain::lock::FileGuard, domain::lock::LockError> {
            // Not reachable in these tests — we test field validation, not lock logic.
            Err(domain::lock::LockError::Timeout { elapsed_ms: 0 })
        }

        fn release(
            &self,
            _path: &domain::lock::FilePath,
            _agent: &domain::lock::AgentId,
        ) -> Result<(), domain::lock::LockError> {
            Ok(())
        }

        fn query(
            &self,
            _path: Option<&domain::lock::FilePath>,
        ) -> Result<Vec<domain::lock::LockEntry>, domain::lock::LockError> {
            Ok(Vec::new())
        }

        fn cleanup(&self) -> Result<usize, domain::lock::LockError> {
            Ok(0)
        }

        fn extend(
            &self,
            _path: &domain::lock::FilePath,
            _agent: &domain::lock::AgentId,
            _additional: std::time::Duration,
        ) -> Result<(), domain::lock::LockError> {
            Ok(())
        }
    }
}
