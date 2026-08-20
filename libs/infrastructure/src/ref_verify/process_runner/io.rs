//! Bounded stream collection and session-log helpers for the shared runner.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::thread::JoinHandle;

use super::{MAX_REF_VERIFY_SESSION_LOG_BYTES, ref_verify_runner_error};
use usecase::ref_verify::RefVerifyError;

pub(super) struct BoundedStdout {
    pub(super) text: String,
    pub(super) exceeded_limit: bool,
}

pub(super) fn spawn_bounded_stdout_reader<R>(
    pipe: R,
    maximum_bytes: usize,
) -> JoinHandle<std::io::Result<BoundedStdout>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || collect_bounded_stdout(pipe, maximum_bytes))
}

fn collect_bounded_stdout<R>(mut pipe: R, maximum_bytes: usize) -> std::io::Result<BoundedStdout>
where
    R: Read,
{
    let mut buffer = [0_u8; 8192];
    let mut retained = Vec::new();
    let mut exceeded_limit = false;
    loop {
        let read = pipe.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let bytes = buffer.get(..read).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid stdout read count")
        })?;
        if exceeded_limit {
            continue;
        }
        if retained.len().saturating_add(bytes.len()) > maximum_bytes {
            exceeded_limit = true;
            retained.clear();
            continue;
        }
        retained.extend_from_slice(bytes);
    }
    if exceeded_limit {
        return Ok(BoundedStdout { text: String::new(), exceeded_limit: true });
    }
    let text = String::from_utf8(retained)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(BoundedStdout { text, exceeded_limit: false })
}

pub(super) struct RefVerifySessionLog {
    pub(super) path: PathBuf,
    pub(super) written_bytes: usize,
}

pub(super) struct BoundedStderr {
    pub(super) bytes: Vec<u8>,
    pub(super) truncated: bool,
}

pub(super) fn spawn_bounded_stderr_reader<R>(
    pipe: R,
    maximum_bytes: usize,
) -> JoinHandle<std::io::Result<BoundedStderr>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || collect_bounded_stderr(pipe, maximum_bytes))
}

pub(super) fn collect_bounded_stderr<R>(
    mut pipe: R,
    maximum_bytes: usize,
) -> std::io::Result<BoundedStderr>
where
    R: Read,
{
    let mut buffer = [0_u8; 8192];
    let mut retained = Vec::new();
    let mut truncated = false;
    loop {
        let read = pipe.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let bytes = buffer.get(..read).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid stderr read count")
        })?;
        if bytes.len() >= maximum_bytes {
            retained.clear();
            retained.extend_from_slice(
                bytes.get(bytes.len().saturating_sub(maximum_bytes)..).unwrap_or_default(),
            );
            truncated = true;
            continue;
        }
        let overflow = retained.len().saturating_add(bytes.len()).saturating_sub(maximum_bytes);
        if overflow > 0 {
            retained.drain(..overflow);
            truncated = true;
        }
        retained.extend_from_slice(bytes);
    }
    Ok(BoundedStderr { bytes: retained, truncated })
}

pub(super) fn append_session_log(
    session_log: &mut RefVerifySessionLog,
    bytes: &[u8],
) -> Result<(), RefVerifyError> {
    let remaining = MAX_REF_VERIFY_SESSION_LOG_BYTES.saturating_sub(session_log.written_bytes);
    let retained = bytes.get(..bytes.len().min(remaining)).unwrap_or_default();
    if retained.is_empty() {
        return Ok(());
    }
    let mut file =
        std::fs::OpenOptions::new().append(true).open(&session_log.path).map_err(|error| {
            ref_verify_runner_error(format!(
                "failed to open ref-verify session log {}: {error}",
                session_log.path.display()
            ))
        })?;
    file.write_all(retained).map_err(|error| {
        ref_verify_runner_error(format!(
            "failed to write ref-verify session log {}: {error}",
            session_log.path.display()
        ))
    })?;
    session_log.written_bytes = session_log.written_bytes.saturating_add(retained.len());
    Ok(())
}

pub(super) fn stderr_tail(text: &str, max_lines: usize, max_bytes: usize) -> String {
    if text.is_empty() {
        return String::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    let mut tail = lines.get(start..).map(|s| s.join("\n")).unwrap_or_default();
    if tail.len() > max_bytes {
        let cut = tail.len() - max_bytes;
        let mut idx = cut;
        while idx < tail.len() && !tail.is_char_boundary(idx) {
            idx += 1;
        }
        tail = tail.split_off(idx);
    }
    tail
}
