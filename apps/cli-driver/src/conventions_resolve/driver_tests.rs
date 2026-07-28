//! Tests of the `conventions resolve` boundary mirror, input, and driver.
//!
//! A child module of `conventions_resolve`, so the mirror's private field is
//! readable here without any accessor widening its visibility.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use usecase::capability_exec::CapabilityFailureDetail;
use usecase::conventions_resolve::{
    ConventionCapabilityId, ConventionCapabilityIdError, ConventionDocumentPath,
    ConventionResolution, ConventionResolveError, ConventionResolveService,
    ResolveConventionsQuery,
};

use super::{ConventionCapabilityIdArg, ConventionResolveDriver, ConventionResolveInput};

const PROJECT_ROOT: &str = "/srv/consumer-project";

/// Stand-in for the resolver, so that what these tests exercise is the
/// driver's own contribution rather than a filesystem scan.
enum Answer {
    Matching(Vec<ConventionDocumentPath>),
    FailingClosed,
}

struct ScriptedResolve {
    answer: Answer,
    queries: Mutex<Vec<ResolveConventionsQuery>>,
}

impl ScriptedResolve {
    fn queries(&self) -> Vec<ResolveConventionsQuery> {
        self.queries.lock().expect("the scripted resolver is never poisoned").clone()
    }
}

impl ConventionResolveService for ScriptedResolve {
    fn resolve(
        &self,
        query: ResolveConventionsQuery,
    ) -> Result<ConventionResolution, ConventionResolveError> {
        self.queries.lock().expect("the scripted resolver is never poisoned").push(query);
        match &self.answer {
            Answer::Matching(matched) => Ok(ConventionResolution::from_matches(matched.clone())),
            Answer::FailingClosed => Err(unreadable()),
        }
    }
}

fn unreadable() -> ConventionResolveError {
    ConventionResolveError::DocumentUnreadable {
        document: document("knowledge/conventions/unreadable.md"),
        detail: CapabilityFailureDetail::new("permission denied"),
    }
}

fn document(path: &str) -> ConventionDocumentPath {
    ConventionDocumentPath::try_new(PathBuf::from(path)).unwrap()
}

fn driver_over(answer: Answer) -> (Arc<ScriptedResolve>, ConventionResolveDriver) {
    let service = Arc::new(ScriptedResolve { answer, queries: Mutex::new(Vec::new()) });
    let driver =
        ConventionResolveDriver::new(Arc::clone(&service) as Arc<dyn ConventionResolveService>);
    (service, driver)
}

fn input(capability: &str, project_root: &str) -> ConventionResolveInput {
    ConventionResolveInput {
        capability: ConventionCapabilityIdArg::from_str(capability)
            .expect("a non-blank capability argument parses"),
        project_root: PathBuf::from(project_root),
    }
}

#[test]
fn test_capability_id_arg_parses_the_argument_the_boundary_neither_trims_nor_looks_up() {
    // `consumer-house-style` names no entry in `.harness/capabilities/` and
    // none in `agent-profiles.json`; the padded spellings are what a trimming
    // lookup key would fold together.
    for accepted in [
        "implementer",
        " implementer ",
        "implementer\t",
        "consumer-house-style",
        "team::custom-lint",
        "SHOUTING_ID",
        "x",
    ] {
        let parsed = ConventionCapabilityIdArg::from_str(accepted)
            .expect("a non-blank argument parses, whatever it is registered as");

        assert_eq!(
            parsed.0,
            ConventionCapabilityId::try_new(accepted).unwrap(),
            "the boundary holds what the constructor built from the argument as typed, so it \
             adds no normalization of its own and consults no registry"
        );
    }

    assert_ne!(
        ConventionCapabilityIdArg::from_str(" reviewer ").unwrap(),
        ConventionCapabilityIdArg::from_str("reviewer").unwrap(),
        "a padded argument stays a different identifier here, which is what keeps the match an \
         exact one rather than a normalize-then-compare"
    );
}

#[test]
fn test_capability_id_arg_rejects_only_the_blank_argument() {
    for blank in ["", "   ", "\t", "\n"] {
        assert!(
            matches!(
                ConventionCapabilityIdArg::from_str(blank),
                Err(ConventionCapabilityIdError::Blank)
            ),
            "'{blank:?}' carries no identifier, and blankness is the one rejection the delegated \
             constructor performs, so it is this boundary's only one"
        );
    }
}

#[test]
fn test_resolve_input_carries_the_parsed_term_and_the_named_root_into_the_query() {
    for requested in ["implementer", " implementer ", "consumer-house-style", "team::custom-lint"] {
        let (service, driver) = driver_over(Answer::Matching(Vec::new()));

        let _ = driver.handle(input(requested, PROJECT_ROOT));

        let observed = service.queries();
        let query = observed.first().expect("the command issues one resolution request");
        assert_eq!(
            *query,
            ResolveConventionsQuery {
                capability: ConventionCapabilityId::try_new(requested).unwrap(),
                project_root: PathBuf::from(PROJECT_ROOT),
            },
            "the identifier the caller named reaches the resolver spelled exactly as typed, and \
             the tree scanned is the one the input named, so the result answers the request that \
             was actually made"
        );
    }
}

