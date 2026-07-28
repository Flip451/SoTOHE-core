//! Primary port of convention resolution and the interactor that realises it
//! over an injected scan (spec `IN-05`, `AC-06`, `AC-08`).
//!
//! Re-exported from the parent module, whose type contract this file is part
//! of; it is a separate file only so that neither module outgrows the
//! workspace module-size limit.

use std::sync::Arc;

use super::{
    ConventionRequirementPort, ConventionResolution, ConventionResolveError,
    ResolveConventionsQuery, select_required_conventions,
};

/// Read-only resolution of the conventions one capability requires (`IN-05`,
/// `AC-06`).
///
/// A trait rather than a free function because the operation has two
/// independent drivers — the `conventions resolve` command and the capability
/// dispatcher's preflight — and the dispatcher holds it as
/// `Arc<dyn ConventionResolveService>`, which is why the bound carries
/// `Send + Sync` and the receiver is shared.
///
/// The declared result is what fixes the shape of the answer: success is a
/// [`ConventionResolution`], the canonical value, so no implementor can hand a
/// caller a raw path list in scan order; failure is a
/// [`ConventionResolveError`], so a read that failed cannot arrive as a
/// resolution that matched nothing.
pub trait ConventionResolveService: Send + Sync {
    /// Resolves the convention documents `query`'s capability requires.
    ///
    /// # Errors
    ///
    /// Returns [`ConventionResolveError`] when the underlying scan meets one of
    /// `AC-07`'s fail-closed conditions. Matching no document is not one of
    /// them: it is the ordinary empty resolution (`AC-08`).
    fn resolve(
        &self,
        query: ResolveConventionsQuery,
    ) -> Result<ConventionResolution, ConventionResolveError>;
}

/// [`ConventionResolveService`] realised by scanning through an injected
/// [`ConventionRequirementPort`] (`IN-05`, `AC-06`, `AC-08`).
///
/// The type exists rather than a free function only to carry that injected
/// dependency; it holds nothing else, so two calls through the same value are
/// as independent as two calls to a function would be.
///
/// Its own contribution is the wiring, and nothing besides: the scan's
/// requirements reach `select_required_conventions` together with the query's
/// capability, and what that selection returns is what the caller receives.
/// Whether a document declares the capability stays
/// `ConventionRequirement::requires`' decision, deduplication and ordering stay
/// [`ConventionResolution`]'s, and the fail-closed conditions stay the port's
/// to raise and [`ConventionResolveError`]'s to name.
pub struct ConventionResolveInteractor {
    requirements: Arc<dyn ConventionRequirementPort>,
}

impl ConventionResolveInteractor {
    /// Wires the interactor to the scan it resolves through.
    #[must_use]
    pub fn new(requirements: Arc<dyn ConventionRequirementPort>) -> Self {
        Self { requirements }
    }
}

