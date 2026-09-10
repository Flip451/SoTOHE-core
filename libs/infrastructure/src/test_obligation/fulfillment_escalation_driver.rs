//! Escalation driver for obligation-fulfillment semantic verification.

use std::sync::Arc;

use domain::ModelTier;
use domain::tddd::test_obligation::errors::SemanticVerifierError;
use domain::tddd::test_obligation::pair::ObligationFulfillmentPair;
use domain::tddd::test_obligation::ports::ObligationFulfillmentVerifierPort;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict,
};
use usecase::semantic_verdict_core::driver::{
    SemanticEscalationDriverPort, SemanticEscalationFuture,
};
use usecase::semantic_verdict_core::probe::SemanticCalibrationProbeConfig;

use crate::test_obligation::spawn_blocking::SpawnBlocking;

/// Concrete escalation driver for the fulfillment verifier lane.
#[derive(Clone)]
pub struct ObligationFulfillmentEscalationDriver {
    verifier: Arc<dyn ObligationFulfillmentVerifierPort + Send + Sync>,
    probe_config: SemanticCalibrationProbeConfig,
}

impl std::fmt::Debug for ObligationFulfillmentEscalationDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObligationFulfillmentEscalationDriver")
            .field("probe_config", &self.probe_config)
            .finish_non_exhaustive()
    }
}

impl ObligationFulfillmentEscalationDriver {
    /// Wires the driver over the verifier port and shared probe config.
    #[must_use]
    pub fn new(
        verifier: Arc<dyn ObligationFulfillmentVerifierPort + Send + Sync>,
        probe_config: SemanticCalibrationProbeConfig,
    ) -> Self {
        Self { verifier, probe_config }
    }

    #[cfg(test)]
    fn probe_config(&self) -> &SemanticCalibrationProbeConfig {
        &self.probe_config
    }
}

