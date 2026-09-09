//! Lifecycle-aware provider-process execution for reviewer adapters.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use usecase::capability_exec::{CapabilityExecError, ProviderName};

use super::{
    CODEX_VERSION_PROBE_MAX_OUTPUT_BYTES, CODEX_VERSION_PROBE_TIMEOUT, PROVIDER_LOG_SEQUENCE,
    ProviderProcessWaitFailure, apply_path_prefix, configure_process_group, dispatch_error,
    open_runtime_file, prepare_output_last_message, prepare_runtime_dir,
    read_output_last_message_at, receive_provider_output, run_command_with_bounded_output,
    spawn_bounded_log_writer, spawn_provider_session_collector, wait_for_bounded_log_writer,
    wait_for_provider_process,
};

/// Observable, bounded result of a provider subprocess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderProcessOutput {
    pub(crate) exit_code: u8,
    pub(crate) session_id: Option<String>,
    /// The bounded provider result retained until the adapter adopts this attempt.
    pub(crate) final_message: Option<Vec<u8>>,
}

/// Observable reviewer-process result with the platform's optional exit status intact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReviewerProcessOutput {
    pub(crate) exit_code: Option<i32>,
    pub(crate) session_id: Option<String>,
    pub(crate) final_message: Option<Vec<u8>>,
}

impl From<ProviderProcessOutput> for ReviewerProcessOutput {
    fn from(output: ProviderProcessOutput) -> Self {
        Self {
            exit_code: Some(i32::from(output.exit_code)),
            session_id: output.session_id,
            final_message: output.final_message,
        }
    }
}

impl From<ReviewerProcessOutput> for ProviderProcessOutput {
    fn from(output: ReviewerProcessOutput) -> Self {
        Self {
            exit_code: output.exit_code.and_then(|code| u8::try_from(code).ok()).unwrap_or(1),
            session_id: output.session_id,
            final_message: output.final_message,
        }
    }
}

pub(crate) trait ProviderProcessRunner: Send + Sync {
    #[allow(clippy::too_many_arguments)]
    fn run(
        &self,
        binary: &str,
        path_prefix: Option<&Path>,
        args: &[OsString],
        repo_root: &Path,
        runtime_dir: &Path,
        provider: &ProviderName,
        timeout: Option<Duration>,
        output_last_message: Option<&Path>,
    ) -> Result<ProviderProcessOutput, CapabilityExecError>;

    /// Runs a provider process while preserving whether an error happened
    /// before or after the child was spawned.
    ///
    /// Existing capability and dry-check callers use [`Self::run`]. The
    /// default keeps test doubles and other callers source-compatible; the
    /// system implementation overrides it with the real process boundary.
    #[allow(clippy::too_many_arguments)]
    fn run_with_lifecycle(
        &self,
        binary: &str,
        path_prefix: Option<&Path>,
        args: &[OsString],
        repo_root: &Path,
        runtime_dir: &Path,
        provider: &ProviderName,
        timeout: Option<Duration>,
        output_last_message: Option<&Path>,
    ) -> Result<ProviderProcessOutput, ProviderProcessFailure> {
        self.run(
            binary,
            path_prefix,
            args,
            repo_root,
            runtime_dir,
            provider,
            timeout,
            output_last_message,
        )
        .map_err(ProviderProcessFailure::PreSpawn)
    }

    /// Runs a reviewer process while preserving its full platform exit status.
    ///
    /// Generic capability dispatch retains its historical opaque `u8` status, but reviewer
    /// diagnostics must distinguish a missing status (for example signal termination) from an
    /// acquired status. The system implementation overrides this at the real spawn boundary;
    /// the default exists only for test doubles and legacy callers.
    #[allow(clippy::too_many_arguments)]
    fn run_reviewer_with_lifecycle(
        &self,
        binary: &str,
        path_prefix: Option<&Path>,
        args: &[OsString],
        repo_root: &Path,
        runtime_dir: &Path,
        provider: &ProviderName,
        timeout: Option<Duration>,
        output_last_message: Option<&Path>,
    ) -> Result<ReviewerProcessOutput, ProviderProcessFailure> {
        self.run_with_lifecycle(
            binary,
            path_prefix,
            args,
            repo_root,
            runtime_dir,
            provider,
            timeout,
            output_last_message,
        )
        .map(ReviewerProcessOutput::from)
    }
}

pub(crate) struct SystemProviderProcessRunner;

