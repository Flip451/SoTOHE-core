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

use std::collections::HashMap;

use domain::spec::{SpecDocument, check_spec_doc_signals};
use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::validate_branch_ref;
use domain::verify::{VerifyFinding, VerifyOutcome};
use domain::{
    AdrVerifyReport, CatalogueSpecSignalsDocument, ChainId, ContentHash, GateKind,
    ImplPlanDocument, SignalGateMatrix, Strictness, TrackId,
};
use domain::{TypeSignalsDocument, check_type_signals};

use crate::catalogue_spec_refs::SpecElementHashReader;

#[path = "merge_gate/blob_fetch_result.rs"]
mod blob_fetch_result;
pub use blob_fetch_result::BlobFetchResult;

#[path = "merge_gate/chain2_gate.rs"]
mod chain2_gate;

/// Usecase port for reading track-level domain documents from an external
/// source (git ref, filesystem, etc).
///
/// Infrastructure implementations are responsible for:
/// - Mapping their native I/O errors to [`BlobFetchResult::FetchError`]
/// - Distinguishing path-not-found from other errors (NotFound vs FetchError)
/// - Decoding raw bytes into domain aggregates (`SpecDocument`,
///   `CatalogueDocument`)
/// - Any locale / stderr-parsing / symlink-rejection concerns that are
///   specific to the adapter implementation
///
/// The port contract deliberately returns domain types rather than raw bytes,
/// keeping the usecase layer decoupled from serde and codec details.
pub trait TrackBlobReader {
    /// Reads and decodes `track/items/<track_id>/spec.json` for the given
    /// `branch` (e.g. `"track/foo-2026-04-12"`).
    fn read_spec_document(&self, branch: &str, track_id: &str) -> BlobFetchResult<SpecDocument>;

    /// Reads the raw bytes of the TDDD catalogue file for a single layer on
    /// the given branch and returns them alongside a pre-computed SHA-256 hex
    /// digest of those bytes.
    ///
    /// Accepts a `layer_id` so the merge-gate multilayer loop can read each
    /// layer's catalogue (`domain-types.json`, `usecase-types.json`, …).
    /// Returns `NotFound` when the file does not exist on the target ref —
    /// this corresponds to "TDDD not active for this layer" per ADR §D2.1.
    ///
    /// The `Found` variant returns `(raw_bytes, declaration_hash)` where
    /// `declaration_hash` is the 64-character lowercase hex SHA-256 digest
    /// of `raw_bytes` (the same digest stored in `<layer>-type-signals.json`
    /// under `declaration_hash`). Callers use it for freshness checks against
    /// `TypeSignalsDocument::declaration_hash()` without re-hashing in the
    /// usecase layer. The raw bytes are carried for callers that need them;
    /// Stage 2 (`check_strict_merge_gate`) only uses the hash.
    ///
    /// Returns `(raw_bytes, declaration_hash)`: the raw catalogue bytes and
    /// their pre-computed 64-character lowercase hex SHA-256. The freshness
    /// check (`declaration_hash` comparison against the signal file) is the
    /// only contract this port fulfills for the type-signal gate.
    fn read_type_catalogue(
        &self,
        branch: &str,
        track_id: &str,
        layer_id: &str,
    ) -> BlobFetchResult<(Vec<u8>, String)>;

    /// Reads and decodes `track/items/<track_id>/impl-plan.json` for the given
    /// `branch`. Returns `NotFound` when the file does not exist on the target
    /// ref — this corresponds to "impl-plan.json not yet generated" and the
    /// caller must decide whether this is a fatal condition.
    ///
    /// A default implementation panics so existing mocks that do not override
    /// it surface the gap explicitly. Mocks used by task-completion tests must
    /// override this method.
    fn read_impl_plan(&self, branch: &str, track_id: &str) -> BlobFetchResult<ImplPlanDocument>;

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

    /// Reads and decodes the `<layer>-types.json` catalogue (v3 native) for
    /// the purpose of catalogue-spec ref integrity checking (ADR
    /// `2026-04-23-0344-catalogue-spec-signal-activation.md` §D2.2).
    ///
    /// Returns the v3 `CatalogueDocument` decoded via `CatalogueDocumentCodec::decode`
    /// alongside the 64-character lowercase hex SHA-256 of the on-disk bytes,
    /// and a per-entry hash map (`entry_name → SHA-256 of the entry's canonical JSON subtree`).
    /// Non-v3 catalogues surface as `BlobFetchResult::FetchError` (CN-11 fail-closed).
    ///
    /// The `Found` variant returns `(doc, raw_bytes_sha256_hex, entry_hashes)` where:
    /// - `raw_bytes_sha256_hex`: 64-character lowercase hex SHA-256 over the canonical
    ///   on-disk bytes of the catalogue. Callers convert this into a [`ContentHash`]
    ///   via [`ContentHash::try_from_hex`] and pass it to inline Chain ② integrity
    ///   checks as `current_catalogue_hash` for stale detection.
    /// - `entry_hashes`: maps each entry name (type / trait / function key) to its
    ///   per-entry SHA-256 computed in infrastructure via `canonical_json_sha256`
    ///   over the entry's canonical JSON subtree. Used by
    ///   [`crate::catalogue_spec_signals::RefreshCatalogueSpecSignalsInteractor`] to
    ///   inject `entry_hash: ContentHash` into each [`domain::CatalogueSpecSignal`]
    ///   (CN-04 / IN-05 / AC-06 of ADR `2026-05-27-1601`).
    ///
    /// A default implementation returns `NotFound` so opt-out test mocks
    /// skip silently without failing the merge gate. Real infrastructure
    /// implementations override.
    fn read_catalogue_for_spec_ref_check(
        &self,
        _branch: &str,
        _track_id: &str,
        _layer_id: &str,
    ) -> BlobFetchResult<(CatalogueDocument, String, HashMap<String, ContentHash>)> {
        // Default: NotFound so opt-out test mocks skip silently without failing
        // the merge gate. Real infrastructure implementations override.
        BlobFetchResult::NotFound
    }

    /// Reads and decodes the `<layer>-catalogue-spec-signals.json` file for
    /// the given layer on the target branch.
    ///
    /// Returns `NotFound` when the signals file has not been generated yet
    /// (expected for tracks before `sotp track catalogue-spec-signals` runs
    /// or for layers whose `catalogue_spec_signal.enabled` flag is false).
    /// Callers (verify / merge-gate) decide whether `NotFound` short-circuits
    /// to a finding or to `pass`.
    ///
    /// A default implementation returns `FetchError` so adapters that opt into
    /// the new port surface the gap explicitly. Implementations in
    /// `libs/infrastructure/` override this method in T011.
    fn read_catalogue_spec_signals_document(
        &self,
        _branch: &str,
        _track_id: &str,
        _layer_id: &str,
    ) -> BlobFetchResult<CatalogueSpecSignalsDocument> {
        // Default: NotFound so opt-out test mocks skip silently without failing
        // the merge gate. Real infrastructure implementations override.
        BlobFetchResult::NotFound
    }

    /// Returns the subset of TDDD-enabled layer ids that have also opted in
    /// to Chain ② evaluation via `tddd.catalogue_spec_signal.enabled = true`
    /// on the given branch.
    ///
    /// This is a strict subset of `read_enabled_layers`: every id returned
    /// here is also `tddd.enabled`, but not every `tddd.enabled` layer is in
    /// this set. The merge gate uses this list to avoid evaluating stale or
    /// mismatched signals files for layers that have intentionally opted out
    /// of Chain ② (for example a layer that previously generated a signals
    /// file and later flipped the flag to `false`). Keying Chain ② behavior
    /// on file presence alone would let such stale files block merges for a
    /// disabled feature.
    ///
    /// The default implementation returns an empty set so mocks that have
    /// not been updated simply skip Chain ②. Real infrastructure adapters
    /// override this to parse `architecture-rules.json` on the branch blob.
    fn read_catalogue_spec_signal_opted_in_layers(
        &self,
        _branch: &str,
    ) -> BlobFetchResult<Vec<String>> {
        BlobFetchResult::Found(Vec::new())
    }

    /// Reads and decodes `<layer>-type-signals.json` (chain-③ signals document,
    /// schema_version 1) for the given layer on the target branch.
    ///
    /// Returns the decoded [`TypeSignalsDocument`] directly — the document
    /// already carries its `declaration_hash` as a field, so no tuple is
    /// needed. Callers (T022's Stage 2 replacement) use the `declaration_hash`
    /// for freshness checks and the signal entries for `check_type_signals`.
    ///
    /// Returns:
    /// - `Found(doc)` when the signals file exists and decodes successfully.
    /// - `NotFound` when the signals file has not been generated yet (the layer
    ///   has TDDD enabled but `sotp track type-signals` has not run yet, or the
    ///   file was deleted).
    /// - `FetchError(msg)` on I/O, UTF-8, or JSON decode failure.
    ///
    /// A default implementation returns `FetchError` so mocks that have not
    /// been updated surface the gap explicitly. Infrastructure adapters
    /// override this method. Implemented in `GitShowTrackBlobReader` in T021.
    fn read_type_signals(
        &self,
        _branch: &str,
        _track_id: &str,
        _layer_id: &str,
    ) -> BlobFetchResult<TypeSignalsDocument> {
        BlobFetchResult::FetchError("read_type_signals not implemented".to_owned())
    }

