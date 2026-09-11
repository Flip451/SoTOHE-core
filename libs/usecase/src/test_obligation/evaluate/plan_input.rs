//! Verifier-pair input construction and cache-key material for `evaluate::plan`.

use domain::tddd::semantic_verify::SpecElementRef;
use domain::tddd::test_obligation::errors::ObligationEvaluateError;
use domain::tddd::test_obligation::ids::{TestObligationBrief, TestObligationId};
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

/// Canonical migration-era material for a structured specification element.
/// The pair retains section membership for the verifier; the shared check and
/// results lanes currently freeze the identifier and text components here.
pub(super) fn spec_element_material(spec_element: &SpecElementRef) -> String {
    format!(
        "element_id={}\ntext_label={}",
        spec_element.element_id.as_ref(),
        spec_element.text_label,
    )
}

/// Canonical material for the entry-local obligation responsibility.
pub(super) fn responsibility_material(
    obligation_id: &TestObligationId,
    obligation_brief: &TestObligationBrief,
) -> String {
    format!(
        "entry_key={}\nobligation_kind={}\nitem_identifier={}\nobligation_brief={}",
        obligation_id.entry_key().as_str(),
        obligation_id.obligation_kind().as_kebab(),
        obligation_id.item_identifier().as_str(),
        obligation_brief.as_str(),
    )
}
