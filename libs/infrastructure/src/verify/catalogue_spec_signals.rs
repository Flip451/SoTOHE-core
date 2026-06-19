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
use crate::verify::plan_artifact_refs::{canonical_json, canonical_json_sha256};
use crate::verify::tddd_layers;

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
    if let Err(e) = crate::verify::trusted_root::ensure_not_symlink_root(items_dir.clone()) {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "symlink guard: {e}"
        ))]);
    }
    if let Err(e) = crate::verify::trusted_root::ensure_not_symlink_root(workspace_root.clone()) {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "symlink guard: {e}"
        ))]);
    }

    // Containment: verify items_dir resolves under workspace_root.
    // `symlink_metadata()` guards against symlinked roots but does not prevent `..` traversal.
    let canonical_workspace = match workspace_root.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot canonicalize workspace_root {}: {e}",
                workspace_root.display()
            ))]);
        }
    };
    let canonical_items = match items_dir.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "items_dir '{}' is outside workspace_root or does not exist: {e}",
                items_dir.display()
            ))]);
        }
    };
    if !canonical_items.starts_with(&canonical_workspace) {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "items_dir '{}' is outside workspace_root '{}'. Only paths under the workspace are allowed.",
            items_dir.display(),
            workspace_root.display()
        ))]);
    }

    // Security: validate track_id via domain::TrackId before joining onto items_dir.
    // `Path::join` resolves `..`, `/`, and multi-segment paths (`foo/bar`) at the OS
    // level. Using the domain type enforces the slug rules (single-segment, no `..`,
    // no path separators) and makes this function safe when called directly without
    // upstream CLI validation.
    let valid_track_id = match domain::TrackId::try_new(&track_id) {
        Ok(id) => id,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "invalid track_id '{track_id}': {e}",
            ))]);
        }
    };

    // Security: guard the track directory itself against a symlinked subdirectory
    // or a non-directory entry (e.g. a regular file). If `track_dir` is not a real
    // directory, `reject_symlinks_below` will not reject child paths (they appear
    // absent), causing every enabled layer to be skipped and the gate to pass
    // silently. Fail-closed: require `is_dir()` before proceeding.
    let track_dir = items_dir.join(valid_track_id.as_ref());
    match track_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "symlink guard: refusing to follow symlink at track directory: {}",
                track_dir.display()
            ))]);
        }
        Ok(meta) if !meta.file_type().is_dir() => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "expected a directory at track path '{}', found a non-directory entry. \
                 Check --items-dir and --track-id.",
                track_dir.display()
            ))]);
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "track directory not found: {} \
                 (branch maps to missing track; check --items-dir)",
                track_dir.display()
            ))]);
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "symlink guard: cannot stat track directory {}: {e}",
                track_dir.display()
            ))]);
        }
    }

    // Enumerate tddd-enabled layers. Fail-closed: a missing `architecture-rules.json`
    // means we cannot know which catalogues to check.
    let rules_path = workspace_root.join("architecture-rules.json");
    let bindings = match tddd_layers::load_tddd_layers(&rules_path, &workspace_root) {
        Ok(bindings) => bindings,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot load architecture-rules.json at '{}': {e}",
                rules_path.display()
            ))]);
        }
    };

    // Fail closed: an empty bindings list means no TDDD-enabled layers were found.
    if bindings.is_empty() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "no tddd.enabled layers found in architecture-rules.json at '{}'; \
                 cannot verify catalogue-spec signals",
            rules_path.display()
        ))]);
    }

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
        let catalogue_text = match std::fs::read_to_string(&catalogue_path) {
            Ok(s) => s,
            Err(e) => {
                outcome.add(VerifyFinding::error(format!("{}: {e}", catalogue_path.display())));
                continue;
            }
        };
        // T024: v3-native decode via `CatalogueDocumentCodec::decode`.
        // Non-v3 catalogues surface as an error finding (CN-11 fail-closed).
        let stem = catalogue_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .strip_suffix("-types.json")
            .unwrap_or_else(|| {
                catalogue_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown")
            })
            .to_owned();
        let catalogue_doc = match CatalogueDocumentCodec::decode(&catalogue_text, &stem) {
            Ok(d) => d,
            Err(e) => {
                outcome.add(VerifyFinding::error(format!(
                    "{}: decode error: {e:?}",
                    catalogue_path.display()
                )));
                continue;
            }
        };

        // T024: coverage + positional-name check over v3 `CatalogueDocument`.
        // Entry ordering: types (BTreeMap sorted) → traits → functions.
        let catalogue_entries: Vec<CatalogueEntryKey> = catalogue_doc
            .types
            .keys()
            .map(|k| CatalogueEntryKey::new("types", k.as_str()))
            .chain(
                catalogue_doc.traits.keys().map(|k| CatalogueEntryKey::new("traits", k.as_str())),
            )
            .chain(
                catalogue_doc
                    .functions
                    .keys()
                    .map(|k| CatalogueEntryKey::new("functions", k.to_string())),
            )
            .collect();
        let total_entries = catalogue_entries.len();
        if total_entries != doc.signals.len() {
            outcome.add(VerifyFinding::error(format!(
                "{catalogue_file}: catalogue-spec signals coverage mismatch — catalogue has \
                 {total_entries} entry/entries, signals document has {} signal(s). \
                 Run `sotp signal calc-catalog-spec` so every catalogue entry is covered.",
                doc.signals.len()
            )));
            continue;
        }

        if let Some((i, entry, sig)) = catalogue_entries
            .iter()
            .zip(doc.signals.iter())
            .enumerate()
            .find(|(_, (entry, sig))| entry.name.as_str() != sig.type_name.as_str())
            .map(|(i, (entry, sig))| (i, entry, sig))
        {
            let cat_name = &entry.name;
            outcome.add(VerifyFinding::error(format!(
                "{catalogue_file}: catalogue-spec signals positional mismatch at index {i} \
                 (catalogue entry '{cat_name}' vs signal '{}'). \
                 Run `sotp signal calc-catalog-spec` to regenerate.",
                sig.type_name
            )));
            continue;
        }

        let freshness_findings = catalogue_spec_signal_freshness_findings(
            catalogue_file,
            &catalogue_text,
            &catalogue_entries,
            &doc,
        );
        if !freshness_findings.is_empty() {
            outcome.merge(VerifyOutcome::from_findings(freshness_findings));
            continue;
        }

        outcome.merge(check_catalogue_spec_signals(&doc, strict));
    }
    outcome
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
}