impl
    SemanticEscalationDriverPort<
        ObligationFulfillmentPair,
        ObligationFulfillmentCacheKey,
        ObligationFulfillmentVerdict,
        SemanticVerifierError,
    > for ObligationFulfillmentEscalationDriver
{
    fn evaluate_with_escalation<'a>(
        &'a self,
        pair: &'a ObligationFulfillmentPair,
        _key: &'a ObligationFulfillmentCacheKey,
        initial_tier: ModelTier,
    ) -> SemanticEscalationFuture<'a, ObligationFulfillmentVerdict, SemanticVerifierError> {
        // Materialise the sync pair into an owned value before the async move
        // so each `SpawnBlocking` closure below has an `'static` capture and
        // can be driven on a worker thread while the bounded multiplexer in
        // `usecase::test_obligation::evaluate` polls its siblings. Keeping the
        // typed pair intact also prevents entry-local responsibility inputs
        // from being dropped at this boundary.
        let verifier = Arc::clone(&self.verifier);
        let pair = pair.clone();
        Box::pin(async move {
            let fast_verifier = Arc::clone(&verifier);
            let fast_pair = pair.clone();
            let fast_tier = initial_tier.clone();
            let verdict =
                SpawnBlocking::new(move || fast_verifier.verify_pair(&fast_pair, fast_tier))
                    .await?;
            if matches!(initial_tier, ModelTier::Fast)
                && !matches!(verdict, ObligationFulfillmentVerdict::Fulfilled { .. })
            {
                let final_verifier = Arc::clone(&verifier);
                return SpawnBlocking::new(move || {
                    final_verifier.verify_pair(&pair, ModelTier::Final)
                })
                .await;
            }
            Ok(verdict)
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::future::Future;
    use std::num::NonZeroU8;
    use std::pin::pin;
    use std::sync::Mutex;
    use std::task::{Context, Poll, Waker};

    use domain::EvidenceCitation;
    use domain::tddd::catalogue_v2::CatalogueEntryKey;
    use domain::tddd::test_obligation::hashes::{
        AnchorTextHash, BoundTestsSetHash, DeclarationHash,
    };
    use domain::tddd::test_obligation::ids::{
        DiagnosticMessage, TestObligationBrief, TestObligationId, TestObligationItemIdentifier,
    };
    use domain::tddd::test_obligation::pair::{AnchorText, EntryDeclaration, TestsSource};
    use domain::tddd::test_obligation::vocab::FulfillmentFailCategory;

    use super::*;

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(output) => return output,
                Poll::Pending => continue,
            }
        }
    }

    fn pass_verdict() -> ObligationFulfillmentVerdict {
        ObligationFulfillmentVerdict::Fulfilled {
            citation: EvidenceCitation::try_new("asserts the promised behavior".to_owned())
                .unwrap(),
        }
    }

    fn fail_verdict() -> ObligationFulfillmentVerdict {
        ObligationFulfillmentVerdict::Fail {
            category: FulfillmentFailCategory::CentralUnverified,
            reason: DiagnosticMessage::try_new("not covered".to_owned()).unwrap(),
        }
    }

    fn pair() -> ObligationFulfillmentPair {
        ObligationFulfillmentPair::new(
            TestsSource::try_new("test body".to_owned()).unwrap(),
            EntryDeclaration::try_new("entry declaration".to_owned()).unwrap(),
            AnchorText::try_new("anchor text".to_owned()).unwrap(),
            TestObligationId::new(
                CatalogueEntryKey::try_new("Entry".to_owned()).unwrap(),
                domain::tddd::test_obligation::vocab::TestObligationKind::Contract,
                TestObligationItemIdentifier::try_new("trait_method:verify".to_owned()).unwrap(),
            ),
            TestObligationBrief::try_new("verify the entry-local contract".to_owned()).unwrap(),
        )
    }

    fn key() -> ObligationFulfillmentCacheKey {
        ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(domain::ContentHash::from_bytes([1; 32])),
            DeclarationHash::new(domain::ContentHash::from_bytes([2; 32])),
            AnchorTextHash::new(domain::ContentHash::from_bytes([3; 32])),
        )
    }

    fn probe_config() -> SemanticCalibrationProbeConfig {
        SemanticCalibrationProbeConfig::new(
            NonZeroU8::new(10).unwrap(),
            NonZeroU8::new(90).unwrap(),
        )
    }

    struct StubVerifier {
        verdicts: Mutex<Vec<ObligationFulfillmentVerdict>>,
        calls: Mutex<Vec<ModelTier>>,
    }

    impl StubVerifier {
        fn new(verdicts: Vec<ObligationFulfillmentVerdict>) -> Self {
            Self { verdicts: Mutex::new(verdicts), calls: Mutex::new(Vec::new()) }
        }

        fn calls(&self) -> Vec<ModelTier> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl ObligationFulfillmentVerifierPort for StubVerifier {
        fn verify_pair(
            &self,
            pair: &ObligationFulfillmentPair,
            tier: ModelTier,
        ) -> Result<ObligationFulfillmentVerdict, SemanticVerifierError> {
            assert_eq!(pair.tests_source().as_str(), "test body");
            assert_eq!(pair.entry_declaration().as_str(), "entry declaration");
            assert_eq!(pair.anchor_text().as_str(), "anchor text");
            assert_eq!(pair.obligation_id().entry_key().as_str(), "Entry");
            assert_eq!(pair.obligation_id().item_identifier().as_str(), "trait_method:verify");
            assert_eq!(pair.obligation_brief().as_str(), "verify the entry-local contract");
            self.calls.lock().unwrap().push(tier);
            Ok(self.verdicts.lock().unwrap().remove(0))
        }
    }

    #[test]
    fn test_fulfillment_driver_with_fast_pass_returns_without_final() {
        let verifier = Arc::new(StubVerifier::new(vec![pass_verdict()]));
        let driver = ObligationFulfillmentEscalationDriver::new(verifier.clone(), probe_config());

        let verdict =
            block_on(driver.evaluate_with_escalation(&pair(), &key(), ModelTier::Fast)).unwrap();

        assert!(matches!(verdict, ObligationFulfillmentVerdict::Fulfilled { .. }));
        assert_eq!(verifier.calls(), vec![ModelTier::Fast]);
    }

    #[test]
    fn test_fulfillment_driver_with_fast_fail_escalates_to_final() {
        let verifier = Arc::new(StubVerifier::new(vec![fail_verdict(), pass_verdict()]));
        let driver = ObligationFulfillmentEscalationDriver::new(verifier.clone(), probe_config());

        let verdict =
            block_on(driver.evaluate_with_escalation(&pair(), &key(), ModelTier::Fast)).unwrap();

        assert!(matches!(verdict, ObligationFulfillmentVerdict::Fulfilled { .. }));
        assert_eq!(verifier.calls(), vec![ModelTier::Fast, ModelTier::Final]);
        assert_eq!(driver.probe_config().threshold().get(), 90);
    }
}
