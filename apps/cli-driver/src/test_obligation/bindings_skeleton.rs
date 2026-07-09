//! Primary adapter for `sotp test-obligation bindings-skeleton`.

use std::sync::Arc;

use serde_json::json;
use usecase::TrackId;
use usecase::test_obligation::bindings_skeleton::{
    TestBindingsSkeletonApplicationService, TestBindingsSkeletonCommand, TestBindingsSkeletonError,
    TestBindingsSkeletonOutcome,
};

use crate::render::CommandOutcome;

use super::resolve_track_id;

/// cli_driver-local DTO for `sotp test-obligation bindings-skeleton`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestBindingsSkeletonInput {
    track_id: Option<TrackId>,
}

impl TestBindingsSkeletonInput {
    /// Constructor for [`TestBindingsSkeletonInput`].
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

    /// Return the resolved track id.
    #[must_use]
    pub fn track_id(&self) -> Option<&TrackId> {
        self.track_id.as_ref()
    }
}

/// Primary adapter for `test-obligation bindings-skeleton`.
pub struct TestBindingsSkeletonHandler {
    pub service: Arc<dyn TestBindingsSkeletonApplicationService>,
}

impl TestBindingsSkeletonHandler {
    /// Builds the handler over its application service.
    #[must_use]
    pub fn new(service: Arc<dyn TestBindingsSkeletonApplicationService>) -> Self {
        Self { service }
    }

    /// Handles one skeleton command.
    #[must_use]
    pub fn handle(&self, input: TestBindingsSkeletonInput) -> CommandOutcome {
        let Some(track_id) = input.track_id().cloned() else {
            return CommandOutcome::failure(Some("--track-id is required".to_owned()));
        };
        let command = TestBindingsSkeletonCommand::new(track_id);
        match self.service.execute(&command) {
            Ok(outcome) => CommandOutcome {
                stdout: Some(render_skeleton(&outcome)),
                stderr: Some(render_note(&outcome)),
                exit_code: 0,
            },
            Err(error) => CommandOutcome::failure(Some(render_error(error))),
        }
    }
}

/// Renders the draft in the exact `test-bindings.json` wire shape, with TODO
/// placeholder test locations. Stdout stays schema-conformant so an author can
/// materialize it as the artifact and only replace values in place; the
/// fail-closed codec rejects it until every placeholder is replaced.
fn render_skeleton(outcome: &TestBindingsSkeletonOutcome) -> String {
    let records: Vec<serde_json::Value> = outcome
        .records()
        .iter()
        .map(|record| {
            json!({
                "kind": "fulfillment",
                "obligation_id": {
                    "entry_key": record.entry_key(),
                    "obligation_kind": record.obligation_kind(),
                    "item_identifier": record.item_identifier()
                },
                "tests": [
                    {
                        "layer": "TODO_LAYER",
                        "module_path": "TODO::module::tests",
                        "test_name": "TODO_test_name"
                    }
                ]
            })
        })
        .collect();
    let document = json!({
        "track_id": outcome.track_id().as_ref(),
        "records": records
    });
    match serde_json::to_string_pretty(&document) {
        Ok(text) => text,
        Err(error) => json!({
            "error": format!("failed to render bindings skeleton: {error}")
        })
        .to_string(),
    }
}

/// Renders the stderr authoring note accompanying the stdout draft.
fn render_note(outcome: &TestBindingsSkeletonOutcome) -> String {
    format!(
        "bindings-skeleton: drafted {} fulfillment record(s) for track '{}'. \
         Replace the TODO test locations (and convert records to `waiver` / \
         `voluntary_binding` where appropriate), then save as \
         track/items/{}/test-bindings.json. Briefs and target entries live in \
         obligations.json.",
        outcome.records().len(),
        outcome.track_id().as_ref(),
        outcome.track_id().as_ref(),
    )
}

fn render_error(error: TestBindingsSkeletonError) -> String {
    match error {
        TestBindingsSkeletonError::ObligationsAbsent => {
            "test-obligation bindings-skeleton failed: obligations.json is absent; run `bin/sotp test-obligation derive` first".to_owned()
        }
        TestBindingsSkeletonError::ArtifactLoad(error) => {
            format!("test-obligation bindings-skeleton failed: {error}")
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    struct StubService;

    impl TestBindingsSkeletonApplicationService for StubService {
        fn execute(
            &self,
            cmd: &TestBindingsSkeletonCommand,
        ) -> Result<TestBindingsSkeletonOutcome, TestBindingsSkeletonError> {
            Ok(TestBindingsSkeletonOutcome::new(
                cmd.track_id().clone(),
                vec![usecase::test_obligation::bindings_skeleton::TestBindingsSkeletonRecord::new(
                    "domain::User".to_owned(),
                    "boundary".to_owned(),
                    "invariant:non_empty".to_owned(),
                )],
            ))
        }
    }

    #[test]
    fn test_skeleton_handler_renders_schema_shaped_draft() {
        let handler = TestBindingsSkeletonHandler::new(Arc::new(StubService));
        let input = TestBindingsSkeletonInput::try_from_raw(
            Some("my-track".to_owned()),
            "detached".to_owned(),
        )
        .unwrap();

        let outcome = handler.handle(input);

        assert_eq!(outcome.exit_code, 0);
        let value: serde_json::Value = serde_json::from_str(&outcome.stdout.unwrap()).unwrap();
        assert_eq!(value["track_id"], "my-track");
        let record = &value["records"][0];
        assert_eq!(record["kind"], "fulfillment");
        assert_eq!(record["obligation_id"]["entry_key"], "domain::User");
        assert_eq!(record["obligation_id"]["obligation_kind"], "boundary");
        assert_eq!(record["obligation_id"]["item_identifier"], "invariant:non_empty");
        assert_eq!(record["tests"][0]["layer"], "TODO_LAYER");
        // Wire-shape guard: the draft carries no annotation fields, only the
        // exact keys the fail-closed test-bindings codec accepts.
        let mut document_keys: Vec<_> = value.as_object().unwrap().keys().collect();
        document_keys.sort();
        assert_eq!(document_keys, ["records", "track_id"]);
        let mut record_keys: Vec<_> = record.as_object().unwrap().keys().collect();
        record_keys.sort();
        assert_eq!(record_keys, ["kind", "obligation_id", "tests"]);
        let note = outcome.stderr.unwrap();
        assert!(note.contains("drafted 1 fulfillment record(s)"));
        assert!(note.contains("test-bindings.json"));
    }
}
