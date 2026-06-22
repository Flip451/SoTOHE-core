// ---------------------------------------------------------------------------
// D4 port adapters for PrReviewPollingInteractor (T008)
// ---------------------------------------------------------------------------

/// Infrastructure adapter: [`usecase::pr_review_polling::PrListReviewsPort`] backed by
/// [`infrastructure::gh_cli::GhClient::list_reviews`].
///
/// Wraps any `GhClient` impl (`SystemGhClient` in production, stubs in tests)
/// so the usecase interactor has no direct dependency on `infrastructure::gh_cli`.
pub(super) struct GhListReviewsAdapter<C: infrastructure::gh_cli::GhClient>(pub(super) C);

impl<C: infrastructure::gh_cli::GhClient + Send + Sync>
    usecase::pr_review_polling::PrListReviewsPort for GhListReviewsAdapter<C>
{
    fn list_reviews(&self, repo_nwo: &str, pr: &str) -> Result<String, String> {
        self.0.list_reviews(repo_nwo, pr).map_err(|e| e.to_string())
    }
}

/// Infrastructure adapter: [`usecase::pr_review_polling::PrListReactionsPort`] backed by
/// [`infrastructure::gh_cli::GhClient::list_reactions`].
pub(super) struct GhListReactionsAdapter<C: infrastructure::gh_cli::GhClient>(pub(super) C);

impl<C: infrastructure::gh_cli::GhClient + Send + Sync>
    usecase::pr_review_polling::PrListReactionsPort for GhListReactionsAdapter<C>
{
    fn list_reactions(&self, repo_nwo: &str, pr: &str) -> Result<String, String> {
        self.0.list_reactions(repo_nwo, pr).map_err(|e| e.to_string())
    }
}

/// Infrastructure adapter: [`usecase::pr_review_polling::PrListIssueCommentsPort`] backed by
/// [`infrastructure::gh_cli::GhClient::list_issue_comments`].
pub(super) struct GhListIssueCommentsAdapter<C: infrastructure::gh_cli::GhClient>(pub(super) C);

impl<C: infrastructure::gh_cli::GhClient + Send + Sync>
    usecase::pr_review_polling::PrListIssueCommentsPort for GhListIssueCommentsAdapter<C>
{
    fn list_issue_comments(&self, repo_nwo: &str, pr: &str) -> Result<String, String> {
        self.0.list_issue_comments(repo_nwo, pr).map_err(|e| e.to_string())
    }
}

/// Build a [`usecase::pr_review_polling::PrReviewPollingInteractor`] wired with
/// `SystemGhClient` adapters and [`infrastructure::SystemSleepAdapter`].
pub(super) fn make_polling_interactor() -> usecase::pr_review_polling::PrReviewPollingInteractor {
    use infrastructure::SystemSleepAdapter;
    use infrastructure::gh_cli::SystemGhClient;
    use std::sync::Arc;
    use usecase::pr_review_polling::PrReviewPollingInteractor;

    PrReviewPollingInteractor::new(
        Arc::new(GhListReviewsAdapter(SystemGhClient)),
        Arc::new(GhListReactionsAdapter(SystemGhClient)),
        Arc::new(GhListIssueCommentsAdapter(SystemGhClient)),
        Arc::new(SystemSleepAdapter),
    )
}
