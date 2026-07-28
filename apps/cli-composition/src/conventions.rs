//! `conventions` command family — `ConventionsCompositionRoot` impl methods.

use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `conventions` command family.
///
/// Wires the `ConventionsPort` adapter into `ConventionsInteractor`, then injects
/// `ConventionsInteractor` into `ConventionsDriver`.
pub struct ConventionsCompositionRoot;

impl ConventionsCompositionRoot {
    /// Create a new `ConventionsCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConventionsCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl ConventionsCompositionRoot {
    /// Build a wired [`cli_driver::conventions::ConventionsDriver`] for the conventions family.
    ///
    /// Wire chain: `FsConventionsAdapter` → `ConventionsInteractor` → `ConventionsDriver`.
    pub fn conventions_driver(&self) -> cli_driver::conventions::ConventionsDriver {
        use infrastructure::conventions::FsConventionsAdapter;
        use usecase::conventions::{ConventionsInteractor, ConventionsPort};

        let adapter = Arc::new(FsConventionsAdapter::new());
        let interactor = Arc::new(ConventionsInteractor::new(adapter as Arc<dyn ConventionsPort>));
        cli_driver::conventions::ConventionsDriver::new(interactor)
    }

    /// Build a wired [`cli_driver::conventions_resolve::ConventionResolveDriver`]
    /// for the read-only `conventions resolve` command (`IN-05`, `AC-06`).
    ///
    /// Wire chain: `FsConventionRequirementAdapter` → `ConventionResolveInteractor`
    /// → `ConventionResolveDriver`. The filesystem side of the chain is the
    /// requirement scan and nothing else, so the assembled command has no route
    /// to creating, updating, deleting, or indexing a convention document —
    /// `AC-06`'s read-only promise holds by what is injected here rather than by
    /// the driver refraining from calling something it was handed.
    pub fn conventions_resolve_driver(
        &self,
    ) -> cli_driver::conventions_resolve::ConventionResolveDriver {
        use infrastructure::conventions_resolve::FsConventionRequirementAdapter;
        use usecase::conventions_resolve::{
            ConventionRequirementPort, ConventionResolveInteractor,
        };

        let adapter = Arc::new(FsConventionRequirementAdapter::new());
        let interactor = Arc::new(ConventionResolveInteractor::new(
            adapter as Arc<dyn ConventionRequirementPort>,
        ));
        cli_driver::conventions_resolve::ConventionResolveDriver::new(interactor)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::path::Path;

    use cli_driver::CommandOutcome;
    use cli_driver::conventions_resolve::{ConventionCapabilityIdArg, ConventionResolveInput};
    use tempfile::TempDir;

    use super::ConventionsCompositionRoot;

    const DECLARING: &str = "---\nrequired_for:\n  - implementer\n---\n\n# Declaring\n";
    const OTHER: &str = "---\nrequired_for:\n  - reviewer\n---\n\n# Other\n";

    /// Builds a project tree in which two documents declare `implementer` and a
    /// third declares something else.
    ///
    /// The file names are deliberately synthetic: none of them occurs in this
    /// repository's own `knowledge/conventions/` tree, so a path naming one of
    /// them can only have come from the tree the caller named.
    fn project_with_conventions() -> TempDir {
        let project = TempDir::new().unwrap();
        let dir = project.path().join("knowledge").join("conventions");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("wiring-probe-alpha.md"), DECLARING).unwrap();
        fs::write(dir.join("wiring-probe-beta.md"), DECLARING).unwrap();
        fs::write(dir.join("wiring-probe-gamma.md"), OTHER).unwrap();
        project
    }

    fn resolve_implementer(project_root: &Path) -> CommandOutcome {
        ConventionsCompositionRoot::new().conventions_resolve_driver().handle(
            ConventionResolveInput {
                capability: "implementer".parse::<ConventionCapabilityIdArg>().unwrap(),
                project_root: project_root.to_path_buf(),
            },
        )
    }

    /// Reads the convention tree back as `(file name, contents)` pairs.
    fn convention_tree(project_root: &Path) -> Vec<(String, String)> {
        let dir = project_root.join("knowledge").join("conventions");
        let mut documents = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                (name, fs::read_to_string(&path).unwrap())
            })
            .collect::<Vec<_>>();
        documents.sort();
        documents
    }

    /// The accessor's chain answers from the tree the caller named: a document
    /// that exists only in the caller's tempdir is named in the answer. Wiring a
    /// different port, or leaving the interactor unwired to the filesystem scan,
    /// could not produce a path naming that document (`IN-05`, `AC-06`).
    ///
    /// The assertion is deliberately a containment rather than an exact stdout
    /// match. This accessor establishes which components are wired together; it
    /// does not establish which subset of the scanned documents is returned, in
    /// what order, or with what deduplication — those arrive inside
    /// `ConventionResolution` and are established by the entries that own them.
    /// An exact match would additionally go red on a change to the selection
    /// rule or to the driver's rendering, neither of which is this entry's to
    /// establish. The fixture's other two documents are left unadjudicated here
    /// for that reason.
    #[test]
    fn test_conventions_resolve_driver_answers_from_the_named_project_tree() {
        let project = project_with_conventions();

        let outcome = resolve_implementer(project.path());

        assert_eq!(outcome.exit_code, 0, "resolution failed: {:?}", outcome.stderr);
        let answer = outcome.stdout.unwrap_or_default();
        assert!(
            answer.contains("wiring-probe-alpha.md"),
            "the answer did not come from the named project tree: {answer:?}"
        );
    }

    /// Nothing on the injected chain writes: the convention tree is byte-identical
    /// after a resolution, so the read-only command cannot maintain documents
    /// through the components this accessor chose (`AC-06`).
    #[test]
    fn test_conventions_resolve_driver_leaves_the_convention_tree_unchanged() {
        let project = project_with_conventions();
        let before = convention_tree(project.path());

        let outcome = resolve_implementer(project.path());

        assert_eq!(outcome.exit_code, 0, "resolution failed: {:?}", outcome.stderr);
        assert_eq!(convention_tree(project.path()), before);
    }
}
