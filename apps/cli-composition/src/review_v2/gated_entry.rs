//! Gate-aware public entry for the local review flow.
//!
//! Split from `review_v2/mod.rs` so the composition module stays within the
//! workspace module-size limit.

use cli_driver::render::CommandOutcome;

use super::{ReviewRunLocalInput, pre_review_command, shim};
use crate::CompositionError;

impl super::ReviewCompositionRoot {
    /// Run the local reviewer through the configured pre-review command gates.
    ///
    /// Every public entry into the local review flow passes the scope-aware
    /// pre-review dispatch; the ungated execution body is reachable only via
    /// the crate-private review service inside the gated service graph. This
    /// preserves the baseline public surface while closing the direct-call
    /// gate bypass.
    ///
    /// # Errors
    /// Never returns `Err` today; gate failures and review-cycle failures are
    /// reported through the returned outcome. The `Result` shape is kept for
    /// public-surface stability.
    pub fn review_run_local(
        &self,
        input: ReviewRunLocalInput,
    ) -> Result<CommandOutcome, CompositionError> {
        let inner = std::sync::Arc::new(shim::review_service_impl())
            as std::sync::Arc<dyn usecase::review_v2::aggregate_service::ReviewService>;
        let service = pre_review_command::gate_local_review_service(inner);
        let output = service.run_local(
            input.model,
            input.timeout_seconds,
            input.briefing_file,
            input.prompt,
            input.track_id,
            input.round_type,
            input.group,
            input.items_dir,
        );
        Ok(CommandOutcome {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.exit_code,
        })
    }
}
