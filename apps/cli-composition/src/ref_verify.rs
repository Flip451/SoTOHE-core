use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{CliApp, CommandOutcome};

#[derive(Debug, Clone)]
pub struct RefVerifyRunInput {
    pub track_id: String,
    pub items_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RefVerifyCheckApprovedInput {
    pub track_id: String,
    pub items_dir: PathBuf,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RefVerifyConfigDto {
    known_bad_injection_rate_percent: Option<u8>,
    known_bad_detection_threshold_percent: Option<u8>,
    max_parallelism: Option<usize>,
}

const REF_VERIFY_CONFIG_PATH: &str = ".harness/config/ref-verify.json";

struct RefVerifyCommandContext {
    canonical_root: PathBuf,
    track_id: domain::TrackId,
}

fn resolve_ref_verify_context(
    items_dir: &Path,
    track_id: &str,
) -> Result<RefVerifyCommandContext, String> {
    let project_root = super::track::resolve_project_root(items_dir)
        .map_err(|e| format!("cannot resolve project root: {e}"))?;
    let canonical_root = project_root
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize project root: {e}"))?;
    super::track::validate_track_id_str(track_id)
        .map_err(|e| format!("invalid --track-id: {e}"))?;
    let track_id = domain::TrackId::try_new(track_id.to_owned())
        .map_err(|e| format!("invalid track ID: {e}"))?;
    Ok(RefVerifyCommandContext { canonical_root, track_id })
}

fn load_ref_verify_config(
    project_root: &std::path::Path,
) -> Result<usecase::ref_verify::RefVerifyConfig, String> {
    let config_path = project_root.join(REF_VERIFY_CONFIG_PATH);
    if !config_path
        .try_exists()
        .map_err(|e| format!("cannot inspect ref-verify config path: {e}"))?
    {
        return Ok(usecase::ref_verify::RefVerifyConfig::default());
    }

    let text = std::fs::read_to_string(&config_path).map_err(|e| {
        format!("cannot read ref-verify config at '{}': {e}", config_path.display())
    })?;

    let dto: RefVerifyConfigDto = serde_json::from_str(&text)
        .map_err(|e| format!("invalid ref-verify config at '{}': {e}", config_path.display()))?;
    let defaults = usecase::ref_verify::RefVerifyConfig::default();
    let injection = dto
        .known_bad_injection_rate_percent
        .unwrap_or_else(|| defaults.known_bad_injection_rate_percent.as_u8());
    let threshold = dto
        .known_bad_detection_threshold_percent
        .unwrap_or_else(|| defaults.known_bad_detection_threshold_percent.as_u8());
    let parallelism = dto.max_parallelism.unwrap_or_else(|| defaults.max_parallelism.as_usize());

    usecase::ref_verify::RefVerifyConfig::try_new(injection, threshold, parallelism)
        .map_err(|e| format!("ref-verify config validation failed: {e}"))
}

fn current_git_branch(project_root: &Path) -> Result<String, String> {
    use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
    SystemGitRepo::discover_from(project_root)
        .map_err(|e| format!("cannot discover git repo: {e}"))?
        .current_branch()
        .map_err(|e| format!("cannot read current branch: {e}"))?
        .ok_or_else(|| "cannot read current branch: HEAD is detached".to_owned())
}

impl CliApp {
    pub fn ref_verify_run(&self, input: RefVerifyRunInput) -> Result<CommandOutcome, String> {
        use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles};
        use infrastructure::ref_verify::{
            AgentRefVerifierAdapter, RefVerifyCacheAdapter, RefVerifyPairSourceAdapter,
            RefVerifyScopeResolver, make_ref_verifier_process_runner,
        };
        use usecase::ref_verify::{RefVerifyApplicationService as _, VerifySemanticRefsInteractor};

        let RefVerifyCommandContext { canonical_root, track_id } =
            resolve_ref_verify_context(&input.items_dir, &input.track_id)?;

        let current_branch = current_git_branch(&canonical_root)?;

        // Existence-based scope resolution (IN-01): the Chain1 / Chain2 / All
        // pair-set derivation follows from which track artifacts exist on
        // disk; cli-composition performs no firing-surface translation.
        let resolver = RefVerifyScopeResolver::new(canonical_root.clone());
        let scope = resolver
            .resolve(track_id.as_ref())
            .map_err(|e| format!("ref-verify scope resolution failed: {e}"))?;

        let config = load_ref_verify_config(&canonical_root)?;

        let pair_source =
            Arc::new(RefVerifyPairSourceAdapter::new(canonical_root.clone())) as Arc<_>;
        let cache = Arc::new(RefVerifyCacheAdapter::new(canonical_root.clone())) as Arc<_>;

        let profiles_path = canonical_root.join(AGENT_PROFILES_PATH);
        let profiles = AgentProfiles::load(&profiles_path)
            .map_err(|e| format!("cannot load agent-profiles.json: {e}"))?;
        let profiles = Arc::new(profiles);

        let runner = make_ref_verifier_process_runner(canonical_root.clone());

        let verifier =
            Arc::new(AgentRefVerifierAdapter::new(profiles, runner, canonical_root.clone()))
                as Arc<_>;

        let interactor = VerifySemanticRefsInteractor::new(pair_source, cache, verifier, config);

        let cmd = usecase::ref_verify::RefVerifyCommand {
            track_id: track_id.clone(),
            scope,
            current_branch: current_branch.clone(),
        };
        match interactor.execute(&cmd) {
            Ok(()) => Ok(CommandOutcome::success(Some(
                "[OK] Semantic reference verification passed — all pairs verified.".to_owned(),
            ))),
            Err(usecase::ref_verify::RefVerifyError::SemanticFailuresConfirmed { pair_count }) => {
                Ok(CommandOutcome {
                    stdout: None,
                    stderr: Some(format!(
                        "[BLOCKED] Semantic review confirmed {pair_count} production failure(s). \
                         Resolve the failures before committing."
                    )),
                    exit_code: 1,
                })
            }
            Err(usecase::ref_verify::RefVerifyError::HumanEscalationRequired { pair_count }) => {
                Ok(CommandOutcome {
                    stdout: None,
                    stderr: Some(format!(
                        "[ESCALATE] Human review required for {pair_count} unresolved pair(s) \
                         or known-bad detection failure."
                    )),
                    exit_code: 1,
                })
            }
            Err(e) => Err(format!("ref-verify run failed: {e}")),
        }
    }

    pub fn ref_verify_check_approved(
        &self,
        input: RefVerifyCheckApprovedInput,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::ref_verify::{
            RefVerifyCacheAdapter, RefVerifyPairSourceAdapter, RefVerifyScopeResolver,
        };
        use usecase::ref_verify::{
            RefVerifyCachePort as _, RefVerifyCacheScope, RefVerifyPairSourcePort as _,
        };

        let RefVerifyCommandContext { canonical_root, track_id } =
            resolve_ref_verify_context(&input.items_dir, &input.track_id)?;

        // check-approved is the commit gate's read-only verification surface;
        // the scope derives from artifact existence like every other run.
        let resolver = RefVerifyScopeResolver::new(canonical_root.clone());
        let scope = resolver
            .resolve(track_id.as_ref())
            .map_err(|e| format!("ref-verify scope resolution failed: {e}"))?;

        let current_branch = current_git_branch(&canonical_root)?;
        let expected_branch = format!("track/{}", track_id.as_ref());
        if current_branch != expected_branch {
            return Err(format!(
                "ref-verify check-approved failed: track is not active: current branch '{current_branch}', expected '{expected_branch}'"
            ));
        }

        let cmd = usecase::ref_verify::RefVerifyCommand {
            track_id: track_id.clone(),
            scope,
            current_branch: current_branch.clone(),
        };
        let config = usecase::ref_verify::RefVerifyConfig::default();

        let pair_source = RefVerifyPairSourceAdapter::new(canonical_root.clone());
        let pairs = pair_source
            .load_pairs(&cmd, &config)
            .map_err(|e| format!("ref-verify check-approved: failed to load pairs: {e}"))?;
        let production_pairs: Vec<_> = pairs.into_iter().filter(|p| !p.known_bad).collect();

        if production_pairs.is_empty() {
            return Ok(CommandOutcome::success(Some(
                "[OK] No production reference pairs found — check-approved gate passes.".to_owned(),
            )));
        }

        let cache_adapter = RefVerifyCacheAdapter::new(canonical_root.clone());

        let mut missing_or_non_pass: Vec<String> = Vec::new();

        let mut scope_keys: std::collections::HashMap<
            RefVerifyCacheScope,
            Vec<(domain::ContentHash, domain::ContentHash)>,
        > = std::collections::HashMap::new();
        for pair in &production_pairs {
            scope_keys
                .entry(pair.cache_scope.clone())
                .or_default()
                .push((pair.claim_hash.clone(), pair.evidence_hash.clone()));
        }

        for (cache_scope, pair_keys) in &scope_keys {
            let entries = cache_adapter.load_entries(&cmd, cache_scope).map_err(|e| {
                format!(
                    "ref-verify check-approved: failed to read verify-cache for {cache_scope:?}: {e}"
                )
            })?;

            use domain::tddd::semantic_verify::SemanticVerdict;
            for (claim_hash, evidence_hash) in pair_keys {
                let matching_entries = entries
                    .iter()
                    .filter(|entry| {
                        entry.claim_hash == *claim_hash && entry.evidence_hash == *evidence_hash
                    })
                    .collect::<Vec<_>>();
                if matching_entries.is_empty() {
                    missing_or_non_pass.push(format!(
                        "pair ({}, {}) has no Pass cache entry",
                        claim_hash.to_hex(),
                        evidence_hash.to_hex()
                    ));
                } else if matching_entries
                    .iter()
                    .any(|entry| !matches!(entry.verdict, SemanticVerdict::Pass { .. }))
                {
                    missing_or_non_pass.push(format!(
                        "pair ({}, {}) has non-Pass cache entry",
                        claim_hash.to_hex(),
                        evidence_hash.to_hex()
                    ));
                }
            }
        }

        if missing_or_non_pass.is_empty() {
            Ok(CommandOutcome::success(Some(
                "[OK] All production reference pairs have verified Pass cache entries.".to_owned(),
            )))
        } else {
            Ok(CommandOutcome {
                stdout: None,
                stderr: Some(format!(
                    "[BLOCKED] ref-verify check-approved failed: {} pair(s) without Pass cache:\n{}",
                    missing_or_non_pass.len(),
                    missing_or_non_pass.join("\n")
                )),
                exit_code: 1,
            })
        }
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

    use crate::{
        CliApp, CommandOutcome, RefVerifyCheckApprovedInput, RefVerifyRunInput,
        test_support::repo_root_for_tests,
    };

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
        // The `hash` field is required by the DTO codec; the exact value is irrelevant for
        // semantic verification (the pair_source re-computes the evidence hash from spec.json).
        let spec_ref_path = format!("track/items/{track_id}/spec.json");
        let catalogue = serde_json::json!({
            "schema_version": 3,
            "crate_name": "test_domain",
            "layer": "test_domain",
            "types": {
                "TestGuardedPath": {
                    "action": "add",
                    "role": "ValueObject",
                    "kind": { "kind": "struct", "shape": { "kind": "unit" } },
                    "methods": [],
                    "module_path": "test_domain",
                    "spec_refs": [{
                        "file": spec_ref_path,
                        "anchor": "GO-01",
                        "hash": "0000000000000000000000000000000000000000000000000000000000000000"
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

    fn ref_verify_chain1_cmd(
        track_id: &str,
    ) -> Result<usecase::ref_verify::RefVerifyCommand, String> {
        Ok(usecase::ref_verify::RefVerifyCommand {
            track_id: domain::TrackId::try_new(track_id.to_owned())
                .map_err(|e| format!("invalid track ID: {e}"))?,
            scope: usecase::ref_verify::RefVerifyScope::Chain1,
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
        let entries = verdicts
            .into_iter()
            .map(|verdict| {
                SemanticVerifyEntry::new(
                    pair.claim_hash.clone(),
                    pair.evidence_hash.clone(),
                    verdict,
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

    #[cfg(unix)]
    fn run_ref_verify_with_fake_claude(
        project_root: &Path,
        track_id: &str,
        items_dir: PathBuf,
    ) -> CommandOutcome {
        write_ref_verifier_profiles(project_root);
        let fake_claude_dir = write_fake_claude_into_path_dir(project_root);

        with_fake_track_branch_and_path(project_root, track_id, &fake_claude_dir, || {
            CliApp::new()
                .ref_verify_run(RefVerifyRunInput { track_id: track_id.to_owned(), items_dir })
                .unwrap()
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
      "prompt_template_path": ".harness/prompts/ref-verifier-chain1.md"
    },
    "ref-verifier-chain2": {
      "provider": "claude",
      "model": "claude-test",
      "prompt_template_path": ".harness/prompts/ref-verifier-chain2.md"
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

    // ── load_ref_verify_config ──────────────────────────────────────────────

    #[test]
    fn test_ref_verify_config_absent_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = super::load_ref_verify_config(dir.path()).unwrap();
        let defaults = usecase::ref_verify::RefVerifyConfig::default();
        assert_eq!(cfg, defaults);
    }

    #[test]
    fn test_ref_verify_config_explicit_values_reflected() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_dir = dir.path().join(".harness").join("config");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        std::fs::write(
            cfg_dir.join("ref-verify.json"),
            r#"{"known_bad_injection_rate_percent": 5, "known_bad_detection_threshold_percent": 80, "max_parallelism": 2}"#,
        )
        .unwrap();
        let cfg = super::load_ref_verify_config(dir.path()).unwrap();
        assert_eq!(cfg.known_bad_injection_rate_percent.as_u8(), 5);
        assert_eq!(cfg.known_bad_detection_threshold_percent.as_u8(), 80);
        assert_eq!(cfg.max_parallelism.as_usize(), 2);
    }

    #[test]
    fn test_ref_verify_config_unknown_field_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_dir = dir.path().join(".harness").join("config");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        std::fs::write(cfg_dir.join("ref-verify.json"), r#"{"unknown_field": 1}"#).unwrap();
        let err = super::load_ref_verify_config(dir.path()).unwrap_err();
        assert!(err.contains("invalid ref-verify config"));
    }

    #[test]
    fn test_ref_verify_config_zero_percent_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_dir = dir.path().join(".harness").join("config");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        std::fs::write(
            cfg_dir.join("ref-verify.json"),
            r#"{"known_bad_injection_rate_percent": 0}"#,
        )
        .unwrap();
        let err = super::load_ref_verify_config(dir.path()).unwrap_err();
        assert!(err.contains("ref-verify config validation failed"));
    }

    #[test]
    fn test_ref_verify_config_zero_max_parallelism_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_dir = dir.path().join(".harness").join("config");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        std::fs::write(cfg_dir.join("ref-verify.json"), r#"{"max_parallelism": 0}"#).unwrap();
        let err = super::load_ref_verify_config(dir.path()).unwrap_err();
        assert!(err.contains("ref-verify config validation failed"));
    }

    // ── ref_verify_check_approved ────────────────────────────────────────────

    #[test]
    fn test_ref_verify_check_approved_invalid_track_id_returns_error() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let result = CliApp::new().ref_verify_check_approved(RefVerifyCheckApprovedInput {
            track_id: "../outside".to_owned(),
            items_dir,
        });
        let msg = result.unwrap_err();
        assert!(
            msg.contains("invalid --track-id") || msg.contains("invalid track"),
            "invalid track id must be rejected, got: {msg}"
        );
    }

    #[test]
    fn test_ref_verify_check_approved_outside_repo_items_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = CliApp::new().ref_verify_check_approved(RefVerifyCheckApprovedInput {
            track_id: "my-track".to_owned(),
            items_dir: dir.path().to_path_buf(),
        });
        let msg = result.unwrap_err();
        assert!(
            msg.contains("items_dir") || msg.contains("project root"),
            "items_dir outside repo must be rejected, got: {msg}"
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
            CliApp::new().ref_verify_check_approved(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir,
            })
        })
        .unwrap();

        assert_eq!(outcome.exit_code, 0, "expected approved outcome: {outcome:?}");
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
            CliApp::new().ref_verify_check_approved(RefVerifyCheckApprovedInput {
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
            CliApp::new().ref_verify_check_approved(RefVerifyCheckApprovedInput {
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
            CliApp::new().ref_verify_check_approved(RefVerifyCheckApprovedInput {
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

        let err = with_fake_track_branch(&project_root, track_id, || {
            CliApp::new().ref_verify_check_approved(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir,
            })
        })
        .unwrap_err();

        assert!(
            err.contains("failed to read verify-cache"),
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

        let err = with_fake_git_branch(&project_root, "not-the-track", || {
            CliApp::new().ref_verify_check_approved(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir,
            })
        })
        .unwrap_err();

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
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {}
}"#,
        )
        .unwrap();

        let result = with_fake_track_branch(&project_root, track_id, || {
            CliApp::new()
                .ref_verify_run(RefVerifyRunInput { track_id: track_id.to_owned(), items_dir })
        });
        let msg = result.unwrap_err();
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
                CliApp::new()
                    .ref_verify_run(RefVerifyRunInput { track_id: track_id.to_owned(), items_dir })
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
            CliApp::new()
                .ref_verify_check_approved(RefVerifyCheckApprovedInput {
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
                CliApp::new()
                    .ref_verify_run(RefVerifyRunInput {
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
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let result = CliApp::new()
            .ref_verify_run(RefVerifyRunInput { track_id: "../outside".to_owned(), items_dir });
        let msg = result.unwrap_err();
        assert!(
            msg.contains("invalid --track-id") || msg.contains("invalid track"),
            "invalid track id must be rejected, got: {msg}"
        );
    }

    #[test]
    fn test_ref_verify_run_outside_repo_items_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = CliApp::new().ref_verify_run(RefVerifyRunInput {
            track_id: "my-track".to_owned(),
            items_dir: dir.path().to_path_buf(),
        });
        let msg = result.unwrap_err();
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
}
