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
        // Materialise the sync pair inputs into owned values before the async
        // move so each `SpawnBlocking` closure below has an `'static` capture
        // and can be driven on a worker thread while the bounded multiplexer
        // in `usecase::test_obligation::evaluate` polls its siblings.
        let verifier = Arc::clone(&self.verifier);
        let tests_source = pair.tests_source().as_str().to_owned();
        let entry_declaration = pair.entry_declaration().as_str().to_owned();
        let anchor_text = pair.anchor_text().as_str().to_owned();
        Box::pin(async move {
            let fast_verifier = Arc::clone(&verifier);
            let (ts, ed, at, tier) = (
                tests_source.clone(),
                entry_declaration.clone(),
                anchor_text.clone(),
                initial_tier.clone(),
            );
            let verdict =
                SpawnBlocking::new(move || fast_verifier.verify_pair(&ts, &ed, &at, tier)).await?;
            if matches!(initial_tier, ModelTier::Fast)
                && !matches!(verdict, ObligationFulfillmentVerdict::Fulfilled { .. })
            {
                let final_verifier = Arc::clone(&verifier);
                return SpawnBlocking::new(move || {
                    final_verifier.verify_pair(
                        &tests_source,
                        &entry_declaration,
                        &anchor_text,
                        ModelTier::Final,
                    )
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
    use domain::tddd::test_obligation::hashes::{
        AnchorTextHash, BoundTestsSetHash, DeclarationHash,
    };
    use domain::tddd::test_obligation::ids::DiagnosticMessage;
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
            tests_source: &str,
            entry_declaration: &str,
            anchor_text: &str,
            tier: ModelTier,
        ) -> Result<ObligationFulfillmentVerdict, SemanticVerifierError> {
            assert_eq!(tests_source, "test body");
            assert_eq!(entry_declaration, "entry declaration");
            assert_eq!(anchor_text, "anchor text");
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
