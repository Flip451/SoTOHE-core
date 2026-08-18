//! Provider stdout collection and authoritative final-message file handling.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

use usecase::capability_exec::{CapabilityExecError, ProviderName};

use crate::grok_common::grok_envelope_bytes_from_stdout;

use super::termination::terminate_provider_process_group;
use super::{PROVIDER_LOG_DRAIN_TIMEOUT, dispatch_error};

pub(super) const MAX_PROVIDER_SESSION_EVENT_BYTES: usize = 64 * 1024;
pub(super) const MAX_PROVIDER_FINAL_MESSAGE_BYTES: u64 = 4 * 1024 * 1024;

pub(super) fn spawn_provider_session_collector(
    stdout: impl Read + Send + 'static,
    provider: ProviderName,
) -> Receiver<Result<ProviderCollectedOutput, std::io::Error>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(collect_provider_output(stdout, &provider));
    });
    receiver
}

pub(super) struct ProviderCollectedOutput {
    pub(super) session_id: Option<String>,
    pub(super) final_message: Option<Vec<u8>>,
}

pub(super) fn receive_provider_output(
    collector: Receiver<Result<ProviderCollectedOutput, std::io::Error>>,
    process_id: u32,
    provider: &ProviderName,
    binary: &str,
) -> Result<ProviderCollectedOutput, CapabilityExecError> {
    match collector.recv_timeout(PROVIDER_LOG_DRAIN_TIMEOUT) {
        Ok(Ok(collected)) => Ok(collected),
        Ok(Err(error)) => Err(dispatch_error(
            provider,
            format!("cannot drain stdout from {binary} provider subprocess: {error}"),
        )),
        Err(RecvTimeoutError::Timeout) => {
            let _ = terminate_provider_process_group(process_id, provider, binary);
            Err(dispatch_error(
                provider,
                format!(
                    "stdout drain did not close within {} seconds after {binary} exited",
                    PROVIDER_LOG_DRAIN_TIMEOUT.as_secs(),
                ),
            ))
        }
        Err(RecvTimeoutError::Disconnected) => {
            Err(dispatch_error(provider, format!("stdout collector disconnected for {binary}")))
        }
    }
}

/// Drains provider output while retaining only bounded protocol fields.
///
/// Provider event envelopes are a control channel, never the capability result.
/// Claude's single JSON envelope carries its final `result`, Grok's JSON
/// envelope carries its structured-output boundary, and Codex writes its
/// authoritative final message to `--output-last-message` instead.
pub(super) fn collect_provider_output<R: Read>(
    pipe: R,
    provider: &ProviderName,
) -> Result<ProviderCollectedOutput, std::io::Error> {
    if provider.as_str() == "grok" {
        return collect_grok_provider_output(pipe);
    }
    let mut reader = BufReader::new(pipe);
    let max_event_bytes = max_provider_event_bytes(provider);
    let mut event = Vec::with_capacity(max_event_bytes);
    let mut discarding_event = false;
    let mut session_id = None;
    let mut final_message = None;

    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            break;
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let event_bytes = newline.unwrap_or(buffer.len());
        let consumed = newline.map_or(buffer.len(), |index| index.saturating_add(1));
        if !discarding_event {
            let remaining = max_event_bytes.saturating_sub(event.len());
            if event_bytes <= remaining {
                if let Some(part) = buffer.get(..event_bytes) {
                    event.extend_from_slice(part);
                }
            } else {
                discarding_event = true;
            }
        }
        reader.consume(consumed);
        if newline.is_some() {
            if !discarding_event {
                collect_event_fields(&event, provider, &mut session_id, &mut final_message);
            }
            event.clear();
            discarding_event = false;
        }
    }
    if !discarding_event {
        collect_event_fields(&event, provider, &mut session_id, &mut final_message);
    }
    Ok(ProviderCollectedOutput { session_id, final_message })
}

