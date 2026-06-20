//! Infrastructure adapter implementing `usecase::merge_gate::TrackBlobReader`
//! via `git show` on the local git repository.
//!
//! This is the bridge between the pure usecase layer (`merge_gate` /
//! `task_completion`) and the low-level git primitives in
//! `crate::git_cli::show`. The adapter:
//!
//! 1. Uses `fetch_blob_safe` to retrieve raw bytes from `origin/<branch>:<path>`
//!    with symlink / submodule rejection baked in.
//! 2. Applies strict UTF-8 decode (`String::from_utf8`) — non-UTF-8 bytes
//!    produce `BlobFetchResult::FetchError` (fail-closed, ADR §D4).
//! 3. Decodes the JSON into a domain aggregate via the existing
//!    `spec::codec` / `tddd::catalogue_document_codec` / `track::codec` modules.
//!
//! Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md`
//! §D5.3.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use domain::AdrVerifyReport;
use domain::ImplPlanDocument;
use domain::TypeSignalsDocument;
use domain::spec::SpecDocument;
use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::validate_branch_ref;
use domain::{AdrFilePort, CatalogueSpecSignalsDocument, ContentHash, SpecElementId};
use usecase::catalogue_spec_refs::SpecElementHashReader;
use usecase::merge_gate::{BlobFetchResult, TrackBlobReader};
use usecase::verify_adr_signals::{
    VerifyAdrSignals, VerifyAdrSignalsCommand, VerifyAdrSignalsInteractor,
};

use crate::tddd::catalogue_document_codec::CatalogueDocumentCodec;

use crate::git_cli::show::{BlobResult, fetch_blob_safe};

/// Adapter that reads track documents from the local git repository via
/// `git show origin/<branch>:<path>`.
///
/// Construct with `GitShowTrackBlobReader::new(repo_root)`. The adapter
/// is stateless apart from the repo root path, so a single instance can
/// be shared across multiple usecase calls (e.g. merge_gate +
/// task_completion from the same `pr.rs::wait_and_merge` invocation).
pub struct GitShowTrackBlobReader {
    repo_root: PathBuf,
}

impl GitShowTrackBlobReader {
    /// Creates a new adapter rooted at the given repository path.
    #[must_use]
    pub fn new(repo_root: PathBuf) -> Self {
        Self { repo_root }
    }

    /// Common path assembly: `track/items/<track_id>/<filename>`.
    fn blob_path(track_id: &str, filename: &str) -> String {
        format!("track/items/{track_id}/{filename}")
    }

    /// Fetches a blob and applies strict UTF-8 decode.
    ///
    /// Maps `BlobResult` → `Result<String, BlobFetchResult<T>>` where the
    /// error variant is already the final port outcome to return (NotFound
    /// or FetchError). Callers use `match` / `?`-style to chain into JSON
    /// decode.
    fn fetch_string<T>(&self, branch: &str, blob_path: &str) -> Result<String, BlobFetchResult<T>> {
        match fetch_blob_safe(&self.repo_root, branch, blob_path) {
            BlobResult::Found(bytes) => String::from_utf8(bytes).map_err(|e| {
                BlobFetchResult::FetchError(format!(
                    "{blob_path}: non-UTF-8 bytes in blob contents: {e}"
                ))
            }),
            BlobResult::NotFound => Err(BlobFetchResult::NotFound),
            BlobResult::CommandFailed(msg) => Err(BlobFetchResult::FetchError(msg)),
        }
    }

    /// Resolves the catalogue filename for `layer_id` by reading
    /// `architecture-rules.json` from the PR branch.
    ///
    /// Returns the explicit `catalogue_file` override if present, the
    /// default `<layer_id>-types.json` when the rules file is absent
    /// (preserving the legacy fallback for non-migrated repos), or an
    /// `Err(msg)` describing a fetch/parse failure when the rules file
    /// is present but unreadable. The fail-closed path prevents a
    /// silent downgrade to the default filename on such failures.
    ///
    /// The error variant is a plain `String` (the diagnostic to embed in a
    /// `BlobFetchResult::FetchError` at the call site). `NotFound` is
    /// absorbed as the legacy fallback, and `Found` is impossible from a
    /// filename lookup, so a generic error carrying those variants would
    /// force callers to handle impossible cases.
    fn resolve_catalogue_filename(&self, branch: &str, layer_id: &str) -> Result<String, String> {
        let text = match self.fetch_string::<String>(branch, "architecture-rules.json") {
            Ok(s) => s,
            Err(BlobFetchResult::NotFound) => {
                // Legacy fallback: no rules file on the branch → use the
                // conventional default. This is a NotFound case, not a
                // fetch failure, so the gate's per-layer NotFound semantic
                // is still meaningful.
                return Ok(format!("{layer_id}-types.json"));
            }
            Err(BlobFetchResult::FetchError(msg)) => {
                // Fetch error on a rules file that exists → fail-closed.
                return Err(msg);
            }
            Err(BlobFetchResult::Found(_)) => {
                // fetch_string never returns Err(Found); match exhaustively
                // to keep the code robust against enum expansion.
                return Err("internal: fetch_string returned Found in the Err arm".to_owned());
            }
        };
        match super::tddd_layers::parse_tddd_layers(&text) {
            Ok(bindings) => Ok(super::tddd_layers::find_binding(&bindings, layer_id)
                .map(|b| b.catalogue_file().to_owned())
                .unwrap_or_else(|| format!("{layer_id}-types.json"))),
            Err(e) => Err(format!(
                "architecture-rules.json parse error while resolving catalogue file for \
                 layer '{layer_id}': {e}"
            )),
        }
    }
}

/// Derives the signal filename for a declaration filename by the same rule
/// as `TdddLayerBinding::signal_file()` (infrastructure/verify/tddd_layers,
/// T003): strip `.json`, drop a trailing `s` if present, append
/// `-signals.json`. Mirrored here so the merge-gate adapter can compute the
/// signal path without constructing a full `TdddLayerBinding` (which would
/// require re-parsing `architecture-rules.json` after `resolve_catalogue_filename`
/// already did so).
fn signal_file_name_for(catalogue_filename: &str) -> String {
    let stem = catalogue_filename.strip_suffix(".json").unwrap_or(catalogue_filename);
    let signal_stem = if let Some(trimmed) = stem.strip_suffix('s') {
        format!("{trimmed}-signals")
    } else {
        format!("{stem}-signals")
    };
    format!("{signal_stem}.json")
}

impl TrackBlobReader for GitShowTrackBlobReader {
    fn read_spec_document(&self, branch: &str, track_id: &str) -> BlobFetchResult<SpecDocument> {
        let path = Self::blob_path(track_id, "spec.json");
        let text = match self.fetch_string::<SpecDocument>(branch, &path) {
            Ok(s) => s,
            Err(result) => return result,
        };
        match crate::spec::codec::decode(&text) {
            Ok(doc) => BlobFetchResult::Found(doc),
            Err(e) => BlobFetchResult::FetchError(format!("{path}: spec.json decode error: {e}")),
        }
    }