impl ProviderProcessRunner for SystemProviderProcessRunner {
    fn run(
        &self,
        binary: &str,
        path_prefix: Option<&Path>,
        args: &[OsString],
        repo_root: &Path,
        runtime_dir: &Path,
        provider: &ProviderName,
        timeout: Option<Duration>,
        output_last_message: Option<&Path>,
    ) -> Result<ProviderProcessOutput, CapabilityExecError> {
        run_provider_process_with_timeout(
            binary,
            path_prefix,
            args,
            repo_root,
            runtime_dir,
            provider,
            timeout,
            output_last_message,
        )
    }

    fn run_with_lifecycle(
        &self,
        binary: &str,
        path_prefix: Option<&Path>,
        args: &[OsString],
        repo_root: &Path,
        runtime_dir: &Path,
        provider: &ProviderName,
        timeout: Option<Duration>,
        output_last_message: Option<&Path>,
    ) -> Result<ProviderProcessOutput, ProviderProcessFailure> {
        run_provider_process_with_timeout_inner_with_lifecycle(
            binary,
            path_prefix,
            args,
            repo_root,
            runtime_dir,
            provider,
            timeout,
            output_last_message,
        )
    }

    fn run_reviewer_with_lifecycle(
        &self,
        binary: &str,
        path_prefix: Option<&Path>,
        args: &[OsString],
        repo_root: &Path,
        runtime_dir: &Path,
        provider: &ProviderName,
        timeout: Option<Duration>,
        output_last_message: Option<&Path>,
    ) -> Result<ReviewerProcessOutput, ProviderProcessFailure> {
        run_reviewer_process_with_timeout_inner_with_lifecycle(
            binary,
            path_prefix,
            args,
            repo_root,
            runtime_dir,
            provider,
            timeout,
            output_last_message,
        )
    }
}

