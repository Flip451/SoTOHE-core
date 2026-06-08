//! Catalogue-spec signal gate check (verify catalogue-spec-signals subcommand).
//!
//! All domain type handling is internal to this module. The CLI layer calls
//! `execute_catalogue_spec_signals` passing resolved `PathBuf` arguments and
//! receives a `VerifyOutcome` — no `domain::` imports needed in `apps/cli/src/`.

use std::path::PathBuf;

use domain::ConfidenceSignal;
use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::tddd::catalogue_document_codec::CatalogueDocumentCodec;
use crate::tddd::catalogue_spec_signals_codec;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers;

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
    // Security: guard `items_dir` itself before using it as the trusted root.
    match items_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "symlink guard: refusing to follow symlink at items_dir: {}",
                items_dir.display()
            ))]);
        }
        Ok(_) => {}
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "symlink guard: cannot stat items_dir {}: {e}",
                items_dir.display()
            ))]);
        }
    }

    // Security: guard `workspace_root` against a directly symlinked root directory.
    match workspace_root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "symlink guard: refusing to follow symlink at workspace_root: {}",
                workspace_root.display()
            ))]);
        }
        Ok(_) => {}
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "symlink guard: cannot stat workspace_root {}: {e}",
                workspace_root.display()
            ))]);
        }
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
    let bindings = match reject_symlinks_below(&rules_path, &workspace_root) {
        Ok(true) => {
            let content = match std::fs::read_to_string(&rules_path) {
                Ok(s) => s,
                Err(e) => {
                    return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                        "cannot read architecture-rules.json at '{}': {e}",
                        rules_path.display()
                    ))]);
                }
            };
            match tddd_layers::parse_tddd_layers(&content) {
                Ok(b) => b,
                Err(e) => {
                    return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                        "architecture-rules.json parse error at '{}': {e}",
                        rules_path.display()
                    ))]);
                }
            }
        }
        Ok(false) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "architecture-rules.json not found at '{}'; \
                 cannot enumerate TDDD layers for verification",
                rules_path.display()
            ))]);
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "symlink guard: architecture-rules.json at '{}': {e}",
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

        // T024: inline equivalent of `check_catalogue_spec_signals` over v3 `CatalogueDocument`.
        // Entry ordering: types (BTreeMap sorted) → traits → functions.
        let total_entries =
            catalogue_doc.types.len() + catalogue_doc.traits.len() + catalogue_doc.functions.len();
        if total_entries != doc.signals.len() {
            outcome.add(VerifyFinding::error(format!(
                "{catalogue_file}: catalogue-spec signals coverage mismatch — catalogue has \
                 {total_entries} entry/entries, signals document has {} signal(s). Regenerate \
                 the signals file with `sotp track catalogue-spec-signals` so every catalogue \
                 entry is covered.",
                doc.signals.len()
            )));
            continue;
        }

        let catalogue_names: Vec<String> = catalogue_doc
            .types
            .keys()
            .map(|k| k.as_str().to_owned())
            .chain(catalogue_doc.traits.keys().map(|k| k.as_str().to_owned()))
            .chain(catalogue_doc.functions.keys().map(|k| k.to_string()))
            .collect();
        if let Some((i, cat_name, sig)) = catalogue_names
            .iter()
            .zip(doc.signals.iter())
            .enumerate()
            .find(|(_, (cat_name, sig))| cat_name.as_str() != sig.type_name.as_str())
            .map(|(i, (cat_name, sig))| (i, cat_name, sig))
        {
            outcome.add(VerifyFinding::error(format!(
                "{catalogue_file}: catalogue-spec signals positional mismatch at index {i} \
                 (catalogue entry '{cat_name}' vs signal '{}'). Regenerate the signals file.",
                sig.type_name
            )));
            continue;
        }

        if doc.signals.is_empty() {
            continue;
        }

        let reds: Vec<&str> = doc
            .signals
            .iter()
            .filter(|s| s.signal == ConfidenceSignal::Red)
            .map(|s| s.type_name.as_str())
            .collect();
        if !reds.is_empty() {
            outcome.add(VerifyFinding::error(format!(
                "{catalogue_file}: {} catalogue entry/entries have Red catalogue-spec signal \
                 (missing both spec_refs[] and informal_grounds[] — every entry must carry \
                 at least one grounding ref): {}",
                reds.len(),
                reds.join(", ")
            )));
            continue;
        }

        let yellows: Vec<&str> = doc
            .signals
            .iter()
            .filter(|s| s.signal == ConfidenceSignal::Yellow)
            .map(|s| s.type_name.as_str())
            .collect();
        if !yellows.is_empty() {
            let message = format!(
                "{catalogue_file}: {} catalogue entry/entries have Yellow catalogue-spec signal \
                 — merge gate will block these until upgraded to Blue. Upgrade by promoting \
                 informal_grounds[] to spec_refs[] with anchor + canonical SHA-256 hash: {}",
                yellows.len(),
                yellows.join(", ")
            );
            if strict {
                outcome.add(VerifyFinding::error(message));
            } else {
                outcome.add(VerifyFinding::warning(message));
            }
        }
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
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "MyType": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "struct", "shape": { "kind": "plain" } },
      "docs": "A simple value object."
    }
  },
  "traits": {},
  "functions": {}
}"#;

    /// Minimal valid v3 domain catalogue with no types.
    const V3_CATALOGUE_EMPTY: &str = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {}
}"#;

    /// Builds a `domain-catalogue-spec-signals.json` fixture referencing the given `type_name`
    /// with a Blue signal.
    ///
    /// Uses the production codec shape: `catalogue_declaration_hash` + `signals` array.
    /// `entry_hash` is a required field per T003/AC-06.
    fn signals_referencing_type(type_name: &str) -> String {
        format!(
            r#"{{
  "schema_version": 1,
  "catalogue_declaration_hash": "0000000000000000000000000000000000000000000000000000000000000000",
  "signals": [
    {{"type_name": "{type_name}", "signal": "blue", "entry_hash": "0000000000000000000000000000000000000000000000000000000000000000"}}
  ]
}}"#
        )
    }

    /// A `domain-catalogue-spec-signals.json` referencing `MyType` with a Blue signal.
    fn signals_referencing_existing_type() -> String {
        signals_referencing_type("MyType")
    }

    /// A `domain-catalogue-spec-signals.json` referencing a type `NonExistentType`
    /// that does NOT appear in the v3 catalogue.
    fn signals_referencing_nonexistent_type() -> String {
        signals_referencing_type("NonExistentType")
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
    }

    // -----------------------------------------------------------------------
    // Test: v3 catalogue + signals referencing NON-EXISTENT type → real finding
    // -----------------------------------------------------------------------

    #[test]
    fn test_v3_catalogue_signals_referencing_nonexistent_type_produces_finding() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (items_dir, track_id) = setup_workspace(
            tmp.path(),
            "my-track-2026-01-01",
            V3_CATALOGUE_EMPTY,
            &signals_referencing_nonexistent_type(),
        );

        let outcome =
            execute_catalogue_spec_signals(items_dir, track_id, tmp.path().to_path_buf(), false);

        // The outcome must have at least one finding (signals reference a type that
        // does not exist in the catalogue — proves the validation is real, not pass-through).
        assert!(
            !outcome.is_ok() || !outcome.findings().is_empty(),
            "signals referencing a non-existent type must produce findings; outcome: {:?}",
            outcome.findings()
        );
    }
}