    /// Scans ADR frontmatter for the given branch and returns a domain
    /// [`AdrVerifyReport`] without exposing a filesystem path to the usecase
    /// layer (Chain ⓪ / ADR §D2, D5).
    ///
    /// The infrastructure adapter runs the ADR signal scan internally
    /// (equivalent to `execute_verify_adr_signals` but returning the domain
    /// report rather than a `VerifyOutcome`) and returns the aggregate signal
    /// counts. The `branch` argument is provided so adapters that operate on
    /// git refs rather than the local filesystem can scope the scan to the
    /// correct branch blob if needed; adapters that always scan from the local
    /// workspace may ignore the argument.
    ///
    /// Returns:
    /// - `Found(report)` when the ADR scan succeeds.
    /// - `NotFound` when the ADR directory does not exist on the target ref.
    /// - `FetchError(msg)` on I/O or parse failure.
    ///
    /// A default implementation returns `NotFound` (treated as Chain ⓪ skip —
    /// no ADR directory means no decisions to evaluate) so that mocks that have
    /// not been updated continue to work without modification. `GitShowTrackBlobReader`
    /// overrides this in T006 to perform the actual ADR scan.
    fn read_adr_verify_report(&self, _branch: String) -> BlobFetchResult<AdrVerifyReport> {
        BlobFetchResult::NotFound
    }
}

