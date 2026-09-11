//! Provider-facing calibration execution for the evaluate interactor.

use std::future::Future;
use std::pin::Pin;

use domain::SpecElementId;
use domain::tddd::semantic_verify::{
    CatalogueEntryKey, ModelTier, SpecElementRef, SpecSectionKind,
};
use domain::tddd::test_obligation::errors::{ObligationEvaluateError, SemanticVerifierError};
use domain::tddd::test_obligation::hashes::{
    BoundTestsSetHash, DeclarationHash, ObligationResponsibilityHash, SpecElementHash,
};
use domain::tddd::test_obligation::ids::{
    TestObligationBrief, TestObligationId, TestObligationItemIdentifier,
};
use domain::tddd::test_obligation::pair::{
    EntryDeclaration, ObligationFulfillmentPair, TestsSource,
};
use domain::tddd::test_obligation::verdict::{
    DetectionRatePercent, ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict,
};
use domain::tddd::test_obligation::vocab::{FulfillmentFailCategory, TestObligationKind};

use super::calibration::{
    CategoryTally, LocalResponsibilityExpectation, calibration_probe_count,
    local_responsibility_probe_shapes, probe_shape_for,
};
use super::concurrency::drive_bounded_in_order;
use super::verify::map_verifier_error;
use super::{EvaluateTestObligationsInteractor, diag, invalid_input_error};

impl EvaluateTestObligationsInteractor {
    pub(super) async fn known_bad_detection_rate(
        &self,
        production_pair_count: usize,
    ) -> Result<DetectionRatePercent, ObligationEvaluateError> {
        let probe_count =
            calibration_probe_count(production_pair_count, self.config.injection_rate());
        if probe_count == 0 {
            return DetectionRatePercent::try_new(100)
                .map_err(|_| invalid_input_error("known_bad_detection_rate"));
        }

        // Plan every calibration probe up front so their verdict futures can
        // be fanned out through the same bounded multiplexer the production
        // pairs use. Categories are tallied here rather than after the fan-out
        // so the per-category gate remains deterministic w.r.t. probe index.
        let mut category_tally = CategoryTally::default();
        let mut probe_categories: Vec<FulfillmentFailCategory> = Vec::with_capacity(probe_count);
        let mut probe_futures = Vec::with_capacity(probe_count);
        for index in 0..probe_count {
            let shape = probe_shape_for(index);
            category_tally.record_issued(&shape.category);
            probe_categories.push(shape.category.clone());
            probe_futures.push(self.calibration_probe_future(shape));
        }

        // Fan out the probe verdicts under the configured concurrency ceiling;
        // results come back in `probe_index` order.
        let verdicts = drive_bounded_in_order(probe_futures, self.config.parallelism()).await?;

        let mut detected = 0usize;
        for (verdict, expected_category) in verdicts.into_iter().zip(probe_categories.into_iter()) {
            if let ObligationFulfillmentVerdict::Fail { category, .. } = verdict
                && category == expected_category
            {
                detected += 1;
                category_tally.record_detected(&expected_category);
            }
        }

        // Per-category gate (AC-08): any exercised category that ends with
        // zero detected probes fails the calibration through the same
        // `VerifierPort` path as the threshold breach, but with a message
        // that names the missed category.
        let undetected = category_tally.undetected_categories();
        if !undetected.is_empty() {
            return Err(ObligationEvaluateError::VerifierPort(
                SemanticVerifierError::VerifierPort(diag(&format!(
                    "known-bad calibration detected 0 probes for categories: {}",
                    undetected.join(", ")
                ))),
            ));
        }

        let rate = ((detected * 100) / probe_count) as u8;
        let detection_rate = DetectionRatePercent::try_new(rate)
            .map_err(|_| invalid_input_error("known_bad_detection_rate"))?;
        if detection_rate.value() < self.config.detection_threshold().get() {
            return Err(ObligationEvaluateError::VerifierPort(
                SemanticVerifierError::VerifierPort(diag(&format!(
                    "known-bad detection rate {} below threshold {}",
                    detection_rate.value(),
                    self.config.detection_threshold().get()
                ))),
            ));
        }
        Ok(detection_rate)
    }

