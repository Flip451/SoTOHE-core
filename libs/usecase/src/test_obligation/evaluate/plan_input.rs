//! Verifier-pair input construction and cache-key material for `evaluate::plan`.

use domain::tddd::test_obligation::errors::ObligationEvaluateError;
use domain::tddd::test_obligation::pair::{
    EntryDeclaration, ObligationFulfillmentPair, TestsSource, WaiverPair,
};

use super::super::invalid_input_error;
use super::{FulfillmentLlmTask, WaiverLlmTask};

/// Materialises the fulfillment pair value once so the LLM future's async
/// block only borrows it — reduces per-poll allocations in the multiplexer.
pub(super) fn build_fulfillment_pair_input(
    task: &FulfillmentLlmTask,
) -> Result<ObligationFulfillmentPair, ObligationEvaluateError> {
    let tests_source = TestsSource::try_new(task.tests_source.clone())
        .map_err(|_| invalid_input_error("tests_source"))?;
    let entry_declaration = EntryDeclaration::try_new(task.declaration.clone())
        .map_err(|_| invalid_input_error("entry_declaration"))?;
    Ok(ObligationFulfillmentPair::new(
        tests_source,
        entry_declaration,
        task.spec_element.clone(),
        task.obligation_id.clone(),
        task.obligation_brief.clone(),
    ))
}

/// Materialises the waiver pair value once so the LLM future's async block
/// only borrows it while the verifier driver is running.
pub(super) fn build_waiver_pair_input(
    task: &WaiverLlmTask,
) -> Result<WaiverPair, ObligationEvaluateError> {
    let entry_declaration = EntryDeclaration::try_new(task.declaration.clone())
        .map_err(|_| invalid_input_error("entry_declaration"))?;
    Ok(WaiverPair::new(
        task.reason.clone(),
        entry_declaration,
        task.spec_element.clone(),
        task.obligation_id.clone(),
        task.obligation_brief.clone(),
    ))
}

pub(super) use super::super::super::freshness::{responsibility_material, spec_element_material};
