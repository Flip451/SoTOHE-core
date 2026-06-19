//! Catalogue-spec signal gate check (verify catalogue-spec-signals subcommand).
//!
//! All domain type handling is internal to this module. The CLI layer calls
//! `execute_catalogue_spec_signals` passing resolved `PathBuf` arguments and
//! receives a `VerifyOutcome` — no `domain::` imports needed in `apps/cli/src/`.

use std::path::PathBuf;

use domain::verify::{VerifyFinding, VerifyOutcome};
use domain::{CatalogueSpecSignalsDocument, ContentHash, check_catalogue_spec_signals};

use crate::tddd::catalogue_document_codec::CatalogueDocumentCodec;
use crate::tddd::catalogue_spec_signals_codec;
use crate::tddd::type_signals_codec;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::path_safety::{check_signals_file, normalize_and_guard_path};
use crate::verify::plan_artifact_refs::{canonical_json, canonical_json_sha256};
use crate::verify::tddd_layers::{self, TdddLayerBinding};

/// Validated preflight context for catalogue verification functions.
///
/// Encapsulates the resolved `items_dir`, `workspace_root`, `track_dir`, and TDDD
/// layer `bindings` after all security checks pass. Shared by
/// `execute_catalogue_spec_signals` and `execute_verify_catalogue_spec_refs` to avoid
/// duplicating symlink guards, canonical-containment, `TrackId` validation, track-dir
/// handling, and architecture-rules loading.
pub(crate) struct CatalogueVerifyContext {
    pub(crate) items_dir: PathBuf,
    pub(crate) track_dir: PathBuf,
    pub(crate) bindings: Vec<TdddLayerBinding>,
}

impl CatalogueVerifyContext {
    /// Run all preflight security and existence checks.
    ///
    /// Applies the stricter `catalogue_spec_signals` policy:
    /// - Symlink guards for both `items_dir` and `workspace_root`.
    /// - Canonical workspace containment (`items_dir` must resolve under `workspace_root`).
    /// - `TrackId` validation (prevents `..`/separator injection on path join).
    /// - Track-directory symlink/non-directory/not-found checks (fail-closed).
    /// - `architecture-rules.json` loading; fails closed on missing or empty bindings.
    ///
    /// # Errors
    ///
    /// Returns a human-readable `String` describing the first failing check.
    pub(crate) fn prepare(
        items_dir: PathBuf,
        track_id: &str,
        workspace_root: PathBuf,
    ) -> Result<Self, String> {
        // Security: guard `items_dir` against symlinks at the leaf.
        if let Err(e) = crate::verify::trusted_root::ensure_not_symlink_root(items_dir.clone()) {
            return Err(format!("symlink guard: {e}"));
        }
        // Security: guard `workspace_root` against symlinks at the leaf.
        if let Err(e) = crate::verify::trusted_root::ensure_not_symlink_root(workspace_root.clone())
        {
            return Err(format!("symlink guard: {e}"));
        }

        // Containment: verify items_dir resolves under workspace_root.
        // `symlink_metadata()` guards against symlinked roots but does not prevent `..` traversal.
        let canonical_workspace = workspace_root.canonicalize().map_err(|e| {
            format!("cannot canonicalize workspace_root {}: {e}", workspace_root.display())
        })?;
        let canonical_items = items_dir.canonicalize().map_err(|e| {
            format!(
                "items_dir '{}' is outside workspace_root or does not exist: {e}",
                items_dir.display()
            )
        })?;
        if !canonical_items.starts_with(&canonical_workspace) {
            return Err(format!(
                "items_dir '{}' is outside workspace_root '{}'. Only paths under the workspace are allowed.",
                items_dir.display(),
                workspace_root.display()
            ));
        }

        // Security: validate track_id via domain::TrackId before joining onto items_dir.
        let valid_track_id = domain::TrackId::try_new(track_id)
            .map_err(|e| format!("invalid track_id '{track_id}': {e}"))?;

        // Security: guard the track directory itself against a symlinked subdirectory
        // or a non-directory entry.
        let track_dir = items_dir.join(valid_track_id.as_ref());
        match track_dir.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!(
                    "symlink guard: refusing to follow symlink at track directory: {}",
                    track_dir.display()
                ));
            }
            Ok(meta) if !meta.file_type().is_dir() => {
                return Err(format!(
                    "expected a directory at track path '{}', found a non-directory entry. \
                     Check --items-dir and --track-id.",
                    track_dir.display()
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(format!(
                    "track directory not found: {} \
                     (branch maps to missing track; check --items-dir)",
                    track_dir.display()
                ));
            }
            Err(e) => {
                return Err(format!(
                    "symlink guard: cannot stat track directory {}: {e}",
                    track_dir.display()
                ));
            }
        }

        // Enumerate tddd-enabled layers.
        let rules_path = workspace_root.join("architecture-rules.json");
        let bindings =
            tddd_layers::load_tddd_layers(&rules_path, &workspace_root).map_err(|e| {
                format!("cannot load architecture-rules.json at '{}': {e}", rules_path.display())
            })?;
        if bindings.is_empty() {
            return Err(format!(
                "no tddd.enabled layers found in architecture-rules.json at '{}'; \
                 cannot verify catalogue-spec signals",
                rules_path.display()
            ));
        }

        Ok(Self { items_dir, track_dir, bindings })
    }
}

