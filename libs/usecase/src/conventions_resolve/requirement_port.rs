//! Driven port producing one scanned requirement per convention document
//! (spec `IN-05`, `AC-06`, `AC-07`).
//!
//! Re-exported from the parent module, whose type contract this file is part
//! of; it is a separate file only so that neither module outgrows the
//! workspace module-size limit.

use std::path::Path;

use super::{ConventionRequirement, ConventionResolveError};

/// Read-only scan of a project's convention tree (`IN-05`, `AC-06`, `AC-07`).
///
/// The port yields one [`ConventionRequirement`] per scanned document — the
/// document paired with what its front matter declares — and not a selection,
/// so the seam falls between reading documents and choosing among them.
///
/// It lives in `usecase` rather than `domain` by the port tie-break that also
/// placed `ConventionInventoryPort` there: walking a tree is a technical
/// capability the orchestration needs, not a domain invariant. Synchronous by
/// the same decision — one call per process invocation, no ambient runtime —
/// so the blocking read is the deliberate CLI-boundary choice rather than an
/// unstated default.
///
/// The five fail-closed conditions of `AC-07` reach the caller through
/// [`ConventionResolveError`]; a scan meeting none of them succeeds, so a
/// document without front matter is an ordinary requirement declaring nothing.
pub trait ConventionRequirementPort: Send + Sync {
    /// Scans `project_root`'s convention tree and pairs every document found
    /// with the capability identifiers it declares.
    ///
    /// # Errors
    ///
    /// Returns [`ConventionResolveError`] when the scan meets one of `AC-07`'s
    /// fail-closed conditions.
    fn scan_requirements(
        &self,
        project_root: &Path,
    ) -> Result<Vec<ConventionRequirement>, ConventionResolveError>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::thread;

    use super::{ConventionRequirement, ConventionRequirementPort, ConventionResolveError};
    use crate::capability_exec::CapabilityFailureDetail;
    use crate::conventions_resolve::{ConventionCapabilityId, ConventionDocumentPath};

    fn document(path: &str) -> ConventionDocumentPath {
        ConventionDocumentPath::try_new(PathBuf::from(path)).unwrap()
    }

    fn capability(id: &str) -> ConventionCapabilityId {
        ConventionCapabilityId::try_new(id).unwrap()
    }

    /// What a scripted scan hands back, built fresh on every call so that the
    /// same fake can be scanned through more than once.
    enum ScriptedOutcome {
        Scanned(Vec<ConventionRequirement>),
        FailedClosed(fn() -> ConventionResolveError),
    }

    /// Fake adapter standing in for the real filesystem scan.
    ///
    /// The contract is discharged against this rather than a real adapter:
    /// nothing below reads a directory, so what is under test is the port's
    /// promise to its caller and not any one implementation of it.
    struct ScriptedScan {
        outcome: ScriptedOutcome,
        observed_roots: Mutex<Vec<PathBuf>>,
    }

    impl ScriptedScan {
        fn scanning(requirements: Vec<ConventionRequirement>) -> Self {
            Self {
                outcome: ScriptedOutcome::Scanned(requirements),
                observed_roots: Mutex::new(Vec::new()),
            }
        }

        fn failing(build: fn() -> ConventionResolveError) -> Self {
            Self {
                outcome: ScriptedOutcome::FailedClosed(build),
                observed_roots: Mutex::new(Vec::new()),
            }
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
            // Recording the call needs interior mutability precisely because
            // the receiver is shared: the port hands its implementor no
            // exclusive borrow to mutate through.
            self.observed_roots
                .lock()
                .expect("the scripted scan is never poisoned")
                .push(project_root.to_path_buf());
            match &self.outcome {
                ScriptedOutcome::Scanned(requirements) => Ok(requirements.clone()),
                ScriptedOutcome::FailedClosed(build) => Err(build()),
            }
        }
    }

    #[test]
    fn test_scan_requirements_yields_one_requirement_per_scanned_document() {
        let port = ScriptedScan::scanning(vec![
            ConventionRequirement::new(
                document("knowledge/conventions/testing.md"),
                vec![capability("implementer"), capability("reviewer")],
            ),
            ConventionRequirement::new(
                document("knowledge/conventions/rust/naming.md"),
                vec![capability("implementer")],
            ),
            ConventionRequirement::new(document("knowledge/conventions/git-notes.md"), Vec::new()),
        ]);

        let scanned = port.scan_requirements(Path::new("/srv/consumer-project")).unwrap();

        let documents: Vec<String> =
            scanned.iter().map(|requirement| requirement.document().to_string()).collect();
        assert_eq!(
            documents,
            [
                "knowledge/conventions/testing.md",
                "knowledge/conventions/rust/naming.md",
                "knowledge/conventions/git-notes.md",
            ],
            "each scanned document arrives once, as its own requirement, in the order the scan \
             produced it"
        );
        let testing = scanned.first().expect("the scan produced three requirements");
        let naming = scanned.get(1).expect("the scan produced three requirements");
        assert!(testing.requires(&capability("reviewer")));
        assert!(
            !naming.requires(&capability("reviewer")),
            "each requirement carries its own document's declarations, not the scan's union"
        );
    }

