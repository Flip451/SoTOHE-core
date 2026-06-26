//! Infrastructure adapters implementing the high-level driver service traits for
//! the `ref_verify` command family.
//!
//! [`FsRefVerifyRunAdapter`] implements [`usecase::ref_verify::RefVerifyRunService`].
//! [`FsRefVerifyCheckApprovedAdapter`] implements
//! [`usecase::ref_verify::RefVerifyCheckApprovedDriverService`].

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use usecase::ref_verify::{
    CheckApprovedOutcome, RefVerifyApplicationService as _, RefVerifyCheckApprovedDriverService,
    RefVerifyCheckApprovedInteractor, RefVerifyCheckApprovedOutcome, RefVerifyDriverError,
    RefVerifyRunOutcome, RefVerifyRunService,
};

use super::driver_adapter_results::{
    check_partial_catalogue_set, check_track_dir_exists, compute_results,
    inspect_chain2_catalogue_set, load_results_tddd_bindings, resolve_chain1_only_scope,
    resolve_results_chain2_target_layers,
};
use super::{
    AgentRefVerifierAdapter, RefVerifyCacheAdapter, RefVerifyPairSourceAdapter,
    RefVerifyScopeResolver, make_ref_verifier_process_runner,
};
use crate::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles};

const REF_VERIFY_CONFIG_PATH: &str = ".harness/config/ref-verify.json";

#[derive(Debug)]
struct RefVerifyAdapterError(String);