struct CatalogueEntryKey {
    section: &'static str,
    name: String,
}

impl CatalogueEntryKey {
    fn new(section: &'static str, name: impl Into<String>) -> Self {
        Self { section, name: name.into() }
    }
}

fn content_hash_from_hex(hex: String, context: &str) -> Result<ContentHash, VerifyFinding> {
    ContentHash::try_from_hex(&hex).map_err(|e| {
        VerifyFinding::error(format!("internal: {context} produced a non-canonical hash: {e}"))
    })
}

fn catalogue_spec_signal_freshness_findings(
    catalogue_file: &str,
    catalogue_text: &str,
    entries: &[CatalogueEntryKey],
    doc: &CatalogueSpecSignalsDocument,
) -> Vec<VerifyFinding> {
    let mut findings = Vec::new();
    let current_catalogue_hash = match content_hash_from_hex(
        type_signals_codec::declaration_hash(catalogue_text.as_bytes()),
        "catalogue declaration hash",
    ) {
        Ok(hash) => hash,
        Err(finding) => return vec![finding],
    };
    if doc.catalogue_declaration_hash != current_catalogue_hash {
        findings.push(VerifyFinding::error(format!(
            "{catalogue_file}: catalogue-spec signals are stale — \
             catalogue_declaration_hash {} does not match current catalogue hash {}. \
             Run `sotp signal calc-catalog-spec` to regenerate.",
            doc.catalogue_declaration_hash.to_hex(),
            current_catalogue_hash.to_hex()
        )));
    }

    for (entry, signal) in entries.iter().zip(doc.signals.iter()) {
        let current_entry_hash =
            match compute_catalogue_entry_hash(catalogue_text, entry.section, &entry.name) {
                Ok(hash) => hash,
                Err(e) => {
                    findings.push(VerifyFinding::error(format!(
                        "{catalogue_file}: entry_hash validation failed for '{}': {e}",
                        entry.name
                    )));
                    continue;
                }
            };
        let signal_entry_hash = signal.entry_hash().to_hex();
        if signal_entry_hash != current_entry_hash {
            findings.push(VerifyFinding::error(format!(
                "{catalogue_file}: catalogue-spec signals are stale for '{}' — entry_hash {} \
                 does not match current catalogue entry hash {}. \
                 Run `sotp signal calc-catalog-spec` to regenerate.",
                entry.name, signal_entry_hash, current_entry_hash
            )));
        }
    }
    findings
}

/// Computes the SHA-256 declaration hash for the given catalogue file bytes.
///
/// Exposed for integration tests in other crates that need to build fresh
/// `catalogue-spec-signals.json` fixtures without going through the full
/// refresher pipeline.
pub fn compute_catalogue_declaration_hash(catalogue_bytes: &[u8]) -> String {
    type_signals_codec::declaration_hash(catalogue_bytes)
}

/// Computes the per-entry hash for the given section key and entry name
/// from a catalogue JSON string.
///
/// Exposed for integration tests in other crates that need to build fresh
/// `catalogue-spec-signals.json` fixtures without going through the full
/// refresher pipeline.
pub fn compute_catalogue_entry_hash(
    catalogue_json: &str,
    section: &str,
    entry_key: &str,
) -> Result<String, String> {
    let raw: serde_json::Value = serde_json::from_str(catalogue_json)
        .map_err(|e| format!("cannot parse catalogue JSON: {e}"))?;
    let value = raw.get(section).and_then(|s| s.get(entry_key)).ok_or_else(|| {
        format!("catalogue entry '{entry_key}' not found in section '{section}' of raw JSON")
    })?;
    let json_str = canonical_json(value);
    Ok(canonical_json_sha256(&json_str))
}