impl ConventionResolveService for ConventionResolveInteractor {
    fn resolve(
        &self,
        query: ResolveConventionsQuery,
    ) -> Result<ConventionResolution, ConventionResolveError> {
        let scanned = self.requirements.scan_requirements(&query.project_root)?;
        Ok(select_required_conventions(&scanned, &query.capability))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::thread;

    use super::{
        ConventionRequirementPort, ConventionResolution, ConventionResolveError,
        ConventionResolveInteractor, ConventionResolveService, ResolveConventionsQuery,
        select_required_conventions,
    };
    use crate::capability_exec::CapabilityFailureDetail;
    use crate::conventions_resolve::{
        ConventionCapabilityId, ConventionDocumentPath, ConventionRequirement,
    };

    const PROJECT_ROOT: &str = "/srv/consumer-project";

    fn document(path: &str) -> ConventionDocumentPath {
        ConventionDocumentPath::try_new(PathBuf::from(path)).unwrap()
    }

    fn capability(id: &str) -> ConventionCapabilityId {
        ConventionCapabilityId::try_new(id).unwrap()
    }

    fn requirement(path: &str, declared: &[&str]) -> ConventionRequirement {
        ConventionRequirement::new(
            document(path),
            declared.iter().map(|id| capability(id)).collect(),
        )
    }

    fn query(id: &str) -> ResolveConventionsQuery {
        ResolveConventionsQuery {
            capability: capability(id),
            project_root: PathBuf::from(PROJECT_ROOT),
        }
    }

    fn paths(resolution: &ConventionResolution) -> Vec<String> {
        resolution.documents().iter().map(ToString::to_string).collect()
    }

    fn unreadable() -> ConventionResolveError {
        ConventionResolveError::DocumentUnreadable {
            document: document("knowledge/conventions/unreadable.md"),
            detail: CapabilityFailureDetail::new("permission denied"),
        }
    }

    /// Stand-in implementor of the port, so that what the tests below exercise
    /// is the operation the trait declares rather than any one realisation of
    /// it.
    enum ScriptedResolve {
        Selecting(Vec<ConventionDocumentPath>),
        FailingClosed,
    }

    impl ConventionResolveService for ScriptedResolve {
        fn resolve(
            &self,
            _query: ResolveConventionsQuery,
        ) -> Result<ConventionResolution, ConventionResolveError> {
            match self {
                Self::Selecting(matched) => Ok(ConventionResolution::from_matches(matched.clone())),
                Self::FailingClosed => Err(unreadable()),
            }
        }
    }

    /// Fake scan the interactor is wired to; the real adapter is not involved,
    /// so what is under test is the wiring and not a filesystem walk.
    struct ScriptedScan {
        scanned: Result<Vec<ConventionRequirement>, fn() -> ConventionResolveError>,
        observed_roots: Mutex<Vec<PathBuf>>,
    }

    impl ScriptedScan {
        fn scanning(requirements: Vec<ConventionRequirement>) -> Self {
            Self { scanned: Ok(requirements), observed_roots: Mutex::new(Vec::new()) }
        }

        fn failing() -> Self {
            Self { scanned: Err(unreadable), observed_roots: Mutex::new(Vec::new()) }
        }

        fn observed_roots(&self) -> Vec<PathBuf> {
            self.observed_roots.lock().expect("the scripted scan is never poisoned").clone()
        }
    }

    impl ConventionRequirementPort for ScriptedScan {
        fn scan_requirements(
            &self,
            project_root: &Path,
        ) -> Result<Vec<ConventionRequirement>, ConventionResolveError> {
            self.observed_roots
                .lock()
                .expect("the scripted scan is never poisoned")
                .push(project_root.to_path_buf());
            match &self.scanned {
                Ok(requirements) => Ok(requirements.clone()),
                Err(build) => Err(build()),
            }
        }
    }

    fn wired_to(port: ScriptedScan) -> (Arc<ScriptedScan>, ConventionResolveInteractor) {
        let port = Arc::new(port);
        let interactor = ConventionResolveInteractor::new(
            Arc::clone(&port) as Arc<dyn ConventionRequirementPort>
        );
        (port, interactor)
    }

    #[test]
    fn test_resolve_hands_the_caller_the_canonical_resolution_and_not_a_raw_path_list() {
        // The stand-in is handed its matches the way a scan would produce
        // them: unordered, with one document matched twice.
        let port: Arc<dyn ConventionResolveService> = Arc::new(ScriptedResolve::Selecting(vec![
            document("knowledge/conventions/testing.md"),
            document("knowledge/conventions/adr.md"),
            document("knowledge/conventions/testing.md"),
        ]));

        let resolved = port.resolve(query("implementer")).unwrap();

        assert_eq!(
            paths(&resolved),
            ["knowledge/conventions/adr.md", "knowledge/conventions/testing.md"],
            "the declared success type is the canonical resolution, so this port offers an \
             implementor no channel through which a scan-ordered or repeated path list could \
             reach the caller"
        );
    }

    #[test]
    fn test_resolve_separates_a_failed_read_from_a_resolution_that_matched_nothing() {
        let failing: Arc<dyn ConventionResolveService> = Arc::new(ScriptedResolve::FailingClosed);
        let matching_nothing: Arc<dyn ConventionResolveService> =
            Arc::new(ScriptedResolve::Selecting(Vec::new()));

        let failure = failing.resolve(query("implementer"));
        let empty = matching_nothing.resolve(query("implementer")).unwrap();

        // The two outcomes arrive on different arms of the declared `Result`,
        // so a caller of this port reads neither into the other: an
        // implementor cannot report a failed read as an empty answer, and a
        // caller holding an empty answer knows the read succeeded.
        assert!(matches!(failure, Err(ConventionResolveError::DocumentUnreadable { .. })));
        assert!(empty.is_empty());
        assert_eq!(empty, ConventionResolution::default());
    }

    #[test]
    fn test_resolve_is_reachable_from_more_than_one_holder_of_one_shared_port() {
        let port: Arc<dyn ConventionResolveService> =
            Arc::new(ScriptedResolve::Selecting(vec![document(
                "knowledge/conventions/testing.md",
            )]));
        let driver = Arc::clone(&port);
        let dispatcher = Arc::clone(&port);

        // Two independent holders resolve through the same value at the same
        // time: the receiver is shared and the supertrait bound carries
        // `Send + Sync`, which is what lets both declared drivers hold the one
        // port rather than each owning an operation of its own.
        let (from_driver, from_dispatcher) = thread::scope(|scope| {
            let left = scope.spawn(move || driver.resolve(query("implementer")));
            let right = scope.spawn(move || dispatcher.resolve(query("implementer")));
            (
                left.join().expect("the resolving thread does not panic").unwrap(),
                right.join().expect("the resolving thread does not panic").unwrap(),
            )
        });

        assert_eq!(from_driver, from_dispatcher, "resolving twice answers the same value");
    }

    #[test]
    fn test_interactor_scans_the_project_root_the_query_named() {
        let (port, interactor) = wired_to(ScriptedScan::scanning(Vec::new()));

        interactor.resolve(query("implementer")).unwrap();
        interactor
            .resolve(ResolveConventionsQuery {
                capability: capability("reviewer"),
                project_root: PathBuf::from("/srv/other-project"),
            })
            .unwrap();

        assert_eq!(
            port.observed_roots(),
            [PathBuf::from(PROJECT_ROOT), PathBuf::from("/srv/other-project")],
            "the tree scanned is the one the request named, so the interactor resolves against no \
             ambient location of its own"
        );
    }

    #[test]
    fn test_interactor_answers_with_what_the_selection_selects_over_the_scanned_requirements() {
        let scanned = vec![
            requirement("knowledge/conventions/testing.md", &["implementer", "reviewer"]),
            requirement("knowledge/conventions/adr.md", &["reviewer"]),
            requirement("knowledge/conventions/rust/naming.md", &["implementer"]),
            requirement("knowledge/conventions/git-notes.md", &[]),
        ];
        let (_port, interactor) = wired_to(ScriptedScan::scanning(scanned.clone()));

        let resolved = interactor.resolve(query("implementer")).unwrap();

        // Computed independently of the interactor from the same two inputs:
        // the requirements the scan produced and the capability the request
        // named. Equality is the whole of the interactor's promise — the
        // scan's result reaches the selection, and the selection's result
        // reaches the caller with nothing inserted, dropped or reordered on
        // the way.
        assert_eq!(resolved, select_required_conventions(&scanned, &capability("implementer")));
        assert_eq!(
            paths(&resolved),
            ["knowledge/conventions/rust/naming.md", "knowledge/conventions/testing.md"]
        );
    }

    #[test]
    fn test_interactor_surfaces_a_failed_scan_as_that_failure() {
        let (_port, interactor) = wired_to(ScriptedScan::failing());

        let result = interactor.resolve(query("implementer"));

        let Err(surfaced) = result else {
            panic!("a scan that failed closed must not resolve to an answer");
        };
        assert_eq!(
            surfaced.to_string(),
            unreadable().to_string(),
            "the port's failure reaches the caller as itself: the interactor neither substitutes \
             a failure of its own nor selects over a list the scan never handed it"
        );
    }

    #[test]
    fn test_interactor_answers_an_empty_resolution_where_nothing_matched() {
        let declaring_nothing = vec![
            requirement("knowledge/conventions/git-notes.md", &[]),
            requirement("knowledge/conventions/adr.md", &["reviewer"]),
        ];

        for scanned in [declaring_nothing, Vec::new()] {
            let (_port, interactor) = wired_to(ScriptedScan::scanning(scanned));

            let resolved = interactor
                .resolve(query("implementer"))
                .expect("matching no document is not a fail-closed condition");

            assert_eq!(
                resolved,
                ConventionResolution::default(),
                "a document declaring nothing and a capability matching none both resolve to the \
                 ordinary empty answer, which the signature offers no route to turn into a failure"
            );
        }
    }

    #[test]
    fn test_interactor_carries_nothing_from_one_resolution_into_the_next() {
        let (port, interactor) = wired_to(ScriptedScan::scanning(vec![
            requirement("knowledge/conventions/testing.md", &["implementer"]),
            requirement("knowledge/conventions/adr.md", &["reviewer"]),
        ]));

        let first = interactor.resolve(query("implementer")).unwrap();
        let intervening = interactor.resolve(query("reviewer")).unwrap();
        let again = interactor.resolve(query("implementer")).unwrap();

        assert_eq!(paths(&intervening), ["knowledge/conventions/adr.md"]);
        assert_eq!(
            first, again,
            "the injected scan is the only thing the interactor holds, so an intervening request \
             for another capability leaves nothing behind to change this answer"
        );
        assert_eq!(port.observed_roots().len(), 3, "every request is answered from its own scan");
    }
}
