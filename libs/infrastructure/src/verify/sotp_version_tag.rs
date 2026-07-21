//! Verify that the configured SoTOHE release tag resolves on its public remote.

use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use domain::verify::{VerifyFinding, VerifyOutcome};
use serde::Deserialize;

use crate::capability_exec::bounded_read_utf8_file;
use crate::git_cli::guarded_git_command;
use crate::track::symlink_guard::reject_symlinks_below;

const VERSION_PIN_PATH: &str = ".harness/config/sotp-version.json";
const SUPPORTED_SCHEMA_VERSION: u32 = 1;
const GIT_LS_REMOTE_TIMEOUT: Duration = Duration::from_secs(30);
const GIT_LS_REMOTE_OUTPUT_BYTES: usize = 64 * 1024;
const GIT_LS_REMOTE_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Minimal envelope used to reject unsupported schemas before strict decoding.
#[derive(Debug, Deserialize)]
struct SchemaVersionEnvelope {
    schema_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SotpVersionPin {
    #[allow(dead_code)]
    schema_version: u32,
    git_url: String,
    tag: String,
    #[allow(dead_code)]
    #[serde(rename = "crate")]
    crate_name: String,
    #[allow(dead_code)]
    binary: String,
}

/// Verifies that the configured release tag resolves on the configured remote.
///
/// # Errors
///
/// Returns a failed [`VerifyOutcome`] when the version pin cannot be read or
/// decoded, is incomplete, Git cannot run, or the configured tag is absent
/// from its remote.
pub fn verify(project_root: &Path) -> VerifyOutcome {
    let pin_path = project_root.join(VERSION_PIN_PATH);
    let pin = match read_version_pin(project_root, &pin_path) {
        Ok(pin) => pin,
        Err(message) => return VerifyOutcome::from_findings(vec![VerifyFinding::error(message)]),
    };

    if pin.git_url.trim().is_empty() || pin.tag.trim().is_empty() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(
            "configured SoTOHE version pin must contain a non-empty git_url and tag",
        )]);
    }

    let tag_ref = format!("refs/tags/{}", pin.tag);
    let output = match run_git_ls_remote(project_root, &pin.git_url, &tag_ref) {
        Ok(output) => output,
        Err(_) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot verify configured SoTOHE release tag '{}' against its public remote",
                pin.tag
            ))]);
        }
    };

    if output.status.success() && !output.stdout.is_empty() && !output.output_exceeded {
        VerifyOutcome::pass()
    } else {
        VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "configured SoTOHE release tag '{}' cannot be resolved from its public remote",
            pin.tag
        ))])
    }
}

/// Runs the remote tag lookup with a finite deadline and bounded stream capture.
fn run_git_ls_remote(
    project_root: &Path,
    git_url: &str,
    tag_ref: &str,
) -> Result<BoundedCommandOutput, ()> {
    let mut command = guarded_git_command();
    command
        .args(["ls-remote", "--exit-code", "--tags", git_url, tag_ref])
        .current_dir(project_root);
    run_command_with_bounded_output(&mut command, GIT_LS_REMOTE_TIMEOUT, GIT_LS_REMOTE_OUTPUT_BYTES)
}

/// The bounded command result needed by release-tag verification.
struct BoundedCommandOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    output_exceeded: bool,
}

/// Executes a command while draining both output streams with bounded retention.
fn run_command_with_bounded_output(
    command: &mut Command,
    timeout: Duration,
    max_stream_bytes: usize,
) -> Result<BoundedCommandOutput, ()> {
    let started = Instant::now();
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|_| ())?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            terminate_child(&mut child);
            return Err(());
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            terminate_child(&mut child);
            return Err(());
        }
    };
    let stdout_drain = match spawn_bounded_stream_drain(stdout, max_stream_bytes) {
        Ok(drain) => drain,
        Err(()) => {
            terminate_child(&mut child);
            return Err(());
        }
    };
    let stderr_drain = match spawn_bounded_stream_drain(stderr, max_stream_bytes) {
        Ok(drain) => drain,
        Err(()) => {
            terminate_child(&mut child);
            return Err(());
        }
    };

    let status = wait_for_child_with_timeout(&mut child, started, timeout)?;
    let stdout = receive_bounded_stream(stdout_drain, started, timeout)?;
    let stderr = receive_bounded_stream(stderr_drain, started, timeout)?;
    Ok(BoundedCommandOutput {
        status,
        stdout: stdout.bytes,
        output_exceeded: stdout.exceeded || stderr.exceeded,
    })
}