/// Core catalogue-spec-signals check logic with explicit, resolved parameters.
///
/// Separated from the git-based branch-resolution entry point so the guard
/// logic (symlink guards, per-layer signals loop) can be exercised from tests
/// without requiring a real git environment.
///
/// # Errors
///
/// Returns a `VerifyOutcome` with error findings on symlink violations, missing
/// `architecture-rules.json`, or signals decode errors.
#[allow(clippy::too_many_lines)]
pub fn execute_catalogue_spec_signals(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    strict: bool,
) -> VerifyOutcome {
    let ctx = match CatalogueVerifyContext::prepare(items_dir, &track_id, workspace_root) {
        Ok(c) => c,
        Err(e) => return VerifyOutcome::from_findings(vec![VerifyFinding::error(e)]),
    };
    let CatalogueVerifyContext { items_dir, track_dir, bindings } = ctx;

    let mut outcome = VerifyOutcome::pass();
    for binding in &bindings {
        if !binding.catalogue_spec_signal_enabled() {
            // ADR §D5.4 phased activation — skip layers that have not opted in.
            continue;
        }
        let layer_id = binding.layer_id();
        let signals_path = track_dir.join(format!("{layer_id}-catalogue-spec-signals.json"));
        // Security: reject symlinks in path components below items_dir before
        // checking file existence or reading. Returns Ok(false) when absent.
        match reject_symlinks_below(&signals_path, &items_dir) {
            Ok(false) => {
                // Lenient CI: missing signals file is "layer not yet activated
                // for catalogue-spec signals".
                continue;
            }
            Ok(true) => {}
            Err(e) => {
                outcome.add(VerifyFinding::error(format!(
                    "symlink guard: {}: {e}",
                    signals_path.display()
                )));
                continue;
            }
        }
        let text = match std::fs::read_to_string(&signals_path) {
            Ok(s) => s,
            Err(e) => {
                outcome.add(VerifyFinding::error(format!("{}: {e}", signals_path.display())));
                continue;
            }
        };
        let doc = match catalogue_spec_signals_codec::decode(&text) {
            Ok(d) => d,
            Err(e) => {
                outcome.add(VerifyFinding::error(format!(
                    "{}: decode error: {e}",
                    signals_path.display()
                )));
                continue;
            }
        };
        let catalogue_file = binding.catalogue_file();
        let catalogue_path = track_dir.join(catalogue_file);
        match reject_symlinks_below(&catalogue_path, &items_dir) {
            Ok(true) => {}
            Ok(false) => {
                outcome.add(VerifyFinding::error(format!(
                    "catalogue file not found: {}",
                    catalogue_path.display()
                )));
                continue;
            }
            Err(e) => {
                outcome.add(VerifyFinding::error(format!(
                    "symlink guard: {}: {e}",
                    catalogue_path.display()
                )));
                continue;
            }
        }
        // T024: decode catalogue and run coverage + positional-name + freshness checks.
        let (catalogue_entries, catalogue_text) =
            match read_and_decode_catalogue(&catalogue_path, &catalogue_path) {
                Ok(pair) => pair,
                Err(err_outcome) => {
                    outcome.merge(err_outcome);
                    continue;
                }
            };
        if let Err(findings) =
            check_catalogue_integrity(catalogue_file, &catalogue_entries, &catalogue_text, &doc)
        {
            outcome.merge(VerifyOutcome::from_findings(findings));
            continue;
        }

        outcome.merge(check_catalogue_spec_signals(&doc, strict));
    }
    outcome
}

/// Run per-catalogue integrity checks (coverage + positional name + entry_hash freshness)
/// against a decoded `CatalogueSpecSignalsDocument`.
///
/// Returns `Ok(())` when all checks pass; `Err(findings)` on the first violation.
fn check_catalogue_integrity(
    signals_display: &str,
    catalogue_entries: &[CatalogueEntryKey],
    catalogue_text: &str,
    doc: &CatalogueSpecSignalsDocument,
) -> Result<(), Vec<VerifyFinding>> {
    let total_entries = catalogue_entries.len();
    if total_entries != doc.signals.len() {
        return Err(vec![VerifyFinding::error(format!(
            "{signals_display}: catalogue-spec signals coverage mismatch — catalogue has \
             {total_entries} entry/entries, signals document has {} signal(s). \
             Run `sotp signal calc-catalog-spec` so every catalogue entry is covered.",
            doc.signals.len()
        ))]);
    }
    if let Some((i, entry, sig)) = catalogue_entries
        .iter()
        .zip(doc.signals.iter())
        .enumerate()
        .find(|(_, (entry, sig))| entry.name.as_str() != sig.type_name.as_str())
        .map(|(i, (entry, sig))| (i, entry, sig))
    {
        let cat_name = &entry.name;
        return Err(vec![VerifyFinding::error(format!(
            "{signals_display}: catalogue-spec signals positional mismatch at index {i} \
             (catalogue entry '{cat_name}' vs signal '{}'). \
             Run `sotp signal calc-catalog-spec` to regenerate.",
            sig.type_name
        ))]);
    }
    let freshness_findings = catalogue_spec_signal_freshness_findings(
        signals_display,
        catalogue_text,
        catalogue_entries,
        doc,
    );
    if !freshness_findings.is_empty() {
        return Err(freshness_findings);
    }
    Ok(())
}

