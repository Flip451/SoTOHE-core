//! Secondary adapters for the typed `review results` projection ports.

use std::path::PathBuf;

use domain::TrackId;
use domain::review_v2::{ReviewExistsPort as _, ReviewReader as _, ReviewState, ScopeName};
use usecase::git_workflow::DiagnosticText;
use usecase::review_v2::{
    NonEmptyReviewerFindingsOutput, ReviewCheckZeroFindingsStatePort, ReviewNotRequiredReason,
    ReviewRequiredReason, ReviewResultsError, ReviewResultsRoundPort, ReviewResultsScopePort,
    ReviewResultsScopeSnapshot, ReviewResultsStatePort, ReviewRoundResultOutput,
    ReviewRoundResultVerdict, ReviewScopeName, ReviewStoredRound, ReviewStoredScopeState,
    ReviewStoredScopeStateEntry, ReviewerFindingOutput,
};

use super::{
    FsReviewStore, ReviewCheckZeroFindingsStateAdapter, load_v2_scope_config,
    resolve_review_diff_base,
};

pub struct ResultsScopeAdapter;
pub struct ResultsStateAdapter;
pub struct ResultsRoundAdapter;

fn resolve_track(
    track_id: Option<&str>,
    items_dir: &std::path::Path,
) -> Result<TrackId, ReviewResultsError> {
    match track_id {
        Some(track_id) => TrackId::try_new(track_id.to_owned())
            .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string()))),
        None => {
            let (git, _) =
                crate::discover_isolated_repo_for_items_dir(items_dir).map_err(|error| {
                    ReviewResultsError::Failed(DiagnosticText::new(error.to_string()))
                })?;
            let output = crate::git_cli::isolated_bounded_git_output(
                git.root(),
                &["rev-parse", "--abbrev-ref", "HEAD"],
                4096,
            )
            .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string())))?;
            if !output.status.success() {
                return Err(ReviewResultsError::Failed(DiagnosticText::new(format!(
                    "git rev-parse --abbrev-ref HEAD failed with exit status {}",
                    output.status.code().unwrap_or(-1)
                ))));
            }
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let track_id = branch.strip_prefix("track/").ok_or_else(|| {
                ReviewResultsError::Failed(DiagnosticText::new(format!(
                    "current branch '{branch}' is not a track branch"
                )))
            })?;
            TrackId::try_new(track_id)
                .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string())))
        }
    }
}

fn repository_paths(
    track_id: &TrackId,
    items_dir: &std::path::Path,
) -> Result<(PathBuf, PathBuf), ReviewResultsError> {
    let (git, canonical_items_dir) = crate::discover_isolated_repo_for_items_dir(items_dir)
        .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string())))?;
    let root = git
        .root()
        .canonicalize()
        .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string())))?;
    if !canonical_items_dir.starts_with(&root) {
        return Err(ReviewResultsError::Failed(DiagnosticText::new(
            "items directory is outside the repository root",
        )));
    }
    let track_dir = canonical_items_dir.join(track_id.as_ref());
    if !track_dir.is_dir() {
        return Err(ReviewResultsError::Failed(DiagnosticText::new(format!(
            "track directory '{}' does not exist",
            track_dir.display()
        ))));
    }
    Ok((root, track_dir))
}

fn finding_output(finding: domain::review_v2::ReviewerFinding) -> ReviewerFindingOutput {
    ReviewerFindingOutput {
        message: DiagnosticText::new(finding.message().to_owned()),
        severity: finding.severity().map(ToOwned::to_owned),
        file: finding.file().map(ToOwned::to_owned),
        line: finding.line(),
        category: finding.category().map(ToOwned::to_owned),
    }
}

fn findings_output(
    findings: domain::review_v2::NonEmptyReviewerFindings,
) -> Result<NonEmptyReviewerFindingsOutput, ReviewResultsError> {
    NonEmptyReviewerFindingsOutput::try_new(
        findings.as_slice().iter().cloned().map(finding_output).collect(),
    )
    .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string())))
}

fn review_scope_name(scope: &ScopeName) -> Result<ReviewScopeName, ReviewResultsError> {
    match scope {
        ScopeName::Main(scope) => ReviewScopeName::try_new(scope.as_str().to_owned())
            .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string()))),
        ScopeName::Other => Ok(ReviewScopeName::other()),
    }
}

