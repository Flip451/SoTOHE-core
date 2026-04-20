//! Strict merge gate orchestration (hexagonal usecase layer).
//!
//! This module implements the merge-time strict spec signal gate as a pure
//! workflow orchestrated through a [`TrackBlobReader`] port. The usecase layer
//! does not touch the filesystem or git directly — infrastructure adapters
//! (e.g. `GitShowTrackBlobReader`) implement the port to read decoded domain
//! documents.
//!
//! Two-mode design (ADR §D8.0): the merge gate is always **strict** (Yellow
//! is blocked). The companion CI path (`verify_from_spec_json`) runs in
//! interim mode with Yellow warnings. Both paths delegate to the same pure
//! domain functions (`check_spec_doc_signals` / `check_type_signals`).
//!
//! Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md`
//! §D2, §D5.2, §D6, §D8.

use domain::spec::{SpecDocument, check_spec_doc_signals};
use domain::validate_branch_ref;
use domain::verify::{VerifyFinding, VerifyOutcome};
use domain::{TrackId, TrackMetadata};
use domain::{TypeCatalogueDocument, check_type_signals};

/// Result of a port-level blob fetch.
///
/// Infrastructure adapters translate their native errors (git spawn errors,
/// UTF-8 decode errors, JSON decode errors, non-path-not-found git errors)
/// into [`BlobFetchResult::FetchError`], and path-not-found cases into
/// [`BlobFetchResult::NotFound`] so the usecase can apply opt-in semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobFetchResult<T> {
    /// The blob was found and decoded into a domain document.
    Found(T),
    /// The blob does not exist at the requested path on the target ref.
    NotFound,
    /// An I/O, decode, or adapter-level error occurred. The string carries
    /// a human-readable description for the caller's error output.
    FetchError(String),
}

/// Usecase port for reading track-level domain documents from an external
/// source (git ref, filesystem, etc).
///
/// Infrastructure implementations are responsible for:
/// - Mapping their native I/O errors to [`BlobFetchResult::FetchError`]
/// - Distinguishing path-not-found from other errors (NotFound vs FetchError)
/// - Decoding raw bytes into domain aggregates (`SpecDocument`,
///   `TypeCatalogueDocument`)
/// - Any locale / stderr-parsing / symlink-rejection concerns that are
///   specific to the adapter implementation
///
/// The port contract deliberately returns domain types rather than raw bytes,
/// keeping the usecase layer decoupled from serde and codec details.
pub trait TrackBlobReader {
    /// Reads and decodes `track/items/<track_id>/spec.json` for the given
    /// `branch` (e.g. `"track/foo-2026-04-12"`).
    fn read_spec_document(&self, branch: &str, track_id: &str) -> BlobFetchResult<SpecDocument>;

    /// Reads and decodes the TDDD catalogue file for a single layer on the
    /// given branch.
    ///
    /// T007 (Phase 1 Task 7): accepts a `layer_id` so the merge-gate
    /// multilayer loop can read each layer's catalogue (`domain-types.json`,
    /// `usecase-types.json`, …). Returns `NotFound` when the file does not
    /// exist on the target ref — this corresponds to "TDDD not active for
    /// this layer" per ADR §D2.1.
    ///
    /// The `Found` variant returns `(doc, catalogue_file)` where
    /// `catalogue_file` is the resolved filename the adapter actually read
    /// (honoring `architecture-rules.json` `tddd.catalogue_file` overrides).
    /// Downstream diagnostics (e.g. `check_type_signals`) must use this
    /// resolved name, not a layer-id derived default, so error messages
    /// point at the real file on disk.
    fn read_type_catalogue(
        &self,
        branch: &str,
        track_id: &str,
        layer_id: &str,
    ) -> BlobFetchResult<(TypeCatalogueDocument, String)>;

    /// Reads and decodes `track/items/<track_id>/metadata.json` into a
    /// [`TrackMetadata`] aggregate.
    ///
    /// Used by the task-completion gate (see `usecase::task_completion`)
    /// which checks that all tasks are resolved before merge.
    fn read_track_metadata(&self, branch: &str, track_id: &str) -> BlobFetchResult<TrackMetadata>;

    /// Returns the list of TDDD-enabled layer ids on the given branch.
    ///
    /// The infrastructure adapter reads `architecture-rules.json` from the
    /// PR branch blob (not the local workspace) so that tracks which modify
    /// `architecture-rules.json` itself are evaluated against the PR's own
    /// rules. A default implementation returns `["domain"]` for backward
    /// compatibility with mocks that have not been updated.
    fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>> {
        BlobFetchResult::Found(vec!["domain".to_string()])
    }
}

