//! Tests of the convention non-shipping check input and driver.
//!
//! A child module of `template_conventions`, so the driver's private field is
//! reachable here without any accessor widening its visibility.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use domain::tddd::catalogue_linter::FreeText;
use domain::tddd::catalogue_v2::NonEmptyVec;
use usecase::conventions_resolve::ConventionDocumentPath;
use usecase::template_conventions::{
    CheckConventionShippingQuery, ConventionShippingCheckError, ConventionShippingCheckService,
    ConventionShippingVerdict,
};

use super::{ConventionShippingCheckDriver, ConventionShippingCheckInput};

const EXPORTED_ROOT: &str = "/tmp/template-export-smoke";
const OVERLAY_DIR: &str = "/srv/sotohe/overlay";

/// Stand-in for the check service, so that what these tests exercise is the
/// driver's own contribution rather than a filesystem walk.
enum Answer {
    Verdict(ConventionShippingVerdict),
    Unanswerable,
}

struct ScriptedCheck {
    answer: Answer,
    queries: Mutex<Vec<CheckConventionShippingQuery>>,
}

impl ScriptedCheck {
    fn queries(&self) -> Vec<CheckConventionShippingQuery> {
        self.queries.lock().expect("the scripted check is never poisoned").clone()
    }
}

impl ConventionShippingCheckService for ScriptedCheck {
    fn check(
        &self,
        query: CheckConventionShippingQuery,
    ) -> Result<ConventionShippingVerdict, ConventionShippingCheckError> {
        self.queries.lock().expect("the scripted check is never poisoned").push(query);
        match &self.answer {
            Answer::Verdict(verdict) => Ok(verdict.clone()),
            Answer::Unanswerable => Err(unanswerable()),
        }
    }
}

fn unanswerable() -> ConventionShippingCheckError {
    ConventionShippingCheckError::TreeUnreadable {
        path: PathBuf::from(EXPORTED_ROOT).join("knowledge/conventions"),
        reason: FreeText::new("permission denied".to_owned()),
    }
}

fn document(name: &str) -> ConventionDocumentPath {
    ConventionDocumentPath::try_new(PathBuf::from(format!("knowledge/conventions/{name}"))).unwrap()
}

/// The documents a source repository ships beyond an overlay's supply, in the
/// order the verdict would hold them.
fn unsupplied(count: usize) -> Vec<ConventionDocumentPath> {
    ["dry-check-workflow.md", "enforce-by-mechanism.md", "language-policy.md", "shell-parsing.md"]
        .into_iter()
        .chain(["typed-deserialization.md"])
        .take(count)
        .map(document)
        .collect()
}

fn violation(documents: Vec<ConventionDocumentPath>) -> ConventionShippingVerdict {
    ConventionShippingVerdict::UnsuppliedDocumentsShipped {
        documents: NonEmptyVec::try_new(documents)
            .expect("a violation names at least one document"),
    }
}

fn driver_over(answer: Answer) -> (Arc<ScriptedCheck>, ConventionShippingCheckDriver) {
    let service = Arc::new(ScriptedCheck { answer, queries: Mutex::new(Vec::new()) });
    let driver = ConventionShippingCheckDriver::new(
        Arc::clone(&service) as Arc<dyn ConventionShippingCheckService>
    );
    (service, driver)
}

fn input(exported_root: &str, overlay_dir: &str) -> ConventionShippingCheckInput {
    ConventionShippingCheckInput {
        exported_root: PathBuf::from(exported_root),
        overlay_dir: PathBuf::from(overlay_dir),
    }
}

#[test]
fn test_input_carries_both_tree_roots_into_the_query_untransposed() {
    // The check's subject is the pair of trees the caller named, and it reaches
    // the service as the caller spelled it: an item that inspected some other
    // tree, or that measured the export against itself, would answer a question
    // nobody asked. The two roots are deliberately dissimilar so that a
    // transposition shows up as a mismatch rather than as an equal pair.
    let (service, driver) = driver_over(Answer::Verdict(ConventionShippingVerdict::Conforming));

    let _ = driver.handle(input(EXPORTED_ROOT, OVERLAY_DIR));

    let observed = service.queries();
    let query = observed.first().expect("the command issues one check request");
    assert_eq!(
        *query,
        CheckConventionShippingQuery {
            exported_root: PathBuf::from(EXPORTED_ROOT),
            overlay_dir: PathBuf::from(OVERLAY_DIR),
        },
        "the exported tree checked is the one the input named and the supply it is measured \
         against is the overlay the input named, neither inferred nor swapped"
    );
}

