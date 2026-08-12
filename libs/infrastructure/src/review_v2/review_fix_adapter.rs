//! Infrastructure adapters for provider-resolved review-fix execution.

use std::path::Path;

use usecase::capability_exec::ModelName;
use usecase::dry_write_driver::CapabilityName;
use usecase::git_workflow::DiagnosticText;
use usecase::review_v2::run_review_fix::{
    ReviewFixResolution, ReviewFixRunner, ReviewFixRunnerError, ReviewFixTrackResolveError,
    ReviewFixTrackResolverPort, ReviewTrackId, RunReviewFixCommand, RunReviewFixOutput,
    SubagentDispatchInstruction, SubagentName,
};
use usecase::review_v2::{ReviewRoundType, ReviewScopeName};

use super::CodexReviewFixRunner;

pub struct ReviewFixRunnerAdapter;
pub struct GitReviewFixTrackResolver;
impl ReviewFixTrackResolverPort for GitReviewFixTrackResolver {
    fn resolve_current_track(
        &self,
        items_dir: &Path,
    ) -> Result<ReviewFixResolution, ReviewFixTrackResolveError> {
        let (git, _) = crate::discover_isolated_repo_for_items_dir(items_dir).map_err(|error| {
            ReviewFixTrackResolveError::BranchReadFailed(diagnostic(error.to_string()))
        })?;
        let output = crate::git_cli::isolated_bounded_git_output(
            git.root(),
            &["rev-parse", "--abbrev-ref", "HEAD"],
            4096,
        )
        .map_err(|error| {
            ReviewFixTrackResolveError::BranchReadFailed(diagnostic(error.to_string()))
        })?;
        if !output.status.success() {
            return Err(ReviewFixTrackResolveError::BranchReadFailed(diagnostic(format!(
                "git rev-parse --abbrev-ref HEAD failed with exit status {}",
                output.status.code().unwrap_or(-1)
            ))));
        }
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let track = branch.strip_prefix("track/").ok_or_else(|| {
            ReviewFixTrackResolveError::NonTrackBranch(diagnostic(format!(
                "current branch '{branch}' is not a track branch"
            )))
        })?;
        let track_id = ReviewTrackId::try_new(track.to_owned()).map_err(|error| match error {
            usecase::review_v2::run_review_fix::ReviewTrackIdValidationError::Invalid(detail) => {
                ReviewFixTrackResolveError::NonTrackBranch(detail)
            }
        })?;
        let repository_root = git.root().canonicalize().map_err(|error| {
            ReviewFixTrackResolveError::BranchReadFailed(diagnostic(error.to_string()))
        })?;
        Ok(ReviewFixResolution::new(track_id, repository_root))
    }
}

