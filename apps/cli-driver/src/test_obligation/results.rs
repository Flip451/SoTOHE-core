//! Primary adapter for `sotp test-obligation results`.

use std::sync::Arc;

use usecase::TrackId;
use usecase::test_obligation::results::{
    TestObligationResultsApplicationService, TestObligationResultsCommand,
};

use crate::render::CommandOutcome;

use super::resolve_track_id;

/// cli_driver-local DTO for `sotp test-obligation results`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationResultsInput {
    track_id: Option<TrackId>,
}

impl TestObligationResultsInput {
    /// Constructor for [`TestObligationResultsInput`].
    #[must_use]
    pub fn new(track_id: Option<TrackId>) -> Self {
        Self { track_id }
    }

    /// Builds the input from CLI string values.
    ///
    /// # Errors
    ///
    /// Returns an error when the optional track id or branch diagnostic is invalid.
    #[cfg(not(doc))]
    pub fn try_from_raw(track_id: Option<String>, current_branch: String) -> Result<Self, String> {
        let (track_id, current_branch) = super::parse_input_parts(track_id, current_branch)?;
        let track_id = resolve_track_id(track_id.as_ref(), &current_branch)?;
        Ok(Self::new(Some(track_id)))
    }

    /// Return the optional track id.
    #[must_use]
    pub fn track_id(&self) -> Option<&TrackId> {
        self.track_id.as_ref()
    }
}

/// Primary adapter for `test-obligation results`.
pub struct TestObligationResultsHandler {
    pub service: Arc<dyn TestObligationResultsApplicationService>,
}

impl TestObligationResultsHandler {
    /// Builds the handler over its application service.
    #[must_use]
    pub fn new(service: Arc<dyn TestObligationResultsApplicationService>) -> Self {
        Self { service }
    }

