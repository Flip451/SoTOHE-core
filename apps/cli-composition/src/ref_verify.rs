use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{CliApp, CommandOutcome};

mod runner;

use runner::run_ref_verifier_agent;
#[cfg(test)]
use runner::{
    build_claude_ref_verifier_args, collect_pipe, join_collector, ref_verify_runtime_path,
    run_test_ref_verifier_process,
};

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
const REF_VERIFY_RUN_SCOPE_ENV: &str = "SOTP_REF_VERIFY_RUN_SCOPE";

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

fn ref_verify_run_approval_surface() -> Result<bool, String> {
    match std::env::var(REF_VERIFY_RUN_SCOPE_ENV) {
        Ok(value) => match value.trim() {
            "" | "phase" => Ok(false),
            "approval" | "all" | "final" | "standalone" => Ok(true),
            other => Err(format!(
                "invalid {REF_VERIFY_RUN_SCOPE_ENV}: expected 'phase' or one of \
                 'approval', 'all', 'final', 'standalone'; got '{other}'"
            )),
        },
        Err(std::env::VarError::NotPresent) => Ok(false),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(format!("{REF_VERIFY_RUN_SCOPE_ENV} must be valid UTF-8"))
        }
    }
}

fn current_git_branch(project_root: &Path) -> Result<String, String> {
    use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
    SystemGitRepo::discover_from(project_root)
        .map_err(|e| format!("cannot discover git repo: {e}"))?
        .current_branch()
        .map_err(|e| format!("cannot read current branch: {e}"))?
        .ok_or_else(|| "cannot read current branch: HEAD is detached".to_owned())
}

#[derive(Debug, PartialEq, Eq)]
enum RefVerifyScopePlan {
    Chain1,
    Chain2All(Vec<domain::tddd::LayerId>),
    All,
}

fn resolve_scope(
    track_id: &str,
    project_root: &Path,
    approval_gate: bool,
) -> Result<RefVerifyScopePlan, String> {
    let track_dir = project_root.join("track").join("items").join(track_id);
    let has_chain1 = path_exists(&track_dir.join("spec.json"))?;
    let chain2_layers = existing_chain2_layers(project_root, &track_dir)?;
    if !chain2_layers.is_empty() {
        return if has_chain1 && should_run_all_chains(track_id, project_root, approval_gate)? {
            Ok(RefVerifyScopePlan::All)
        } else {
            Ok(RefVerifyScopePlan::Chain2All(chain2_layers))
        };
    }
    if has_chain1 { Ok(RefVerifyScopePlan::Chain1) } else { Ok(RefVerifyScopePlan::All) }
}

fn should_run_all_chains(
    track_id: &str,
    project_root: &Path,
    approval_gate: bool,
) -> Result<bool, String> {
    if approval_gate {
        return Ok(true);
    }

    let track_id = domain::TrackId::try_new(track_id.to_owned())
        .map_err(|e| format!("invalid track ID: {e}"))?;
    let status = resolve_track_status(project_root, &track_id)?;
    Ok(!matches!(status, domain::TrackStatus::Planned | domain::TrackStatus::InProgress))
}

fn resolve_track_status(
    project_root: &Path,
    track_id: &domain::TrackId,
) -> Result<domain::TrackStatus, String> {
    use domain::{ImplPlanReader as _, TrackReader as _};

    let store = infrastructure::track::fs_store::FsTrackStore::new(
        project_root.join("track").join("items"),
    );
    let track = store
        .find(track_id)
        .map_err(|e| format!("cannot load metadata for ref-verify scope resolution: {e}"))?;
    let impl_plan = store
        .load_impl_plan(track_id)
        .map_err(|e| format!("cannot load impl-plan for ref-verify scope resolution: {e}"))?;
    Ok(domain::derive_track_status(
        impl_plan.as_ref(),
        track.as_ref().and_then(|track| track.status_override()),
    ))
}

fn path_exists(path: &Path) -> Result<bool, String> {
    path.try_exists().map_err(|e| format!("cannot inspect '{}': {e}", path.display()))
}

fn existing_chain2_layers(
    project_root: &Path,
    track_dir: &Path,
) -> Result<Vec<domain::tddd::LayerId>, String> {
    let rules_path = project_root.join("architecture-rules.json");
    if !path_exists(&rules_path)? {
        return Ok(Vec::new());
    }

    let bindings = infrastructure::verify::tddd_layers::load_tddd_layers(&rules_path, project_root)
        .map_err(|e| format!("cannot load TDDD layer bindings: {e}"))?;
    let mut layers = Vec::new();
    let mut missing = Vec::new();
    for binding in bindings {
        let catalogue_path = track_dir.join(binding.catalogue_file());
        if path_exists(&catalogue_path)? {
            let layer = domain::tddd::LayerId::try_new(binding.layer_id().to_owned())
                .map_err(|e| format!("invalid TDDD layer id '{}': {e}", binding.layer_id()))?;
            layers.push(layer);
        } else {
            missing.push(binding.catalogue_file().to_owned());
        }
    }
    if !layers.is_empty() && !missing.is_empty() {
        return Err(format!("missing TDDD catalogue file(s): {}", missing.join(", ")));
    }
    Ok(layers)
}

fn execute_ref_verify_plan(
    interactor: &usecase::ref_verify::VerifySemanticRefsInteractor,
    track_id: &domain::TrackId,
    current_branch: &str,
    scope_plan: RefVerifyScopePlan,
) -> Result<(), usecase::ref_verify::RefVerifyError> {
    use usecase::ref_verify::RefVerifyApplicationService as _;

    let mut first_blocking_error = None;
    let mut semantic_failure_count = 0;
    let mut human_escalation_count = 0;
    for cmd in ref_verify_commands(track_id, current_branch, scope_plan) {
        match interactor.execute(&cmd) {
            Ok(()) => {}
            Err(usecase::ref_verify::RefVerifyError::SemanticFailuresConfirmed { pair_count }) => {
                semantic_failure_count += pair_count;
            }
            Err(usecase::ref_verify::RefVerifyError::HumanEscalationRequired { pair_count }) => {
                human_escalation_count += pair_count;
            }
            Err(err) => {
                if first_blocking_error.is_none() {
                    first_blocking_error = Some(err);
                }
            }
        }
    }

    if let Some(err) = first_blocking_error {
        return Err(err);
    }
    // HumanEscalationRequired can represent verifier degradation; do not let a
    // separate actionable semantic failure mask a fail-closed escalation.
    if human_escalation_count > 0 {
        return Err(usecase::ref_verify::RefVerifyError::HumanEscalationRequired {
            pair_count: human_escalation_count,
        });
    }
    if semantic_failure_count > 0 {
        return Err(usecase::ref_verify::RefVerifyError::SemanticFailuresConfirmed {
            pair_count: semantic_failure_count,
        });
    }
    Ok(())
}

