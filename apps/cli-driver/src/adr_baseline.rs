//! Primary adapter for ADR baseline snapshot, restore, and freeze-check commands.

use std::str::FromStr;
use std::sync::Arc;

use usecase::adr_baseline::{
    AdrBaselineCheckOutcome, AdrBaselineCommand, AdrBaselineQuery, AdrBaselineQueryOutput,
    AdrBaselineQueryService, AdrBaselineService, AdrBaselineSnapshotKind,
    AdrBaselineTimestampError, AdrBaselineValidationError, AdrSourceFileName,
};
use usecase::{NonEmptyString, Timestamp, TrackId, ValidationError};

use crate::render::CommandOutcome;

/// Validated track-id supplied at the CLI boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackIdInput {
    value: TrackId,
}

impl FromStr for TrackIdInput {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        TrackId::try_new(value.to_owned()).map(|value| Self { value })
    }
}

impl TryFrom<String> for TrackIdInput {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl AsRef<str> for TrackIdInput {
    fn as_ref(&self) -> &str {
        self.value.as_ref()
    }
}

impl std::fmt::Display for TrackIdInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(formatter)
    }
}

/// Validated ADR filename supplied at the CLI boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdrSourceFileNameInput {
    value: AdrSourceFileName,
}

impl FromStr for AdrSourceFileNameInput {
    type Err = AdrBaselineValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        AdrSourceFileName::try_new(value.to_owned())
            .map(|value| Self { value })
            .map_err(AdrBaselineValidationError::InvalidSourceFileName)
    }
}

impl TryFrom<String> for AdrSourceFileNameInput {
    type Error = AdrBaselineValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl AsRef<str> for AdrSourceFileNameInput {
    fn as_ref(&self) -> &str {
        self.value.as_str()
    }
}

impl std::fmt::Display for AdrSourceFileNameInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(formatter)
    }
}

/// Finite snapshot kinds accepted by the CLI boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineKindInput {
    Init,
    Cite,
    NewAdr,
    NonSemanticFix,
    Escalation,
}

impl FromStr for AdrBaselineKindInput {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "init" => Ok(Self::Init),
            "cite" => Ok(Self::Cite),
            "new-adr" => Ok(Self::NewAdr),
            "non-semantic-fix" => Ok(Self::NonSemanticFix),
            "escalation" => Ok(Self::Escalation),
            _ => Err(format!("unsupported ADR baseline snapshot kind: {value}")),
        }
    }
}

impl AsRef<str> for AdrBaselineKindInput {
    fn as_ref(&self) -> &str {
        match self {
            Self::Init => "init",
            Self::Cite => "cite",
            Self::NewAdr => "new-adr",
            Self::NonSemanticFix => "non-semantic-fix",
            Self::Escalation => "escalation",
        }
    }
}

impl std::fmt::Display for AdrBaselineKindInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_ref())
    }
}

/// Validated snapshot rationale supplied at the CLI boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdrBaselineReasonInput {
    value: NonEmptyString,
}

impl FromStr for AdrBaselineReasonInput {
    type Err = AdrBaselineValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        NonEmptyString::try_new(value.to_owned())
            .map(|value| Self { value })
            .map_err(|_| AdrBaselineValidationError::InvalidReason)
    }
}

impl TryFrom<String> for AdrBaselineReasonInput {
    type Error = AdrBaselineValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl AsRef<str> for AdrBaselineReasonInput {
    fn as_ref(&self) -> &str {
        self.value.as_ref()
    }
}

impl std::fmt::Display for AdrBaselineReasonInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(formatter)
    }
}

/// Validated reason-aware snapshot input accepted by the primary adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineSnapshotInput {
    /// Initial designation snapshot.
    Init,
    /// Snapshot of a cited ADR.
    Cite,
    /// New ADR snapshot with required rationale.
    NewAdr(NonEmptyString),
    /// Non-semantic correction snapshot.
    NonSemanticFix,
    /// Escalated snapshot with required rationale.
    Escalation(NonEmptyString),
}