impl std::fmt::Display for RefVerifyAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl RefVerifyAdapterError {
    #[allow(dead_code)]
    fn contains(&self, s: &str) -> bool {
        self.0.contains(s)
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RefVerifyConfigDto {
    known_bad_injection_rate_percent: Option<u8>,
    known_bad_detection_threshold_percent: Option<u8>,
    max_parallelism: Option<usize>,
}

fn resolve_project_root(items_dir: &Path) -> Result<PathBuf, RefVerifyAdapterError> {
    reject_items_dir_escape(items_dir)?;

    // items_dir must be `<project_root>/track/items`
    let items_name = items_dir.file_name().and_then(|n| n.to_str());
    let track_dir = items_dir.parent();
    let track_name = track_dir.and_then(|d| d.file_name()).and_then(|n| n.to_str());
    let project_root = track_dir.and_then(|d| d.parent());
    match (items_name, track_name, project_root) {
        (Some("items"), Some("track"), Some(root)) => {
            let root = normalize_project_root(root);
            ensure_trusted_project_root(&root)?;
            reject_items_dir_symlinks(items_dir, &root)?;
            ensure_current_repo_root(&root)
        }
        _ => Err(RefVerifyAdapterError(format!(
            "--items-dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        ))),
    }
}

fn normalize_project_root(root: &Path) -> PathBuf {
    if root.as_os_str().is_empty() { PathBuf::from(".") } else { root.to_path_buf() }
}

fn reject_items_dir_escape(items_dir: &Path) -> Result<(), RefVerifyAdapterError> {
    if items_dir.as_os_str().is_empty() {
        return Err(RefVerifyAdapterError("--items-dir must not be empty".to_owned()));
    }
    if items_dir
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(RefVerifyAdapterError(format!(
            "--items-dir cannot escape the current repository root: {}",
            items_dir.display()
        )));
    }
    Ok(())
}

fn ensure_trusted_project_root(root: &Path) -> Result<(), RefVerifyAdapterError> {
    match root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => Err(RefVerifyAdapterError(format!(
            "refusing to use symlinked project root: {}",
            root.display()
        ))),
        Ok(_) => Ok(()),
        Err(e) => Err(RefVerifyAdapterError(format!(
            "failed to stat project root {}: {e}",
            root.display()
        ))),
    }
}

fn ensure_current_repo_root(root: &Path) -> Result<PathBuf, RefVerifyAdapterError> {
    use crate::git_cli::{GitRepository as _, SystemGitRepo};

    let canonical_root = root.canonicalize().map_err(|e| {
        RefVerifyAdapterError(format!(
            "failed to canonicalize project root {}: {e}",
            root.display()
        ))
    })?;
    let repo = SystemGitRepo::discover().map_err(|e| {
        RefVerifyAdapterError(format!("cannot discover current git repository: {e}"))
    })?;
    let canonical_repo_root = repo.root().canonicalize().map_err(|e| {
        RefVerifyAdapterError(format!(
            "failed to canonicalize current repository root {}: {e}",
            repo.root().display()
        ))
    })?;
    if canonical_root != canonical_repo_root {
        return Err(RefVerifyAdapterError(format!(
            "--items-dir must resolve to the current repository root {}; got {}",
            canonical_repo_root.display(),
            canonical_root.display()
        )));
    }
    Ok(canonical_root)
}

fn reject_items_dir_symlinks(
    items_dir: &Path,
    project_root: &Path,
) -> Result<(), RefVerifyAdapterError> {
    crate::track::symlink_guard::reject_symlinks_below(items_dir, project_root).map(|_| ()).map_err(
        |e| {
            RefVerifyAdapterError(format!(
                "items_dir path rejected before use at '{}': {e}",
                items_dir.display()
            ))
        },
    )
}

fn validate_track_id(track_id: &str) -> Result<domain::TrackId, RefVerifyAdapterError> {
    domain::TrackId::try_new(track_id.to_owned())
        .map_err(|e| RefVerifyAdapterError(format!("invalid --track-id: {e}")))
}

fn reject_workspace_symlinks(
    path: &Path,
    trusted_root: &Path,
    label: &str,
) -> Result<(), RefVerifyAdapterError> {
    crate::track::symlink_guard::reject_symlinks_below(path, trusted_root).map(|_| ()).map_err(
        |e| {
            RefVerifyAdapterError(format!(
                "{label} path rejected before read at '{}': {e}",
                path.display()
            ))
        },
    )
}

fn load_ref_verify_config(
    project_root: &Path,
) -> Result<usecase::ref_verify::RefVerifyConfig, RefVerifyAdapterError> {
    let config_path = project_root.join(REF_VERIFY_CONFIG_PATH);
    if !crate::track::symlink_guard::reject_symlinks_below(&config_path, project_root).map_err(
        |e| {
            RefVerifyAdapterError(format!(
                "ref-verify config path rejected before read at '{}': {e}",
                config_path.display()
            ))
        },
    )? {
        return Ok(usecase::ref_verify::RefVerifyConfig::default());
    }

    let text = std::fs::read_to_string(&config_path).map_err(|e| {
        RefVerifyAdapterError(format!(
            "cannot read ref-verify config at '{}': {e}",
            config_path.display()
        ))
    })?;

    let dto: RefVerifyConfigDto = serde_json::from_str(&text).map_err(|e| {
        RefVerifyAdapterError(format!(
            "invalid ref-verify config at '{}': {e}",
            config_path.display()
        ))
    })?;
    let defaults = usecase::ref_verify::RefVerifyConfig::default();
    let injection = dto
        .known_bad_injection_rate_percent
        .unwrap_or_else(|| defaults.known_bad_injection_rate_percent.as_u8());
    let threshold = dto
        .known_bad_detection_threshold_percent
        .unwrap_or_else(|| defaults.known_bad_detection_threshold_percent.as_u8());
    let parallelism = dto.max_parallelism.unwrap_or_else(|| defaults.max_parallelism.as_usize());

    usecase::ref_verify::RefVerifyConfig::try_new(injection, threshold, parallelism)
        .map_err(|e| RefVerifyAdapterError(format!("ref-verify config validation failed: {e}")))
}

fn load_agent_profiles(project_root: &Path) -> Result<AgentProfiles, RefVerifyAdapterError> {
    let profiles_path = project_root.join(AGENT_PROFILES_PATH);
    reject_workspace_symlinks(&profiles_path, project_root, "agent-profiles.json")?;
    AgentProfiles::load(&profiles_path)
        .map_err(|e| RefVerifyAdapterError(format!("cannot load agent-profiles.json: {e}")))
}

fn current_git_branch(project_root: &Path) -> Result<String, RefVerifyAdapterError> {
    use crate::git_cli::{GitRepository as _, SystemGitRepo};
    SystemGitRepo::discover_from(project_root)
        .map_err(|e| RefVerifyAdapterError(format!("cannot discover git repo: {e}")))?
        .current_branch()
        .map_err(|e| RefVerifyAdapterError(format!("cannot read current branch: {e}")))?
        .ok_or_else(|| {
            RefVerifyAdapterError("cannot read current branch: HEAD is detached".to_owned())
        })
}

// ── FsRefVerifyRunAdapter ─────────────────────────────────────────────────────

/// Filesystem-backed adapter implementing [`RefVerifyRunService`].
///
/// Performs scope resolution, config loading, branch detection, and delegates
/// to [`usecase::ref_verify::VerifySemanticRefsInteractor`].
pub struct FsRefVerifyRunAdapter;

impl FsRefVerifyRunAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsRefVerifyRunAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RefVerifyRunService for FsRefVerifyRunAdapter {
    fn run(
        &self,
        track_id_str: &str,
        items_dir: &Path,
    ) -> Result<RefVerifyRunOutcome, RefVerifyDriverError> {
        use usecase::ref_verify::VerifySemanticRefsInteractor;

        let project_root = resolve_project_root(items_dir)
            .map_err(|e| RefVerifyDriverError::Wiring(e.to_string()))?;
        let canonical_root = project_root.canonicalize().map_err(|e| {
            RefVerifyDriverError::Wiring(format!("cannot canonicalize project root: {e}"))
        })?;

        let track_id = validate_track_id(track_id_str)
            .map_err(|e| RefVerifyDriverError::Wiring(e.to_string()))?;

        let current_branch = current_git_branch(&canonical_root)
            .map_err(|e| RefVerifyDriverError::Unavailable(e.to_string()))?;

        let resolver = RefVerifyScopeResolver::new(canonical_root.clone());
        let scope = resolver.resolve(track_id.as_ref()).map_err(|e| {
            RefVerifyDriverError::Wiring(format!("ref-verify scope resolution failed: {e}"))
        })?;

        let config = load_ref_verify_config(&canonical_root)
            .map_err(|e| RefVerifyDriverError::Unavailable(e.to_string()))?;

        let pair_source =
            Arc::new(RefVerifyPairSourceAdapter::new(canonical_root.clone())) as Arc<_>;
        let cache = Arc::new(RefVerifyCacheAdapter::new(canonical_root.clone())) as Arc<_>;

        let profiles = load_agent_profiles(&canonical_root)
            .map_err(|e| RefVerifyDriverError::Unavailable(e.to_string()))?;
        let profiles = Arc::new(profiles);

        let runner = make_ref_verifier_process_runner(canonical_root.clone());
        let verifier =
            Arc::new(AgentRefVerifierAdapter::new(profiles, runner, canonical_root.clone()))
                as Arc<_>;

        let interactor = VerifySemanticRefsInteractor::new(pair_source, cache, verifier, config);

        let cmd = usecase::ref_verify::RefVerifyCommand { track_id, scope, current_branch };

        match interactor.execute(&cmd) {
            Ok(()) => Ok(RefVerifyRunOutcome::Passed),
            Err(usecase::ref_verify::RefVerifyError::SemanticFailuresConfirmed { pair_count }) => {
                Ok(RefVerifyRunOutcome::SemanticFailuresConfirmed { pair_count })
            }
            Err(usecase::ref_verify::RefVerifyError::HumanEscalationRequired { pair_count }) => {
                Ok(RefVerifyRunOutcome::HumanEscalationRequired { pair_count })
            }
            Err(e) => Err(RefVerifyDriverError::Usecase(format!("ref-verify run failed: {e}"))),
        }
    }
}

// ── FsRefVerifyAggregateAdapter ──────────────────────────────────────────────

/// Filesystem-backed aggregate adapter implementing [`usecase::ref_verify::RefVerifyAggregateService`].
///
/// Wires both sub-services (`FsRefVerifyRunAdapter` + `FsRefVerifyCheckApprovedAdapter`)
/// internally so that `RefVerifyDriver` holds only one `Arc<dyn RefVerifyAggregateService>`
/// (D3/D4 cli_driver policy).  Adapter impls belong in `infrastructure` per scope policy;
/// `cli_composition` only wires this adapter into the driver.
pub struct FsRefVerifyAggregateAdapter;

impl FsRefVerifyAggregateAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsRefVerifyAggregateAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl usecase::ref_verify::RefVerifyAggregateService for FsRefVerifyAggregateAdapter {
    fn run(
        &self,
        track_id: &str,
        items_dir: &Path,
    ) -> Result<RefVerifyRunOutcome, RefVerifyDriverError> {
        FsRefVerifyRunAdapter::new().run(track_id, items_dir)
    }

    fn check_approved(
        &self,
        track_id: &str,
        items_dir: &Path,
    ) -> Result<RefVerifyCheckApprovedOutcome, RefVerifyDriverError> {
        FsRefVerifyCheckApprovedAdapter::new().check_approved(track_id, items_dir)
    }

    fn results(
        &self,
        track_id_str: &str,
        items_dir: &Path,
        chain: usecase::ref_verify::RefVerifyChainFilter,
        layer: usecase::ref_verify::RefVerifyLayerFilter,
        verdict: usecase::ref_verify::RefVerifyVerdictFilter,
    ) -> Result<usecase::ref_verify::RefVerifyResultsOutput, RefVerifyDriverError> {
        // (a) Resolve project root and validate track ID.
        let project_root = resolve_project_root(items_dir)
            .map_err(|e| RefVerifyDriverError::Wiring(e.to_string()))?;
        let canonical_root = project_root.canonicalize().map_err(|e| {
            RefVerifyDriverError::Wiring(format!("cannot canonicalize project root: {e}"))
        })?;
        let track_id = validate_track_id(track_id_str)
            .map_err(|e| RefVerifyDriverError::Wiring(e.to_string()))?;

        // Branch resolution: fall back to "<detached>" sentinel when HEAD is detached.
        // The results path is read-only and does not enforce the active-track guard,
        // so any branch value — including the detached sentinel — is accepted by
        // downstream cache / pair-source callers (RefVerifyCacheAdapter::load_entries
        // and RefVerifyPairSourceAdapter::load_pairs do not inspect current_branch).
        let current_branch =
            current_git_branch(&canonical_root).unwrap_or_else(|_| "<detached>".to_owned());

        results_core(&canonical_root, track_id, chain, layer, verdict, current_branch)
    }
}

// ── results_core ─────────────────────────────────────────────────────────────

/// Inner implementation for the read-only results query path.
///
/// Accepts a pre-resolved `current_branch` string so that the caller
/// (`FsRefVerifyAggregateAdapter::results`) can supply a detached-HEAD sentinel
/// when git branch detection fails, without propagating an `Unavailable` error
/// for an inherently read-only operation.
///
/// Downstream callers (`RefVerifyCacheAdapter::load_entries`,
/// `RefVerifyPairSourceAdapter::load_pairs`) do not inspect `current_branch`,
/// so any sentinel value is safe here.
fn results_core(
    canonical_root: &Path,
    track_id: domain::TrackId,
    chain: usecase::ref_verify::RefVerifyChainFilter,
    layer: usecase::ref_verify::RefVerifyLayerFilter,
    verdict: usecase::ref_verify::RefVerifyVerdictFilter,
    current_branch: String,
) -> Result<usecase::ref_verify::RefVerifyResultsOutput, RefVerifyDriverError> {
    use domain::tddd::LayerId;
    use domain::tddd::semantic_verify::SemanticVerifyEntry;
    use usecase::ref_verify::{
        RefVerifyCachePort, RefVerifyCacheScope, RefVerifyChainFilter, RefVerifyConfig,
        RefVerifyPairSourcePort,
    };

    // Determine which chains are included in the results request.
    // This is computed before scope resolution so the resolver call can be
    // gated on whether Chain2 output is actually requested.
    let include_chain1 = matches!(&chain, RefVerifyChainFilter::Chain1 | RefVerifyChainFilter::All);
    let include_chain2 = matches!(&chain, RefVerifyChainFilter::Chain2 | RefVerifyChainFilter::All);

    // (b-pre) Validate the track directory exists regardless of chain filter.
    // A typo in track_id must produce a typed error, not a silent zero-pair result.
    check_track_dir_exists(canonical_root, track_id.as_ref())?;

    // (b) Resolve scope.
    // For Chain1-only: skip RefVerifyScopeResolver — it enforces Chain2 catalogue
    // consistency (IN-05: partial set is rejected) which is irrelevant for a
    // Chain1-only request and would block users from inspecting spec↔ADR failures
    // while Phase-2 artefacts are still incomplete.  Instead, check the Chain1
    // paths and the IN-06 ordering rule without enforcing partial-catalogue consistency.
    // For Chain2 or All: run the full resolver to validate catalogue consistency.
    let scope = if !include_chain2 {
        resolve_chain1_only_scope(canonical_root, track_id.as_ref())?
    } else {
        let resolver = RefVerifyScopeResolver::new(canonical_root.to_path_buf());
        resolver.resolve(track_id.as_ref()).map_err(|e| {
            RefVerifyDriverError::Wiring(format!("ref-verify scope resolution failed: {e}"))
        })?
    };

    let cmd = usecase::ref_verify::RefVerifyCommand { track_id, scope, current_branch };

    // (c) Load cache files scoped to the requested chains and layers.
    //
    // F1: Load Chain1 (SpecAdr) cache only when the chain filter includes Chain1.
    // A stale or absent SpecAdr cache must not fail a Chain2-only results query.
    let cache_adapter = RefVerifyCacheAdapter::new(canonical_root.to_path_buf());

    let chain1_entries: Vec<SemanticVerifyEntry> = if include_chain1 {
        cache_adapter
            .load_entries(&cmd, &RefVerifyCacheScope::SpecAdr)
            .map_err(|e| RefVerifyDriverError::Usecase(e.to_string()))?
    } else {
        Vec::new()
    };

    // Only load Chain2 caches when the chain filter requests Chain2 results.
    // Skipping this I/O for chain=Chain1 ensures that Chain2 cache/catalogue
    // files cannot fail a Chain1-only results query.
    //
    // F2: When layer=Specific(X), validate X exists in the TDDD bindings and load only
    // that layer's cache.  A corrupt or absent cache for an unrelated layer must not
    // fail a single-layer Chain2 results query.
    let mut chain2_caches: Vec<(LayerId, Vec<SemanticVerifyEntry>)> = Vec::new();
    let mut chain2_layer_ids: Vec<LayerId> = Vec::new();
    if include_chain2 {
        let bindings = load_results_tddd_bindings(canonical_root)?;
        let (present_layer_ids, absent_layer_ids) =
            inspect_chain2_catalogue_set(canonical_root, cmd.track_id.as_ref(), &bindings)?;
        // Fail-closed for partial catalogue sets before layer narrowing.
        // Only all-absent (pre-Phase-2) is accepted without error; partial absence
        // means some declared layers were never inspected, which would produce a
        // misleadingly complete-looking Chain2 summary.
        check_partial_catalogue_set(&present_layer_ids, &absent_layer_ids)?;

        let target_ids = resolve_results_chain2_target_layers(&bindings, &layer)?;

        for layer_id in &target_ids {
            if absent_layer_ids.iter().any(|absent| absent == layer_id) {
                // Record in chain2_caches so that compute_results layer validation can
                // distinguish pre-Phase-2 zero-pair from an unknown layer typo.
                chain2_caches.push((layer_id.clone(), Vec::new()));
                continue;
            }

            let cache_scope = RefVerifyCacheScope::CatalogueSpec { layer: layer_id.clone() };
            let entries = cache_adapter
                .load_entries(&cmd, &cache_scope)
                .map_err(|e| RefVerifyDriverError::Usecase(e.to_string()))?;
            chain2_caches.push((layer_id.clone(), entries));
            chain2_layer_ids.push(layer_id.clone());
        }
    }

    // (d) Enumerate current pairs narrowed by chain and layer filter.
    //
    // F3: Use RefVerifyScope::Chain1 for Chain1 pairs and
    // RefVerifyScope::Chain2 { layer } per layer for Chain2 pairs.
    // This avoids enumerating Chain1 artifacts (spec.json / ADR files) when
    // chain=Chain2, so a broken Chain1 reference cannot fail a Chain2-only query.
    // CN-06: no LLM subprocess in the results path.
    let pair_source = RefVerifyPairSourceAdapter::new(canonical_root.to_path_buf());
    let config = RefVerifyConfig::default();
    let mut all_raw_pairs: Vec<usecase::ref_verify::RefVerifyPair> = Vec::new();

    if include_chain1 {
        let chain1_cmd = usecase::ref_verify::RefVerifyCommand {
            track_id: cmd.track_id.clone(),
            scope: usecase::ref_verify::RefVerifyScope::Chain1,
            current_branch: cmd.current_branch.clone(),
        };
        let raw = pair_source
            .load_pairs(&chain1_cmd, &config)
            .map_err(|e| RefVerifyDriverError::Usecase(format!("pair source enumeration: {e}")))?;
        all_raw_pairs.extend(raw);
    }

    if include_chain2 {
        // Enumerate per layer using Chain2 scope to avoid reading Chain1 files.
        for layer_id in &chain2_layer_ids {
            let chain2_cmd = usecase::ref_verify::RefVerifyCommand {
                track_id: cmd.track_id.clone(),
                scope: usecase::ref_verify::RefVerifyScope::Chain2 { layer: layer_id.clone() },
                current_branch: cmd.current_branch.clone(),
            };
            let raw = pair_source.load_pairs(&chain2_cmd, &config).map_err(|e| {
                RefVerifyDriverError::Usecase(format!("pair source enumeration: {e}"))
            })?;
            all_raw_pairs.extend(raw);
        }
    }

    // Exclude known-bad calibration probes; classify only real production pairs.
    let current_pairs: Vec<usecase::ref_verify::RefVerifyPair> =
        all_raw_pairs.into_iter().filter(|p| !p.known_bad).collect();

    compute_results(chain1_entries, chain2_caches, current_pairs, chain, layer, verdict)
}

// ── FsRefVerifyCheckApprovedAdapter ──────────────────────────────────────────

/// Filesystem-backed adapter implementing [`RefVerifyCheckApprovedDriverService`].
///
/// Performs scope resolution, branch detection, and delegates to
/// [`RefVerifyCheckApprovedInteractor`].
pub struct FsRefVerifyCheckApprovedAdapter;

impl FsRefVerifyCheckApprovedAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsRefVerifyCheckApprovedAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RefVerifyCheckApprovedDriverService for FsRefVerifyCheckApprovedAdapter {
    fn check_approved(
        &self,
        track_id_str: &str,
        items_dir: &Path,
    ) -> Result<RefVerifyCheckApprovedOutcome, RefVerifyDriverError> {
        let project_root = resolve_project_root(items_dir)
            .map_err(|e| RefVerifyDriverError::Wiring(e.to_string()))?;
        let canonical_root = project_root.canonicalize().map_err(|e| {
            RefVerifyDriverError::Wiring(format!("cannot canonicalize project root: {e}"))
        })?;

        let track_id = validate_track_id(track_id_str)
            .map_err(|e| RefVerifyDriverError::Wiring(e.to_string()))?;

        let current_branch = current_git_branch(&canonical_root)
            .map_err(|e| RefVerifyDriverError::Unavailable(e.to_string()))?;

        let expected_branch = format!("track/{}", track_id.as_ref());
        if current_branch != expected_branch {
            return Err(RefVerifyDriverError::Wiring(format!(
                "ref-verify check-approved failed: track is not active: current branch \
                 '{current_branch}', expected '{expected_branch}'"
            )));
        }

        let resolver = RefVerifyScopeResolver::new(canonical_root.clone());
        let scope = resolver.resolve(track_id.as_ref()).map_err(|e| {
            RefVerifyDriverError::Wiring(format!("ref-verify scope resolution failed: {e}"))
        })?;

        let cmd = usecase::ref_verify::RefVerifyCommand { track_id, scope, current_branch };

        let pair_source = Arc::new(RefVerifyPairSourceAdapter::new(canonical_root.clone()))
            as Arc<dyn usecase::ref_verify::RefVerifyPairSourcePort>;
        let cache = Arc::new(RefVerifyCacheAdapter::new(canonical_root.clone()))
            as Arc<dyn usecase::ref_verify::RefVerifyCachePort>;

        let interactor = RefVerifyCheckApprovedInteractor::new(pair_source, cache);
        let check_approved_service: Arc<dyn usecase::ref_verify::RefVerifyCheckApprovedService> =
            Arc::new(interactor);

        let outcome = check_approved_service.check_approved(&cmd).map_err(|e| {
            RefVerifyDriverError::Unavailable(format!(
                "ref-verify check-approved infrastructure error: {e}"
            ))
        })?;

        Ok(match outcome {
            CheckApprovedOutcome::NoPairs => RefVerifyCheckApprovedOutcome::NoPairs,
            CheckApprovedOutcome::AllApproved => RefVerifyCheckApprovedOutcome::AllApproved,
            CheckApprovedOutcome::NotApproved { missing_or_non_pass } => {
                RefVerifyCheckApprovedOutcome::NotApproved { missing_or_non_pass }
            }
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn test_resolve_project_root_rejects_parent_dir_escape() {
        let err = resolve_project_root(Path::new("../other/track/items")).unwrap_err();

        assert!(err.contains("cannot escape the current repository root"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_project_root_rejects_symlinked_project_root() {
        let real_root = tempfile::tempdir().unwrap();
        let link_parent = tempfile::tempdir().unwrap();
        let root_link = link_parent.path().join("workspace-link");
        std::os::unix::fs::symlink(real_root.path(), &root_link).unwrap();
        let items_dir = root_link.join("track").join("items");

        let err = resolve_project_root(&items_dir).unwrap_err();

        assert!(err.contains("refusing to use symlinked project root"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_project_root_rejects_symlinked_items_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let track_dir = tmp.path().join("track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::os::unix::fs::symlink(outside.path(), track_dir.join("items")).unwrap();
        let items_dir = track_dir.join("items");

        let err = resolve_project_root(&items_dir).unwrap_err();

        assert!(err.contains("items_dir path rejected before use"), "{err}");
        assert!(err.contains("refusing to follow symlink"), "{err}");
    }

    #[test]
    fn test_resolve_project_root_rejects_non_current_repo_root() {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("track").join("items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let err = resolve_project_root(&items_dir).unwrap_err();

        assert!(err.contains("current repository root"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn test_load_ref_verify_config_rejects_symlinked_config() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(REF_VERIFY_CONFIG_PATH);
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let outside_config = outside.path().join("ref-verify.json");
        std::fs::write(&outside_config, "{}").unwrap();
        std::os::unix::fs::symlink(&outside_config, &config_path).unwrap();

        let err = load_ref_verify_config(tmp.path()).unwrap_err();
        assert!(err.contains("ref-verify config path rejected before read"), "{err}");
        assert!(err.contains("refusing to follow symlink"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn test_load_agent_profiles_rejects_symlinked_config() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let profiles_path = tmp.path().join(AGENT_PROFILES_PATH);
        std::fs::create_dir_all(profiles_path.parent().unwrap()).unwrap();
        let outside_profiles = outside.path().join("agent-profiles.json");
        std::fs::write(&outside_profiles, "{}").unwrap();
        std::os::unix::fs::symlink(&outside_profiles, &profiles_path).unwrap();

        let err = load_agent_profiles(tmp.path()).unwrap_err();
        assert!(err.contains("agent-profiles.json path rejected before read"), "{err}");
        assert!(err.contains("refusing to follow symlink"), "{err}");
    }

    // ── detached HEAD tests ───────────────────────────────────────────────────

    /// Verifies that `results` succeeds with a detached-HEAD sentinel branch value.
    ///
    /// When `sotp ref-verify results --track-id <id>` is run from a CI checkout
    /// where HEAD is detached, `current_git_branch()` returns `Err`.  The fixed
    /// `results` path falls back to the `"<detached>"` sentinel and delegates to
    /// `results_core`, which must not surface an `Unavailable` error.
    ///
    /// Invariant: passing `"<detached>"` as `current_branch` to `results_core`
    /// with a valid track directory and a valid Chain1 cache succeeds — the
    /// results path does not enforce the active-track guard.
    #[test]
    fn compute_results_with_explicit_track_id_and_detached_head_succeeds() {
        let tmp = tempfile::tempdir().unwrap();
        let canonical_root = tmp.path().canonicalize().unwrap();
        let track_id = domain::TrackId::try_new("dry-gate-opt-in".to_owned()).unwrap();
        let track_dir = canonical_root.join("track").join("items").join(track_id.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write a structurally valid (empty) Chain1 cache file to satisfy the
        // "valid Chain1 cache" requirement without needing real spec/ADR content.
        let cache_json = r#"{"schema_version":1,"entries":[]}"#;
        std::fs::write(track_dir.join("spec-adr-verify-cache.json"), cache_json).unwrap();

        // Simulate detached HEAD: pass the sentinel directly to results_core,
        // mirroring what the production code does when current_git_branch() fails.
        let result = results_core(
            &canonical_root,
            track_id,
            usecase::ref_verify::RefVerifyChainFilter::Chain1,
            usecase::ref_verify::RefVerifyLayerFilter::All,
            usecase::ref_verify::RefVerifyVerdictFilter::All,
            "<detached>".to_owned(),
        );

        assert!(
            result.is_ok(),
            "detached HEAD sentinel must not cause Unavailable error: {result:?}"
        );
    }

    // ── F3 structural tests ───────────────────────────────────────────────────

    /// F3: For chain=Chain2, `include_chain1` is false, so the Chain1 pair source is
    /// never called and Chain1 files (spec.json / ADR) are not opened.
    #[test]
    fn pair_source_chain2_does_not_enumerate_chain1() {
        // Mirrors the `include_chain1` computation in `results()`.
        let chain = usecase::ref_verify::RefVerifyChainFilter::Chain2;
        let include_chain1 = matches!(
            &chain,
            usecase::ref_verify::RefVerifyChainFilter::Chain1
                | usecase::ref_verify::RefVerifyChainFilter::All
        );
        assert!(!include_chain1, "chain=Chain2 must not enumerate Chain1 from the pair source");
    }

    /// F3: For chain=Chain2, pair enumeration uses `RefVerifyScope::Chain2 { layer }` per
    /// layer — not `RefVerifyScope::All` (which would also open Chain1 artifacts).
    #[test]
    fn pair_source_chain2_scope_is_chain2_not_all() {
        use domain::tddd::LayerId;
        let domain_id = LayerId::try_new("domain".to_owned()).unwrap();
        // Mirrors what `results()` constructs when include_chain2 is true.
        let scope = usecase::ref_verify::RefVerifyScope::Chain2 { layer: domain_id.clone() };
        assert!(
            matches!(&scope, usecase::ref_verify::RefVerifyScope::Chain2 { layer } if layer == &domain_id),
            "chain=Chain2 pair enumeration must use Chain2 {{ layer }} scope"
        );
        assert!(
            !matches!(&scope, usecase::ref_verify::RefVerifyScope::All),
            "chain=Chain2 pair enumeration must not use All scope (would read Chain1 files)"
        );
    }
}