impl ReviewFixRunner for ReviewFixRunnerAdapter {
    fn run_fix(
        &self,
        command: RunReviewFixCommand,
    ) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
        let repository_root = command.repository_root().canonicalize().map_err(|error| {
            ReviewFixRunnerError::Unexpected(diagnostic(format!(
                "failed to access resolver-proven repository root: {error}"
            )))
        })?;
        let repo = crate::git_cli::SystemGitRepo::discover_from_isolated(&repository_root)
            .map_err(|error| {
                ReviewFixRunnerError::Unexpected(diagnostic(format!(
                    "resolver-proven repository root is not a repository: {error}"
                )))
            })?;
        if repo.root().canonicalize().ok().as_deref() != Some(repository_root.as_path()) {
            return Err(ReviewFixRunnerError::Unexpected(diagnostic(
                "resolver-proven repository root does not match the runner repository",
            )));
        }
        let briefing_content =
            crate::review_v2::review_fix_briefing::read_trusted_briefing(&command)?;
        let profiles_path = repository_root.join(crate::agent_profiles::AGENT_PROFILES_PATH);
        let profiles = crate::agent_profiles::AgentProfiles::load(&repository_root, &profiles_path)
            .map_err(|error| ReviewFixRunnerError::Unexpected(diagnostic(error.to_string())))?;
        let capability = CapabilityName::try_new("review-fix-lead")
            .map_err(|error| ReviewFixRunnerError::Unexpected(diagnostic(error.to_string())))?;
        let round = match command.round_type() {
            ReviewRoundType::Fast => crate::agent_profiles::RoundType::Fast,
            ReviewRoundType::Final => crate::agent_profiles::RoundType::Final,
        };
        let crate::agent_profiles::ResolvedExecution::ProviderCli { provider, model, effort } =
            profiles
                .resolve_execution(&capability, round)
                .map_err(|error| ReviewFixRunnerError::Unexpected(diagnostic(error.to_string())))?
        else {
            return Err(ReviewFixRunnerError::Unexpected(diagnostic(
                "review-fix-lead must resolve to a provider CLI execution",
            )));
        };
        let model = match command.model().cloned() {
            Some(model) => model,
            None => ModelName::try_new(model.as_str().to_owned())
                .map_err(|error| ReviewFixRunnerError::Unexpected(diagnostic(error.to_string())))?,
        };
        match provider.as_str() {
            "codex" => CodexReviewFixRunner::new(model, effort)
                .run_fix_with_briefing(command, briefing_content),
            "claude" => Err(ReviewFixRunnerError::SubagentDispatchRequired(Box::new(
                SubagentDispatchInstruction {
                    agent: SubagentName::try_new("review-fix-lead".to_owned()).map_err(
                        |error| ReviewFixRunnerError::Unexpected(diagnostic(error.to_string())),
                    )?,
                    model,
                    effort,
                    scope: ReviewScopeName::try_new(command.scope().to_owned()).map_err(
                        |error| ReviewFixRunnerError::Unexpected(diagnostic(error.to_string())),
                    )?,
                    briefing_file: command.briefing_file().to_path_buf(),
                    track_id: ReviewTrackId::try_new(command.track_id().to_owned()).map_err(
                        |error| ReviewFixRunnerError::Unexpected(diagnostic(error.to_string())),
                    )?,
                    repository_root: command.repository_root().to_path_buf(),
                    round_type: command.round_type().clone(),
                },
            ))),
            other => Err(ReviewFixRunnerError::Unexpected(diagnostic(format!(
                "unsupported review-fix-lead provider '{other}' (supported: 'codex', 'claude')"
            )))),
        }
    }
}