/// Drains one command stream without retaining more than `max_stream_bytes`.
fn spawn_bounded_stream_drain<R: Read + Send + 'static>(
    mut stream: R,
    max_stream_bytes: usize,
) -> Result<Receiver<Result<BoundedStream, ()>>, ()> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("sotp-version-tag-drain".to_owned())
        .spawn(move || {
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 8 * 1024];
            let mut exceeded = false;
            let result = loop {
                match stream.read(&mut buffer) {
                    Ok(0) => break Ok(BoundedStream { bytes, exceeded }),
                    Ok(read) => {
                        let available = max_stream_bytes.saturating_sub(bytes.len());
                        let retained = read.min(available);
                        if retained > 0 {
                            let retained_bytes = match buffer.get(..retained) {
                                Some(retained_bytes) => retained_bytes,
                                None => break Err(()),
                            };
                            bytes.extend_from_slice(retained_bytes);
                        }
                        exceeded |= read > available;
                    }
                    Err(_) => break Err(()),
                }
            };
            let _ = sender.send(result);
        })
        .map_err(|_| ())?;
    Ok(receiver)
}

/// Bounded bytes retained from one command output stream.
struct BoundedStream {
    bytes: Vec<u8>,
    exceeded: bool,
}

/// Polls a child until it exits or the command deadline expires.
fn wait_for_child_with_timeout(
    child: &mut std::process::Child,
    started: Instant,
    timeout: Duration,
) -> Result<ExitStatus, ()> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() >= timeout => {
                terminate_child(child);
                return Err(());
            }
            Ok(None) => thread::sleep(GIT_LS_REMOTE_POLL_INTERVAL),
            Err(_) => {
                terminate_child(child);
                return Err(());
            }
        }
    }
}

/// Receives a finished drain without extending the command's deadline.
fn receive_bounded_stream(
    receiver: Receiver<Result<BoundedStream, ()>>,
    started: Instant,
    timeout: Duration,
) -> Result<BoundedStream, ()> {
    let remaining = timeout.checked_sub(started.elapsed()).ok_or(())?;
    match receiver.recv_timeout(remaining) {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(())) | Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {
            Err(())
        }
    }
}

