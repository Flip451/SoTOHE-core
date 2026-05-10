//! Catalogue-spec signal gate check (verify catalogue-spec-signals subcommand).
//!
//! All domain type handling is internal to this module. The CLI layer calls
//! `execute_catalogue_spec_signals` passing resolved `PathBuf` arguments and
//! receives a `VerifyOutcome` — no `domain::` imports needed in `apps/cli/src/`.

use std::path::PathBuf;

use domain::check_catalogue_spec_signals;
use domain::verify::{Severity, VerifyFinding, VerifyOutcome};

use crate::tddd::catalogue_bulk_loader::v3_doc_to_stub;
use crate::tddd::catalogue_document_codec::CatalogueDocumentCodec;
use crate::tddd::{catalogue_codec, catalogue_spec_signals_codec};
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
        // Decode the catalogue.  For schema_version=3 (v3) catalogues, fall through
        // to the `CatalogueDocumentCodec` + `v3_doc_to_stub` path which produces a
        // v2-compat `TypeCatalogueDocument` stub so that `check_catalogue_spec_signals`
        // can evaluate the externally-stored signals file against the stub's entries.
        // This is genuine validation: if the signals file references a type that does
        // not exist in the v3 catalogue, `check_catalogue_spec_signals` surfaces a real
        // finding.  It is NOT fail-open.
        let catalogue_doc = match catalogue_codec::decode(&catalogue_text) {
            Ok(d) => d,
            Err(catalogue_codec::TypeCatalogueCodecError::UnsupportedSchemaVersion(_)) => {
                // Try the v3 codec path.  Derive the crate-name stem (e.g. `"domain"` from
                // `"domain-types.json"`) the same way `spec_states` does: strip the
                // `-types.json` suffix from the filename, falling back to `file_stem()`.
                let stem = catalogue_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .strip_suffix("-types.json")
                    .unwrap_or_else(|| {
                        catalogue_path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown")
                    })
                    .to_owned();
                match CatalogueDocumentCodec::decode(&catalogue_text, &stem) {
                    Ok(v3_doc) => match v3_doc_to_stub(&v3_doc) {
                        Ok(stub) => stub,
                        Err(e) => {
                            outcome.add(VerifyFinding::error(format!(
                                "{}: failed to convert v3 catalogue to stub: {e}",
                                catalogue_path.display()
                            )));
                            continue;
                        }
                    },
                    Err(e) => {
                        outcome.add(VerifyFinding::error(format!(
                            "{}: failed to decode as v3 catalogue: {e:?}",
                            catalogue_path.display()
                        )));
                        continue;
                    }
                }
            }
            Err(e) => {
                outcome.add(VerifyFinding::error(format!(
                    "{}: decode error: {e}",
                    catalogue_path.display()
                )));
                continue;
            }
        };
        let layer_outcome =
            check_catalogue_spec_signals(&catalogue_doc, &doc, strict, catalogue_file);
        for finding in layer_outcome.findings() {
            outcome.add(finding.clone());
        }
    }
    outcome
}

/// Execute `verify catalogue-spec-signals` after git-based branch resolution.
///
/// Resolves the active track branch, then delegates to `execute_catalogue_spec_signals`.
/// Follows the fail-closed pattern: git errors → error finding; non-track branches → Info/SKIP.
///
/// Returns a tuple `(label, outcome)` for use with `print_outcome`.
#[allow(clippy::too_many_lines)]
pub fn execute_catalogue_spec_signals_check(
    items_dir: PathBuf,
    workspace_root: PathBuf,
    strict: bool,
) -> VerifyOutcome {
    use crate::git_cli::{GitRepository, resolve_repo_path};
    use usecase::track_resolution::{TrackResolutionError, resolve_track_id_from_branch};

    let repo = match crate::git_cli::SystemGitRepo::discover() {
        Ok(r) => r,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "[ERROR] cannot discover git repository: {e}"
            ))]);
        }
    };
    let branch = match repo.current_branch() {
        Ok(Some(b)) => b,
        Ok(None) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(
                "[ERROR] git rev-parse --abbrev-ref HEAD failed (non-zero exit)".to_owned(),
            )]);
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "[ERROR] cannot read current branch: {e}"
            ))]);
        }
    };
    let track_id = match resolve_track_id_from_branch(Some(branch.as_str())) {
        Ok(id) => id,
        Err(TrackResolutionError::InvalidTrackId(slug, e)) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "[ERROR] invalid track id '{slug}' from branch '{branch}': {e}"
            ))]);
        }
        Err(_) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::new(
                Severity::Info,
                format!("[SKIP] not on a track branch (branch: {branch})"),
            )]);
        }
    };

    let resolved_items_dir = resolve_repo_path(repo.root(), &items_dir);
    let resolved_workspace_root = resolve_repo_path(repo.root(), &workspace_root);

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
    const ARCH_RULES_WITH_DOMAIN: &str = r#"{
  "layers": [
    {
      "id": "domain",
      "tddd": {
        "enabled": true,
        "catalogue_spec_signal": true,
        "spec_states": true
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
      "kind": { "kind": "struct", "pattern": { "pattern": "plain" } },
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

    /// A `domain-catalogue-spec-signals.json` referencing `MyType` with a Blue signal.
    ///
    /// Uses the production codec shape: `catalogue_declaration_hash` + `signals` array.
    fn signals_referencing_existing_type() -> String {
        r#"{
  "schema_version": 1,
  "catalogue_declaration_hash": "0000000000000000000000000000000000000000000000000000000000000000",
  "signals": [
    {"type_name": "MyType", "signal": "blue"}
  ]
}"#
        .to_owned()
    }

    /// A `domain-catalogue-spec-signals.json` referencing a type `NonExistentType`
    /// that does NOT appear in the v3 catalogue.
    ///
    /// Uses the production codec shape: `catalogue_declaration_hash` + `signals` array.
    fn signals_referencing_nonexistent_type() -> String {
        r#"{
  "schema_version": 1,
  "catalogue_declaration_hash": "0000000000000000000000000000000000000000000000000000000000000000",
  "signals": [
    {"type_name": "NonExistentType", "signal": "blue"}
  ]
}"#
        .to_owned()
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