#[test]
fn test_driver_names_every_unsupplied_document_and_summarises_none_of_them() {
    // The verdict carries the offending paths rather than a count so that no
    // consumer can report the violation without being able to name it. This is
    // the last step at which that could be discarded, so the rendering is
    // checked at several cardinalities: a rendering that named only the first
    // document, or only how many there were, would pass a single-document case.
    for count in 1..=5 {
        let documents = unsupplied(count);
        let (_service, driver) = driver_over(Answer::Verdict(violation(documents.clone())));

        let outcome = driver.handle(input(EXPORTED_ROOT, OVERLAY_DIR));

        let rendered = outcome.stderr.expect("a violation is reported");
        let named: Vec<String> = rendered.lines().skip(1).map(str::to_owned).collect();
        let expected: Vec<String> = documents.iter().map(ToString::to_string).collect();
        assert_eq!(
            named, expected,
            "every offending document is named, on a line of its own, in the order the verdict \
             holds them, with none dropped and none folded into a summary"
        );
        for document in &documents {
            assert!(
                rendered.contains(&document.to_string()),
                "the rendering names {document} rather than counting it: {rendered}"
            );
        }
    }
}

#[test]
fn test_driver_settles_the_shipping_question_with_the_exit_code_a_gate_reads() {
    // The only consumer of this command is a gate, so the two verdicts have to
    // be distinguishable by exit code alone. A violation rendered to stderr
    // while exiting 0 would report the mutation to a reader and hide it from the
    // smoke run that has to fail on it.
    let (_service, conforming) =
        driver_over(Answer::Verdict(ConventionShippingVerdict::Conforming));
    let (_service, violating) = driver_over(Answer::Verdict(violation(unsupplied(2))));

    let passed = conforming.handle(input(EXPORTED_ROOT, OVERLAY_DIR));
    let failed = violating.handle(input(EXPORTED_ROOT, OVERLAY_DIR));

    assert_eq!(passed.exit_code, 0, "a tree shipping only the overlay's supply passes");
    assert_eq!(passed.stderr, None, "a passing check reports no problem");
    assert!(
        passed.stdout.is_some_and(|message| message.contains("convention shipping check")),
        "a passing check still says what was checked"
    );
    assert_eq!(failed.exit_code, 1, "a tree shipping an unsupplied convention fails");
    assert_eq!(failed.stdout, None, "a failing check emits no result on the success stream");
}

#[test]
fn test_driver_reports_an_unanswerable_check_with_that_conditions_own_message() {
    // A tree that could not be inventoried is neither a pass nor a violation:
    // it reaches the failure arm carrying the condition's own message, so a
    // reader is told the check could not run rather than that the export is
    // clean.
    let (_service, driver) = driver_over(Answer::Unanswerable);

    let outcome = driver.handle(input(EXPORTED_ROOT, OVERLAY_DIR));

    assert_eq!(outcome.exit_code, 1);
    assert_eq!(outcome.stdout, None, "a check that could not run produces no result");
    assert_eq!(
        outcome.stderr.as_deref(),
        Some(unanswerable().to_string().as_str()),
        "the service's own condition is what reaches the failure arm, unrestated by the driver"
    );
}

#[test]
fn test_driver_reaches_no_operation_other_than_the_check() {
    // The injected check service is the driver's only collaborator, so every
    // effect a command of this family can have is one of these checks: exporting
    // a tree, writing into one, or repairing one has no route out of here.
    let (service, driver) = driver_over(Answer::Verdict(ConventionShippingVerdict::Conforming));

    let _ = driver.handle(input(EXPORTED_ROOT, OVERLAY_DIR));
    let _ = driver.handle(input("/tmp/second-export", OVERLAY_DIR));

    assert_eq!(
        service.queries().len(),
        2,
        "each command is answered by exactly one check request and nothing else"
    );
}
