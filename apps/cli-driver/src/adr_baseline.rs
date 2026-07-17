//! Primary adapter for ADR baseline snapshot, restore, and freeze-check commands.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use usecase::adr_baseline::{
    AdrBaselineCheckOutcome, AdrBaselineCommand, AdrBaselineKind, AdrBaselineQuery,
    AdrBaselineQueryOutput, AdrBaselineQueryService, AdrBaselineService,
    AdrBaselineValidationError, AdrSourceFileName,
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

impl AdrBaselineKindInput {
    fn into_domain(self) -> AdrBaselineKind {
        match self {
            Self::Init => AdrBaselineKind::Init,
            Self::Cite => AdrBaselineKind::Cite,
            Self::NewAdr => AdrBaselineKind::NewAdr,
            Self::NonSemanticFix => AdrBaselineKind::NonSemanticFix,
            Self::Escalation => AdrBaselineKind::Escalation,
        }
    }
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

/// Unresolved ADR baseline requests accepted by the composition boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineRequest {
    Snapshot {
        items_dir: PathBuf,
        track_id: Option<TrackIdInput>,
        source: AdrSourceFileNameInput,
        kind: AdrBaselineKindInput,
        reason: Option<AdrBaselineReasonInput>,
    },
    Restore {
        items_dir: PathBuf,
        track_id: Option<TrackIdInput>,
        source: AdrSourceFileNameInput,
    },
    CheckReview {
        items_dir: PathBuf,
        track_id: Option<TrackIdInput>,
        primary_source: Option<AdrSourceFileNameInput>,
    },
    CheckCommit {
        items_dir: PathBuf,
        track_id: Option<TrackIdInput>,
    },
}

impl AdrBaselineRequest {
    /// Returns the request's track-items directory without interpreting it.
    #[must_use]
    pub fn items_dir(&self) -> &std::path::Path {
        match self {
            Self::Snapshot { items_dir, .. }
            | Self::Restore { items_dir, .. }
            | Self::CheckReview { items_dir, .. }
            | Self::CheckCommit { items_dir, .. } => items_dir,
        }
    }
}

/// Resolved ADR baseline operations accepted by the primary adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineInput {
    Snapshot {
        track_id: TrackIdInput,
        source: AdrSourceFileNameInput,
        kind: AdrBaselineKindInput,
        reason: Option<AdrBaselineReasonInput>,
        timestamp: Timestamp,
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
}

impl AdrBaselineDriver {
    /// Creates an ADR baseline driver from the command and query services.
    #[must_use]
    pub fn new(
        command_service: Arc<dyn AdrBaselineService>,
        query_service: Arc<dyn AdrBaselineQueryService>,
    ) -> Self {
        Self { command_service, query_service }
    }

    /// Maps one validated CLI request to the matching use-case operation.
    #[must_use]
    pub fn handle(&self, input: AdrBaselineInput) -> CommandOutcome {
        match input {
            AdrBaselineInput::Snapshot { track_id, source, kind, reason, timestamp } => self
                .command_service
                .execute(AdrBaselineCommand::Snapshot {
                    track_id: track_id.value,
                    source: source.value,
                    kind: kind.into_domain(),
                    reason: reason.map(|reason| reason.value),
                    timestamp,
                })
                .map(|output| {
                    CommandOutcome::success(Some(format!("ADR baseline snapshot: {output:?}")))
                })
                .unwrap_or_else(|error| CommandOutcome::failure(Some(error.to_string()))),
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

    use usecase::adr_baseline::{AdrBaselineError, AdrBaselineOutput, AdrBaselineQueryError};

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
            Err(AdrBaselineError::Validation(AdrBaselineValidationError::InvalidReason))
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
        let driver = AdrBaselineDriver::new(service.clone(), Arc::new(NoopQueryService));
        let outcome = driver.handle(AdrBaselineInput::Snapshot {
            track_id: "test-track".parse().unwrap(),
            source: "decision.md".parse().unwrap(),
            kind: "new-adr".parse().unwrap(),
            reason: Some("approved by user".parse().unwrap()),
            timestamp: Timestamp::new("2026-07-16T00:00:00Z").unwrap(),
        });

        assert_eq!(outcome.exit_code, 1);
        let recorded = service.recorded.lock().unwrap();
        assert!(matches!(
            recorded.as_slice(),
            [AdrBaselineCommand::Snapshot { track_id, source, kind: AdrBaselineKind::NewAdr, reason: Some(reason), timestamp }]
                if track_id.as_ref() == "test-track"
                    && source.as_str() == "decision.md"
                    && reason.as_ref() == "approved by user"
                    && timestamp.as_str() == "2026-07-16T00:00:00Z"
        ));
    }

    #[test]
    fn test_adr_baseline_driver_defers_kind_reason_legality_to_service() {
        let service = Arc::new(RecordingCommandService { recorded: Mutex::new(Vec::new()) });
        let driver = AdrBaselineDriver::new(service.clone(), Arc::new(NoopQueryService));
        let outcome = driver.handle(AdrBaselineInput::Snapshot {
            track_id: "test-track".parse().unwrap(),
            source: "decision.md".parse().unwrap(),
            kind: AdrBaselineKindInput::Init,
            reason: Some("unexpected".parse().unwrap()),
            timestamp: Timestamp::new("2026-07-16T00:00:00Z").unwrap(),
        });

        assert_eq!(outcome.exit_code, 1);
        assert!(matches!(
            service.recorded.lock().unwrap().as_slice(),
            [AdrBaselineCommand::Snapshot { kind: AdrBaselineKind::Init, reason: Some(reason), .. }]
                if reason.as_ref() == "unexpected"
        ));
    }
}
