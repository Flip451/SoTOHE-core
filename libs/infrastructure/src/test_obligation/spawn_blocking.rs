//! Thread-offloaded future for the semantic verifier subprocess calls.
//!
//! The obligation-fulfillment and waiver verifier ports (`verify_pair`) are
//! synchronous — they spawn a provider subprocess and block until it returns.
//! Polling such a call inside an `async` block ties up the executor thread for
//! seconds to minutes at a time, which serialises `bin/sotp test-obligation
//! evaluate` even though the pairs are independent.
//!
//! [`SpawnBlocking`] wraps one such sync call as a waker-driven future: the
//! first poll moves the closure onto a fresh `std::thread` and stashes the
//! caller's waker, subsequent polls re-check the shared result slot and either
//! yield `Ready(result)` or update the stored waker with the latest one. When
//! the worker thread finishes it stores the result and wakes the stored waker,
//! so a `park`-based `block_on` (see [`crate::test_obligation::fulfillment_escalation_driver`]
//! callers) is unblocked exactly once per verifier call.
//!
//! Dropping the future before the thread finishes is safe: the state slot
//! lives inside an `Arc` shared with the worker, so the worker's later write
//! goes to memory it still co-owns. The worker thread is detached — it runs
//! to completion even if its future was dropped (e.g. fail-fast abandonment
//! of the surrounding multiplexer). Detached threads finishing after future
//! drop is intentional: cancelling a subprocess mid-flight has no safe
//! primitive here, so we accept the completed work.

use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use domain::tddd::test_obligation::errors::SemanticVerifierError;

use crate::test_obligation::semantic_verifier::semantic_verifier_error;

/// Shared state between the future and its worker thread.
struct SpawnBlockingState<T> {
    /// Filled by the worker thread when the closure returns; taken by
    /// the future's `poll` to become the `Poll::Ready` value.
    result: Option<Result<T, SemanticVerifierError>>,
    /// Latest waker registered by `poll`; the worker calls `wake()` on it
    /// after storing the result. Updated on every poll so the executor's
    /// current task, not a stale one, is woken.
    waker: Option<Waker>,
}

/// A future that runs a blocking semantic-verifier call on a fresh worker thread.
///
/// The future always resolves to a typed verifier result: a thread-spawn failure
/// or worker panic is returned as a fail-closed [`SemanticVerifierError`]. See
/// the module docs for the drop / cancellation contract.
pub(crate) struct SpawnBlocking<T: Send + 'static> {
    state: Arc<Mutex<SpawnBlockingState<T>>>,
    /// `Some(_)` until the first poll spawns the worker; consumed there.
    thunk: Option<Box<dyn FnOnce() -> Result<T, SemanticVerifierError> + Send + 'static>>,
}

impl<T: Send + 'static> SpawnBlocking<T> {
    /// Constructs a not-yet-spawned future around `thunk`.
    ///
    /// The thread is spawned on the first `poll`; if the future is dropped
    /// before it is polled once, no worker thread is created.
    pub(crate) fn new<F>(thunk: F) -> Self
    where
        F: FnOnce() -> Result<T, SemanticVerifierError> + Send + 'static,
    {
        Self {
            state: Arc::new(Mutex::new(SpawnBlockingState { result: None, waker: None })),
            thunk: Some(Box::new(thunk)),
        }
    }
}