/// Evaluate chain ② (`check-catalog-spec`) gate for a single layer with explicit paths.
///
/// Called by `signal check-catalog-spec --signals-path P --catalog-hash H --gate commit|merge`.
/// Performs symlink guards, freshness checks, coverage/positional/entry_hash integrity,
/// and the Red/Yellow/Blue domain gate. See module-level docs for the full gate rule set.
///
/// # Errors
///
/// Returns a `VerifyOutcome` with error findings on I/O, decode, coverage, or gate failures.
pub fn check_catalog_spec_from_signals_file(
    signals_path: &std::path::Path,
    catalog_hash_hex: &str,
    strict: bool,
) -> VerifyOutcome {
    // Capture signals_path display before moving into closure (for error messages).
    let signals_path_owned = signals_path.to_path_buf();
    check_signals_file(
        signals_path,
        catalog_hash_hex,
        &format!("signals file not found: {}", signals_path.display()),
        |text| catalogue_spec_signals_codec::decode(text).map_err(|e| e.to_string()),
        |doc| doc.catalogue_declaration_hash.to_hex(),
        |recorded, current, path| {
            format!(
                "{}: catalogue-spec signals are stale — \
                 catalogue_declaration_hash {} does not match current catalogue hash {}. \
                 Run `sotp signal calc-catalog-spec` to regenerate.",
                path.display(),
                recorded,
                current
            )
        },
        move |doc, normalized_signals, workspace_root| {
            // Resolve the catalogue path via architecture-rules.json.
            let catalogue_path = match resolve_catalogue_path(
                &normalized_signals,
                &signals_path_owned,
                &workspace_root,
            ) {
                Ok(p) => p,
                Err(outcome) => return outcome,
            };

            // Normalize, contain, and symlink-guard the catalogue path.
            let guarded_catalogue = match normalize_and_guard_path(
                &catalogue_path,
                &workspace_root,
                &catalogue_path,
                &format!("catalogue file not found: {}", catalogue_path.display()),
            ) {
                Ok(p) => p,
                Err(finding) => return VerifyOutcome::from_findings(vec![finding]),
            };

            // Read and decode the catalogue.
            let (catalogue_entries, catalogue_text) =
                match read_and_decode_catalogue(&guarded_catalogue, &signals_path_owned) {
                    Ok(pair) => pair,
                    Err(outcome) => return outcome,
                };

            // Per-entry integrity checks (coverage + positional name + entry_hash freshness).
            let signals_display = signals_path_owned.display().to_string();
            if let Err(findings) = check_catalogue_integrity(
                &signals_display,
                &catalogue_entries,
                &catalogue_text,
                &doc,
            ) {
                return VerifyOutcome::from_findings(findings);
            }

            check_catalogue_spec_signals(&doc, strict)
        },
    )
}

/// Resolve the catalogue path for the layer identified by `normalized_signals`.
///
/// Loads `architecture-rules.json` from `workspace_root`, finds the enabled
/// `TdddLayerBinding` whose `catalogue_spec_signal_file()` equals the filename of
/// `normalized_signals`, then returns
/// `<normalized_signals parent>/<binding.catalogue_file()>`.
fn resolve_catalogue_path(
    normalized_signals: &std::path::Path,
    signals_display: &std::path::Path,
    workspace_root: &std::path::Path,
) -> Result<PathBuf, VerifyOutcome> {
    let rules_path = workspace_root.join("architecture-rules.json");
    let bindings = match tddd_layers::load_tddd_layers(&rules_path, workspace_root) {
        Ok(b) => b,
        Err(e) => {
            return Err(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot load architecture-rules.json for catalogue resolution: {e}"
            ))]));
        }
    };

    let signals_file_name = normalized_signals.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let binding = match bindings.iter().find(|b| {
        b.catalogue_spec_signal_enabled() && b.catalogue_spec_signal_file() == signals_file_name
    }) {
        Some(b) => b,
        None => {
            return Err(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: no enabled TDDD layer in architecture-rules.json has \
                 catalogue_spec_signal_file() == '{}'. \
                 Check --signals-path matches an enabled layer.",
                signals_display.display(),
                signals_file_name
            ))]));
        }
    };

    let track_dir = match normalized_signals.parent() {
        Some(d) => d,
        None => {
            return Err(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: cannot determine track directory (signals_path has no parent)",
                signals_display.display()
            ))]));
        }
    };
    Ok(track_dir.join(binding.catalogue_file()))
}

