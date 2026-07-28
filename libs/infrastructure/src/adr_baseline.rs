//! Filesystem and Git adapters for append-only ADR baseline snapshots.

mod source;
mod store;

pub use source::FsGitAdrBaselineSource;
pub use store::FsAdrBaselineStore;

#[cfg(test)]
mod tests;

use domain::adr_baseline::{AdrBaselineKind, AdrBaselineLedgerEntry, AdrSourceFileName};
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::{ContentHash, NonEmptyString, Timestamp};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use usecase::adr_baseline::{AdrBaselineTimestampError, ClockPort};

/// Infrastructure adapter that supplies the current system timestamp.
pub struct SystemClockAdapter;

impl ClockPort for SystemClockAdapter {
    fn now(&self) -> Result<Timestamp, AdrBaselineTimestampError> {
        crate::timestamp_now().map_err(AdrBaselineTimestampError::InvalidTimestamp)
    }
}

/// Serde-facing representation of one typed ledger entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdrBaselineLedgerRecordDto {
    /// Direct child filename under `knowledge/adr`.
    pub source: AdrSourceFileName,
    /// Full SHA-256 of the snapshot bytes.
    pub hash: ContentHash,
    /// Snapshot kind.
    pub kind: AdrBaselineKind,
    /// Required only for `new-adr` and `escalation` records.
    pub reason: Option<NonEmptyString>,
    /// Record creation instant.
    pub timestamp: Timestamp,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLedgerRecord {
    source: String,
    hash: String,
    kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    timestamp: String,
}

impl Serialize for AdrBaselineLedgerRecordDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RawLedgerRecord {
            source: self.source.as_str().to_owned(),
            hash: self.hash.to_hex(),
            kind: kind_name(&self.kind).to_owned(),
            reason: self.reason.as_ref().map(ToString::to_string),
            timestamp: self.timestamp.as_str().to_owned(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AdrBaselineLedgerRecordDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawLedgerRecord::deserialize(deserializer)?;
        dto_from_raw(raw).map_err(serde::de::Error::custom)
    }
}

/// Ledger codec failures.
#[derive(Debug, Error)]
pub enum AdrBaselineCodecError {
    /// JSON syntax or serialization failed.
    #[error("ADR baseline ledger JSON error: {0:?}")]
    Json(DiagnosticMessage),
    /// A decoded record violated the typed domain contract.
    #[error("ADR baseline ledger domain error: {0:?}")]
    Domain(DiagnosticMessage),
}

/// Encodes a ledger entry as one JSONL line (without the trailing newline).
///
/// # Errors
///
/// Returns [`AdrBaselineCodecError::Json`] when serialization fails.
pub fn encode_ledger_entry(
    entry: &AdrBaselineLedgerEntry,
) -> Result<String, AdrBaselineCodecError> {
    serde_json::to_string(&dto_from_entry(entry))
        .map_err(|error| AdrBaselineCodecError::Json(diagnostic(&error.to_string())))
}

/// Decodes one JSONL ledger line into a validated domain entry.
///
/// # Errors
///
/// Returns [`AdrBaselineCodecError::Json`] for malformed JSON and
/// [`AdrBaselineCodecError::Domain`] when decoded data violates the ledger contract.
pub fn decode_ledger_line(line: &str) -> Result<AdrBaselineLedgerEntry, AdrBaselineCodecError> {
    let raw = serde_json::from_str::<RawLedgerRecord>(line)
        .map_err(|error| AdrBaselineCodecError::Json(diagnostic(&error.to_string())))?;
    let dto =
        dto_from_raw(raw).map_err(|error| AdrBaselineCodecError::Domain(diagnostic(&error)))?;
    entry_from_dto(dto)
}

fn dto_from_entry(entry: &AdrBaselineLedgerEntry) -> AdrBaselineLedgerRecordDto {
    AdrBaselineLedgerRecordDto {
        source: entry.source().clone(),
        hash: entry.hash().clone(),
        kind: entry.kind(),
        reason: entry.reason().cloned(),
        timestamp: entry.timestamp().clone(),
    }
}

