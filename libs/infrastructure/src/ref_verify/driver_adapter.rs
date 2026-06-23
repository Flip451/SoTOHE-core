//! Infrastructure adapters implementing the high-level driver service traits for
//! the `ref_verify` command family.
//!
//! [`FsRefVerifyRunAdapter`] implements [`usecase::ref_verify::RefVerifyRunService`].
//! [`FsRefVerifyCheckApprovedAdapter`] implements
//! [`usecase::ref_verify::RefVerifyCheckApprovedDriverService`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use usecase::ref_verify::{
    CheckApprovedOutcome, RefVerifyApplicationService as _, RefVerifyCheckApprovedDriverService,
    RefVerifyCheckApprovedInteractor, RefVerifyCheckApprovedOutcome, RefVerifyDriverError,
    RefVerifyRunOutcome, RefVerifyRunService,
};

use super::{
    AgentRefVerifierAdapter, RefVerifyCacheAdapter, RefVerifyPairSourceAdapter,
    RefVerifyScopeResolver, make_ref_verifier_process_runner,
};
use crate::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles};

const REF_VERIFY_CONFIG_PATH: &str = ".harness/config/ref-verify.json";

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RefVerifyConfigDto {
    known_bad_injection_rate_percent: Option<u8>,
    known_bad_detection_threshold_percent: Option<u8>,
    max_parallelism: Option<usize>,
}

fn resolve_project_root(items_dir: &Path) -> Result<PathBuf, String> {
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
            Ok(root)
        }
        _ => Err(format!(
            "--items-dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        )),
    }
}

fn normalize_project_root(root: &Path) -> PathBuf {
    if root.as_os_str().is_empty() { PathBuf::from(".") } else { root.to_path_buf() }
}

fn ensure_trusted_project_root(root: &Path) -> Result<(), String> {
    match root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(format!("refusing to use symlinked project root: {}", root.display()))
        }
        Ok(_) => Ok(()),
        Err(e) => Err(format!("failed to stat project root {}: {e}", root.display())),
    }
}

fn reject_items_dir_symlinks(items_dir: &Path, project_root: &Path) -> Result<(), String> {
    crate::track::symlink_guard::reject_symlinks_below(items_dir, project_root).map(|_| ()).map_err(
        |e| format!("items_dir path rejected before use at '{}': {e}", items_dir.display()),
    )
}

fn validate_track_id(track_id: &str) -> Result<domain::TrackId, String> {
    domain::TrackId::try_new(track_id.to_owned()).map_err(|e| format!("invalid --track-id: {e}"))
}

fn reject_workspace_symlinks(path: &Path, trusted_root: &Path, label: &str) -> Result<(), String> {
    crate::track::symlink_guard::reject_symlinks_below(path, trusted_root)
        .map(|_| ())
        .map_err(|e| format!("{label} path rejected before read at '{}': {e}", path.display()))
}

fn load_ref_verify_config(
    project_root: &Path,
) -> Result<usecase::ref_verify::RefVerifyConfig, String> {
    let config_path = project_root.join(REF_VERIFY_CONFIG_PATH);
    if !crate::track::symlink_guard::reject_symlinks_below(&config_path, project_root).map_err(
        |e| {
            format!(
                "ref-verify config path rejected before read at '{}': {e}",
                config_path.display()
            )
        },
    )? {
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

fn load_agent_profiles(project_root: &Path) -> Result<AgentProfiles, String> {
    let profiles_path = project_root.join(AGENT_PROFILES_PATH);
    reject_workspace_symlinks(&profiles_path, project_root, "agent-profiles.json")?;
    AgentProfiles::load(&profiles_path).map_err(|e| format!("cannot load agent-profiles.json: {e}"))
}

fn current_git_branch(project_root: &Path) -> Result<String, String> {
    use crate::git_cli::{GitRepository as _, SystemGitRepo};
    SystemGitRepo::discover_from(project_root)
        .map_err(|e| format!("cannot discover git repo: {e}"))?
        .current_branch()
        .map_err(|e| format!("cannot read current branch: {e}"))?
        .ok_or_else(|| "cannot read current branch: HEAD is detached".to_owned())
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

        let project_root = resolve_project_root(items_dir).map_err(RefVerifyDriverError::Wiring)?;
        let canonical_root = project_root.canonicalize().map_err(|e| {
            RefVerifyDriverError::Wiring(format!("cannot canonicalize project root: {e}"))
        })?;

        let track_id = validate_track_id(track_id_str).map_err(RefVerifyDriverError::Wiring)?;

        let current_branch =
            current_git_branch(&canonical_root).map_err(RefVerifyDriverError::Unavailable)?;

        let resolver = RefVerifyScopeResolver::new(canonical_root.clone());
        let scope = resolver.resolve(track_id.as_ref()).map_err(|e| {
            RefVerifyDriverError::Wiring(format!("ref-verify scope resolution failed: {e}"))
        })?;

        let config =
            load_ref_verify_config(&canonical_root).map_err(RefVerifyDriverError::Unavailable)?;

        let pair_source =
            Arc::new(RefVerifyPairSourceAdapter::new(canonical_root.clone())) as Arc<_>;
        let cache = Arc::new(RefVerifyCacheAdapter::new(canonical_root.clone())) as Arc<_>;

        let profiles =
            load_agent_profiles(&canonical_root).map_err(RefVerifyDriverError::Unavailable)?;
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
        let project_root = resolve_project_root(items_dir).map_err(RefVerifyDriverError::Wiring)?;
        let canonical_root = project_root.canonicalize().map_err(|e| {
            RefVerifyDriverError::Wiring(format!("cannot canonicalize project root: {e}"))
        })?;

        let track_id = validate_track_id(track_id_str).map_err(RefVerifyDriverError::Wiring)?;

        let current_branch =
            current_git_branch(&canonical_root).map_err(RefVerifyDriverError::Unavailable)?;

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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
}