    /// Handles one results command.
    #[must_use]
    pub fn handle(&self, input: TestObligationResultsInput) -> CommandOutcome {
        let Some(track_id) = input.track_id().cloned() else {
            return CommandOutcome::failure(Some("--track-id is required".to_owned()));
        };
        let command = TestObligationResultsCommand::new(track_id);
        match self.service.execute(&command) {
            Ok(output) => {
                let mut text = String::new();
                text.push_str("test-obligation results\n");
                for lane in output.lane_summaries() {
                    text.push_str(&format!(
                        "{:?}:{} pass={} fail={} pending={}\n",
                        lane.chain_name(),
                        lane.layer().as_ref(),
                        lane.pass_count(),
                        lane.fail_count(),
                        lane.pending_count()
                    ));
                }
                for record in output.records() {
                    text.push_str(&format!("record={record:?}\n"));
                }
                text.push_str(&format!(
                    "records={} uncited_findings={}",
                    output.records().len(),
                    output.uncited_findings().len()
                ));
                CommandOutcome::success(Some(text))
            }
            Err(error) => {
                CommandOutcome::failure(Some(format!("test-obligation results failed: {error:?}")))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use usecase::DiagnosticMessage;
    use usecase::LayerId;
    use usecase::test_obligation::results::{
        CatalogueEntryKey, EdgeResolutionOutcome, EdgeVerdictRecord, FulfillmentFailCategory,
        TestObligationAnchorId, TestObligationDrift, TestObligationEdgeId, TestObligationId,
        TestObligationItemIdentifier, TestObligationKind, TestObligationResultsOutput,
    };

    use super::*;

    struct StubService;

    impl TestObligationResultsApplicationService for StubService {
        fn execute(
            &self,
            _cmd: &TestObligationResultsCommand,
        ) -> Result<
            TestObligationResultsOutput,
            usecase::test_obligation::errors::ObligationResultsError,
        > {
            Ok(TestObligationResultsOutput::new(Vec::new(), Vec::new(), Vec::new()))
        }
    }

    #[test]
    fn test_results_handler_with_valid_input_returns_success() {
        let handler = TestObligationResultsHandler::new(Arc::new(StubService));
        let input =
            TestObligationResultsInput::try_from_raw(None, "track/test-track".to_owned()).unwrap();

        let outcome = handler.handle(input);

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.unwrap().contains("records=0"));
    }

    #[test]
    fn test_results_handler_renders_per_lane_counts() {
        struct DetailedStubService;

        impl TestObligationResultsApplicationService for DetailedStubService {
            fn execute(
                &self,
                _cmd: &TestObligationResultsCommand,
            ) -> Result<
                TestObligationResultsOutput,
                usecase::test_obligation::errors::ObligationResultsError,
            > {
                Ok(TestObligationResultsOutput::new(
                    vec![usecase::test_obligation::results::TestObligationLaneSummary::new(
                        usecase::test_obligation::results::TestObligationChainLabel::Fulfillment,
                        LayerId::try_new("infrastructure".to_owned()).unwrap(),
                        1,
                        1,
                        0,
                    )],
                    Vec::new(),
                    Vec::new(),
                ))
            }
        }

        let handler = TestObligationResultsHandler::new(Arc::new(DetailedStubService));
        let input = TestObligationResultsInput::new(Some(TrackId::try_new("test-track").unwrap()));

        let outcome = handler.handle(input);

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.unwrap();
        assert!(stdout.contains("Fulfillment:infrastructure pass=1 fail=1 pending=0"));
        assert!(stdout.ends_with("records=0 uncited_findings=0"));
    }

    #[test]
    fn test_results_handler_renders_failing_record_detail() {
        struct DetailedStubService;

        impl TestObligationResultsApplicationService for DetailedStubService {
            fn execute(
                &self,
                _cmd: &TestObligationResultsCommand,
            ) -> Result<
                TestObligationResultsOutput,
                usecase::test_obligation::errors::ObligationResultsError,
            > {
                let edge = TestObligationEdgeId::new(
                    CatalogueEntryKey::try_new("domain::Money".to_owned()).unwrap(),
                    TestObligationAnchorId::try_new("spec.json".to_owned(), "AC-09".to_owned())
                        .unwrap(),
                );
                let drift = TestObligationDrift::reason_changed_edge(
                    edge.clone(),
                    DiagnosticMessage::try_new("changed waiver reason".to_owned()).unwrap(),
                );
                Ok(TestObligationResultsOutput::new(
                    Vec::new(),
                    vec![EdgeVerdictRecord::new(
                        None,
                        edge,
                        None,
                        None,
                        EdgeResolutionOutcome::Fail(FulfillmentFailCategory::Contradiction),
                        None,
                        Some(drift),
                    )],
                    Vec::new(),
                ))
            }
        }

        let handler = TestObligationResultsHandler::new(Arc::new(DetailedStubService));
        let input = TestObligationResultsInput::new(Some(TrackId::try_new("test-track").unwrap()));

        let outcome = handler.handle(input);

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.unwrap();
        assert!(stdout.contains("record=EdgeVerdictRecord"));
        assert!(stdout.contains("domain::Money"));
        assert!(stdout.contains("AC-09"));
        assert!(stdout.contains("Fail(Contradiction)"));
        assert!(stdout.contains("ReasonChanged"));
        assert!(stdout.contains("changed waiver reason"));
    }

    #[test]
    fn test_results_handler_renders_record_claim_evidence_verdict_and_reason() {
        struct DetailedStubService;

        impl TestObligationResultsApplicationService for DetailedStubService {
            fn execute(
                &self,
                _cmd: &TestObligationResultsCommand,
            ) -> Result<
                TestObligationResultsOutput,
                usecase::test_obligation::errors::ObligationResultsError,
            > {
                let edge = TestObligationEdgeId::new(
                    CatalogueEntryKey::try_new("domain::Invoice".to_owned()).unwrap(),
                    TestObligationAnchorId::try_new(
                        "track/spec.json".to_owned(),
                        "AC-09".to_owned(),
                    )
                    .unwrap(),
                );
                let drift = TestObligationDrift::reason_changed_edge(
                    edge.clone(),
                    DiagnosticMessage::try_new("evidence no longer matches the claim".to_owned())
                        .unwrap(),
                );
                let obligation_id = TestObligationId::new(
                    CatalogueEntryKey::try_new("domain::Invoice".to_owned()).unwrap(),
                    TestObligationKind::Boundary,
                    TestObligationItemIdentifier::try_new("invariant:non_empty".to_owned())
                        .unwrap(),
                );
                Ok(TestObligationResultsOutput::new(
                    Vec::new(),
                    vec![EdgeVerdictRecord::new(
                        Some(obligation_id),
                        edge,
                        Some(
                            DiagnosticMessage::try_new("fulfillment binding".to_owned()).unwrap(),
                        ),
                        Some(
                            DiagnosticMessage::try_new(
                                "cli_driver::test_obligation::results::tests::test_results_handler_renders_record_claim_evidence_verdict_and_reason"
                                    .to_owned(),
                            )
                            .unwrap(),
                        ),
                        EdgeResolutionOutcome::Fail(FulfillmentFailCategory::CentralUnverified),
                        Some(
                            DiagnosticMessage::try_new(
                                "entry-relevant proof is missing".to_owned(),
                            )
                            .unwrap(),
                        ),
                        Some(drift),
                    )],
                    Vec::new(),
                ))
            }
        }

        let handler = TestObligationResultsHandler::new(Arc::new(DetailedStubService));
        let input = TestObligationResultsInput::new(Some(TrackId::try_new("test-track").unwrap()));

        let outcome = handler.handle(input);

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.unwrap();
        assert!(stdout.contains("entry_key: CatalogueEntryKey(\"domain::Invoice\")"));
        assert!(stdout.contains("invariant:non_empty"));
        assert!(stdout.contains("file_path: \"track/spec.json\""));
        assert!(stdout.contains("element_id: \"AC-09\""));
        assert!(stdout.contains("claim_source: Some(DiagnosticMessage(\"fulfillment binding\"))"));
        assert!(stdout.contains(
            "evidence_source: Some(DiagnosticMessage(\"cli_driver::test_obligation::results::tests::test_results_handler_renders_record_claim_evidence_verdict_and_reason\"))"
        ));
        assert!(stdout.contains("outcome: Fail(CentralUnverified)"));
        assert!(stdout.contains(
            "verdict_reason: Some(DiagnosticMessage(\"entry-relevant proof is missing\"))"
        ));
        assert!(
            stdout.contains("detail: DiagnosticMessage(\"evidence no longer matches the claim\")")
        );
    }
}