pub(crate) fn system_process_runner() -> Arc<dyn ProviderProcessRunner> {
    Arc::new(SystemProviderProcessRunner)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_provider_process_with_timeout(
    binary: &str,
    path_prefix: Option<&Path>,
    args: &[OsString],
    repo_root: &Path,
    runtime_dir: &Path,
    provider: &ProviderName,
    timeout: Option<Duration>,
    output_last_message: Option<&Path>,
) -> Result<ProviderProcessOutput, CapabilityExecError> {
    run_provider_process_with_timeout_inner(
        binary,
        path_prefix,
        args,
        repo_root,
        runtime_dir,
        provider,
        timeout,
        output_last_message,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_provider_process_with_timeout_inner(
    binary: &str,
    path_prefix: Option<&Path>,
    args: &[OsString],
    repo_root: &Path,
    runtime_dir: &Path,
    provider: &ProviderName,
    timeout: Option<Duration>,
    output_last_message: Option<&Path>,
) -> Result<ProviderProcessOutput, CapabilityExecError> {
    run_provider_process_with_timeout_inner_with_lifecycle(
        binary,
        path_prefix,
        args,
        repo_root,
        runtime_dir,
        provider,
        timeout,
        output_last_message,
    )
    .map_err(ProviderProcessFailure::into_error)
}

#[allow(clippy::too_many_arguments)]
fn run_provider_process_with_timeout_inner_with_lifecycle(
    binary: &str,
    path_prefix: Option<&Path>,
    args: &[OsString],
    repo_root: &Path,
    runtime_dir: &Path,
    provider: &ProviderName,
    timeout: Option<Duration>,
    output_last_message: Option<&Path>,
) -> Result<ProviderProcessOutput, ProviderProcessFailure> {
    run_reviewer_process_with_timeout_inner_with_lifecycle(
        binary,
        path_prefix,
        args,
        repo_root,
        runtime_dir,
        provider,
        timeout,
        output_last_message,
    )
    .map(ProviderProcessOutput::from)
}

/// A provider-process failure annotated at the infrastructure spawn boundary.
#[derive(Debug)]
pub(crate) enum ProviderProcessFailure {
    PreSpawn(CapabilityExecError),
    PostSpawn(CapabilityExecError),
    Timeout(CapabilityExecError),
}

impl ProviderProcessFailure {
    pub(crate) fn into_error(self) -> CapabilityExecError {
        match self {
            Self::PreSpawn(error) | Self::PostSpawn(error) | Self::Timeout(error) => error,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_reviewer_process_with_timeout_inner_with_lifecycle(
    binary: &str,
    path_prefix: Option<&Path>,
    args: &[OsString],
    repo_root: &Path,
    runtime_dir: &Path,
    provider: &ProviderName,
    timeout: Option<Duration>,
    output_last_message: Option<&Path>,
) -> Result<ReviewerProcessOutput, ProviderProcessFailure> {
    let runtime_dir = prepare_runtime_dir(repo_root, runtime_dir, provider)
        .map_err(ProviderProcessFailure::PreSpawn)?;
    let output_last_message = output_last_message
        .map(|path| prepare_output_last_message(path, &runtime_dir, provider))
        .transpose()
        .map_err(ProviderProcessFailure::PreSpawn)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            ProviderProcessFailure::PreSpawn(dispatch_error(
                provider,
                format!("cannot create session log timestamp: {error}"),
            ))
        })?
        .as_nanos();
    let log_name = OsString::from(format!(
        "capability-exec-{}-{}-{timestamp}-{}.log",
        provider.as_str(),
        std::process::id(),
        PROVIDER_LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ));
    let log_path = runtime_dir.path.join(&log_name);
    let mut log_file = open_runtime_file(
        &runtime_dir.directory,
        &log_name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        provider,
        &log_path,
        "create session log",
    )
    .map_err(ProviderProcessFailure::PreSpawn)?;
    if provider.as_str() == "codex" {
        let real_path = std::path::Path::new(binary)
            .canonicalize()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| binary.to_owned());
        let mut version_command = Command::new(binary);
        version_command.arg("--version");
        apply_path_prefix(&mut version_command, path_prefix, provider)
            .map_err(ProviderProcessFailure::PreSpawn)?;
        let version = run_command_with_bounded_output(
            &mut version_command,
            CODEX_VERSION_PROBE_MAX_OUTPUT_BYTES,
            CODEX_VERSION_PROBE_TIMEOUT,
            "Codex version probe",
        )
        .map(|output| {
            let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            text
        })
        .unwrap_or_else(|error| format!("probe failed: {error}"));
        writeln!(
            log_file,
            "resolved_real_path: {real_path}\ncodex_version: {}",
            version.trim_end()
        )
        .map_err(|error| {
            ProviderProcessFailure::PreSpawn(dispatch_error(
                provider,
                format!("cannot write session log {}: {error}", log_path.display()),
            ))
        })?;
    }

    let mut command = Command::new(binary);
    command
        .args(args)
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_path_prefix(&mut command, path_prefix, provider)
        .map_err(ProviderProcessFailure::PreSpawn)?;
    configure_process_group(&mut command);
    let mut child = command.spawn().map_err(|error| {
        ProviderProcessFailure::PreSpawn(dispatch_error(
            provider,
            format!("cannot start {binary}: {error}"),
        ))
    })?;
    let process_id = child.id();
    let stderr = child.stderr.take().ok_or_else(|| {
        ProviderProcessFailure::PostSpawn(dispatch_error(
            provider,
            format!("cannot capture stderr for {binary} provider subprocess"),
        ))
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        ProviderProcessFailure::PostSpawn(dispatch_error(
            provider,
            format!("cannot capture stdout for {binary} provider subprocess"),
        ))
    })?;
    let log_writer = spawn_bounded_log_writer(stderr, log_file);
    let session_collector = spawn_provider_session_collector(stdout, provider.clone());
    let status =
        wait_for_provider_process(&mut child, provider, binary, timeout).map_err(|failure| {
            match failure {
                ProviderProcessWaitFailure::Timeout(error) => {
                    ProviderProcessFailure::Timeout(error)
                }
                ProviderProcessWaitFailure::Failed(error) => {
                    ProviderProcessFailure::PostSpawn(error)
                }
            }
        })?;
    wait_for_bounded_log_writer(log_writer, process_id, provider, binary, &log_path)
        .map_err(ProviderProcessFailure::PostSpawn)?;
    let collected = receive_provider_output(session_collector, process_id, provider, binary)
        .map_err(ProviderProcessFailure::PostSpawn)?;
    // The provider may mutate its writable runtime directory while it runs. Re-open every
    // component relative to a pinned repository handle before opening provider-controlled
    // output, so a replaced parent cannot redirect the leaf read outside the repository runtime.
    let runtime_dir = prepare_runtime_dir(repo_root, &runtime_dir.path, provider)
        .map_err(ProviderProcessFailure::PostSpawn)?;
    let final_message = match output_last_message {
        Some(output) => read_output_last_message_at(&runtime_dir, &output, provider)
            .map_err(ProviderProcessFailure::PostSpawn)?,
        None => collected.final_message,
    };
    Ok(ReviewerProcessOutput {
        exit_code: status.code(),
        session_id: collected.session_id,
        final_message,
    })
}