    /// Runs the D6 positive/negative locality examples through the same
    /// configured fulfillment provider used for production pairs.
    ///
    /// This deliberately remains separate from the known-bad category rate:
    /// the latter measures contradiction/substitution/central-unverified
    /// detection, while these probes check that a target-owned memory or
    /// persistence responsibility is accepted only on its own evidence. The
    /// result is enforced here; the CLI reports it under the configured
    /// provider-calibration lane rather than mixing it with production
    /// verdict counts.
    pub(super) async fn local_responsibility_calibration(
        &self,
        production_pair_count: usize,
    ) -> Result<(), ObligationEvaluateError> {
        // Preserve the existing calibration opt-out and the no-production-pair
        // fast path: disabling known-bad probes must not silently add provider
        // calls through the locality lane.
        if production_pair_count == 0 || self.config.injection_rate() == 0 {
            return Ok(());
        }

        let shapes = local_responsibility_probe_shapes();
        let mut expectations = Vec::with_capacity(shapes.len());
        let mut futures = Vec::with_capacity(shapes.len());
        for shape in shapes {
            expectations.push(shape.expectation);
            futures.push(self.local_responsibility_probe_future(shape));
        }

        let verdicts = drive_bounded_in_order(futures, self.config.parallelism()).await?;
        for (index, (verdict, expectation)) in
            verdicts.into_iter().zip(expectations.into_iter()).enumerate()
        {
            let accepted = match expectation {
                LocalResponsibilityExpectation::Fulfilled => {
                    matches!(verdict, ObligationFulfillmentVerdict::Fulfilled { .. })
                }
                LocalResponsibilityExpectation::Rejected => {
                    matches!(verdict, ObligationFulfillmentVerdict::Fail { .. })
                }
            };
            if !accepted {
                return Err(ObligationEvaluateError::VerifierPort(
                    SemanticVerifierError::VerifierPort(diag(&format!(
                        "local responsibility calibration probe {index} returned an unexpected verdict: {verdict:?}"
                    ))),
                ));
            }
        }
        Ok(())
    }

    /// Builds one calibration-probe verdict future.
    ///
    /// Split out so the concurrency helper can fan the probes out under the
    /// same bounded ceiling as production pairs; the future's success value
    /// is a fulfillment verdict for the probe's category.
    fn calibration_probe_future<'a>(
        &'a self,
        shape: super::calibration::CalibrationProbeShape,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ObligationFulfillmentVerdict, ObligationEvaluateError>>
                + Send
                + 'a,
        >,
    > {
        let super::calibration::CalibrationProbeShape {
            tests_source,
            declaration,
            anchor_text,
            ..
        } = shape;
        Box::pin(async move {
            let calibration_entry_key = CatalogueEntryKey::try_new("calibration".to_owned())
                .map_err(|_| invalid_input_error("calibration_entry_key"))?;
            let calibration_item =
                TestObligationItemIdentifier::try_new("known_bad_calibration".to_owned())
                    .map_err(|_| invalid_input_error("calibration_item_identifier"))?;
            let calibration_brief =
                TestObligationBrief::try_new("exercise the known-bad calibration probe".to_owned())
                    .map_err(|_| invalid_input_error("calibration_obligation_brief"))?;
            let obligation_id = TestObligationId::new(
                calibration_entry_key,
                TestObligationKind::Logic,
                calibration_item,
            );
            let spec_element = SpecElementRef::new(
                SpecSectionKind::InScope,
                SpecElementId::try_new("IN-01".to_owned())
                    .map_err(|_| invalid_input_error("calibration_spec_element_id"))?,
                anchor_text.to_owned(),
            );
            let key = ObligationFulfillmentCacheKey::new(
                BoundTestsSetHash::new(self.hasher.sha256(tests_source.as_bytes())),
                DeclarationHash::new(self.hasher.sha256(declaration.as_bytes())),
                SpecElementHash::new(
                    self.hasher.sha256(spec_element_material(&spec_element).as_bytes()),
                ),
                ObligationResponsibilityHash::new(self.hasher.sha256(
                    responsibility_material(&obligation_id, &calibration_brief).as_bytes(),
                )),
            );
            let pair = ObligationFulfillmentPair::new(
                TestsSource::try_new(tests_source)
                    .map_err(|_| invalid_input_error("tests_source"))?,
                EntryDeclaration::try_new(declaration.to_owned())
                    .map_err(|_| invalid_input_error("entry_declaration"))?,
                spec_element,
                obligation_id,
                calibration_brief,
            );
            self.fulfillment_driver
                .evaluate_with_escalation(&pair, &key, ModelTier::Fast)
                .await
                .map_err(map_verifier_error)
        })
    }

