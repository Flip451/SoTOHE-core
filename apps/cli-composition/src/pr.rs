//! Pure DI composition root for the `pr` command family.

use std::sync::Arc;

/// Composition root for the `pr` command family.
pub struct PrCompositionRoot;

impl PrCompositionRoot {
    /// Creates a PR composition root.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Builds the fully wired PR driver.
    #[must_use]
    pub fn pr_driver(&self) -> cli_driver::pr::PrDriver {
        let port = Arc::new(infrastructure::pr::SystemPrCommandAdapter::new());
        self.pr_driver_with_port(port)
    }

    fn pr_driver_with_port(
        &self,
        port: Arc<dyn usecase::pr::PrCommandPort>,
    ) -> cli_driver::pr::PrDriver {
        let service = Arc::new(usecase::pr::PrCommandInteractor::new(port));
        cli_driver::pr::PrDriver::new(service)
    }
}

impl Default for PrCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::Path;
    use std::process::Command;
    use std::sync::Mutex;

    use cli_driver::pr::PrInput;
    use usecase::pr::{PrCommand, PrCommandOutput, PrCommandPort};

    use super::*;

    struct RecordingPort {
        commands: Mutex<Vec<PrCommand>>,
    }

    impl RecordingPort {
        fn new() -> Self {
            Self { commands: Mutex::new(Vec::new()) }
        }
    }

    impl PrCommandPort for RecordingPort {
        fn execute(&self, command: PrCommand) -> PrCommandOutput {
            self.commands.lock().unwrap().push(command);
            PrCommandOutput::with_exit_code(
                Some("driver stdout".to_owned()),
                Some("driver stderr".to_owned()),
                23,
            )
        }
    }

    fn run_git(path: &Path, args: &[&str]) {
        let output = Command::new("git").current_dir(path).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed:\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    #[test]
    fn test_pr_composition_root_pr_driver_constructs_wired_driver() {
        let root = PrCompositionRoot::new();
        let _driver = root.pr_driver();
    }

    #[test]
    fn test_pr_composition_root_wires_driver_through_interactor_to_typed_port() {
        let root = PrCompositionRoot::new();
        let port = Arc::new(RecordingPort::new());
        let driver = root.pr_driver_with_port(port.clone());

        let outcome = driver.handle(PrInput::Push { track_id: Some("wired-track".to_owned()) });

        assert_eq!(outcome.stdout.as_deref(), Some("driver stdout"));
        assert_eq!(outcome.stderr.as_deref(), Some("driver stderr"));
        assert_eq!(outcome.exit_code, 23);
        let commands = port.commands.lock().unwrap();
        assert!(
            matches!(commands.as_slice(), [PrCommand::Push { track_id: Some(id) }] if id.as_str() == "wired-track")
        );
    }

    #[test]
    fn test_pr_composition_root_all_commands_follow_single_path_and_preserve_outcomes() {
        let root = PrCompositionRoot::new();
        let port = Arc::new(RecordingPort::new());
        let driver = root.pr_driver_with_port(port.clone());

        let outcomes = [
            driver.handle(PrInput::Push { track_id: Some("INVALID".to_owned()) }),
            driver.handle(PrInput::Ensure {
                track_id: Some("INVALID".to_owned()),
                base: Some(" main ".to_owned()),
            }),
            driver.handle(PrInput::Status { pr: "123".to_owned() }),
            driver.handle(PrInput::WaitAndMerge {
                pr: "123".to_owned(),
                interval: 0,
                timeout: 0,
                method: Some("squash".to_owned()),
            }),
            driver.handle(PrInput::TriggerReview { pr: "123".to_owned() }),
            driver.handle(PrInput::PollReview {
                pr: "123".to_owned(),
                trigger_timestamp: "2026-07-26T00:00:00Z".to_owned(),
                interval: 0,
                timeout: 0,
            }),
            driver.handle(PrInput::ReviewCycle {
                track_id: Some("INVALID".to_owned()),
                resume: false,
            }),
            driver.handle(PrInput::ReviewCycle {
                track_id: Some("INVALID".to_owned()),
                resume: true,
            }),
        ];

        assert!(outcomes.iter().all(|outcome| {
            outcome.stdout.as_deref() == Some("driver stdout")
                && outcome.stderr.as_deref() == Some("driver stderr")
                && outcome.exit_code == 23
        }));
        let commands = port.commands.lock().unwrap();
        assert!(matches!(
            commands.as_slice(),
            [
                PrCommand::Push { track_id: Some(push_track_id) },
                PrCommand::Ensure { track_id: Some(ensure_track_id), base: Some(base) },
                PrCommand::Status(status_pr),
                PrCommand::WaitAndMerge { pr: merge_pr, interval: merge_interval, timeout: merge_timeout, method },
                PrCommand::TriggerReview(trigger_pr),
                PrCommand::PollReview { pr: poll_pr, trigger_timestamp, interval: poll_interval, timeout: poll_timeout },
                PrCommand::ReviewCycle { track_id: Some(start_track_id), mode: usecase::pr::PrReviewCycleMode::Start },
                PrCommand::ReviewCycle { track_id: Some(resume_track_id), mode: usecase::pr::PrReviewCycleMode::Resume },
            ] if push_track_id.as_str() == "INVALID"
                && ensure_track_id.as_str() == "INVALID"
                && base.as_str() == " main "
                && status_pr.as_str() == "123"
                && merge_pr.as_str() == "123"
                && merge_interval.as_secs() == 0
                && merge_timeout.as_secs() == 0
                && matches!(method, Some(domain::MergeMethod::Squash))
                && trigger_pr.as_str() == "123"
                && poll_pr.as_str() == "123"
                && trigger_timestamp.as_str() == "2026-07-26T00:00:00Z"
                && poll_interval.as_secs() == 0
                && poll_timeout.as_secs() == 0
                && start_track_id.as_str() == "INVALID"
                && resume_track_id.as_str() == "INVALID"
        ));
    }

    #[test]
    fn test_pr_composition_root_push_executes_system_adapter_and_persists_remote_ref() {
        let _process_lock = crate::test_support::process_env_lock().lock().unwrap();
        let sandbox = tempfile::tempdir().unwrap();
        let remote = sandbox.path().join("origin.git");
        let workspace = sandbox.path().join("workspace");
        let track_id = "pr-composition-contract";
        let branch = format!("track/{track_id}");

        let remote_init = Command::new("git")
            .args(["init", "--bare", remote.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            remote_init.status.success(),
            "bare remote init failed: {}",
            String::from_utf8_lossy(&remote_init.stderr),
        );
        std::fs::create_dir_all(&workspace).unwrap();
        run_git(&workspace, &["init", "--initial-branch", &branch]);
        run_git(&workspace, &["config", "user.email", "test@example.com"]);
        run_git(&workspace, &["config", "user.name", "Test User"]);
        std::fs::write(workspace.join("contract.txt"), "pure DI path\n").unwrap();
        run_git(&workspace, &["add", "contract.txt"]);
        run_git(&workspace, &["commit", "-m", "contract fixture"]);
        run_git(&workspace, &["remote", "add", "origin", remote.to_str().unwrap()]);

        let driver = PrCompositionRoot::new().pr_driver();
        let outcome = crate::test_support::run_in_dir(&workspace, || {
            driver.handle(PrInput::Push { track_id: Some(track_id.to_owned()) })
        });

        assert_eq!(outcome.stdout.as_deref(), Some(format!("[OK] Pushed {branch}").as_str()));
        assert_eq!(outcome.stderr, None);
        assert_eq!(outcome.exit_code, 0);

        let local_head = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&workspace)
            .output()
            .unwrap();
        let remote_head = Command::new("git")
            .args([
                "--git-dir",
                remote.to_str().unwrap(),
                "rev-parse",
                &format!("refs/heads/{branch}"),
            ])
            .output()
            .unwrap();
        assert!(local_head.status.success());
        assert!(remote_head.status.success());
        assert_eq!(
            local_head.stdout, remote_head.stdout,
            "pure-DI push must persist the branch ref at origin"
        );
    }
}