/// Read and decode a catalogue file, returning `(catalogue_entries, catalogue_text)`.
fn read_and_decode_catalogue(
    catalogue_path: &std::path::Path,
    signals_display: &std::path::Path,
) -> Result<(Vec<CatalogueEntryKey>, String), VerifyOutcome> {
    let catalogue_file = catalogue_path.file_name().and_then(|n| n.to_str()).unwrap_or("<unknown>");
    let catalogue_text = match std::fs::read_to_string(catalogue_path) {
        Ok(s) => s,
        Err(e) => {
            return Err(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot read catalogue {}: {e}",
                catalogue_path.display()
            ))]));
        }
    };
    let stem = catalogue_file
        .strip_suffix("-types.json")
        .unwrap_or_else(|| catalogue_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown"))
        .to_owned();
    let catalogue_doc = match CatalogueDocumentCodec::decode(&catalogue_text, &stem) {
        Ok(d) => d,
        Err(e) => {
            return Err(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: catalogue decode error: {e:?}",
                signals_display.display()
            ))]));
        }
    };
    let catalogue_entries: Vec<CatalogueEntryKey> = catalogue_doc
        .types
        .keys()
        .map(|k| CatalogueEntryKey::new("types", k.as_str()))
        .chain(catalogue_doc.traits.keys().map(|k| CatalogueEntryKey::new("traits", k.as_str())))
        .chain(
            catalogue_doc
                .functions
                .keys()
                .map(|k| CatalogueEntryKey::new("functions", k.to_string())),
        )
        .collect();
    Ok((catalogue_entries, catalogue_text))
}