#[test]
fn test_driver_renders_the_canonical_result_as_the_machine_readable_output() {
    // Handed to the stand-in the way a scan would produce them: unordered, with
    // one document matched twice. The dedup and the ordering that turns them
    // into a resolution are established inside `ConventionResolution` and are
    // asserted there; what is asserted here is only what the driver adds, so
    // the expectation is read back off the same resolution rather than spelled
    // out as a literal that a usecase-side change would turn red.
    let matched = vec![
        document("knowledge/conventions/testing.md"),
        document("knowledge/conventions/adr.md"),
        document("knowledge/conventions/testing.md"),
    ];
    let (_service, driver) = driver_over(Answer::Matching(matched.clone()));

    let outcome = driver.handle(input("implementer", PROJECT_ROOT));

    let held = ConventionResolution::from_matches(matched);
    let expected: Vec<String> = held.documents().iter().map(ToString::to_string).collect();
    let rendered: Vec<String> = outcome
        .stdout
        .as_deref()
        .expect("a matched resolution has a canonical result")
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(
        rendered, expected,
        "the machine-readable output is one line per document the resolution holds, in the order \
         it holds them, with nothing added, dropped, reordered, or written beside them"
    );
}

#[test]
fn test_driver_human_display_is_rendered_from_the_same_resolution() {
    let (_service, driver) = driver_over(Answer::Matching(vec![
        document("knowledge/conventions/testing.md"),
        document("knowledge/conventions/adr.md"),
    ]));

    let outcome = driver.handle(input("implementer", PROJECT_ROOT));

    let machine = outcome.stdout.expect("the machine-readable result is present");
    let human = outcome.stderr.expect("the human display is present");
    assert!(
        human.ends_with(&machine),
        "the display lists the canonical result verbatim rather than assembling the documents a \
         second time, so the two cannot disagree about which documents matched or in what order: \
         {human}"
    );
    assert!(
        human.starts_with("2 convention document(s)"),
        "what the display adds is its own count of that same result: {human}"
    );
}

#[test]
fn test_driver_reports_a_resolution_matching_no_document_as_success() {
    // A document without front matter, a document without `required_for`, and
    // a capability matching nothing all arrive here as this one empty
    // resolution, and the success arm is the only arm it can reach.
    let (_service, driver) = driver_over(Answer::Matching(Vec::new()));

    let outcome = driver.handle(input("consumer-house-style", PROJECT_ROOT));

    assert_eq!(outcome.exit_code, 0, "an empty result is an answer, not a failure");
    assert_eq!(
        outcome.stderr.as_deref(),
        Some("no convention document declares this capability"),
        "the display says so plainly rather than presenting it as a problem"
    );
}

#[test]
fn test_driver_gives_a_zero_match_resolution_no_canonical_line_at_all() {
    // Separate from the success-shape test above because this is an `IN-05`
    // claim about the line-per-document stream rather than the `AC-08` claim
    // that matching nothing is not a failure.
    let (_service, driver) = driver_over(Answer::Matching(Vec::new()));

    let outcome = driver.handle(input("zero-match-probe", PROJECT_ROOT));

    assert_eq!(
        outcome.stdout, None,
        "the emitting `bin` writes a present stdout as a line, so rendering the empty resolution \
         as an empty string would put one blank line on the stream whose lines are the answer, \
         and a caller counting or iterating those lines would read one nameless document where \
         none was declared"
    );
    assert_eq!(
        outcome.stdout.as_deref().unwrap_or_default().lines().count(),
        0,
        "no document declared means no record, which on a line-per-record stream is zero lines"
    );
}

#[test]
fn test_driver_reports_only_a_failed_read_as_a_failure() {
    let (_service, driver) = driver_over(Answer::FailingClosed);

    let outcome = driver.handle(input("implementer", PROJECT_ROOT));

    let rendered = unreadable().to_string();
    assert_eq!(outcome.exit_code, 1);
    assert_eq!(outcome.stdout, None, "a read that failed produces no canonical result");
    assert_eq!(
        outcome.stderr.as_deref(),
        Some(rendered.as_str()),
        "the resolver's error is the only thing that reaches the failure arm, which is what \
         keeps an empty answer off it"
    );
}

#[test]
fn test_driver_resolves_a_capability_registered_in_no_registry() {
    let (_service, driver) =
        driver_over(Answer::Matching(vec![document("knowledge/conventions/house-style.md")]));

    let outcome = driver.handle(input("consumer-house-style", PROJECT_ROOT));

    assert_eq!(
        outcome.exit_code, 0,
        "no step of this path consults `.harness/capabilities/` or `agent-profiles.json`, so an \
         unregistered identifier has no state in which the command fails for want of a \
         registration"
    );
    assert_eq!(outcome.stdout.as_deref(), Some("knowledge/conventions/house-style.md"));
}

#[test]
fn test_driver_reaches_no_operation_other_than_the_read() {
    let (service, driver) =
        driver_over(Answer::Matching(vec![document("knowledge/conventions/testing.md")]));

    let _ = driver.handle(input("implementer", PROJECT_ROOT));
    let _ = driver.handle(input("reviewer", PROJECT_ROOT));

    // The injected resolver is the driver's only collaborator, so every effect
    // a command of this family can have is one of these reads: creating,
    // updating, deleting, or re-indexing a convention document has no route
    // out of here.
    assert_eq!(
        service.queries().len(),
        2,
        "each command is answered by exactly one resolution request and nothing else"
    );
}