fn diagnostic(value: impl Into<String>) -> DiagnosticText {
    let value = value.into();
    DiagnosticText::new(if value.trim().is_empty() {
        "review-fix diagnostic detail unavailable".to_owned()
    } else {
        value
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::{Arc, Mutex, OnceLock};

    use usecase::capability_exec::ModelName;
    use usecase::review_v2::ReviewRoundType;
    use usecase::review_v2::run_review_fix::{
        ReviewFixResolution, ReviewFixRunner, ReviewFixRunnerError, ReviewFixTrackResolverPort,
        ReviewTrackId, RunReviewFixCommand, RunReviewFixInteractor, RunReviewFixOutput,
        RunReviewFixRequest, RunReviewFixService,
    };

    use super::{GitReviewFixTrackResolver, ReviewFixRunnerAdapter};

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn git_success(root: &Path, arguments: &[&str]) {
        let output = Command::new("git")
            .args(arguments)
            .current_dir(root)
            .output()
            .expect("git must start for the fixture");
        assert!(output.status.success(), "git {} failed", arguments.join(" "));
    }

    #[test]
    fn test_git_review_fix_track_resolver_resolves_active_track_branch() {
        let dir = tempfile::tempdir().expect("temporary repository");
        git_success(dir.path(), &["init", "-b", "main"]);
        git_success(dir.path(), &["config", "user.email", "test@example.invalid"]);
        git_success(dir.path(), &["config", "user.name", "Test User"]);
        fs::create_dir_all(dir.path().join("track/items")).expect("items directory");
        fs::write(dir.path().join("README.md"), "fixture\n").expect("fixture file");
        git_success(dir.path(), &["add", "."]);
        git_success(dir.path(), &["commit", "-m", "fixture"]);
        git_success(dir.path(), &["checkout", "-b", "track/review-fix-resolution-2026"]);

        let resolved = GitReviewFixTrackResolver
            .resolve_current_track(&dir.path().join("track/items"))
            .expect("a track branch must resolve from the items directory");

        assert_eq!(resolved.track_id().as_str(), "review-fix-resolution-2026");
        assert_eq!(
            resolved.repository_root(),
            dir.path().canonicalize().expect("fixture repository root must canonicalize")
        );
    }

    #[test]
    fn test_git_review_fix_track_resolver_rejects_non_track_branch() {
        let directory = tempfile::tempdir().expect("temporary repository");
        git_success(directory.path(), &["init", "-b", "main"]);
        let items_dir = directory.path().join("track/items");
        fs::create_dir_all(&items_dir).expect("items directory");
        git_success(directory.path(), &["config", "user.email", "test@example.invalid"]);
        git_success(directory.path(), &["config", "user.name", "Test User"]);
        fs::write(directory.path().join("README.md"), "fixture\n").expect("fixture file");
        git_success(directory.path(), &["add", "."]);
        git_success(directory.path(), &["commit", "-m", "fixture"]);

        let error = GitReviewFixTrackResolver
            .resolve_current_track(&items_dir)
            .expect_err("a non-track branch must be rejected");

        assert!(matches!(
            error,
            usecase::review_v2::run_review_fix::ReviewFixTrackResolveError::NonTrackBranch(_)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_git_review_fix_track_resolver_rejects_ambient_git_dir_after_isolation_failure() {
        let requested = tempfile::tempdir().expect("requested repository");
        git_success(requested.path(), &["init", "-b", "main"]);
        let real_items_dir = requested.path().join("real-items");
        fs::create_dir_all(&real_items_dir).expect("real items directory");
        fs::create_dir_all(requested.path().join("track")).expect("track directory");
        std::os::unix::fs::symlink(&real_items_dir, requested.path().join("track/items"))
            .expect("items symlink");

        let elsewhere = tempfile::tempdir().expect("ambient repository");
        git_success(elsewhere.path(), &["init", "-b", "track/ambient-repository-2026"]);

        let status = Command::new(std::env::current_exe().expect("current test executable"))
            .args([
                "--exact",
                "review_v2::review_fix_adapter::tests::test_git_review_fix_track_resolver_rejects_ambient_git_dir_after_isolation_failure_subprocess",
            ])
            .env("GIT_DIR", elsewhere.path().join(".git"))
            .env("SOTP_TEST_REVIEW_FIX_ITEMS_DIR", requested.path().join("track/items"))
            .status()
            .expect("isolated child test must start");

        assert!(status.success(), "the resolver must reject the isolated discovery failure");
    }

    #[cfg(unix)]
    #[test]
    fn test_git_review_fix_track_resolver_rejects_ambient_git_dir_after_isolation_failure_subprocess()
     {
        let Some(items_dir) = std::env::var_os("SOTP_TEST_REVIEW_FIX_ITEMS_DIR") else {
            return;
        };

        assert!(matches!(
            GitReviewFixTrackResolver.resolve_current_track(Path::new(&items_dir)),
            Err(usecase::review_v2::run_review_fix::ReviewFixTrackResolveError::BranchReadFailed(
                _
            ))
        ));
    }

    fn claude_adapter_fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temporary repository");
        git_success(directory.path(), &["init", "-b", "main"]);
        fs::create_dir_all(directory.path().join(".harness/config")).expect("config directory");
        fs::write(
            directory.path().join(".harness/config/agent-profiles.json"),
            r#"{"schema_version":1,"providers":{"claude":{"label":"Claude"}},"capabilities":{"review-fix-lead":{"provider":"claude","model":"claude-test","fast_provider":"claude","fast_model":"claude-fast","reasoning_effort":"high","fast_reasoning_effort":"low","execution_mode":"typed-pipeline"}}}"#,
        )
        .expect("agent profiles");
        directory
    }

    fn claude_command(repository_root: &Path, briefing_file: PathBuf) -> RunReviewFixCommand {
        RunReviewFixCommand::new_resolved(
            usecase::review_v2::ReviewScopeName::try_new("cli".to_owned()).expect("valid scope"),
            briefing_file,
            ReviewFixResolution::new(
                ReviewTrackId::try_new("review-fix-adapter-2026".to_owned())
                    .expect("valid track ID"),
                repository_root.to_path_buf(),
            ),
            ReviewRoundType::Fast,
            Some(ModelName::try_new("claude-override").expect("valid model")),
        )
    }

    #[test]
    fn test_review_fix_runner_adapter_dispatches_claude_through_runner_port() {
        let _lock = cwd_lock().lock().expect("test mutex");
        let directory = claude_adapter_fixture();
        fs::write(directory.path().join("briefing.md"), "# Briefing\n")
            .expect("trusted briefing file");
        let unrelated_directory = tempfile::tempdir().expect("unrelated working directory");
        let original = std::env::current_dir().expect("current directory");
        std::env::set_current_dir(unrelated_directory.path())
            .expect("unrelated working directory must be usable");

        let runner: &dyn ReviewFixRunner = &ReviewFixRunnerAdapter;
        let result = runner.run_fix(claude_command(directory.path(), PathBuf::from("briefing.md")));

        std::env::set_current_dir(original).expect("restore current directory");
        assert!(matches!(
            result,
            Err(ReviewFixRunnerError::SubagentDispatchRequired(instruction))
                if instruction.model.as_str() == "claude-override"
                    && instruction.round_type == ReviewRoundType::Fast
                    && instruction.repository_root == directory.path()
        ));
    }

    #[test]
    fn test_review_fix_runner_adapter_claude_rejects_absolute_briefing_path() {
        let directory = claude_adapter_fixture();
        let outside = tempfile::NamedTempFile::new().expect("outside briefing");

        let result = ReviewFixRunnerAdapter
            .run_fix(claude_command(directory.path(), outside.path().to_path_buf()));
        let error = match result {
            Err(error) => error,
            Ok(_) => {
                panic!("absolute briefing must be rejected before Claude dispatch");
            }
        };

        assert!(error.to_string().contains("relative path beneath the repository root"));
    }

    #[test]
    fn test_review_fix_runner_adapter_claude_rejects_traversal_briefing_path() {
        let directory = claude_adapter_fixture();

        let result = ReviewFixRunnerAdapter
            .run_fix(claude_command(directory.path(), PathBuf::from("../outside.md")));
        let error = match result {
            Err(error) => error,
            Ok(_) => {
                panic!("traversal briefing must be rejected before Claude dispatch");
            }
        };

        assert!(error.to_string().contains("relative path beneath the repository root"));
    }

    #[cfg(unix)]
    #[test]
    fn test_review_fix_runner_adapter_claude_rejects_symlinked_briefing_path() {
        let directory = claude_adapter_fixture();
        let outside = tempfile::NamedTempFile::new().expect("outside briefing");
        std::os::unix::fs::symlink(outside.path(), directory.path().join("briefing.md"))
            .expect("briefing symlink");

        let result = ReviewFixRunnerAdapter
            .run_fix(claude_command(directory.path(), PathBuf::from("briefing.md")));
        let error = match result {
            Err(error) => error,
            Ok(_) => {
                panic!("symlinked briefing must be rejected before Claude dispatch");
            }
        };

        assert!(error.to_string().contains("not trusted"));
    }

    #[test]
    fn test_review_fix_runner_adapter_claude_rejects_over_bound_briefing_path() {
        let directory = claude_adapter_fixture();
        let briefing = fs::File::create(directory.path().join("briefing.md"))
            .expect("over-bound briefing fixture");
        briefing.set_len(64 * 1024 + 1).expect("set briefing size");

        let result = ReviewFixRunnerAdapter
            .run_fix(claude_command(directory.path(), PathBuf::from("briefing.md")));
        let error = match result {
            Err(error) => error,
            Ok(_) => {
                panic!("over-bound briefing must be rejected before Claude dispatch");
            }
        };

        assert!(error.to_string().contains("larger than the configured bound"));
    }

    #[test]
    fn test_review_fix_runner_adapter_rejects_resolver_root_that_is_not_repository_root() {
        let directory = tempfile::tempdir().expect("temporary repository");
        git_success(directory.path(), &["init", "-b", "main"]);
        let nested_root = directory.path().join("nested");
        fs::create_dir_all(&nested_root).expect("nested directory");

        let result = ReviewFixRunnerAdapter.run_fix(RunReviewFixCommand::new_resolved(
            usecase::review_v2::ReviewScopeName::try_new("cli".to_owned()).expect("valid scope"),
            PathBuf::from("briefing.md"),
            ReviewFixResolution::new(
                ReviewTrackId::try_new("review-fix-root-mismatch-2026".to_owned())
                    .expect("valid track ID"),
                nested_root,
            ),
            ReviewRoundType::Fast,
            None,
        ));

        assert!(matches!(
            result,
            Err(ReviewFixRunnerError::Unexpected(detail))
                if detail.as_str().contains("does not match the runner repository")
        ));
    }

    #[cfg(any())]
    #[test]
    fn test_review_fix_runner_adapter_claude_rejects_traversal_briefing() {
        let directory = tempfile::tempdir().expect("temporary repository");
        git_success(directory.path(), &["init", "-b", "main"]);
        fs::create_dir_all(directory.path().join(".harness/config")).expect("config directory");
        fs::write(
            directory.path().join(".harness/config/agent-profiles.json"),
            r#"{"schema_version":1,"providers":{"claude":{"label":"Claude"}},"capabilities":{"review-fix-lead":{"provider":"claude","model":"claude-test","fast_provider":"claude","fast_model":"claude-fast","reasoning_effort":"high","fast_reasoning_effort":"low","execution_mode":"typed-pipeline"}}}"#,
        )
        .expect("agent profiles");

        let result = TrustedReviewFixBriefingLoader
            .load_briefing_content(directory.path(), Path::new("../outside.md"));

        assert!(matches!(
            result,
            Err(usecase::review_v2::run_review_fix::ReviewFixBriefingLoadError::UntrustedFile(detail))
                if detail.as_str().contains("briefing_file must be a relative path")
        ));
    }

    #[cfg(any())]
    #[test]
    fn test_trusted_review_fix_briefing_loader_returns_read_failed_for_missing_file() {
        let directory = tempfile::tempdir().expect("temporary repository");

        let result = TrustedReviewFixBriefingLoader
            .load_briefing_content(directory.path(), Path::new("missing.md"));

        assert!(matches!(
            result,
            Err(usecase::review_v2::run_review_fix::ReviewFixBriefingLoadError::ReadFailed(detail))
                if detail.as_str().contains("does not exist")
        ));
    }

    #[cfg(any())]
    #[test]
    fn test_trusted_review_fix_briefing_loader_valid_relative_file_returns_validated_content() {
        let directory = tempfile::tempdir().expect("temporary repository");
        let expected = "# Valid review-fix briefing\n\nUse the trusted delivery boundary.\n";
        fs::write(directory.path().join("briefing.md"), expected).expect("briefing fixture");

        let content = TrustedReviewFixBriefingLoader
            .load_briefing_content(directory.path(), Path::new("briefing.md"))
            .expect("a trusted, bounded briefing must load");

        assert_eq!(content.as_str(), expected);
        assert!(content.as_str().len() <= 64 * 1024);
    }

    #[cfg(any())]
    #[test]
    fn test_trusted_review_fix_briefing_loader_content_over_domain_bound_returns_invalid_content() {
        let directory = tempfile::tempdir().expect("temporary repository");
        fs::write(directory.path().join("briefing.md"), "x".repeat(64 * 1024 + 1))
            .expect("over-bound briefing fixture");

        let result = TrustedReviewFixBriefingLoader
            .load_briefing_content(directory.path(), Path::new("briefing.md"));

        assert!(matches!(
            result,
            Err(usecase::review_v2::run_review_fix::ReviewFixBriefingLoadError::InvalidContent(
                usecase::review_v2::run_review_fix::SubagentBriefingContentValidationError::ExceedsMaximumBytes
            ))
        ));
    }

    #[cfg(any())]
    #[test]
    fn test_trusted_review_fix_briefing_loader_rejects_non_regular_target_as_untrusted() {
        let directory = tempfile::tempdir().expect("temporary repository");
        fs::create_dir(directory.path().join("briefing.md")).expect("non-regular briefing fixture");

        let result = TrustedReviewFixBriefingLoader
            .load_briefing_content(directory.path(), Path::new("briefing.md"));

        assert!(matches!(
            result,
            Err(usecase::review_v2::run_review_fix::ReviewFixBriefingLoadError::UntrustedFile(detail))
                if detail.as_str().contains("must be a regular file")
        ));
    }

    #[cfg(any())]
    #[cfg(unix)]
    #[test]
    fn test_review_fix_runner_adapter_claude_rejects_symlinked_briefing() {
        let directory = tempfile::tempdir().expect("temporary repository");
        let outside = tempfile::tempdir().expect("outside fixture");
        git_success(directory.path(), &["init", "-b", "main"]);
        fs::create_dir_all(directory.path().join(".harness/config")).expect("config directory");
        fs::write(
            directory.path().join(".harness/config/agent-profiles.json"),
            r#"{"schema_version":1,"providers":{"claude":{"label":"Claude"}},"capabilities":{"review-fix-lead":{"provider":"claude","model":"claude-test","fast_provider":"claude","fast_model":"claude-fast","reasoning_effort":"high","fast_reasoning_effort":"low","execution_mode":"typed-pipeline"}}}"#,
        )
        .expect("agent profiles");
        fs::write(outside.path().join("briefing.md"), "outside").expect("outside briefing");
        std::os::unix::fs::symlink(
            outside.path().join("briefing.md"),
            directory.path().join("briefing.md"),
        )
        .expect("briefing symlink");

        let result = TrustedReviewFixBriefingLoader
            .load_briefing_content(directory.path(), Path::new("briefing.md"));

        assert!(matches!(
            result,
            Err(usecase::review_v2::run_review_fix::ReviewFixBriefingLoadError::UntrustedFile(_))
        ));
    }

    #[cfg(any())]
    #[cfg(unix)]
    #[test]
    fn test_trusted_review_fix_briefing_loader_rejects_symlinked_intermediate_component() {
        let directory = tempfile::tempdir().expect("temporary repository");
        let real_directory = directory.path().join("real-briefing-directory");
        fs::create_dir(&real_directory).expect("real briefing directory");
        fs::write(real_directory.join("briefing.md"), "trusted content").expect("briefing fixture");
        std::os::unix::fs::symlink(&real_directory, directory.path().join("linked-directory"))
            .expect("intermediate directory symlink");

        let result = TrustedReviewFixBriefingLoader
            .load_briefing_content(directory.path(), Path::new("linked-directory/briefing.md"));

        assert!(matches!(
            result,
            Err(usecase::review_v2::run_review_fix::ReviewFixBriefingLoadError::UntrustedFile(detail))
                if detail.as_str().contains("symlink")
        ));
    }

    #[test]
    fn test_git_review_fix_track_resolver_participates_in_interactor_delivery_flow() {
        struct CompletedRunner {
            expected_repository_root: PathBuf,
        }

        impl ReviewFixRunner for CompletedRunner {
            fn run_fix(
                &self,
                command: RunReviewFixCommand,
            ) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
                assert_eq!(command.track_id(), "review-fix-delivery-2026");
                assert_eq!(command.repository_root(), self.expected_repository_root);
                Ok(RunReviewFixOutput {
                    status: "completed".to_owned(),
                    exit_code: 0,
                    stderr: None,
                })
            }
        }

        let directory = tempfile::tempdir().expect("temporary repository");
        git_success(directory.path(), &["init", "-b", "main"]);
        git_success(directory.path(), &["config", "user.email", "test@example.invalid"]);
        git_success(directory.path(), &["config", "user.name", "Test User"]);
        fs::create_dir_all(directory.path().join("track/items")).expect("items directory");
        fs::write(directory.path().join("briefing.md"), "# Briefing\n").expect("briefing file");
        fs::write(directory.path().join("README.md"), "fixture\n").expect("fixture file");
        git_success(directory.path(), &["add", "."]);
        git_success(directory.path(), &["commit", "-m", "fixture"]);
        git_success(directory.path(), &["checkout", "-b", "track/review-fix-delivery-2026"]);

        let service: Arc<dyn RunReviewFixService> = Arc::new(RunReviewFixInteractor::new(
            Arc::new(GitReviewFixTrackResolver),
            Arc::new(CompletedRunner {
                expected_repository_root: directory
                    .path()
                    .canonicalize()
                    .expect("fixture repository root must canonicalize"),
            }),
        ));
        let output = service
            .run(
                RunReviewFixRequest::try_new(
                    "cli".to_owned(),
                    Path::new("briefing.md").to_path_buf(),
                    None,
                    directory.path().join("track/items"),
                    "fast".to_owned(),
                    None,
                )
                .expect("valid delivery request"),
            )
            .expect("resolved branch must invoke the runner");

        assert_eq!(output.status, "completed");
    }
}
