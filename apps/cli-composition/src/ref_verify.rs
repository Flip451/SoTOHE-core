//! `ref_verify` command family — per-context composition root and CliApp shim.

use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `ref_verify` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct RefVerifyCompositionRoot;

impl RefVerifyCompositionRoot {
    /// Create a new `RefVerifyCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for RefVerifyCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl RefVerifyCompositionRoot {
    /// Build a wired [`cli_driver::ref_verify::RefVerifyDriver`] for the ref_verify family.
    ///
    /// Delegates to `FsRefVerifyAggregateAdapter` in `infrastructure`, which wires both
    /// sub-services internally (D3/D4 cli_driver policy).  Adapter impls belong in
    /// `infrastructure`; `cli_composition` only performs wiring here.
    pub fn ref_verify_driver(&self) -> cli_driver::ref_verify::RefVerifyDriver {
        let service = Arc::new(infrastructure::FsRefVerifyAggregateAdapter::new())
            as Arc<dyn usecase::ref_verify::RefVerifyAggregateService>;
        cli_driver::ref_verify::RefVerifyDriver::new(service)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use super::RefVerifyCompositionRoot;
    use crate::{CommandOutcome, test_support::repo_root_for_tests};
    use cli_driver::ref_verify::{
        RefVerifyChainSelect, RefVerifyCheckApprovedInput as DriverCheckApprovedInput,
        RefVerifyInput, RefVerifyResultsInput as DriverResultsInput,
        RefVerifyRunInput as DriverRunInput, RefVerifyVerdictSelect,
    };

    #[derive(Debug, Clone)]
    struct RefVerifyRunInput {
        track_id: String,
        items_dir: PathBuf,
    }

    #[derive(Debug, Clone)]
    struct RefVerifyCheckApprovedInput {
        track_id: String,
        items_dir: PathBuf,
    }

    fn run_via_driver(
        input: RefVerifyRunInput,
    ) -> Result<CommandOutcome, crate::error::CompositionError> {
        let driver_input = DriverRunInput { track_id: input.track_id, items_dir: input.items_dir };
        Ok(RefVerifyCompositionRoot::new()
            .ref_verify_driver()
            .handle(RefVerifyInput::Run(driver_input)))
    }

    fn check_approved_via_driver(
        input: RefVerifyCheckApprovedInput,
    ) -> Result<CommandOutcome, crate::error::CompositionError> {
        check_selected_chain_approved_via_driver(
            input.track_id,
            input.items_dir,
            RefVerifyChainSelect::All,
        )
    }

    fn check_selected_chain_approved_via_driver(
        track_id: String,
        items_dir: PathBuf,
        chain: RefVerifyChainSelect,
    ) -> Result<CommandOutcome, crate::error::CompositionError> {
        let driver_input = DriverCheckApprovedInput { track_id, items_dir, chain };
        Ok(RefVerifyCompositionRoot::new()
            .ref_verify_driver()
            .handle(RefVerifyInput::CheckApproved(driver_input)))
    }

    fn results_via_driver(
        driver_input: DriverResultsInput,
    ) -> Result<CommandOutcome, crate::error::CompositionError> {
        Ok(RefVerifyCompositionRoot::new()
            .ref_verify_driver()
            .handle(RefVerifyInput::Results(driver_input)))
    }

    fn with_env_var<T>(key: &'static str, value: OsString, run: impl FnOnce() -> T) -> T {
        let previous = std::env::var_os(key);
        // Safety: callers hold process_env_lock for this helper's full execution.
        unsafe {
            std::env::set_var(key, value);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
        unsafe {
            if let Some(value) = previous {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
        match result {
            Ok(v) => v,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn temp_project_with_items_dir() -> (tempfile::TempDir, PathBuf) {
        let base = repo_root_for_tests().join("target").join("ref-verify-cli-tests");
        std::fs::create_dir_all(&base).expect("test temp base must be creatable");
        let tmp = tempfile::Builder::new()
            .prefix("proj-")
            .tempdir_in(base)
            .expect("repo-local temp project dir must be creatable");
        let items_dir = tmp.path().join("track").join("items");
        std::fs::create_dir_all(&items_dir).expect("track/items must be creatable");
        (tmp, items_dir)
    }

    fn project_root_from_items_dir(items_dir: &Path) -> &Path {
        items_dir.parent().and_then(Path::parent).unwrap()
    }

    fn write_chain1_fixture(items_dir: &Path, track_id: &str) {
        let project_root = project_root_from_items_dir(items_dir);
        let track_items_dir = items_dir.join(track_id);
        let adr_dir = project_root.join("knowledge").join("adr");
        std::fs::create_dir_all(&track_items_dir).unwrap();
        std::fs::create_dir_all(&adr_dir).unwrap();
        // All-scope pair loading requires architecture-rules.json; a no-TDDD
        // placeholder keeps Chain2 empty for Chain1-only fixtures.
        write_architecture_rules_no_tddd(project_root);
        std::fs::write(
            track_items_dir.join("spec.json"),
            serde_json::json!({
                "schema_version": 2,
                "version": "0.1",
                "title": "Test",
                "goal": [{
                    "id": "GO-01",
                    "text": "The guarded path must stay inside the trusted repository root.",
                    "adr_refs": [{ "file": "knowledge/adr/decision.md", "anchor": "D1" }]
                }],
                "scope": { "in_scope": [], "out_of_scope": [] },
                "constraints": [],
                "acceptance_criteria": []
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            adr_dir.join("decision.md"),
            "---\n\
            adr_id: test-adr\n\
            decisions:\n\
            \x20\x20- id: D1\n\
            \x20\x20\x20\x20status: proposed\n\
            \x20\x20\x20\x20candidate_selection: \"choose the guarded path\"\n\
            ---\n\
            # ADR\n\n\
            ### D1: Guarded path decision\n\
            The guarded path must stay inside the trusted repository root.\n",
        )
        .unwrap();
    }

    fn write_architecture_rules_no_tddd(project_root: &Path) {
        std::fs::write(
            project_root.join("architecture-rules.json"),
            r#"{"layers":[{"crate":"placeholder-no-tddd"}]}"#,
        )
        .unwrap();
    }

    /// Overwrites `architecture-rules.json` with a TDDD-enabled `test-domain` layer and
    /// writes `test-domain-types.json` into the track directory with a single type entry
    /// that has a `spec_ref` pointing to `GO-01` in the track's `spec.json`.
    ///
    /// This creates a Chain-2 pair that is only loaded when the resolved scope is
    /// `Chain2` or `All`, so it discriminates correct scope resolution from Chain1-only.
    ///
    /// Must be called **after** `write_chain1_fixture` (which creates `spec.json` and
    /// the track directory).
    fn add_chain2_tddd_layer_to_fixture(items_dir: &Path, track_id: &str) {
        let project_root = project_root_from_items_dir(items_dir);
        let track_items_dir = items_dir.join(track_id);

        // Enable TDDD for the "test_domain" layer using the default catalogue filename
        // (`test_domain-types.json`).  The layer id must be a valid Rust identifier, so
        // use underscores — `"test-domain"` would be rejected by the catalogue codec.
        std::fs::write(
            project_root.join("architecture-rules.json"),
            r#"{"layers":[{"crate":"placeholder_no_tddd"},{"crate":"test_domain","tddd":{"enabled":true}}]}"#,
        )
        .unwrap();

        // Write a minimal catalogue with one type entry that references GO-01 in spec.json.
        let spec_ref_path = format!("track/items/{track_id}/spec.json");
        let catalogue = serde_json::json!({
            "schema_version": 5,
            "crate_name": "test_domain",
            "layer": "test_domain",
            "types": {
                "TestGuardedPath": {
                    "action": "add",
                    "role": { "ValueObject": {} },
                    "kind": { "kind": "struct", "shape": { "kind": "unit" } },
                    "methods": [],
                    "module_path": "test_domain",
                    "spec_refs": [{
                        "file": spec_ref_path,
                        "anchor": "GO-01"
                    }],
                    "informal_grounds": []
                }
            },
            "traits": {},
            "functions": {}
        });
        std::fs::write(track_items_dir.join("test_domain-types.json"), catalogue.to_string())
            .unwrap();
    }

    #[derive(Debug, thiserror::Error)]
    #[error("{0}")]
    struct RefVerifyTestError(String);

    fn ref_verify_chain1_cmd(
        track_id: &str,
    ) -> Result<usecase::ref_verify::RefVerifyCommand, RefVerifyTestError> {
        Ok(usecase::ref_verify::RefVerifyCommand {
            track_id: domain::TrackId::try_new(track_id.to_owned())
                .map_err(|e| RefVerifyTestError(format!("invalid track ID: {e}")))?,
            scope: usecase::ref_verify::RefVerifyScope::Chain1,
            current_branch: format!("track/{track_id}"),
        })
    }

    fn ref_verify_chain2_cmd(
        track_id: &str,
    ) -> Result<usecase::ref_verify::RefVerifyCommand, RefVerifyTestError> {
        Ok(usecase::ref_verify::RefVerifyCommand {
            track_id: domain::TrackId::try_new(track_id.to_owned())
                .map_err(|e| RefVerifyTestError(format!("invalid track ID: {e}")))?,
            scope: usecase::ref_verify::RefVerifyScope::Chain2 {
                layer: domain::tddd::LayerId::try_new("test_domain".to_owned())
                    .map_err(|e| RefVerifyTestError(format!("invalid layer ID: {e}")))?,
            },
            current_branch: format!("track/{track_id}"),
        })
    }

    fn write_cache_for_first_chain1_pair(
        items_dir: &Path,
        track_id: &str,
        verdicts: Vec<domain::tddd::semantic_verify::SemanticVerdict>,
    ) {
        use domain::tddd::semantic_verify::SemanticVerifyEntry;
        use infrastructure::ref_verify::{RefVerifyCacheAdapter, RefVerifyPairSourceAdapter};
        use usecase::ref_verify::{
            RefVerifyCachePort as _, RefVerifyCacheScope, RefVerifyPairSourcePort as _,
        };

        let project_root = project_root_from_items_dir(items_dir).to_path_buf();
        let cmd = ref_verify_chain1_cmd(track_id).unwrap();
        let pair_source = RefVerifyPairSourceAdapter::new(project_root.clone());
        let pairs =
            pair_source.load_pairs(&cmd, &usecase::ref_verify::RefVerifyConfig::default()).unwrap();
        let pair = pairs.into_iter().find(|pair| !pair.known_bad).unwrap();
        // Use the actual pair origins so that the four-field cache lookup
        // (claim_hash, evidence_hash, claim_origin, evidence_origin) succeeds.
        let entries = verdicts
            .into_iter()
            .map(|verdict| {
                SemanticVerifyEntry::new(
                    pair.claim_hash.clone(),
                    pair.evidence_hash.clone(),
                    verdict,
                    pair.claim_origin.clone(),
                    pair.evidence_origin.clone(),
                )
            })
            .collect();
        RefVerifyCacheAdapter::new(project_root)
            .save_entries(&cmd, &RefVerifyCacheScope::SpecAdr, entries)
            .unwrap();
    }

    fn write_pass_cache_for_first_chain1_pair(items_dir: &Path, track_id: &str) {
        use domain::tddd::semantic_verify::{EvidenceCitation, SemanticVerdict};

        write_cache_for_first_chain1_pair(
            items_dir,
            track_id,
            vec![SemanticVerdict::Pass {
                citation: EvidenceCitation::try_new("guarded path decision".to_owned()).unwrap(),
            }],
        );
    }

    fn write_stale_pass_cache_for_first_chain1_pair(items_dir: &Path, track_id: &str) {
        use domain::ContentHash;
        use domain::tddd::semantic_verify::{
            EvidenceCitation, SemanticVerdict, SemanticVerifyEntry,
        };
        use infrastructure::ref_verify::{RefVerifyCacheAdapter, RefVerifyPairSourceAdapter};
        use usecase::ref_verify::{
            RefVerifyCachePort as _, RefVerifyCacheScope, RefVerifyPairSourcePort as _,
        };

        let project_root = project_root_from_items_dir(items_dir).to_path_buf();
        let cmd = ref_verify_chain1_cmd(track_id).unwrap();
        let pair = RefVerifyPairSourceAdapter::new(project_root.clone())
            .load_pairs(&cmd, &usecase::ref_verify::RefVerifyConfig::default())
            .unwrap()
            .into_iter()
            .find(|pair| !pair.known_bad)
            .unwrap();
        let stale_entry = SemanticVerifyEntry::new(
            ContentHash::from_bytes([0; 32]),
            pair.evidence_hash,
            SemanticVerdict::Pass {
                citation: EvidenceCitation::try_new("stale guarded path decision".to_owned())
                    .unwrap(),
            },
            pair.claim_origin,
            pair.evidence_origin,
        );
        RefVerifyCacheAdapter::new(project_root)
            .save_entries(&cmd, &RefVerifyCacheScope::SpecAdr, vec![stale_entry])
            .unwrap();
    }

    fn write_cache_for_first_chain2_pair(
        items_dir: &Path,
        track_id: &str,
        verdicts: Vec<domain::tddd::semantic_verify::SemanticVerdict>,
    ) {
        use domain::tddd::semantic_verify::SemanticVerifyEntry;
        use infrastructure::ref_verify::{RefVerifyCacheAdapter, RefVerifyPairSourceAdapter};
        use usecase::ref_verify::{
            RefVerifyCachePort as _, RefVerifyCacheScope, RefVerifyPairSourcePort as _,
        };

        let project_root = project_root_from_items_dir(items_dir).to_path_buf();
        let cmd = ref_verify_chain2_cmd(track_id).unwrap();
        let pair = RefVerifyPairSourceAdapter::new(project_root.clone())
            .load_pairs(&cmd, &usecase::ref_verify::RefVerifyConfig::default())
            .unwrap()
            .into_iter()
            .find(|pair| !pair.known_bad)
            .unwrap();
        let entries = verdicts
            .into_iter()
            .map(|verdict| {
                SemanticVerifyEntry::new(
                    pair.claim_hash.clone(),
                    pair.evidence_hash.clone(),
                    verdict,
                    pair.claim_origin.clone(),
                    pair.evidence_origin.clone(),
                )
            })
            .collect();
        let layer = domain::tddd::LayerId::try_new("test_domain".to_owned()).unwrap();
        RefVerifyCacheAdapter::new(project_root)
            .save_entries(&cmd, &RefVerifyCacheScope::CatalogueSpec { layer }, entries)
            .unwrap();
    }

    fn write_pass_cache_for_first_chain2_pair(items_dir: &Path, track_id: &str) {
        use domain::tddd::semantic_verify::{EvidenceCitation, SemanticVerdict};

        write_cache_for_first_chain2_pair(
            items_dir,
            track_id,
            vec![SemanticVerdict::Pass {
                citation: EvidenceCitation::try_new("guarded catalogue reference".to_owned())
                    .unwrap(),
            }],
        );
    }

    fn write_stale_pass_cache_for_first_chain2_pair(items_dir: &Path, track_id: &str) {
        use domain::ContentHash;
        use domain::tddd::semantic_verify::{
            EvidenceCitation, SemanticVerdict, SemanticVerifyEntry,
        };
        use infrastructure::ref_verify::{RefVerifyCacheAdapter, RefVerifyPairSourceAdapter};
        use usecase::ref_verify::{
            RefVerifyCachePort as _, RefVerifyCacheScope, RefVerifyPairSourcePort as _,
        };

        let project_root = project_root_from_items_dir(items_dir).to_path_buf();
        let cmd = ref_verify_chain2_cmd(track_id).unwrap();
        let pair = RefVerifyPairSourceAdapter::new(project_root.clone())
            .load_pairs(&cmd, &usecase::ref_verify::RefVerifyConfig::default())
            .unwrap()
            .into_iter()
            .find(|pair| !pair.known_bad)
            .unwrap();
        let stale_entry = SemanticVerifyEntry::new(
            ContentHash::from_bytes([0; 32]),
            pair.evidence_hash,
            SemanticVerdict::Pass {
                citation: EvidenceCitation::try_new("stale guarded catalogue reference".to_owned())
                    .unwrap(),
            },
            pair.claim_origin,
            pair.evidence_origin,
        );
        let layer = domain::tddd::LayerId::try_new("test_domain".to_owned()).unwrap();
        RefVerifyCacheAdapter::new(project_root)
            .save_entries(&cmd, &RefVerifyCacheScope::CatalogueSpec { layer }, vec![stale_entry])
            .unwrap();
    }

    #[cfg(unix)]
    fn run_ref_verify_with_fake_claude(
        project_root: &Path,
        track_id: &str,
        items_dir: PathBuf,
    ) -> CommandOutcome {
        write_ref_verifier_profiles(project_root);
        let fake_claude_dir = write_fake_claude_into_path_dir(project_root);

        with_fake_track_branch_and_path(project_root, track_id, &fake_claude_dir, || {
            run_via_driver(RefVerifyRunInput { track_id: track_id.to_owned(), items_dir }).unwrap()
        })
    }

    #[cfg(unix)]
    fn write_ref_verifier_profiles(project_root: &Path) {
        let config_dir = project_root.join(".harness").join("config");
        let prompt_dir = project_root.join(".harness").join("prompts");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&prompt_dir).unwrap();
        std::fs::write(
            prompt_dir.join("ref-verifier-chain1.md"),
            "{{claim}}\n{{evidence}}\n{{tier}}",
        )
        .unwrap();
        std::fs::write(
            prompt_dir.join("ref-verifier-chain2.md"),
            "{{claim}}\n{{evidence}}\n{{tier}}",
        )
        .unwrap();
        std::fs::write(
            config_dir.join("agent-profiles.json"),
            r#"{
  "schema_version": 1,
  "providers": { "claude": { "label": "Claude Code" } },
  "capabilities": {
    "ref-verifier-chain1": {
      "provider": "claude",
      "model": "claude-test",
      "reasoning_effort": "high",
      "prompt_template_path": ".harness/prompts/ref-verifier-chain1.md",
      "execution_mode": "typed-pipeline"
    },
    "ref-verifier-chain2": {
      "provider": "claude",
      "model": "claude-test",
      "reasoning_effort": "high",
      "prompt_template_path": ".harness/prompts/ref-verifier-chain2.md",
      "execution_mode": "typed-pipeline"
    }
  }
}"#,
        )
        .unwrap();
    }

    #[cfg(unix)]
    fn write_fake_claude_into_path_dir(project_root: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;

        let fake_bin_dir = project_root.join("fake-claude-bin");
        std::fs::create_dir_all(&fake_bin_dir).unwrap();
        let script = fake_bin_dir.join("claude");
        std::fs::write(
            &script,
            r#"#!/bin/sh
orig_args=" $* "
case "$orig_args" in *known-bad-probe*) printf '{"type":"result","structured_output":{"kind":"fail","reason":"known bad probe","citation":null}}\n'; exit 0 ;; esac
printf '{"type":"result","structured_output":{"kind":"pass","citation":"claude ok","reason":null}}\n'
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        fake_bin_dir
    }

    #[cfg(unix)]
    fn with_fake_git_branch<T>(project_root: &Path, branch: &str, run: impl FnOnce() -> T) -> T {
        use std::os::unix::fs::PermissionsExt as _;

        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let fake_bin_dir = project_root.join("fake-git-bin");
        std::fs::create_dir_all(&fake_bin_dir).unwrap();
        let git_script = fake_bin_dir.join("git");
        std::fs::write(
            &git_script,
            r#"#!/bin/sh
if [ "$1" = "rev-parse" ] && [ "$2" = "--show-toplevel" ]; then
  printf '%s\n' "$SOTP_FAKE_GIT_ROOT"
  exit 0
fi
if [ "$1" = "rev-parse" ] && [ "$2" = "--abbrev-ref" ] && [ "$3" = "HEAD" ]; then
  printf '%s\n' "$SOTP_FAKE_GIT_BRANCH"
  exit 0
fi
printf 'unexpected git invocation: %s\n' "$*" >&2
exit 64
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&git_script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&git_script, perms).unwrap();

        let mut path_entries = vec![fake_bin_dir];
        if let Some(path) = std::env::var_os("PATH") {
            path_entries.extend(std::env::split_paths(&path));
        }
        let path = std::env::join_paths(path_entries).unwrap();

        with_env_var("PATH", path, || {
            with_env_var("SOTP_FAKE_GIT_ROOT", project_root.as_os_str().to_os_string(), || {
                with_env_var("SOTP_FAKE_GIT_BRANCH", OsString::from(branch), run)
            })
        })
    }

    #[cfg(unix)]
    fn with_fake_track_branch<T>(
        project_root: &Path,
        track_id: &str,
        run: impl FnOnce() -> T,
    ) -> T {
        with_fake_git_branch(project_root, &format!("track/{track_id}"), run)
    }

    #[cfg(unix)]
    fn with_fake_track_branch_and_path<T>(
        project_root: &Path,
        track_id: &str,
        extra_bin_dir: &Path,
        run: impl FnOnce() -> T,
    ) -> T {
        let extra = extra_bin_dir.to_path_buf();
        with_fake_track_branch(project_root, track_id, || {
            let mut path_entries = vec![extra.clone()];
            if let Some(path) = std::env::var_os("PATH") {
                path_entries.extend(std::env::split_paths(&path));
            }
            let path = std::env::join_paths(path_entries).unwrap();
            with_env_var("PATH", path, run)
        })
    }

    // ── ref_verify_check_approved ────────────────────────────────────────────

    #[test]
    fn test_ref_verify_check_approved_invalid_track_id_returns_error() {
        let outcome = check_approved_via_driver(RefVerifyCheckApprovedInput {
            track_id: "../outside".to_owned(),
            items_dir: repo_root_for_tests().join("track").join("items"),
        })
        .unwrap();
        let msg = outcome.stderr.as_deref().unwrap_or_default();
        assert_eq!(outcome.exit_code, 1, "invalid track id must fail, got: {outcome:?}");
        assert!(
            msg.contains("invalid --track-id") || msg.contains("invalid track"),
            "invalid track id must be rejected, got: {msg}"
        );
    }

    #[test]
    fn test_ref_verify_check_approved_outside_repo_items_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = check_approved_via_driver(RefVerifyCheckApprovedInput {
            track_id: "my-track".to_owned(),
            items_dir: dir.path().to_path_buf(),
        });
        let outcome = result.unwrap();
        let msg = outcome.stderr.as_deref().unwrap_or_default();
        assert_eq!(outcome.exit_code, 1, "outside items_dir must fail, got: {outcome:?}");
        assert!(
            msg.contains("items-dir") || msg.contains("project root"),
            "items_dir outside repo must be rejected, got: {msg}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain2_missing_track_exits_one() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain2-missing-track";

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain2,
            )
        })
        .unwrap();
        let error = outcome.stderr.as_deref().unwrap_or_default();

        assert_eq!(outcome.exit_code, 1, "missing selected Chain-2 track must block: {outcome:?}");
        assert!(
            error.contains("track directory not found"),
            "missing selected Chain-2 track must fail closed, got: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_non_vacuous_pass_cache_exits_zero() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-approved-pass-cache";
        write_chain1_fixture(&items_dir, track_id);
        write_pass_cache_for_first_chain1_pair(&items_dir, track_id);

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_approved_via_driver(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir,
            })
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 0, "expected approved outcome: {outcome:?}");
    }

    /// A selected Chain-1 gate succeeds when its real pair is current and
    /// verified, even while the independently scoped Chain-2 pair is pending.
    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain1_pass_ignores_pending_chain2() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain1-pass";
        write_chain1_fixture(&items_dir, track_id);
        add_chain2_tddd_layer_to_fixture(&items_dir, track_id);
        write_pass_cache_for_first_chain1_pair(&items_dir, track_id);

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain1,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 0, "selected Chain-1 must be approved: {outcome:?}");
    }