    fn read_type_catalogue(
        &self,
        branch: &str,
        track_id: &str,
        layer_id: &str,
    ) -> BlobFetchResult<(Vec<u8>, String)> {
        // Resolve the catalogue filename from the PR branch's
        // `architecture-rules.json` so that layers with an explicit
        // `tddd.catalogue_file` override are handled consistently between
        // the CI path (`verify_from_spec_json`) and the merge gate.
        // Fall back to `<layer_id>-types.json` when the rules file is absent
        // (NotFound); fail-closed when the rules file is present but
        // unreadable or unparseable.
        //
        // T022: returns raw bytes + pre-computed SHA-256 hex digest.
        // Decoding and Stage-2 signal-file hydration move to the usecase
        // caller (`check_strict_merge_gate`) which owns the freshness check.
        let filename = match self.resolve_catalogue_filename(branch, layer_id) {
            Ok(name) => name,
            Err(msg) => return BlobFetchResult::FetchError(msg),
        };
        let path = Self::blob_path(track_id, &filename);
        let bytes = match fetch_blob_safe(&self.repo_root, branch, &path) {
            BlobResult::Found(b) => b,
            BlobResult::NotFound => return BlobFetchResult::NotFound,
            BlobResult::CommandFailed(msg) => return BlobFetchResult::FetchError(msg),
        };
        // Validate that the catalogue blob is well-formed before treating it as
        // present.  Without this guard, a malformed or non-UTF-8 `<layer>-types.json`
        // could pass Stage 2 as long as the committed type-signals file carries a
        // matching hash — the freshness check alone does not detect structural
        // corruption (parent ADR 2026-05-08-0258 requires the catalogue to be a
        // decodable v3 `TypeCatalogueDocument` before the gate may rely on it).
        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(e) => {
                return BlobFetchResult::FetchError(format!(
                    "{path}: {filename} is not valid UTF-8: {e}"
                ));
            }
        };
        // Derive the filename stem for CatalogueDocumentCodec::decode validation
        // (crate_name field must match the stem). Mirror the same derivation used
        // by `read_catalogue_for_spec_ref_check` so both paths are consistent.
        let filename_stem_owned = std::path::Path::new(&filename)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .strip_suffix("-types.json")
            .map(str::to_owned)
            .unwrap_or_else(|| {
                std::path::Path::new(&filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_owned()
            });
        if let Err(e) = crate::tddd::catalogue_document_codec::CatalogueDocumentCodec::decode(
            text,
            &filename_stem_owned,
        ) {
            return BlobFetchResult::FetchError(format!("{path}: {filename} decode error: {e}"));
        }
        let hash_hex = crate::tddd::type_signals_codec::declaration_hash(&bytes);
        BlobFetchResult::Found((bytes, hash_hex))
    }

    fn read_enabled_layers(&self, branch: &str) -> BlobFetchResult<Vec<String>> {
        // Read `architecture-rules.json` from the PR branch blob so that
        // tracks which modify the rules file itself are evaluated against
        // their own layer definitions (not the local workspace). An empty
        // binding list (legacy rules file, or a PR that disables every layer)
        // is returned verbatim — the usecase gate is the fail-closed authority
        // and will reject an empty set explicitly.
        let text = match self.fetch_string::<Vec<String>>(branch, "architecture-rules.json") {
            Ok(s) => s,
            Err(result) => return result,
        };
        let bindings = match super::tddd_layers::parse_tddd_layers(&text) {
            Ok(b) => b,
            Err(e) => {
                return BlobFetchResult::FetchError(format!(
                    "architecture-rules.json parse error: {e}"
                ));
            }
        };
        BlobFetchResult::Found(bindings.iter().map(|b| b.layer_id().to_owned()).collect())
    }

    fn read_catalogue_spec_signal_opted_in_layers(
        &self,
        branch: &str,
    ) -> BlobFetchResult<Vec<String>> {
        // Mirrors `read_enabled_layers` but filters the binding set to layers
        // whose `tddd.catalogue_spec_signal.enabled = true` (ADR §D5.4 phased
        // activation). The merge gate's Stage 3 loop uses this subset so that
        // a layer which flipped the flag to false after generating a signals
        // file is not accidentally re-evaluated on presence alone.
        let text = match self.fetch_string::<Vec<String>>(branch, "architecture-rules.json") {
            Ok(s) => s,
            Err(result) => return result,
        };
        let bindings = match super::tddd_layers::parse_tddd_layers(&text) {
            Ok(b) => b,
            Err(e) => {
                return BlobFetchResult::FetchError(format!(
                    "architecture-rules.json parse error: {e}"
                ));
            }
        };
        BlobFetchResult::Found(
            bindings
                .iter()
                .filter(|b| b.catalogue_spec_signal_enabled())
                .map(|b| b.layer_id().to_owned())
                .collect(),
        )
    }

    fn read_impl_plan(&self, branch: &str, track_id: &str) -> BlobFetchResult<ImplPlanDocument> {
        let path = Self::blob_path(track_id, "impl-plan.json");
        let text = match self.fetch_string::<ImplPlanDocument>(branch, &path) {
            Ok(s) => s,
            Err(result) => return result,
        };
        match crate::impl_plan_codec::decode(&text) {
            Ok(doc) => BlobFetchResult::Found(doc),
            Err(e) => {
                BlobFetchResult::FetchError(format!("{path}: impl-plan.json decode error: {e}"))
            }
        }
    }