/// Evaluates the strict merge gate for the given branch using the provided
/// [`TrackBlobReader`].
///
/// This is the **strict** entry point: Yellow signals are always blocked.
/// The merge gate is the only caller; interim mode lives in the CI path
/// (`verify_from_spec_json` with `strict=false`).
///
/// # Behavior
///
/// 1. Reject `plan/` branches (gate does not apply — they carry no code tasks).
/// 2. Run [`validate_branch_ref`] on the branch name (fail-closed on dangerous
///    characters — `..`, `@{`, `~`, `^`, `:`, whitespace, control chars).
/// 3. Derive `track_id`: for `track/` branches, strip the prefix and validate
///    the suffix against [`TrackId`] slug rules (fail-closed on empty suffix,
///    uppercase letters, `//`, etc.); non-`track/` branches fall back to the
///    full branch name as a best-effort passthrough.
/// 4. Read `spec.json` via the reader:
///    - `Found(doc)` → delegate to [`check_spec_doc_signals`] with `strict=true`
///    - `NotFound` → BLOCKED (spec.json is required for every track)
///    - `FetchError` → BLOCKED
/// 5. If Stage 1 passes, read `domain-types.json`:
///    - `Found(doc)` → delegate to [`check_type_signals`] with `strict=true`
///    - `NotFound` → skip (TDDD opt-in)
///    - `FetchError` → BLOCKED
///
/// Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` §D5.2.
#[must_use]
pub fn check_strict_merge_gate(branch: &str, reader: &impl TrackBlobReader) -> VerifyOutcome {
    // 1. plan/ branches skip the gate entirely (D6)
    if branch.starts_with("plan/") {
        return VerifyOutcome::pass();
    }

    // 2. Branch-name validation (D4.2, D5.2)
    if let Err(err) = validate_branch_ref(branch) {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "invalid branch ref: {err}"
        ))]);
    }

    // 3. Derive and validate track_id (fail-closed on malformed track/ suffix).
    //    For track/ branches, the suffix must be a valid TrackId slug (lowercase,
    //    digits, hyphens; non-empty; no consecutive hyphens; no trailing hyphen).
    //    Non-track/ branches fall back to the full branch name (best-effort passthrough).
    let track_id = if let Some(suffix) = branch.strip_prefix("track/") {
        if let Err(err) = TrackId::try_new(suffix) {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "invalid track id derived from branch '{branch}': {err}"
            ))]);
        }
        suffix
    } else {
        branch
    };

    // 4. Stage 1: spec.json is required (D5.2).
    let spec_doc = match reader.read_spec_document(branch, track_id) {
        BlobFetchResult::Found(doc) => doc,
        BlobFetchResult::NotFound => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "spec.json not found on origin/{branch} — every track must have a spec.json"
            ))]);
        }
        BlobFetchResult::FetchError(msg) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "failed to read spec.json on origin/{branch}: {msg}"
            ))]);
        }
    };

    let stage1 = check_spec_doc_signals(&spec_doc, /* strict */ true);
    if stage1.has_errors() {
        return stage1;
    }

    // 5. Stage 2: multi-layer TDDD gate. T007 — loop every `tddd.enabled`
    //    layer read from `architecture-rules.json` on the PR branch blob.
    //    For each layer:
    //      - NotFound → TDDD opt-out for that layer (no finding)
    //      - FetchError → fail-closed
    //      - Found → run `check_type_signals` with strict=true
    //    All findings are merged (AND-aggregation across layers) so one
    //    diagnostic shows every problem.
    let layer_ids = match reader.read_enabled_layers(branch) {
        BlobFetchResult::Found(ids) => {
            if ids.is_empty() {
                // Fail-closed: architecture-rules.json parses but no layers
                // are `tddd.enabled = true`. Skipping Stage 2 would let a
                // PR that disables every layer bypass strict gating. The
                // caller must enable at least one layer (or explicitly
                // delete the file, which is caught by the `NotFound` arm).
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "architecture-rules.json on origin/{branch} declares no tddd.enabled \
                     layers — the strict merge gate cannot verify an empty layer set"
                ))]);
            }
            ids
        }
        BlobFetchResult::NotFound => {
            // Fail-closed: a PR branch that removes or renames
            // `architecture-rules.json` must not be able to bypass Stage 2
            // enforcement. The strict merge gate always requires the file
            // to exist so that the enabled-layer set is auditable on the PR
            // branch itself (ADR 0002 D1 + strict-signal-gate-v2 §D5.2).
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "architecture-rules.json not found on origin/{branch} — \
                 the strict merge gate requires the file to exist to enumerate TDDD layers"
            ))]);
        }
        BlobFetchResult::FetchError(msg) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "failed to read architecture-rules.json on origin/{branch}: {msg}"
            ))]);
        }
    };

    let mut outcome = stage1;
    for layer_id in &layer_ids {
        match reader.read_type_catalogue(branch, track_id, layer_id) {
            BlobFetchResult::NotFound => {
                // TDDD opt-out for this layer — skip silently.
            }
            BlobFetchResult::FetchError(msg) => {
                // Diagnostic uses the layer_id rather than a hardcoded
                // `{layer_id}-types.json` filename: when a layer overrides its
                // `tddd.catalogue_file` (e.g. `custom-types.json`) the
                // FetchError variant does not return the resolved filename, so
                // the adapter's `msg` already carries the actual path. Prefixing
                // with `{layer_id}-types.json` here would point maintainers at a
                // file that does not exist. See `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md`
                // §D9 TDDD-BUG-02 for the broader catalogue-filename contract.
                outcome.merge(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "failed to read catalogue for layer '{layer_id}' on origin/{branch}: {msg}"
                ))]));
            }
            BlobFetchResult::Found((dt_doc, catalogue_file)) => {
                outcome.merge(check_type_signals(&dt_doc, /* strict */ true, &catalogue_file));
            }
        }
    }
    outcome
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::cell::RefCell;

    use domain::spec::SpecScope;
    use domain::tddd::catalogue::{TypeAction, TypeCatalogueEntry, TypeDefinitionKind, TypeSignal};
    use domain::{ConfidenceSignal, SignalCounts};

    use super::*;

    /// Mock reader that returns pre-programmed outcomes for the two document types.
    struct MockTrackBlobReader {
        spec: RefCell<Option<BlobFetchResult<SpecDocument>>>,
        /// `Some(result)` → return result when called; `None` → panic (unreachable assertion).
        dt: RefCell<Option<BlobFetchResult<TypeCatalogueDocument>>>,
        /// When `true`, calling `read_type_catalogue` panics with a clear message,
        /// making the short-circuit contract directly observable in tests.
        dt_unreachable: bool,
    }

    impl MockTrackBlobReader {
        fn new(
            spec: BlobFetchResult<SpecDocument>,
            dt: BlobFetchResult<TypeCatalogueDocument>,
        ) -> Self {
            Self {
                spec: RefCell::new(Some(spec)),
                dt: RefCell::new(Some(dt)),
                dt_unreachable: false,
            }
        }

        /// Shortcut for tests that must assert Stage 2 is never reached.
        ///
        /// If `read_type_catalogue` is called, the test panics immediately,
        /// making regressions in the short-circuit logic observable.
        fn with_unreachable_dt(spec: BlobFetchResult<SpecDocument>) -> Self {
            Self { spec: RefCell::new(Some(spec)), dt: RefCell::new(None), dt_unreachable: true }
        }
    }

    impl TrackBlobReader for MockTrackBlobReader {
        fn read_spec_document(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<SpecDocument> {
            self.spec.borrow_mut().take().expect("spec read called twice")
        }

        fn read_type_catalogue(
            &self,
            _branch: &str,
            _track_id: &str,
            layer_id: &str,
        ) -> BlobFetchResult<(TypeCatalogueDocument, String)> {
            if self.dt_unreachable {
                panic!("Stage 2 must not be reached: read_type_catalogue was called unexpectedly");
            }
            match self.dt.borrow_mut().take().expect("dt read called twice") {
                BlobFetchResult::Found(doc) => {
                    BlobFetchResult::Found((doc, format!("{layer_id}-types.json")))
                }
                BlobFetchResult::NotFound => BlobFetchResult::NotFound,
                BlobFetchResult::FetchError(msg) => BlobFetchResult::FetchError(msg),
            }
        }

        fn read_track_metadata(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<TrackMetadata> {
            // merge_gate tests don't exercise this port method;
            // task_completion tests (T007) use a separate mock.
            panic!("read_track_metadata must not be called by merge_gate tests")
        }
    }

    /// Recording mock that captures the branch and track_id arguments passed by
    /// `check_strict_merge_gate`, allowing tests to assert the port contract:
    /// - `branch` is passed verbatim (no stripping)
    /// - `track_id` has the `track/` prefix stripped
    struct RecordingTrackBlobReader {
        spec_result: BlobFetchResult<SpecDocument>,
        recorded_spec_branch: RefCell<Option<String>>,
        recorded_spec_track_id: RefCell<Option<String>>,
        recorded_dt_branch: RefCell<Option<String>>,
        recorded_dt_track_id: RefCell<Option<String>>,
    }

    impl RecordingTrackBlobReader {
        fn new(spec_result: BlobFetchResult<SpecDocument>) -> Self {
            Self {
                spec_result,
                recorded_spec_branch: RefCell::new(None),
                recorded_spec_track_id: RefCell::new(None),
                recorded_dt_branch: RefCell::new(None),
                recorded_dt_track_id: RefCell::new(None),
            }
        }
    }

    impl TrackBlobReader for RecordingTrackBlobReader {
        fn read_spec_document(
            &self,
            branch: &str,
            track_id: &str,
        ) -> BlobFetchResult<SpecDocument> {
            *self.recorded_spec_branch.borrow_mut() = Some(branch.to_owned());
            *self.recorded_spec_track_id.borrow_mut() = Some(track_id.to_owned());
            self.spec_result.clone()
        }

        fn read_type_catalogue(
            &self,
            branch: &str,
            track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<(TypeCatalogueDocument, String)> {
            *self.recorded_dt_branch.borrow_mut() = Some(branch.to_owned());
            *self.recorded_dt_track_id.borrow_mut() = Some(track_id.to_owned());
            BlobFetchResult::NotFound
        }

        fn read_track_metadata(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<TrackMetadata> {
            panic!("read_track_metadata must not be called by merge_gate tests")
        }
    }

    // --- Helpers to construct domain aggregates ---

    fn spec_doc_with_signals(signals: Option<SignalCounts>) -> SpecDocument {
        let mut doc = SpecDocument::new(
            "Feature",
            "1.0",
            vec![],
            SpecScope::new(Vec::new(), Vec::new()),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
        )
        .unwrap();
        if let Some(counts) = signals {
            doc.set_signals(counts);
        }
        doc
    }

    fn all_blue_spec() -> SpecDocument {
        spec_doc_with_signals(Some(SignalCounts::new(5, 0, 0)))
    }

    fn make_entry(name: &str) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(
            name,
            "test",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap()
    }

    fn make_signal(name: &str, signal: ConfidenceSignal) -> TypeSignal {
        TypeSignal::new(name, "value_object", signal, true, Vec::new(), Vec::new(), Vec::new())
    }

    fn dt_all_blue() -> TypeCatalogueDocument {
        let mut doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        doc.set_signals(vec![make_signal("TrackId", ConfidenceSignal::Blue)]);
        doc
    }

    fn dt_with_yellow() -> TypeCatalogueDocument {
        let mut doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        doc.set_signals(vec![make_signal("TrackId", ConfidenceSignal::Yellow)]);
        doc
    }

    fn dt_with_red() -> TypeCatalogueDocument {
        let mut doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        doc.set_signals(vec![make_signal("TrackId", ConfidenceSignal::Red)]);
        doc
    }

    // --- U1–U18 test matrix ---

    #[test]
    fn test_u1_spec_all_blue_dt_not_found_passes() {
        // U1: spec=Found(all-Blue), dt=NotFound → PASS (Stage 2 opt-out)
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::NotFound,
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(!outcome.has_errors(), "{outcome:?}");
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_u2_spec_blue_dt_all_blue_passes() {
        // U2: spec=all-Blue, dt=all-Blue → PASS
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::Found(dt_all_blue()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(!outcome.has_errors(), "{outcome:?}");
    }

    #[test]
    fn test_u3_spec_blue_dt_yellow_blocks_in_strict() {
        // U3: spec=Blue, dt=declared Yellow → BLOCKED (strict)
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::Found(dt_with_yellow()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("Yellow")));
    }

    #[test]
    fn test_u4_spec_blue_dt_red_blocks() {
        // U4: spec=Blue, dt=Red → BLOCKED
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::Found(dt_with_red()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("Red")));
    }

    #[test]
    fn test_u5_spec_blue_dt_empty_entries_blocks() {
        // U5: spec=Blue, dt=empty entries → BLOCKED
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::Found(TypeCatalogueDocument::new(1, Vec::new())),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_u6_spec_blue_dt_coverage_gap_blocks() {
        // U6: spec=Blue, dt has entry with no matching signal → BLOCKED
        let mut doc =
            TypeCatalogueDocument::new(1, vec![make_entry("TrackId"), make_entry("Other")]);
        doc.set_signals(vec![make_signal("TrackId", ConfidenceSignal::Blue)]);
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::Found(doc),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_u7_spec_blue_dt_signals_none_blocks() {
        // U7: spec=Blue, dt=None (unevaluated) → BLOCKED
        let doc = TypeCatalogueDocument::new(1, vec![make_entry("TrackId")]);
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::Found(doc),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_u8_spec_blue_dt_fetch_error_blocks() {
        // U8: spec=Blue, dt=FetchError → BLOCKED
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::FetchError("git show failed".to_owned()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors());
        // FetchError message identifies the layer by id, not a hardcoded filename —
        // the adapter's msg already carries the resolved path.
        assert!(
            outcome
                .findings()
                .iter()
                .any(|f| f.message().contains("failed to read catalogue for layer 'domain'"))
        );
    }

    #[test]
    fn test_u9_spec_yellow_blocks_in_strict() {
        // U9: spec=Yellow (Stage 1 strict) → BLOCKED
        let reader = MockTrackBlobReader::with_unreachable_dt(BlobFetchResult::Found(
            spec_doc_with_signals(Some(SignalCounts::new(3, 2, 0))),
        ));
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("yellow")));
    }

    #[test]
    fn test_u10_spec_red_blocks() {
        // U10: spec=Red → BLOCKED
        let reader = MockTrackBlobReader::with_unreachable_dt(BlobFetchResult::Found(
            spec_doc_with_signals(Some(SignalCounts::new(2, 0, 1))),
        ));
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("red")));
    }

    #[test]
    fn test_u11_spec_signals_none_blocks() {
        // U11: spec signals=None → BLOCKED
        let reader = MockTrackBlobReader::with_unreachable_dt(BlobFetchResult::Found(
            spec_doc_with_signals(None),
        ));
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_u12_spec_signals_all_zero_blocks() {
        // U12: spec signals=(0,0,0) → BLOCKED (treated as unevaluated)
        let reader = MockTrackBlobReader::with_unreachable_dt(BlobFetchResult::Found(
            spec_doc_with_signals(Some(SignalCounts::new(0, 0, 0))),
        ));
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_u13_spec_not_found_blocks() {
        // U13: spec=NotFound → BLOCKED (Stage 1 required)
        let reader = MockTrackBlobReader::with_unreachable_dt(BlobFetchResult::NotFound);
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("spec.json")));
    }

    #[test]
    fn test_u14_spec_fetch_error_blocks() {
        // U14: spec=FetchError → BLOCKED
        let reader = MockTrackBlobReader::with_unreachable_dt(BlobFetchResult::FetchError(
            "git show failed for spec.json".to_owned(),
        ));
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_u15_plan_branch_passes_without_reading() {
        // U15: plan/ branch → PASS (D6 skip)
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::FetchError("spec must not be read for plan/ branch".to_owned()),
            BlobFetchResult::FetchError("dt must not be read".to_owned()),
        );
        let outcome = check_strict_merge_gate("plan/dummy", &reader);
        assert!(!outcome.has_errors());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_u16_branch_with_double_dot_blocks() {
        // U16: branch contains `..` → validate_branch_ref rejects
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::FetchError("must not read".to_owned()),
            BlobFetchResult::FetchError("must not read".to_owned()),
        );
        let outcome = check_strict_merge_gate("track/feature/foo..bar", &reader);
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("invalid branch ref")));
    }

    #[test]
    fn test_u17_branch_with_reflog_expr_blocks() {
        // U17: branch contains `@{` → rejected
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::FetchError("must not read".to_owned()),
            BlobFetchResult::FetchError("must not read".to_owned()),
        );
        let outcome = check_strict_merge_gate("track/feature/foo@{0}", &reader);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_u18_empty_branch_blocks() {
        // U18: empty branch name → rejected (Empty variant)
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::FetchError("must not read".to_owned()),
            BlobFetchResult::FetchError("must not read".to_owned()),
        );
        let outcome = check_strict_merge_gate("", &reader);
        assert!(outcome.has_errors());
    }

    // --- Port contract: argument passing ---

    #[test]
    fn test_port_contract_branch_passed_verbatim_and_track_id_stripped() {
        // Verifies that check_strict_merge_gate:
        // - passes the original branch name verbatim to the reader (no stripping)
        // - strips the "track/" prefix when computing track_id for the reader
        let reader = RecordingTrackBlobReader::new(BlobFetchResult::Found(all_blue_spec()));
        let outcome = check_strict_merge_gate("track/some-feature-2026-04-12", &reader);

        // Should PASS (all-blue spec, dt NotFound)
        assert!(!outcome.has_errors(), "{outcome:?}");

        // Stage 1: branch passed verbatim
        assert_eq!(
            reader.recorded_spec_branch.borrow().as_deref(),
            Some("track/some-feature-2026-04-12"),
            "spec read must receive the original branch, not the track_id-stripped form"
        );
        // Stage 1: track_id has "track/" stripped
        assert_eq!(
            reader.recorded_spec_track_id.borrow().as_deref(),
            Some("some-feature-2026-04-12"),
            "spec read must receive track_id with 'track/' prefix stripped"
        );

        // Stage 2: same branch/track_id contract (NotFound was returned so it was reached)
        assert_eq!(
            reader.recorded_dt_branch.borrow().as_deref(),
            Some("track/some-feature-2026-04-12"),
            "dt read must receive the original branch"
        );
        assert_eq!(
            reader.recorded_dt_track_id.borrow().as_deref(),
            Some("some-feature-2026-04-12"),
            "dt read must receive track_id with 'track/' prefix stripped"
        );
    }

    #[test]
    fn test_port_contract_non_track_branch_passed_as_is() {
        // A branch without "track/" prefix: track_id falls back to the full branch name.
        let reader = RecordingTrackBlobReader::new(BlobFetchResult::Found(all_blue_spec()));
        let _ = check_strict_merge_gate("feature/no-prefix", &reader);

        assert_eq!(reader.recorded_spec_branch.borrow().as_deref(), Some("feature/no-prefix"),);
        // strip_prefix("track/") returns None → falls back to the full branch
        assert_eq!(reader.recorded_spec_track_id.borrow().as_deref(), Some("feature/no-prefix"),);
    }

    // --- Track-id validation (step 3) ---

    #[test]
    fn test_track_bare_suffix_empty_blocks() {
        // "track/" has an empty suffix → invalid track_id → BLOCKED
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::FetchError("must not read".to_owned()),
            BlobFetchResult::FetchError("must not read".to_owned()),
        );
        let outcome = check_strict_merge_gate("track/", &reader);
        assert!(outcome.has_errors());
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("invalid track id")),
            "expected 'invalid track id' in findings: {:?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_track_suffix_with_uppercase_blocks() {
        // "track/FooBar" has uppercase chars → invalid track_id → BLOCKED
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::FetchError("must not read".to_owned()),
            BlobFetchResult::FetchError("must not read".to_owned()),
        );
        let outcome = check_strict_merge_gate("track/FooBar", &reader);
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("invalid track id")));
    }

    #[test]
    fn test_track_suffix_with_double_slash_blocks() {
        // "track//foo" has empty first segment → invalid track_id → BLOCKED
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::FetchError("must not read".to_owned()),
            BlobFetchResult::FetchError("must not read".to_owned()),
        );
        let outcome = check_strict_merge_gate("track//foo", &reader);
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("invalid track id")));
    }

    // ===============================================================
    // U19–U26 — multilayer merge gate tests (T007)
    //
    // A `MultiLayerMock` returns per-layer catalogue outcomes keyed by
    // `layer_id`, and also drives `read_enabled_layers`. The 8 scenarios
    // below exercise the AND-aggregation of findings across 2 layers.
    // ===============================================================

    struct MultiLayerMock {
        spec: BlobFetchResult<SpecDocument>,
        enabled_layers: BlobFetchResult<Vec<String>>,
        catalogues: std::collections::HashMap<String, BlobFetchResult<TypeCatalogueDocument>>,
    }

    impl MultiLayerMock {
        fn new(
            spec: BlobFetchResult<SpecDocument>,
            enabled_layers: Vec<String>,
            catalogues: Vec<(&str, BlobFetchResult<TypeCatalogueDocument>)>,
        ) -> Self {
            Self {
                spec,
                enabled_layers: BlobFetchResult::Found(enabled_layers),
                catalogues: catalogues.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
            }
        }

        fn with_enabled_layer_error(spec: BlobFetchResult<SpecDocument>, error: &str) -> Self {
            Self {
                spec,
                enabled_layers: BlobFetchResult::FetchError(error.to_owned()),
                catalogues: std::collections::HashMap::new(),
            }
        }
    }

    impl TrackBlobReader for MultiLayerMock {
        fn read_spec_document(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<SpecDocument> {
            self.spec.clone()
        }

        fn read_type_catalogue(
            &self,
            _branch: &str,
            _track_id: &str,
            layer_id: &str,
        ) -> BlobFetchResult<(TypeCatalogueDocument, String)> {
            match self.catalogues.get(layer_id).cloned().unwrap_or(BlobFetchResult::NotFound) {
                BlobFetchResult::Found(doc) => {
                    BlobFetchResult::Found((doc, format!("{layer_id}-types.json")))
                }
                BlobFetchResult::NotFound => BlobFetchResult::NotFound,
                BlobFetchResult::FetchError(msg) => BlobFetchResult::FetchError(msg),
            }
        }

        fn read_track_metadata(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<TrackMetadata> {
            panic!("read_track_metadata must not be called by merge_gate tests")
        }

        fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>> {
            self.enabled_layers.clone()
        }
    }

    #[test]
    fn test_u19_two_layers_both_not_found_passes() {
        let reader = MultiLayerMock::new(
            BlobFetchResult::Found(all_blue_spec()),
            vec!["domain".to_string(), "usecase".to_string()],
            vec![],
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(!outcome.has_errors(), "both layers NotFound must pass: {outcome:?}");
    }

    #[test]
    fn test_u20_two_layers_both_all_blue_passes() {
        let reader = MultiLayerMock::new(
            BlobFetchResult::Found(all_blue_spec()),
            vec!["domain".to_string(), "usecase".to_string()],
            vec![
                ("domain", BlobFetchResult::Found(dt_all_blue())),
                ("usecase", BlobFetchResult::Found(dt_all_blue())),
            ],
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(!outcome.has_errors(), "both all-Blue must pass: {outcome:?}");
    }

    #[test]
    fn test_u21_two_layers_one_blue_one_red_blocks() {
        let reader = MultiLayerMock::new(
            BlobFetchResult::Found(all_blue_spec()),
            vec!["domain".to_string(), "usecase".to_string()],
            vec![
                ("domain", BlobFetchResult::Found(dt_all_blue())),
                ("usecase", BlobFetchResult::Found(dt_with_red())),
            ],
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors(), "Red in usecase must block: {outcome:?}");
    }

    #[test]
    fn test_u22_two_layers_one_not_found_one_blue_passes() {
        let reader = MultiLayerMock::new(
            BlobFetchResult::Found(all_blue_spec()),
            vec!["domain".to_string(), "usecase".to_string()],
            vec![("domain", BlobFetchResult::Found(dt_all_blue()))],
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(
            !outcome.has_errors(),
            "NotFound for usecase + Blue for domain must pass: {outcome:?}"
        );
    }

    #[test]
    fn test_u23_two_layers_one_yellow_one_blue_blocks_strict() {
        let reader = MultiLayerMock::new(
            BlobFetchResult::Found(all_blue_spec()),
            vec!["domain".to_string(), "usecase".to_string()],
            vec![
                ("domain", BlobFetchResult::Found(dt_all_blue())),
                ("usecase", BlobFetchResult::Found(dt_with_yellow())),
            ],
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(
            outcome.has_errors(),
            "Yellow in strict mode must block even with the other layer Blue: {outcome:?}"
        );
    }

    #[test]
    fn test_u24_two_layers_one_fetch_error_one_blue_blocks() {
        let reader = MultiLayerMock::new(
            BlobFetchResult::Found(all_blue_spec()),
            vec!["domain".to_string(), "usecase".to_string()],
            vec![
                ("domain", BlobFetchResult::Found(dt_all_blue())),
                ("usecase", BlobFetchResult::FetchError("network".to_string())),
            ],
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors(), "FetchError must block: {outcome:?}");
        assert!(
            outcome
                .findings()
                .iter()
                .any(|f| f.message().contains("failed to read catalogue for layer 'usecase'")),
            "error message must mention the failing layer: {outcome:?}"
        );
    }

    #[test]
    fn test_u25_read_enabled_layers_not_found_fails_closed() {
        // Fail-closed: removing / renaming architecture-rules.json on the
        // PR branch must not bypass Stage 2 enforcement. The strict merge
        // gate reports an error that mentions the missing file.
        let reader = MultiLayerMock {
            spec: BlobFetchResult::Found(all_blue_spec()),
            enabled_layers: BlobFetchResult::NotFound,
            catalogues: std::collections::HashMap::new(),
        };
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(
            outcome.has_errors(),
            "architecture-rules.json NotFound must fail-closed: {outcome:?}"
        );
        assert!(
            outcome
                .findings()
                .iter()
                .any(|f| f.message().contains("architecture-rules.json not found")),
            "error must mention architecture-rules.json: {outcome:?}"
        );
    }

    #[test]
    fn test_u26_read_enabled_layers_fetch_error_blocks() {
        let reader = MultiLayerMock::with_enabled_layer_error(
            BlobFetchResult::Found(all_blue_spec()),
            "git show failed",
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors(), "architecture-rules.json FetchError must block");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("architecture-rules.json")),
            "error must mention architecture-rules.json: {outcome:?}"
        );
    }

    // ===============================================================
    // U27-U30: tddd-02 usecase-enablement-focused scenarios.
    //
    // These scenarios treat `usecase` as the foreground layer. They assert
    // that the gate behaves symmetrically when the roles of `domain` and
    // `usecase` are swapped versus the U19-U26 baselines, and add one new
    // fail-closed scenario (empty enabled_layers list) that U19-U26 did
    // not cover.
    // ===============================================================

    #[test]
    fn test_u27_usecase_blue_domain_not_found_passes() {
        // Symmetric to U22 (which has domain Blue + usecase NotFound).
        // Confirms that opt-out on either layer works in isolation when the
        // other layer is all-Blue.
        let reader = MultiLayerMock::new(
            BlobFetchResult::Found(all_blue_spec()),
            vec!["domain".to_string(), "usecase".to_string()],
            vec![("usecase", BlobFetchResult::Found(dt_all_blue()))],
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(
            !outcome.has_errors(),
            "NotFound for domain + Blue for usecase must pass: {outcome:?}"
        );
    }

    #[test]
    fn test_u28_usecase_red_domain_not_found_blocks() {
        // Asserts that domain opt-out (NotFound) does not mask a Red on the
        // usecase layer. This is a usecase-forward version of "any Red blocks",
        // distinct from U21 which pairs Red with Blue.
        let reader = MultiLayerMock::new(
            BlobFetchResult::Found(all_blue_spec()),
            vec!["domain".to_string(), "usecase".to_string()],
            vec![("usecase", BlobFetchResult::Found(dt_with_red()))],
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(
            outcome.has_errors(),
            "Red in usecase must block even when domain is NotFound: {outcome:?}"
        );
    }

    #[test]
    fn test_u29_usecase_yellow_domain_not_found_blocks_strict() {
        // Strict-mode Yellow blocking, with domain opted out. This is distinct
        // from U23 (Yellow paired with Blue) because it verifies that an
        // opt-out on one layer does not downgrade strict-mode enforcement on
        // another layer's Yellow signal.
        let reader = MultiLayerMock::new(
            BlobFetchResult::Found(all_blue_spec()),
            vec!["domain".to_string(), "usecase".to_string()],
            vec![("usecase", BlobFetchResult::Found(dt_with_yellow()))],
        );
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(
            outcome.has_errors(),
            "Yellow in usecase must block in strict mode even when domain is NotFound: {outcome:?}"
        );
    }

    #[test]
    fn test_u30_empty_enabled_layers_list_fails_closed() {
        // Empty enabled_layers list (every layer `tddd.enabled=false` in
        // architecture-rules.json) must fail-closed with a clear error that
        // mentions architecture-rules.json. This is distinct from U25
        // (architecture-rules.json NotFound) because here the file exists
        // and parses, but yields no enabled layers. The gate must not
        // silently pass on the assumption that "no layers are enabled means
        // nothing to check" — fail-closed prevents a configuration-level
        // bypass of Stage 2 enforcement.
        let reader = MultiLayerMock::new(BlobFetchResult::Found(all_blue_spec()), vec![], vec![]);
        let outcome = check_strict_merge_gate("track/foo", &reader);
        assert!(outcome.has_errors(), "empty enabled_layers must fail-closed: {outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("architecture-rules.json")),
            "fail-closed error must mention architecture-rules.json: {outcome:?}"
        );
    }
}