fn collect_grok_provider_output<R: Read>(
    mut pipe: R,
) -> Result<ProviderCollectedOutput, std::io::Error> {
    let mut stdout = Vec::new();
    pipe.by_ref()
        .take(MAX_PROVIDER_FINAL_MESSAGE_BYTES.saturating_add(1))
        .read_to_end(&mut stdout)?;
    // Drain remaining stdout so a verbose Grok child cannot block on a full pipe.
    std::io::copy(&mut pipe, &mut std::io::sink())?;
    if stdout.len() > MAX_PROVIDER_FINAL_MESSAGE_BYTES as usize {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("Grok stdout exceeds {MAX_PROVIDER_FINAL_MESSAGE_BYTES} bytes"),
        ));
    }
    let final_message = grok_envelope_bytes_from_stdout(&stdout);
    let session_id = session_id_from_grok_stdout(&stdout);
    Ok(ProviderCollectedOutput { session_id, final_message })
}

/// Session metadata can arrive on an earlier NDJSON event than the selected
/// structured-result envelope. Scan the retained stdout independently.
fn session_id_from_grok_stdout(stdout: &[u8]) -> Option<String> {
    let mut session_id = None;
    for line in stdout.split(|byte| *byte == b'\n') {
        if session_id.is_none() {
            session_id = extract_provider_session_id(line);
        }
    }
    if session_id.is_none() {
        session_id = extract_provider_session_id(stdout);
    }
    session_id
}

fn max_provider_event_bytes(provider: &ProviderName) -> usize {
    if provider.as_str() == "grok" {
        usize::try_from(MAX_PROVIDER_FINAL_MESSAGE_BYTES).unwrap_or(usize::MAX)
    } else {
        MAX_PROVIDER_SESSION_EVENT_BYTES
    }
}

fn collect_event_fields(
    event: &[u8],
    provider: &ProviderName,
    session_id: &mut Option<String>,
    final_message: &mut Option<Vec<u8>>,
) {
    if session_id.is_none() {
        *session_id = extract_provider_session_id(event);
    }
    if final_message.is_none() && provider.as_str() == "claude" {
        *final_message = extract_claude_final_message(event);
    }
    if final_message.is_none() && provider.as_str() == "grok" {
        *final_message = extract_grok_final_message(event);
    }
}

fn extract_provider_session_id(event: &[u8]) -> Option<String> {
    let event = std::str::from_utf8(event).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(event.trim()).ok()?;
    value
        .get("session_id")
        .or_else(|| value.get("thread_id"))
        .or_else(|| value.get("sessionId"))?
        .as_str()
        .filter(|id| !id.trim().is_empty())
        .map(str::to_owned)
}

fn extract_claude_final_message(event: &[u8]) -> Option<Vec<u8>> {
    let event = std::str::from_utf8(event).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(event.trim()).ok()?;
    value.get("result")?.as_str().map(|result| result.as_bytes().to_vec())
}

fn extract_grok_final_message(event: &[u8]) -> Option<Vec<u8>> {
    let event_text = std::str::from_utf8(event).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(event_text.trim()).ok()?;
    let is_envelope = value.get("structured_output").is_some()
        || value.get("failure_reason").is_some()
        || value.get("text").is_some()
        || value.get("error").is_some();
    let is_terminal_stream_event = value.get("type").and_then(serde_json::Value::as_str)
        == Some("end")
        || value.get("type").and_then(serde_json::Value::as_str) == Some("error");
    (is_envelope || is_terminal_stream_event).then(|| event_text.trim().as_bytes().to_vec())
}

pub(super) fn initialize_output_last_message(
    path: &Path,
    runtime_dir: &Path,
    provider: &ProviderName,
) -> Result<PathBuf, CapabilityExecError> {
    validate_output_last_message_parent(path, runtime_dir, provider)?;
    if let Ok(metadata) = path.symlink_metadata() {
        if metadata.file_type().is_symlink() {
            return Err(dispatch_error(
                provider,
                format!("refusing to follow symlink: {}", path.display()),
            ));
        }
        if !metadata.is_file() {
            return Err(dispatch_error(
                provider,
                format!("output-last-message path is not a regular file: {}", path.display()),
            ));
        }
    }
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    let file = open_output_last_message_nofollow(path, &mut options).map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot initialize output-last-message {}: {error}", path.display()),
        )
    })?;
    let opened_metadata = file.metadata().map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot inspect output-last-message {}: {error}", path.display()),
        )
    })?;
    if opened_metadata.file_type().is_symlink() || !opened_metadata.is_file() {
        return Err(dispatch_error(
            provider,
            format!("output-last-message path is not a regular file: {}", path.display()),
        ));
    }
    file.set_len(0).map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot initialize output-last-message {}: {error}", path.display()),
        )
    })?;
    Ok(path.to_path_buf())
}

