//! Responsibility-neutral escalation driver port for the semantic-verdict core.
//!
//! [`SemanticEscalationDriverPort`] is the shared, verifier-agnostic driven port
//! that both ref-verify (chain 1 / chain 2) and the obligation-fulfillment gate
//! ride on: it owns the fast → final → human escalation and hash-frozen cache
//! access, while each verifier keeps its own payload (`P`), cache key (`K`),
//! verdict (`V`), and error (`E`) types (IN-01 / AC-15 / OS-02).

use std::future::Future;
use std::pin::Pin;

use domain::tddd::semantic_verify::ModelTier;

/// Send-safe boxed future returned by
/// [`SemanticEscalationDriverPort::evaluate_with_escalation`].
pub type SemanticEscalationFuture<'a, V, E> =
    Pin<Box<dyn Future<Output = Result<V, E>> + Send + 'a>>;

/// Drives a semantic verdict through the fast → final → human escalation ladder.
///
/// Generic over the four verifier-owned types so the core carries no knowledge
/// of any specific pair shape:
///
/// - `P` — the claim/evidence payload the verifier submits.
/// - `K` — the hash-frozen cache key the verifier defines.
/// - `V` — the verifier-owned verdict returned on success.
/// - `E` — the verifier-owned error returned on failure.
pub trait SemanticEscalationDriverPort<P, K, V, E>: Send + Sync
where
    P: Send + Sync,
    K: Send + Sync,
    V: Send + Sync,
    E: Send + Sync,
{
    /// Evaluates `pair` under `key`, starting at `initial_tier` and escalating.
    ///
    /// The returned future is `Send` so callers may fan out independent
    /// semantic evaluations across worker tasks.
    ///
    /// # Errors
    ///
    /// Returns the verifier-owned error `E` when evaluation cannot produce a
    /// verdict (e.g. the provider fails at every tier).
    fn evaluate_with_escalation<'a>(
        &'a self,
        pair: &'a P,
        key: &'a K,
        initial_tier: ModelTier,
    ) -> SemanticEscalationFuture<'a, V, E>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use super::*;

    /// Minimal executor for futures that never pend (the test stub resolves
    /// synchronously), avoiding a runtime dependency in the usecase crate.
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

    /// Verifier double: records the tier it was invoked at and returns a
    /// pre-programmed verdict / error.
    struct StubDriver {
        succeed: bool,
    }

    impl SemanticEscalationDriverPort<String, u8, bool, String> for StubDriver {
        fn evaluate_with_escalation<'a>(
            &'a self,
            pair: &'a String,
            key: &'a u8,
            initial_tier: ModelTier,
        ) -> SemanticEscalationFuture<'a, bool, String> {
            Box::pin(async move {
                // Exercise every parameter so the double mirrors a real driver.
                assert!(!pair.is_empty());
                assert_eq!(*key, 7);
                assert_eq!(initial_tier, ModelTier::Fast);
                if self.succeed { Ok(true) } else { Err("no verdict".to_owned()) }
            })
        }
    }

    #[test]
    fn test_driver_double_returns_verdict() {
        let driver = StubDriver { succeed: true };
        let outcome =
            block_on(driver.evaluate_with_escalation(&"claim".to_owned(), &7, ModelTier::Fast));
        assert_eq!(outcome, Ok(true));
    }

    #[test]
    fn test_driver_double_returns_error() {
        let driver = StubDriver { succeed: false };
        let outcome =
            block_on(driver.evaluate_with_escalation(&"claim".to_owned(), &7, ModelTier::Fast));
        assert_eq!(outcome, Err("no verdict".to_owned()));
    }
}