    /// Reads `<layer>-types.json` and returns `(doc, raw_bytes_sha256_hex, entry_hashes)`.
    ///
    /// Shares the same filename-resolution + UTF-8 + decode pipeline as
    /// `read_type_catalogue`, but the `String` slot carries the SHA-256 hex
    /// digest of the raw catalogue bytes (used for catalogue-spec stale
    /// detection) instead of the resolved filename. `entry_hashes` maps each
    /// entry name (type / trait / function key) to its per-entry SHA-256,
    /// computed via `canonical_json_sha256` so the usecase layer never needs
    /// to import infrastructure hashing helpers (CN-04 / IN-05).
    /// No Stage-2 signal-file hydration runs here — this port feeds the SoT
    /// Chain ② binary gate which does its own freshness check via
    /// `catalogue_declaration_hash`.
    fn read_catalogue_for_spec_ref_check(
        &self,
        branch: &str,
        track_id: &str,
        layer_id: &str,
    ) -> BlobFetchResult<(CatalogueDocument, String, HashMap<String, ContentHash>)> {
        let filename = match self.resolve_catalogue_filename(branch, layer_id) {
            Ok(name) => name,
            Err(msg) => return BlobFetchResult::FetchError(msg),
        };
        let path = Self::blob_path(track_id, &filename);
        let text = match self
            .fetch_string::<(CatalogueDocument, String, HashMap<String, ContentHash>)>(
                branch, &path,
            ) {
            Ok(s) => s,
            Err(result) => return result,
        };
        // T024: v3-native decode via `CatalogueDocumentCodec::decode`.
        // Non-v3 catalogues surface as `FetchError` (CN-11 fail-closed).
        // Derive `filename_stem` the same way the other verify paths do:
        // try to strip the `-types.json` suffix; fall back to `file_stem()`
        // (strips just the `.json`) so that arbitrary `tddd.catalogue_file`
        // overrides such as `shared.json` produce `shared`, not `shared.json`.
        let filename_stem_owned = std::path::Path::new(&filename)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .strip_suffix("-types.json")
            .map(str::to_owned)
            .unwrap_or_else(|| {
                std::path::Path::new(&filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_owned()
            });
        let filename_stem = filename_stem_owned.as_str();
        let doc = match CatalogueDocumentCodec::decode(&text, filename_stem) {
            Ok(doc) => doc,
            Err(e) => {
                return BlobFetchResult::FetchError(format!(
                    "{path}: {filename} decode error: {e}"
                ));
            }
        };
        let hash_hex = crate::tddd::type_signals_codec::declaration_hash(text.as_bytes());

        // Compute per-entry canonical JSON hashes (CN-04 / IN-05).
        // Parse raw text as serde_json::Value to extract per-entry subtrees.
        let entry_hashes = match build_catalogue_entry_hashes(&text) {
            Ok(map) => map,
            Err(e) => {
                return BlobFetchResult::FetchError(format!(
                    "{path}: failed to compute per-entry hashes: {e}"
                ));
            }
        };

        BlobFetchResult::Found((doc, hash_hex, entry_hashes))
    }

    /// Reads `<layer>-catalogue-spec-signals.json` and decodes it via the
    /// T010 codec. Returns `NotFound` when the signals file has not been
    /// generated yet (no `sotp signal calc-catalog-spec` run on this
    /// branch), `FetchError` on I/O / decode failure.
    fn read_catalogue_spec_signals_document(
        &self,
        branch: &str,
        track_id: &str,
        layer_id: &str,
    ) -> BlobFetchResult<CatalogueSpecSignalsDocument> {
        let filename = format!("{layer_id}-catalogue-spec-signals.json");
        let path = Self::blob_path(track_id, &filename);
        let text = match self.fetch_string::<CatalogueSpecSignalsDocument>(branch, &path) {
            Ok(s) => s,
            Err(result) => return result,
        };
        match crate::tddd::catalogue_spec_signals_codec::decode(&text) {
            Ok(doc) => BlobFetchResult::Found(doc),
            Err(e) => BlobFetchResult::FetchError(format!("{path}: {filename} decode error: {e}")),
        }
    }

    /// Reads `<layer>-type-signals.json` (chain-③ signals document,
    /// schema_version 1) from the git blob and decodes it via
    /// `type_signals_codec`.
    ///
    /// The filename is derived from the layer's catalogue filename (resolved
    /// from `architecture-rules.json`) via the same `signal_file_name_for`
    /// rule used by `read_type_catalogue`, so that layers with a
    /// `tddd.catalogue_file` override produce the correct signal path.
    ///
    /// Returns:
    /// - `Found(doc)` when the signals file exists and decodes successfully.
    /// - `NotFound` when the signals file is absent on the target ref.
    /// - `FetchError(msg)` on I/O, UTF-8, or JSON decode failure.
    fn read_type_signals(
        &self,
        branch: &str,
        track_id: &str,
        layer_id: &str,
    ) -> BlobFetchResult<TypeSignalsDocument> {
        let catalogue_filename = match self.resolve_catalogue_filename(branch, layer_id) {
            Ok(name) => name,
            Err(msg) => return BlobFetchResult::FetchError(msg),
        };
        let signal_filename = signal_file_name_for(&catalogue_filename);
        let path = Self::blob_path(track_id, &signal_filename);
        let text = match self.fetch_string::<TypeSignalsDocument>(branch, &path) {
            Ok(s) => s,
            Err(result) => return result,
        };
        match crate::tddd::type_signals_codec::decode(&text) {
            Ok(doc) => BlobFetchResult::Found(doc),
            Err(e) => {
                BlobFetchResult::FetchError(format!("{path}: {signal_filename} decode error: {e}"))
            }
        }
    }

    /// Reads ADR frontmatter from `origin/<branch>:knowledge/adr/` via git-blob
    /// fetching and returns a domain [`AdrVerifyReport`].
    ///
    /// Uses [`crate::adr_decision::GitBlobAdrFileAdapter`] — the same
    /// `git ls-tree` / `git show` pipeline as the other merge-gate readers —
    /// so that chain ⓪ ADR evaluation is always performed against the PR branch
    /// ref rather than the local worktree.
    ///
    /// ## Why not the local worktree?
    ///
    /// Reading from `repo_root/knowledge/adr/` (the previous approach) means the
    /// ADR state visible to the merge gate depends on what is checked out locally,
    /// not what was pushed to the PR branch. A reviewer could therefore pass the
    /// gate with locally-edited ADRs that were never committed. The git-blob path
    /// is consistent with how all other chain signals are evaluated.
    ///
    /// Returns:
    /// - `Found(report)` when ADR scanning succeeds (empty `knowledge/adr/`
    ///   directory is also a valid success — zero decisions maps to a clean report).
    /// - `NotFound` when `knowledge/adr` is absent on the branch (empty `ls-tree`
    ///   stdout → no ADR directory committed → chain ⓪ is vacuously green).
    /// - `FetchError(msg)` on branch validation failure, git spawn / exit error,
    ///   non-UTF-8 blob content, or YAML front-matter parse failure.
    fn read_adr_verify_report(&self, branch: String) -> BlobFetchResult<AdrVerifyReport> {
        // Validate the branch name before constructing any git-ref string (D4.2).
        if let Err(err) = validate_branch_ref(&branch) {
            return BlobFetchResult::FetchError(format!("invalid branch ref: {err}"));
        }

        // Probe ancestor + target tree entries on the branch BEFORE listing.
        // `git ls-tree origin/<branch> -- knowledge/adr/` yields zero entries
        // both when the directory is genuinely absent AND when `knowledge/adr`
        // (or its ancestor `knowledge`) is a committed symlink/submodule/file —
        // without these probes an attacker could replace any ancestor with a
        // symlink and silently bypass chain ⓪. Fail-closed for non-tree modes
        // at every level; only the truly-absent case maps to NotFound (chain
        // ⓪ vacuously green for fresh tracks).
        use crate::git_cli::show::{TreeEntryKind, git_ls_tree_entry_kind};
        for ancestor in ["knowledge"] {
            match git_ls_tree_entry_kind(&self.repo_root, &branch, ancestor) {
                Ok(TreeEntryKind::NotFound) => return BlobFetchResult::NotFound,
                Ok(TreeEntryKind::Other(0o040_000)) => {}
                Ok(TreeEntryKind::Symlink) => {
                    return BlobFetchResult::FetchError(format!(
                        "{ancestor} on branch '{branch}' is a symlink — refusing to follow"
                    ));
                }
                Ok(TreeEntryKind::Submodule) => {
                    return BlobFetchResult::FetchError(format!(
                        "{ancestor} on branch '{branch}' is a submodule — refusing to follow"
                    ));
                }
                Ok(TreeEntryKind::RegularFile) => {
                    return BlobFetchResult::FetchError(format!(
                        "{ancestor} on branch '{branch}' is a regular file, not a directory"
                    ));
                }
                Ok(TreeEntryKind::Other(mode)) => {
                    return BlobFetchResult::FetchError(format!(
                        "{ancestor} on branch '{branch}' has unsupported git tree mode {mode:o}"
                    ));
                }
                Err(e) => {
                    return BlobFetchResult::FetchError(format!(
                        "failed to probe {ancestor} on branch '{branch}': {e}"
                    ));
                }
            }
        }
        match git_ls_tree_entry_kind(&self.repo_root, &branch, "knowledge/adr") {
            Ok(TreeEntryKind::NotFound) => return BlobFetchResult::NotFound,
            Ok(TreeEntryKind::Symlink) => {
                return BlobFetchResult::FetchError(format!(
                    "knowledge/adr on branch '{branch}' is a symlink — refusing to follow"
                ));
            }
            Ok(TreeEntryKind::Submodule) => {
                return BlobFetchResult::FetchError(format!(
                    "knowledge/adr on branch '{branch}' is a submodule — refusing to follow"
                ));
            }
            Ok(TreeEntryKind::RegularFile) => {
                return BlobFetchResult::FetchError(format!(
                    "knowledge/adr on branch '{branch}' is a regular file, not a directory"
                ));
            }
            // Mode 040000 (tree / directory) lands in Other(0o040000); any
            // other unexpected mode also flows through this arm and must be
            // rejected fail-closed (the only mode the merge gate accepts is a
            // real directory tree).
            Ok(TreeEntryKind::Other(0o040_000)) => {}
            Ok(TreeEntryKind::Other(mode)) => {
                return BlobFetchResult::FetchError(format!(
                    "knowledge/adr on branch '{branch}' has unsupported git tree mode {mode:o}"
                ));
            }
            Err(e) => {
                return BlobFetchResult::FetchError(format!(
                    "failed to probe knowledge/adr on branch '{branch}': {e}"
                ));
            }
        }

        let adapter =
            crate::adr_decision::GitBlobAdrFileAdapter::new(self.repo_root.clone(), branch.clone());
        let port: Arc<dyn AdrFilePort> = Arc::new(adapter);

        let paths = match port.list_adr_paths() {
            Ok(p) => p,
            Err(e) => {
                return BlobFetchResult::FetchError(format!("adr scan failed: {e}"));
            }
        };

        // After the tree-mode probe, an empty listing means the directory is
        // committed-but-empty: treat as Found with zero decisions (the
        // interactor produces a clean report).
        let _ = paths;

        let interactor = VerifyAdrSignalsInteractor::new(port);
        match interactor.verify(VerifyAdrSignalsCommand) {
            Ok(report) => BlobFetchResult::Found(report),
            Err(e) => BlobFetchResult::FetchError(format!("adr scan failed: {e}")),
        }
    }
}

/// Build a `HashMap<String, ContentHash>` mapping each catalogue entry to
/// the SHA-256 of its canonical JSON subtree.
///
/// Keys are **section-qualified** (`"types:<name>"`, `"traits:<name>"`,
/// `"functions:<path>"`) so that a type and a trait that share the same short
/// name cannot overwrite each other.  Callers must look up by the same
/// section-qualified key (see [`crate::catalogue_traversal::CatalogueEntryRef::section_key`]).
///
/// Parses `catalogue_text` as raw JSON and extracts the `types`, `traits`, and
/// `functions` sections, hashing each entry's value object via
/// `super::plan_artifact_refs::canonical_json_sha256` (CN-04 / IN-05).
///
/// # Errors
///
/// Returns a human-readable error string when `catalogue_text` is not valid JSON
/// or when an expected section is not an object.
fn build_catalogue_entry_hashes(
    catalogue_text: &str,
) -> Result<HashMap<String, ContentHash>, String> {
    let raw: serde_json::Value =
        serde_json::from_str(catalogue_text).map_err(|e| format!("JSON parse error: {e}"))?;
    let mut out: HashMap<String, ContentHash> = HashMap::new();
    for section in &["types", "traits", "functions"] {
        if let Some(obj) = raw.get(section).and_then(|v| v.as_object()) {
            for (entry_name, entry_value) in obj {
                let json_str = super::plan_artifact_refs::canonical_json(entry_value);
                let hex = super::plan_artifact_refs::canonical_json_sha256(&json_str);
                let hash = ContentHash::try_from_hex(&hex).map_err(|e| {
                    format!(
                        "internal: canonical_json_sha256 produced non-hex for \
                         entry '{entry_name}' in section '{section}': {e}"
                    )
                })?;
                // Section-qualified key: "types:Foo", "traits:Foo", "functions:my_fn".
                // CatalogueDocument guarantees uniqueness within each section, not
                // across sections, so bare names would overwrite each other when a
                // type and a trait share the same short name.
                out.insert(format!("{section}:{entry_name}"), hash);
            }
        }
    }
    Ok(out)
}

/// `SpecElementHashReader` is implemented on the same adapter so consumers
/// share a single `GitShowTrackBlobReader` instance for all three read ports
/// (TrackBlobReader read_* methods, SpecElementHashReader).
impl SpecElementHashReader for GitShowTrackBlobReader {
    fn read_spec_element_hashes(
        &self,
        branch: &str,
        track_id: &str,
    ) -> BlobFetchResult<BTreeMap<SpecElementId, ContentHash>> {
        let path = Self::blob_path(track_id, "spec.json");
        let text = match self.fetch_string::<BTreeMap<SpecElementId, ContentHash>>(branch, &path) {
            Ok(s) => s,
            Err(result) => return result,
        };
        // Delegate all text-parsing and hashing to the shared pure helper so
        // the logic is maintained in one place (`plan_artifact_refs::spec_element_hashes_from_text`).
        match super::plan_artifact_refs::spec_element_hashes_from_text(&text, &path) {
            Ok(map) => BlobFetchResult::Found(map),
            Err(e) => BlobFetchResult::FetchError(e),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::adr_decision::test_support::{git, setup_repo_with_adr_blobs};

    // --- Fixture helpers ---

    /// Creates a temp git repo with a track directory containing the
    /// supplied blobs, then sets up a local `origin` remote pointing to
    /// itself so `origin/main:track/items/<id>/<file>` resolves.
    ///
    /// When `files` is empty, a placeholder `.gitkeep` is committed so the
    /// initial commit can succeed (git rejects empty commits by default).
    /// The placeholder lives at the repo root, not in the track dir, so
    /// `track/items/<id>/...` resolves to NotFound as expected by the test.
    fn setup_repo_with_track(track_id: &str, files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);

        if files.is_empty() {
            // Commit a placeholder so the initial commit is non-empty.
            std::fs::write(repo.join(".gitkeep"), b"").unwrap();
            git(repo, &["add", ".gitkeep"]);
        } else {
            let track_dir = repo.join("track/items").join(track_id);
            std::fs::create_dir_all(&track_dir).unwrap();
            for (name, contents) in files {
                std::fs::write(track_dir.join(name), contents).unwrap();
            }
            git(repo, &["add", "track"]);
        }

        git(repo, &["commit", "--quiet", "-m", "initial"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);
        dir
    }

    // --- Spec document fixtures ---

    const SPEC_JSON_MINIMAL: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature",
  "scope": { "in_scope": [], "out_of_scope": [] },
  "signals": { "blue": 1, "yellow": 0, "red": 0 }
}"#;

    const DOMAIN_TYPES_MINIMAL: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "TrackId", "kind": "value_object", "description": "test", "approved": true, "expected_members": [], "expected_methods": [] }
  ],
  "signals": [
    { "type_name": "TrackId", "kind_tag": "value_object", "signal": "blue", "found_type": true }
  ]
}"#;

    /// v3-native catalogue fixture required by `CatalogueDocumentCodec::decode`.
    /// Used by `read_type_catalogue` tests and `read_catalogue_for_spec_ref_check` tests.
    const DOMAIN_TYPES_V3_MINIMAL: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "TrackId": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } }
    }
  },
  "traits": {},
  "functions": {}
}"#;

    /// v3-native catalogue fixture with `crate_name: "domain_ext"`, used by
    /// `test_read_type_catalogue_found_with_custom_catalogue_file_override` where
    /// the file is committed as `domain_ext-types.json` (a valid Rust crate name).
    const CUSTOM_DOMAIN_TYPES_V3_MINIMAL: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain_ext",
  "layer": "domain",
  "types": {
    "TrackId": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } }
    }
  },
  "traits": {},
  "functions": {}
}"#;

    fn impl_plan_json_minimal() -> String {
        // schema_version 1: minimal impl-plan with one todo task
        r#"{
  "schema_version": 1,
  "tasks": [
    { "id": "T001", "description": "Test task", "status": "todo" }
  ],
  "plan": {
    "summary": [],
    "sections": [
      { "id": "S1", "title": "Section", "description": [], "task_ids": ["T001"] }
    ]
  }
}"#
        .to_owned()
    }

    // --- read_spec_document ---

    #[test]
    fn test_read_spec_document_found() {
        let dir = setup_repo_with_track("foo", &[("spec.json", SPEC_JSON_MINIMAL.as_bytes())]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_spec_document("main", "foo") {
            BlobFetchResult::Found(doc) => {
                assert_eq!(doc.title(), "Feature");
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_read_spec_document_not_found() {
        let dir = setup_repo_with_track("foo", &[]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(matches!(reader.read_spec_document("main", "foo"), BlobFetchResult::NotFound));
    }

    #[test]
    fn test_read_spec_document_decode_error() {
        let dir = setup_repo_with_track("foo", &[("spec.json", b"not valid json")]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_spec_document("main", "foo") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("decode error"), "{msg}");
            }
            other => panic!("expected FetchError, got {other:?}"),
        }
    }

    #[test]
    fn test_read_spec_document_invalid_utf8_fetch_error() {
        // Invalid UTF-8 byte sequence (lone 0xFF)
        let dir = setup_repo_with_track("foo", &[("spec.json", &[0xFF, 0xFE, 0xFD])]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_spec_document("main", "foo") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("non-UTF-8"), "{msg}");
            }
            other => panic!("expected FetchError for non-UTF-8, got {other:?}"),
        }
    }

    #[test]
    fn test_read_spec_document_bad_branch_fetch_error() {
        let dir = setup_repo_with_track("foo", &[("spec.json", SPEC_JSON_MINIMAL.as_bytes())]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_spec_document("does-not-exist", "foo") {
            BlobFetchResult::FetchError(_) => {}
            other => panic!("expected FetchError, got {other:?}"),
        }
    }

    // --- read_type_catalogue ---
    //
    // T022: read_type_catalogue now returns raw bytes + pre-computed SHA-256
    // hex digest. It no longer fetches the companion signal file, decodes the
    // catalogue, or checks the declaration_hash — those steps moved to the
    // usecase caller (check_strict_merge_gate). Tests reflect the new contract.

    #[test]
    fn test_read_type_catalogue_found_returns_bytes_and_hash() {
        // Happy path: catalogue file exists → returns raw bytes + matching SHA-256.
        // No signal file is needed because read_type_catalogue no longer fetches it.
        // Uses v3 fixture because CatalogueDocumentCodec::decode (called during validation)
        // rejects non-v3 catalogues.
        let dir = setup_repo_with_track(
            "foo",
            &[("domain-types.json", DOMAIN_TYPES_V3_MINIMAL.as_bytes())],
        );
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_type_catalogue("main", "foo", "domain") {
            BlobFetchResult::Found((bytes, hash_hex)) => {
                // Bytes must match the raw catalogue content.
                assert_eq!(bytes, DOMAIN_TYPES_V3_MINIMAL.as_bytes());
                // Hash must be the SHA-256 of those bytes.
                let expected = crate::tddd::type_signals_codec::declaration_hash(
                    DOMAIN_TYPES_V3_MINIMAL.as_bytes(),
                );
                assert_eq!(hash_hex, expected, "hash must be SHA-256 of raw catalogue bytes");
                assert_eq!(hash_hex.len(), 64, "hash must be 64-char hex");
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_read_type_catalogue_succeeds_without_signal_file() {
        // T022: read_type_catalogue must succeed even when no companion signal
        // file exists. Previously it fetched and checked the signal file here;
        // that responsibility moved to the usecase caller.
        // Uses v3 fixture because decode validation now runs inside read_type_catalogue.
        let dir = setup_repo_with_track(
            "foo",
            &[("domain-types.json", DOMAIN_TYPES_V3_MINIMAL.as_bytes())],
        );
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(
            matches!(
                reader.read_type_catalogue("main", "foo", "domain"),
                BlobFetchResult::Found(_)
            ),
            "read_type_catalogue must not require the signal file (T022)"
        );
    }

    #[test]
    fn test_read_type_catalogue_found_with_custom_catalogue_file_override() {
        // Verify that `architecture-rules.json` at the repo root on the branch
        // with an explicit `tddd.catalogue_file` override is honoured: the
        // adapter must return `Found((bytes, hash))` for the overridden file,
        // not the default `domain-types.json`.
        // The custom filename `domain_ext-types.json` has stem `domain_ext`, a valid
        // Rust crate name (underscores are permitted).
        const ARCH_RULES_CUSTOM: &str = r#"{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "deny_reason": "",
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain_ext-types.json"
      }
    }
  ]
}"#;
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        // Write architecture-rules.json at repo root.
        std::fs::write(repo.join("architecture-rules.json"), ARCH_RULES_CUSTOM).unwrap();
        // Write the custom-named catalogue in the track directory.
        // Uses CUSTOM_DOMAIN_TYPES_V3_MINIMAL (crate_name = "domain_ext") because
        // CatalogueDocumentCodec::decode now runs during read_type_catalogue; the
        // fixture must be v3 and its crate_name must match the filename stem "domain_ext".
        let track_dir = repo.join("track/items/foo");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("domain_ext-types.json"), CUSTOM_DOMAIN_TYPES_V3_MINIMAL)
            .unwrap();
        git(repo, &["add", "."]);
        git(repo, &["commit", "--quiet", "-m", "initial"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let reader = GitShowTrackBlobReader::new(repo.to_path_buf());
        match reader.read_type_catalogue("main", "foo", "domain") {
            BlobFetchResult::Found((bytes, hash_hex)) => {
                // Must have read the overridden file, not domain-types.json.
                assert_eq!(bytes, CUSTOM_DOMAIN_TYPES_V3_MINIMAL.as_bytes());
                let expected = crate::tddd::type_signals_codec::declaration_hash(
                    CUSTOM_DOMAIN_TYPES_V3_MINIMAL.as_bytes(),
                );
                assert_eq!(
                    hash_hex, expected,
                    "hash must be SHA-256 of the overridden catalogue bytes"
                );
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_read_type_catalogue_not_found() {
        let dir = setup_repo_with_track("foo", &[("spec.json", SPEC_JSON_MINIMAL.as_bytes())]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(matches!(
            reader.read_type_catalogue("main", "foo", "domain"),
            BlobFetchResult::NotFound
        ));
    }

    // --- read_impl_plan ---

    #[test]
    fn test_read_impl_plan_found() {
        let impl_plan = impl_plan_json_minimal();
        let dir = setup_repo_with_track("foo", &[("impl-plan.json", impl_plan.as_bytes())]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_impl_plan("main", "foo") {
            BlobFetchResult::Found(doc) => {
                assert_eq!(doc.tasks().len(), 1);
                assert_eq!(doc.tasks()[0].id().to_string(), "T001");
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_read_impl_plan_not_found() {
        let dir = setup_repo_with_track("foo", &[]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(matches!(reader.read_impl_plan("main", "foo"), BlobFetchResult::NotFound));
    }

    #[test]
    fn test_read_impl_plan_decode_error() {
        let dir = setup_repo_with_track("foo", &[("impl-plan.json", b"not json")]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_impl_plan("main", "foo") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("decode error"), "{msg}");
            }
            other => panic!("expected FetchError, got {other:?}"),
        }
    }

    // --- Symlink / submodule rejection ---

    #[cfg(unix)]
    #[test]
    fn test_read_spec_document_rejects_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        let track_dir = repo.join("track/items/foo");
        std::fs::create_dir_all(&track_dir).unwrap();
        // Create spec.json as a symlink to another file
        std::fs::write(track_dir.join("target.json"), SPEC_JSON_MINIMAL).unwrap();
        std::os::unix::fs::symlink("target.json", track_dir.join("spec.json")).unwrap();
        git(repo, &["add", "track"]);
        git(repo, &["commit", "--quiet", "-m", "add symlink"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let reader = GitShowTrackBlobReader::new(repo.to_path_buf());
        match reader.read_spec_document("main", "foo") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("symlink"), "{msg}");
            }
            other => panic!("expected FetchError(symlink), got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_read_type_catalogue_rejects_symlink() {
        // T022: read_type_catalogue uses fetch_blob_safe which rejects symlinks.
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        let track_dir = repo.join("track/items/foo");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("target.json"), DOMAIN_TYPES_MINIMAL).unwrap();
        std::os::unix::fs::symlink("target.json", track_dir.join("domain-types.json")).unwrap();
        git(repo, &["add", "track"]);
        git(repo, &["commit", "--quiet", "-m", "add symlink"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let reader = GitShowTrackBlobReader::new(repo.to_path_buf());
        match reader.read_type_catalogue("main", "foo", "domain") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("symlink"), "{msg}");
            }
            other => panic!("expected FetchError(symlink), got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_read_impl_plan_rejects_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        let track_dir = repo.join("track/items/foo");
        std::fs::create_dir_all(&track_dir).unwrap();
        let impl_plan = impl_plan_json_minimal();
        std::fs::write(track_dir.join("target.json"), impl_plan).unwrap();
        std::os::unix::fs::symlink("target.json", track_dir.join("impl-plan.json")).unwrap();
        git(repo, &["add", "track"]);
        git(repo, &["commit", "--quiet", "-m", "add symlink"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let reader = GitShowTrackBlobReader::new(repo.to_path_buf());
        match reader.read_impl_plan("main", "foo") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("symlink"), "{msg}");
            }
            other => panic!("expected FetchError(symlink), got {other:?}"),
        }
    }

    // --- read_adr_verify_report (git-blob path) ---
    //
    // These tests use a real git repo so that `origin/<branch>:knowledge/adr/`
    // can be resolved. The fixture helpers follow the same pattern as
    // `setup_repo_with_track` above.

    const ADR_ACCEPTED_FRONTMATTER: &str = "\
---
adr_id: test-accepted
decisions:
  - id: D1
    status: accepted
    user_decision_ref: chat:2026-01-01
---
# body
";

    const ADR_BAD_FRONTMATTER: &str = "# no frontmatter here\n";

    #[test]
    fn test_read_adr_verify_report_found_counts_adr_signals() {
        // Happy path: one accepted ADR on the branch → blue=1.
        // Verifies the `branch` argument is actually used (not local worktree).
        let dir =
            setup_repo_with_adr_blobs(&[("2026-06-18-0001-test.md", ADR_ACCEPTED_FRONTMATTER)]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_adr_verify_report("main".to_owned()) {
            BlobFetchResult::Found(report) => {
                assert_eq!(report.blue_count(), 1);
                assert_eq!(report.yellow_count(), 0);
                assert_eq!(report.red_count(), 0);
                assert_eq!(report.grandfathered_count(), 0);
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_read_adr_verify_report_not_found_when_no_adr_files() {
        // No knowledge/adr directory on the branch → NotFound.
        let dir = setup_repo_with_adr_blobs(&[]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(
            matches!(reader.read_adr_verify_report("main".to_owned()), BlobFetchResult::NotFound),
            "expected NotFound when no ADR files on branch"
        );
    }

    #[test]
    fn test_read_adr_verify_report_parse_failure_returns_fetch_error() {
        // An ADR file with no front-matter → parse failure → FetchError.
        let dir = setup_repo_with_adr_blobs(&[("bad.md", ADR_BAD_FRONTMATTER)]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_adr_verify_report("main".to_owned()) {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("adr scan failed"), "{msg}");
            }
            other => panic!("expected FetchError, got {other:?}"),
        }
    }

    #[test]
    fn test_read_adr_verify_report_invalid_branch_returns_fetch_error() {
        // A branch name with embedded whitespace must be rejected before git is called.
        let dir = setup_repo_with_adr_blobs(&[]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_adr_verify_report("bad branch name".to_owned()) {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("invalid branch ref"), "{msg}");
            }
            other => panic!("expected FetchError for invalid branch, got {other:?}"),
        }
    }

    #[test]
    fn test_read_adr_verify_report_branch_arg_matters() {
        // Verifies that the `branch` argument drives the git-blob lookup:
        // "main" has an ADR but "does-not-exist" should fail with FetchError
        // (bad branch → git ls-tree fails → ListPaths error → FetchError).
        let dir =
            setup_repo_with_adr_blobs(&[("2026-06-18-0001-test.md", ADR_ACCEPTED_FRONTMATTER)]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        // "main" succeeds
        assert!(matches!(
            reader.read_adr_verify_report("main".to_owned()),
            BlobFetchResult::Found(_)
        ));
        // non-existent branch → git ls-tree returns non-zero → FetchError
        match reader.read_adr_verify_report("does-not-exist".to_owned()) {
            BlobFetchResult::FetchError(_) => {}
            other => panic!("expected FetchError for non-existent branch, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_read_adr_verify_report_rejects_symlink_adr_file_via_git_blob() {
        // A symlinked .md file committed in knowledge/adr/ must be rejected by
        // git_ls_tree_dir (fail-closed at the listing stage: mode 120000 is not
        // a regular file, so the whole listing fails rather than silently skipping
        // the symlink, ADR §D4.3). The error propagates as FetchError.
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        let adr_dir = repo.join("knowledge/adr");
        std::fs::create_dir_all(&adr_dir).unwrap();
        // Commit a real file and a symlink pointing to it.
        std::fs::write(adr_dir.join("real.md"), ADR_ACCEPTED_FRONTMATTER).unwrap();
        std::os::unix::fs::symlink("real.md", adr_dir.join("link.md")).unwrap();
        git(repo, &["add", "knowledge"]);
        git(repo, &["commit", "--quiet", "-m", "add files"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);

        // git_ls_tree_dir rejects symlinks at the listing stage (fail-closed).
        // The whole listing fails, so read_adr_verify_report returns FetchError.
        let reader = GitShowTrackBlobReader::new(repo.to_path_buf());
        match reader.read_adr_verify_report("main".to_owned()) {
            BlobFetchResult::FetchError(msg) => {
                assert!(
                    msg.contains("symlink") || msg.contains("adr scan failed"),
                    "expected symlink rejection error, got: {msg}"
                );
            }
            other => panic!("expected FetchError(symlink rejection), got {other:?}"),
        }
    }

    // --- Fixtures for catalogue-spec-signals ---

    /// A minimal `<layer>-catalogue-spec-signals.json` payload (schema_version 1).
    /// Uses an all-zeroes hash which is valid per the codec (64-char lowercase hex).
    const CATALOGUE_SPEC_SIGNALS_MINIMAL: &str = r#"{
  "schema_version": 1,
  "catalogue_declaration_hash": "0000000000000000000000000000000000000000000000000000000000000000",
  "signals": []
}"#;

    /// A spec.json with one in_scope element (id = "IN-01") so that
    /// `read_spec_element_hashes` can return a non-empty map.
    const SPEC_JSON_WITH_ELEMENT: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature",
  "scope": {
    "in_scope": [
      { "id": "IN-01", "text": "Some requirement" }
    ],
    "out_of_scope": []
  },
  "signals": { "blue": 1, "yellow": 0, "red": 0 }
}"#;

    // --- read_catalogue_for_spec_ref_check ---

    #[test]
    fn test_read_catalogue_for_spec_ref_check_returns_raw_sha256_not_filename() {
        // Verify that the `String` slot is the SHA-256 of the raw catalogue bytes
        // (for stale detection), NOT the resolved filename.
        let dir = setup_repo_with_track(
            "foo",
            &[("domain-types.json", DOMAIN_TYPES_V3_MINIMAL.as_bytes())],
        );
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_catalogue_for_spec_ref_check("main", "foo", "domain") {
            BlobFetchResult::Found((_doc, hash_hex, _entry_hashes)) => {
                // Must be a 64-char lowercase hex string (SHA-256), not a filename.
                assert_eq!(hash_hex.len(), 64, "hash_hex must be 64-char hex, got '{hash_hex}'");
                assert!(
                    hash_hex.chars().all(|c: char| c.is_ascii_hexdigit() && !c.is_uppercase()),
                    "hash_hex must be lowercase hex, got '{hash_hex}'"
                );
                // Must NOT be the filename.
                assert_ne!(hash_hex, "domain-types.json", "String slot must be hash, not filename");
                // Must match the SHA-256 of the raw catalogue bytes.
                let expected = crate::tddd::type_signals_codec::declaration_hash(
                    DOMAIN_TYPES_V3_MINIMAL.as_bytes(),
                );
                assert_eq!(hash_hex, expected, "hash_hex must be SHA-256 of raw catalogue bytes");
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_read_catalogue_for_spec_ref_check_no_stage2_hydration() {
        // Verify that read_catalogue_for_spec_ref_check does NOT look up or require
        // the Stage-2 signal file (unlike read_type_catalogue). This test has only
        // the declaration file and no companion signal file — it must succeed.
        let dir = setup_repo_with_track(
            "foo",
            &[("domain-types.json", DOMAIN_TYPES_V3_MINIMAL.as_bytes())],
        );
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        // If Stage-2 hydration ran, it would return FetchError (signal file absent).
        // A clean Found confirms no signal file hydration occurred.
        assert!(
            matches!(
                reader.read_catalogue_for_spec_ref_check("main", "foo", "domain"),
                BlobFetchResult::Found(_)
            ),
            "read_catalogue_for_spec_ref_check must not require signal file"
        );
    }

    #[test]
    fn test_read_catalogue_for_spec_ref_check_not_found() {
        let dir = setup_repo_with_track("foo", &[]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(matches!(
            reader.read_catalogue_for_spec_ref_check("main", "foo", "domain"),
            BlobFetchResult::NotFound
        ));
    }

    #[test]
    fn test_read_catalogue_for_spec_ref_check_decode_error() {
        let dir = setup_repo_with_track("foo", &[("domain-types.json", b"{}")]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_catalogue_for_spec_ref_check("main", "foo", "domain") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("decode error"), "{msg}");
            }
            other => panic!("expected FetchError, got {other:?}"),
        }
    }

    // --- read_catalogue_spec_signals_document ---

    #[test]
    fn test_read_catalogue_spec_signals_document_found() {
        let dir = setup_repo_with_track(
            "foo",
            &[("domain-catalogue-spec-signals.json", CATALOGUE_SPEC_SIGNALS_MINIMAL.as_bytes())],
        );
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(
            matches!(
                reader.read_catalogue_spec_signals_document("main", "foo", "domain"),
                BlobFetchResult::Found(_)
            ),
            "expected Found for existing signals file"
        );
    }

    #[test]
    fn test_read_catalogue_spec_signals_document_not_found_propagates() {
        // NotFound is expected (and acceptable) when the signals file has not been
        // generated yet. The adapter must propagate NotFound, not convert it to FetchError.
        let dir = setup_repo_with_track("foo", &[]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(
            matches!(
                reader.read_catalogue_spec_signals_document("main", "foo", "domain"),
                BlobFetchResult::NotFound
            ),
            "NotFound must propagate for absent signals file"
        );
    }

    #[test]
    fn test_read_catalogue_spec_signals_document_decode_error() {
        let dir = setup_repo_with_track(
            "foo",
            &[("domain-catalogue-spec-signals.json", b"not valid json")],
        );
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_catalogue_spec_signals_document("main", "foo", "domain") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("decode error"), "{msg}");
            }
            other => panic!("expected FetchError, got {other:?}"),
        }
    }

    // --- read_spec_element_hashes ---

    #[test]
    fn test_read_spec_element_hashes_found_with_element() {
        let dir = setup_repo_with_track("foo", &[("spec.json", SPEC_JSON_WITH_ELEMENT.as_bytes())]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_spec_element_hashes("main", "foo") {
            BlobFetchResult::Found(map) => {
                // IN-01 element must be in the returned map.
                assert_eq!(map.len(), 1, "expected exactly one element");
                let id = domain::SpecElementId::try_new("IN-01").unwrap();
                assert!(map.contains_key(&id), "IN-01 must be present in the hash map");
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_read_spec_element_hashes_not_found() {
        let dir = setup_repo_with_track("foo", &[]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(matches!(
            reader.read_spec_element_hashes("main", "foo"),
            BlobFetchResult::NotFound
        ));
    }

    #[test]
    fn test_read_spec_element_hashes_fails_closed_on_invalid_spec_json() {
        // A spec.json that is valid JSON but fails spec_codec validation
        // (e.g. wrong schema_version) must produce FetchError, not a partial map.
        let invalid_spec = br#"{"schema_version": 1, "version": "1.0", "title": "F",
            "scope": {"in_scope": [], "out_of_scope": []}, "signals": {"blue":1,"yellow":0,"red":0}}"#;
        let dir = setup_repo_with_track("foo", &[("spec.json", invalid_spec)]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_spec_element_hashes("main", "foo") {
            BlobFetchResult::FetchError(msg) => {
                assert!(
                    msg.contains("validation error") || msg.contains("unsupported"),
                    "expected validation error for wrong schema_version, got: {msg}"
                );
            }
            other => panic!("expected FetchError for invalid spec.json, got {other:?}"),
        }
    }

    #[test]
    fn test_read_spec_element_hashes_fails_on_non_json() {
        let dir = setup_repo_with_track("foo", &[("spec.json", b"not json at all")]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_spec_element_hashes("main", "foo") {
            BlobFetchResult::FetchError(_) => {}
            other => panic!("expected FetchError for non-JSON spec.json, got {other:?}"),
        }
    }

    // --- read_type_signals ---

    /// A minimal valid `<layer>-type-signals.json` payload (schema_version 1).
    ///
    /// `declaration_hash` is all-zeroes — valid per the codec (any 64-char
    /// lowercase hex string is accepted; freshness checking lives in the
    /// caller, not the codec). Used by `read_type_signals` tests that only
    /// need successful decoding.
    const TYPE_SIGNALS_MINIMAL: &str = r#"{
  "schema_version": 1,
  "generated_at": "2026-04-18T12:00:00Z",
  "declaration_hash": "0000000000000000000000000000000000000000000000000000000000000000",
  "signals": [
    { "type_name": "TrackId", "kind_tag": "value_object", "signal": "blue", "found_type": true }
  ]
}"#;

    #[test]
    fn test_read_type_signals_found_decodes_document() {
        // Happy path: signals file exists and decodes correctly.
        // `domain-types.json` → signal file name = `domain-type-signals.json`.
        let dir = setup_repo_with_track(
            "foo",
            &[("domain-type-signals.json", TYPE_SIGNALS_MINIMAL.as_bytes())],
        );
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_type_signals("main", "foo", "domain") {
            BlobFetchResult::Found(doc) => {
                assert_eq!(doc.signals().len(), 1);
                assert_eq!(doc.signals()[0].type_name(), "TrackId");
                assert_eq!(
                    doc.declaration_hash(),
                    "0000000000000000000000000000000000000000000000000000000000000000"
                );
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_read_type_signals_not_found_when_absent() {
        // File absent on the branch → NotFound (not FetchError).
        let dir = setup_repo_with_track("foo", &[]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(
            matches!(reader.read_type_signals("main", "foo", "domain"), BlobFetchResult::NotFound),
            "absent signals file must produce NotFound"
        );
    }

    #[test]
    fn test_read_type_signals_fetch_error_on_decode_failure() {
        // File exists but contains invalid JSON → FetchError with decode error.
        let dir = setup_repo_with_track("foo", &[("domain-type-signals.json", b"not valid json")]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_type_signals("main", "foo", "domain") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("decode error"), "expected decode error, got: {msg}");
            }
            other => panic!("expected FetchError, got {other:?}"),
        }
    }
}
