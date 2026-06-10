//! Telemetry module for JSONL-format workflow telemetry.
//!
//! This module contains:
//! - `TelemetryEvent`: wire-format DTO enum (T002)
//! - `TelemetryWriteError`: error type for `TelemetryWriter` (T002)
//! - `TelemetryConfig`: env-var resolved configuration (T003)
//! - `TelemetryWriter`: O_APPEND single-write JSONL event writer (T003)
//!
//! Each variant of `TelemetryEvent` corresponds to one JSONL event line written
//! to `track/items/<id>/logs/telemetry.jsonl`.  Every variant payload includes a
//! `schema_version: u32` field so that readers can perform per-line version checks
//! (CN-09 / AC-09 / IN-08).

pub mod config;
pub mod writer;

pub use config::TelemetryConfig;
pub use writer::TelemetryWriter;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// TelemetryEvent
// ---------------------------------------------------------------------------

/// Wire format (serde DTO) for a single JSONL event line in
/// `track/items/<id>/logs/telemetry.jsonl`.
///
/// Each variant carries the event-specific fields plus a per-line
/// `schema_version` field (IN-08, CN-09).  All categorical `String` fields
/// (`command`, `gate_name`, `provider`, `model`, `hook_name`, `error_chain`,
/// `timestamp`, `verdict`, `round_type`) are free-form log labels or opaque
/// values — not domain-constrained.
///
/// `round_type` carries `"fast"` or `"final"` as a lowercase string rather
/// than an enum to keep the serde DTO self-contained at the JSONL boundary and
/// avoid coupling to `agent_profiles::RoundType` which lacks serde derives.
/// `track_id` is recorded as a plain `String` for the same DTO-boundary reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum TelemetryEvent {
    /// A track operation subcommand completed (IN-03 / AC-02).
    TrackSubcommand {
        /// Per-line schema version (CN-09 / AC-09).
        schema_version: u32,
        /// Track identifier, e.g. `"my-feature-2026-01-01"`.
        track_id: String,
        /// Subcommand name, e.g. `"track spec-design"`.
        command: String,
        /// Process exit code; `0` = success.
        exit_code: i32,
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
        /// ISO-8601 timestamp of the event.
        timestamp: String,
    },

    /// A `bin/sotp verify *` gate was evaluated (IN-03 / AC-03 / GO-01).
    GateEval {
        /// Per-line schema version (CN-09 / AC-09).
        schema_version: u32,
        /// Track identifier.
        track_id: String,
        /// Gate / verify subcommand name, e.g. `"verify-adr-signals"`.
        gate_name: String,
        /// `"ok"` or `"error"`.
        verdict: String,
        /// Short summary of findings (leading N findings, ≤ 4096 bytes).
        reason_summary: String,
        /// SHA-256 hex digest of the evaluated artifact.
        input_hash: String,
        /// Wall-clock duration of the gate evaluation in milliseconds (GO-01).
        duration_ms: u64,
        /// ISO-8601 timestamp of the event.
        timestamp: String,
    },

    /// A review or dry-check round completed (IN-03 / AC-03).
    ReviewRound {
        /// Per-line schema version (CN-09 / AC-09).
        schema_version: u32,
        /// Track identifier.
        track_id: String,
        /// Reviewer provider name, e.g. `"codex"` or `"claude"`.
        provider: String,
        /// Model identifier used for the round.
        model: String,
        /// `"fast"` or `"final"` as a lowercase string (catalogue-declared as
        /// `String` to avoid coupling to `agent_profiles::RoundType`).
        round_type: String,
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
        /// Number of findings emitted by the reviewer.
        findings_count: u32,
        /// ISO-8601 timestamp of the event.
        timestamp: String,
    },

    /// An external subprocess (e.g. Codex CLI, Gemini CLI) was invoked
    /// (IN-03 / AC-03).
    ExternalSubprocess {
        /// Per-line schema version (CN-09 / AC-09).
        schema_version: u32,
        /// Track identifier.
        track_id: String,
        /// Binary name without arguments, e.g. `"codex"`.
        command: String,
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
        /// Number of retry attempts performed (0 = no retries).
        retry_count: u32,
        /// Whether the subprocess stdout verdict could not be parsed.
        verdict_parse_failed: bool,
        /// ISO-8601 timestamp of the event.
        timestamp: String,
    },

    /// A hook blocked the operation (IN-03 / AC-04).
    HookBlock {
        /// Per-line schema version (CN-09 / AC-09).
        schema_version: u32,
        /// Track identifier.
        track_id: String,
        /// Hook identifier that triggered the block.
        hook_name: String,
        /// ISO-8601 timestamp of the event.
        timestamp: String,
    },

    /// An advisory (injection-type) hook fired (IN-03 / AC-04).
    AdvisoryHookFired {
        /// Per-line schema version (CN-09 / AC-09).
        schema_version: u32,
        /// Track identifier.
        track_id: String,
        /// Hook identifier that fired.
        hook_name: String,
        /// ISO-8601 timestamp of the event.
        timestamp: String,
    },

    /// A subcommand exited with a non-zero exit code (IN-03 / AC-02).
    NonZeroExit {
        /// Per-line schema version (CN-09 / AC-09).
        schema_version: u32,
        /// Track identifier.
        track_id: String,
        /// Subcommand name.
        command: String,
        /// Non-zero exit code.
        exit_code: i32,
        /// Human-readable error chain (may be truncated to ≤ 256 bytes by
        /// `TelemetryWriter` when the overall line would exceed 4096 bytes).
        error_chain: String,
        /// ISO-8601 timestamp of the event.
        timestamp: String,
    },
}

