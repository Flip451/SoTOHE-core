//! `track contract-map` primary adapter.
//!
//! [`ContractMapDriver`] invokes the injected usecase and renders its result
//! into a user-facing [`CommandOutcome`].

use std::fmt::Debug;
use std::sync::Arc;

use usecase::contract_map_workflow::{
    RenderContractMap, RenderContractMapCommand, RenderContractMapOutput,
};

use crate::render::CommandOutcome;

/// Typed input for the `track contract-map` command.
pub struct ContractMapInput {
    /// Validated command passed to the contract-map usecase.
    pub command: RenderContractMapCommand,
}

/// Primary adapter for the `track contract-map` command.
pub struct ContractMapDriver {
    service: Arc<dyn RenderContractMap>,
}

impl ContractMapDriver {
    /// Creates a driver that invokes the supplied contract-map usecase.
    #[must_use]
    pub fn new(service: Arc<dyn RenderContractMap>) -> ContractMapDriver {
        Self { service }
    }

    /// Invokes the usecase and renders its result for the CLI.
    pub fn handle(&self, input: ContractMapInput) -> CommandOutcome {
        match self.service.execute(&input.command) {
            Ok(output) => contract_map_output_to_outcome(input.command.track_id.as_ref(), &output),
            Err(error) => CommandOutcome::failure(Some(error.to_string())),
        }
    }
}

fn contract_map_output_to_outcome(
    track_id: &str,
    output: &RenderContractMapOutput,
) -> CommandOutcome {
    CommandOutcome::success(Some(contract_map_success_message(
        track_id,
        output.rendered_layer_count,
        output.total_entry_count,
        &output.warnings,
    )))
}

fn contract_map_success_message<W: Debug>(
    track_id: &str,
    rendered_layer_count: usize,
    total_entry_count: usize,
    warnings: &[W],
) -> String {
    let warnings =
        if warnings.is_empty() { String::new() } else { format!(", warnings={warnings:?}") };
    format!(
        "[OK] contract-map: wrote track/items/{track_id}/contract-map.md \
         (layers={rendered_layer_count}, entries={total_entry_count}{warnings})"
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use usecase::TrackId;

    #[derive(Debug)]
    enum TestRole {
        ValueObject,
    }

    #[derive(Debug)]
    enum TestWarning {
        UndefinedRoleStyle { role: TestRole },
    }

    struct FailingContractMapService;

    impl RenderContractMap for FailingContractMapService {
        fn execute(
            &self,
            command: &RenderContractMapCommand,
        ) -> Result<RenderContractMapOutput, usecase::contract_map_workflow::RenderContractMapError>
        {
            Err(usecase::contract_map_workflow::RenderContractMapError::EmptyCatalogue {
                track_id: command.track_id.clone(),
            })
        }
    }

    #[test]
    fn test_contract_map_success_message_with_warnings_surfaces_warnings() {
        let warning = TestWarning::UndefinedRoleStyle { role: TestRole::ValueObject };
        assert!(matches!(
            &warning,
            TestWarning::UndefinedRoleStyle { role: TestRole::ValueObject }
        ));

        let message = contract_map_success_message("test-track", 1, 2, &[warning]);

        assert_eq!(
            message,
            "[OK] contract-map: wrote track/items/test-track/contract-map.md \
             (layers=1, entries=2, warnings=[UndefinedRoleStyle { role: ValueObject }])"
        );
    }

    #[test]
    fn test_contract_map_success_message_without_warnings_preserves_success_format() {
        let message = contract_map_success_message::<TestWarning>("test-track", 1, 2, &[]);

        assert_eq!(
            message,
            "[OK] contract-map: wrote track/items/test-track/contract-map.md (layers=1, entries=2)"
        );
    }

    #[test]
    fn test_contract_map_driver_usecase_error_renders_failure_outcome() {
        let driver = ContractMapDriver::new(Arc::new(FailingContractMapService));
        let input = ContractMapInput {
            command: RenderContractMapCommand {
                track_id: TrackId::try_new("test-track").unwrap(),
                layer_filter: None,
            },
        };

        let outcome = driver.handle(input);

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stdout, None);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some(
                "catalogue loader returned no enabled layers for track 'test-track'; \
                 check `architecture-rules.json` tddd blocks"
            )
        );
    }
}
