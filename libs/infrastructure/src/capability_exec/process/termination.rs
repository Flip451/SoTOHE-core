//! Platform-specific process-group setup and termination.

use std::io::Error;
use std::process::{Child, Command, Stdio};

use usecase::capability_exec::{CapabilityExecError, ProviderName};

use super::dispatch_error;

#[cfg(unix)]
pub(crate) fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

#[cfg(not(unix))]
pub(crate) fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
pub(crate) fn terminate_bounded_process_group(process_id: u32) -> Result<(), std::io::Error> {
    let process_group = format!("-{process_id}");
    let output = Command::new("/bin/kill")
        .args(["-KILL", "--", process_group.as_str()])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()?;
    if output.status.success()
        || String::from_utf8_lossy(&output.stderr).contains("No such process")
    {
        Ok(())
    } else {
        Err(Error::other(format!(
            "cannot terminate subprocess process group {process_id}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(windows)]
pub(crate) fn terminate_bounded_process_group(process_id: u32) -> Result<(), std::io::Error> {
    let process_id = process_id.to_string();
    let output = Command::new("taskkill")
        .args(["/PID", process_id.as_str(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .output()?;
    if output.status.success()
        || taskkill_reports_target_absent(output.status.code(), &output.stderr)
    {
        Ok(())
    } else {
        Err(Error::other(format!(
            "cannot terminate subprocess process tree {process_id}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(any(test, windows))]
fn taskkill_reports_target_absent(status_code: Option<i32>, stderr: &[u8]) -> bool {
    if status_code == Some(128) {
        return true;
    }
    let detail = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    detail.contains("not found") || detail.contains("no running instance")
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn terminate_bounded_process_group(_process_id: u32) -> Result<(), std::io::Error> {
    Err(Error::other("cannot terminate subprocess process tree on this platform"))
}

#[cfg(unix)]
pub(super) fn terminate_provider_process(
    child: &mut Child,
    provider: &ProviderName,
    binary: &str,
) -> Result<(), CapabilityExecError> {
    let process_id = child.id();
    if terminate_provider_process_group(process_id, provider, binary).is_err() {
        child.kill().map_err(|error| {
            dispatch_error(provider, format!("cannot terminate {binary} provider process: {error}"))
        })?;
    }
    child.wait().map_err(|error| {
        dispatch_error(provider, format!("cannot reap {binary} provider process: {error}"))
    })?;
    Ok(())
}

#[cfg(unix)]
pub(super) fn terminate_provider_process_group(
    process_id: u32,
    provider: &ProviderName,
    binary: &str,
) -> Result<(), CapabilityExecError> {
    let process_group = format!("-{process_id}");
    let status = Command::new("/bin/kill")
        .args(["-KILL", "--", process_group.as_str()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| {
            dispatch_error(
                provider,
                format!("cannot terminate {binary} provider process group: {error}"),
            )
        })?;
    if !status.success() {
        return Err(dispatch_error(
            provider,
            format!("cannot terminate {binary} provider process group {process_id}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::taskkill_reports_target_absent;

    #[test]
    fn test_taskkill_reports_target_absent_for_dead_pid() {
        assert!(taskkill_reports_target_absent(Some(128), b""));
        assert!(taskkill_reports_target_absent(Some(1), b"ERROR: The process \"1234\" not found."));
        assert!(!taskkill_reports_target_absent(Some(1), b"ERROR: Access is denied."));
    }
}

#[cfg(windows)]
pub(super) fn terminate_provider_process(
    child: &mut Child,
    provider: &ProviderName,
    binary: &str,
) -> Result<(), CapabilityExecError> {
    let process_id = child.id();
    if terminate_provider_process_group(process_id, provider, binary).is_err() {
        child.kill().map_err(|error| {
            dispatch_error(provider, format!("cannot terminate {binary} provider process: {error}"))
        })?;
    }
    child.wait().map_err(|error| {
        dispatch_error(provider, format!("cannot reap {binary} provider process: {error}"))
    })?;
    Ok(())
}

#[cfg(windows)]
pub(super) fn terminate_provider_process_group(
    process_id: u32,
    provider: &ProviderName,
    binary: &str,
) -> Result<(), CapabilityExecError> {
    let process_id = process_id.to_string();
    let status = Command::new("taskkill")
        .args(["/PID", process_id.as_str(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| {
            dispatch_error(
                provider,
                format!("cannot terminate {binary} provider process tree: {error}"),
            )
        })?;
    if !status.success() {
        return Err(dispatch_error(
            provider,
            format!("cannot terminate {binary} provider process tree {process_id}"),
        ));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
pub(super) fn terminate_provider_process(
    child: &mut Child,
    provider: &ProviderName,
    binary: &str,
) -> Result<(), CapabilityExecError> {
    child.kill().map_err(|error| {
        dispatch_error(provider, format!("cannot terminate {binary} provider process: {error}"))
    })?;
    child.wait().map_err(|error| {
        dispatch_error(provider, format!("cannot reap {binary} provider process: {error}"))
    })?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
pub(super) fn terminate_provider_process_group(
    _process_id: u32,
    provider: &ProviderName,
    binary: &str,
) -> Result<(), CapabilityExecError> {
    Err(dispatch_error(
        provider,
        format!("cannot terminate {binary} provider process tree on this platform"),
    ))
}
