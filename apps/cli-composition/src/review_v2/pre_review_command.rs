//! Pre-review command adapter wiring for the review composition root.

use std::sync::Arc;

use infrastructure::operator_command_config::{
    FsPreReviewCommandConfigLoader, GitCurrentReviewTrackResolver,
};
use infrastructure::program_runner::ProcessProgramRunner;
use usecase::pre_review_command::{
    CurrentReviewTrackResolverPort, PreReviewCommandConfigLoaderPort,
    PreReviewCommandDispatchInteractor, PreReviewCommandDispatchService,
    PreReviewCommandGatedReviewInteractor,
};
use usecase::program_runner::ProgramRunnerPort;
use usecase::review_v2::ReviewService;

/// Wraps the real review service with the pre-review command dispatcher.
///
/// The returned service gates only `run_local`; every other review operation is
/// delegated unchanged by the use-case interactor.
pub(super) fn gate_local_review_service(inner: Arc<dyn ReviewService>) -> Arc<dyn ReviewService> {
    let config_loader: Arc<dyn PreReviewCommandConfigLoaderPort> =
        Arc::new(FsPreReviewCommandConfigLoader::new());
    let track_resolver: Arc<dyn CurrentReviewTrackResolverPort> =
        Arc::new(GitCurrentReviewTrackResolver::new());
    let runner: Arc<dyn ProgramRunnerPort> = Arc::new(ProcessProgramRunner::new());
    let dispatcher: Arc<dyn PreReviewCommandDispatchService> =
        Arc::new(PreReviewCommandDispatchInteractor::new(config_loader, track_resolver, runner));

    Arc::new(PreReviewCommandGatedReviewInteractor::new(inner, dispatcher))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    #[test]
    fn test_pre_review_command_wiring_uses_declared_adapters_only() {
        let source = include_str!("pre_review_command.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap();

        for required in [
            "FsPreReviewCommandConfigLoader::new()",
            "GitCurrentReviewTrackResolver::new()",
            "ProcessProgramRunner::new()",
            "PreReviewCommandDispatchInteractor::new(config_loader, track_resolver, runner)",
            "PreReviewCommandGatedReviewInteractor::new(inner, dispatcher)",
        ] {
            assert!(
                production_source.contains(required),
                "pre-review wiring must contain {required}"
            );
        }
        for forbidden in ["std::process::", "std::fs::", "Command::new", "clap", "bin/sotp"] {
            assert!(
                !production_source.contains(forbidden),
                "composition wiring must not contain {forbidden}"
            );
        }
    }
}