    /// Builds one generic memory/persistence locality probe future.
    fn local_responsibility_probe_future<'a>(
        &'a self,
        shape: super::calibration::LocalResponsibilityProbeShape,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ObligationFulfillmentVerdict, ObligationEvaluateError>>
                + Send
                + 'a,
        >,
    > {
        let super::calibration::LocalResponsibilityProbeShape {
            tests_source,
            entry_key,
            item_identifier,
            obligation_brief,
            entry_declaration,
            anchor_text,
            ..
        } = shape;
        Box::pin(async move {
            let entry_key = CatalogueEntryKey::try_new(entry_key.to_owned())
                .map_err(|_| invalid_input_error("local_calibration_entry_key"))?;
            let item = TestObligationItemIdentifier::try_new(item_identifier.to_owned())
                .map_err(|_| invalid_input_error("local_calibration_item_identifier"))?;
            let brief = TestObligationBrief::try_new(obligation_brief.to_owned())
                .map_err(|_| invalid_input_error("local_calibration_obligation_brief"))?;
            let obligation_id =
                TestObligationId::new(entry_key, TestObligationKind::Contract, item);
            let spec_element = SpecElementRef::new(
                SpecSectionKind::InScope,
                SpecElementId::try_new("IN-01".to_owned())
                    .map_err(|_| invalid_input_error("local_calibration_spec_element_id"))?,
                anchor_text.to_owned(),
            );
            let key = ObligationFulfillmentCacheKey::new(
                BoundTestsSetHash::new(self.hasher.sha256(tests_source.as_bytes())),
                DeclarationHash::new(self.hasher.sha256(entry_declaration.as_bytes())),
                SpecElementHash::new(
                    self.hasher.sha256(spec_element_material(&spec_element).as_bytes()),
                ),
                ObligationResponsibilityHash::new(
                    self.hasher.sha256(responsibility_material(&obligation_id, &brief).as_bytes()),
                ),
            );
            let pair = ObligationFulfillmentPair::new(
                TestsSource::try_new(tests_source)
                    .map_err(|_| invalid_input_error("tests_source"))?,
                EntryDeclaration::try_new(entry_declaration.to_owned())
                    .map_err(|_| invalid_input_error("entry_declaration"))?,
                spec_element,
                obligation_id,
                brief,
            );
            self.fulfillment_driver
                .evaluate_with_escalation(&pair, &key, ModelTier::Fast)
                .await
                .map_err(map_verifier_error)
        })
    }
}

/// Canonical material for the structured specification element carried by a
/// calibration pair. The section, identifier, and text are all part of the
/// cache identity so a change in any semantic input cannot reuse a verdict.
fn spec_element_material(spec_element: &SpecElementRef) -> String {
    format!(
        "section={:?}\nelement_id={}\ntext_label={}",
        spec_element.section,
        spec_element.element_id.as_ref(),
        spec_element.text_label,
    )
}

/// Canonical material for the entry-local obligation responsibility carried by
/// a calibration pair.
fn responsibility_material(
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