impl<T: Send + 'static> Future for SpawnBlocking<T> {
    type Output = Result<T, SemanticVerifierError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // `SpawnBlocking` never projects a pinned field: the closure is moved
        // into a boxed heap slot before being consumed by the worker thread,
        // and the shared state lives behind an `Arc`. Both are structurally
        // `Unpin`, so a plain `&mut Self` is safe.
        let this = self.get_mut();

        // First poll: hand the closure to a worker thread and record the
        // caller's waker so the eventual `wake()` reaches the current task.
        if let Some(thunk) = this.thunk.take() {
            {
                let mut guard = this.state.lock().unwrap_or_else(|e| e.into_inner());
                guard.waker = Some(cx.waker().clone());
            }
            let state = Arc::clone(&this.state);
            return match std::thread::Builder::new().spawn(move || {
                let outcome = catch_unwind(AssertUnwindSafe(thunk)).unwrap_or_else(|_| {
                    Err(semantic_verifier_error("semantic verifier worker thread panicked"))
                });
                let waker = {
                    let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
                    guard.result = Some(outcome);
                    guard.waker.take()
                };
                // Wake outside the lock so the executor's re-poll does not
                // contend with our still-held guard.
                if let Some(waker) = waker {
                    waker.wake();
                }
            }) {
                Ok(_) => Poll::Pending,
                Err(error) => Poll::Ready(Err(semantic_verifier_error(&format!(
                    "failed to spawn semantic verifier worker thread: {error}"
                )))),
            };
        }

        // Subsequent polls: check whether the worker finished, and refresh
        // the stored waker so re-polls under a new task waker still fire.
        let mut guard = this.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(outcome) = guard.result.take() {
            return Poll::Ready(outcome);
        }
        // Only replace the stored waker if it is missing or wakes a different
        // task; `will_wake` avoids reallocating the same waker every poll.
        let refresh = guard.waker.as_ref().is_none_or(|stored| !stored.will_wake(cx.waker()));
        if refresh {
            guard.waker = Some(cx.waker().clone());
        }
        Poll::Pending
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::pin::pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::Wake;
    use std::time::Duration;

    use super::*;

    /// Waker that parks the current thread — matches the `block_on` pattern
    /// the cli-driver adapter uses, so we exercise the same code path here.
    struct ThreadParkWaker(std::thread::Thread);
    impl Wake for ThreadParkWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }

    fn parking_block_on<F: Future>(future: F) -> F::Output {
        let mut future = pin!(future);
        let waker = Waker::from(Arc::new(ThreadParkWaker(std::thread::current())));
        let mut cx = Context::from_waker(&waker);
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(out) => return out,
                Poll::Pending => std::thread::park(),
            }
        }
    }

    #[test]
    fn resolves_to_the_thunk_return_value() {
        let out = parking_block_on(SpawnBlocking::new(|| Ok(42_u32)));
        assert_eq!(out.unwrap(), 42);
    }

    #[test]
    fn parking_executor_is_woken_after_short_thread_sleep() {
        // If the waker were not registered / fired, `park()` would deadlock
        // — the test would hang until the harness timed out. A successful
        // return is the assertion.
        let out = parking_block_on(SpawnBlocking::new(|| {
            std::thread::sleep(Duration::from_millis(20));
            Ok("done")
        }));
        assert_eq!(out.unwrap(), "done");
    }

    #[test]
    fn thunk_runs_only_after_first_poll() {
        let ran = Arc::new(AtomicUsize::new(0));
        let ran_clone = Arc::clone(&ran);
        // Construct but never poll — dropping the future must not spawn the thread.
        let fut = SpawnBlocking::new(move || {
            ran_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        drop(fut);
        // Give any (erroneously-spawned) thread time to run.
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(ran.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn late_result_is_delivered_after_repoll_with_new_waker() {
        // Simulate an executor that re-polls with a different waker each time
        // — the second waker must be the one that fires.
        struct NoopWaker;
        impl Wake for NoopWaker {
            fn wake(self: Arc<Self>) {}
            fn wake_by_ref(self: &Arc<Self>) {}
        }

        let mut future = pin!(SpawnBlocking::new(|| {
            std::thread::sleep(Duration::from_millis(30));
            Ok(123_u32)
        }));

        // Poll #1 with a noop waker (no wake-up would ever fire on this waker).
        let noop = Waker::from(Arc::new(NoopWaker));
        let mut cx = Context::from_waker(&noop);
        assert!(matches!(future.as_mut().poll(&mut cx), Poll::Pending));

        // Poll #2 immediately with the parking waker — SpawnBlocking must
        // replace the stored waker with this one so the eventual wake reaches
        // us (via park unblock).
        let parking = Waker::from(Arc::new(ThreadParkWaker(std::thread::current())));
        let mut cx2 = Context::from_waker(&parking);
        loop {
            match future.as_mut().poll(&mut cx2) {
                Poll::Ready(v) => {
                    assert_eq!(v.unwrap(), 123);
                    return;
                }
                Poll::Pending => std::thread::park(),
            }
        }
    }

    #[test]
    fn test_spawn_blocking_worker_panic_returns_typed_error() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let _worker = std::thread::spawn(move || {
            let outcome =
                parking_block_on(SpawnBlocking::new(|| -> Result<u32, SemanticVerifierError> {
                    panic!("simulated verifier worker panic");
                }));
            sender.send(outcome).unwrap();
        });

        let error = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("a panicking worker must wake and complete the future")
            .expect_err("a panicking worker must fail closed");
        let SemanticVerifierError::VerifierPort(message) = error;
        assert_eq!(message.as_str(), "semantic verifier worker thread panicked");
    }
}