// ---------------------------------------------------------------------------
// TelemetryWriteError
// ---------------------------------------------------------------------------

/// Failure modes for `TelemetryWriter::write` (T003).
///
/// `TelemetryWriter::write` returns `Result<(), TelemetryWriteError>` so the
/// error is observable (e.g. in tests), but the composition root caller
/// suppresses it via fire-and-forget (CN-01 / diagnostic-only).
#[derive(Debug, Error)]
pub enum TelemetryWriteError {
    /// JSON serialization of a `TelemetryEvent` failed.
    #[error("telemetry serialize error: {message}")]
    Serialize {
        /// `serde_json` error message.
        message: String,
    },

    /// An I/O error occurred while opening or writing the JSONL file.
    #[error("telemetry I/O error writing to {path}: {message}")]
    Io {
        /// Filesystem path of the JSONL output file.
        path: String,
        /// Underlying I/O error message.
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // --- TelemetryEvent::TrackSubcommand ---

    #[test]
    fn test_track_subcommand_serialized_json_contains_schema_version() {
        let event = TelemetryEvent::TrackSubcommand {
            schema_version: 1,
            track_id: "my-feature-2026-01-01".to_string(),
            command: "track spec-design".to_string(),
            exit_code: 0,
            duration_ms: 12_345,
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(
            json.contains("\"schema_version\":1"),
            "schema_version must be present in serialized JSON; got: {json}"
        );
    }

    #[test]
    fn test_track_subcommand_serde_round_trip() {
        let event = TelemetryEvent::TrackSubcommand {
            schema_version: 1,
            track_id: "my-feature-2026-01-01".to_string(),
            command: "track spec-design".to_string(),
            exit_code: 0,
            duration_ms: 42,
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: TelemetryEvent = serde_json::from_str(&json).unwrap();

        if let TelemetryEvent::TrackSubcommand {
            schema_version,
            track_id,
            command,
            exit_code,
            duration_ms,
            timestamp,
        } = decoded
        {
            assert_eq!(schema_version, 1);
            assert_eq!(track_id, "my-feature-2026-01-01");
            assert_eq!(command, "track spec-design");
            assert_eq!(exit_code, 0);
            assert_eq!(duration_ms, 42);
            assert_eq!(timestamp, "2026-06-10T00:00:00Z");
        } else {
            panic!("decoded to wrong variant");
        }
    }

    // --- TelemetryEvent::GateEval ---

    #[test]
    fn test_gate_eval_serde_round_trip() {
        let event = TelemetryEvent::GateEval {
            schema_version: 1,
            track_id: "my-feature-2026-01-01".to_string(),
            gate_name: "verify-adr-signals".to_string(),
            verdict: "ok".to_string(),
            reason_summary: "".to_string(),
            input_hash: "abc123".to_string(),
            duration_ms: 500,
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: TelemetryEvent = serde_json::from_str(&json).unwrap();

        if let TelemetryEvent::GateEval {
            schema_version, gate_name, verdict, duration_ms, ..
        } = decoded
        {
            assert_eq!(schema_version, 1);
            assert_eq!(gate_name, "verify-adr-signals");
            assert_eq!(verdict, "ok");
            assert_eq!(duration_ms, 500);
        } else {
            panic!("decoded to wrong variant");
        }
    }

    #[test]
    fn test_gate_eval_serialized_json_contains_schema_version() {
        let event = TelemetryEvent::GateEval {
            schema_version: 1,
            track_id: "t".to_string(),
            gate_name: "g".to_string(),
            verdict: "ok".to_string(),
            reason_summary: "".to_string(),
            input_hash: "h".to_string(),
            duration_ms: 1,
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(
            json.contains("\"schema_version\":1"),
            "schema_version must be present; got: {json}"
        );
    }

    // --- TelemetryEvent::ReviewRound (round_type as String) ---

    #[test]
    fn test_review_round_round_type_fast_round_trips_as_string() {
        let event = TelemetryEvent::ReviewRound {
            schema_version: 1,
            track_id: "t".to_string(),
            provider: "codex".to_string(),
            model: "o4-mini".to_string(),
            round_type: "fast".to_string(),
            duration_ms: 3_000,
            findings_count: 2,
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: TelemetryEvent = serde_json::from_str(&json).unwrap();

        if let TelemetryEvent::ReviewRound { round_type, .. } = decoded {
            assert_eq!(round_type, "fast");
        } else {
            panic!("decoded to wrong variant");
        }
    }

    #[test]
    fn test_review_round_round_type_final_round_trips_as_string() {
        let event = TelemetryEvent::ReviewRound {
            schema_version: 1,
            track_id: "t".to_string(),
            provider: "codex".to_string(),
            model: "o4-mini".to_string(),
            round_type: "final".to_string(),
            duration_ms: 4_000,
            findings_count: 0,
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();

        // The JSON should contain the literal string "final"
        assert!(
            json.contains("\"final\""),
            "round_type 'final' must serialize as a string; got: {json}"
        );

        let decoded: TelemetryEvent = serde_json::from_str(&json).unwrap();
        if let TelemetryEvent::ReviewRound { round_type, .. } = decoded {
            assert_eq!(round_type, "final");
        } else {
            panic!("decoded to wrong variant");
        }
    }

    // --- TelemetryEvent: all other variants round-trip ---

    #[test]
    fn test_external_subprocess_serde_round_trip() {
        let event = TelemetryEvent::ExternalSubprocess {
            schema_version: 1,
            track_id: "t".to_string(),
            command: "codex".to_string(),
            duration_ms: 8_000,
            retry_count: 1,
            verdict_parse_failed: false,
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: TelemetryEvent = serde_json::from_str(&json).unwrap();

        if let TelemetryEvent::ExternalSubprocess {
            schema_version,
            command,
            retry_count,
            verdict_parse_failed,
            ..
        } = decoded
        {
            assert_eq!(schema_version, 1);
            assert_eq!(command, "codex");
            assert_eq!(retry_count, 1);
            assert!(!verdict_parse_failed);
        } else {
            panic!("decoded to wrong variant");
        }
    }

    #[test]
    fn test_hook_block_serde_round_trip() {
        let event = TelemetryEvent::HookBlock {
            schema_version: 1,
            track_id: "t".to_string(),
            hook_name: "block-direct-git-ops".to_string(),
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: TelemetryEvent = serde_json::from_str(&json).unwrap();

        if let TelemetryEvent::HookBlock { schema_version, hook_name, .. } = decoded {
            assert_eq!(schema_version, 1);
            assert_eq!(hook_name, "block-direct-git-ops");
        } else {
            panic!("decoded to wrong variant");
        }
    }

    #[test]
    fn test_advisory_hook_fired_serde_round_trip() {
        let event = TelemetryEvent::AdvisoryHookFired {
            schema_version: 1,
            track_id: "t".to_string(),
            hook_name: "skill-compliance".to_string(),
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: TelemetryEvent = serde_json::from_str(&json).unwrap();

        if let TelemetryEvent::AdvisoryHookFired { schema_version, hook_name, .. } = decoded {
            assert_eq!(schema_version, 1);
            assert_eq!(hook_name, "skill-compliance");
        } else {
            panic!("decoded to wrong variant");
        }
    }

    #[test]
    fn test_non_zero_exit_serde_round_trip() {
        let event = TelemetryEvent::NonZeroExit {
            schema_version: 1,
            track_id: "t".to_string(),
            command: "track spec-design".to_string(),
            exit_code: 1,
            error_chain: "spec gate failed".to_string(),
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: TelemetryEvent = serde_json::from_str(&json).unwrap();

        if let TelemetryEvent::NonZeroExit { schema_version, exit_code, error_chain, .. } = decoded
        {
            assert_eq!(schema_version, 1);
            assert_eq!(exit_code, 1);
            assert_eq!(error_chain, "spec gate failed");
        } else {
            panic!("decoded to wrong variant");
        }
    }

    // --- TelemetryWriteError::Io implements Display ---

    #[test]
    fn test_telemetry_write_error_io_implements_display() {
        let err = TelemetryWriteError::Io {
            path: "/tmp/telemetry.jsonl".to_string(),
            message: "permission denied".to_string(),
        };
        let display = format!("{err}");
        assert!(
            display.contains("/tmp/telemetry.jsonl"),
            "Display should include path; got: {display}"
        );
        assert!(
            display.contains("permission denied"),
            "Display should include message; got: {display}"
        );
    }

    #[test]
    fn test_telemetry_write_error_serialize_implements_display() {
        let err = TelemetryWriteError::Serialize { message: "invalid UTF-8".to_string() };
        let display = format!("{err}");
        assert!(
            display.contains("invalid UTF-8"),
            "Display should include message; got: {display}"
        );
    }

    // --- Negative deserialization: unknown event_type is rejected ---

    #[test]
    fn test_deserialize_unknown_event_type_returns_error() {
        // A JSONL row with an unrecognised event_type must not deserialise into
        // TelemetryEvent (internally-tagged serde with `#[serde(tag = "event_type")]`).
        let json = r#"{"event_type":"UnknownVariant","schema_version":1,"track_id":"t","timestamp":"2026-06-10T00:00:00Z"}"#;
        let result = serde_json::from_str::<TelemetryEvent>(json);
        assert!(
            result.is_err(),
            "Expected error for unknown event_type, but deserialization succeeded: {result:?}"
        );
    }

    // --- Negative deserialization: missing required field (schema_version) ---

    #[test]
    fn test_deserialize_missing_schema_version_returns_error() {
        // A row that is otherwise valid but omits schema_version must be rejected,
        // ensuring per-line schema_version is always present (IN-08 / CN-09).
        let json = r#"{"event_type":"HookBlock","track_id":"t","hook_name":"block-direct-git-ops","timestamp":"2026-06-10T00:00:00Z"}"#;
        let result = serde_json::from_str::<TelemetryEvent>(json);
        assert!(
            result.is_err(),
            "Expected error for missing schema_version, but deserialization succeeded: {result:?}"
        );
    }

    // --- Negative deserialization: missing required field (event_type tag) ---

    #[test]
    fn test_deserialize_missing_event_type_tag_returns_error() {
        // A row missing the discriminant tag entirely must be rejected.
        let json = r#"{"schema_version":1,"track_id":"t","hook_name":"block-direct-git-ops","timestamp":"2026-06-10T00:00:00Z"}"#;
        let result = serde_json::from_str::<TelemetryEvent>(json);
        assert!(
            result.is_err(),
            "Expected error for missing event_type tag, but deserialization succeeded: {result:?}"
        );
    }

    // --- Negative deserialization: completely malformed JSON ---

    #[test]
    fn test_deserialize_malformed_json_returns_error() {
        let json = "not valid json at all";
        let result = serde_json::from_str::<TelemetryEvent>(json);
        assert!(result.is_err(), "Expected error for malformed JSON input");
    }
}
