//! Escalation driver for waiver semantic verification.

use std::sync::Arc;

use domain::ModelTier;
use domain::tddd::test_obligation::errors::SemanticVerifierError;
use domain::tddd::test_obligation::pair::WaiverPair;
use domain::tddd::test_obligation::ports::WaiverVerifierPort;
use domain::tddd::test_obligation::verdict::{WaiverCacheKey, WaiverVerdict};
use usecase::semantic_verdict_core::driver::{
    SemanticEscalationDriverPort, SemanticEscalationFuture,
};
use usecase::semantic_verdict_core::probe::SemanticCalibrationProbeConfig;

use crate::test_obligation::spawn_blocking::SpawnBlocking;

/// Concrete escalation driver for the waiver verifier lane.
#[derive(Clone)]
pub struct WaiverEscalationDriver {
    verifier: Arc<dyn WaiverVerifierPort + Send + Sync>,
    probe_config: SemanticCalibrationProbeConfig,
}

impl std::fmt::Debug for WaiverEscalationDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaiverEscalationDriver")
            .field("probe_config", &self.probe_config)
            .finish_non_exhaustive()
    }
}

impl WaiverEscalationDriver {
    /// Wires the driver over the verifier port and shared probe config.
    #[must_use]
    pub fn new(
        verifier: Arc<dyn WaiverVerifierPort + Send + Sync>,
        probe_config: SemanticCalibrationProbeConfig,
    ) -> Self {
        Self { verifier, probe_config }
    }

    #[cfg(test)]
    fn probe_config(&self) -> &SemanticCalibrationProbeConfig {
        &self.probe_config
    }
}

impl SemanticEscalationDriverPort<WaiverPair, WaiverCacheKey, WaiverVerdict, SemanticVerifierError>
    for WaiverEscalationDriver
{
    fn evaluate_with_escalation<'a>(
        &'a self,
        pair: &'a WaiverPair,
        _key: &'a WaiverCacheKey,
        initial_tier: ModelTier,
    ) -> SemanticEscalationFuture<'a, WaiverVerdict, SemanticVerifierError> {
        // Materialise the pair inputs into owned values before the async move
        // so each `SpawnBlocking` closure has an `'static` capture and can be
        // driven on a worker thread while the usecase-level bounded
        // multiplexer polls its siblings.
        let verifier = Arc::clone(&self.verifier);
        let waived_reason = pair.waived_reason().as_str().to_owned();
        let entry_declaration = pair.entry_declaration().as_str().to_owned();
        let anchor_text = pair.anchor_text().as_str().to_owned();
        Box::pin(async move {
            let fast_verifier = Arc::clone(&verifier);
            let (wr, ed, at, tier) = (
                waived_reason.clone(),
                entry_declaration.clone(),
                anchor_text.clone(),
                initial_tier.clone(),
            );
            let verdict =
                SpawnBlocking::new(move || fast_verifier.verify_pair(&wr, &ed, &at, tier)).await?;
            if matches!(initial_tier, ModelTier::Fast)
                && !matches!(verdict, WaiverVerdict::Waived { .. })
            {
                let final_verifier = Arc::clone(&verifier);
                return SpawnBlocking::new(move || {
                    final_verifier.verify_pair(
                        &waived_reason,
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
        AnchorTextHash, DeclarationHash, WaivedReasonHash,
    };
    use domain::tddd::test_obligation::ids::{DiagnosticMessage, WaivedReason};
    use domain::tddd::test_obligation::pair::{AnchorText, EntryDeclaration};

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

    fn pass_verdict() -> WaiverVerdict {
        WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("anchor makes this structural".to_owned()).unwrap(),
        }
    }

    fn fail_verdict() -> WaiverVerdict {
        WaiverVerdict::Fail {
            reason: DiagnosticMessage::try_new("waiver does not hold".to_owned()).unwrap(),
        }
    }

    fn pair() -> WaiverPair {
        WaiverPair::new(
            WaivedReason::try_new("type system guarantee".to_owned()).unwrap(),
            EntryDeclaration::try_new("entry declaration".to_owned()).unwrap(),
            AnchorText::try_new("anchor text".to_owned()).unwrap(),
        )
    }

    fn key() -> WaiverCacheKey {
        WaiverCacheKey::new(
            WaivedReasonHash::new(domain::ContentHash::from_bytes([1; 32])),
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
        verdicts: Mutex<Vec<WaiverVerdict>>,
        calls: Mutex<Vec<ModelTier>>,
    }

    impl StubVerifier {
        fn new(verdicts: Vec<WaiverVerdict>) -> Self {
            Self { verdicts: Mutex::new(verdicts), calls: Mutex::new(Vec::new()) }
        }

        fn calls(&self) -> Vec<ModelTier> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl WaiverVerifierPort for StubVerifier {
        fn verify_pair(
            &self,
            waived_reason: &str,
            entry_declaration: &str,
            anchor_text: &str,
            tier: ModelTier,
        ) -> Result<WaiverVerdict, SemanticVerifierError> {
            assert_eq!(waived_reason, "type system guarantee");
            assert_eq!(entry_declaration, "entry declaration");
            assert_eq!(anchor_text, "anchor text");
            self.calls.lock().unwrap().push(tier);
            Ok(self.verdicts.lock().unwrap().remove(0))
        }
    }

    #[test]
    fn test_waiver_driver_with_fast_pass_returns_without_final() {
        let verifier = Arc::new(StubVerifier::new(vec![pass_verdict()]));
        let driver = WaiverEscalationDriver::new(verifier.clone(), probe_config());

        let verdict =
            block_on(driver.evaluate_with_escalation(&pair(), &key(), ModelTier::Fast)).unwrap();

        assert!(matches!(verdict, WaiverVerdict::Waived { .. }));
        assert_eq!(verifier.calls(), vec![ModelTier::Fast]);
    }

    #[test]
    fn test_waiver_driver_with_fast_fail_escalates_to_final() {
        let verifier = Arc::new(StubVerifier::new(vec![fail_verdict(), pass_verdict()]));
        let driver = WaiverEscalationDriver::new(verifier.clone(), probe_config());

        let verdict =
            block_on(driver.evaluate_with_escalation(&pair(), &key(), ModelTier::Fast)).unwrap();

        assert!(matches!(verdict, WaiverVerdict::Waived { .. }));
        assert_eq!(verifier.calls(), vec![ModelTier::Fast, ModelTier::Final]);
        assert_eq!(driver.probe_config().injection().get(), 10);
    }
}