/// Confirms the provider's output path is directly below the currently trusted runtime dir.
///
/// This is called both before spawning and after the provider exits because the provider can
/// replace its writable runtime directory while it runs.
pub(super) fn validate_output_last_message_parent(
    path: &Path,
    runtime_dir: &Path,
    provider: &ProviderName,
) -> Result<(), CapabilityExecError> {
    let parent = path.parent().ok_or_else(|| {
        dispatch_error(
            provider,
            format!("output-last-message path has no parent: {}", path.display()),
        )
    })?;
    let canonical_parent = parent.canonicalize().map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot resolve output-last-message parent {}: {error}", parent.display()),
        )
    })?;
    if canonical_parent != runtime_dir {
        return Err(dispatch_error(
            provider,
            format!(
                "output-last-message path {} is outside runtime directory {}",
                path.display(),
                runtime_dir.display()
            ),
        ));
    }
    Ok(())
}

pub(super) fn read_output_last_message(
    path: &Path,
    provider: &ProviderName,
) -> Result<Option<Vec<u8>>, CapabilityExecError> {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(dispatch_error(
                provider,
                format!("cannot inspect output-last-message {}: {error}", path.display()),
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(dispatch_error(
            provider,
            format!("output-last-message path is not a regular file: {}", path.display()),
        ));
    }
    if metadata.len() > MAX_PROVIDER_FINAL_MESSAGE_BYTES {
        return Err(output_too_large_error(path, provider));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    let file = open_output_last_message_nofollow(path, &mut options).map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot open output-last-message {}: {error}", path.display()),
        )
    })?;
    let opened_metadata = file.metadata().map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot inspect output-last-message {}: {error}", path.display()),
        )
    })?;
    if opened_metadata.file_type().is_symlink() || !opened_metadata.is_file() {
        return Err(dispatch_error(
            provider,
            format!("output-last-message path is not a regular file: {}", path.display()),
        ));
    }
    if opened_metadata.len() > MAX_PROVIDER_FINAL_MESSAGE_BYTES {
        return Err(output_too_large_error(path, provider));
    }

    read_bounded_output_last_message(file, path, provider)
}

pub(super) fn read_bounded_output_last_message(
    reader: impl Read,
    path: &Path,
    provider: &ProviderName,
) -> Result<Option<Vec<u8>>, CapabilityExecError> {
    // Metadata is a snapshot. Read one byte past the limit so a provider that grows its
    // final-message file after inspection is rejected without an unbounded allocation.
    let mut reader = reader.take(MAX_PROVIDER_FINAL_MESSAGE_BYTES.saturating_add(1));
    let mut content = Vec::new();
    reader.read_to_end(&mut content).map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot read output-last-message {}: {error}", path.display()),
        )
    })?;
    if content.len() > MAX_PROVIDER_FINAL_MESSAGE_BYTES as usize {
        return Err(output_too_large_error(path, provider));
    }
    Ok((!content.is_empty()).then_some(content))
}

fn output_too_large_error(path: &Path, provider: &ProviderName) -> CapabilityExecError {
    dispatch_error(
        provider,
        format!(
            "output-last-message {} exceeds {MAX_PROVIDER_FINAL_MESSAGE_BYTES} bytes",
            path.display()
        ),
    )
}

/// Atomically opens the untrusted provider output leaf without following a symlink.
///
/// This fail-closes on unsupported platforms because an `symlink_metadata` check alone cannot
/// prevent a provider from replacing the file with a symlink between check and open.
fn open_output_last_message_nofollow(
    path: &Path,
    options: &mut OpenOptions,
) -> Result<File, std::io::Error> {
    #[cfg(unix)]
    {
        options.custom_flags(libc::O_NOFOLLOW);
        options.open(path)
    }
    #[cfg(windows)]
    {
        // FILE_FLAG_OPEN_REPARSE_POINT opens the reparse point itself so the handle metadata
        // validation can reject it rather than following a symlink or junction.
        options.custom_flags(0x0020_0000);
        options.open(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, options);
        Err(std::io::Error::new(
            ErrorKind::Unsupported,
            "atomic no-follow file open is unavailable on this platform",
        ))
    }
}