fn ref_verify_commands(
    track_id: &domain::TrackId,
    current_branch: &str,
    scope_plan: RefVerifyScopePlan,
) -> Vec<usecase::ref_verify::RefVerifyCommand> {
    use usecase::ref_verify::{RefVerifyCommand, RefVerifyScope};

    match scope_plan {
        RefVerifyScopePlan::Chain1 => {
            vec![RefVerifyCommand {
                track_id: track_id.clone(),
                scope: RefVerifyScope::Chain1,
                current_branch: current_branch.to_owned(),
            }]
        }
        RefVerifyScopePlan::Chain2All(layers) => layers
            .into_iter()
            .map(|layer| RefVerifyCommand {
                track_id: track_id.clone(),
                scope: RefVerifyScope::Chain2 { layer },
                current_branch: current_branch.to_owned(),
            })
            .collect(),
        RefVerifyScopePlan::All => {
            vec![RefVerifyCommand {
                track_id: track_id.clone(),
                scope: RefVerifyScope::All,
                current_branch: current_branch.to_owned(),
            }]
        }
    }
}

impl CliApp {
    pub fn ref_verify_run(&self, input: RefVerifyRunInput) -> Result<CommandOutcome, String> {
        use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles};
        use infrastructure::ref_verify::{
            AgentRefVerifierAdapter, RefVerifyCacheAdapter, RefVerifyPairSourceAdapter,
        };
        use usecase::ref_verify::VerifySemanticRefsInteractor;

        let RefVerifyCommandContext { canonical_root, track_id } =
            resolve_ref_verify_context(&input.items_dir, &input.track_id)?;

        let current_branch = current_git_branch(&canonical_root)?;

        // Default `run` follows the phase context. Commit/final/standalone
        // callers can select the approval surface via SOTP_REF_VERIFY_RUN_SCOPE.
        let scope_plan =
            resolve_scope(track_id.as_ref(), &canonical_root, ref_verify_run_approval_surface()?)?;

        let config = load_ref_verify_config(&canonical_root)?;

        let pair_source =
            Arc::new(RefVerifyPairSourceAdapter::new(canonical_root.clone())) as Arc<_>;
        let cache = Arc::new(RefVerifyCacheAdapter::new(canonical_root.clone())) as Arc<_>;

        let profiles_path = canonical_root.join(AGENT_PROFILES_PATH);
        let profiles = AgentProfiles::load(&profiles_path)
            .map_err(|e| format!("cannot load agent-profiles.json: {e}"))?;
        let profiles = Arc::new(profiles);

        let runner_root = canonical_root.clone();
        let runner: Arc<infrastructure::ref_verify::AgentExecutionRunner> =
            Arc::new(move |resolved, prompt, secs| {
                run_ref_verifier_agent(&runner_root, resolved, prompt, secs)
            });

        let verifier =
            Arc::new(AgentRefVerifierAdapter::new(profiles, runner, canonical_root.clone()))
                as Arc<_>;

        let interactor = VerifySemanticRefsInteractor::new(pair_source, cache, verifier, config);

