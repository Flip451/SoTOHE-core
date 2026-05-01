//! Scope routing query application service.
//!
//! Provides a `ReviewCycle`-independent driving port for the CLI to query
//! scope classification (`classify`) and per-scope diff file lists (`files`)
//! with the minimum dependency set required by ADR D6: only `ReviewScopeConfig`
//! (domain) and `DiffGetter` (usecase secondary port) — no `Reviewer`,
//! `ReviewHasher`, or `ReviewWriter` dependency.

use std::collections::{HashMap, HashSet};

use thiserror::Error;

use domain::CommitHash;
use domain::review_v2::{FilePath, MainScopeName, ReviewScopeConfig, ScopeName};

use super::error::DiffGetError;
use super::ports::DiffGetter;

// ── ScopeClassification ───────────────────────────────────────────────

/// Classification result for a single file path.
///
/// Variants are mutually exclusive: a path is either matched to one or more
/// named scopes (`Named`), falls through to the implicit `Other` scope, or
/// was filtered out by `operational` / `other_track` patterns (`Excluded`).
///
/// `Named(MainScopeName, Vec<MainScopeName>)` is a head + tail tuple so that
/// the at-least-one invariant is structurally enforced (a `Named` value
/// cannot be empty) and the variant cannot hold the implicit `Other` scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeClassification {
    Named(MainScopeName, Vec<MainScopeName>),
    Other,
    Excluded,
}

// ── PathClassification ────────────────────────────────────────────────

/// Read-only record pairing a file path with its scope classification.
///
/// Returned by `ScopeQueryService::classify` in input-argument order
/// (CN-04: order preserved).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathClassification {
    pub path: FilePath,
    pub classification: ScopeClassification,
}

// ── ScopeQueryError ───────────────────────────────────────────────────

/// Errors from `ScopeQueryService` operations.
///
/// `DiffGet` wraps `DiffGetError` from the `DiffGetter` port (used by the
/// `files` method when git diff retrieval fails).
///
/// `UnknownScope` is emitted by the `files` method when the requested scope
/// name is not in `ReviewScopeConfig`. This is reachable even though the
/// parameter type is `ScopeName`: `MainScopeName` is only format-validated
/// (non-empty, ASCII, not "other") — it is NOT validated against the
/// configured scope set, so a name like "nonexistent-group" passes type
/// checking but is absent from the config (AC-08).
#[derive(Debug, Error)]
pub enum ScopeQueryError {
    #[error("diff error: {0}")]
    DiffGet(#[from] DiffGetError),
    #[error("unknown scope: {0}")]
    UnknownScope(ScopeName),
    /// A path string could not be parsed into a valid `FilePath`.
    #[error("invalid path '{path}': {reason}")]
    InvalidPath { path: String, reason: String },
    /// A scope name string could not be parsed into a valid `ScopeName`.
    #[error("invalid scope name '{name}': {reason}")]
    InvalidScopeName { name: String, reason: String },
}

// ── String-based output types (no domain imports for callers) ─────────

/// String-only output produced by [`ScopeQueryService::classify_by_strings`].
///
/// Callers (e.g. CLI) never import `FilePath`, `MainScopeName`, or
/// `ScopeClassification` directly (CN-01 / AC-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeClassificationOutput {
    /// The repo-relative path that was classified (as a raw string).
    pub path: String,
    /// The scope classification result:
    /// - `vec!["scope1", "scope2", ...]` for named scopes (sorted alphabetically).
    /// - `vec!["other"]` for the implicit `Other` scope.
    /// - `vec!["<excluded>"]` for paths filtered by operational/other-track patterns.
    pub scopes: Vec<String>,
}

// ── ScopeQueryService ─────────────────────────────────────────────────

/// Application service (driving port) for scope routing queries.
///
/// Implemented by `ScopeQueryInteractor`. Driven by the CLI `classify` and
/// `files` subcommands. Independent of `ReviewCycle` (CN-05 / CN-06 / D6).
pub trait ScopeQueryService {
    /// Classifies the given paths into per-path scope classifications.
    ///
    /// Returns one `PathClassification` per input path in input order
    /// (CN-04). Pure logic — does not perform any I/O (CN-03), so the
    /// `Result` only exists for trait-level uniformity with `files`.
    ///
    /// # Errors
    /// Currently never returns an error (the underlying classification is
    /// pure), but the signature reserves the option for future I/O.
    fn classify(&self, paths: Vec<FilePath>) -> Result<Vec<PathClassification>, ScopeQueryError>;

