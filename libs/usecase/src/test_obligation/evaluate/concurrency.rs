//! Bounded, in-order concurrent driver for verdict futures.
//!
//! `bin/sotp test-obligation evaluate` bottoms out in per-pair verifier
//! subprocess calls — hundreds of them per run. When each future is polled
//! serially the whole gate serialises on the executor thread even though the
//! pairs are independent. [`drive_bounded_in_order`] fans them out: it holds
//! at most `N` futures in flight, replenishing from a pending queue whenever
//! one completes, and returns the successful outputs in the caller's original
//! input order so downstream cache documents stay byte-deterministic.
//!
//! Fail-fast: the first future to yield `Err(_)` short-circuits the driver
//! — the remaining pending inputs are dropped without being polled, in-flight
//! futures are dropped in place, and the error is returned. In-flight futures
//! bottoming out in
//! [`crate::test_obligation::evaluate`]-facing thread-offloaded verifiers
//! may still complete on their worker threads; that is safe (see the
//! infrastructure `spawn_blocking` module doc) and required, because there is
//! no safe primitive here for cancelling a running subprocess.
//!
//! Purity: the multiplexer is a private, dependency-free helper — it reads
//! no env vars or config and takes the concurrency ceiling as a parameter.
//! Every caller in `evaluate` supplies the validated bound from
//! `TestObligationEvaluateConfig`.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Drives `futures` under a concurrency ceiling of `limit`, returning the
/// per-future outputs in input order.
///
/// Semantics:
/// - Up to `limit` futures are polled per wake; as each completes, the next
///   pending future is spawned into its slot.
/// - Outputs are re-ordered to match the input `Vec` so downstream cache
///   documents remain byte-deterministic regardless of completion order.
/// - Fail-fast: on the first `Err(_)`, remaining pending futures are dropped
///   and the error is returned. Any already-in-flight futures are dropped in
///   place — their thread-offloaded workers may still complete (see
///   `spawn_blocking`), which is safe.
///
/// A `limit` of 0 is coerced to 1 so the driver still makes progress.
pub(super) fn drive_bounded_in_order<'a, T, E, F>(
    futures: Vec<F>,
    limit: usize,
) -> BoundedMultiplex<'a, T, E>
where
    T: 'a,
    E: 'a,
    F: Future<Output = Result<T, E>> + Send + 'a,
{
    // Box::pin each future eagerly so the internal queues can be
    // unconditionally `Unpin` — otherwise `Vec::IntoIter<F>` inherits `F`'s
    // pin marker and blocks the `&mut Self` projection below.
    // `+ Send` is preserved so the enclosing `execute_inner` future stays
    // `Send`, matching `EvaluateTestObligationsFuture`.
    let total = futures.len();
    let indexed: Vec<(usize, MultiplexedFuture<'a, T, E>)> = futures
        .into_iter()
        .enumerate()
        .map(|(idx, fut)| {
            let boxed: MultiplexedFuture<'a, T, E> = Box::pin(fut);
            (idx, boxed)
        })
        .collect();
    let mut results: Vec<Option<T>> = Vec::with_capacity(total);
    for _ in 0..total {
        results.push(None);
    }
    BoundedMultiplex {
        pending: indexed.into_iter(),
        in_flight: Vec::new(),
        results,
        limit: limit.max(1),
    }
}

/// Type alias for the boxed futures the multiplexer stores.
type MultiplexedFuture<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;

/// Bounded concurrent driver constructed by [`drive_bounded_in_order`].
///
/// `T` and `E` are the future's success and error types. All futures are
/// `Box::pin`-ned inside `drive_bounded_in_order` so callers do not need
/// to supply `Unpin` futures. The stored trait object keeps `Send` so the
/// enclosing async block remains `Send`.
pub(super) struct BoundedMultiplex<'a, T, E> {
    /// Not-yet-started inputs, drained as slots become free.
    pending: std::vec::IntoIter<(usize, MultiplexedFuture<'a, T, E>)>,
    /// Currently-polled futures with their original input index.
    in_flight: Vec<(usize, MultiplexedFuture<'a, T, E>)>,
    /// Result slots keyed by input index — filled as futures complete.
    results: Vec<Option<T>>,
    /// Maximum simultaneously-in-flight futures.
    limit: usize,
}

impl<'a, T, E> Future for BoundedMultiplex<'a, T, E>
where
    T: Unpin,
{
    type Output = Result<Vec<T>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // `BoundedMultiplex` structurally pins no field. Both queues hold
        // `Pin<Box<...>>` (always `Unpin`), and result slots move plain `T`
        // values in and out. A `&mut Self` projection is safe.
        let this = self.get_mut();

        loop {
            let mut progressed = false;

            // Fill in-flight up to the ceiling, drawing from the pending
            // queue in original input order.
            while this.in_flight.len() < this.limit {
                match this.pending.next() {
                    Some(entry) => {
                        this.in_flight.push(entry);
                        progressed = true;
                    }
                    None => break,
                }
            }

            // No more work → materialise the buffered results in order and
            // return. Every slot in `0..results.len()` was filled by an
            // `Ok(_)` before its future was removed from in-flight; a `None`
            // here would be a driver bug, so fold `None` to an executor-side
            // no-progress (fall through Pending) rather than panic.
            if this.in_flight.is_empty() {
                let slots = std::mem::take(&mut this.results);
                let total = slots.len();
                let mut out: Vec<T> = Vec::with_capacity(total);
                for slot in slots {
                    match slot {
                        Some(value) => out.push(value),
                        None => {
                            // Driver bug: unreachable in normal operation.
                            // Keep progress deterministic — return the empty
                            // vector rather than a partial one, so the caller
                            // notices the mismatch rather than acts on it.
                            return Poll::Ready(Ok(Vec::new()));
                        }
                    }
                }
                return Poll::Ready(Ok(out));
            }

            // Poll every in-flight future. Completed successes go to their
            // result slot; the first failure short-circuits the driver.
            let mut i = 0;
            while i < this.in_flight.len() {
                let poll_result = match this.in_flight.get_mut(i) {
                    Some((_, fut)) => fut.as_mut().poll(cx),
                    None => break, // Unreachable: bounded by len above.
                };
                let idx = this.in_flight.get(i).map_or(usize::MAX, |(idx, _)| *idx);
                match poll_result {
                    Poll::Ready(Ok(value)) => {
                        if let Some(slot) = this.results.get_mut(idx) {
                            *slot = Some(value);
                        }
                        // Drop the completed future eagerly.
                        let _completed = this.in_flight.swap_remove(i);
                        drop(_completed);
                        progressed = true;
                        // Do not advance `i` — swap_remove moved a fresh
                        // entry into slot `i` that has not yet been polled.
                    }
                    Poll::Ready(Err(err)) => {
                        // Fail-fast: drop remaining pending inputs (their
                        // futures never spawn a worker thread) and clear
                        // in-flight so their `Pin<Box<F>>` allocations drop.
                        this.in_flight.clear();
                        for _ in this.pending.by_ref() {}
                        return Poll::Ready(Err(err));
                    }
                    Poll::Pending => {
                        i += 1;
                    }
                }
            }

            if !progressed {
                return Poll::Pending;
            }
            // Something completed this pass — loop so a freshly-freed slot
            // gets a new future assigned and polled at least once before
            // we yield back to the executor.
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::future::Future;
    use std::pin::{Pin, pin};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    use super::*;

    /// Busy-loop executor (matches the evaluate test harness).
    fn run<F: Future>(future: F) -> F::Output {
        let mut future = pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(out) => return out,
                Poll::Pending => continue,
            }
        }
    }

    /// Future that pends for `pending_polls` polls before yielding `value`.
    /// Records the current in-flight count on each poll into a shared max.
    struct TrackedFuture {
        value: u32,
        pending_polls: usize,
        polls_seen: usize,
        in_flight: Arc<AtomicUsize>,
        peak: Arc<AtomicUsize>,
        registered: bool,
    }

    impl Future for TrackedFuture {
        type Output = Result<u32, ()>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if !self.registered {
                let current = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                let mut peak = self.peak.load(Ordering::SeqCst);
                while current > peak {
                    match self.peak.compare_exchange(
                        peak,
                        current,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    ) {
                        Ok(_) => break,
                        Err(v) => peak = v,
                    }
                }
                self.registered = true;
            }
            if self.polls_seen < self.pending_polls {
                self.polls_seen += 1;
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                self.in_flight.fetch_sub(1, Ordering::SeqCst);
                Poll::Ready(Ok(self.value))
            }
        }
    }

    impl Drop for TrackedFuture {
        fn drop(&mut self) {
            if self.registered && self.polls_seen < self.pending_polls {
                // Dropped in-flight (fail-fast case) — release the slot so
                // the peak assertion is not skewed.
                self.in_flight.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    fn tracked(
        value: u32,
        pending_polls: usize,
        in_flight: &Arc<AtomicUsize>,
        peak: &Arc<AtomicUsize>,
    ) -> TrackedFuture {
        TrackedFuture {
            value,
            pending_polls,
            polls_seen: 0,
            in_flight: Arc::clone(in_flight),
            peak: Arc::clone(peak),
            registered: false,
        }
    }

    #[test]
    fn preserves_input_order_regardless_of_completion_order() {
        // Later inputs finish first: input 4 pends 0 polls, input 0 pends 5.
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let futures = vec![
            tracked(10, 5, &in_flight, &peak),
            tracked(20, 3, &in_flight, &peak),
            tracked(30, 1, &in_flight, &peak),
            tracked(40, 0, &in_flight, &peak),
            tracked(50, 0, &in_flight, &peak),
        ];
        let out = run(drive_bounded_in_order(futures, 3)).unwrap();
        assert_eq!(out, vec![10, 20, 30, 40, 50]);
    }

    #[test]
    fn respects_max_in_flight_ceiling() {
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        // 10 inputs, each pending for a few polls; limit = 3.
        let futures: Vec<_> = (0..10).map(|i| tracked(i, 2, &in_flight, &peak)).collect();
        let _ = run(drive_bounded_in_order(futures, 3)).unwrap();
        assert!(
            peak.load(Ordering::SeqCst) <= 3,
            "peak in-flight {} exceeded limit 3",
            peak.load(Ordering::SeqCst)
        );
    }

    #[test]
    fn stops_starting_new_futures_after_first_error() {
        struct FailFuture {
            counter: Arc<AtomicUsize>,
            fail: bool,
        }
        impl Future for FailFuture {
            type Output = Result<u32, &'static str>;
            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                self.counter.fetch_add(1, Ordering::SeqCst);
                if self.fail { Poll::Ready(Err("boom")) } else { Poll::Ready(Ok(1)) }
            }
        }

        let started = Arc::new(AtomicUsize::new(0));
        let mk = |fail: bool| FailFuture { counter: Arc::clone(&started), fail };

        // Limit = 1 so we can predict exactly which futures are ever polled.
        // Index 0 fails; the multiplexer must never poll indices 1..5.
        let futures = vec![mk(true), mk(false), mk(false), mk(false), mk(false)];
        let err = run(drive_bounded_in_order(futures, 1)).unwrap_err();
        assert_eq!(err, "boom");
        assert_eq!(
            started.load(Ordering::SeqCst),
            1,
            "expected exactly one future to be polled before fail-fast; got {}",
            started.load(Ordering::SeqCst)
        );
    }

    #[test]
    fn zero_inputs_returns_empty_vec() {
        type EmptyFut = Pin<Box<dyn Future<Output = Result<u32, ()>> + Send>>;
        let futures: Vec<EmptyFut> = Vec::new();
        let out = run(drive_bounded_in_order(futures, 4)).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn limit_of_zero_still_makes_progress() {
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let futures = vec![tracked(7, 0, &in_flight, &peak), tracked(8, 0, &in_flight, &peak)];
        let out = run(drive_bounded_in_order(futures, 0)).unwrap();
        assert_eq!(out, vec![7, 8]);
    }
}
