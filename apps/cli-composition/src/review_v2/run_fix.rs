//! Wire-only review-fix composition.

use std::sync::Arc;

use usecase::review_v2::run_review_fix::{
    ReviewFixRunner, RunReviewFixInteractor, RunReviewFixService,
};

/// Wires infrastructure adapters into the review-fix usecase without invoking it.
pub(crate) fn review_fix_service() -> Arc<dyn RunReviewFixService> {
    review_fix_service_with_runner(Arc::new(infrastructure::review_v2::ReviewFixRunnerAdapter))
}

fn review_fix_service_with_runner(
    runner: Arc<dyn ReviewFixRunner>,
) -> Arc<dyn RunReviewFixService> {
    Arc::new(RunReviewFixInteractor::new(
        Arc::new(infrastructure::review_v2::GitReviewFixTrackResolver),
        runner,
    ))
}

#[cfg(test)]
pub(crate) fn review_fix_service_with_capturing_runner(
    runner: Arc<dyn ReviewFixRunner>,
) -> Arc<dyn RunReviewFixService> {
    review_fix_service_with_runner(runner)
}