        match execute_ref_verify_plan(&interactor, &track_id, &current_branch, scope_plan) {
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
        use infrastructure::ref_verify::{RefVerifyCacheAdapter, RefVerifyPairSourceAdapter};
        use usecase::ref_verify::{
            RefVerifyCachePort as _, RefVerifyCacheScope, RefVerifyPairSourcePort as _,
        };

        let RefVerifyCommandContext { canonical_root, track_id } =
            resolve_ref_verify_context(&input.items_dir, &input.track_id)?;

        let scope_plan = resolve_scope(&input.track_id, &canonical_root, true)?;
        let current_branch = current_git_branch(&canonical_root)?;
        let expected_branch = format!("track/{}", track_id.as_ref());
        if current_branch != expected_branch {
            return Err(format!(
                "ref-verify check-approved failed: track is not active: current branch '{current_branch}', expected '{expected_branch}'"
            ));
        }
        let commands = ref_verify_commands(&track_id, &current_branch, scope_plan);
        let config = usecase::ref_verify::RefVerifyConfig::default();

        let pair_source = RefVerifyPairSourceAdapter::new(canonical_root.clone());
        let mut production_pairs = Vec::new();
        for cmd in &commands {
            let pairs = pair_source
                .load_pairs(cmd, &config)
                .map_err(|e| format!("ref-verify check-approved: failed to load pairs: {e}"))?;
            production_pairs.extend(pairs.into_iter().filter(|p| !p.known_bad));
        }

        if production_pairs.is_empty() {
            return Ok(CommandOutcome::success(Some(
                "[OK] No production reference pairs found — check-approved gate passes.".to_owned(),
            )));
        }
        let cache_cmd = commands
            .first()
            .ok_or_else(|| "ref-verify check-approved: no scope commands generated".to_owned())?;

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

        for (scope, pair_keys) in &scope_keys {
            let entries = cache_adapter.load_entries(cache_cmd, scope).map_err(|e| {
                format!("ref-verify check-approved: failed to read verify-cache for {scope:?}: {e}")
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use crate::{CliApp, CommandOutcome, RefVerifyCheckApprovedInput, RefVerifyRunInput};

    const ADR_WITH_D1: &str = r#"---
adr_id: test-adr
decisions:
  - id: D1
    status: proposed
    candidate_selection: "choose the guarded path"
---
# ADR

### D1: Guarded path decision
The guarded path must stay inside the trusted repository root.
"#;

    fn with_env_var<T>(key: &'static str, value: OsString, run: impl FnOnce() -> T) -> T {
        let previous = std::env::var_os(key);
        // Safety: callers hold process_env_lock for this helper's full execution,
        // so process-wide environment mutation is serialized in this test crate.
        unsafe {
            std::env::set_var(key, value);
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));

        // Safety: see the set_var block above; restoration happens before the
        // caller releases process_env_lock.
        unsafe {
            if let Some(value) = previous {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }

        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn without_env_var<T>(key: &'static str, run: impl FnOnce() -> T) -> T {
        let previous = std::env::var_os(key);
        // Safety: callers hold process_env_lock for this helper's full execution,
        // so process-wide environment mutation is serialized in this test crate.
        unsafe {
            std::env::remove_var(key);
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));

        // Safety: see the remove_var block above; restoration happens before the
        // caller releases process_env_lock.
        unsafe {
            if let Some(value) = previous {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }

        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn repo_root_for_tests() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("cli-composition manifest must be under apps/")
            .to_path_buf()
    }

    /// Create a tempdir rooted under `target/ref-verify-cli-tests/` that has the
    /// `track/items` sub-structure required by `resolve_project_root`.
    ///
    /// Returns `(TempDir, items_dir_path)` where `items_dir_path` is
    /// `<tempdir>/track/items`.  Keep the `TempDir` alive for the test duration.
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
        std::fs::write(adr_dir.join("decision.md"), ADR_WITH_D1).unwrap();
    }

    fn write_tddd_architecture_rules(project_root: &Path) {
        std::fs::write(
            project_root.join("architecture-rules.json"),
            r#"{"layers":[
{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}},
{"crate":"usecase","tddd":{"enabled":true,"catalogue_file":"usecase-types.json"}},
{"crate":"infrastructure","tddd":{"enabled":true,"catalogue_file":"infrastructure-types.json"}}
]}"#,
        )
        .unwrap();
    }

    fn write_impl_plan_statuses(track_dir: &Path, statuses: &[&str]) {
        let tasks: Vec<_> = statuses
            .iter()
            .enumerate()
            .map(|(index, status)| {
                serde_json::json!({
                    "id": format!("T{:03}", index + 1),
                    "description": "task",
                    "status": status,
                })
            })
            .collect();
        let task_ids: Vec<_> = tasks
            .iter()
            .map(|task| task.get("id").cloned().unwrap_or(serde_json::Value::Null))
            .collect();
        std::fs::write(
            track_dir.join("impl-plan.json"),
            serde_json::json!({
                "schema_version": 1,
                "tasks": tasks,
                "plan": {
                    "summary": [],
                    "sections": [{
                        "id": "S1",
                        "title": "All",
                        "description": [],
                        "task_ids": task_ids
                    }]
                }
            })
            .to_string(),
        )
        .unwrap();
    }

    fn write_status_override_metadata(track_dir: &Path, track_id: &str, status: &str) {
        std::fs::write(
            track_dir.join("metadata.json"),
            serde_json::json!({
                "schema_version": 5,
                "id": track_id,
                "title": "Test",
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z",
                "status_override": {
                    "status": status,
                    "reason": "waiting on dependency"
                }
            })
            .to_string(),
        )
        .unwrap();
    }

    fn write_catalogue_with_goal_ref(track_dir: &Path, layer: &str) {
        std::fs::write(
            track_dir.join(format!("{layer}-types.json")),
            serde_json::json!({
                "schema_version": 3,
                "crate_name": layer,
                "layer": layer,
                "types": {
                    format!("{layer}Type"): {
                        "action": "add",
                        "role": "ValueObject",
                        "kind": { "kind": "struct", "shape": { "kind": "unit" } },
                        "spec_refs": [
                            {
                                "file": format!(
                                    "track/items/{}/spec.json",
                                    track_dir.file_name().and_then(|name| name.to_str()).unwrap()
                                ),
                                "anchor": "GO-01",
                                "hash": "0000000000000000000000000000000000000000000000000000000000000000"
                            }
                        ],
                        "informal_grounds": []
                    }
                },
                "traits": {},
                "functions": {}
            })
            .to_string(),
        )
        .unwrap();
    }

    fn ref_verify_chain1_cmd(
        track_id: &str,
    ) -> Result<usecase::ref_verify::RefVerifyCommand, String> {
        let track_id = domain::TrackId::try_new(track_id.to_owned()).map_err(|e| e.to_string())?;
        Ok(usecase::ref_verify::RefVerifyCommand {
            current_branch: format!("track/{}", track_id.as_ref()),
            track_id,
            scope: usecase::ref_verify::RefVerifyScope::Chain1,
        })
    }

    struct LayerPairSource;

    impl usecase::ref_verify::RefVerifyPairSourcePort for LayerPairSource {
        fn load_pairs(
            &self,
            cmd: &usecase::ref_verify::RefVerifyCommand,
            _config: &usecase::ref_verify::RefVerifyConfig,
        ) -> Result<Vec<usecase::ref_verify::RefVerifyPair>, usecase::ref_verify::RefVerifyError>
        {
            let usecase::ref_verify::RefVerifyScope::Chain2 { layer } = &cmd.scope else {
                return Ok(Vec::new());
            };
            let byte = match layer.as_ref() {
                "domain" => 1,
                "usecase" => 2,
                "infrastructure" => 3,
                _ => 4,
            };
            Ok(vec![usecase::ref_verify::RefVerifyPair {
                claim: layer.as_ref().to_owned(),
                evidence: "layer evidence".to_owned(),
                claim_hash: domain::ContentHash::from_bytes([byte; 32]),
                evidence_hash: domain::ContentHash::from_bytes([byte + 10; 32]),
                cache_scope: usecase::ref_verify::RefVerifyCacheScope::CatalogueSpec {
                    layer: layer.clone(),
                },
                known_bad: false,
            }])
        }
    }

    struct FailingDomainVerifier;

    impl usecase::ref_verify::RefVerifierPort for FailingDomainVerifier {
        fn verify_pair(
            &self,
            claim: String,
            _evidence: String,
            _tier: domain::tddd::semantic_verify::ModelTier,
        ) -> Result<
            domain::tddd::semantic_verify::SemanticVerdict,
            usecase::ref_verify::RefVerifyError,
        > {
            if claim == "domain" {
                return Ok(domain::tddd::semantic_verify::SemanticVerdict::Fail {
                    reason: "domain layer failed".to_owned(),
                });
            }
            Ok(domain::tddd::semantic_verify::SemanticVerdict::Pass {
                citation: domain::tddd::semantic_verify::EvidenceCitation::try_new(
                    "layer evidence".to_owned(),
                )
                .unwrap(),
            })
        }
    }

    struct MixedLayerOutcomeVerifier;

    impl usecase::ref_verify::RefVerifierPort for MixedLayerOutcomeVerifier {
        fn verify_pair(
            &self,
            claim: String,
            _evidence: String,
            _tier: domain::tddd::semantic_verify::ModelTier,
        ) -> Result<
            domain::tddd::semantic_verify::SemanticVerdict,
            usecase::ref_verify::RefVerifyError,
        > {
            match claim.as_str() {
                "domain" => Ok(domain::tddd::semantic_verify::SemanticVerdict::Fail {
                    reason: "domain layer failed".to_owned(),
                }),
                "usecase" => Ok(domain::tddd::semantic_verify::SemanticVerdict::Pending),
                _ => Ok(domain::tddd::semantic_verify::SemanticVerdict::Pass {
                    citation: domain::tddd::semantic_verify::EvidenceCitation::try_new(
                        "layer evidence".to_owned(),
                    )
                    .unwrap(),
                }),
            }
        }
    }

    #[derive(Default)]
    struct RecordingCache {
        saved_scopes: Mutex<Vec<String>>,
    }

    impl usecase::ref_verify::RefVerifyCachePort for RecordingCache {
        fn load_entries(
            &self,
            _cmd: &usecase::ref_verify::RefVerifyCommand,
            _cache_scope: &usecase::ref_verify::RefVerifyCacheScope,
        ) -> Result<
            Vec<domain::tddd::semantic_verify::SemanticVerifyEntry>,
            usecase::ref_verify::RefVerifyError,
        > {
            Ok(Vec::new())
        }

        fn save_entries(
            &self,
            _cmd: &usecase::ref_verify::RefVerifyCommand,
            cache_scope: &usecase::ref_verify::RefVerifyCacheScope,
            _entries: Vec<domain::tddd::semantic_verify::SemanticVerifyEntry>,
        ) -> Result<(), usecase::ref_verify::RefVerifyError> {
            let usecase::ref_verify::RefVerifyCacheScope::CatalogueSpec { layer } = cache_scope
            else {
                return Ok(());
            };
            self.saved_scopes.lock().unwrap().push(layer.as_ref().to_owned());
            Ok(())
        }
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
        run_ref_verify_with_fake_claude_scope(project_root, track_id, items_dir, None)
    }

    #[cfg(unix)]
    fn run_ref_verify_with_fake_claude_scope(
        project_root: &Path,
        track_id: &str,
        items_dir: PathBuf,
        run_scope: Option<&str>,
    ) -> CommandOutcome {
        let fake_claude = write_ref_verifier_profiles_and_fake_claude(project_root);

        with_fake_track_branch(project_root, track_id, || {
            with_env_var("CLAUDE_BIN", fake_claude.into_os_string(), || {
                let run = || {
                    CliApp::new()
                        .ref_verify_run(RefVerifyRunInput {
                            track_id: track_id.to_owned(),
                            items_dir,
                        })
                        .unwrap()
                };
                if let Some(run_scope) = run_scope {
                    with_env_var(super::REF_VERIFY_RUN_SCOPE_ENV, OsString::from(run_scope), run)
                } else {
                    without_env_var(super::REF_VERIFY_RUN_SCOPE_ENV, run)
                }
            })
        })
    }

    #[cfg(unix)]
    fn write_ref_verifier_profiles_and_fake_claude(project_root: &Path) -> PathBuf {
        let config_dir = project_root.join(".harness").join("config");
        let prompt_dir = project_root.join(".harness").join("prompts");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&prompt_dir).unwrap();
        std::fs::write(prompt_dir.join("ref-verifier.md"), "{{claim}}\n{{evidence}}\n{{tier}}")
            .unwrap();
        std::fs::write(
            config_dir.join("agent-profiles.json"),
            r#"{
  "schema_version": 1,
  "providers": { "claude": { "label": "Claude Code" } },
  "capabilities": {
    "ref-verifier": {
      "provider": "claude",
      "model": "claude-test",
      "timeout_seconds": 30,
      "prompt_template_path": ".harness/prompts/ref-verifier.md"
    }
  }
}"#,
        )
        .unwrap();
        write_fake_claude(project_root)
    }

    #[cfg(unix)]
    fn write_fake_verifier_script(project_root: &Path, filename: &str, contents: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;

        let script = project_root.join(filename);
        std::fs::write(&script, contents).unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        script
    }

    #[cfg(unix)]
    fn write_fake_pwd_verifier(project_root: &Path) -> PathBuf {
        write_fake_verifier_script(project_root, "fake-pwd-verifier.sh", "#!/bin/sh\npwd\n")
    }

    #[cfg(unix)]
    fn write_fake_hanging_verifier(project_root: &Path) -> PathBuf {
        write_fake_verifier_script(
            project_root,
            "fake-hanging-verifier.sh",
            "#!/bin/sh\nsleep 10 &\nsleep 10\n",
        )
    }

    #[cfg(unix)]
    fn write_fake_exiting_with_inherited_pipe_verifier(project_root: &Path) -> PathBuf {
        write_fake_verifier_script(
            project_root,
            "fake-exiting-inherited-pipe-verifier.sh",
            "#!/bin/sh\nsleep 10 &\nprintf 'ok\\n'\n",
        )
    }

    #[cfg(unix)]
    fn write_fake_claude(project_root: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;

        let script = project_root.join("fake-claude.sh");
        std::fs::write(
	            &script,
	            r#"#!/bin/sh
	orig_args=" $* "
	saw_permission=0
	saw_tools_empty=0
	saw_allowed_empty=0
	while [ "$#" -gt 0 ]; do
	  case "$1" in
	    --permission-mode)
	      shift
	      [ "$#" -gt 0 ] || exit 20
	      [ "$1" = "dontAsk" ] || exit 20
	      saw_permission=1
	      ;;
	    --tools)
	      shift
	      [ "$#" -gt 0 ] || exit 21
	      [ "$1" = "" ] || exit 21
	      saw_tools_empty=1
	      ;;
	    --allowedTools|--allowed-tools)
	      shift
	      [ "$#" -gt 0 ] || exit 19
	      [ "$1" = "" ] || exit 19
	      saw_allowed_empty=1
	      ;;
	    --safe-mode)
	      exit 22
	      ;;
	  esac
	  shift
	done
	[ "$saw_permission" = 1 ] || exit 20
	[ "$saw_tools_empty" = 1 ] || exit 21
	[ "$saw_allowed_empty" = 1 ] || exit 19
	case "$orig_args" in *" --disallowedTools "*) ;; *) exit 22 ;; esac
	for tool in Read Grep Glob Bash Edit Write; do
	  case "$orig_args" in *" $tool "*) ;; *) exit 22 ;; esac
	done
	case "$orig_args" in *" --output-format json "*) ;; *) exit 23 ;; esac
	case "$orig_args" in *" --model claude-test "*) ;; *) exit 24 ;; esac
	case "$orig_args" in
	  *known-bad-probe*) printf '{"type":"result","structured_output":{"kind":"fail","reason":"known bad probe"}}\n'; exit 0 ;;
	esac