    /// Returns the diff files belonging to the requested scope.
    ///
    /// Validates the scope name first via `ReviewScopeConfig::contains_scope`;
    /// returns `UnknownScope` immediately for unknown names — before any
    /// diff I/O — so an undefined scope always yields `UnknownScope`, never
    /// `DiffGet` (IN-04 / AC-08).
    ///
    /// # Errors
    /// - `UnknownScope` when the scope is not configured.
    /// - `DiffGet` when `DiffGetter::list_diff_files` fails.
    fn files(&self, scope: ScopeName) -> Result<Vec<FilePath>, ScopeQueryError>;

    /// String-accepting variant of [`Self::classify`] for callers that must not import
    /// domain types (CN-01 / AC-03).
    ///
    /// Converts each raw string to a `FilePath`, delegates to `classify`, and
    /// converts the result to `ScopeClassificationOutput`.
    ///
    /// # Errors
    /// - `InvalidPath` when any path string is rejected by `FilePath::new`.
    /// - Same as [`Self::classify`] otherwise (currently never).
    fn classify_by_strings(
        &self,
        paths: Vec<String>,
    ) -> Result<Vec<ScopeClassificationOutput>, ScopeQueryError>;

    /// String-accepting variant of [`Self::files`] for callers that must not import
    /// domain types (CN-01 / AC-03).
    ///
    /// Parses the scope name string, delegates to `files`, and converts the
    /// result to `Vec<String>`.
    ///
    /// # Errors
    /// - `InvalidScopeName` when the string is rejected by `ScopeName` parsing.
    /// - `UnknownScope` when the scope is not configured.
    /// - `DiffGet` when `DiffGetter::list_diff_files` fails.
    fn files_by_string(&self, scope: String) -> Result<Vec<String>, ScopeQueryError>;
}

// ── ScopeQueryInteractor ──────────────────────────────────────────────

/// Default implementation of `ScopeQueryService`.
///
/// Holds `scope_config` (domain), a generic `diff_getter: D` (driven port),
/// and the diff `base` commit. Does NOT depend on `Reviewer` or
/// `ReviewHasher` (CN-06).
///
/// Fields are public so the type catalogue's declared `expected_members`
/// resolve in rustdoc's public schema (the TDDD type-signal pipeline only
/// observes `pub` members). Construction normally goes through `new()`;
/// direct field access is allowed for composition-root wiring and tests.
pub struct ScopeQueryInteractor<D: DiffGetter> {
    pub scope_config: ReviewScopeConfig,
    pub diff_getter: D,
    pub base: CommitHash,
}

impl<D: DiffGetter> ScopeQueryInteractor<D> {
    #[must_use]
    pub fn new(scope_config: ReviewScopeConfig, diff_getter: D, base: CommitHash) -> Self {
        Self { scope_config, diff_getter, base }
    }
}

impl<D: DiffGetter> ScopeQueryService for ScopeQueryInteractor<D> {
    fn classify(&self, paths: Vec<FilePath>) -> Result<Vec<PathClassification>, ScopeQueryError> {
        let classified = self.scope_config.classify(&paths);

        // Invert: file → ordered list of MainScopeNames it belongs to.
        let mut path_to_named: HashMap<FilePath, Vec<MainScopeName>> = HashMap::new();
        let mut other_paths: HashSet<FilePath> = HashSet::new();
        for (scope_name, files) in &classified {
            match scope_name {
                ScopeName::Main(main_name) => {
                    for file in files {
                        path_to_named.entry(file.clone()).or_default().push(main_name.clone());
                    }
                }
                ScopeName::Other => {
                    for file in files {
                        other_paths.insert(file.clone());
                    }
                }
            }
        }

        // Sort each path's matched names alphabetically for deterministic
        // (head, tail) decomposition independent of HashMap iteration order.
        // Dedup after sorting to remove duplicates that arise when the same
        // path appears multiple times in the input (e.g. duplicate entries).
        let path_to_pair: HashMap<FilePath, (MainScopeName, Vec<MainScopeName>)> = path_to_named
            .into_iter()
            .filter_map(|(path, mut names)| {
                names.sort_by(|a, b| a.as_str().cmp(b.as_str()));
                names.dedup();
                let mut iter = names.into_iter();
                iter.next().map(|head| (path, (head, iter.collect())))
            })
            .collect();

        let result: Vec<PathClassification> = paths
            .into_iter()
            .map(|path| {
                let classification = if let Some((head, tail)) = path_to_pair.get(&path) {
                    ScopeClassification::Named(head.clone(), tail.clone())
                } else if other_paths.contains(&path) {
                    ScopeClassification::Other
                } else {
                    ScopeClassification::Excluded
                };
                PathClassification { path, classification }
            })
            .collect();

        Ok(result)
    }