    #[test]
    fn test_scan_requirements_scans_the_caller_supplied_project_root() {
        let port = ScriptedScan::scanning(Vec::new());

        port.scan_requirements(Path::new("/srv/consumer-project")).unwrap();
        port.scan_requirements(Path::new("/srv/other-project")).unwrap();

        assert_eq!(
            port.observed_roots(),
            [PathBuf::from("/srv/consumer-project"), PathBuf::from("/srv/other-project")],
            "the tree scanned is the one the caller named, so the port reads no ambient location"
        );
    }

    #[test]
    fn test_scan_requirements_surfaces_every_fail_closed_condition_as_an_error() {
        let conditions: [fn() -> ConventionResolveError; 5] = [
            || ConventionResolveError::FrontMatterUnparseable {
                document: document("knowledge/conventions/front-matter.md"),
                detail: CapabilityFailureDetail::new("unexpected token at line 2"),
            },
            || ConventionResolveError::RequiredForNotStringArray {
                document: document("knowledge/conventions/required-for.md"),
                detail: CapabilityFailureDetail::new("expected a sequence of strings"),
            },
            || ConventionResolveError::EmptyCapabilityId {
                document: document("knowledge/conventions/empty-capability-id.md"),
            },
            || ConventionResolveError::DocumentPathRejected {
                source: ConventionDocumentPath::try_new(PathBuf::from("knowledge/adr/README.md"))
                    .expect_err("a sibling directory is outside the convention root"),
            },
            || ConventionResolveError::DocumentUnreadable {
                document: document("knowledge/conventions/unreadable.md"),
                detail: CapabilityFailureDetail::new("permission denied"),
            },
        ];

        for build in conditions {
            let port = ScriptedScan::failing(build);

            let result = port.scan_requirements(Path::new("/srv/consumer-project"));

            let Err(surfaced) = result else {
                panic!("a fail-closed condition must reach the caller as an error");
            };
            // The return type is `Result<Vec<_>, _>` and not a pair, so a scan
            // that hit this condition has no way to also hand back the
            // documents it had already read: the caller cannot proceed on a
            // partial list without noticing.
            assert_eq!(
                surfaced.to_string(),
                build().to_string(),
                "the condition reaches the caller as itself, not as a substitute error"
            );
        }
    }

    #[test]
    fn test_scan_requirements_succeeds_for_a_scan_meeting_none_of_those_conditions() {
        let port = ScriptedScan::scanning(vec![ConventionRequirement::new(
            document("knowledge/conventions/git-notes.md"),
            Vec::new(),
        )]);

        let scanned = port.scan_requirements(Path::new("/srv/consumer-project")).unwrap();

        assert_eq!(scanned.len(), 1);
        let declaring_nothing = scanned.first().expect("the scan produced one requirement");
        assert!(
            !declaring_nothing.requires(&capability("implementer")),
            "a document declaring nothing is scanned into an ordinary requirement, so the port \
             has no route to fail outside the conditions it can express"
        );
    }

    #[test]
    fn test_scan_requirements_reads_through_a_shared_receiver_across_threads() {
        let port = ScriptedScan::scanning(vec![ConventionRequirement::new(
            document("knowledge/conventions/testing.md"),
            vec![capability("implementer")],
        )]);
        let shared: &dyn ConventionRequirementPort = &port;

        // Two concurrent scans through one shared reference: the call needs no
        // exclusive borrow and no executor, which is what makes the port both
        // injectable behind `&dyn` and safe to hold in a `Send + Sync` root.
        let (first, second) = thread::scope(|scope| {
            let left = scope.spawn(|| shared.scan_requirements(Path::new("/srv/consumer-project")));
            let right =
                scope.spawn(|| shared.scan_requirements(Path::new("/srv/consumer-project")));
            (
                left.join().expect("the scan thread does not panic").unwrap(),
                right.join().expect("the scan thread does not panic").unwrap(),
            )
        });

        assert_eq!(first, second, "reading twice observes the same documents");
        assert_eq!(port.observed_roots().len(), 2);
    }

    #[test]
    fn test_convention_requirement_port_bound_implies_send_and_sync() {
        fn implied_by_the_port<T: ConventionRequirementPort + ?Sized>() {
            fn shareable<U: Send + Sync + ?Sized>() {}
            // Compiles only because the supertrait bound carries `Send + Sync`
            // through: the check is on the trait, not on any one implementor.
            shareable::<T>();
        }

        implied_by_the_port::<dyn ConventionRequirementPort>();
        implied_by_the_port::<ScriptedScan>();
    }
}