printf '{"type":"result","structured_output":{"kind":"pass","citation":"claude ok"}}\n'
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        script
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

    // ── load_ref_verify_config ────────────────────────────────────────────────

    #[test]
    fn test_ref_verify_config_absent_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = super::load_ref_verify_config(dir.path()).unwrap();
        let defaults = usecase::ref_verify::RefVerifyConfig::default();
        assert_eq!(
            config.known_bad_injection_rate_percent.as_u8(),
            defaults.known_bad_injection_rate_percent.as_u8()
        );
        assert_eq!(
            config.known_bad_detection_threshold_percent.as_u8(),
            defaults.known_bad_detection_threshold_percent.as_u8()
        );
        assert_eq!(config.max_parallelism.as_usize(), defaults.max_parallelism.as_usize());
    }

    #[test]
    fn test_ref_verify_config_explicit_values_reflected() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("ref-verify.json"),
            r#"{"known_bad_injection_rate_percent": 15, "known_bad_detection_threshold_percent": 85, "max_parallelism": 8}"#,
        )
        .unwrap();
        let config = super::load_ref_verify_config(dir.path()).unwrap();
        assert_eq!(config.known_bad_injection_rate_percent.as_u8(), 15);
        assert_eq!(config.known_bad_detection_threshold_percent.as_u8(), 85);
        assert_eq!(config.max_parallelism.as_usize(), 8);
    }

    #[test]
    fn test_ref_verify_config_unknown_field_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("ref-verify.json"),
            r#"{"known_bad_injection_rate_percent": 10, "unknown_field": "value"}"#,
        )
        .unwrap();
        let err = super::load_ref_verify_config(dir.path()).unwrap_err();
        assert!(err.contains("invalid ref-verify config"), "expected config error, got: {err}");
    }

    #[test]
    fn test_ref_verify_config_zero_percent_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("ref-verify.json"),
            r#"{"known_bad_injection_rate_percent": 0}"#,
        )
        .unwrap();
        let err = super::load_ref_verify_config(dir.path()).unwrap_err();
        assert!(
            err.contains("config validation failed"),
            "zero percent must be rejected, got: {err}"
        );
    }

    #[test]
    fn test_ref_verify_config_zero_max_parallelism_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("ref-verify.json"), r#"{"max_parallelism": 0}"#).unwrap();
        let err = super::load_ref_verify_config(dir.path()).unwrap_err();
        assert!(
            err.contains("config validation failed"),
            "zero max_parallelism must be rejected, got: {err}"
        );
    }

    #[test]
    fn test_join_collector_read_error_returns_runner_error() {
        struct FailingReader;

        impl std::io::Read for FailingReader {
            fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("pipe read failed"))
            }
        }

        let handle = super::collect_pipe(FailingReader);
        let err = super::join_collector(handle, "fake-verifier", "stdout").unwrap_err();

        match err {
            usecase::ref_verify::RefVerifyError::VerifierPort { message } => assert!(
                message.contains("failed to read fake-verifier stdout"),
                "pipe read errors must be propagated, got: {message}"
            ),
            other => panic!("expected VerifierPort, got {other:?}"),
        }
    }

    #[test]
    fn test_build_claude_ref_verifier_args_denies_local_tools() {
        let args = super::build_claude_ref_verifier_args("claude-test", "prompt");
        let strs: Vec<_> = args.iter().filter_map(|arg| arg.to_str()).collect();

        let tools_idx = strs.iter().position(|arg| *arg == "--tools").expect("tools flag");
        assert_eq!(
            strs.get(tools_idx + 1).copied(),
            Some(""),
            "ref-verifier must disable all built-in Claude tools"
        );
        let allowed_idx =
            strs.iter().position(|arg| *arg == "--allowedTools").expect("allowed tools flag");
        assert_eq!(
            strs.get(allowed_idx + 1).copied(),
            Some(""),
            "ref-verifier must keep the Claude permission allowlist empty"
        );
        assert!(!strs.contains(&"--safe-mode"), "ref-verifier must use supported Claude flags");

        let denied_idx =
            strs.iter().position(|arg| *arg == "--disallowedTools").expect("disallowed tools flag");
        assert!(allowed_idx < denied_idx, "allowlist must be closed before denylist defense");
        for tool in ["Read", "Grep", "Glob", "Bash", "Edit", "Write"] {
            let tool_idx = strs.iter().position(|arg| *arg == tool).expect("disallowed tool");
            assert!(tool_idx > denied_idx, "disallowed tool {tool} must follow --disallowedTools");
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_run_ref_verifier_agent_claude_parses_structured_output() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().canonicalize().unwrap();
        let fake_claude = write_fake_claude(&project_root);
        let resolved = infrastructure::agent_profiles::ResolvedExecution {
            provider: "claude".to_owned(),
            model: Some("claude-test".to_owned()),
        };

        let output = with_env_var("CLAUDE_BIN", fake_claude.into_os_string(), || {
            super::run_ref_verifier_agent(&project_root, resolved, "prompt".to_owned(), 5).unwrap()
        });
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value.get("kind").and_then(|v| v.as_str()), Some("pass"));
        assert_eq!(value.get("citation").and_then(|v| v.as_str()), Some("claude ok"));
    }

    #[cfg(unix)]
    #[test]
    fn test_run_ref_verifier_agent_codex_rejected_without_no_tools_boundary() {
        assert_ref_verifier_agent_rejects_no_tools_boundary("codex", Some("codex-test"));
    }

    #[test]
    fn test_run_ref_verifier_agent_gemini_rejected_without_no_tools_boundary() {
        assert_ref_verifier_agent_rejects_no_tools_boundary("gemini", None);
    }

    fn assert_ref_verifier_agent_rejects_no_tools_boundary(provider: &str, model: Option<&str>) {
        let dir = tempfile::tempdir().unwrap();
        let resolved = infrastructure::agent_profiles::ResolvedExecution {
            provider: provider.to_owned(),
            model: model.map(str::to_owned),
        };

        let err = super::run_ref_verifier_agent(dir.path(), resolved, "prompt".to_owned(), 5)
            .unwrap_err();

        assert!(
            matches!(
                err,
                usecase::ref_verify::RefVerifyError::VerifierPort { ref message }
                    if message.contains("cannot enforce the required no-tools boundary")
            ),
            "expected no-tools boundary rejection, got: {err:?}"
        );
    }

    #[test]
    fn test_run_ref_verifier_agent_rejects_zero_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let resolved = infrastructure::agent_profiles::ResolvedExecution {
            provider: "gemini".to_owned(),
            model: None,
        };

        let err = super::run_ref_verifier_agent(dir.path(), resolved, "prompt".to_owned(), 0)
            .unwrap_err();

        assert!(
            matches!(
                err,
                usecase::ref_verify::RefVerifyError::VerifierPort { ref message }
                    if message.contains("timeout_seconds must be nonzero")
            ),
            "expected zero-timeout verifier error, got: {err:?}"
        );
    }

    #[test]
    fn test_ref_verify_runtime_path_is_project_root_anchored() {
        let dir = tempfile::tempdir().unwrap();

        let path = super::ref_verify_runtime_path(dir.path(), "probe", "txt").unwrap();

        assert!(
            path.starts_with(dir.path().join("tmp").join("reviewer-runtime")),
            "runtime path must stay under project root: {}",
            path.display()
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_run_ref_verifier_process_uses_project_root_as_child_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().canonicalize().unwrap();
        let fake_verifier = write_fake_pwd_verifier(&project_root);

        let output = super::run_test_ref_verifier_process(
            fake_verifier.as_os_str(),
            &project_root,
            std::time::Duration::from_secs(5),
        )
        .unwrap();

        assert_eq!(output, project_root.display().to_string());
    }

    #[cfg(unix)]
    #[test]
    fn test_run_ref_verifier_process_rejects_overflowing_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().canonicalize().unwrap();
        let fake_verifier = write_fake_pwd_verifier(&project_root);

        let err = super::run_test_ref_verifier_process(
            fake_verifier.as_os_str(),
            &project_root,
            std::time::Duration::MAX,
        )
        .unwrap_err();

        assert!(
            matches!(
                err,
                usecase::ref_verify::RefVerifyError::VerifierPort { ref message }
                    if message.contains("timeout is too large")
            ),
            "expected oversized-timeout verifier error, got: {err:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_run_ref_verifier_process_timeout_kills_inherited_pipe_descendants() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().canonicalize().unwrap();
        let fake_verifier = write_fake_hanging_verifier(&project_root);

        let started = std::time::Instant::now();
        let err = super::run_test_ref_verifier_process(
            fake_verifier.as_os_str(),
            &project_root,
            std::time::Duration::from_millis(200),
        )
        .unwrap_err();

        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "timeout must not wait for descendant sleep processes to close inherited pipes"
        );
        assert!(
            matches!(
                err,
                usecase::ref_verify::RefVerifyError::VerifierPort { ref message }
                    if message.contains("timed out")
            ),
            "expected timeout verifier error, got: {err:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_run_ref_verifier_process_normal_exit_kills_inherited_pipe_descendants() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path().canonicalize().unwrap();
        let fake_verifier = write_fake_exiting_with_inherited_pipe_verifier(&project_root);

        let started = std::time::Instant::now();
        let output = super::run_test_ref_verifier_process(
            fake_verifier.as_os_str(),
            &project_root,
            std::time::Duration::from_secs(1),
        )
        .unwrap();

        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "collector joins must not wait for descendant sleep processes to close inherited pipes"
        );
        assert_eq!(output, "ok");
    }

    // ── resolve_scope ─────────────────────────────────────────────

    #[test]
    fn test_resolve_scope_spec_only_returns_chain1() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir);
        let track_id = "test-ref-verify-scope-chain1";
        write_chain1_fixture(&items_dir, track_id);

        let scope = super::resolve_scope(track_id, project_root, false).unwrap();

        assert_eq!(scope, super::RefVerifyScopePlan::Chain1);
    }

    #[test]
    fn test_resolve_scope_complete_type_design_returns_chain2() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir);
        let track_id = "test-ref-verify-scope-chain2";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        write_tddd_architecture_rules(project_root);
        for file in ["domain-types.json", "usecase-types.json", "infrastructure-types.json"] {
            std::fs::write(track_dir.join(file), "{}").unwrap();
        }

        let scope = super::resolve_scope(track_id, project_root, false).unwrap();

        let super::RefVerifyScopePlan::Chain2All(layers) = scope else {
            panic!("expected Chain2 scope");
        };
        let names: Vec<_> = layers.iter().map(|layer| layer.as_ref()).collect();
        assert_eq!(names, vec!["domain", "usecase", "infrastructure"]);
    }

    #[test]
    fn test_resolve_scope_phase_run_spec_and_catalogues_returns_chain2() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir);
        let track_id = "test-ref-verify-scope-phase-chain2";
        let track_dir = items_dir.join(track_id);
        write_chain1_fixture(&items_dir, track_id);
        write_tddd_architecture_rules(project_root);
        for file in ["domain-types.json", "usecase-types.json", "infrastructure-types.json"] {
            std::fs::write(track_dir.join(file), "{}").unwrap();
        }

        let scope = super::resolve_scope(track_id, project_root, false).unwrap();

        let super::RefVerifyScopePlan::Chain2All(layers) = scope else {
            panic!("expected Chain2 scope");
        };
        let names: Vec<_> = layers.iter().map(|layer| layer.as_ref()).collect();
        assert_eq!(names, vec!["domain", "usecase", "infrastructure"]);
    }

    #[test]
    fn test_resolve_scope_approval_gate_spec_and_catalogues_returns_all() {
        assert_resolve_scope_with_impl_plan_and_catalogues_returns_all(
            "test-ref-verify-scope-gate-all",
            true,
            &["done", "in_progress", "todo"],
        );
    }

    #[test]
    fn test_resolve_scope_partial_type_design_fails_closed() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir);
        let track_id = "test-ref-verify-scope-partial-chain2";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        write_tddd_architecture_rules(project_root);
        std::fs::write(track_dir.join("domain-types.json"), "{}").unwrap();

        let err = super::resolve_scope(track_id, project_root, false).unwrap_err();

        assert!(
            err.contains("missing TDDD catalogue file"),
            "partial type-design must fail closed, got: {err}"
        );
    }

    #[test]
    fn test_resolve_scope_run_with_in_progress_impl_plan_and_catalogues_returns_chain2() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir);
        let track_id = "test-ref-verify-scope-in-progress-chain2";
        let track_dir = items_dir.join(track_id);
        write_chain1_fixture(&items_dir, track_id);
        write_tddd_architecture_rules(project_root);
        write_impl_plan_statuses(&track_dir, &["done", "in_progress", "todo"]);
        for file in ["domain-types.json", "usecase-types.json", "infrastructure-types.json"] {
            std::fs::write(track_dir.join(file), "{}").unwrap();
        }

        let scope = super::resolve_scope(track_id, project_root, false).unwrap();

        let super::RefVerifyScopePlan::Chain2All(layers) = scope else {
            panic!("expected Chain2 scope");
        };
        let names: Vec<_> = layers.iter().map(|layer| layer.as_ref()).collect();
        assert_eq!(names, vec!["domain", "usecase", "infrastructure"]);
    }

    #[test]
    fn test_resolve_scope_run_ready_to_ship_with_catalogues_returns_all() {
        assert_resolve_scope_with_impl_plan_and_catalogues_returns_all(
            "test-ref-verify-scope-ready-all",
            false,
            &["done", "skipped"],
        );
    }

    #[test]
    fn test_resolve_scope_blocked_override_with_catalogues_returns_all() {
        assert_resolve_scope_with_status_override_and_catalogues_returns_all(
            "test-ref-verify-scope-blocked-all",
            "blocked",
        );
    }

    #[test]
    fn test_resolve_scope_cancelled_override_with_catalogues_returns_all() {
        assert_resolve_scope_with_status_override_and_catalogues_returns_all(
            "test-ref-verify-scope-cancelled-all",
            "cancelled",
        );
    }

    fn assert_resolve_scope_with_impl_plan_and_catalogues_returns_all(
        track_id: &str,
        approval_gate: bool,
        statuses: &[&str],
    ) {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir);
        let track_dir = items_dir.join(track_id);
        write_chain1_fixture(&items_dir, track_id);
        write_tddd_architecture_rules(project_root);
        write_impl_plan_statuses(&track_dir, statuses);
        for file in ["domain-types.json", "usecase-types.json", "infrastructure-types.json"] {
            std::fs::write(track_dir.join(file), "{}").unwrap();
        }

        let scope = super::resolve_scope(track_id, project_root, approval_gate).unwrap();

        assert_eq!(scope, super::RefVerifyScopePlan::All);
    }

    fn assert_resolve_scope_with_status_override_and_catalogues_returns_all(
        track_id: &str,
        status_override: &str,
    ) {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir);
        let track_dir = items_dir.join(track_id);
        write_chain1_fixture(&items_dir, track_id);
        write_tddd_architecture_rules(project_root);
        write_impl_plan_statuses(&track_dir, &["done", "in_progress", "todo"]);
        write_status_override_metadata(&track_dir, track_id, status_override);
        for file in ["domain-types.json", "usecase-types.json", "infrastructure-types.json"] {
            std::fs::write(track_dir.join(file), "{}").unwrap();
        }

        let scope = super::resolve_scope(track_id, project_root, false).unwrap();

        assert_eq!(scope, super::RefVerifyScopePlan::All);
    }

    #[test]
    fn test_execute_ref_verify_plan_chain2_all_failure_runs_remaining_layers() {
        let track_id = domain::TrackId::try_new("test-ref-verify-layer-loop").unwrap();
        let cache = Arc::new(RecordingCache::default());
        let cache_port: Arc<dyn usecase::ref_verify::RefVerifyCachePort> = cache.clone();
        let interactor = usecase::ref_verify::VerifySemanticRefsInteractor::new(
            Arc::new(LayerPairSource),
            cache_port,
            Arc::new(FailingDomainVerifier),
            usecase::ref_verify::RefVerifyConfig::default(),
        );
        let layers = ["domain", "usecase", "infrastructure"]
            .into_iter()
            .map(|layer| domain::tddd::LayerId::try_new(layer.to_owned()).unwrap())
            .collect();

        let result = super::execute_ref_verify_plan(
            &interactor,
            &track_id,
            "track/test-ref-verify-layer-loop",
            super::RefVerifyScopePlan::Chain2All(layers),
        );

        assert!(
            matches!(
                result,
                Err(usecase::ref_verify::RefVerifyError::SemanticFailuresConfirmed {
                    pair_count: 1
                })
            ),
            "expected one aggregated semantic failure, got: {result:?}"
        );
        let saved_scopes = cache.saved_scopes.lock().unwrap().clone();
        assert_eq!(saved_scopes, vec!["domain", "usecase", "infrastructure"]);
    }

    #[test]
    fn test_execute_ref_verify_plan_human_escalation_takes_precedence_across_layers() {
        let track_id = domain::TrackId::try_new("test-ref-verify-escalation-precedence").unwrap();
        let interactor = usecase::ref_verify::VerifySemanticRefsInteractor::new(
            Arc::new(LayerPairSource),
            Arc::new(RecordingCache::default()),
            Arc::new(MixedLayerOutcomeVerifier),
            usecase::ref_verify::RefVerifyConfig::default(),
        );
        let layers = ["domain", "usecase", "infrastructure"]
            .into_iter()
            .map(|layer| domain::tddd::LayerId::try_new(layer.to_owned()).unwrap())
            .collect();

        let result = super::execute_ref_verify_plan(
            &interactor,
            &track_id,
            "track/test-ref-verify-escalation-precedence",
            super::RefVerifyScopePlan::Chain2All(layers),
        );

        assert!(
            matches!(
                result,
                Err(usecase::ref_verify::RefVerifyError::HumanEscalationRequired { pair_count: 1 })
            ),
            "human escalation must not be masked by a separate semantic failure: {result:?}"
        );
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
    fn test_ref_verify_check_approved_empty_spec_exits_zero() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-approved-empty-spec";
        let track_items_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_items_dir).unwrap();

        std::fs::write(
            track_items_dir.join("spec.json"),
            r#"{"schema_version":2,"version":"0.1","title":"Test","goal":[],"scope":{"in_scope":[],"out_of_scope":[]},"constraints":[],"acceptance_criteria":[]}"#,
        )
        .unwrap();
        std::fs::write(
            project_root.join("architecture-rules.json"),
            r#"{"layers":[{"crate":"placeholder-no-tddd"}]}"#,
        )
        .unwrap();
        let result = with_fake_track_branch(&project_root, track_id, || {
            CliApp::new().ref_verify_check_approved(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir,
            })
        });

        let outcome =
            result.expect("check-approved must return Ok(CommandOutcome) for empty-pair case");
        assert_eq!(
            outcome.exit_code, 0,
            "check-approved must exit 0 when no production pairs exist, got: {outcome:?}"
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

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_check_approved_invalid_config_still_checks_cache() {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_id = "test-ref-verify-approved-config";
        write_chain1_fixture(&items_dir, track_id);
        write_pass_cache_for_first_chain1_pair(&items_dir, track_id);
        let config_dir = project_root.join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("ref-verify.json"), r#"{"max_parallelism": 0}"#).unwrap();

        let outcome = with_fake_track_branch(&project_root, track_id, || {
            CliApp::new().ref_verify_check_approved(RefVerifyCheckApprovedInput {
                track_id: track_id.to_owned(),
                items_dir,
            })
        })
        .unwrap();

        assert_eq!(
            outcome.exit_code, 0,
            "expected cache approval despite runtime config: {outcome:?}"
        );
    }

    // ── ref_verify_run ───────────────────────────────────────────────────────

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

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_run_in_progress_catalogues_refreshes_chain2_caches_by_default() {
        assert_ref_verify_cache_refresh_behavior(
            "test-ref-verify-run-phase-caches",
            None,
            false,
            "expected successful phase run",
            "phase ref-verify run must not refresh Chain1 approval cache",
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_ref_verify_run_approval_scope_refreshes_all_caches() {
        assert_ref_verify_cache_refresh_behavior(
            "test-ref-verify-run-approval-caches",
            Some("approval"),
            true,
            "expected successful approval run",
            "approval ref-verify run must refresh Chain1 cache before check-approved",
        );
    }

    #[cfg(unix)]
    fn assert_ref_verify_cache_refresh_behavior(
        track_id: &str,
        run_scope: Option<&str>,
        expect_chain1_cache: bool,
        success_message: &str,
        chain1_message: &str,
    ) {
        let (_tmp, items_dir) = temp_project_with_items_dir();
        let project_root = project_root_from_items_dir(&items_dir).to_path_buf();
        let track_dir = items_dir.join(track_id);
        write_chain1_fixture(&items_dir, track_id);
        write_tddd_architecture_rules(&project_root);
        write_impl_plan_statuses(&track_dir, &["done", "in_progress", "todo"]);
        for layer in ["domain", "usecase", "infrastructure"] {
            write_catalogue_with_goal_ref(&track_dir, layer);
        }

        let outcome =
            run_ref_verify_with_fake_claude_scope(&project_root, track_id, items_dir, run_scope);

        assert_eq!(outcome.exit_code, 0, "{success_message}: {outcome:?}");
        assert_eq!(
            track_dir.join("spec-adr-verify-cache.json").exists(),
            expect_chain1_cache,
            "{chain1_message}"
        );
        for file in [
            "domain-catalogue-spec-verify-cache.json",
            "usecase-catalogue-spec-verify-cache.json",
            "infrastructure-catalogue-spec-verify-cache.json",
        ] {
            assert!(track_dir.join(file).exists(), "expected cache artifact {file}");
        }
    }
}