impl TryFrom<(AdrBaselineKindInput, Option<AdrBaselineReasonInput>)> for AdrBaselineSnapshotInput {
    type Error = AdrBaselineValidationError;

    fn try_from(
        (kind, reason): (AdrBaselineKindInput, Option<AdrBaselineReasonInput>),
    ) -> Result<Self, Self::Error> {
        match (kind, reason) {
            (AdrBaselineKindInput::Init, None) => Ok(Self::Init),
            (AdrBaselineKindInput::Cite, None) => Ok(Self::Cite),
            (AdrBaselineKindInput::NewAdr, Some(reason)) => Ok(Self::NewAdr(reason.value)),
            (AdrBaselineKindInput::NonSemanticFix, None) => Ok(Self::NonSemanticFix),
            (AdrBaselineKindInput::Escalation, Some(reason)) => Ok(Self::Escalation(reason.value)),
            _ => Err(AdrBaselineValidationError::InvalidReason),
        }
    }
}

impl From<AdrBaselineSnapshotInput> for AdrBaselineSnapshotKind {
    fn from(input: AdrBaselineSnapshotInput) -> Self {
        match input {
            AdrBaselineSnapshotInput::Init => Self::Init,
            AdrBaselineSnapshotInput::Cite => Self::Cite,
            AdrBaselineSnapshotInput::NewAdr(reason) => Self::NewAdr(reason),
            AdrBaselineSnapshotInput::NonSemanticFix => Self::NonSemanticFix,
            AdrBaselineSnapshotInput::Escalation(reason) => Self::Escalation(reason),
        }
    }
}

/// Resolved ADR baseline operations accepted by the primary adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineInput {
    Snapshot {
        track_id: TrackIdInput,
        source: AdrSourceFileNameInput,
        kind: AdrBaselineSnapshotInput,
    },
    Restore {
        track_id: TrackIdInput,
        source: AdrSourceFileNameInput,
    },
    CheckReview {
        track_id: TrackIdInput,
        primary_source: Option<AdrSourceFileNameInput>,
    },
    CheckCommit {
        track_id: TrackIdInput,
    },
}

/// ADR baseline primary adapter dispatching typed command and query services.
pub struct AdrBaselineDriver {
    command_service: Arc<dyn AdrBaselineService>,
    query_service: Arc<dyn AdrBaselineQueryService>,
    timestamp_provider: fn() -> Result<Timestamp, AdrBaselineTimestampError>,
}

impl AdrBaselineDriver {
    /// Creates an ADR baseline driver from its application services and timestamp provider.
    #[must_use]
    pub fn new(
        command_service: Arc<dyn AdrBaselineService>,
        query_service: Arc<dyn AdrBaselineQueryService>,
        timestamp_provider: fn() -> Result<Timestamp, AdrBaselineTimestampError>,
    ) -> Self {
        Self { command_service, query_service, timestamp_provider }
    }

    /// Maps one validated CLI request to the matching use-case operation.
    #[must_use]
    pub fn handle(&self, input: AdrBaselineInput) -> CommandOutcome {
        match input {
            AdrBaselineInput::Snapshot { track_id, source, kind } => {
                let timestamp = match (self.timestamp_provider)() {
                    Ok(timestamp) => timestamp,
                    Err(error) => {
                        return CommandOutcome::failure(Some(error.to_string()));
                    }
                };
                self.command_service
                    .execute(AdrBaselineCommand::Snapshot {
                        track_id: track_id.value,
                        source: source.value,
                        kind: kind.into(),
                        timestamp,
                    })
                    .map(|output| {
                        CommandOutcome::success(Some(format!("ADR baseline snapshot: {output:?}")))
                    })
                    .unwrap_or_else(|error| CommandOutcome::failure(Some(error.to_string())))
            }
            AdrBaselineInput::Restore { track_id, source } => self
                .command_service
                .execute(AdrBaselineCommand::Restore {
                    track_id: track_id.value,
                    source: source.value,
                })
                .map(|output| {
                    CommandOutcome::success(Some(format!("ADR baseline restore: {output:?}")))
                })
                .unwrap_or_else(|error| CommandOutcome::failure(Some(error.to_string()))),
            AdrBaselineInput::CheckReview { track_id, primary_source } => self
                .query_service
                .execute(AdrBaselineQuery::CheckReview {
                    track_id: track_id.value,
                    primary_source: primary_source.map(|source| source.value),
                })
                .map(render_query_outcome)
                .unwrap_or_else(|error| CommandOutcome::failure(Some(error.to_string()))),
            AdrBaselineInput::CheckCommit { track_id } => self
                .query_service
                .execute(AdrBaselineQuery::CheckCommit { track_id: track_id.value })
                .map(render_query_outcome)
                .unwrap_or_else(|error| CommandOutcome::failure(Some(error.to_string()))),
        }
    }
}