impl ReviewResultsScopePort for ResultsScopeAdapter {
    fn load_scope_snapshot(
        &self,
        track_id: Option<&str>,
        items_dir: &std::path::Path,
    ) -> Result<ReviewResultsScopeSnapshot, ReviewResultsError> {
        let track_id = resolve_track(track_id, items_dir)?;
        let (root, track_dir) = repository_paths(&track_id, items_dir)?;
        let config =
            load_v2_scope_config(&root.join(".harness/config/review-scope.json"), &track_id, &root)
                .map_err(|error| {
                    ReviewResultsError::Failed(DiagnosticText::new(error.to_string()))
                })?;
        let configured_scopes = config
            .all_scope_names()
            .into_iter()
            .map(|scope| review_scope_name(&scope))
            .collect::<Result<Vec<_>, _>>()?;
        let review_json_path = track_dir.join("review.json");
        crate::track::symlink_guard::reject_symlinks_below(&review_json_path, &root)
            .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string())))?;
        let review_store = FsReviewStore::new(review_json_path, root);
        let review_json_exists = review_store
            .review_json_exists()
            .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string())))?;
        let base = resolve_review_diff_base(&track_id, items_dir)
            .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string())))?;
        Ok(ReviewResultsScopeSnapshot {
            base: base.as_ref().to_owned(),
            configured_scopes,
            review_json_exists,
        })
    }
}

impl ReviewResultsStatePort for ResultsStateAdapter {
    fn load_scope_states(
        &self,
        track_id: Option<&str>,
        items_dir: &std::path::Path,
    ) -> Result<Vec<ReviewStoredScopeStateEntry>, ReviewResultsError> {
        let track_id = resolve_track(track_id, items_dir)?;
        let (root, _) = repository_paths(&track_id, items_dir)?;
        let config =
            load_v2_scope_config(&root.join(".harness/config/review-scope.json"), &track_id, &root)
                .map_err(|error| {
                    ReviewResultsError::Failed(DiagnosticText::new(error.to_string()))
                })?;
        let port = ReviewCheckZeroFindingsStateAdapter;
        config
            .all_scope_names()
            .into_iter()
            .map(|domain_scope| {
                let scope = review_scope_name(&domain_scope)?;
                let state = port
                    .state_for(&track_id, items_dir, &domain_scope)
                    .map_err(|error| {
                        ReviewResultsError::Failed(DiagnosticText::new(error.to_string()))
                    })?
                    .unwrap_or(ReviewState::Required(
                        domain::review_v2::RequiredReason::NotStarted,
                    ));
                let state = match state {
                    ReviewState::Required(domain::review_v2::RequiredReason::NotStarted) => {
                        ReviewStoredScopeState::Required(ReviewRequiredReason::NotStarted)
                    }
                    ReviewState::Required(domain::review_v2::RequiredReason::FindingsRemain) => {
                        ReviewStoredScopeState::Required(ReviewRequiredReason::FindingsRemain)
                    }
                    ReviewState::Required(domain::review_v2::RequiredReason::StaleHash) => {
                        ReviewStoredScopeState::Required(ReviewRequiredReason::StaleHash)
                    }
                    ReviewState::NotRequired(domain::review_v2::NotRequiredReason::Empty) => {
                        ReviewStoredScopeState::NotRequired(ReviewNotRequiredReason::Empty)
                    }
                    ReviewState::NotRequired(
                        domain::review_v2::NotRequiredReason::ZeroFindings,
                    ) => ReviewStoredScopeState::NotRequired(ReviewNotRequiredReason::ZeroFindings),
                };
                Ok(ReviewStoredScopeStateEntry { scope, state })
            })
            .collect()
    }
}