/// Execute `verify catalogue-spec-signals` after git-based branch resolution.
///
/// Resolves the active track via the shared `ActiveTrackResolveInteractor`
/// (IN-08 / IN-09: consolidates individual auto-detect implementations onto the
/// shared interactor path). Fail-closed on non-track branches: git errors and
/// non-`track/<id>` branches both produce an error finding and exit 1 (CN-01).
/// This removes the previous `[SKIP]` Info behaviour that masked CI failures on
/// unexpected branches.
pub fn execute_catalogue_spec_signals_check(
    items_dir: PathBuf,
    workspace_root: PathBuf,
    strict: bool,
) -> VerifyOutcome {
    use std::sync::Arc;

    use crate::git_cli::{GitRepository, resolve_repo_path};
    use usecase::track_resolution::{ActiveTrackResolveInteractor, ActiveTrackResolveService};

    let repo = match crate::git_cli::SystemGitRepo::discover() {
        Ok(r) => r,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "[ERROR] cannot discover git repository: {e}"
            ))]);
        }
    };

    let repo_root = repo.root().to_path_buf();
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
    let track_id = match interactor.resolve_active_track() {
        Ok(id) => id,
        Err(_e) => {
            // Suppress the nested resolver detail (which may reference
            // "provide an explicit track-id" — an option this command does not
            // expose). The actionable fix is always to switch branches.
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(
                "[ERROR] cannot resolve active track: not on a track/<id> branch.\n\
                 Hint: run this command on a track/<id> branch."
                    .to_owned(),
            )]);
        }
    };

    let resolved_items_dir = resolve_repo_path(&repo_root, &items_dir);
    let resolved_workspace_root = resolve_repo_path(&repo_root, &workspace_root);

    execute_catalogue_spec_signals(resolved_items_dir, track_id, resolved_workspace_root, strict)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Minimal `architecture-rules.json` enabling the `domain` layer with
    /// `catalogue_spec_signal` activated.
    ///
    /// Uses `"crate"` (not `"id"`) as the layer key, matching the serde rename in
    /// `parse_tddd_layers`.  `catalogue_spec_signal` is an object (`{ "enabled": true }`)
    /// because the parser deserialises it as `CatalogueSpecSignalBlock`, not a bool.
    const ARCH_RULES_WITH_DOMAIN: &str = r#"{
  "layers": [
    {
      "crate": "domain",
      "tddd": {
        "enabled": true,
        "catalogue_spec_signal": { "enabled": true }
      }
    }
  ]
}"#;

    /// Minimal valid v3 domain catalogue with a single type `MyType`.
    const V3_CATALOGUE_ONE_TYPE: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "MyType": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "plain" } },
      "docs": "A simple value object."
    }
  },
  "traits": {},
  "functions": {}
}"#;

    /// Builds a `domain-catalogue-spec-signals.json` fixture referencing the given `type_name`
    /// with a Blue signal.
    ///
    /// Uses the production codec shape: `catalogue_declaration_hash` + `signals` array.
    /// `entry_hash` is a required field per T003/AC-06.
    fn signals_referencing_type(
        type_name: &str,
        declaration_hash: &str,
        entry_hash: &str,
    ) -> String {
        format!(
            r#"{{
  "schema_version": 1,
  "catalogue_declaration_hash": "{declaration_hash}",
  "signals": [
    {{"type_name": "{type_name}", "signal": "blue", "entry_hash": "{entry_hash}"}}
  ]
}}"#
        )
    }

    fn declaration_hash_for(catalogue_content: &str) -> String {
        type_signals_codec::declaration_hash(catalogue_content.as_bytes())
    }

    fn entry_hash_for(catalogue_content: &str, section: &str, entry_key: &str) -> String {
        compute_catalogue_entry_hash(catalogue_content, section, entry_key).unwrap()
    }

    /// A `domain-catalogue-spec-signals.json` referencing `MyType` with a Blue signal.
    fn signals_referencing_existing_type() -> String {
        signals_referencing_type(
            "MyType",
            &declaration_hash_for(V3_CATALOGUE_ONE_TYPE),
            &entry_hash_for(V3_CATALOGUE_ONE_TYPE, "types", "MyType"),
        )
    }

    /// A `domain-catalogue-spec-signals.json` referencing a type `NonExistentType`
    /// that does NOT appear in the v3 catalogue.
    fn signals_referencing_nonexistent_type() -> String {
        signals_referencing_type(
            "NonExistentType",
            &declaration_hash_for(V3_CATALOGUE_ONE_TYPE),
            &entry_hash_for(V3_CATALOGUE_ONE_TYPE, "types", "MyType"),
        )
    }

    fn signals_with_hashes(declaration_hash: &str, entry_hash: &str) -> String {
        signals_referencing_type("MyType", declaration_hash, entry_hash)
    }

    #[test]
    fn test_compute_catalogue_declaration_hash_valid_catalogue_matches_declaration_hash() {
        let hash = compute_catalogue_declaration_hash(V3_CATALOGUE_ONE_TYPE.as_bytes());

        assert_eq!(hash, declaration_hash_for(V3_CATALOGUE_ONE_TYPE));
        assert_eq!(hash.len(), 64, "declaration hash must be lowercase hex SHA-256");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "declaration hash must be lowercase hex: {hash}"
        );
    }

    #[test]
    fn test_compute_catalogue_entry_hash_valid_type_matches_canonical_entry_hash() {
        let hash = compute_catalogue_entry_hash(V3_CATALOGUE_ONE_TYPE, "types", "MyType")
            .expect("entry hash should compute for existing type");

        assert_eq!(hash, entry_hash_for(V3_CATALOGUE_ONE_TYPE, "types", "MyType"));
        assert_eq!(hash.len(), 64, "entry hash must be lowercase hex SHA-256");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "entry hash must be lowercase hex: {hash}"
        );
    }

    #[test]
    fn test_compute_catalogue_entry_hash_invalid_json_returns_error() {
        let err = compute_catalogue_entry_hash("{", "types", "MyType").unwrap_err();

        assert!(
            err.contains("cannot parse catalogue JSON"),
            "invalid JSON error must identify parse failure: {err}"
        );
    }

    #[test]
    fn test_compute_catalogue_entry_hash_missing_section_returns_error() {
        let err =
            compute_catalogue_entry_hash(V3_CATALOGUE_ONE_TYPE, "missing", "MyType").unwrap_err();

        assert!(
            err.contains("catalogue entry 'MyType' not found in section 'missing'"),
            "missing section error must identify section and key: {err}"
        );
    }

    #[test]
    fn test_compute_catalogue_entry_hash_missing_key_returns_error() {
        let err =
            compute_catalogue_entry_hash(V3_CATALOGUE_ONE_TYPE, "types", "Missing").unwrap_err();

        assert!(
            err.contains("catalogue entry 'Missing' not found in section 'types'"),
            "missing key error must identify section and key: {err}"
        );
    }

    fn write_file(dir: &std::path::Path, relative: &str, content: &str) {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    /// Set up a minimal workspace with:
    ///   tmp/
    ///     architecture-rules.json
    ///     track/items/<track_id>/
    ///       domain-types.json        (catalogue_content)
    ///       domain-catalogue-spec-signals.json  (signals_content)
    fn setup_workspace(
        tmp: &std::path::Path,
        track_id: &str,
        catalogue_content: &str,
        signals_content: &str,
    ) -> (PathBuf, String) {
        write_file(tmp, "architecture-rules.json", ARCH_RULES_WITH_DOMAIN);
        let items_dir = tmp.join("track").join("items");
        std::fs::create_dir_all(items_dir.join(track_id)).unwrap();
        write_file(tmp, &format!("track/items/{track_id}/domain-types.json"), catalogue_content);
        write_file(
            tmp,
            &format!("track/items/{track_id}/domain-catalogue-spec-signals.json"),
            signals_content,
        );
        (items_dir, track_id.to_owned())
    }

    // -----------------------------------------------------------------------
    // Test: v3 catalogue + valid signals → no UnsupportedSchemaVersion error
    // -----------------------------------------------------------------------

    #[test]
    fn test_v3_catalogue_with_valid_signals_passes_without_unsupported_schema_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (items_dir, track_id) = setup_workspace(
            tmp.path(),
            "my-track-2026-01-01",
            V3_CATALOGUE_ONE_TYPE,
            &signals_referencing_existing_type(),
        );

        let outcome =
            execute_catalogue_spec_signals(items_dir, track_id, tmp.path().to_path_buf(), false);

        // The outcome must NOT contain any error finding with "UnsupportedSchemaVersion".
        let has_unsupported =
            outcome.findings().iter().any(|f| f.message().contains("UnsupportedSchemaVersion"));
        assert!(
            !has_unsupported,
            "v3 catalogue must not produce UnsupportedSchemaVersion; findings: {:?}",
            outcome.findings()
        );
        assert!(
            outcome.findings().is_empty(),
            "valid fresh catalogue-spec signals must pass: {:?}",
            outcome.findings()
        );
    }

    fn assert_stale_declaration_hash_returns_error(strict: bool) {
        use domain::verify::Severity;

        let tmp = tempfile::TempDir::new().unwrap();
        let stale_signals = signals_with_hashes(
            "0000000000000000000000000000000000000000000000000000000000000000",
            &entry_hash_for(V3_CATALOGUE_ONE_TYPE, "types", "MyType"),
        );
        let (items_dir, track_id) = setup_workspace(
            tmp.path(),
            "my-track-2026-01-01",
            V3_CATALOGUE_ONE_TYPE,
            &stale_signals,
        );

        let outcome =
            execute_catalogue_spec_signals(items_dir, track_id, tmp.path().to_path_buf(), strict);

        let stale_error = outcome.findings().iter().find(|f| {
            f.message().contains("catalogue-spec signals are stale")
                && f.severity() == Severity::Error
        });
        assert!(
            stale_error.is_some(),
            "stale catalogue_declaration_hash must be Severity::Error with strict={strict}: {:?}",
            outcome.findings()
        );
    }

    /// CN-04: stale `catalogue_declaration_hash` must produce a `Severity::Error` finding
    /// regardless of the `strict` flag. Verifies both strict=false and strict=true.
    #[test]
    fn test_stale_catalogue_declaration_hash_is_always_error_regardless_of_strict() {
        for strict in [false, true] {
            assert_stale_declaration_hash_returns_error(strict);
        }
    }

    #[test]
    fn test_v3_catalogue_stale_entry_hash_produces_finding() {
        let tmp = tempfile::TempDir::new().unwrap();
        let stale_signals = signals_with_hashes(
            &declaration_hash_for(V3_CATALOGUE_ONE_TYPE),
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        let (items_dir, track_id) = setup_workspace(
            tmp.path(),
            "my-track-2026-01-01",
            V3_CATALOGUE_ONE_TYPE,
            &stale_signals,
        );

        let outcome =
            execute_catalogue_spec_signals(items_dir, track_id, tmp.path().to_path_buf(), false);

        let has_stale_entry = outcome
            .findings()
            .iter()
            .any(|f| f.message().contains("catalogue-spec signals are stale for 'MyType'"));
        assert!(has_stale_entry, "stale entry hash must produce finding: {:?}", outcome.findings());
    }

    // -----------------------------------------------------------------------
    // Test: v3 catalogue + signals referencing NON-EXISTENT type → positional finding
    // -----------------------------------------------------------------------

    #[test]
    fn test_v3_catalogue_signals_referencing_nonexistent_type_produces_positional_finding() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (items_dir, track_id) = setup_workspace(
            tmp.path(),
            "my-track-2026-01-01",
            V3_CATALOGUE_ONE_TYPE,
            &signals_referencing_nonexistent_type(),
        );

        let outcome =
            execute_catalogue_spec_signals(items_dir, track_id, tmp.path().to_path_buf(), false);

        let messages: Vec<&str> = outcome.findings().iter().map(|f| f.message()).collect();
        assert!(
            messages.iter().any(|m| {
                m.contains("positional mismatch")
                    && m.contains("MyType")
                    && m.contains("NonExistentType")
            }),
            "signals referencing a non-existent type must produce a positional mismatch finding; outcome: {:?}",
            messages
        );
        assert!(
            !messages.iter().any(|m| m.contains("coverage mismatch")),
            "test fixture must not stop at coverage mismatch before positional validation: {:?}",
            messages
        );
    }

    // -----------------------------------------------------------------------
    // Tests for check_catalog_spec_from_signals_file (T011 explicit-path gate)
    // -----------------------------------------------------------------------

    use crate::verify::test_support::git_init;

    /// Set up a minimal git repo containing:
    ///   - `architecture-rules.json` (domain layer with catalogue_spec_signal enabled)
    ///   - `domain-types.json` (the catalogue fixture)
    ///   - `domain-catalogue-spec-signals.json` (the signals file with given signal)
    ///
    /// Returns `(TempDir, signals_path, declaration_hash)`.
    fn setup_catalog_spec_git_repo(signal: &str) -> (tempfile::TempDir, PathBuf, String) {
        let dir = tempfile::tempdir().unwrap();
        git_init(dir.path());
        write_file(dir.path(), "architecture-rules.json", ARCH_RULES_WITH_DOMAIN);
        write_file(dir.path(), "domain-types.json", V3_CATALOGUE_ONE_TYPE);
        let declaration_hash = declaration_hash_for(V3_CATALOGUE_ONE_TYPE);
        let entry_hash = entry_hash_for(V3_CATALOGUE_ONE_TYPE, "types", "MyType");
        let signals_content = signals_referencing_type("MyType", &declaration_hash, &entry_hash);
        // Replace "blue" signal with the requested signal value.
        let signals_content = signals_content.replace("\"blue\"", &format!("\"{signal}\""));
        let signals_path = dir.path().join("domain-catalogue-spec-signals.json");
        write_file(dir.path(), "domain-catalogue-spec-signals.json", &signals_content);
        (dir, signals_path, declaration_hash)
    }

    #[test]
    fn test_check_catalog_spec_blue_signal_non_strict_passes() {
        let (_dir, signals_path, declaration_hash) = setup_catalog_spec_git_repo("blue");

        let outcome = check_catalog_spec_from_signals_file(&signals_path, &declaration_hash, false);

        assert!(
            !outcome.has_errors(),
            "blue signal with correct hash must pass (non-strict): {outcome:?}"
        );
    }

    #[test]
    fn test_check_catalog_spec_stale_hash_returns_error() {
        let (_dir, signals_path, _) = setup_catalog_spec_git_repo("blue");
        let stale_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let outcome = check_catalog_spec_from_signals_file(&signals_path, stale_hash, false);

        let has_stale = outcome
            .findings()
            .iter()
            .any(|f| f.message().contains("catalogue-spec signals are stale"));
        assert!(has_stale, "stale catalog_hash must produce a stale-signals error: {outcome:?}");
    }

    #[test]
    fn test_check_catalog_spec_signals_file_not_found_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        git_init(dir.path());
        let missing_path = dir.path().join("domain-catalogue-spec-signals.json");
        let any_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let outcome = check_catalog_spec_from_signals_file(&missing_path, any_hash, false);

        assert!(outcome.has_errors(), "missing signals file must return an error: {outcome:?}");
    }

    #[test]
    fn test_check_catalog_spec_yellow_strict_returns_error() {
        let (_dir, signals_path, declaration_hash) = setup_catalog_spec_git_repo("yellow");

        let outcome = check_catalog_spec_from_signals_file(&signals_path, &declaration_hash, true);

        let has_error =
            outcome.findings().iter().any(|f| f.severity() == domain::verify::Severity::Error);
        assert!(
            has_error,
            "yellow signal with strict=true must produce an error finding: {outcome:?}"
        );
    }
}