/// Reaps a failing child without exposing process details to verifier output.
fn terminate_child(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn read_version_pin(project_root: &Path, path: &Path) -> Result<SotpVersionPin, String> {
    match reject_symlinks_below(path, project_root) {
        Ok(true) => {}
        Ok(false) => {
            return Err(format!("cannot read configured SoTOHE version pin: {}", path.display()));
        }
        Err(_) => {
            return Err(format!("cannot read configured SoTOHE version pin: {}", path.display()));
        }
    }

    let content = bounded_read_utf8_file(path)
        .map_err(|_| format!("cannot read configured SoTOHE version pin: {}", path.display()))?;
    let envelope: SchemaVersionEnvelope = serde_json::from_str(&content)
        .map_err(|_| format!("cannot decode configured SoTOHE version pin: {}", path.display()))?;
    if envelope.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(format!(
            "configured SoTOHE version pin has unsupported schema version {}",
            envelope.schema_version
        ));
    }
    serde_json::from_str(&content)
        .map_err(|_| format!("cannot decode configured SoTOHE version pin: {}", path.display()))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{Duration, Instant};

    use tempfile::TempDir;

    use super::{run_command_with_bounded_output, verify};
    use crate::capability_exec::MAX_CAPABILITY_EXEC_TEXT_BYTES;

    fn run_git(root: &Path, args: &[&str]) {
        let output = Command::new("git").args(args).current_dir(root).output().unwrap();
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn create_tagged_bare_remote(root: &Path, tag: &str) -> PathBuf {
        let source = root.join("source");
        let remote = root.join("remote.git");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&remote).unwrap();
        run_git(&source, &["init", "--quiet", "--initial-branch=main"]);
        run_git(&source, &["config", "user.email", "test@example.invalid"]);
        run_git(&source, &["config", "user.name", "Verifier Test"]);
        std::fs::write(source.join("README.md"), "fixture\n").unwrap();
        run_git(&source, &["add", "README.md"]);
        run_git(&source, &["commit", "--quiet", "-m", "fixture"]);
        run_git(&source, &["tag", tag]);
        run_git(&remote, &["init", "--bare", "--quiet"]);
        run_git(&source, &["remote", "add", "origin", remote.to_str().unwrap()]);
        run_git(&source, &["push", "--quiet", "origin", "HEAD:refs/heads/main"]);
        run_git(&source, &["push", "--quiet", "origin", &format!("refs/tags/{tag}")]);
        remote
    }

    fn write_version_pin(project_root: &Path, remote: &Path, tag: &str) {
        let config_dir = project_root.join(".harness/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let remote = serde_json::to_string(&remote.to_string_lossy()).unwrap();
        std::fs::write(
            config_dir.join("sotp-version.json"),
            format!(
                r#"{{"schema_version":1,"git_url":{remote},"tag":"{tag}","crate":"cli","binary":"sotp"}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn test_verify_configured_tag_resolvable_returns_pass() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        std::fs::create_dir_all(&project_root).unwrap();
        let remote = create_tagged_bare_remote(temp_dir.path(), "sotp-v1.2.3");
        write_version_pin(&project_root, &remote, "sotp-v1.2.3");

        assert!(verify(&project_root).is_ok());
    }

    #[test]
    fn test_verify_configured_tag_unavailable_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        std::fs::create_dir_all(&project_root).unwrap();
        let remote = create_tagged_bare_remote(temp_dir.path(), "sotp-v1.2.3");
        write_version_pin(&project_root, &remote, "sotp-v9.9.9");

        assert!(verify(&project_root).has_errors());
    }

    #[test]
    fn test_verify_unsupported_schema_version_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let config_dir = project_root.join(".harness/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("sotp-version.json"),
            r#"{"schema_version":2,"git_url":"https://example.invalid/repo","tag":"sotp-v1","crate":"cli","binary":"sotp"}"#,
        )
        .unwrap();

        assert!(verify(&project_root).has_errors());
    }

    #[test]
    fn test_verify_unknown_version_pin_field_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let config_dir = project_root.join(".harness/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("sotp-version.json"),
            r#"{"schema_version":1,"git_url":"https://example.invalid/repo","tag":"sotp-v1","crate":"cli","binary":"sotp","unexpected":true}"#,
        )
        .unwrap();

        assert!(verify(&project_root).has_errors());
    }

    #[test]
    fn test_verify_oversized_version_pin_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let config_dir = project_root.join(".harness/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::File::create(config_dir.join("sotp-version.json"))
            .unwrap()
            .set_len(MAX_CAPABILITY_EXEC_TEXT_BYTES.saturating_add(1))
            .unwrap();

        assert!(verify(&project_root).has_errors());
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_symlinked_version_pin_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let config_dir = project_root.join(".harness/config");
        let external_pin = temp_dir.path().join("external-version-pin.json");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(&external_pin, "{}\n").unwrap();
        std::os::unix::fs::symlink(&external_pin, config_dir.join("sotp-version.json")).unwrap();

        assert!(verify(&project_root).has_errors());
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_symlinked_version_pin_parent_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let real_config_dir = temp_dir.path().join("real-config");
        std::fs::create_dir_all(project_root.join(".harness")).unwrap();
        std::fs::create_dir_all(&real_config_dir).unwrap();
        std::fs::write(real_config_dir.join("sotp-version.json"), "{}\n").unwrap();
        std::os::unix::fs::symlink(&real_config_dir, project_root.join(".harness/config")).unwrap();

        assert!(verify(&project_root).has_errors());
    }

    #[cfg(unix)]
    #[test]
    fn test_run_command_with_bounded_output_excessive_stdout_is_marked() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf 123456789"]);

        let output = run_command_with_bounded_output(&mut command, Duration::from_secs(1), 4)
            .expect("command completes");

        assert!(output.status.success());
        assert_eq!(output.stdout, b"1234");
        assert!(output.output_exceeded);
    }

    #[cfg(unix)]
    #[test]
    fn test_run_command_with_bounded_output_timeout_returns_error() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 1"]);
        let started = Instant::now();

        assert!(
            run_command_with_bounded_output(&mut command, Duration::from_millis(10), 4).is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