impl ReviewResultsRoundPort for ResultsRoundAdapter {
    fn load_scope_rounds(
        &self,
        track_id: Option<&str>,
        items_dir: &std::path::Path,
        scope: &ReviewScopeName,
    ) -> Result<Vec<ReviewStoredRound>, ReviewResultsError> {
        let track_id = resolve_track(track_id, items_dir)?;
        let (root, track_dir) = repository_paths(&track_id, items_dir)?;
        let review_store = FsReviewStore::new(track_dir.join("review.json"), root);
        let domain_scope = domain::review_v2::ScopeName::parse(scope.as_str())
            .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string())))?;
        review_store
            .read_all_rounds(&domain_scope)
            .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string())))?
            .into_iter()
            .map(|round| {
                Ok(ReviewRoundResultOutput {
                    round_type: match round.round_type {
                        domain::review_v2::RoundType::Fast => {
                            usecase::review_v2::ReviewRoundType::Fast
                        }
                        domain::review_v2::RoundType::Final => {
                            usecase::review_v2::ReviewRoundType::Final
                        }
                    },
                    at: round.at,
                    verdict: match round.verdict {
                        domain::review_v2::Verdict::ZeroFindings => {
                            ReviewRoundResultVerdict::ZeroFindings
                        }
                        domain::review_v2::Verdict::FindingsRemain(findings) => {
                            ReviewRoundResultVerdict::FindingsRemain(findings_output(findings)?)
                        }
                    },
                })
            })
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::fs;
    use std::process::Command;

    use usecase::review_v2::{
        ReviewRequiredReason, ReviewResultsError, ReviewResultsRoundPort, ReviewResultsScopePort,
        ReviewResultsStatePort, ReviewRoundResultVerdict, ReviewRoundType, ReviewScopeName,
        ReviewStoredScopeState,
    };

    use super::{ResultsRoundAdapter, ResultsScopeAdapter, ResultsStateAdapter};

    fn git_success(root: &std::path::Path, arguments: &[&str]) {
        let output = Command::new("git")
            .args(arguments)
            .current_dir(root)
            .output()
            .expect("git must start for the fixture");
        assert!(output.status.success(), "git {} failed", arguments.join(" "));
    }

    fn git_stdout(root: &std::path::Path, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .args(arguments)
            .current_dir(root)
            .output()
            .expect("git must start for the fixture");
        assert!(output.status.success(), "git {} failed", arguments.join(" "));
        String::from_utf8(output.stdout).expect("git output must be UTF-8").trim().to_owned()
    }

    #[test]
    fn test_results_scope_adapter_loads_configured_snapshot_from_real_fixture() {
        let directory = tempfile::tempdir().expect("temporary repository");
        git_success(directory.path(), &["init", "-b", "main"]);
        let items_dir = directory.path().join("track/items");
        let track_id = "results-snapshot-2026";
        fs::create_dir_all(items_dir.join(track_id)).expect("track directory");
        fs::create_dir_all(directory.path().join(".harness/config")).expect("config directory");
        fs::write(
            directory.path().join(".harness/config/review-scope.json"),
            r#"{"version":2,"groups":{"cli":{"patterns":["apps/cli/**"]},"infra":{"patterns":["libs/infrastructure/**"]}}}"#,
        )
        .expect("scope config");
        fs::write(
            items_dir.join(track_id).join("review.json"),
            "{\"schema_version\":2,\"scopes\":{}}",
        )
        .expect("review state");
        fs::write(
            items_dir.join(track_id).join("metadata.json"),
            format!(
                r#"{{"schema_version":6,"id":"{track_id}","title":"Results fixture","created_at":"2026-08-10T00:00:00Z","updated_at":"2026-08-10T00:00:00Z","branch_strategy_snapshot":{{"base_branch":"release/2026.08","merge_target":"main","merge_method":"merge"}}}}"#
            ),
        )
        .expect("track metadata");
        git_success(directory.path(), &["config", "user.email", "results@example.invalid"]);
        git_success(directory.path(), &["config", "user.name", "Results Fixture"]);
        git_success(directory.path(), &["add", "."]);
        git_success(directory.path(), &["commit", "-m", "initial results fixture"]);
        let pinned_base = git_stdout(directory.path(), &["rev-parse", "HEAD"]);
        fs::write(directory.path().join("fixture-change.txt"), "head moved after review base\n")
            .expect("second fixture change");
        git_success(directory.path(), &["add", "fixture-change.txt"]);
        git_success(directory.path(), &["commit", "-m", "advance fixture head"]);
        fs::write(items_dir.join(track_id).join(".commit_hash"), format!("{pinned_base}\n"))
            .expect("pinned review base");

        let snapshot = ResultsScopeAdapter
            .load_scope_snapshot(Some(track_id), &items_dir)
            .expect("real configuration must produce a snapshot");

        assert_eq!(
            snapshot.base, pinned_base,
            "results must display the exact pinned commit used to derive review state"
        );
        let mut scopes =
            snapshot.configured_scopes.iter().map(|scope| scope.as_str()).collect::<Vec<_>>();
        scopes.sort_unstable();
        assert_eq!(
            scopes,
            vec!["cli", "infra", "other"],
            "the snapshot must preserve the configured scope universe used by results output"
        );
        assert!(snapshot.review_json_exists);
    }

    #[test]
    fn test_results_adapters_propagate_missing_track_directory_errors() {
        let directory = tempfile::tempdir().expect("temporary repository");
        git_success(directory.path(), &["init", "-b", "main"]);
        let items_dir = directory.path().join("track/items");
        fs::create_dir_all(&items_dir).expect("items directory");
        let track_id = "missing-results-track-2026";
        let scope = ReviewScopeName::try_new("cli".to_owned()).expect("valid scope name");

        let scope_error = ResultsScopeAdapter
            .load_scope_snapshot(Some(track_id), &items_dir)
            .expect_err("a missing track directory must fail the scope adapter");
        let state_error = ResultsStateAdapter
            .load_scope_states(Some(track_id), &items_dir)
            .expect_err("a missing track directory must fail the state adapter");
        let round_error = ResultsRoundAdapter
            .load_scope_rounds(Some(track_id), &items_dir, &scope)
            .expect_err("a missing track directory must fail the round adapter");

        for error in [scope_error, state_error, round_error] {
            assert!(error.to_string().contains("track directory"));
            assert!(matches!(error, ReviewResultsError::Failed(_)));
        }
    }

    #[test]
    fn test_results_state_adapter_with_persisted_findings_projects_scope_states() {
        let directory = tempfile::tempdir().expect("temporary repository");
        git_success(directory.path(), &["init", "-b", "main"]);
        let items_dir = directory.path().join("track/items");
        let track_id = "results-states-2026";
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).expect("track directory");
        fs::create_dir_all(directory.path().join(".harness/config")).expect("config directory");
        fs::write(
            directory.path().join(".harness/config/review-scope.json"),
            r#"{"version":2,"groups":{"cli":{"patterns":["apps/cli/**"]}}}"#,
        )
        .expect("scope config");
        fs::write(
            track_dir.join("review.json"),
            r#"{"schema_version":2,"scopes":{"cli":{"rounds":[{"type":"final","verdict":"findings_remain","findings":[{"message":"missing happy path","severity":"P1","file":"apps/cli/src/lib.rs","line":12,"category":"test_coverage"}],"hash":"rvw1:sha256:abcdef0123456789","at":"2026-08-11T00:00:00Z"}]}}}"#,
        )
        .expect("review state");
        fs::write(
            track_dir.join("metadata.json"),
            format!(
                r#"{{"schema_version":6,"id":"{track_id}","title":"Results state fixture","created_at":"2026-08-10T00:00:00Z","updated_at":"2026-08-10T00:00:00Z","branch_strategy_snapshot":{{"base_branch":"main","merge_target":"main","merge_method":"merge"}}}}"#
            ),
        )
        .expect("track metadata");
        git_success(directory.path(), &["config", "user.email", "results@example.invalid"]);
        git_success(directory.path(), &["config", "user.name", "Results Fixture"]);
        git_success(directory.path(), &["add", "."]);
        git_success(directory.path(), &["commit", "-m", "initial results state fixture"]);
        let pinned_base = git_stdout(directory.path(), &["rev-parse", "HEAD"]);
        fs::create_dir_all(directory.path().join("apps/cli")).expect("cli fixture directory");
        fs::write(
            directory.path().join("apps/cli/review_fixture.rs"),
            "pub fn changed_after_review_base() {}\n",
        )
        .expect("changed review input");
        fs::write(
            directory.path().join("unmatched-review-input.txt"),
            "this input belongs to the implicit other scope\n",
        )
        .expect("unmatched review input");
        git_success(
            directory.path(),
            &["add", "apps/cli/review_fixture.rs", "unmatched-review-input.txt"],
        );
        git_success(directory.path(), &["commit", "-m", "add reviewed fixture inputs"]);
        fs::write(track_dir.join(".commit_hash"), format!("{pinned_base}\n"))
            .expect("pinned review base");

        let states = ResultsStateAdapter
            .load_scope_states(Some(track_id), &items_dir)
            .expect("persisted review state must project through the results adapter");

        assert!(states.iter().any(|entry| {
            entry.scope.as_str() == "cli"
                && entry.state
                    == ReviewStoredScopeState::Required(ReviewRequiredReason::FindingsRemain)
        }));
        assert!(states.iter().any(|entry| {
            entry.scope.as_str() == "other"
                && entry.state == ReviewStoredScopeState::Required(ReviewRequiredReason::NotStarted)
        }));
    }

    #[test]
    fn test_results_round_adapter_with_persisted_rounds_projects_all_round_metadata() {
        let directory = tempfile::tempdir().expect("temporary repository");
        git_success(directory.path(), &["init", "-b", "main"]);
        let items_dir = directory.path().join("track/items");
        let track_id = "results-rounds-2026";
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).expect("track directory");
        fs::write(
            track_dir.join("review.json"),
            r#"{"schema_version":2,"scopes":{"cli":{"rounds":[{"type":"fast","verdict":"zero_findings","findings":[],"hash":"rvw1:sha256:abcdef0123456789","at":"2026-08-11T01:00:00Z"},{"type":"final","verdict":"findings_remain","findings":[{"message":"missing happy path","severity":"P1","file":"apps/cli/src/lib.rs","line":12,"category":"test_coverage"}],"hash":"rvw1:sha256:abcdef0123456789","at":"2026-08-11T02:00:00Z"}]}}}"#,
        )
        .expect("review state");
        let scope = ReviewScopeName::try_new("cli".to_owned()).expect("valid scope name");

        let rounds = ResultsRoundAdapter
            .load_scope_rounds(Some(track_id), &items_dir, &scope)
            .expect("persisted review rounds must project through the results adapter");

        assert_eq!(rounds.len(), 2);
        let fast_round = rounds.first().expect("fixture must contain a fast round");
        assert_eq!(fast_round.round_type, ReviewRoundType::Fast);
        assert_eq!(fast_round.at, "2026-08-11T01:00:00Z");
        assert_eq!(fast_round.verdict, ReviewRoundResultVerdict::ZeroFindings);
        let final_round = rounds.get(1).expect("fixture must contain a final round");
        assert_eq!(final_round.round_type, ReviewRoundType::Final);
        assert_eq!(final_round.at, "2026-08-11T02:00:00Z");
        assert!(matches!(final_round.verdict, ReviewRoundResultVerdict::FindingsRemain(_)));
        let ReviewRoundResultVerdict::FindingsRemain(findings) = &final_round.verdict else {
            return;
        };
        assert_eq!(findings.as_slice().len(), 1);
        let finding =
            findings.as_slice().first().expect("findings-remain verdict must retain its finding");
        assert_eq!(finding.message.as_str(), "missing happy path");
        assert_eq!(finding.severity.as_deref(), Some("P1"));
        assert_eq!(finding.file.as_deref(), Some("apps/cli/src/lib.rs"));
        assert_eq!(finding.line, Some(12));
        assert_eq!(finding.category.as_deref(), Some("test_coverage"));
    }

    #[cfg(unix)]
    #[test]
    fn test_results_scope_adapter_rejects_symlinked_review_json() {
        let directory = tempfile::tempdir().expect("temporary repository");
        git_success(directory.path(), &["init", "-b", "main"]);
        let items_dir = directory.path().join("track/items");
        let track_id = "results-symlink-2026";
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).expect("track directory");
        fs::create_dir_all(directory.path().join(".harness/config")).expect("config directory");
        fs::write(
            directory.path().join(".harness/config/review-scope.json"),
            r#"{"version":2,"groups":{"cli":{"patterns":["apps/cli/**"]}}}"#,
        )
        .expect("scope config");
        let target = directory.path().join("review-target.json");
        fs::write(&target, r#"{"schema_version":2,"scopes":{}}"#).expect("review target");
        std::os::unix::fs::symlink(&target, track_dir.join("review.json"))
            .expect("review state symlink");

        let error = ResultsScopeAdapter
            .load_scope_snapshot(Some(track_id), &items_dir)
            .expect_err("a symlinked review state must be rejected before its existence is read");

        assert!(error.to_string().contains("refusing to follow symlink"));
    }
}
