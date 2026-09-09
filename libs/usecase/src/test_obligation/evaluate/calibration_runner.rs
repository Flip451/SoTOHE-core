//! Provider-facing calibration execution for the evaluate interactor.

use std::future::Future;
use std::pin::Pin;

use domain::tddd::semantic_verify::{CatalogueEntryKey, ModelTier};
use domain::tddd::test_obligation::errors::{ObligationEvaluateError, SemanticVerifierError};
use domain::tddd::test_obligation::hashes::{AnchorTextHash, BoundTestsSetHash, DeclarationHash};
use domain::tddd::test_obligation::ids::{
    TestObligationBrief, TestObligationId, TestObligationItemIdentifier,
};
use domain::tddd::test_obligation::pair::{
    AnchorText, EntryDeclaration, ObligationFulfillmentPair, TestsSource,
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
    /// result is enforced here; T010 owns any additional reporting surface.
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
        let key = ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(self.hasher.sha256(shape.tests_source.as_bytes())),
            DeclarationHash::new(self.hasher.sha256(shape.declaration.as_bytes())),
            AnchorTextHash::new(self.hasher.sha256(shape.anchor_text.as_bytes())),
        );
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
            let pair = ObligationFulfillmentPair::new(
                TestsSource::try_new(tests_source)
                    .map_err(|_| invalid_input_error("tests_source"))?,
                EntryDeclaration::try_new(declaration.to_owned())
                    .map_err(|_| invalid_input_error("entry_declaration"))?,
                AnchorText::try_new(anchor_text.to_owned())
                    .map_err(|_| invalid_input_error("anchor_text"))?,
                TestObligationId::new(
                    calibration_entry_key,
                    TestObligationKind::Logic,
                    calibration_item,
                ),
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
        let key = ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(self.hasher.sha256(shape.tests_source.as_bytes())),
            DeclarationHash::new(self.hasher.sha256(shape.entry_declaration.as_bytes())),
            AnchorTextHash::new(self.hasher.sha256(shape.anchor_text.as_bytes())),
        );
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
            let pair = ObligationFulfillmentPair::new(
                TestsSource::try_new(tests_source)
                    .map_err(|_| invalid_input_error("tests_source"))?,
                EntryDeclaration::try_new(entry_declaration.to_owned())
                    .map_err(|_| invalid_input_error("entry_declaration"))?,
                AnchorText::try_new(anchor_text.to_owned())
                    .map_err(|_| invalid_input_error("anchor_text"))?,
                TestObligationId::new(entry_key, TestObligationKind::Contract, item),
                brief,
            );
            self.fulfillment_driver
                .evaluate_with_escalation(&pair, &key, ModelTier::Fast)
                .await
                .map_err(map_verifier_error)
        })
    }
}