    fn files(&self, scope: ScopeName) -> Result<Vec<FilePath>, ScopeQueryError> {
        if !self.scope_config.contains_scope(&scope) {
            return Err(ScopeQueryError::UnknownScope(scope));
        }
        let diff_files = self.diff_getter.list_diff_files(&self.base)?;
        let classified = self.scope_config.classify(&diff_files);
        Ok(classified.get(&scope).cloned().unwrap_or_default())
    }

    fn classify_by_strings(
        &self,
        paths: Vec<String>,
    ) -> Result<Vec<ScopeClassificationOutput>, ScopeQueryError> {
        // Convert raw strings to FilePath values, collecting errors.
        let file_paths: Vec<FilePath> = paths
            .iter()
            .map(|raw| {
                FilePath::new(raw.as_str()).map_err(|e| ScopeQueryError::InvalidPath {
                    path: raw.clone(),
                    reason: e.to_string(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Delegate to the typed classify method.
        let classified = self.classify(file_paths)?;

        // Convert PathClassification → ScopeClassificationOutput.
        Ok(classified
            .into_iter()
            .map(|pc| {
                let scopes = match pc.classification {
                    ScopeClassification::Named(head, tail) => {
                        let mut names: Vec<String> = std::iter::once(head.as_str().to_owned())
                            .chain(tail.iter().map(|n| n.as_str().to_owned()))
                            .collect();
                        names.sort();
                        names
                    }
                    ScopeClassification::Other => vec!["other".to_owned()],
                    ScopeClassification::Excluded => vec!["<excluded>".to_owned()],
                };
                ScopeClassificationOutput { path: pc.path.as_str().to_owned(), scopes }
            })
            .collect())
    }

    fn files_by_string(&self, scope: String) -> Result<Vec<String>, ScopeQueryError> {
        // Parse the scope name string into a domain ScopeName.
        let scope_name = if scope == "other" {
            ScopeName::Other
        } else {
            match MainScopeName::new(&scope) {
                Ok(main) => ScopeName::Main(main),
                Err(e) => {
                    return Err(ScopeQueryError::InvalidScopeName {
                        name: scope,
                        reason: e.to_string(),
                    });
                }
            }
        };

        // Delegate to the typed files method and convert FilePath → String.
        let file_paths = self.files(scope_name)?;
        Ok(file_paths.into_iter().map(|fp| fp.as_str().to_owned()).collect())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use domain::TrackId;

    // ── Mock DiffGetter ────────────────────────────────────────────

    struct MockDiffGetter {
        files: Vec<FilePath>,
    }

    impl MockDiffGetter {
        fn new(paths: &[&str]) -> Self {
            Self { files: paths.iter().map(|p| FilePath::new(*p).unwrap()).collect() }
        }
    }

    impl DiffGetter for MockDiffGetter {
        fn list_diff_files(&self, _base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError> {
            Ok(self.files.clone())
        }
    }

    struct FailingDiffGetter;

    impl DiffGetter for FailingDiffGetter {
        fn list_diff_files(&self, _base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError> {
            Err(DiffGetError::Failed("simulated diff failure".to_owned()))
        }
    }

    // ── Helpers ───────────────────────────────────────────────────

    fn track_id() -> TrackId {
        TrackId::try_new("test-track-2026-04-30").unwrap()
    }

    fn base_commit() -> CommitHash {
        CommitHash::try_new("abcdef1234567").unwrap()
    }

    fn config_domain_usecase() -> ReviewScopeConfig {
        ReviewScopeConfig::new(
            &track_id(),
            vec![
                ("domain".to_owned(), vec!["libs/domain/**".to_owned()], None),
                ("usecase".to_owned(), vec!["libs/usecase/**".to_owned()], None),
            ],
            vec!["track/**".to_owned()],
            vec![],
        )
        .unwrap()
    }

    fn fp(s: &str) -> FilePath {
        FilePath::new(s).unwrap()
    }

    fn main_scope(name: &str) -> MainScopeName {
        MainScopeName::new(name).unwrap()
    }

    // ── ScopeClassification construction tests ─────────────────────

    #[test]
    fn test_scope_classification_named_carries_head_and_tail() {
        let cls = ScopeClassification::Named(main_scope("domain"), vec![main_scope("usecase")]);
        match cls {
            ScopeClassification::Named(head, tail) => {
                assert_eq!(head.as_str(), "domain");
                assert_eq!(tail.len(), 1);
                assert_eq!(tail[0].as_str(), "usecase");
            }
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn test_scope_classification_named_with_empty_tail_is_valid() {
        let cls = ScopeClassification::Named(main_scope("domain"), vec![]);
        match cls {
            ScopeClassification::Named(head, tail) => {
                assert_eq!(head.as_str(), "domain");
                assert!(tail.is_empty());
            }
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn test_scope_classification_other_constructs() {
        let cls = ScopeClassification::Other;
        assert_eq!(cls, ScopeClassification::Other);
    }

    #[test]
    fn test_scope_classification_excluded_constructs() {
        let cls = ScopeClassification::Excluded;
        assert_eq!(cls, ScopeClassification::Excluded);
    }

    // ── PathClassification construction tests ──────────────────────

    #[test]
    fn test_path_classification_holds_path_and_classification() {
        let pc = PathClassification {
            path: fp("libs/domain/src/lib.rs"),
            classification: ScopeClassification::Named(main_scope("domain"), vec![]),
        };
        assert_eq!(pc.path.as_str(), "libs/domain/src/lib.rs");
        assert!(matches!(pc.classification, ScopeClassification::Named(_, _)));
    }

    // ── ScopeQueryError variant tests ──────────────────────────────

    #[test]
    fn test_scope_query_error_diffget_wraps_inner() {
        let err: ScopeQueryError = DiffGetError::Failed("boom".to_owned()).into();
        assert!(matches!(err, ScopeQueryError::DiffGet(_)));
    }

    #[test]
    fn test_scope_query_error_unknown_scope_carries_name() {
        let scope = ScopeName::Main(main_scope("nonexistent"));
        let err = ScopeQueryError::UnknownScope(scope);
        match err {
            ScopeQueryError::UnknownScope(ScopeName::Main(name)) => {
                assert_eq!(name.as_str(), "nonexistent");
            }
            _ => panic!("expected UnknownScope(Main)"),
        }
    }

    // ── ScopeQueryInteractor::classify tests ───────────────────────

    #[test]
    fn test_classify_named_path_returns_named_with_head_and_empty_tail() {
        let interactor = ScopeQueryInteractor::new(
            config_domain_usecase(),
            MockDiffGetter::new(&[]),
            base_commit(),
        );
        let result = interactor.classify(vec![fp("libs/domain/src/lib.rs")]).unwrap();
        assert_eq!(result.len(), 1);
        match &result[0].classification {
            ScopeClassification::Named(head, tail) => {
                assert_eq!(head.as_str(), "domain");
                assert!(tail.is_empty());
            }
            other => panic!("expected Named, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_other_path_returns_other() {
        let interactor = ScopeQueryInteractor::new(
            config_domain_usecase(),
            MockDiffGetter::new(&[]),
            base_commit(),
        );
        let result = interactor.classify(vec![fp("Cargo.toml")]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].classification, ScopeClassification::Other);
    }

    #[test]
    fn test_classify_operational_path_returns_excluded() {
        let interactor = ScopeQueryInteractor::new(
            config_domain_usecase(),
            MockDiffGetter::new(&[]),
            base_commit(),
        );
        let result = interactor.classify(vec![fp("track/registry.md")]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].classification, ScopeClassification::Excluded);
    }

    #[test]
    fn test_classify_preserves_input_order() {
        let interactor = ScopeQueryInteractor::new(
            config_domain_usecase(),
            MockDiffGetter::new(&[]),
            base_commit(),
        );
        let inputs = vec![fp("Cargo.toml"), fp("libs/domain/src/lib.rs"), fp("track/registry.md")];
        let result = interactor.classify(inputs.clone()).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].path, inputs[0]);
        assert_eq!(result[1].path, inputs[1]);
        assert_eq!(result[2].path, inputs[2]);
        assert_eq!(result[0].classification, ScopeClassification::Other);
        assert!(matches!(result[1].classification, ScopeClassification::Named(_, _)));
        assert_eq!(result[2].classification, ScopeClassification::Excluded);
    }

    #[test]
    fn test_classify_multi_match_path_returns_named_with_sorted_head_tail() {
        // Configure with overlapping patterns so a single path matches both scopes.
        let cfg = ReviewScopeConfig::new(
            &track_id(),
            vec![
                ("alpha".to_owned(), vec!["shared/**".to_owned()], None),
                ("beta".to_owned(), vec!["shared/**".to_owned()], None),
            ],
            vec![],
            vec![],
        )
        .unwrap();
        let interactor = ScopeQueryInteractor::new(cfg, MockDiffGetter::new(&[]), base_commit());
        let result = interactor.classify(vec![fp("shared/foo.rs")]).unwrap();
        match &result[0].classification {
            ScopeClassification::Named(head, tail) => {
                assert_eq!(head.as_str(), "alpha");
                assert_eq!(tail.len(), 1);
                assert_eq!(tail[0].as_str(), "beta");
            }
            other => panic!("expected Named, got {other:?}"),
        }
    }

    #[test]
    fn test_classify_does_not_call_diff_getter() {
        // Use FailingDiffGetter; if classify touches it the test fails (it
        // would propagate an error). Pure-logic invariant per CN-03.
        let interactor =
            ScopeQueryInteractor::new(config_domain_usecase(), FailingDiffGetter, base_commit());
        let result = interactor.classify(vec![fp("libs/domain/src/lib.rs")]);
        assert!(result.is_ok());
    }

    // ── ScopeQueryInteractor::files tests ──────────────────────────

    #[test]
    fn test_files_unknown_scope_returns_unknown_scope_before_diff_io() {
        // FailingDiffGetter would Err if files() called list_diff_files;
        // proves scope validation runs first (AC-08).
        let interactor =
            ScopeQueryInteractor::new(config_domain_usecase(), FailingDiffGetter, base_commit());
        let unknown = ScopeName::Main(main_scope("nonexistent"));
        let result = interactor.files(unknown);
        assert!(matches!(result, Err(ScopeQueryError::UnknownScope(_))));
    }

    #[test]
    fn test_files_other_scope_succeeds_without_unknown_error() {
        // ScopeName::Other is always valid (implicit scope).
        let interactor = ScopeQueryInteractor::new(
            config_domain_usecase(),
            MockDiffGetter::new(&["Cargo.toml"]),
            base_commit(),
        );
        let result = interactor.files(ScopeName::Other).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "Cargo.toml");
    }

    #[test]
    fn test_files_returns_diff_files_matching_scope() {
        let interactor = ScopeQueryInteractor::new(
            config_domain_usecase(),
            MockDiffGetter::new(&[
                "libs/domain/src/lib.rs",
                "libs/usecase/src/lib.rs",
                "Cargo.toml",
            ]),
            base_commit(),
        );
        let result = interactor.files(ScopeName::Main(main_scope("domain"))).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "libs/domain/src/lib.rs");
    }

    #[test]
    fn test_files_returns_empty_when_no_diff_files_match_scope() {
        let interactor = ScopeQueryInteractor::new(
            config_domain_usecase(),
            MockDiffGetter::new(&["Cargo.toml"]),
            base_commit(),
        );
        let result = interactor.files(ScopeName::Main(main_scope("domain"))).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_files_diff_get_error_wraps_into_scope_query_error() {
        let interactor =
            ScopeQueryInteractor::new(config_domain_usecase(), FailingDiffGetter, base_commit());
        let result = interactor.files(ScopeName::Main(main_scope("domain")));
        assert!(matches!(result, Err(ScopeQueryError::DiffGet(_))));
    }

    #[test]
    fn test_classify_duplicate_input_paths_do_not_produce_duplicate_scope_names() {
        // When the same path appears twice in the input, classify must not
        // emit Named(domain, [domain]) — each scope name must appear at most
        // once in the (head, tail) pair.
        let interactor = ScopeQueryInteractor::new(
            config_domain_usecase(),
            MockDiffGetter::new(&[]),
            base_commit(),
        );
        let dup_path = fp("libs/domain/src/lib.rs");
        let result = interactor.classify(vec![dup_path.clone(), dup_path.clone()]).unwrap();
        assert_eq!(result.len(), 2);
        for pc in &result {
            match &pc.classification {
                ScopeClassification::Named(head, tail) => {
                    assert_eq!(head.as_str(), "domain");
                    // tail must NOT contain "domain" again
                    assert!(
                        tail.iter().all(|n| n.as_str() != "domain"),
                        "tail contains duplicate scope name"
                    );
                }
                other => panic!("expected Named, got {other:?}"),
            }
        }
    }
}