fn entry_from_dto(
    dto: AdrBaselineLedgerRecordDto,
) -> Result<AdrBaselineLedgerEntry, AdrBaselineCodecError> {
    if dto.kind.requires_reason() != dto.reason.is_some() {
        return Err(AdrBaselineCodecError::Domain(diagnostic(
            "ledger kind and reason are incompatible",
        )));
    }
    make_entry(dto.source, dto.hash, dto.kind, dto.reason, dto.timestamp)
        .map_err(AdrBaselineCodecError::Domain)
}

fn dto_from_raw(raw: RawLedgerRecord) -> Result<AdrBaselineLedgerRecordDto, String> {
    let source = AdrSourceFileName::try_new(raw.source).map_err(|error| error.to_string())?;
    let hash = ContentHash::try_from_hex(raw.hash).map_err(|error| error.to_string())?;
    let kind = parse_kind(&raw.kind)
        .ok_or_else(|| format!("unsupported ADR baseline kind: {}", raw.kind))?;
    let reason =
        raw.reason.map(NonEmptyString::try_new).transpose().map_err(|error| error.to_string())?;
    let timestamp = Timestamp::new(raw.timestamp).map_err(|error| error.to_string())?;
    if kind.requires_reason() != reason.is_some() {
        return Err("ledger kind and reason are incompatible".to_owned());
    }
    Ok(AdrBaselineLedgerRecordDto { source, hash, kind, reason, timestamp })
}

fn make_entry(
    source: AdrSourceFileName,
    hash: ContentHash,
    kind: AdrBaselineKind,
    reason: Option<NonEmptyString>,
    timestamp: Timestamp,
) -> Result<AdrBaselineLedgerEntry, DiagnosticMessage> {
    match (kind, reason) {
        (AdrBaselineKind::Init, None) => {
            Ok(AdrBaselineLedgerEntry::Init { source, hash, timestamp })
        }
        (AdrBaselineKind::Cite, None) => {
            Ok(AdrBaselineLedgerEntry::Cite { source, hash, timestamp })
        }
        (AdrBaselineKind::NewAdr, Some(reason)) => {
            Ok(AdrBaselineLedgerEntry::NewAdr { source, hash, reason, timestamp })
        }
        (AdrBaselineKind::NonSemanticFix, None) => {
            Ok(AdrBaselineLedgerEntry::NonSemanticFix { source, hash, timestamp })
        }
        (AdrBaselineKind::Escalation, Some(reason)) => {
            Ok(AdrBaselineLedgerEntry::Escalation { source, hash, reason, timestamp })
        }
        (_, _) => Err(diagnostic("ledger kind and reason are incompatible")),
    }
}

fn kind_name(kind: &AdrBaselineKind) -> &'static str {
    match kind {
        AdrBaselineKind::Init => "init",
        AdrBaselineKind::Cite => "cite",
        AdrBaselineKind::NewAdr => "new-adr",
        AdrBaselineKind::NonSemanticFix => "non-semantic-fix",
        AdrBaselineKind::Escalation => "escalation",
    }
}

fn parse_kind(value: &str) -> Option<AdrBaselineKind> {
    match value {
        "init" => Some(AdrBaselineKind::Init),
        "cite" => Some(AdrBaselineKind::Cite),
        "new-adr" => Some(AdrBaselineKind::NewAdr),
        "non-semantic-fix" => Some(AdrBaselineKind::NonSemanticFix),
        "escalation" => Some(AdrBaselineKind::Escalation),
        _ => None,
    }
}

fn diagnostic(message: &str) -> DiagnosticMessage {
    let mut text = if message.trim().is_empty() {
        "ADR baseline adapter failure".to_owned()
    } else {
        message.to_owned()
    };
    loop {
        match DiagnosticMessage::try_new(text) {
            Ok(message) => return message,
            Err(_) => text = "ADR baseline adapter failure".to_owned(),
        }
    }
}