fn render_query_outcome(output: AdrBaselineQueryOutput) -> CommandOutcome {
    match output {
        AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Passed) => {
            CommandOutcome::success(Some("ADR baseline check: passed".to_owned()))
        }
        AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Blocked { violations }) => {
            CommandOutcome::failure(Some(format!("ADR baseline check blocked: {violations:?}")))
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use usecase::adr_baseline::{
        AdrBaselineError, AdrBaselineOutput, AdrBaselineQueryError, AdrBaselineSourceError,
    };

    use super::*;

    struct RecordingCommandService {
        recorded: Mutex<Vec<AdrBaselineCommand>>,
    }

    impl AdrBaselineService for RecordingCommandService {
        fn execute(
            &self,
            command: AdrBaselineCommand,
        ) -> Result<AdrBaselineOutput, AdrBaselineError> {
            self.recorded.lock().unwrap().push(command);
            Err(AdrBaselineError::Source(AdrBaselineSourceError::Read(
                usecase::DiagnosticMessage::try_new("not used".to_owned()).unwrap(),
            )))
        }
    }

    struct NoopQueryService;

    impl AdrBaselineQueryService for NoopQueryService {
        fn execute(
            &self,
            _: AdrBaselineQuery,
        ) -> Result<AdrBaselineQueryOutput, AdrBaselineQueryError> {
            Err(AdrBaselineQueryError::SourceRead(
                usecase::DiagnosticMessage::try_new("not used".to_owned()).unwrap(),
            ))
        }
    }

    struct RecordingQueryService {
        recorded: Mutex<Vec<AdrBaselineQuery>>,
    }

    impl AdrBaselineQueryService for RecordingQueryService {
        fn execute(
            &self,
            query: AdrBaselineQuery,
        ) -> Result<AdrBaselineQueryOutput, AdrBaselineQueryError> {
            self.recorded.lock().unwrap().push(query);
            Ok(AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Passed))
        }
    }

    struct PassingQueryService;

    impl AdrBaselineQueryService for PassingQueryService {
        fn execute(
            &self,
            _: AdrBaselineQuery,
        ) -> Result<AdrBaselineQueryOutput, AdrBaselineQueryError> {
            Ok(AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Passed))
        }
    }

    fn fixed_timestamp_provider() -> Result<Timestamp, AdrBaselineTimestampError> {
        Ok(Timestamp::new("2026-07-16T00:00:00Z").unwrap())
    }

    fn invalid_timestamp_provider() -> Result<Timestamp, AdrBaselineTimestampError> {
        Err(AdrBaselineTimestampError::InvalidTimestamp(
            Timestamp::new("not-a-timestamp").unwrap_err(),
        ))
    }

    #[test]
    fn test_adr_baseline_inputs_reject_invalid_track_and_source_values() {
        assert!("Invalid Track".parse::<TrackIdInput>().is_err());
        assert!("../decision.md".parse::<AdrSourceFileNameInput>().is_err());
        assert!("   ".parse::<AdrBaselineReasonInput>().is_err());
        assert!("unsupported".parse::<AdrBaselineKindInput>().is_err());
    }

    #[test]
    fn test_adr_baseline_driver_snapshot_maps_typed_input_losslessly() {
        let service = Arc::new(RecordingCommandService { recorded: Mutex::new(Vec::new()) });
        let driver = AdrBaselineDriver::new(
            service.clone(),
            Arc::new(NoopQueryService),
            fixed_timestamp_provider,
        );
        let outcome = driver.handle(AdrBaselineInput::Snapshot {
            track_id: "test-track".parse().unwrap(),
            source: "decision.md".parse().unwrap(),
            kind: AdrBaselineSnapshotInput::NewAdr(
                NonEmptyString::try_new("approved by user".to_owned()).unwrap(),
            ),
        });

        assert_eq!(outcome.exit_code, 1);
        let recorded = service.recorded.lock().unwrap();
        assert!(matches!(
            recorded.as_slice(),
            [AdrBaselineCommand::Snapshot { track_id, source, kind: AdrBaselineSnapshotKind::NewAdr(reason), timestamp }]
                if track_id.as_ref() == "test-track"
                    && source.as_str() == "decision.md"
                    && reason.as_ref() == "approved by user"
                    && timestamp.as_str() == "2026-07-16T00:00:00Z"
        ));
    }

    #[test]
    fn test_adr_baseline_driver_query_variants_delegate_losslessly_to_injected_service() {
        let command_service =
            Arc::new(RecordingCommandService { recorded: Mutex::new(Vec::new()) });
        let query_service = Arc::new(RecordingQueryService { recorded: Mutex::new(Vec::new()) });
        let driver = AdrBaselineDriver::new(
            command_service.clone(),
            query_service.clone(),
            fixed_timestamp_provider,
        );

        let review_outcome = driver.handle(AdrBaselineInput::CheckReview {
            track_id: "review-track".parse().unwrap(),
            primary_source: Some("primary.md".parse().unwrap()),
        });
        assert_eq!(review_outcome.stdout.as_deref(), Some("ADR baseline check: passed"));
        assert_eq!(review_outcome.stderr, None);
        assert_eq!(review_outcome.exit_code, 0);

        let commit_outcome = driver
            .handle(AdrBaselineInput::CheckCommit { track_id: "commit-track".parse().unwrap() });
        assert_eq!(commit_outcome.stdout.as_deref(), Some("ADR baseline check: passed"));
        assert_eq!(commit_outcome.stderr, None);
        assert_eq!(commit_outcome.exit_code, 0);

        assert!(command_service.recorded.lock().unwrap().is_empty());
        assert!(matches!(
            query_service.recorded.lock().unwrap().as_slice(),
            [
                AdrBaselineQuery::CheckReview { track_id: review_track, primary_source: Some(primary_source) },
                AdrBaselineQuery::CheckCommit { track_id: commit_track },
            ] if review_track.as_ref() == "review-track"
                && primary_source.as_str() == "primary.md"
                && commit_track.as_ref() == "commit-track"
        ));
    }

    #[test]
    fn test_adr_baseline_snapshot_input_rejects_illegal_kind_reason_combination() {
        let result = AdrBaselineSnapshotInput::try_from((
            AdrBaselineKindInput::Init,
            Some("unexpected".parse().unwrap()),
        ));

        assert_eq!(result, Err(AdrBaselineValidationError::InvalidReason));
    }

    #[test]
    fn test_adr_baseline_driver_renders_clock_error_without_calling_service() {
        let service = Arc::new(RecordingCommandService { recorded: Mutex::new(Vec::new()) });
        let driver = AdrBaselineDriver::new(
            service.clone(),
            Arc::new(NoopQueryService),
            invalid_timestamp_provider,
        );
        let outcome = driver.handle(AdrBaselineInput::Snapshot {
            track_id: "test-track".parse().unwrap(),
            source: "decision.md".parse().unwrap(),
            kind: AdrBaselineSnapshotInput::Init,
        });

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stdout, None);
        assert_eq!(outcome.stderr.as_deref(), Some("invalid timestamp: not-a-timestamp"));
        assert!(service.recorded.lock().unwrap().is_empty());
    }

    #[test]
    fn test_adr_baseline_driver_routes_only_to_injected_services() {
        let source = include_str!("adr_baseline.rs");
        let driver_source = source
            .split("pub struct AdrBaselineDriver")
            .nth(1)
            .unwrap()
            .split("fn render_query_outcome")
            .next()
            .unwrap();

        assert!(driver_source.contains("Arc<dyn AdrBaselineService>"));
        assert!(driver_source.contains("Arc<dyn AdrBaselineQueryService>"));
        assert!(
            driver_source.contains(
                "timestamp_provider: fn() -> Result<Timestamp, AdrBaselineTimestampError>"
            )
        );
        assert!(driver_source.contains("self.command_service"));
        assert!(driver_source.contains(".query_service"));
        for forbidden_runtime_path in [
            "AdrBaselineCompositionRoot",
            "adr_baseline_driver(",
            "FsAdrBaseline",
            "infrastructure::",
            "AdrBaselineClockReading",
            "ServiceImpl",
            "CompatibilityShim",
            "CompatService",
            "std::fs::",
            "std::process::",
            "std::net::",
            "std::io::",
            "println!",
            "eprintln!",
            "print!",
            "eprint!",
        ] {
            assert!(
                !driver_source.contains(forbidden_runtime_path),
                "primary adapter must not execute or reverse-delegate through {forbidden_runtime_path}"
            );
        }
    }

    #[test]
    fn test_adr_baseline_driver_new_handle_preserves_rendering_and_injected_clock() {
        let command_service =
            Arc::new(RecordingCommandService { recorded: Mutex::new(Vec::new()) });
        let driver = AdrBaselineDriver::new(
            command_service.clone(),
            Arc::new(PassingQueryService),
            fixed_timestamp_provider,
        );

        let snapshot_outcome = driver.handle(AdrBaselineInput::Snapshot {
            track_id: "test-track".parse().unwrap(),
            source: "decision.md".parse().unwrap(),
            kind: AdrBaselineSnapshotInput::Init,
        });
        assert_eq!(snapshot_outcome.stdout, None);
        assert_eq!(
            snapshot_outcome.stderr.as_deref(),
            Some("ADR baseline source read failed: DiagnosticMessage(\"not used\")")
        );
        assert_eq!(snapshot_outcome.exit_code, 1);
        assert!(matches!(
            command_service.recorded.lock().unwrap().as_slice(),
            [AdrBaselineCommand::Snapshot { timestamp, .. }]
                if timestamp.as_str() == "2026-07-16T00:00:00Z"
        ));

        let check_outcome = driver
            .handle(AdrBaselineInput::CheckCommit { track_id: "test-track".parse().unwrap() });
        assert_eq!(check_outcome.stdout.as_deref(), Some("ADR baseline check: passed"));
        assert_eq!(check_outcome.stderr, None);
        assert_eq!(check_outcome.exit_code, 0);
    }

    #[test]
    fn test_adr_baseline_snapshot_input_is_passive_reason_aware_dto() {
        let source = include_str!("adr_baseline.rs");
        let dto_source = source
            .split("pub enum AdrBaselineSnapshotInput")
            .nth(1)
            .unwrap()
            .split("/// Resolved ADR baseline operations")
            .next()
            .unwrap();

        for required_variant in [
            "Init,",
            "Cite,",
            "NewAdr(NonEmptyString)",
            "NonSemanticFix,",
            "Escalation(NonEmptyString)",
        ] {
            assert!(dto_source.contains(required_variant));
        }
        for forbidden_runtime_path in [
            "AdrBaselineDriver",
            "AdrBaselineService",
            "AdrBaselineQueryService",
            "AdrBaselineCompositionRoot",
            "ServiceImpl",
            "CompatibilityShim",
            "CompatService",
            "std::fs::",
            "std::process::",
            "std::net::",
            "std::io::",
        ] {
            assert!(
                !dto_source.contains(forbidden_runtime_path),
                "snapshot DTO must not retain or call runtime path {forbidden_runtime_path}"
            );
        }
    }
}