/// Evaluates the strict merge gate for the given branch using the provided
/// [`TrackBlobReader`] and [`SignalGateMatrix`].
///
/// Strictness for each chain is resolved from `gate_matrix` at the
/// [`GateKind::Merge`] axis rather than being hardcoded. The caller (CLI
/// composition root) loads the matrix via `load_signal_gates_config` before
/// invoking this function; the usecase layer does not depend on infrastructure.
///
/// # Behavior
///
/// 1. Run [`validate_branch_ref`] on the branch name (fail-closed on dangerous
///    characters — `..`, `@{`, `~`, `^`, `:`, whitespace, control chars).
/// 2. Chain ⓪ (ADR → user): call `reader.read_adr_verify_report(branch)` and
///    evaluate ADR signal counts with `strict` resolved from
///    `gate_matrix.adr_user` at `GateKind::Merge`.
/// 3. Require a `track/` branch, then strip the prefix and validate the suffix
///    against [`TrackId`] slug rules (fail-closed on empty suffix, uppercase
///    letters, `//`, etc.).
/// 4. Read `spec.json` via the reader (Chain ①):
///    - `Found(doc)` → delegate to [`check_spec_doc_signals`] with `strict`
///      resolved from `gate_matrix.spec_adr` at `GateKind::Merge`
///    - `NotFound` → BLOCKED (spec.json is required for every track)
///    - `FetchError` → BLOCKED
/// 5. If Stage 1 passes, read `domain-types.json` (Chain ③, Stage 2):
///    - `Found(doc)` → delegate to [`check_type_signals`] with `strict`
///      resolved from `gate_matrix.impl_catalog` at `GateKind::Merge`
///    - `NotFound` → skip (TDDD opt-in)
///    - `FetchError` → BLOCKED
///
/// Reference: ADR `knowledge/adr/2026-06-16-1030-signal-gate-strictness-config.md`
/// §D2, §D5.
#[must_use]
pub fn check_strict_merge_gate<R>(
    branch: &str,
    reader: &R,
    gate_matrix: &SignalGateMatrix,
) -> VerifyOutcome
where
    R: TrackBlobReader + SpecElementHashReader,
{
    // 1. Branch-name validation (D4.2, D5.2). No reader port is called before
    // this guard because adapters may interpret `branch` as a git ref.
    if let Err(err) = validate_branch_ref(branch) {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "invalid branch ref: {err}"
        ))]);
    }

    // 2. Chain ⓪ (ADR → user): evaluate ADR signal counts with strictness
    //    resolved from gate_matrix.adr_user at GateKind::Merge. This is a soft
    //    dependency: Chain ⓪ does not gate the remaining stages — its findings
    //    are accumulated into `outcome` but do not short-circuit Chains ①②③.
    let adr_strict = gate_matrix.resolve(ChainId::AdrUser, GateKind::Merge) == Strictness::Strict;
    let chain0_outcome = match reader.read_adr_verify_report(branch.to_owned()) {
        BlobFetchResult::Found(report) => {
            crate::chain::adr_user::adr_report_to_outcome(&report, adr_strict)
        }
        BlobFetchResult::NotFound => {
            // ADR directory absent on the branch — treat as clean (no decisions
            // to evaluate). This is not a block: the ADR directory is optional
            // on branches that predate the ADR convention.
            VerifyOutcome::pass()
        }
        BlobFetchResult::FetchError(msg) => {
            // Fail-closed: an I/O error reading the ADR directory must block.
            VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "chain ⓪ (adr-user): failed to read ADR verify report: {msg}"
            ))])
        }
    };
    let with_chain0 = |mut outcome: VerifyOutcome| {
        outcome.merge(chain0_outcome.clone());
        outcome
    };

    // 3. Derive and validate track_id (fail-closed on non-track branches or a
    //    malformed track/ suffix).
    let Some(track_id) = branch.strip_prefix("track/") else {
        return with_chain0(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "strict merge gate requires a track/<id> branch (current: {branch})"
        ))]));
    };
    if let Err(err) = TrackId::try_new(track_id) {
        return with_chain0(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "invalid track id derived from branch '{branch}': {err}"
        ))]));
    }

    // 4. Stage 1: spec.json is required (D5.2). (Chain ①: spec → ADR)
    let spec_doc = match reader.read_spec_document(branch, track_id) {
        BlobFetchResult::Found(doc) => doc,
        BlobFetchResult::NotFound => {
            return with_chain0(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "spec.json not found on origin/{branch} — every track must have a spec.json"
            ))]));
        }
        BlobFetchResult::FetchError(msg) => {
            return with_chain0(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "failed to read spec.json on origin/{branch}: {msg}"
            ))]));
        }
    };

    let spec_adr_strict =
        gate_matrix.resolve(ChainId::SpecAdr, GateKind::Merge) == Strictness::Strict;
    let stage1 = check_spec_doc_signals(&spec_doc, spec_adr_strict);
    if stage1.has_errors() {
        return with_chain0(stage1);
    }

    // 4. Stage 2: multi-layer TDDD gate — loop every `tddd.enabled` layer
    //    read from `architecture-rules.json` on the PR branch blob.
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
                return with_chain0(VerifyOutcome::from_findings(vec![VerifyFinding::error(
                    format!(
                        "architecture-rules.json on origin/{branch} declares no tddd.enabled \
                     layers — the strict merge gate cannot verify an empty layer set"
                    ),
                )]));
            }
            ids
        }
        BlobFetchResult::NotFound => {
            // Fail-closed: a PR branch that removes or renames
            // `architecture-rules.json` must not be able to bypass Stage 2
            // enforcement. The strict merge gate always requires the file
            // to exist so that the enabled-layer set is auditable on the PR
            // branch itself (ADR 0002 D1 + strict-signal-gate-v2 §D5.2).
            return with_chain0(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "architecture-rules.json not found on origin/{branch} — \
                 the strict merge gate requires the file to exist to enumerate TDDD layers"
            ))]));
        }
        BlobFetchResult::FetchError(msg) => {
            return with_chain0(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "failed to read architecture-rules.json on origin/{branch}: {msg}"
            ))]));
        }
    };

    // Stage 2: multi-layer TDDD type-signal gate (v3-native, T022). (Chain ③: impl → catalogue)
    //
    // For each tddd.enabled layer:
    //   1. Read the catalogue bytes + pre-computed hash (`read_type_catalogue`).
    //      NotFound → TDDD opt-out for this layer (skip silently).
    //      FetchError → fail-closed.
    //   2. Read the type-signals document (`read_type_signals`).
    //      NotFound → fail-closed (signals file required when catalogue is present).
    //      FetchError → fail-closed.
    //   3. Compare `signals_doc.declaration_hash()` to the catalogue hash from
    //      step 1 — a mismatch means the signals file is stale (fail-closed, CN-11).
    //   4. Evaluate `check_type_signals(&signals_doc, strict)` with strictness
    //      resolved from gate_matrix.impl_catalog at GateKind::Merge.
    //
    // The catalogue's TDDD opt-out check (step 1 NotFound) preserves the pre-T022
    // behavior: a layer without a catalogue file is not enrolled in Stage 2.
    let impl_catalog_strict =
        gate_matrix.resolve(ChainId::ImplCatalog, GateKind::Merge) == Strictness::Strict;
    let mut outcome = stage1;
    for layer_id in &layer_ids {
        // Step 1: read catalogue bytes + hash (opt-out check and freshness data).
        let declaration_hash_from_catalogue =
            match reader.read_type_catalogue(branch, track_id, layer_id) {
                BlobFetchResult::NotFound => {
                    // TDDD opt-out for this layer — no catalogue → skip silently.
                    continue;
                }
                BlobFetchResult::FetchError(msg) => {
                    // Diagnostic uses the layer_id per TDDD-BUG-02 (catalogue-filename contract):
                    // the adapter's msg already carries the actual resolved path.
                    outcome.merge(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "failed to read catalogue for layer '{layer_id}' on origin/{branch}: {msg}"
                ))]));
                    continue;
                }
                BlobFetchResult::Found((_bytes, hash)) => hash,
            };

        // Step 2: read type-signals document.
        // Fail-closed: if the catalogue is present but the signals file is absent,
        // the gate cannot verify the type-signal state (symmetric with CI path per ADR §D5).
        let signals_doc = match reader.read_type_signals(branch, track_id, layer_id) {
            BlobFetchResult::Found(doc) => doc,
            BlobFetchResult::NotFound => {
                outcome.merge(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "type-signals file for layer '{layer_id}' not found on origin/{branch} — \
                     run `sotp track type-signals` and commit the generated file"
                ))]));
                continue;
            }
            BlobFetchResult::FetchError(msg) => {
                outcome.merge(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "failed to read type-signals for layer '{layer_id}' on origin/{branch}: {msg}"
                ))]));
                continue;
            }
        };

        // Step 3: freshness check — declaration_hash must match (CN-11).
        if signals_doc.declaration_hash() != declaration_hash_from_catalogue {
            outcome.merge(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "layer '{layer_id}': type-signals declaration_hash mismatch \
                 (recorded={}, current={}) — re-run `sotp track type-signals` \
                 and commit the refreshed evaluation result",
                signals_doc.declaration_hash(),
                declaration_hash_from_catalogue
            ))]));
            continue;
        }

        // Step 4: signal gate — strictness resolved from gate_matrix.impl_catalog.
        outcome.merge(check_type_signals(&signals_doc, impl_catalog_strict));
    }

    // Stage 3 (Chain ② — ADR §D3.6 / IN-14): catalogue-spec integrity binary
    // gate + signal gate. Runs AFTER Stage 1 (spec signals) + Stage 2 (type
    // signals) so that lower-layer failures short-circuit before paying for
    // per-layer spec-ref integrity checks. A full bottom-up reorder (Chain ③ →
    // ② → ①) per D3.6 is deferred — T017 ships the new gate logic at the end
    // of the existing order to avoid rewriting 30+ existing tests in the same
    // commit. The behavioural effect is identical when Stage 1 / 2 pass
    // cleanly (which is the success path the reorder optimises for).
    //
    // Short-circuit: if Stage 1 or Stage 2 already produced errors, Chain ②
    // would only add unrelated failures on top of a known-broken state. Return
    // early so the caller sees only the primary failure first.
    if outcome.has_errors() {
        return with_chain0(outcome);
    }

    // Per-layer Chain ② loop (ADR §D3.6 / briefing §Design Intent).
    //
    // Layer-id validation runs first (before the spec-hash opt-out gate) so
    // that a malformed layer id in `architecture-rules.json` is always reported,
    // even when the spec-hash reader is unavailable. This prevents a PR from
    // adding an invalid layer id that would silently bypass fail-closed checking
    // regardless of the spec-hash opt-out state.
    for layer_id in &layer_ids {
        if let Err(e) = domain::tddd::LayerId::try_new(layer_id) {
            outcome.merge(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "invalid layer id '{layer_id}' in architecture-rules.json on origin/{branch}: {e}"
            ))]));
        }
    }
    if outcome.has_errors() {
        return with_chain0(outcome);
    }

    // Opt-out gate: `read_spec_element_hashes` NotFound/FetchError means the
    // spec hash codec is unavailable → SKIP the whole Chain ② block.
    // Per the briefing: "if NotFound / FetchError, SKIP the whole Chain ② block
    // (catalogue-spec activation is per-layer opt-in and depends on having a
    // valid spec.json)." A real spec.json parse failure already surfaces in
    // Stage 1 via check_spec_doc_signals.
    let spec_element_hashes = match reader.read_spec_element_hashes(branch, track_id) {
        BlobFetchResult::Found(map) => map,
        BlobFetchResult::NotFound | BlobFetchResult::FetchError(_) => {
            return with_chain0(outcome); // hash codec unavailable → skip whole Chain ②
        }
    };

    // Per-layer opt-in gate (ADR §D5.4 phased activation): only layers whose
    // `tddd.catalogue_spec_signal.enabled = true` participate in Chain ②.
    // Presence of a committed `<layer>-catalogue-spec-signals.json` on the
    // branch is NOT sufficient on its own — a layer that was previously opted
    // in and later flipped the flag back to `false` may still carry a stale
    // signals file; blocking merges on it for a now-disabled feature would be
    // wrong.
    //
    // `FetchError` is fail-closed: `architecture-rules.json` was already
    // parsed successfully once in `read_enabled_layers` (the `layer_ids` set
    // above was produced from it), so a second-call failure is a transient /
    // systemic adapter issue. Returning an error here prevents the merge gate
    // from silently skipping Chain ② validation for opted-in layers when the
    // opt-in lookup fails (which would be fail-open).
    //
    // `NotFound` means "no rules file on the branch" — opt-in defaults to
    // empty, which skips Chain ② entirely. That is consistent with the default
    // trait impl's empty-Vec semantic for mocks, and with the fact that a PR
    // without `architecture-rules.json` would already have been rejected by
    // the Stage 2 `read_enabled_layers` fail-closed check above (this code is
    // unreachable on such a PR in practice; the empty-set fallback is just
    // defensive).
    let opted_in_layers: std::collections::HashSet<String> = match reader
        .read_catalogue_spec_signal_opted_in_layers(branch)
    {
        BlobFetchResult::Found(ids) => ids.into_iter().collect(),
        BlobFetchResult::NotFound => std::collections::HashSet::new(),
        BlobFetchResult::FetchError(msg) => {
            return with_chain0(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "failed to read catalogue-spec opt-in layers on origin/{branch}: {msg}"
            ))]));
        }
    };

    // Resolve strictness for Chain ② (catalog-spec) from gate_matrix.
    let catalog_spec_strict =
        gate_matrix.resolve(ChainId::CatalogSpec, GateKind::Merge) == Strictness::Strict;
    for layer_id in &layer_ids {
        // Skip layers that have NOT opted in to Chain ② — mere presence of a
        // signals file is insufficient (see the opted_in_layers comment above).
        if !opted_in_layers.contains(layer_id) {
            continue;
        }
        // Delegate the per-layer Chain ② check to the extracted helper.
        // Each call returns a VerifyOutcome that is merged into the accumulator.
        outcome.merge(chain2_gate::check_chain2_for_layer(
            reader,
            branch,
            track_id,
            layer_id,
            &spec_element_hashes,
            catalog_spec_strict,
        ));
    }

    // Merge Chain ⓪ (ADR → user) outcome last. Chain ⓪ findings are accumulated
    // independently of the other chains and do not short-circuit them; the gate
    // reports all findings together so the caller sees the complete picture.
    with_chain0(outcome)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::cell::RefCell;

    use domain::spec::SpecScope;
    use domain::verify::Severity;
    use domain::{ChainGateEntry, ConfidenceSignal, SignalCounts};

    use super::*;

    /// Returns an all-strict `SignalGateMatrix` for use in tests that do not
    /// specifically test per-chain strictness resolution.  Mirrors the
    /// recommended default from ADR §D3 (all merge-gate cells = strict).
    fn all_strict_matrix() -> SignalGateMatrix {
        let strict_entry =
            || ChainGateEntry { commit_gate: Strictness::Strict, merge_gate: Strictness::Strict };
        SignalGateMatrix {
            adr_user: strict_entry(),
            spec_adr: strict_entry(),
            catalog_spec: strict_entry(),
            impl_catalog: strict_entry(),
        }
    }

    fn catalog_spec_interim_matrix() -> SignalGateMatrix {
        let mut matrix = all_strict_matrix();
        matrix.catalog_spec.merge_gate = Strictness::Interim;
        matrix
    }

    fn adr_user_interim_matrix() -> SignalGateMatrix {
        let mut matrix = all_strict_matrix();
        matrix.adr_user.merge_gate = Strictness::Interim;
        matrix
    }

    fn spec_adr_interim_matrix() -> SignalGateMatrix {
        let mut matrix = all_strict_matrix();
        matrix.spec_adr.merge_gate = Strictness::Interim;
        matrix
    }

    fn impl_catalog_interim_matrix() -> SignalGateMatrix {
        let mut matrix = all_strict_matrix();
        matrix.impl_catalog.merge_gate = Strictness::Interim;
        matrix
    }

    /// Minimal SHA-256 hex of an all-zeroes byte sequence, used as a stable
    /// "fresh" hash for test signals documents that carry this hash.
    const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    /// Builds a minimal `TypeSignalsDocument` for use in Stage 2 mock tests.
    ///
    /// All signals carry the given `ConfidenceSignal`. The `declaration_hash`
    /// is set to [`ZERO_HASH`] so the companion catalogue mock (bytes of all
    /// zeros, or any fixture where the adapter returns `ZERO_HASH`) matches.
    fn signals_doc_with(entries: &[(&str, ConfidenceSignal)]) -> TypeSignalsDocument {
        let ts = domain::Timestamp::new("2026-05-08T00:00:00Z").unwrap();
        let sigs: Vec<domain::tddd::catalogue::TypeSignal> = entries
            .iter()
            .map(|(name, sig)| {
                domain::tddd::catalogue::TypeSignal::new(
                    *name,
                    "value_object",
                    *sig,
                    true,
                    vec![],
                    vec![],
                    vec![],
                )
            })
            .collect();
        TypeSignalsDocument::new(ts, ZERO_HASH, sigs)
    }

    /// Mock reader that returns pre-programmed outcomes for the two document types.
    ///
    /// T022: `dt` now carries `BlobFetchResult<TypeSignalsDocument>` for
    /// `read_type_signals`, while `read_type_catalogue` always returns bytes
    /// whose hash matches `ZERO_HASH` (allowing freshness checks to pass when
    /// the signals document was built with `ZERO_HASH` via `signals_doc_with`).
    struct MockTrackBlobReader {
        spec: RefCell<Option<BlobFetchResult<SpecDocument>>>,
        /// Outcome returned by `read_type_signals`. `None` means the call is
        /// unreachable (Stage 2 short-circuit test).
        dt: RefCell<Option<BlobFetchResult<TypeSignalsDocument>>>,
        /// When `true`, calling `read_type_signals` or `read_type_catalogue`
        /// panics, making the short-circuit contract directly observable in tests.
        dt_unreachable: bool,
    }

    impl MockTrackBlobReader {
        fn new(
            spec: BlobFetchResult<SpecDocument>,
            dt: BlobFetchResult<TypeSignalsDocument>,
        ) -> Self {
            Self {
                spec: RefCell::new(Some(spec)),
                dt: RefCell::new(Some(dt)),
                dt_unreachable: false,
            }
        }

        /// Shortcut for tests that must assert Stage 2 is never reached.
        ///
        /// If `read_type_signals` or `read_type_catalogue` is called, the test
        /// panics immediately, making regressions in the short-circuit logic
        /// observable.
        fn with_unreachable_dt(spec: BlobFetchResult<SpecDocument>) -> Self {
            Self { spec: RefCell::new(Some(spec)), dt: RefCell::new(None), dt_unreachable: true }
        }
    }

    impl SpecElementHashReader for MockTrackBlobReader {
        fn read_spec_element_hashes(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<std::collections::BTreeMap<domain::SpecElementId, ContentHash>>
        {
            BlobFetchResult::Found(std::collections::BTreeMap::new())
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
            _layer_id: &str,
        ) -> BlobFetchResult<(Vec<u8>, String)> {
            if self.dt_unreachable {
                panic!("Stage 2 must not be reached: read_type_catalogue was called unexpectedly");
            }
            // Return bytes whose hash equals ZERO_HASH so the freshness check
            // passes when the signals doc was built with `signals_doc_with`
            // (which also uses ZERO_HASH). Content of the bytes does not matter
            // for the gate logic — only the hash is compared.
            match self.dt.borrow().as_ref().map(|r| r.clone()) {
                Some(BlobFetchResult::NotFound) => BlobFetchResult::NotFound,
                Some(BlobFetchResult::FetchError(msg)) => BlobFetchResult::FetchError(msg),
                Some(BlobFetchResult::Found(_)) => {
                    // Catalogue present (signals found) → return bytes with ZERO_HASH.
                    BlobFetchResult::Found((vec![], ZERO_HASH.to_owned()))
                }
                None => {
                    if self.dt_unreachable {
                        panic!(
                            "Stage 2 must not be reached: read_type_catalogue was called unexpectedly"
                        );
                    }
                    panic!("read_type_catalogue called but dt is None and not unreachable")
                }
            }
        }

        fn read_type_signals(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<TypeSignalsDocument> {
            if self.dt_unreachable {
                panic!("Stage 2 must not be reached: read_type_signals was called unexpectedly");
            }
            self.dt.borrow_mut().take().expect("dt read called twice")
        }

        fn read_impl_plan(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<ImplPlanDocument> {
            panic!("read_impl_plan must not be called by merge_gate tests")
        }
    }

    /// Recording mock that captures the branch and track_id arguments passed by
    /// `check_strict_merge_gate`, allowing tests to assert the port contract:
    /// - `branch` is passed verbatim (no stripping)
    /// - `track_id` has the `track/` prefix stripped
    struct RecordingTrackBlobReader {
        spec_result: BlobFetchResult<SpecDocument>,
        adr_result: BlobFetchResult<AdrVerifyReport>,
        recorded_adr_branch: RefCell<Option<String>>,
        recorded_spec_branch: RefCell<Option<String>>,
        recorded_spec_track_id: RefCell<Option<String>>,
        recorded_dt_branch: RefCell<Option<String>>,
        recorded_dt_track_id: RefCell<Option<String>>,
    }

    impl RecordingTrackBlobReader {
        fn new(spec_result: BlobFetchResult<SpecDocument>) -> Self {
            Self {
                spec_result,
                adr_result: BlobFetchResult::NotFound,
                recorded_adr_branch: RefCell::new(None),
                recorded_spec_branch: RefCell::new(None),
                recorded_spec_track_id: RefCell::new(None),
                recorded_dt_branch: RefCell::new(None),
                recorded_dt_track_id: RefCell::new(None),
            }
        }

        fn with_adr_report(
            spec_result: BlobFetchResult<SpecDocument>,
            adr_report: AdrVerifyReport,
        ) -> Self {
            Self::with_adr_result(spec_result, BlobFetchResult::Found(adr_report))
        }

        fn with_adr_result(
            spec_result: BlobFetchResult<SpecDocument>,
            adr_result: BlobFetchResult<AdrVerifyReport>,
        ) -> Self {
            Self {
                spec_result,
                adr_result,
                recorded_adr_branch: RefCell::new(None),
                recorded_spec_branch: RefCell::new(None),
                recorded_spec_track_id: RefCell::new(None),
                recorded_dt_branch: RefCell::new(None),
                recorded_dt_track_id: RefCell::new(None),
            }
        }
    }

    impl SpecElementHashReader for RecordingTrackBlobReader {
        fn read_spec_element_hashes(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<std::collections::BTreeMap<domain::SpecElementId, ContentHash>>
        {
            BlobFetchResult::Found(std::collections::BTreeMap::new())
        }
    }

    impl TrackBlobReader for RecordingTrackBlobReader {
        fn read_adr_verify_report(&self, branch: String) -> BlobFetchResult<AdrVerifyReport> {
            *self.recorded_adr_branch.borrow_mut() = Some(branch);
            self.adr_result.clone()
        }

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
        ) -> BlobFetchResult<(Vec<u8>, String)> {
            *self.recorded_dt_branch.borrow_mut() = Some(branch.to_owned());
            *self.recorded_dt_track_id.borrow_mut() = Some(track_id.to_owned());
            BlobFetchResult::NotFound
        }

        fn read_impl_plan(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<ImplPlanDocument> {
            panic!("read_impl_plan must not be called by merge_gate tests")
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

    fn dt_all_blue() -> TypeSignalsDocument {
        signals_doc_with(&[("TrackId", ConfidenceSignal::Blue)])
    }

    fn dt_with_yellow() -> TypeSignalsDocument {
        signals_doc_with(&[("TrackId", ConfidenceSignal::Yellow)])
    }

    fn dt_with_red() -> TypeSignalsDocument {
        signals_doc_with(&[("TrackId", ConfidenceSignal::Red)])
    }

    // --- U3–U18 test matrix ---

    #[test]
    fn test_u3_spec_blue_dt_yellow_blocks_in_strict() {
        // U3: spec=Blue, dt=declared Yellow → BLOCKED (strict)
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::Found(dt_with_yellow()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("Yellow")));
    }

    #[test]
    fn test_chain3_type_yellow_warns_when_impl_catalog_merge_gate_is_interim() {
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::Found(dt_with_yellow()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &impl_catalog_interim_matrix());

        assert!(
            !outcome.has_errors(),
            "Yellow type signal must not block when impl-catalog merge gate is interim: {outcome:?}"
        );
        let finding = outcome
            .findings()
            .iter()
            .find(|f| f.message().contains("type(s) have Yellow signal"))
            .expect("expected an impl-catalog warning");
        assert_eq!(finding.severity(), Severity::Warning);
    }

    #[test]
    fn test_u4_spec_blue_dt_red_blocks() {
        // U4: spec=Blue, dt=Red → BLOCKED
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::Found(dt_with_red()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("Red")));
    }

    #[test]
    fn test_u5_spec_blue_dt_empty_signals_passes_per_adr_d64() {
        // U5 (T022): spec=all-Blue, type-signals empty → PASS
        // ADR 2026-04-19-1242 §D6.4: empty signal list corresponds to an empty
        // catalogue (zero declarations). Valid for tracks that reuse pre-existing types.
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::Found(signals_doc_with(&[])),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.findings().is_empty(), "empty signals must pass per D6.4: {outcome:?}");
    }

    #[test]
    fn test_u6_spec_blue_dt_not_found_passes_opt_out() {
        // U6 (T022): spec=Blue, catalogue=NotFound → PASS (TDDD opt-out).
        // NotFound on read_type_catalogue means this layer has not enrolled in TDDD.
        // Consistent with the pre-T022 behavior.
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::NotFound,
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        // catalogue NotFound → skip → no findings from Stage 2
        assert!(!outcome.has_errors(), "TDDD opt-out (catalogue NotFound) must pass: {outcome:?}");
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_u7_spec_blue_dt_all_blue_passes() {
        // U7 (T022): catalogue present + all-Blue signals → PASS.
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::Found(dt_all_blue()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(!outcome.has_errors(), "all-Blue type-signals must pass: {outcome:?}");
    }

    #[test]
    fn test_u8_spec_blue_dt_catalogue_fetch_error_blocks() {
        // U8 (T022): spec=Blue, catalogue FetchError → BLOCKED.
        // FetchError on read_type_catalogue is fail-closed.
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::FetchError("git show failed".to_owned()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors(), "catalogue FetchError must block: {outcome:?}");
        // FetchError message identifies the layer by id, not a hardcoded filename.
        assert!(
            outcome
                .findings()
                .iter()
                .any(|f| f.message().contains("failed to read catalogue for layer 'domain'")),
            "findings: {outcome:?}"
        );
    }

    #[test]
    fn test_u9_spec_yellow_blocks_in_strict() {
        // U9: spec=Yellow (Stage 1 strict) → BLOCKED
        let reader = MockTrackBlobReader::with_unreachable_dt(BlobFetchResult::Found(
            spec_doc_with_signals(Some(SignalCounts::new(3, 2, 0))),
        ));
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("yellow")));
    }

    #[test]
    fn test_chain1_spec_yellow_warns_when_spec_adr_merge_gate_is_interim() {
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::Found(spec_doc_with_signals(Some(SignalCounts::new(3, 2, 0)))),
            BlobFetchResult::NotFound,
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &spec_adr_interim_matrix());

        assert!(
            !outcome.has_errors(),
            "Yellow spec signal must not block when spec-ADR merge gate is interim: {outcome:?}"
        );
        let finding = outcome
            .findings()
            .iter()
            .find(|f| f.message().contains("spec.json: 2 yellow signal"))
            .expect("expected a spec-ADR warning");
        assert_eq!(finding.severity(), Severity::Warning);
    }

    #[test]
    fn test_u10_spec_red_blocks() {
        // U10: spec=Red → BLOCKED
        let reader = MockTrackBlobReader::with_unreachable_dt(BlobFetchResult::Found(
            spec_doc_with_signals(Some(SignalCounts::new(2, 0, 1))),
        ));
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("red")));
    }

    #[test]
    fn test_u11_spec_signals_none_blocks() {
        // U11: spec signals=None → BLOCKED
        let reader = MockTrackBlobReader::with_unreachable_dt(BlobFetchResult::Found(
            spec_doc_with_signals(None),
        ));
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_u12_spec_signals_all_zero_blocks() {
        // U12: spec signals=(0,0,0) → BLOCKED (treated as unevaluated)
        let reader = MockTrackBlobReader::with_unreachable_dt(BlobFetchResult::Found(
            spec_doc_with_signals(Some(SignalCounts::new(0, 0, 0))),
        ));
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_u13_spec_not_found_blocks() {
        // U13: spec=NotFound → BLOCKED (Stage 1 required)
        let reader = MockTrackBlobReader::with_unreachable_dt(BlobFetchResult::NotFound);
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("spec.json")));
    }

    #[test]
    fn test_u14_spec_fetch_error_blocks() {
        // U14: spec=FetchError → BLOCKED
        let reader = MockTrackBlobReader::with_unreachable_dt(BlobFetchResult::FetchError(
            "git show failed for spec.json".to_owned(),
        ));
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_u16_branch_with_double_dot_blocks() {
        // U16: branch contains `..` → validate_branch_ref rejects
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::FetchError("must not read".to_owned()),
            BlobFetchResult::FetchError("must not read".to_owned()),
        );
        let outcome =
            check_strict_merge_gate("track/feature/foo..bar", &reader, &all_strict_matrix());
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
        let outcome =
            check_strict_merge_gate("track/feature/foo@{0}", &reader, &all_strict_matrix());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_u18_empty_branch_blocks() {
        // U18: empty branch name → rejected (Empty variant)
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::FetchError("must not read".to_owned()),
            BlobFetchResult::FetchError("must not read".to_owned()),
        );
        let outcome = check_strict_merge_gate("", &reader, &all_strict_matrix());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_invalid_branch_ref_blocks_before_adr_reader() {
        let reader = RecordingTrackBlobReader::new(BlobFetchResult::Found(all_blue_spec()));
        let outcome =
            check_strict_merge_gate("track/feature/foo..bar", &reader, &all_strict_matrix());

        assert!(outcome.has_errors());
        assert_eq!(reader.recorded_adr_branch.borrow().as_deref(), None);
        assert_eq!(reader.recorded_spec_branch.borrow().as_deref(), None);
    }

    // --- Port contract: argument passing ---

    #[test]
    fn test_port_contract_branch_passed_verbatim_and_track_id_stripped() {
        // Verifies that check_strict_merge_gate:
        // - passes the original branch name verbatim to the reader (no stripping)
        // - strips the "track/" prefix when computing track_id for the reader
        let reader = RecordingTrackBlobReader::new(BlobFetchResult::Found(all_blue_spec()));
        let outcome =
            check_strict_merge_gate("track/some-feature-2026-04-12", &reader, &all_strict_matrix());

        // Should PASS (all-blue spec, dt NotFound)
        assert!(!outcome.has_errors(), "{outcome:?}");

        // Stage 1: branch passed verbatim
        assert_eq!(
            reader.recorded_adr_branch.borrow().as_deref(),
            Some("track/some-feature-2026-04-12"),
            "ADR read must receive the original branch"
        );
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
    fn test_non_track_branch_blocks_without_reading() {
        let reader = RecordingTrackBlobReader::new(BlobFetchResult::Found(all_blue_spec()));
        let outcome = check_strict_merge_gate("plan/no-prefix", &reader, &all_strict_matrix());

        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("track/<id> branch")));
        assert_eq!(reader.recorded_spec_branch.borrow().as_deref(), None);
        assert_eq!(reader.recorded_spec_track_id.borrow().as_deref(), None);
    }

    #[test]
    fn test_chain0_findings_are_merged_when_spec_not_found_blocks() {
        let reader = RecordingTrackBlobReader::with_adr_report(
            BlobFetchResult::NotFound,
            AdrVerifyReport::new(0, 1, 0, 0),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());

        assert!(outcome.has_errors(), "{outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("spec.json not found")),
            "expected the Stage 1 spec failure: {outcome:?}"
        );
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("ADR decision")),
            "expected Chain ⓪ ADR finding to survive the early return: {outcome:?}"
        );
    }

    #[test]
    fn test_chain0_yellow_warns_when_adr_user_merge_gate_is_interim() {
        let reader = RecordingTrackBlobReader::with_adr_report(
            BlobFetchResult::Found(all_blue_spec()),
            AdrVerifyReport::new(0, 1, 0, 0),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &adr_user_interim_matrix());

        assert!(
            !outcome.has_errors(),
            "Yellow ADR-user signal must not block when adr-user merge gate is interim: {outcome:?}"
        );
        let finding = outcome
            .findings()
            .iter()
            .find(|f| f.message().contains("ADR decision(s) have Yellow signal"))
            .expect("expected an ADR-user warning");
        assert_eq!(finding.severity(), Severity::Warning);
    }

    #[test]
    fn test_chain0_fetch_error_blocks_even_when_other_chains_are_clean() {
        let reader = RecordingTrackBlobReader::with_adr_result(
            BlobFetchResult::Found(all_blue_spec()),
            BlobFetchResult::FetchError("ADR parse failed".to_owned()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());

        assert!(outcome.has_errors(), "{outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains(
                "chain ⓪ (adr-user): failed to read ADR verify report: ADR parse failed"
            )),
            "expected fail-closed ADR read error: {outcome:?}"
        );
    }

    // --- Track-id validation (step 2) ---

    #[test]
    fn test_track_bare_suffix_empty_blocks() {
        // "track/" has an empty suffix → invalid track_id → BLOCKED
        let reader = MockTrackBlobReader::new(
            BlobFetchResult::FetchError("must not read".to_owned()),
            BlobFetchResult::FetchError("must not read".to_owned()),
        );
        let outcome = check_strict_merge_gate("track/", &reader, &all_strict_matrix());
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
        let outcome = check_strict_merge_gate("track/FooBar", &reader, &all_strict_matrix());
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
        let outcome = check_strict_merge_gate("track//foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors());
        assert!(outcome.findings().iter().any(|f| f.message().contains("invalid track id")));
    }

    // ===============================================================
    // U19–U26 — multilayer merge gate tests
    //
    // A `MultiLayerMock` returns per-layer catalogue outcomes keyed by
    // `layer_id`, and also drives `read_enabled_layers`. The 8 scenarios
    // below exercise the AND-aggregation of findings across 2 layers.
    // ===============================================================

    struct MultiLayerMock {
        spec: BlobFetchResult<SpecDocument>,
        enabled_layers: BlobFetchResult<Vec<String>>,
        /// Keyed by layer_id. Values are `TypeSignalsDocument` outcomes returned
        /// by `read_type_signals`. When a layer is found here, `read_type_catalogue`
        /// returns `Found((vec![], ZERO_HASH))` so the freshness check passes.
        catalogues: std::collections::HashMap<String, BlobFetchResult<TypeSignalsDocument>>,
    }

    impl MultiLayerMock {
        fn new(
            spec: BlobFetchResult<SpecDocument>,
            enabled_layers: Vec<String>,
            catalogues: Vec<(&str, BlobFetchResult<TypeSignalsDocument>)>,
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

    impl SpecElementHashReader for MultiLayerMock {
        fn read_spec_element_hashes(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<std::collections::BTreeMap<domain::SpecElementId, ContentHash>>
        {
            BlobFetchResult::Found(std::collections::BTreeMap::new())
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
        ) -> BlobFetchResult<(Vec<u8>, String)> {
            match self.catalogues.get(layer_id).cloned().unwrap_or(BlobFetchResult::NotFound) {
                BlobFetchResult::Found(_) => {
                    // Return bytes with ZERO_HASH so the freshness check passes.
                    BlobFetchResult::Found((vec![], ZERO_HASH.to_owned()))
                }
                BlobFetchResult::NotFound => BlobFetchResult::NotFound,
                BlobFetchResult::FetchError(msg) => BlobFetchResult::FetchError(msg),
            }
        }

        fn read_type_signals(
            &self,
            _branch: &str,
            _track_id: &str,
            layer_id: &str,
        ) -> BlobFetchResult<TypeSignalsDocument> {
            self.catalogues.get(layer_id).cloned().unwrap_or(BlobFetchResult::NotFound)
        }

        fn read_impl_plan(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<ImplPlanDocument> {
            panic!("read_impl_plan must not be called by merge_gate tests")
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
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
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
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
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
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors(), "Red in usecase must block: {outcome:?}");
    }

    #[test]
    fn test_u22_two_layers_one_not_found_one_blue_passes() {
        let reader = MultiLayerMock::new(
            BlobFetchResult::Found(all_blue_spec()),
            vec!["domain".to_string(), "usecase".to_string()],
            vec![("domain", BlobFetchResult::Found(dt_all_blue()))],
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
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
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
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
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
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
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
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
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
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
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
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
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
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
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
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
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors(), "empty enabled_layers must fail-closed: {outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("architecture-rules.json")),
            "fail-closed error must mention architecture-rules.json: {outcome:?}"
        );
    }

    // ===============================================================
    // U31–U35 — Chain ② (catalogue-spec integrity + signal gate) tests.
    //
    // A dedicated `ChainTwoMock` controls the signals / catalogue / hash
    // readers for Chain ②. Stage 1 always returns an all-Blue spec and
    // Stage 2 always returns NotFound (TDDD opt-out) so the test isolates
    // Chain ② behaviour exclusively.
    // ===============================================================

    struct ChainTwoMock {
        signals: BlobFetchResult<domain::CatalogueSpecSignalsDocument>,
        catalogue: BlobFetchResult<(
            domain::tddd::catalogue_v2::CatalogueDocument,
            String,
            HashMap<String, ContentHash>,
        )>,
        spec_hashes:
            BlobFetchResult<std::collections::BTreeMap<domain::SpecElementId, ContentHash>>,
    }

    impl ChainTwoMock {
        /// Build a mock where Stage 1/2 succeed (Blue spec, TDDD opt-out for
        /// Stage 2) and Chain ② reads are controlled via arguments.
        fn new(
            signals: BlobFetchResult<domain::CatalogueSpecSignalsDocument>,
            catalogue: BlobFetchResult<(
                domain::tddd::catalogue_v2::CatalogueDocument,
                String,
                HashMap<String, ContentHash>,
            )>,
            spec_hashes: BlobFetchResult<
                std::collections::BTreeMap<domain::SpecElementId, ContentHash>,
            >,
        ) -> Self {
            Self { signals, catalogue, spec_hashes }
        }
    }

    impl SpecElementHashReader for ChainTwoMock {
        fn read_spec_element_hashes(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<std::collections::BTreeMap<domain::SpecElementId, ContentHash>>
        {
            self.spec_hashes.clone()
        }
    }

    impl TrackBlobReader for ChainTwoMock {
        fn read_spec_document(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<SpecDocument> {
            BlobFetchResult::Found(all_blue_spec())
        }

        /// T022: Stage 2 opt-out (NotFound) so that Chain ② tests are isolated.
        /// Returns `(Vec<u8>, String)` as required by the new trait signature.
        fn read_type_catalogue(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<(Vec<u8>, String)> {
            BlobFetchResult::NotFound // Stage 2 opt-out — isolates Chain ②
        }

        fn read_impl_plan(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<domain::ImplPlanDocument> {
            panic!("read_impl_plan must not be called by Chain ② tests")
        }

        fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>> {
            BlobFetchResult::Found(vec!["domain".to_string()])
        }

        fn read_catalogue_spec_signal_opted_in_layers(
            &self,
            _branch: &str,
        ) -> BlobFetchResult<Vec<String>> {
            // These tests exercise Chain ② actively, so the mocked layer must be
            // opted in. The real infrastructure adapter derives this subset from
            // `architecture-rules.json` via
            // `TdddLayerBinding::catalogue_spec_signal_enabled()`.
            BlobFetchResult::Found(vec!["domain".to_string()])
        }

        fn read_catalogue_spec_signals_document(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<domain::CatalogueSpecSignalsDocument> {
            self.signals.clone()
        }

        fn read_catalogue_for_spec_ref_check(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<(
            domain::tddd::catalogue_v2::CatalogueDocument,
            String,
            HashMap<String, ContentHash>,
        )> {
            self.catalogue.clone()
        }
    }

    fn all_yellow_signals() -> domain::CatalogueSpecSignalsDocument {
        domain::CatalogueSpecSignalsDocument::new(
            ContentHash::from_bytes([0xcd_u8; 32]),
            vec![domain::CatalogueSpecSignal::new(
                "TrackId",
                domain::ConfidenceSignal::Yellow,
                ContentHash::from_bytes([0u8; 32]),
            )],
        )
    }

    fn all_blue_catalogue_signals() -> domain::CatalogueSpecSignalsDocument {
        domain::CatalogueSpecSignalsDocument::new(
            ContentHash::from_bytes([0xcd_u8; 32]),
            vec![domain::CatalogueSpecSignal::new(
                "TrackId",
                domain::ConfidenceSignal::Blue,
                ContentHash::from_bytes([0u8; 32]),
            )],
        )
    }

    /// A 64-char lowercase hex string of repeating `byte` (for catalogue_hash_hex).
    fn hex64(byte: u8) -> String {
        format!("{:02x}", byte).repeat(32)
    }

    /// Build a v3 `CatalogueDocument` with a single `TrackId` `ValueObject` entry.
    /// Shared construction used by both `catalogue_doc_with_entry_hashes` and
    /// `catalogue_with_trackid_entry` so the fixture stays in one place.
    fn trackid_catalogue_doc() -> domain::tddd::catalogue_v2::CatalogueDocument {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::CatalogueDocument;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::TypeEntry;
        use domain::tddd::catalogue_v2::identifiers::{CrateName, ModulePath, TypeName};
        use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer);
        doc.types.insert(
            TypeName::new("TrackId").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        doc
    }

    /// Build a catalogue result containing the `TrackId` entry paired with the
    /// `0xcd`-repeat declaration hash and the provided `entry_hashes` map.
    /// Used by U43/U44 to vary only the per-entry hash state.
    fn catalogue_doc_with_entry_hashes(
        entry_hashes: HashMap<String, ContentHash>,
    ) -> BlobFetchResult<(
        domain::tddd::catalogue_v2::CatalogueDocument,
        String,
        HashMap<String, ContentHash>,
    )> {
        BlobFetchResult::Found((trackid_catalogue_doc(), hex64(0xcd), entry_hashes))
    }

    /// Catalogue with a single `TrackId` entry matching `all_*_signals()` doc
    /// structure. Hash uses `0xcd` repeat so `declaration_hash` matches the
    /// signals document used in these tests.
    ///
    /// T024: now returns a v3-native `CatalogueDocument` with one type entry.
    fn catalogue_with_trackid_entry() -> BlobFetchResult<(
        domain::tddd::catalogue_v2::CatalogueDocument,
        String,
        HashMap<String, ContentHash>,
    )> {
        // Per-entry hash for `"types:TrackId"` must match the `entry_hash` stored
        // in the companion signals documents (`all_blue_catalogue_signals` and
        // `all_yellow_signals`), which both use `ContentHash::from_bytes([0u8; 32])`.
        // The per-entry hash freshness check (added alongside AC-06) will block if
        // this map is empty or supplies the wrong hash for an opted-in layer.
        let mut entry_hashes = HashMap::new();
        entry_hashes.insert("types:TrackId".to_owned(), ContentHash::from_bytes([0u8; 32]));
        catalogue_doc_with_entry_hashes(entry_hashes)
    }

    #[test]
    fn test_u31_chain2_signals_not_found_blocks_for_opted_in_layer() {
        // U31 (updated per PR #111 fail-open fix): Chain ② signals=NotFound
        // for an opted-in layer → BLOCKED. Previously this was treated as
        // silent opt-out, but that let a PR bypass Chain ② by deleting
        // `<layer>-catalogue-spec-signals.json`.
        let reader = ChainTwoMock::new(
            BlobFetchResult::NotFound,
            BlobFetchResult::NotFound,
            BlobFetchResult::Found(std::collections::BTreeMap::new()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors(), "signals NotFound on opted-in layer must block: {outcome:?}");
        assert!(
            outcome
                .findings()
                .iter()
                .any(|f| f.message().contains("missing <layer>-catalogue-spec-signals.json")),
            "error must explicitly name the missing signals file: {outcome:?}"
        );
    }

    #[test]
    fn test_u32_chain2_yellow_signal_blocks_strict() {
        // U32: Chain ② activated layer with Yellow signal → BLOCKED (strict=true)
        // Both signals and catalogue must be Found so the full Chain ② path runs.
        let reader = ChainTwoMock::new(
            BlobFetchResult::Found(all_yellow_signals()),
            catalogue_with_trackid_entry(),
            BlobFetchResult::Found(std::collections::BTreeMap::new()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(
            outcome.has_errors(),
            "Yellow catalogue-spec signal must block in strict mode: {outcome:?}"
        );
        // Error message must reference the catalogue file (not the signals file)
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("domain-types.json")),
            "error must mention the catalogue file 'domain-types.json', not the signals file: {outcome:?}"
        );
    }

    #[test]
    fn test_chain2_yellow_signal_warns_when_catalog_spec_merge_gate_is_interim() {
        let reader = ChainTwoMock::new(
            BlobFetchResult::Found(all_yellow_signals()),
            catalogue_with_trackid_entry(),
            BlobFetchResult::Found(std::collections::BTreeMap::new()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &catalog_spec_interim_matrix());

        assert!(
            !outcome.has_errors(),
            "Yellow catalogue-spec signal must not block in interim mode: {outcome:?}"
        );
        let finding = outcome
            .findings()
            .iter()
            .find(|f| f.message().contains("Yellow catalogue-spec signal"))
            .expect("expected a catalogue-spec warning");
        assert_eq!(finding.severity(), Severity::Warning);
    }

    #[test]
    fn test_u33_chain2_blue_signals_passes() {
        // U33: Chain ② activated layer with all-Blue signals → PASS
        // Catalogue is provided with a matching `TrackId` entry so the
        // coverage check in `check_catalogue_spec_signals` passes.
        let reader = ChainTwoMock::new(
            BlobFetchResult::Found(all_blue_catalogue_signals()),
            catalogue_with_trackid_entry(),
            BlobFetchResult::Found(std::collections::BTreeMap::new()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(!outcome.has_errors(), "all-Blue catalogue-spec signals must pass: {outcome:?}");
    }

    #[test]
    fn test_u34_chain2_signals_fetch_error_blocks() {
        // U34: Chain ② signals=FetchError → BLOCKED (fail-closed)
        let reader = ChainTwoMock::new(
            BlobFetchResult::FetchError("signals file corrupted".to_owned()),
            BlobFetchResult::NotFound,
            BlobFetchResult::Found(std::collections::BTreeMap::new()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors(), "signals FetchError must block: {outcome:?}");
        assert!(
            outcome
                .findings()
                .iter()
                .any(|f| f.message().contains("failed to read catalogue-spec signals")),
            "error must mention catalogue-spec signals read failure: {outcome:?}"
        );
    }

    #[test]
    fn test_u39_chain2_catalogue_not_found_blocks_for_opted_in_layer() {
        // U39 (PR #111 regression): Step 2 catalogue=NotFound for an opted-in
        // layer must block the merge even when Step 1 signals=Found. The
        // `continue` after the Step 2 NotFound arm still accumulates the error
        // into `outcome`, so this is fail-closed.
        //
        // Distinct from U31 (signals=NotFound): U31 never reaches Step 2 because
        // the Step 1 `continue` fires first. U39 exercises the Step 2 path by
        // providing valid signals but a missing catalogue.
        let reader = ChainTwoMock::new(
            BlobFetchResult::Found(all_blue_catalogue_signals()),
            BlobFetchResult::NotFound, // catalogue missing
            BlobFetchResult::Found(std::collections::BTreeMap::new()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(
            outcome.has_errors(),
            "catalogue NotFound on opted-in layer must block: {outcome:?}"
        );
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("missing its catalogue file")),
            "error must mention the missing catalogue file: {outcome:?}"
        );
    }

    /// Mock variant of `ChainTwoMock` where the layer is `tddd.enabled` (Stage 2)
    /// but NOT in `read_catalogue_spec_signal_opted_in_layers` (Chain ② opt-out).
    /// Used by the regression test covering the PR #111 finding: a stale signals
    /// file for an opted-out layer must not be re-evaluated by the merge gate.
    struct ChainTwoOptOutMock {
        signals: BlobFetchResult<domain::CatalogueSpecSignalsDocument>,
    }

    impl SpecElementHashReader for ChainTwoOptOutMock {
        fn read_spec_element_hashes(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<std::collections::BTreeMap<domain::SpecElementId, ContentHash>>
        {
            BlobFetchResult::Found(std::collections::BTreeMap::new())
        }
    }

    impl TrackBlobReader for ChainTwoOptOutMock {
        fn read_spec_document(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<SpecDocument> {
            BlobFetchResult::Found(all_blue_spec())
        }

        fn read_type_catalogue(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<(Vec<u8>, String)> {
            BlobFetchResult::NotFound // Stage 2 opt-out — isolates Chain ②
        }

        fn read_impl_plan(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<domain::ImplPlanDocument> {
            panic!("read_impl_plan must not be called")
        }

        fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>> {
            // Layer IS `tddd.enabled` → included in Stage 2 iteration.
            BlobFetchResult::Found(vec!["domain".to_string()])
        }

        fn read_catalogue_spec_signal_opted_in_layers(
            &self,
            _branch: &str,
        ) -> BlobFetchResult<Vec<String>> {
            // But NOT opted in for Chain ② → should be skipped in Stage 3.
            BlobFetchResult::Found(Vec::new())
        }

        fn read_catalogue_spec_signals_document(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<domain::CatalogueSpecSignalsDocument> {
            self.signals.clone()
        }

        /// Override to provide a valid catalogue+hash so the test is falsifiable:
        /// if the opt-in guard were removed, Stage 3 would proceed past signals to
        /// the catalogue read, parse the hash, run the signal gate, and then block
        /// on the Yellow signal. Without this override the default `NotFound` causes
        /// a `continue` before the signal gate is ever reached,
        /// making `test_u36` pass vacuously even when the guard is absent.
        fn read_catalogue_for_spec_ref_check(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<(
            domain::tddd::catalogue_v2::CatalogueDocument,
            String,
            HashMap<String, ContentHash>,
        )> {
            // Match the hash embedded in `all_yellow_signals()`:
            // ContentHash::from_bytes([0xcd; 32]) → hex64(0xcd)
            catalogue_with_trackid_entry()
        }
    }

    #[test]
    fn test_u36_chain2_opt_out_skips_even_when_signals_yellow() {
        // Regression: PR #111 found that a layer which previously emitted a
        // signals file and later flipped `catalogue_spec_signal.enabled = false`
        // would still be evaluated on file presence, blocking merges on stale
        // Yellow/Red findings for a disabled feature. The fix consults
        // `read_catalogue_spec_signal_opted_in_layers` and skips layers that
        // are not in the opt-in set — even when the signals file is Found and
        // Yellow (which would otherwise block in strict mode per U32).
        let reader = ChainTwoOptOutMock { signals: BlobFetchResult::Found(all_yellow_signals()) };
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(
            !outcome.has_errors(),
            "opted-out layer with stale Yellow signals must NOT block merge: {outcome:?}"
        );
    }

    /// Mock that returns `FetchError` specifically from
    /// `read_catalogue_spec_signal_opted_in_layers` while all earlier reads
    /// succeed. Exercises the fail-closed branch that blocks merges when the
    /// opt-in lookup itself cannot be resolved.
    struct ChainTwoOptInLookupErrorMock;

    impl SpecElementHashReader for ChainTwoOptInLookupErrorMock {
        fn read_spec_element_hashes(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<std::collections::BTreeMap<domain::SpecElementId, ContentHash>>
        {
            BlobFetchResult::Found(std::collections::BTreeMap::new())
        }
    }

    impl TrackBlobReader for ChainTwoOptInLookupErrorMock {
        fn read_spec_document(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<SpecDocument> {
            BlobFetchResult::Found(all_blue_spec())
        }

        fn read_type_catalogue(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<(Vec<u8>, String)> {
            BlobFetchResult::NotFound // Stage 2 opt-out — isolates Chain ②
        }

        fn read_impl_plan(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<domain::ImplPlanDocument> {
            panic!("read_impl_plan must not be called")
        }

        fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>> {
            BlobFetchResult::Found(vec!["domain".to_string()])
        }

        fn read_catalogue_spec_signal_opted_in_layers(
            &self,
            _branch: &str,
        ) -> BlobFetchResult<Vec<String>> {
            BlobFetchResult::FetchError(
                "architecture-rules.json transient read error (simulated)".to_owned(),
            )
        }
    }

    #[test]
    fn test_u38_chain2_opt_in_lookup_fetch_error_blocks_fail_closed() {
        // Regression: the opt-in lookup itself failing must block the merge
        // (fail-closed), not silently skip Chain ②. Treating `FetchError` as
        // an empty set here would be fail-open — an adapter-side transient
        // error on `architecture-rules.json` would silently disable Chain ②
        // for opted-in layers, which PR #111 explicitly flagged.
        let reader = ChainTwoOptInLookupErrorMock;
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(
            outcome.has_errors(),
            "opt-in lookup FetchError must block merge (fail-closed): {outcome:?}"
        );
        assert!(
            outcome
                .findings()
                .iter()
                .any(|f| f.message().contains("failed to read catalogue-spec opt-in layers")),
            "error must mention the opt-in lookup failure: {outcome:?}"
        );
    }

    #[test]
    fn test_u37_chain2_opt_out_skips_even_when_signals_fetch_error() {
        // Same opt-out gate: even a FetchError on a layer that is NOT opted in
        // must not surface — the signals file is irrelevant to a disabled
        // feature, so the adapter's fetch outcome should never be consulted.
        let reader = ChainTwoOptOutMock {
            signals: BlobFetchResult::FetchError("signals file corrupted".to_owned()),
        };
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(
            !outcome.has_errors(),
            "opted-out layer signals FetchError must NOT surface: {outcome:?}"
        );
    }

    // ===============================================================
    // U40–U41 — Stage 2 fail-closed: signals absent / stale hash.
    //
    // Both tests use a single-layer (domain) setup where the catalogue is
    // Found (layer is enrolled in Stage 2) but the signals file is either
    // absent (U40) or carries a stale declaration_hash (U41).  These are
    // the two fail-closed branches that live between Step 1 (opt-out check)
    // and Step 4 (signal gate) in the Stage 2 loop.
    // ===============================================================

    #[test]
    fn test_u40_stage2_catalogue_found_signals_not_found_blocks_fail_closed() {
        // U40: catalogue=Found, signals=NotFound → BLOCKED (fail-closed).
        // When the catalogue is present the layer is enrolled; a missing
        // signals file means `sotp track type-signals` has not been run, so
        // the gate cannot verify the type-signal state.
        struct Stage2SignalsNotFoundMock;
        impl SpecElementHashReader for Stage2SignalsNotFoundMock {
            fn read_spec_element_hashes(
                &self,
                _branch: &str,
                _track_id: &str,
            ) -> BlobFetchResult<std::collections::BTreeMap<domain::SpecElementId, ContentHash>>
            {
                BlobFetchResult::Found(std::collections::BTreeMap::new())
            }
        }
        impl TrackBlobReader for Stage2SignalsNotFoundMock {
            fn read_spec_document(
                &self,
                _branch: &str,
                _track_id: &str,
            ) -> BlobFetchResult<SpecDocument> {
                BlobFetchResult::Found(all_blue_spec())
            }
            fn read_type_catalogue(
                &self,
                _branch: &str,
                _track_id: &str,
                _layer_id: &str,
            ) -> BlobFetchResult<(Vec<u8>, String)> {
                // Catalogue is present → layer enrolled in Stage 2.
                BlobFetchResult::Found((vec![], ZERO_HASH.to_owned()))
            }
            fn read_type_signals(
                &self,
                _branch: &str,
                _track_id: &str,
                _layer_id: &str,
            ) -> BlobFetchResult<TypeSignalsDocument> {
                // Signals file absent → fail-closed.
                BlobFetchResult::NotFound
            }
            fn read_impl_plan(
                &self,
                _branch: &str,
                _track_id: &str,
            ) -> BlobFetchResult<domain::ImplPlanDocument> {
                panic!("read_impl_plan must not be called by U40")
            }
            fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>> {
                BlobFetchResult::Found(vec!["domain".to_string()])
            }
        }
        let reader = Stage2SignalsNotFoundMock;
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(
            outcome.has_errors(),
            "catalogue Found + signals NotFound must block (fail-closed): {outcome:?}"
        );
        assert!(
            outcome
                .findings()
                .iter()
                .any(|f| f.message().contains("type-signals file for layer 'domain' not found")),
            "error must mention the missing type-signals file for 'domain': {outcome:?}"
        );
    }

    #[test]
    fn test_u41_stage2_declaration_hash_mismatch_blocks_fail_closed() {
        // U41: catalogue=Found (returns hash AA*64), signals=Found (carries
        // ZERO_HASH) → declaration_hash mismatch → BLOCKED (fail-closed, CN-11).
        // The signals file is stale (not regenerated after the catalogue changed).
        const STALE_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        struct Stage2HashMismatchMock;
        impl SpecElementHashReader for Stage2HashMismatchMock {
            fn read_spec_element_hashes(
                &self,
                _branch: &str,
                _track_id: &str,
            ) -> BlobFetchResult<std::collections::BTreeMap<domain::SpecElementId, ContentHash>>
            {
                BlobFetchResult::Found(std::collections::BTreeMap::new())
            }
        }
        impl TrackBlobReader for Stage2HashMismatchMock {
            fn read_spec_document(
                &self,
                _branch: &str,
                _track_id: &str,
            ) -> BlobFetchResult<SpecDocument> {
                BlobFetchResult::Found(all_blue_spec())
            }
            fn read_type_catalogue(
                &self,
                _branch: &str,
                _track_id: &str,
                _layer_id: &str,
            ) -> BlobFetchResult<(Vec<u8>, String)> {
                // Catalogue hash is STALE_HASH (aa*64), not ZERO_HASH.
                BlobFetchResult::Found((vec![], STALE_HASH.to_owned()))
            }
            fn read_type_signals(
                &self,
                _branch: &str,
                _track_id: &str,
                _layer_id: &str,
            ) -> BlobFetchResult<TypeSignalsDocument> {
                // Signals doc built with ZERO_HASH (via signals_doc_with) —
                // mismatches STALE_HASH returned by read_type_catalogue.
                BlobFetchResult::Found(signals_doc_with(&[("TrackId", ConfidenceSignal::Blue)]))
            }
            fn read_impl_plan(
                &self,
                _branch: &str,
                _track_id: &str,
            ) -> BlobFetchResult<domain::ImplPlanDocument> {
                panic!("read_impl_plan must not be called by U41")
            }
            fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>> {
                BlobFetchResult::Found(vec!["domain".to_string()])
            }
        }
        let reader = Stage2HashMismatchMock;
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(
            outcome.has_errors(),
            "declaration_hash mismatch must block (fail-closed, CN-11): {outcome:?}"
        );
        assert!(
            outcome
                .findings()
                .iter()
                .any(|f| f.message().contains("layer 'domain'") && f.message().contains("mismatch")),
            "error must mention the layer name and 'mismatch': {outcome:?}"
        );
    }

    #[test]
    fn test_u42_stage2_catalogue_found_signals_fetch_error_blocks_fail_closed() {
        // U42: catalogue=Found, read_type_signals=FetchError → BLOCKED (fail-closed, CN-11).
        // This exercises the `FetchError` arm of Step 2 in the Stage 2 loop
        // (distinct from U24 which routes FetchError through read_type_catalogue,
        // and from U40 which exercises signals=NotFound). A regression that silently
        // skipped or swallowed the signal-file read error would not be caught by
        // U24/U40; this test makes the Step 2 FetchError path directly observable.
        struct Stage2SignalsFetchErrorMock;
        impl SpecElementHashReader for Stage2SignalsFetchErrorMock {
            fn read_spec_element_hashes(
                &self,
                _branch: &str,
                _track_id: &str,
            ) -> BlobFetchResult<std::collections::BTreeMap<domain::SpecElementId, ContentHash>>
            {
                BlobFetchResult::Found(std::collections::BTreeMap::new())
            }
        }
        impl TrackBlobReader for Stage2SignalsFetchErrorMock {
            fn read_spec_document(
                &self,
                _branch: &str,
                _track_id: &str,
            ) -> BlobFetchResult<SpecDocument> {
                BlobFetchResult::Found(all_blue_spec())
            }
            fn read_type_catalogue(
                &self,
                _branch: &str,
                _track_id: &str,
                _layer_id: &str,
            ) -> BlobFetchResult<(Vec<u8>, String)> {
                // Catalogue is present → layer enrolled in Stage 2.
                BlobFetchResult::Found((vec![], ZERO_HASH.to_owned()))
            }
            fn read_type_signals(
                &self,
                _branch: &str,
                _track_id: &str,
                _layer_id: &str,
            ) -> BlobFetchResult<TypeSignalsDocument> {
                // Signals file I/O error → fail-closed.
                BlobFetchResult::FetchError("git show failed for type-signals".to_owned())
            }
            fn read_impl_plan(
                &self,
                _branch: &str,
                _track_id: &str,
            ) -> BlobFetchResult<domain::ImplPlanDocument> {
                panic!("read_impl_plan must not be called by U42")
            }
            fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>> {
                BlobFetchResult::Found(vec!["domain".to_string()])
            }
        }
        let reader = Stage2SignalsFetchErrorMock;
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(
            outcome.has_errors(),
            "catalogue Found + signals FetchError must block (fail-closed): {outcome:?}"
        );
        assert!(
            outcome
                .findings()
                .iter()
                .any(|f| f.message().contains("failed to read type-signals for layer 'domain'")),
            "error must mention the type-signals read failure for 'domain': {outcome:?}"
        );
    }

    // ===============================================================
    // U43–U44 — per-entry hash freshness check (AC-06 / IN-05 / D4).
    //
    // `catalogue_with_trackid_entry` now supplies `entry_hashes` with the
    // correct hash for `"types:TrackId"`.  These two tests verify the
    // newly-added per-entry check in isolation:
    //   U43: entry_hashes empty → hash missing → BLOCKED
    //   U44: entry_hashes supplies a wrong hash → BLOCKED (stale signals)
    // ===============================================================

    #[test]
    fn test_u43_chain2_missing_entry_hash_blocks_fail_closed() {
        // U43: The infrastructure adapter returns an empty `entry_hashes` map
        // (no hash for `"types:TrackId"`). The gate must block fail-closed
        // rather than silently passing an unverified entry.
        // Return empty entry_hashes → triggers the "hash missing" branch.
        let catalogue = catalogue_doc_with_entry_hashes(HashMap::new());
        let reader = ChainTwoMock::new(
            BlobFetchResult::Found(all_blue_catalogue_signals()),
            catalogue,
            BlobFetchResult::Found(std::collections::BTreeMap::new()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors(), "missing per-entry hash must block fail-closed: {outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("per-entry hash missing")),
            "error must mention 'per-entry hash missing': {outcome:?}"
        );
    }

    #[test]
    fn test_u44_chain2_stale_entry_hash_blocks() {
        // U44: The adapter supplies a hash for `"types:TrackId"` that differs
        // from the `entry_hash` stored in the signals document.  This indicates
        // the signals file was not regenerated after the catalogue entry changed.
        // Supply a hash that differs from the signal's entry_hash([0u8; 32]).
        let mut entry_hashes = HashMap::new();
        entry_hashes.insert("types:TrackId".to_owned(), ContentHash::from_bytes([0xab_u8; 32]));
        let catalogue = catalogue_doc_with_entry_hashes(entry_hashes);
        let reader = ChainTwoMock::new(
            BlobFetchResult::Found(all_blue_catalogue_signals()),
            catalogue,
            BlobFetchResult::Found(std::collections::BTreeMap::new()),
        );
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors(), "stale per-entry hash must block: {outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("per-entry hash mismatch")),
            "error must mention 'per-entry hash mismatch': {outcome:?}"
        );
    }

    #[test]
    fn test_u35_chain2_does_not_run_when_stage2_fails() {
        // U35: Stage 2 (type catalogue) failure → short-circuits before Chain ②
        // even when Chain ② signals would block. This test is falsifiable:
        // removing the `if outcome.has_errors() { return outcome; }` guard would
        // allow Chain ② to run. With Yellow signals AND a valid catalogue/hash
        // available, `check_catalogue_spec_signals(strict=true)` would then emit
        // a "catalogue-spec" finding, causing the final assertion to fail.
        struct Stage2FailWithChain2Mock;
        impl SpecElementHashReader for Stage2FailWithChain2Mock {
            fn read_spec_element_hashes(
                &self,
                _branch: &str,
                _track_id: &str,
            ) -> BlobFetchResult<std::collections::BTreeMap<domain::SpecElementId, ContentHash>>
            {
                BlobFetchResult::Found(std::collections::BTreeMap::new())
            }
        }
        impl TrackBlobReader for Stage2FailWithChain2Mock {
            fn read_spec_document(
                &self,
                _branch: &str,
                _track_id: &str,
            ) -> BlobFetchResult<SpecDocument> {
                BlobFetchResult::Found(all_blue_spec())
            }
            fn read_type_catalogue(
                &self,
                _branch: &str,
                _track_id: &str,
                _layer_id: &str,
            ) -> BlobFetchResult<(Vec<u8>, String)> {
                // Stage 2: catalogue present so Stage 2 proceeds to read signals.
                BlobFetchResult::Found((vec![], ZERO_HASH.to_owned()))
            }
            fn read_type_signals(
                &self,
                _branch: &str,
                _track_id: &str,
                _layer_id: &str,
            ) -> BlobFetchResult<TypeSignalsDocument> {
                // Stage 2: Red signal → blocks immediately and triggers early return.
                BlobFetchResult::Found(dt_with_red())
            }
            fn read_impl_plan(
                &self,
                _branch: &str,
                _track_id: &str,
            ) -> BlobFetchResult<domain::ImplPlanDocument> {
                panic!("read_impl_plan must not be called by U35")
            }
            fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>> {
                BlobFetchResult::Found(vec!["domain".to_string()])
            }
            fn read_catalogue_spec_signals_document(
                &self,
                _branch: &str,
                _track_id: &str,
                _layer_id: &str,
            ) -> BlobFetchResult<domain::CatalogueSpecSignalsDocument> {
                // Chain ②: Yellow signals — would block if Chain ② ran.
                BlobFetchResult::Found(all_yellow_signals())
            }
            fn read_catalogue_for_spec_ref_check(
                &self,
                _branch: &str,
                _track_id: &str,
                _layer_id: &str,
            ) -> BlobFetchResult<(
                domain::tddd::catalogue_v2::CatalogueDocument,
                String,
                HashMap<String, ContentHash>,
            )> {
                // Return a valid catalogue so that the signal gate would be reached
                // if Chain ② were not short-circuited. Without this override the
                // NotFound default causes the per-layer loop to `continue` before
                // reaching the signal gate, making the test non-falsifiable.
                catalogue_with_trackid_entry()
            }
        }
        let reader = Stage2FailWithChain2Mock;
        let outcome = check_strict_merge_gate("track/foo", &reader, &all_strict_matrix());
        assert!(outcome.has_errors(), "Stage 2 Red must block: {outcome:?}");
        // If the guard were removed, Chain ② would run and `check_catalogue_spec_signals`
        // would emit a finding containing "catalogue-spec" for the Yellow signal.
        assert!(
            outcome.findings().iter().all(|f| !f.message().contains("catalogue-spec")),
            "no Chain ② finding expected after Stage 2 short-circuit: {outcome:?}"
        );
    }
}
