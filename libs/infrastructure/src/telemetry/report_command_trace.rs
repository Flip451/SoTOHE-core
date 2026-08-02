//! Private decoding support for persisted completed-command trace records.

use serde::Deserialize;
use usecase::telemetry::command_trace::{CommandExitCode, SotpCommandIdentity};

pub(super) struct DecodedCommandTrace {
    pub(super) command: SotpCommandIdentity,
    pub(super) duration_ms: u64,
    pub(super) failed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedCommandTraceRecord {
    #[serde(default)]
    schema_version: PersistedCommandTraceSchemaVersion,
    command: String,
    duration_ms: u64,
    result: PersistedCommandTraceResult,
}

/// Field omission predates schema versioning and is accepted for backward compatibility.
/// An explicit null remains malformed: only concrete unsigned values deserialize.
#[derive(Debug, Default)]
enum PersistedCommandTraceSchemaVersion {
    #[default]
    Absent,
    Present(u32),
}

impl<'de> Deserialize<'de> for PersistedCommandTraceSchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        u32::deserialize(deserializer).map(Self::Present)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PersistedCommandTraceResult {
    Success(PersistedCommandTraceSuccess),
    Failure(PersistedCommandTraceFailure),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedCommandTraceSuccess {
    #[serde(rename = "status")]
    _status: PersistedCommandTraceSuccessStatus,
}

#[derive(Debug, Deserialize)]
enum PersistedCommandTraceSuccessStatus {
    #[serde(rename = "success")]
    Success,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedCommandTraceFailure {
    #[serde(rename = "status")]
    _status: PersistedCommandTraceFailureStatus,
    exit_code: i32,
}

#[derive(Debug, Deserialize)]
enum PersistedCommandTraceFailureStatus {
    #[serde(rename = "failure")]
    Failure,
}

pub(super) fn decode(line: &[u8], known_schema_versions: &[u32]) -> Option<DecodedCommandTrace> {
    let record = serde_json::from_slice::<PersistedCommandTraceRecord>(line).ok()?;
    if matches!(
        record.schema_version,
        PersistedCommandTraceSchemaVersion::Present(version)
            if !known_schema_versions.contains(&version)
    ) {
        return None;
    }

    let command = SotpCommandIdentity::try_new(record.command).ok()?;
    let failed = match record.result {
        PersistedCommandTraceResult::Success(_) => false,
        PersistedCommandTraceResult::Failure(PersistedCommandTraceFailure {
            exit_code, ..
        }) => {
            let exit_code = u8::try_from(exit_code).ok()?;
            CommandExitCode::try_new(exit_code).ok()?;
            true
        }
    };

    Some(DecodedCommandTrace { command, duration_ms: record.duration_ms, failed })
}