    /// A selected Chain-1 gate does not parse Chain-2 rules at all.  A malformed
    /// Chain-2 configuration therefore cannot block an independently approved
    /// Chain-1 cache snapshot.
    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain1_pass_ignores_unenumerable_chain2() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain1-unenumerable-chain2";
        write_chain1_fixture(&items_dir, track_id);
        write_pass_cache_for_first_chain1_pair(&items_dir, track_id);
        std::fs::write(project_root.join("architecture-rules.json"), "{ malformed rules").unwrap();

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain1,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 0, "selected Chain-1 must be approved: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain2_missing_rules_exits_one() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain2-missing-rules";
        std::fs::create_dir_all(items_dir.join(track_id)).unwrap();

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain2,
            )
        })
        .unwrap();
        let error = outcome.stderr.as_deref().unwrap_or_default();

        assert_eq!(outcome.exit_code, 1, "missing Chain-2 rules must block: {outcome:?}");
        assert!(
            error.contains("cannot load TDDD layer bindings for selected Chain-2 approval"),
            "missing selected Chain-2 rules must fail closed, got: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain2_catalogue_without_spec_exits_one() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain2-catalogue-without-spec";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            project_root.join("architecture-rules.json"),
            r#"{"layers":[{"crate":"test_domain","tddd":{"enabled":true}}]}"#,
        )
        .unwrap();
        std::fs::write(track_dir.join("test_domain-types.json"), "{}").unwrap();

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain2,
            )
        })
        .unwrap();
        let error = outcome.stderr.as_deref().unwrap_or_default();

        assert_eq!(outcome.exit_code, 1, "catalogue without spec must block: {outcome:?}");
        assert!(
            error.contains("spec.json not found") && error.contains("SoT Chain ordering violation"),
            "selected Chain-2 catalogue before spec must fail closed, got: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain2_spec_directory_exits_one() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain2-spec-directory";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(track_dir.join("spec.json")).unwrap();
        std::fs::write(
            project_root.join("architecture-rules.json"),
            r#"{"layers":[{"crate":"test_domain","tddd":{"enabled":true}}]}"#,
        )
        .unwrap();
        std::fs::write(track_dir.join("test_domain-types.json"), "{}").unwrap();

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain2,
            )
        })
        .unwrap();
        let error = outcome.stderr.as_deref().unwrap_or_default();

        assert_eq!(outcome.exit_code, 1, "spec directory must block: {outcome:?}");
        assert!(
            error.contains("is not a regular file"),
            "selected Chain-2 spec directory must fail closed, got: {error}"
        );
    }

    /// A selected Chain-2 gate succeeds when its real pair is current and
    /// verified, even while the independently scoped Chain-1 pair is pending.
    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain2_pass_ignores_pending_chain1() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain2-pass";
        write_chain1_fixture(&items_dir, track_id);
        add_chain2_tddd_layer_to_fixture(&items_dir, track_id);
        write_pass_cache_for_first_chain2_pair(&items_dir, track_id);

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain2,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 0, "selected Chain-2 must be approved: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain1_absent_cache_exits_one() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain1-absent";
        write_chain1_fixture(&items_dir, track_id);

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain1,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 1, "absent Chain-1 cache must block: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain1_stale_cache_exits_one() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain1-stale";
        write_chain1_fixture(&items_dir, track_id);
        write_stale_pass_cache_for_first_chain1_pair(&items_dir, track_id);

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain1,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 1, "stale Chain-1 cache must block: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain1_pending_cache_exits_one() {
        use domain::tddd::semantic_verify::SemanticVerdict;

        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain1-pending";
        write_chain1_fixture(&items_dir, track_id);
        write_cache_for_first_chain1_pair(&items_dir, track_id, vec![SemanticVerdict::Pending]);

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain1,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 1, "pending Chain-1 cache must block: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain1_failed_cache_exits_one() {
        use domain::tddd::semantic_verify::SemanticVerdict;

        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain1-failed";
        write_chain1_fixture(&items_dir, track_id);
        write_cache_for_first_chain1_pair(
            &items_dir,
            track_id,
            vec![SemanticVerdict::Fail {
                reason: "selected Chain-1 verification failed".to_owned(),
            }],
        );

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain1,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 1, "failed Chain-1 cache must block: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain1_duplicate_pass_fail_exits_one() {
        use domain::tddd::semantic_verify::{EvidenceCitation, SemanticVerdict};

        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain1-duplicate";
        write_chain1_fixture(&items_dir, track_id);
        write_cache_for_first_chain1_pair(
            &items_dir,
            track_id,
            vec![
                SemanticVerdict::Pass {
                    citation: EvidenceCitation::try_new("guarded path decision".to_owned())
                        .unwrap(),
                },
                SemanticVerdict::Fail { reason: "duplicate selected Chain-1 failure".to_owned() },
            ],
        );

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain1,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 1, "duplicate Chain-1 failure must block: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain2_absent_cache_exits_one() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain2-absent";
        write_chain1_fixture(&items_dir, track_id);
        add_chain2_tddd_layer_to_fixture(&items_dir, track_id);

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain2,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 1, "absent Chain-2 cache must block: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain2_stale_cache_exits_one() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain2-stale";
        write_chain1_fixture(&items_dir, track_id);
        add_chain2_tddd_layer_to_fixture(&items_dir, track_id);
        write_stale_pass_cache_for_first_chain2_pair(&items_dir, track_id);

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain2,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 1, "stale Chain-2 cache must block: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain2_pending_cache_exits_one() {
        use domain::tddd::semantic_verify::SemanticVerdict;

        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain2-pending";
        write_chain1_fixture(&items_dir, track_id);
        add_chain2_tddd_layer_to_fixture(&items_dir, track_id);
        write_cache_for_first_chain2_pair(&items_dir, track_id, vec![SemanticVerdict::Pending]);

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain2,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 1, "pending Chain-2 cache must block: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain2_failed_cache_exits_one() {
        use domain::tddd::semantic_verify::SemanticVerdict;

        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain2-failed";
        write_chain1_fixture(&items_dir, track_id);
        add_chain2_tddd_layer_to_fixture(&items_dir, track_id);
        write_cache_for_first_chain2_pair(
            &items_dir,
            track_id,
            vec![SemanticVerdict::Fail {
                reason: "selected Chain-2 verification failed".to_owned(),
            }],
        );

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain2,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 1, "failed Chain-2 cache must block: {outcome:?}");
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|message| message.contains("selected Chain-2 verification failed")),
            "failure reason must surface through selected Chain-2: {outcome:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_selected_chain2_duplicate_pass_fail_exits_one() {
        use domain::tddd::semantic_verify::{EvidenceCitation, SemanticVerdict};

        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-selected-chain2-duplicate";
        write_chain1_fixture(&items_dir, track_id);
        add_chain2_tddd_layer_to_fixture(&items_dir, track_id);
        write_cache_for_first_chain2_pair(
            &items_dir,
            track_id,
            vec![
                SemanticVerdict::Pass {
                    citation: EvidenceCitation::try_new("catalogue reference".to_owned()).unwrap(),
                },
                SemanticVerdict::Fail { reason: "duplicate selected Chain-2 failure".to_owned() },
            ],
        );

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_selected_chain_approved_via_driver(
                track_id.to_owned(),
                items_dir,
                RefVerifyChainSelect::Chain2,
            )
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 1, "duplicate Chain-2 failure must block: {outcome:?}");
    }

    /// Discriminates the All-scope pair set in `ref_verify_check_approved`.
    ///
    /// Setup: Chain-1 fixture + Chain-2 TDDD layer (`test-domain`).  Only the
    /// Chain-1 Pass cache is written; the Chain-2 cache (`test-domain-catalogue-
    /// spec-verify-cache.json`) is intentionally absent.
    ///
    /// Expected: `ref_verify_check_approved` exits 1 with a "no Pass cache entry"
    /// message for the Chain-2 pair.
    ///
    /// If the existence-based resolution wrongly derived a Chain1-only pair set, only Chain-1 pairs
    /// would be loaded, the Chain-2 pair would never appear, and the function would
    /// exit 0 — causing this test to fail and revealing the regression.
    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_chain2_missing_cache_exits_one() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-approved-chain2-missing";
        write_chain1_fixture(&items_dir, track_id);
        // Add Chain-2 TDDD layer — this introduces a Chain-2 pair that CommitGate (All) will load.
        add_chain2_tddd_layer_to_fixture(&items_dir, track_id);
        // Write Pass cache for Chain-1 only; Chain-2 cache is intentionally absent.
        write_pass_cache_for_first_chain1_pair(&items_dir, track_id);

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_approved_via_driver(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir,
            })
        })
        .unwrap();

        assert_eq!(
            outcome.exit_code, 1,
            "CommitGate (All) must detect the missing Chain-2 cache: {outcome:?}"
        );
        assert!(
            outcome.stderr.as_deref().is_some_and(|s| s.contains("no Pass cache entry")),
            "expected 'no Pass cache entry' message for the Chain-2 pair: {outcome:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_missing_cache_exits_one() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-approved-missing-cache";
        write_chain1_fixture(&items_dir, track_id);

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_approved_via_driver(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir,
            })
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 1, "expected blocked outcome: {outcome:?}");
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|stderr| stderr.contains("has no Pass cache entry")),
            "expected missing-cache message: {outcome:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_duplicate_non_pass_cache_exits_one() {
        use domain::tddd::semantic_verify::{EvidenceCitation, SemanticVerdict};

        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-approved-duplicate-fail-cache";
        write_chain1_fixture(&items_dir, track_id);
        write_cache_for_first_chain1_pair(
            &items_dir,
            track_id,
            vec![
                SemanticVerdict::Pass {
                    citation: EvidenceCitation::try_new("guarded path decision".to_owned())
                        .unwrap(),
                },
                SemanticVerdict::Fail {
                    reason: "duplicate non-pass verdict must block approval".to_owned(),
                },
            ],
        );

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_approved_via_driver(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir,
            })
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 1, "expected blocked outcome: {outcome:?}");
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|stderr| stderr.contains("has non-Pass cache entry")),
            "expected non-Pass cache message: {outcome:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_corrupt_cache_returns_error() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-approved-corrupt-cache";
        write_chain1_fixture(&items_dir, track_id);
        std::fs::write(
            items_dir.join(track_id).join("spec-adr-verify-cache.json"),
            "{not valid json",
        )
        .unwrap();

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_approved_via_driver(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir,
            })
        })
        .unwrap();
        let err = outcome.stderr.as_deref().unwrap_or_default();

        assert_eq!(outcome.exit_code, 1, "cache corruption must fail, got: {outcome:?}");
        assert!(
            err.contains("verify-cache"),
            "cache corruption must be surfaced as an infrastructure error, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_wrong_branch_returns_error() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-approved-branch-guard";
        write_chain1_fixture(&items_dir, track_id);

        let outcome = with_fake_git_branch(&project_root, "not-the-track", || {
            check_approved_via_driver(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir,
            })
        })
        .unwrap();
        let err = outcome.stderr.as_deref().unwrap_or_default();

        assert_eq!(outcome.exit_code, 1, "wrong branch must fail, got: {outcome:?}");
        assert!(err.contains("track is not active"), "expected active-track error, got: {err}");
    }

    // ── ref_verify_run: fail-closed artifact-state cases ────────────────────

    /// Catalogue present + spec.json absent is a SoT Chain ordering violation
    /// (IN-06 / AC-09): the scope resolver must fail closed and the error must
    /// surface through the public `ref_verify_run` API.
    #[cfg(unix)]
    #[test]
    fn test_ref_verify_run_catalogue_without_spec_fails_closed() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-catalogue-without-spec";

        // TDDD layer with its catalogue present, but no spec.json.
        std::fs::write(
            project_root.join("architecture-rules.json"),
            r#"{
  "layers": [
    {
      "crate": "domain",
      "tddd": { "enabled": true, "catalogue_file": "domain-types.json" }
    }
  ]
}"#,
        )
        .unwrap();
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("domain-types.json"),
            r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {}
}"#,
        )
        .unwrap();

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            run_via_driver(RefVerifyRunInput { track_id: track_id.to_owned(), items_dir })
        })
        .unwrap();
        let msg = outcome.stderr.as_deref().unwrap_or_default();
        assert_eq!(outcome.exit_code, 1, "catalogue-without-spec must fail, got: {outcome:?}");
        assert!(
            msg.contains("scope resolution failed"),
            "catalogue-without-spec must fail closed in scope resolution, got: {msg}"
        );
    }

    // ── ref_verify_run ───────────────────────────────────────────────────────

    /// Phase 0 end-to-end (AC-01 / AC-02): no spec.json and no catalogue exist.
    /// The run derives zero pairs for both chains and exits 0 without invoking
    /// any verifier agent — this is the state the commit gate hits right after
    /// `/track:init`.
    ///
    /// A fake `claude` binary is placed on `PATH` so that if zero-pair detection
    /// regresses and the test accidentally reaches the verifier, the failure is
    /// deterministic rather than dependent on a host-installed binary.
    #[cfg(unix)]
    #[test]
    fn test_ref_verify_run_phase0_no_artifacts_exits_zero() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-phase0";
        write_architecture_rules_no_tddd(&project_root);
        // ref_verify_run loads agent-profiles.json unconditionally even though
        // a zero-pair run never invokes a verifier agent.
        write_ref_verifier_profiles(&project_root);
        let fake_claude_dir = write_fake_claude_into_path_dir(&project_root);
        std::fs::create_dir_all(items_dir.join(track_id)).unwrap();

        let outcome =
            with_fake_track_branch_and_path(&project_root, track_id, &fake_claude_dir, || {
                run_via_driver(RefVerifyRunInput { track_id: track_id.to_owned(), items_dir })
                    .unwrap()
            });

        assert_eq!(outcome.exit_code, 0, "Phase 0 run must exit zero: {outcome:?}");
        assert!(
            outcome.stdout.as_deref().is_some_and(|s| s.contains("passed")),
            "success message must contain 'passed': {outcome:?}"
        );
    }

    /// Phase 0 check-approved (AC-02): with zero production pairs the gate
    /// passes without any verify-cache artifact.
    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_phase0_no_artifacts_exits_zero() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-check-approved-phase0";
        write_architecture_rules_no_tddd(&project_root);
        std::fs::create_dir_all(items_dir.join(track_id)).unwrap();

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_approved_via_driver(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir,
            })
            .unwrap()
        });

        assert_eq!(outcome.exit_code, 0, "Phase 0 check-approved must pass: {outcome:?}");
        assert!(
            outcome
                .stdout
                .as_deref()
                .is_some_and(|s| s.contains("No production reference pairs found")),
            "success message must identify the zero-pairs path: {outcome:?}"
        );
    }

    /// End-to-end test for an All-scope `ref_verify_run` with both chains
    /// present.
    ///
    /// Both spec.json and the TDDD catalogue exist, so the existence-based
    /// resolver derives `RefVerifyScope::All`. Uses `write_chain1_fixture` +
    /// `add_chain2_tddd_layer_to_fixture` to create real Chain-1 (spec→ADR)
    /// and Chain-2 (catalogue→spec) pairs so that the test discriminates `All`
    /// from a single-chain pair set.
    ///
    /// Scope discrimination: after a successful run the test asserts that both
    /// per-chain verify-cache files were written; a missing file would mean one
    /// chain's pairs were never loaded.
    #[cfg(unix)]
    #[test]
    fn test_ref_verify_run_all_scope_with_real_pair_exits_zero() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-standalone-all-real";
        write_chain1_fixture(&items_dir, track_id);
        // Add a TDDD layer so the All-scope run has a real Chain-2 pair to verify.
        add_chain2_tddd_layer_to_fixture(&items_dir, track_id);

        write_ref_verifier_profiles(&project_root);
        let fake_claude_dir = write_fake_claude_into_path_dir(&project_root);

        let outcome =
            with_fake_track_branch_and_path(&project_root, track_id, &fake_claude_dir, || {
                run_via_driver(RefVerifyRunInput {
                    track_id: track_id.to_owned(),
                    items_dir: items_dir.clone(),
                })
                .unwrap()
            });

        assert_eq!(
            outcome.exit_code, 0,
            "All-scope run with real pair and fake claude must exit zero: {outcome:?}"
        );
        assert!(
            outcome.stdout.as_deref().is_some_and(|s| s.contains("passed")),
            "success message must contain 'passed': {outcome:?}"
        );

        // Scope discrimination: the Chain-1 cache file is written only when the
        // All-scope path ran `enumerate_chain1_pairs`.  If the existence-based
        // resolution wrongly derived a Chain2-only pair set, the Chain-1 ADR
        // pairs would never be loaded and this file would not be written.
        let chain1_cache = items_dir.join(track_id).join("spec-adr-verify-cache.json");
        assert!(
            chain1_cache.exists(),
            "Chain-1 cache file must exist after the All-scope run — \
             absent file means the run skipped Chain-1 pairs: {chain1_cache:?}"
        );

        // Scope discrimination: the Chain-2 cache file is written only when the
        // All-scope path ran `enumerate_chain2_all_layers`.
        let chain2_cache =
            items_dir.join(track_id).join("test_domain-catalogue-spec-verify-cache.json");
        assert!(
            chain2_cache.exists(),
            "Chain-2 cache file must exist after the All-scope run — \
             absent file means the run skipped Chain-2 pairs: {chain2_cache:?}"
        );
    }

    #[test]
    fn test_ref_verify_run_invalid_track_id_returns_error() {
        let outcome = run_via_driver(RefVerifyRunInput {
            track_id: "../outside".to_owned(),
            items_dir: repo_root_for_tests().join("track").join("items"),
        })
        .unwrap();
        let msg = outcome.stderr.as_deref().unwrap_or_default();
        assert_eq!(outcome.exit_code, 1, "invalid track id must fail, got: {outcome:?}");
        assert!(
            msg.contains("invalid --track-id") || msg.contains("invalid track"),
            "invalid track id must be rejected, got: {msg}"
        );
    }

    #[test]
    fn test_ref_verify_run_outside_repo_items_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = run_via_driver(RefVerifyRunInput {
            track_id: "my-track".to_owned(),
            items_dir: dir.path().to_path_buf(),
        })
        .unwrap();
        let msg = outcome.stderr.as_deref().unwrap_or_default();
        assert_eq!(outcome.exit_code, 1, "outside items_dir must fail, got: {outcome:?}");
        assert!(
            msg.contains("items_dir") || msg.contains("project root"),
            "items_dir outside repo must be rejected, got: {msg}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_run_chain1_success_exits_zero() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-run-success";
        write_chain1_fixture(&items_dir, track_id);

        let outcome = run_ref_verify_with_fake_claude(&project_root, track_id, items_dir);

        assert_eq!(outcome.exit_code, 0, "expected successful run outcome: {outcome:?}");
        assert!(
            project_root
                .join("track")
                .join("items")
                .join(track_id)
                .join("spec-adr-verify-cache.json")
                .exists()
        );
    }

    /// Creates a spec.json where GO-01 references TWO identical ADR files.
    ///
    /// Because both ADR files have identical content, the pair source produces two
    /// Chain-1 pairs that share the same `(claim_hash, evidence_hash)` but have
    /// different `evidence_origin` (different ADR file paths).  This fixture is the
    /// minimal setup for testing origin-discriminating cache lookups at the
    /// composition boundary.
    fn write_chain1_fixture_two_identical_adrs(items_dir: &Path, track_id: &str) {
        let project_root = project_root_from_items_dir(items_dir);
        let track_items_dir = items_dir.join(track_id);
        let adr_dir = project_root.join("knowledge").join("adr");
        std::fs::create_dir_all(&track_items_dir).unwrap();
        std::fs::create_dir_all(&adr_dir).unwrap();
        write_architecture_rules_no_tddd(project_root);

        // Identical content in both files → same git-blob hash → same evidence_hash.
        let adr_content = "---\nadr_id: alpha\ndecisions:\n  - id: D1\n    \
                           status: proposed\n    candidate_selection: \"choose the guarded path\"\n\
                           ---\n# ADR\n\n### D1: Guarded path decision\n\
                           The guarded path must stay inside the trusted repository root.\n";
        std::fs::write(adr_dir.join("adr-alpha.md"), adr_content).unwrap();
        std::fs::write(adr_dir.join("adr-beta.md"), adr_content).unwrap();

        std::fs::write(
            track_items_dir.join("spec.json"),
            serde_json::json!({
                "schema_version": 2,
                "version": "0.1",
                "title": "Test",
                "goal": [{
                    "id": "GO-01",
                    "text": "The guarded path must stay inside the trusted repository root.",
                    "adr_refs": [
                        { "file": "knowledge/adr/adr-alpha.md", "anchor": "D1" },
                        { "file": "knowledge/adr/adr-beta.md", "anchor": "D1" }
                    ]
                }],
                "scope": { "in_scope": [], "out_of_scope": [] },
                "constraints": [],
                "acceptance_criteria": []
            })
            .to_string(),
        )
        .unwrap();
    }

    /// Verifies that `ref_verify_check_approved` uses the four-field cache key
    /// `(claim_hash, evidence_hash, claim_origin, evidence_origin)` and does NOT
    /// approve a production pair solely because another pair with the same content
    /// hashes already has a Pass cache entry.
    ///
    /// Setup: one spec goal (GO-01) references two ADR files (adr-alpha.md and
    /// adr-beta.md) that have identical content.  Because content is identical,
    /// both Chain-1 pairs share the same `(claim_hash, evidence_hash)`.  They
    /// differ only in `evidence_origin` (different file paths).
    ///
    /// A Pass cache entry is written for pair P (adr-alpha.md origin only).
    /// `ref_verify_check_approved` must report pair Q (adr-beta.md) as missing a
    /// Pass cache entry, because the four-field key for Q does not match the cached
    /// entry for P even though the hashes are equal.
    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_distinguishes_pass_by_origin() {
        use domain::tddd::semantic_verify::{
            EvidenceCitation, SemanticVerdict, SemanticVerifyEntry,
        };
        use infrastructure::ref_verify::{RefVerifyCacheAdapter, RefVerifyPairSourceAdapter};
        use usecase::ref_verify::{
            RefVerifyCachePort as _, RefVerifyCacheScope, RefVerifyPairSourcePort as _,
        };

        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-check-approved-origin-distinguish";
        write_chain1_fixture_two_identical_adrs(&items_dir, track_id);

        // Load the two production pairs; they share (claim_hash, evidence_hash)
        // but differ in evidence_origin (adr-alpha.md vs adr-beta.md).
        let cmd = ref_verify_chain1_cmd(track_id).unwrap();
        let pair_source = RefVerifyPairSourceAdapter::new(project_root.clone());
        let all_pairs =
            pair_source.load_pairs(&cmd, &usecase::ref_verify::RefVerifyConfig::default()).unwrap();
        let mut production_pairs: Vec<_> = all_pairs.into_iter().filter(|p| !p.known_bad).collect();
        assert_eq!(production_pairs.len(), 2, "fixture must produce exactly two production pairs");

        // Sort by evidence_origin debug string for deterministic ordering (alpha < beta).
        production_pairs.sort_by_key(|p| format!("{:?}", p.evidence_origin));
        let pair_alpha = &production_pairs[0]; // adr-alpha.md
        let pair_beta = &production_pairs[1]; // adr-beta.md

        // Both pairs must share the same content hashes (same spec element, identical ADR files).
        assert_eq!(
            pair_alpha.claim_hash, pair_beta.claim_hash,
            "fixture invariant: both pairs must share claim_hash"
        );
        assert_eq!(
            pair_alpha.evidence_hash, pair_beta.evidence_hash,
            "fixture invariant: both pairs must share evidence_hash (identical ADR content)"
        );

        // Write a Pass cache entry for pair P (adr-alpha.md origin) only.
        let pass_entry = SemanticVerifyEntry::new(
            pair_alpha.claim_hash.clone(),
            pair_alpha.evidence_hash.clone(),
            SemanticVerdict::Pass {
                citation: EvidenceCitation::try_new("guarded path".to_owned()).unwrap(),
            },
            pair_alpha.claim_origin.clone(),
            pair_alpha.evidence_origin.clone(),
        );
        RefVerifyCacheAdapter::new(project_root.clone())
            .save_entries(&cmd, &RefVerifyCacheScope::SpecAdr, vec![pass_entry])
            .unwrap();

        // check_approved must detect that pair Q (adr-beta.md) is not covered.
        let outcome = with_fake_track_branch(&project_root, track_id, || {
            check_approved_via_driver(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir: items_dir.clone(),
            })
        })
        .unwrap();

        assert_eq!(
            outcome.exit_code, 1,
            "pair Q (adr-beta.md) shares hashes with pair P but has a different evidence_origin — \
             must not be approved by P's cache entry: {outcome:?}"
        );
        assert!(
            outcome.stderr.as_deref().is_some_and(|s| s.contains("no Pass cache entry")),
            "expected 'no Pass cache entry' for origin-mismatched pair Q: {outcome:?}"
        );
        // Exactly one pair is missing (pair Q); pair P is covered.
        assert!(
            outcome.stderr.as_deref().is_some_and(|s| s.contains("1 pair(s)")),
            "expected exactly 1 missing pair (pair Q only — pair P has a matching Pass entry): {outcome:?}"
        );
    }

    /// Integration test for `ref_verify_results` with no cache (AC-01 / AC-06 / CN-03).
    ///
    /// Chain-1 fixture present, no verify-cache written. All pairs are pending.
    /// `ref_verify_results` must exit 0 (CN-02) and include a `Summary:` line
    /// with 0 pass and 0 fail (all pending).
    #[cfg(unix)]
    #[test]
    fn test_ref_verify_results_no_cache_returns_all_pending() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-results-no-cache";
        write_chain1_fixture(&items_dir, track_id);

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            results_via_driver(DriverResultsInput {
                track_id: track_id.to_owned(),
                items_dir: items_dir.clone(),
                chain: RefVerifyChainSelect::All,
                layer: "all".to_owned(),
                verdict: RefVerifyVerdictSelect::FailPending,
            })
            .unwrap()
        });

        assert_eq!(outcome.exit_code, 0, "ref_verify_results must always exit 0: {outcome:?}");
        let stdout = outcome.stdout.as_deref().unwrap_or("");
        assert!(stdout.contains("Summary:"), "stdout must contain 'Summary:' line: {stdout:?}");
        // With no cache all pairs are pending — pass and fail counts must both be 0.
        assert!(
            stdout.contains("0 pass"),
            "stdout must contain '0 pass' when no cache: {stdout:?}"
        );
        assert!(
            stdout.contains("0 fail"),
            "stdout must contain '0 fail' when no cache: {stdout:?}"
        );
    }
}
