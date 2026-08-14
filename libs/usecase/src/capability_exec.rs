//! Validated values and error vocabulary for generic capability dispatch.
//!
//! This module deliberately contains no I/O, profile resolution, or provider
//! dispatch. Those concerns are introduced through ports and the interactor in
//! the follow-up task.
//!
//! The validated values themselves live in the private `values` submodule and
//! are re-exported here unchanged, so `capability_exec` remains their public
//! path; the split exists only to keep each file under the production-code
//! line limit.

mod values;

use std::path::PathBuf;
use std::sync::Arc;

pub use values::{
    BriefingText, CAPABILITY_EXEC_DISCIPLINE_PATH, CLAUDE_PROVIDER_NAME, CODEX_PROVIDER_NAME,
    CapabilityFailureDetail, CapabilityFilePath, CapabilityInputValidationError,
    CapabilityProviderBinding, DisciplineText, ExecutionMode, ModelName, ModelProviderName,
    ProviderName, ReasoningEffort, TargetArtifactPath, TargetArtifactSet, TimeoutSeconds,
};

use crate::conventions_resolve::{
    ConventionCapabilityId, ConventionResolveError, ConventionResolveService,
    ResolveConventionsQuery,
};
use crate::dry_write_driver::CapabilityName;

/// Request values supplied to generic capability dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityExecRequest {
    /// Capability selected from the runtime profile.
    pub capability: CapabilityName,
    /// Caller-declared host provider, when the caller supplied one.
    pub host: Option<ProviderName>,
    /// Validated path to the capability briefing.
    pub briefing_file: CapabilityFilePath,
    /// Provider-process timeout; `None` waits without a time limit.
    pub timeout: Option<TimeoutSeconds>,
    /// The orchestrator-selected session policy for this dispatch.
    pub resume: CapabilityResumeRequest,
}

/// Explicit capability-session selection.  A fresh dispatch never consults a
/// cache; targetless resume is deliberately fail-closed outside a track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityResumeRequest {
    Fresh,
    ResumeWithoutTarget,
    Resume(TargetArtifactSet),
}

/// Typed routing data resolved from the capability profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityProfile {
    /// Provider binding selected by the profile.
    pub provider: CapabilityProviderBinding,
    /// Single model selected by the profile.
    pub model: ModelName,
    /// Explicit reasoning effort selected by the profile.
    pub effort: ReasoningEffort,
    /// Execution category selected by the profile.
    pub execution_mode: ExecutionMode,
}

/// Fully resolved data supplied to a provider-native dispatch adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDispatchRequest {
    /// The original validated command request.
    pub request: CapabilityExecRequest,
    /// The routing profile resolved for the requested capability.
    pub profile: CapabilityProfile,
    /// The validated contents of the requested briefing file.
    pub briefing: BriefingText,
    /// The validated common execution discipline.
    pub discipline: DisciplineText,
}

/// Observable result of a generic capability dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityDispatchOutcome {
    /// A provider subprocess completed with its opaque process exit status.
    Executed {
        /// Provider that executed the subprocess.
        provider: ProviderName,
        /// Opaque process status returned by the provider executable.
        exit_code: u8,
    },
    /// The current host must invoke the validated native Claude adapter itself.
    DelegateInHost {
        /// Capability to invoke through the host-native mechanism.
        capability: CapabilityName,
        /// Validated briefing path to pass to the host-native mechanism.
        briefing_file: CapabilityFilePath,
        /// Validated discipline text that must accompany the briefing.
        discipline: DisciplineText,
    },
}

/// Application service for generic capability dispatch.
pub trait CapabilityExecService: Send + Sync {
    /// Dispatches a capability after validating all profile and adapter inputs.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed [`CapabilityExecError`] before an in-host
    /// delegation or provider subprocess can occur when any input is invalid.
    fn execute(
        &self,
        request: CapabilityExecRequest,
    ) -> Result<CapabilityDispatchOutcome, CapabilityExecError>;
}

/// Resolves a capability routing profile from the runtime configuration.
pub trait CapabilityProfilePort: Send + Sync {
    /// Resolves the profile for `capability`.
    ///
    /// # Errors
    ///
    /// Implementations may return only [`CapabilityExecError::ProfileResolution`],
    /// [`CapabilityExecError::ModelMissing`], or [`CapabilityExecError::EffortMissing`].
    fn resolve(
        &self,
        capability: &CapabilityName,
    ) -> Result<CapabilityProfile, CapabilityExecError>;
}

/// Loads and validates the common text inputs required by every dispatch branch.
pub trait CapabilitySourcePort: Send + Sync {
    /// Loads and validates the requested briefing content.
    ///
    /// # Errors
    ///
    /// Implementations return [`CapabilityExecError::SourceValidation`] for an
    /// unreadable, non-UTF-8, non-file, or empty source.
    fn load_briefing(&self, path: &CapabilityFilePath)
    -> Result<BriefingText, CapabilityExecError>;

    /// Loads and validates the fixed shared execution discipline.
    ///
    /// # Errors
    ///
    /// Implementations return [`CapabilityExecError::SourceValidation`] when
    /// the fixed template cannot be used.
    fn load_discipline(&self) -> Result<DisciplineText, CapabilityExecError>;
}

/// Provider-native dispatch adapter selected through the runtime profile.
pub trait CapabilityProviderPort: Send + Sync {
    /// Returns the provider identifier implemented by this adapter.
    fn provider(&self) -> &ProviderName;

    /// Performs provider-native preflight and dispatch.
    ///
    /// # Errors
    ///
    /// Implementations may return only [`CapabilityExecError::AdapterPreflight`]
    /// or [`CapabilityExecError::DispatchFailed`].
    fn dispatch(
        &self,
        request: &CapabilityDispatchRequest,
    ) -> Result<CapabilityDispatchOutcome, CapabilityExecError>;
}

/// Pure application-layer coordinator for generic capability dispatch.
///
/// The convention preflight is a dependency of this coordinator rather than of
/// either route branch: both dispatch outcomes pass through here, so resolving
/// above the branch is what makes one resolved value reach whichever route runs,
/// instead of the same rule living at two sites kept equal by review.
///
/// The root that preflight scans is held here as well, because
/// [`ResolveConventionsQuery`] needs one and this type can reach no filesystem
/// fact of its own: anything it derived would be the process working directory,
/// and a dispatch from a subdirectory would then scan a tree holding no
/// `knowledge/conventions/` and be told, truthfully for that tree, that the
/// capability requires nothing. The value is wired in by the composition root,
/// which discovers the repository through git and hands the same root to every
/// adapter it builds.
pub struct CapabilityExecInteractor {
    profile: Arc<dyn CapabilityProfilePort>,
    source: Arc<dyn CapabilitySourcePort>,
    conventions: Arc<dyn ConventionResolveService>,
    providers: Vec<Arc<dyn CapabilityProviderPort>>,
    project_root: PathBuf,
}

impl CapabilityExecInteractor {
    /// Creates an interactor from its profile, source, convention-resolution,
    /// and provider ports, together with the project root its convention
    /// preflight scans.
    #[must_use]
    pub fn new(
        profile: Arc<dyn CapabilityProfilePort>,
        source: Arc<dyn CapabilitySourcePort>,
        conventions: Arc<dyn ConventionResolveService>,
        providers: Vec<Arc<dyn CapabilityProviderPort>>,
        project_root: PathBuf,
    ) -> Self {
        Self { profile, source, conventions, providers, project_root }
    }
}

impl CapabilityExecService for CapabilityExecInteractor {
    fn execute(
        &self,
        request: CapabilityExecRequest,
    ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
        // Both shared sources are deliberately loaded before routing decisions.
        // This keeps every exit path fail-closed under the same discipline.
        let briefing = self.source.load_briefing(&request.briefing_file)?;
        let discipline = self.source.load_discipline()?;
        let profile = self.profile.resolve(&request.capability)?;

        if profile.execution_mode != ExecutionMode::OrchestratorOutput {
            return Err(CapabilityExecError::ExecutionModeRejected {
                capability: request.capability,
                mode: profile.execution_mode,
            });
        }

        // The single resolver call of this dispatch, placed between the two
        // decisions it has to sit between. It is below the execution-mode check
        // because a `typed-pipeline` capability owns its own prompt
        // construction: that dispatch must not resolve conventions at all, and
        // returning above leaves nothing for a later branch to call. It is above
        // the provider lookup because the execution subject is chosen there, and
        // the resolution has to exist before whichever subject that is receives
        // its input.
        //
        // The query names the dispatched capability through the one conversion
        // the workspace has from a profile lookup key to a matching term. The
        // conversion is total, so no dispatch can fail for want of a
        // well-formed identifier once the request itself validated. The root is
        // the one this interactor was wired with, not one derived here.
        let conventions = self
            .conventions
            .resolve(ResolveConventionsQuery {
                capability: ConventionCapabilityId::from(&request.capability),
                project_root: self.project_root.clone(),
            })
            .map_err(|source| CapabilityExecError::ConventionResolutionFailed {
                capability: request.capability.clone(),
                source,
            })?;
        // Folded into the discipline rather than carried beside it: the routes
        // then share one text by construction, and the resolution is the only
        // thing folded in, so nothing but what the resolver returned can enter
        // the dispatched obligation.
        let discipline = discipline.with_conventions(&conventions);

        let provider = self
            .providers
            .iter()
            .find(|adapter| match &profile.provider {
                CapabilityProviderBinding::Standard(provider) => adapter.provider() == provider,
                CapabilityProviderBinding::CodexCustom(_) => {
                    adapter.provider() == &*CODEX_PROVIDER_NAME
                }
            })
            .ok_or_else(|| CapabilityExecError::UnsupportedProvider {
                provider: adapter_provider_name(&profile.provider),
            })?;

        provider.dispatch(&CapabilityDispatchRequest { request, profile, briefing, discipline })
    }
}

fn adapter_provider_name(binding: &CapabilityProviderBinding) -> ProviderName {
    match binding {
        CapabilityProviderBinding::Standard(provider) => provider.clone(),
        CapabilityProviderBinding::CodexCustom(_) => CODEX_PROVIDER_NAME.clone(),
    }
}

/// Errors that stop capability dispatch before a successful outcome.
#[derive(Debug, thiserror::Error)]
pub enum CapabilityExecError {
    /// Profile lookup or decoding failed for a capability.
    #[error("capability profile resolution failed for '{capability}': {detail}")]
    ProfileResolution {
        /// Capability whose profile could not be resolved.
        capability: CapabilityName,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
    /// A profile's execution category is not eligible for generic dispatch.
    #[error("execution mode rejected for capability '{capability}': {mode:?}")]
    ExecutionModeRejected {
        /// Capability whose execution category was rejected.
        capability: CapabilityName,
        /// Rejected execution category.
        mode: ExecutionMode,
    },
    /// A profile omitted the single model required for dispatch.
    #[error("capability profile has no model for '{capability}'")]
    ModelMissing {
        /// Capability whose profile omitted its model.
        capability: CapabilityName,
    },
    /// A provider-CLI profile omitted the effort required for dispatch.
    #[error("capability profile has no reasoning effort for '{0}'")]
    EffortMissing(CapabilityName),
    /// A provider has no supported generic-dispatch adapter.
    #[error("unsupported capability provider '{provider}'")]
    UnsupportedProvider {
        /// Unsupported provider identifier.
        provider: ProviderName,
    },
    /// Briefing or discipline source validation failed.
    #[error("capability source validation failed for '{path}': {detail}")]
    SourceValidation {
        /// Source path associated with the failed validation.
        path: CapabilityFilePath,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
    /// A provider-native adapter did not meet its preflight requirements.
    #[error(
        "capability adapter preflight failed for '{capability}' on provider '{provider}': {detail}"
    )]
    AdapterPreflight {
        /// Capability whose adapter could not pass preflight.
        capability: CapabilityName,
        /// Provider whose adapter failed preflight.
        provider: ProviderName,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
    /// A provider-native dispatch attempt failed.
    #[error("capability dispatch failed for provider '{provider}': {detail}")]
    DispatchFailed {
        /// Provider whose dispatch attempt failed.
        provider: ProviderName,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
    /// The dispatch preflight could not resolve the capability's conventions.
    ///
    /// The resolver rejection is carried as a typed source rather than a
    /// flattened summary, so the condition that stopped the dispatch survives
    /// the layer boundary and a caller can still match on it. This enum
    /// declares no `From` impl for it, keeping the wrap an explicit named site
    /// as the lifts inside [`ConventionResolveError`] are.
    #[error("capability convention resolution failed for '{capability}'")]
    ConventionResolutionFailed {
        /// Capability whose required conventions were being resolved.
        capability: CapabilityName,
        /// Resolver rejection this variant carries unchanged.
        source: ConventionResolveError,
    },
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::{
        BriefingText, CapabilityDispatchOutcome, CapabilityDispatchRequest, CapabilityExecError,
        CapabilityExecInteractor, CapabilityExecRequest, CapabilityExecService,
        CapabilityFailureDetail, CapabilityFilePath, CapabilityInputValidationError,
        CapabilityProfile, CapabilityProfilePort, CapabilityProviderBinding,
        CapabilityProviderPort, CapabilityResumeRequest, CapabilitySourcePort,
        ConventionResolveError, DisciplineText, ExecutionMode, ModelName, ModelProviderName,
        ProviderName, ReasoningEffort, TargetArtifactPath, TargetArtifactSet, TimeoutSeconds,
    };
    use crate::conventions_resolve::{
        ConventionCapabilityId, ConventionDocumentPath, ConventionResolution,
        ConventionResolveService, ResolveConventionsQuery,
    };
    use crate::dry_write_driver::CapabilityName;

    struct StaticProfilePort {
        profile: CapabilityProfile,
        calls: Arc<AtomicUsize>,
    }

    impl CapabilityProfilePort for StaticProfilePort {
        fn resolve(
            &self,
            _capability: &CapabilityName,
        ) -> Result<CapabilityProfile, CapabilityExecError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.profile.clone())
        }
    }

    struct StaticSourcePort {
        briefing: Result<BriefingText, CapabilityFailureDetail>,
        discipline: Result<DisciplineText, CapabilityFailureDetail>,
        briefing_path: CapabilityFilePath,
        calls: Arc<AtomicUsize>,
    }

    impl CapabilitySourcePort for StaticSourcePort {
        fn load_briefing(
            &self,
            path: &CapabilityFilePath,
        ) -> Result<BriefingText, CapabilityExecError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.briefing.clone().map_err(|detail| CapabilityExecError::SourceValidation {
                path: path.clone(),
                detail,
            })
        }

        fn load_discipline(&self) -> Result<DisciplineText, CapabilityExecError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.discipline.clone().map_err(|detail| CapabilityExecError::SourceValidation {
                path: self.briefing_path.clone(),
                detail,
            })
        }
    }

    struct RecordingProviderPort {
        provider: ProviderName,
        outcome: CapabilityDispatchOutcome,
        dispatches: Arc<Mutex<Vec<CapabilityDispatchRequest>>>,
    }

    impl CapabilityProviderPort for RecordingProviderPort {
        fn provider(&self) -> &ProviderName {
            &self.provider
        }

        fn dispatch(
            &self,
            request: &CapabilityDispatchRequest,
        ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
            self.dispatches.lock().expect("test dispatch recorder lock").push(request.clone());
            Ok(self.outcome.clone())
        }
    }

    struct PreflightFailingProviderPort {
        provider: ProviderName,
        dispatches: Arc<AtomicUsize>,
    }

    impl CapabilityProviderPort for PreflightFailingProviderPort {
        fn provider(&self) -> &ProviderName {
            &self.provider
        }

        fn dispatch(
            &self,
            request: &CapabilityDispatchRequest,
        ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
            self.dispatches.fetch_add(1, Ordering::SeqCst);
            Err(CapabilityExecError::AdapterPreflight {
                capability: request.request.capability.clone(),
                provider: self.provider.clone(),
                detail: CapabilityFailureDetail::new(
                    "adapter definition model does not match profile",
                ),
            })
        }
    }

    /// Convention resolver that answers with a fixed match set and keeps every
    /// query it was asked, so a test can count the calls of one dispatch and
    /// read back what the interactor put in them.
    struct RecordingConventionResolveService {
        documents: Vec<ConventionDocumentPath>,
        queries: Arc<Mutex<Vec<ResolveConventionsQuery>>>,
    }

    impl ConventionResolveService for RecordingConventionResolveService {
        fn resolve(
            &self,
            query: ResolveConventionsQuery,
        ) -> Result<ConventionResolution, ConventionResolveError> {
            self.queries.lock().expect("test resolver recorder lock").push(query);
            Ok(ConventionResolution::from_matches(self.documents.clone()))
        }
    }

    /// Convention resolver that meets one of `AC-07`'s fail-closed conditions.
    ///
    /// It counts its calls as well, so a test can tell a dispatch that failed
    /// the preflight from one that never reached it.
    struct FailingConventionResolveService {
        calls: Arc<AtomicUsize>,
    }

    impl ConventionResolveService for FailingConventionResolveService {
        fn resolve(
            &self,
            _query: ResolveConventionsQuery,
        ) -> Result<ConventionResolution, ConventionResolveError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(ConventionResolveError::ConventionRootUnlistable {
                root: PathBuf::from("knowledge/conventions"),
                detail: CapabilityFailureDetail::new("permission denied"),
            })
        }
    }

    fn recorder() -> Arc<Mutex<Vec<ResolveConventionsQuery>>> {
        Arc::new(Mutex::new(Vec::new()))
    }

    fn resolver(
        queries: &Arc<Mutex<Vec<ResolveConventionsQuery>>>,
        documents: &[&str],
    ) -> Result<Arc<RecordingConventionResolveService>, Box<dyn std::error::Error>> {
        let documents = documents
            .iter()
            .map(|path| convention(path))
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
        Ok(Arc::new(RecordingConventionResolveService { documents, queries: queries.clone() }))
    }

    fn request() -> Result<CapabilityExecRequest, Box<dyn std::error::Error>> {
        Ok(CapabilityExecRequest {
            capability: CapabilityName::try_new("implementer")?,
            host: Some(ProviderName::try_new("codex")?),
            briefing_file: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
            timeout: None,
            resume: CapabilityResumeRequest::Fresh,
        })
    }

    fn convention(path: &str) -> Result<ConventionDocumentPath, Box<dyn std::error::Error>> {
        Ok(ConventionDocumentPath::try_new(PathBuf::from(path))?)
    }

    fn profile(mode: ExecutionMode) -> Result<CapabilityProfile, CapabilityInputValidationError> {
        Ok(CapabilityProfile {
            provider: CapabilityProviderBinding::Standard(ProviderName::try_new("codex")?),
            model: ModelName::try_new("gpt-5")?,
            effort: ReasoningEffort::High,
            execution_mode: mode,
        })
    }

    #[test]
    fn test_provider_name_whitespace_rejected() {
        assert!(matches!(
            ProviderName::try_new(" \t\n "),
            Err(CapabilityInputValidationError::EmptyProviderName)
        ));
    }

    #[test]
    fn test_provider_name_valid_value_preserved() {
        assert!(matches!(
            ProviderName::try_new("codex"),
            Ok(provider) if provider.as_str() == "codex" && provider.to_string() == "codex"
        ));
    }

    #[test]
    fn test_model_provider_name_whitespace_rejected() {
        assert!(matches!(
            ModelProviderName::try_new(" \t\n "),
            Err(CapabilityInputValidationError::EmptyModelProviderName)
        ));
    }

    #[test]
    fn test_model_provider_name_valid_value_preserved() {
        assert!(matches!(
            ModelProviderName::try_new("deepseek"),
            Ok(provider) if provider.as_str() == "deepseek" && provider.to_string() == "deepseek"
        ));
    }

    #[test]
    fn test_capability_provider_binding_standard_retains_provider()
    -> Result<(), Box<dyn std::error::Error>> {
        let provider = ProviderName::try_new("claude")?;
        let binding = CapabilityProviderBinding::Standard(provider.clone());

        assert_eq!(binding, CapabilityProviderBinding::Standard(provider));
        Ok(())
    }

    #[test]
    fn test_capability_provider_binding_codex_custom_retains_model_provider()
    -> Result<(), Box<dyn std::error::Error>> {
        let model_provider = ModelProviderName::try_new("qwen")?;
        let binding = CapabilityProviderBinding::CodexCustom(model_provider.clone());

        assert_eq!(binding, CapabilityProviderBinding::CodexCustom(model_provider));
        Ok(())
    }

    #[test]
    fn test_model_name_whitespace_rejected() {
        assert!(matches!(
            ModelName::try_new(" "),
            Err(CapabilityInputValidationError::EmptyModelName)
        ));
    }

    #[test]
    fn test_model_name_valid_value_preserved() {
        assert!(matches!(
            ModelName::try_new("gpt-5"),
            Ok(model) if model.as_str() == "gpt-5" && model.to_string() == "gpt-5"
        ));
    }

    #[test]
    fn test_capability_file_path_empty_path_rejected() {
        assert!(matches!(
            CapabilityFilePath::try_new(PathBuf::new()),
            Err(CapabilityInputValidationError::EmptyFilePath)
        ));
    }

    #[test]
    fn test_capability_file_path_valid_path_preserved() {
        assert!(matches!(
            CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md")),
            Ok(path) if path.as_path() == "tmp/briefing.md"
                && path.to_string() == "tmp/briefing.md"
        ));
    }

    #[test]
    fn test_capability_file_path_absolute_and_traversal_paths_rejected() {
        for path in [PathBuf::from("/outside/briefing.md"), PathBuf::from("tmp/../briefing.md")] {
            assert!(matches!(
                CapabilityFilePath::try_new(path),
                Err(CapabilityInputValidationError::InvalidFilePath)
            ));
        }
    }

    #[test]
    fn test_resume_target_inputs_rejected_with_typed_variants_before_dispatch() {
        // Resume-option targets are validated by this error type before any
        // dispatch or cache-key derivation: an invalid target can never name a
        // continuation session outside the requested track/capability scope.
        assert!(matches!(
            TargetArtifactPath::try_new(PathBuf::from("../outside/spec.json")),
            Err(CapabilityInputValidationError::InvalidFilePath)
        ));
        assert!(matches!(
            TargetArtifactPath::try_new(PathBuf::from("/absolute/spec.json")),
            Err(CapabilityInputValidationError::InvalidFilePath)
        ));
        assert!(matches!(
            TargetArtifactPath::try_new(PathBuf::new()),
            Err(CapabilityInputValidationError::EmptyFilePath)
        ));
        assert!(matches!(
            TargetArtifactSet::try_new(vec![]),
            Err(CapabilityInputValidationError::EmptyTargetArtifactSet)
        ));
    }

    #[test]
    fn test_briefing_text_whitespace_rejected() {
        assert!(matches!(
            BriefingText::try_new("\n  \t".to_owned()),
            Err(CapabilityInputValidationError::EmptyContent)
        ));
    }

    #[test]
    fn test_briefing_text_valid_content_preserved() {
        assert!(matches!(
            BriefingText::try_new("Implement T001.".to_owned()),
            Ok(briefing) if briefing.as_str() == "Implement T001."
        ));
    }

    #[test]
    fn test_discipline_text_whitespace_rejected() {
        assert!(matches!(
            DisciplineText::try_new("\n  \t".to_owned()),
            Err(CapabilityInputValidationError::EmptyContent)
        ));
    }

    #[test]
    fn test_discipline_text_valid_content_preserved() {
        assert!(matches!(
            DisciplineText::try_new("Do not stage changes.".to_owned()),
            Ok(discipline) if discipline.as_str() == "Do not stage changes."
        ));
    }

    #[test]
    fn test_discipline_text_with_conventions_appends_every_resolved_document_path()
    -> Result<(), Box<dyn std::error::Error>> {
        let discipline = DisciplineText::try_new("Do not stage changes.".to_owned())?;
        let resolution = ConventionResolution::from_matches(vec![
            convention("knowledge/conventions/coding-principles.md")?,
            convention("knowledge/conventions/rust/testing.md")?,
        ]);

        let composed = discipline.with_conventions(&resolution);

        assert!(
            composed.as_str().starts_with("Do not stage changes."),
            "the discipline the source adapter loaded is carried through unchanged"
        );
        for document in resolution.documents() {
            assert!(
                composed.as_str().contains(&document.to_string()),
                "every resolved document reaches the dispatched capability, not a selection"
            );
        }
        Ok(())
    }

    #[test]
    fn test_discipline_text_with_conventions_names_only_resolved_paths_in_resolution_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let discipline = DisciplineText::try_new("Do not stage changes.".to_owned())?;
        let resolution = ConventionResolution::from_matches(vec![
            convention("knowledge/conventions/rust/testing.md")?,
            convention("knowledge/conventions/coding-principles.md")?,
        ]);

        let composed = discipline.with_conventions(&resolution);

        let listed: Vec<&str> =
            composed.as_str().lines().filter_map(|line| line.strip_prefix("- ")).collect();
        let resolved: Vec<String> =
            resolution.documents().iter().map(ToString::to_string).collect();
        assert_eq!(
            listed, resolved,
            "the rendered obligation names the resolver's own document paths in its order, so \
             no fixed convention filename can enter the text here"
        );
        Ok(())
    }

    #[test]
    fn test_discipline_text_with_conventions_renders_an_empty_resolution_without_an_obligation()
    -> Result<(), Box<dyn std::error::Error>> {
        let discipline = DisciplineText::try_new("Do not stage changes.".to_owned())?;

        let composed = discipline.with_conventions(&ConventionResolution::default());

        assert!(composed.as_str().starts_with("Do not stage changes."));
        assert!(
            !composed.as_str().contains("knowledge/conventions"),
            "a resolution that matched nothing names no document"
        );
        assert!(
            composed.as_str().lines().all(|line| !line.starts_with("- ")),
            "a resolution that matched nothing lists nothing to read"
        );
        assert!(
            composed.as_str().contains("returned zero"),
            "the empty case states that resolution ran and returned nothing, so it cannot be \
             read as conventions having never been resolved"
        );
        Ok(())
    }

    #[test]
    fn test_discipline_text_with_conventions_composes_one_value_from_a_discipline_and_a_resolution()
    -> Result<(), Box<dyn std::error::Error>> {
        // The composition depends on nothing but the two values handed to it, so
        // two dispatch routes handed the same discipline and the same resolution
        // have no way to end up carrying different obligation texts.
        let discipline = DisciplineText::try_new("Do not stage changes.".to_owned())?;
        let resolution = ConventionResolution::from_matches(vec![convention(
            "knowledge/conventions/coding-principles.md",
        )?]);

        assert_eq!(
            discipline.with_conventions(&resolution),
            discipline.with_conventions(&resolution)
        );
        Ok(())
    }

    #[test]
    fn test_capability_exec_error_convention_resolution_failure_carries_the_typed_resolver_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let error = CapabilityExecError::ConventionResolutionFailed {
            capability: CapabilityName::try_new("implementer")?,
            source: ConventionResolveError::EmptyCapabilityId {
                document: convention("knowledge/conventions/testing.md")?,
            },
        };

        assert!(
            error.to_string().contains("implementer"),
            "the failure names the capability whose conventions were being resolved"
        );
        let source = std::error::Error::source(&error)
            .and_then(|source| source.downcast_ref::<ConventionResolveError>())
            .expect("the resolver rejection is carried as a typed source");
        assert!(
            matches!(
                source,
                ConventionResolveError::EmptyCapabilityId { document }
                    if document.as_path() == Path::new("knowledge/conventions/testing.md")
            ),
            "the failing condition and its payload survive the wrap instead of being flattened \
             into prose at the boundary"
        );
        Ok(())
    }

    #[test]
    fn test_capability_profile_valid_values_retained() -> Result<(), Box<dyn std::error::Error>> {
        let profile = CapabilityProfile {
            provider: CapabilityProviderBinding::Standard(ProviderName::try_new("codex")?),
            model: ModelName::try_new("gpt-5")?,
            effort: ReasoningEffort::High,
            execution_mode: ExecutionMode::OrchestratorOutput,
        };

        assert!(matches!(
            profile.provider,
            CapabilityProviderBinding::Standard(ref provider) if provider.as_str() == "codex"
        ));
        assert_eq!(profile.model.as_str(), "gpt-5");
        assert_eq!(profile.effort, ReasoningEffort::High);
        assert_eq!(profile.execution_mode, ExecutionMode::OrchestratorOutput);
        Ok(())
    }

    #[test]
    fn test_capability_exec_codex_custom_binding_selects_codex_adapter()
    -> Result<(), Box<dyn std::error::Error>> {
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = dispatching_interactor(
            CapabilityProfile {
                provider: CapabilityProviderBinding::CodexCustom(ModelProviderName::try_new(
                    "deepseek",
                )?),
                model: ModelName::try_new("gpt-5")?,
                effort: ReasoningEffort::High,
                execution_mode: ExecutionMode::OrchestratorOutput,
            },
            resolver(&recorder(), &[])?,
            recording_provider(&dispatches)?,
        )?;

        let outcome = interactor.execute(request()?)?;

        assert!(matches!(
            outcome,
            CapabilityDispatchOutcome::Executed { ref provider, exit_code: 0 }
                if provider.as_str() == "codex"
        ));
        let recorded = dispatches.lock().expect("test dispatch recorder lock");
        assert!(matches!(
            &recorded[0].profile.provider,
            CapabilityProviderBinding::CodexCustom(provider) if provider.as_str() == "deepseek"
        ));
        Ok(())
    }

    #[test]
    fn test_capability_exec_request_shared_capability_name_retained()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = CapabilityExecRequest {
            capability: CapabilityName::try_new("implementer")?,
            host: Some(ProviderName::try_new("codex")?),
            briefing_file: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
            timeout: Some(TimeoutSeconds::try_new(1800)?),
            resume: CapabilityResumeRequest::Fresh,
        };

        assert_eq!(request.capability.as_str(), "implementer");
        assert_eq!(request.host.as_ref().map(ProviderName::as_str), Some("codex"));
        assert_eq!(request.briefing_file.as_path(), PathBuf::from("tmp/briefing.md"));
        assert_eq!(request.timeout.map(|timeout| timeout.as_secs()), Some(1800));
        Ok(())
    }

    #[test]
    fn test_timeout_seconds_zero_rejected() {
        assert!(matches!(
            TimeoutSeconds::try_new(0),
            Err(CapabilityInputValidationError::ZeroTimeoutSeconds)
        ));
    }

    #[test]
    fn test_timeout_seconds_positive_value_preserved() {
        assert!(matches!(
            TimeoutSeconds::try_new(600),
            Ok(timeout) if timeout.as_secs() == 600
        ));
    }

    #[test]
    fn test_execution_mode_distinct_variants_retained() {
        assert_ne!(ExecutionMode::OrchestratorOutput, ExecutionMode::TypedPipeline);
    }

    #[test]
    fn test_capability_failure_detail_input_preserved() {
        let detail = CapabilityFailureDetail::new("definition is missing");

        assert_eq!(detail.as_str(), "definition is missing");
        assert_eq!(detail.to_string(), "definition is missing");
    }

    #[test]
    fn test_capability_exec_error_all_variants_render_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let detail = CapabilityFailureDetail::new("definition is missing");
        let variants = [
            CapabilityExecError::ProfileResolution {
                capability: CapabilityName::try_new("implementer")?,
                detail: detail.clone(),
            },
            CapabilityExecError::ExecutionModeRejected {
                capability: CapabilityName::try_new("implementer")?,
                mode: ExecutionMode::TypedPipeline,
            },
            CapabilityExecError::ModelMissing {
                capability: CapabilityName::try_new("implementer")?,
            },
            CapabilityExecError::UnsupportedProvider { provider: ProviderName::try_new("codex")? },
            CapabilityExecError::SourceValidation {
                path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                detail: detail.clone(),
            },
            CapabilityExecError::AdapterPreflight {
                capability: CapabilityName::try_new("implementer")?,
                provider: ProviderName::try_new("codex")?,
                detail: detail.clone(),
            },
            CapabilityExecError::DispatchFailed {
                provider: ProviderName::try_new("codex")?,
                detail,
            },
            CapabilityExecError::ConventionResolutionFailed {
                capability: CapabilityName::try_new("implementer")?,
                source: ConventionResolveError::EmptyCapabilityId {
                    document: convention("knowledge/conventions/testing.md")?,
                },
            },
        ];

        for error in variants {
            assert!(!error.to_string().is_empty());
        }
        Ok(())
    }

    #[test]
    fn test_capability_exec_valid_orchestrator_output_dispatches_validated_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let source_calls = Arc::new(AtomicUsize::new(0));
        let profile_calls = Arc::new(AtomicUsize::new(0));
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let provider = ProviderName::try_new("codex")?;
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: profile(ExecutionMode::OrchestratorOutput)?,
                calls: profile_calls.clone(),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new("perform the task".to_owned())?),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: source_calls.clone(),
            }),
            resolver(&recorder(), &[])?,
            vec![Arc::new(RecordingProviderPort {
                provider: provider.clone(),
                outcome: CapabilityDispatchOutcome::Executed { provider, exit_code: 0 },
                dispatches: dispatches.clone(),
            })],
            PathBuf::from(PROJECT_ROOT),
        );

        let outcome = interactor.execute(request()?)?;

        assert!(matches!(outcome, CapabilityDispatchOutcome::Executed { exit_code: 0, .. }));
        assert_eq!(source_calls.load(Ordering::SeqCst), 2);
        assert_eq!(profile_calls.load(Ordering::SeqCst), 1);
        let recorded = dispatches.lock().expect("test dispatch recorder lock");
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].request.capability.as_str(), "implementer");
        assert_eq!(recorded[0].request.host.as_ref().map(ProviderName::as_str), Some("codex"));
        assert!(matches!(
            &recorded[0].profile.provider,
            CapabilityProviderBinding::Standard(provider) if provider.as_str() == "codex"
        ));
        assert_eq!(recorded[0].profile.model.as_str(), "gpt-5");
        assert_eq!(recorded[0].briefing.as_str(), "perform the task");
        assert_eq!(
            recorded[0].discipline,
            DisciplineText::try_new("no direct git writes".to_owned())?
                .with_conventions(&ConventionResolution::default()),
            "the loaded discipline reaches the provider carrying this dispatch's resolution, \
             which here matched nothing"
        );
        Ok(())
    }

    #[test]
    fn test_capability_exec_omitted_host_dispatches_to_requested_capability_provider()
    -> Result<(), Box<dyn std::error::Error>> {
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let provider = ProviderName::try_new("claude")?;
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: CapabilityProfile {
                    provider: CapabilityProviderBinding::Standard(provider.clone()),
                    model: ModelName::try_new("claude-opus")?,
                    effort: ReasoningEffort::High,
                    execution_mode: ExecutionMode::OrchestratorOutput,
                },
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new("perform the task".to_owned())?),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            resolver(&recorder(), &[])?,
            vec![Arc::new(RecordingProviderPort {
                provider: provider.clone(),
                outcome: CapabilityDispatchOutcome::Executed { provider, exit_code: 0 },
                dispatches: dispatches.clone(),
            })],
            PathBuf::from(PROJECT_ROOT),
        );

        let mut request = request()?;
        request.host = None;
        let outcome = interactor.execute(request)?;

        assert!(matches!(
            outcome,
            CapabilityDispatchOutcome::Executed { ref provider, exit_code: 0 }
                if provider.as_str() == "claude"
        ));
        let recorded = dispatches.lock().expect("test dispatch recorder lock");
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].request.host, None);
        assert!(matches!(
            &recorded[0].profile.provider,
            CapabilityProviderBinding::Standard(provider) if provider.as_str() == "claude"
        ));
        assert_eq!(recorded[0].profile.model.as_str(), "claude-opus");
        Ok(())
    }

    #[test]
    fn test_capability_exec_preserves_explicit_mismatched_host_for_selected_provider()
    -> Result<(), Box<dyn std::error::Error>> {
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let provider = ProviderName::try_new("claude")?;
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: CapabilityProfile {
                    provider: CapabilityProviderBinding::Standard(provider.clone()),
                    model: ModelName::try_new("claude-opus")?,
                    effort: ReasoningEffort::High,
                    execution_mode: ExecutionMode::OrchestratorOutput,
                },
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new("perform the task".to_owned())?),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            resolver(&recorder(), &[])?,
            vec![Arc::new(RecordingProviderPort {
                provider: provider.clone(),
                outcome: CapabilityDispatchOutcome::Executed { provider, exit_code: 0 },
                dispatches: dispatches.clone(),
            })],
            PathBuf::from(PROJECT_ROOT),
        );

        let outcome = interactor.execute(request()?)?;

        assert!(matches!(outcome, CapabilityDispatchOutcome::Executed { exit_code: 0, .. }));
        let recorded = dispatches.lock().expect("test dispatch recorder lock");
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].request.host.as_ref().map(ProviderName::as_str), Some("codex"));
        assert!(matches!(
            &recorded[0].profile.provider,
            CapabilityProviderBinding::Standard(provider) if provider.as_str() == "claude"
        ));
        Ok(())
    }

    #[test]
    fn test_capability_exec_source_failure_prevents_profile_and_provider_dispatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let source_calls = Arc::new(AtomicUsize::new(0));
        let profile_calls = Arc::new(AtomicUsize::new(0));
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: profile(ExecutionMode::OrchestratorOutput)?,
                calls: profile_calls.clone(),
            }),
            Arc::new(StaticSourcePort {
                briefing: Err(CapabilityFailureDetail::new("not readable")),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: source_calls.clone(),
            }),
            resolver(&recorder(), &[])?,
            vec![Arc::new(RecordingProviderPort {
                provider: ProviderName::try_new("codex")?,
                outcome: CapabilityDispatchOutcome::Executed {
                    provider: ProviderName::try_new("codex")?,
                    exit_code: 0,
                },
                dispatches: dispatches.clone(),
            })],
            PathBuf::from(PROJECT_ROOT),
        );

        assert!(matches!(
            interactor.execute(request()?),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
        assert_eq!(source_calls.load(Ordering::SeqCst), 1);
        assert_eq!(profile_calls.load(Ordering::SeqCst), 0);
        assert!(dispatches.lock().expect("test dispatch recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_capability_exec_typed_pipeline_rejected_before_provider_dispatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: profile(ExecutionMode::TypedPipeline)?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new("perform the task".to_owned())?),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            resolver(&recorder(), &[])?,
            vec![Arc::new(RecordingProviderPort {
                provider: ProviderName::try_new("codex")?,
                outcome: CapabilityDispatchOutcome::Executed {
                    provider: ProviderName::try_new("codex")?,
                    exit_code: 0,
                },
                dispatches: dispatches.clone(),
            })],
            PathBuf::from(PROJECT_ROOT),
        );

        assert!(matches!(
            interactor.execute(request()?),
            Err(CapabilityExecError::ExecutionModeRejected { .. })
        ));
        assert!(dispatches.lock().expect("test dispatch recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_capability_exec_unsupported_provider_rejected_before_provider_dispatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: CapabilityProfile {
                    provider: CapabilityProviderBinding::Standard(ProviderName::try_new(
                        "unsupported-provider",
                    )?),
                    model: ModelName::try_new("model-x")?,
                    effort: ReasoningEffort::High,
                    execution_mode: ExecutionMode::OrchestratorOutput,
                },
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new("perform the task".to_owned())?),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            resolver(&recorder(), &[])?,
            vec![Arc::new(RecordingProviderPort {
                provider: ProviderName::try_new("codex")?,
                outcome: CapabilityDispatchOutcome::Executed {
                    provider: ProviderName::try_new("codex")?,
                    exit_code: 0,
                },
                dispatches: dispatches.clone(),
            })],
            PathBuf::from(PROJECT_ROOT),
        );

        assert!(matches!(
            interactor.execute(request()?),
            Err(CapabilityExecError::UnsupportedProvider { .. })
        ));
        assert!(dispatches.lock().expect("test dispatch recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_capability_exec_adapter_preflight_failure_returns_without_successful_outcome()
    -> Result<(), Box<dyn std::error::Error>> {
        let dispatches = Arc::new(AtomicUsize::new(0));
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: profile(ExecutionMode::OrchestratorOutput)?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new("perform the task".to_owned())?),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            resolver(&recorder(), &[])?,
            vec![Arc::new(PreflightFailingProviderPort {
                provider: ProviderName::try_new("codex")?,
                dispatches: dispatches.clone(),
            })],
            PathBuf::from(PROJECT_ROOT),
        );

        assert!(matches!(
            interactor.execute(request()?),
            Err(CapabilityExecError::AdapterPreflight { .. })
        ));
        assert_eq!(dispatches.load(Ordering::SeqCst), 1);
        Ok(())
    }

    /// Discipline text the source port hands every dispatch below, before the
    /// interactor folds this dispatch's resolution into it.
    const BASE_DISCIPLINE: &str = "no direct git writes";

    /// Project root the interactors below are wired with.
    ///
    /// Deliberately not `.`: a root the interactor could have derived from its
    /// own process would be indistinguishable from the wired one in an
    /// assertion, and this is the value a composition root discovers and hands
    /// in.
    const PROJECT_ROOT: &str = "/srv/consumer-project";

    /// Provider adapter that returns the in-host delegation outcome, carrying
    /// the discipline it was dispatched with into the payload the host reads.
    struct DelegatingProviderPort {
        provider: ProviderName,
    }

    impl CapabilityProviderPort for DelegatingProviderPort {
        fn provider(&self) -> &ProviderName {
            &self.provider
        }

        fn dispatch(
            &self,
            request: &CapabilityDispatchRequest,
        ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
            Ok(CapabilityDispatchOutcome::DelegateInHost {
                capability: request.request.capability.clone(),
                briefing_file: request.request.briefing_file.clone(),
                discipline: request.discipline.clone(),
            })
        }
    }

    fn dispatching_interactor(
        profile: CapabilityProfile,
        conventions: Arc<dyn ConventionResolveService>,
        providers: Vec<Arc<dyn CapabilityProviderPort>>,
    ) -> Result<CapabilityExecInteractor, Box<dyn std::error::Error>> {
        Ok(CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort { profile, calls: Arc::new(AtomicUsize::new(0)) }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new("perform the task".to_owned())?),
                discipline: Ok(DisciplineText::try_new(BASE_DISCIPLINE.to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            conventions,
            providers,
            PathBuf::from(PROJECT_ROOT),
        ))
    }

    fn recording_provider(
        dispatches: &Arc<Mutex<Vec<CapabilityDispatchRequest>>>,
    ) -> Result<Vec<Arc<dyn CapabilityProviderPort>>, Box<dyn std::error::Error>> {
        let provider = ProviderName::try_new("codex")?;
        Ok(vec![Arc::new(RecordingProviderPort {
            provider: provider.clone(),
            outcome: CapabilityDispatchOutcome::Executed { provider, exit_code: 0 },
            dispatches: dispatches.clone(),
        })])
    }

    fn listed_documents(discipline: &DisciplineText) -> Vec<&str> {
        discipline.as_str().lines().filter_map(|line| line.strip_prefix("- ")).collect()
    }

    #[test]
    fn test_capability_exec_resolves_conventions_once_and_dispatches_the_composed_discipline()
    -> Result<(), Box<dyn std::error::Error>> {
        let queries = recorder();
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = dispatching_interactor(
            profile(ExecutionMode::OrchestratorOutput)?,
            resolver(
                &queries,
                &[
                    "knowledge/conventions/coding-principles.md",
                    "knowledge/conventions/rust/testing.md",
                ],
            )?,
            recording_provider(&dispatches)?,
        )?;

        interactor.execute(request()?)?;

        assert_eq!(
            queries.lock().expect("test resolver recorder lock").len(),
            1,
            "one dispatch asks the resolver once; a second call would mean a route resolving \
             again below the seam"
        );
        let recorded = dispatches.lock().expect("test dispatch recorder lock");
        assert_eq!(
            recorded[0].discipline,
            DisciplineText::try_new(BASE_DISCIPLINE.to_owned())?.with_conventions(
                &ConventionResolution::from_matches(vec![
                    convention("knowledge/conventions/coding-principles.md")?,
                    convention("knowledge/conventions/rust/testing.md")?,
                ])
            ),
            "what reaches the provider is the loaded discipline with this dispatch's resolution \
             folded in — not the discipline alone, and not a text composed some other way"
        );
        Ok(())
    }

    #[test]
    fn test_capability_exec_query_names_the_dispatched_capability_as_a_matching_term()
    -> Result<(), Box<dyn std::error::Error>> {
        let queries = recorder();
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = dispatching_interactor(
            profile(ExecutionMode::OrchestratorOutput)?,
            resolver(&queries, &[])?,
            recording_provider(&dispatches)?,
        )?;

        interactor.execute(request()?)?;

        let asked = queries.lock().expect("test resolver recorder lock");
        // The request's capability is the `implementer` profile lookup key; the
        // query has to carry that same identifier as a matching term, since a
        // dispatch resolving under any other name would hand the capability
        // documents that were written for something else.
        assert_eq!(
            asked[0].capability,
            ConventionCapabilityId::from(&CapabilityName::try_new("implementer")?)
        );
        // The scan root is the one this interactor was wired with. A root
        // derived inside `execute` from the running process would put the test
        // runner's working directory here instead, and a dispatch issued
        // anywhere but a repository root would then resolve against a tree that
        // holds no conventions and report that the capability requires none.
        assert_eq!(asked[0].project_root, PathBuf::from(PROJECT_ROOT));
        Ok(())
    }

    #[test]
    fn test_capability_exec_resolves_conventions_before_the_execution_subject_is_chosen()
    -> Result<(), Box<dyn std::error::Error>> {
        let queries = recorder();
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        // The profile names a provider no injected adapter implements, so
        // choosing the execution subject is the step that fails here.
        let interactor = dispatching_interactor(
            CapabilityProfile {
                provider: CapabilityProviderBinding::Standard(ProviderName::try_new(
                    "unsupported-provider",
                )?),
                model: ModelName::try_new("model-x")?,
                effort: ReasoningEffort::High,
                execution_mode: ExecutionMode::OrchestratorOutput,
            },
            resolver(&queries, &["knowledge/conventions/coding-principles.md"])?,
            recording_provider(&dispatches)?,
        )?;

        assert!(matches!(
            interactor.execute(request()?),
            Err(CapabilityExecError::UnsupportedProvider { .. })
        ));

        // The resolution exists even though no subject was ever selected, which
        // is only possible if it was taken before that selection. Were the
        // resolver called after the subject was chosen, this dispatch would
        // have returned without asking it at all.
        assert_eq!(queries.lock().expect("test resolver recorder lock").len(), 1);
        Ok(())
    }

    #[test]
    fn test_capability_exec_typed_pipeline_dispatch_resolves_no_conventions()
    -> Result<(), Box<dyn std::error::Error>> {
        let queries = recorder();
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = dispatching_interactor(
            profile(ExecutionMode::TypedPipeline)?,
            resolver(&queries, &["knowledge/conventions/coding-principles.md"])?,
            recording_provider(&dispatches)?,
        )?;

        assert!(matches!(
            interactor.execute(request()?),
            Err(CapabilityExecError::ExecutionModeRejected { .. })
        ));

        // A `typed-pipeline` capability builds its own prompt through its
        // dedicated CLI, so this dispatcher injects nothing into it. The
        // resolver is never asked — not asked and then discarded — because the
        // rejection returns above the only call site.
        assert!(queries.lock().expect("test resolver recorder lock").is_empty());
        assert!(dispatches.lock().expect("test dispatch recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_capability_exec_convention_resolution_failure_stops_before_the_provider()
    -> Result<(), Box<dyn std::error::Error>> {
        let calls = Arc::new(AtomicUsize::new(0));
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = dispatching_interactor(
            profile(ExecutionMode::OrchestratorOutput)?,
            Arc::new(FailingConventionResolveService { calls: calls.clone() }),
            recording_provider(&dispatches)?,
        )?;

        let error = match interactor.execute(request()?) {
            Ok(_) => return Err("a failed preflight must not produce a dispatch outcome".into()),
            Err(error) => error,
        };

        assert!(
            matches!(
                error,
                CapabilityExecError::ConventionResolutionFailed { ref capability, source: ConventionResolveError::ConventionRootUnlistable { .. } }
                    if capability.as_str() == "implementer"
            ),
            "the preflight rejection reaches the caller as the resolver's own condition, named \
             against the capability being dispatched"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            dispatches.lock().expect("test dispatch recorder lock").is_empty(),
            "a capability whose conventions could not be resolved is never dispatched, so it \
             cannot run under an obligation list that was never established"
        );
        Ok(())
    }

    /// Track artifact the trap fixture writes under the project root, carrying
    /// a `convention_refs` block. Repo-relative, because that is how a resume
    /// target names a track artifact and how one sits under a project root.
    const TRAP_TRACK_ARTIFACT: &str = "track/items/trap-track/spec.json";

    /// Convention path declared by the trap track artifact's `convention_refs`.
    const TRAP_TRACK_DOCUMENT: &str = "knowledge/conventions/trap-track-declared.md";

    /// Convention path declared in the briefing the source port loads.
    const TRAP_BRIEFING_DOCUMENT: &str = "knowledge/conventions/trap-briefing-declared.md";

    /// Convention document present in the project root's own
    /// `knowledge/conventions/` tree but absent from the resolver's answer.
    const TRAP_ON_DISK_DOCUMENT: &str = "knowledge/conventions/trap-on-disk.md";

    /// The one document the resolver returns for the trapped dispatch.
    const RESOLVED_DOCUMENT: &str = "knowledge/conventions/coding-principles.md";

    /// Builds a project root in which a competing convention declaration is
    /// genuinely available: a track artifact whose `convention_refs` names one
    /// document, and a convention tree holding another. Both are real files
    /// under the root the interactor is wired with, so a dispatcher that read
    /// either would find something to read.
    fn trap_project_root() -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;

        let artifact = root.path().join(TRAP_TRACK_ARTIFACT);
        std::fs::create_dir_all(
            artifact.parent().ok_or("the trap artifact path has a parent directory")?,
        )?;
        std::fs::write(
            &artifact,
            format!(
                r#"{{"constraints":[{{"id":"CO-01","text":"trap","convention_refs":[{{"file":"{TRAP_TRACK_DOCUMENT}","anchor":"intro"}}]}}]}}"#
            ),
        )?;

        let document = root.path().join(TRAP_ON_DISK_DOCUMENT);
        std::fs::create_dir_all(
            document.parent().ok_or("the trap document path has a parent directory")?,
        )?;
        std::fs::write(&document, "---\nrequired_for: [implementer]\n---\n\nTrap.\n")?;

        Ok(root)
    }

    #[test]
    fn test_capability_exec_injects_the_resolved_documents_and_nothing_else()
    -> Result<(), Box<dyn std::error::Error>> {
        // The exclusion this establishes is only worth something if the thing
        // excluded is present, so every input this interactor can reach carries
        // a competing convention declaration that the resolver does not return:
        // `project_root` is a real tree holding both a track artifact with a
        // `convention_refs` block and a `knowledge/conventions/` document, the
        // briefing text declares a third path, and the request resumes against
        // that same track artifact so its path is in the request as well.
        //
        // The briefing carrier is the one that makes this test load-bearing.
        // Reaching the two on-disk declarations requires `std::fs`, which
        // `verify usecase-purity` already rejects in this crate's production
        // code, so a dispatcher that read them would be stopped by that gate
        // whatever this test asserted. The briefing arrives as an owned
        // `BriefingText`: a dispatcher that folded the convention paths written
        // in it into the obligation would perform no I/O, pass every other gate
        // in the repository, and violate AC-12 — and nothing but the assertion
        // below would see it.
        let root = trap_project_root()?;
        let queries = recorder();
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: profile(ExecutionMode::OrchestratorOutput)?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new(format!(
                    "perform the task\n\nconvention_refs:\n- {TRAP_BRIEFING_DOCUMENT}\n"
                ))?),
                discipline: Ok(DisciplineText::try_new(BASE_DISCIPLINE.to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            resolver(&queries, &[RESOLVED_DOCUMENT])?,
            recording_provider(&dispatches)?,
            root.path().to_path_buf(),
        );

        interactor.execute(CapabilityExecRequest {
            resume: CapabilityResumeRequest::Resume(TargetArtifactSet::try_new(vec![
                TargetArtifactPath::try_new(PathBuf::from(TRAP_TRACK_ARTIFACT))?,
            ])?),
            ..request()?
        })?;

        let recorded = dispatches.lock().expect("test dispatch recorder lock");
        let dispatched = &recorded[0].discipline;
        // The dispatched obligation lists exactly the documents this dispatch's
        // resolver returned. Any convention the dispatcher had added from
        // somewhere else — the track artifact's recorded references, the
        // briefing's own declaration, the convention tree under the root it was
        // handed — would appear here as an extra entry, and any it dropped
        // would leave one missing.
        assert_eq!(listed_documents(dispatched), vec![RESOLVED_DOCUMENT]);
        for trap in [TRAP_TRACK_DOCUMENT, TRAP_BRIEFING_DOCUMENT, TRAP_ON_DISK_DOCUMENT] {
            assert!(
                !dispatched.as_str().contains(trap),
                "a convention declaration available to this dispatch but absent from the \
                 resolver's answer reaches the dispatched capability nowhere in its text, not \
                 merely outside the list"
            );
        }
        Ok(())
    }

    #[test]
    fn test_capability_exec_delegate_in_host_payload_carries_the_resolved_obligation()
    -> Result<(), Box<dyn std::error::Error>> {
        let queries = recorder();
        let provider = ProviderName::try_new("claude")?;
        let interactor = dispatching_interactor(
            CapabilityProfile {
                provider: CapabilityProviderBinding::Standard(provider.clone()),
                model: ModelName::try_new("claude-opus")?,
                effort: ReasoningEffort::High,
                execution_mode: ExecutionMode::OrchestratorOutput,
            },
            resolver(&queries, &[RESOLVED_DOCUMENT])?,
            vec![Arc::new(DelegatingProviderPort { provider })],
        )?;

        let outcome = interactor.execute(request()?)?;

        // The host reads this payload instead of a provider subprocess reading
        // an execution input, so the obligation has to be in it: the route that
        // delegates carries the same composed discipline as the route that
        // executes, because both were handed the one value composed above the
        // branch.
        let CapabilityDispatchOutcome::DelegateInHost { discipline, .. } = outcome else {
            return Err("the claude-provider dispatch delegates to the host".into());
        };
        // Compared as one whole value rather than probed for its parts. The
        // instruction to read every listed document in full lives in the
        // sentence that introduces the list, not in the list or in the base
        // text, so a payload checked only for the base prefix and the resolved
        // paths would still be satisfied by a delegation that dropped it.
        assert_eq!(
            discipline,
            DisciplineText::try_new(BASE_DISCIPLINE.to_owned())?.with_conventions(
                &ConventionResolution::from_matches(vec![convention(RESOLVED_DOCUMENT)?])
            ),
            "the delegated payload is the composed discipline entire — anything this route \
             dropped, reordered, truncated, or added makes it a different value"
        );
        Ok(())
    }

    /// The sentence by which the composed discipline states the reading
    /// obligation, as distinct from the list of paths that obligation applies
    /// to. Held here as its own value because the two are separate halves of
    /// `AC-10` and a test asserting one must not be satisfied by the other.
    const READING_OBLIGATION: &str = "Read each of them in full";

    /// The document the resolver returns for a dispatch naming `reviewer`,
    /// distinct from [`RESOLVED_DOCUMENT`], which it returns for `implementer`.
    /// The two exist as separate values so that a discipline composed from the
    /// wrong dispatch's resolution is visible in the text rather than hidden
    /// behind an answer that happens to be the same either way.
    ///
    /// Any consumer convention distinct from `RESOLVED_DOCUMENT` serves; what
    /// this value may not be is a harness-owned document, because
    /// [`ConventionDocumentPath::try_new`] admits only paths under
    /// `knowledge/conventions/`. A capability's reading obligation and the
    /// resolver's answer are therefore different sets, and a fixture naming a
    /// path outside that root would not survive construction.
    const REVIEWER_DOCUMENT: &str = "knowledge/conventions/security.md";

    /// Convention resolver that answers according to the capability it was
    /// asked about, and keeps every query.
    ///
    /// [`RecordingConventionResolveService`] answers with one fixed set, under
    /// which a discipline carrying an earlier dispatch's resolution is
    /// indistinguishable from one carrying its own. Keying the answer is what
    /// makes that difference observable.
    struct PerCapabilityConventionResolveService {
        documents: Vec<(ConventionCapabilityId, ConventionDocumentPath)>,
        queries: Arc<Mutex<Vec<ResolveConventionsQuery>>>,
    }

    impl ConventionResolveService for PerCapabilityConventionResolveService {
        fn resolve(
            &self,
            query: ResolveConventionsQuery,
        ) -> Result<ConventionResolution, ConventionResolveError> {
            let matched = self
                .documents
                .iter()
                .filter(|(capability, _)| *capability == query.capability)
                .map(|(_, document)| document.clone())
                .collect();
            self.queries.lock().expect("test resolver recorder lock").push(query);
            Ok(ConventionResolution::from_matches(matched))
        }
    }

    fn per_capability_resolver(
        queries: &Arc<Mutex<Vec<ResolveConventionsQuery>>>,
        documents: &[(&str, &str)],
    ) -> Result<Arc<PerCapabilityConventionResolveService>, Box<dyn std::error::Error>> {
        let documents = documents
            .iter()
            .map(|(capability, path)| {
                Ok((ConventionCapabilityId::try_new(*capability)?, convention(path)?))
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
        Ok(Arc::new(PerCapabilityConventionResolveService { documents, queries: queries.clone() }))
    }

    /// Builds an interactor whose profile selects the provider that hands the
    /// dispatch back to the host, so a test can exercise the second route
    /// without restating the profile at each site.
    fn delegating_interactor(
        conventions: Arc<dyn ConventionResolveService>,
    ) -> Result<CapabilityExecInteractor, Box<dyn std::error::Error>> {
        let provider = ProviderName::try_new("claude")?;
        dispatching_interactor(
            CapabilityProfile {
                provider: CapabilityProviderBinding::Standard(provider.clone()),
                model: ModelName::try_new("claude-opus")?,
                effort: ReasoningEffort::High,
                execution_mode: ExecutionMode::OrchestratorOutput,
            },
            conventions,
            vec![Arc::new(DelegatingProviderPort { provider })],
        )
    }

    #[test]
    fn test_capability_exec_both_routes_carry_the_reading_obligation_and_not_only_the_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let documents =
            ["knowledge/conventions/coding-principles.md", "knowledge/conventions/rust/testing.md"];
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let executing = dispatching_interactor(
            profile(ExecutionMode::OrchestratorOutput)?,
            resolver(&recorder(), &documents)?,
            recording_provider(&dispatches)?,
        )?;
        let delegating = delegating_interactor(resolver(&recorder(), &documents)?)?;

        executing.execute(request()?)?;
        let outcome = delegating.execute(request()?)?;

        let recorded = dispatches.lock().expect("test dispatch recorder lock");
        let executed = recorded[0].discipline.clone();
        let CapabilityDispatchOutcome::DelegateInHost { discipline: delegated, .. } = outcome
        else {
            return Err("the claude-provider dispatch delegates to the host".into());
        };
        // `AC-10` asks two things of each route: that the resolved paths reach
        // the dispatched capability, and that the obligation to read every one
        // of those documents in full reaches it too. The paths are pinned by
        // the route assertions above; the obligation is not. Those assertions
        // compare a whole dispatched value against `with_conventions` itself,
        // so a render reduced to a bare bullet list — or one that softened the
        // instruction into a suggestion — composes both sides identically and
        // leaves every one of them green while no capability is told to read
        // anything. This is the assertion that separates the two halves: the
        // obligation sentence is checked as content, on both routes, and is
        // not part of the base discipline the source port supplied.
        for dispatched in [&executed, &delegated] {
            assert_eq!(listed_documents(dispatched), documents.to_vec());
            assert!(
                dispatched.as_str().contains(READING_OBLIGATION),
                "each route states the reading obligation, not merely the list it applies to"
            );
        }
        assert!(!BASE_DISCIPLINE.contains(READING_OBLIGATION));
        assert_eq!(
            executed, delegated,
            "the two routes were handed the same discipline and the same resolution, so the \
             obligation they carry is one text and not two kept equal by review"
        );
        Ok(())
    }

    #[test]
    fn test_capability_exec_empty_resolution_dispatches_no_document_despite_available_declarations()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = trap_project_root()?;
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: profile(ExecutionMode::OrchestratorOutput)?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new(format!(
                    "perform the task\n\nconvention_refs:\n- {TRAP_BRIEFING_DOCUMENT}\n"
                ))?),
                discipline: Ok(DisciplineText::try_new(BASE_DISCIPLINE.to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            resolver(&recorder(), &[])?,
            recording_provider(&dispatches)?,
            root.path().to_path_buf(),
        );

        interactor.execute(CapabilityExecRequest {
            resume: CapabilityResumeRequest::Resume(TargetArtifactSet::try_new(vec![
                TargetArtifactPath::try_new(PathBuf::from(TRAP_TRACK_ARTIFACT))?,
            ])?),
            ..request()?
        })?;

        let recorded = dispatches.lock().expect("test dispatch recorder lock");
        let dispatched = &recorded[0].discipline;
        // The sibling trap dispatch hands the resolver a non-empty answer, so a
        // dispatcher that consulted a track artifact's `convention_refs` only
        // when resolution came back with nothing would satisfy it and still
        // break `AC-12`. This is that dispatcher's case: the resolver returns
        // an empty resolution while the same three competing declarations are
        // reachable — the track artifact under the wired root, the convention
        // document in that root's own tree, and the briefing text the source
        // port loaded — and the dispatched obligation still names none of them.
        assert!(
            listed_documents(dispatched).is_empty(),
            "an empty resolution lists nothing to read, whatever else this dispatch could reach"
        );
        for trap in [TRAP_TRACK_DOCUMENT, TRAP_BRIEFING_DOCUMENT, TRAP_ON_DISK_DOCUMENT] {
            assert!(
                !dispatched.as_str().contains(trap),
                "no available declaration substitutes for a resolution that matched nothing"
            );
        }
        assert!(
            dispatched.as_str().contains("returned zero"),
            "the capability is told resolution ran and matched nothing, rather than being handed \
             a substitute list assembled from somewhere else"
        );
        Ok(())
    }

    #[test]
    fn test_capability_exec_delegate_in_host_route_resolves_conventions_exactly_once()
    -> Result<(), Box<dyn std::error::Error>> {
        let queries = recorder();
        let interactor = delegating_interactor(resolver(&queries, &[RESOLVED_DOCUMENT])?)?;

        interactor.execute(request()?)?;

        // `AC-11`'s once-per-dispatch clause holds for whichever route runs,
        // and the counting assertion above exercises the executing route only.
        // A duplicate resolution is invisible to this route's payload
        // assertion: the resolver answers the same way every time, so a second
        // call composes the same text and a whole-value comparison stays
        // satisfied. The call count is the only thing that sees it.
        assert_eq!(queries.lock().expect("test resolver recorder lock").len(), 1);
        Ok(())
    }

    #[test]
    fn test_capability_exec_resolves_again_for_a_second_dispatch_naming_another_capability()
    -> Result<(), Box<dyn std::error::Error>> {
        let queries = recorder();
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = dispatching_interactor(
            profile(ExecutionMode::OrchestratorOutput)?,
            per_capability_resolver(
                &queries,
                &[("implementer", RESOLVED_DOCUMENT), ("reviewer", REVIEWER_DOCUMENT)],
            )?,
            recording_provider(&dispatches)?,
        )?;

        interactor.execute(request()?)?;
        interactor.execute(CapabilityExecRequest {
            capability: CapabilityName::try_new("reviewer")?,
            ..request()?
        })?;
        interactor.execute(request()?)?;

        // "Once per dispatch" is a rate, not a lifetime budget: every dispatch
        // resolves for itself. Two memoizations would survive a weaker
        // sequence than this one. A resolution cached for the interactor's
        // lifetime is caught by the change of capability at the second
        // dispatch; one cached per capability is not, and is caught only by
        // the third, which names the first capability again. Both would hand a
        // dispatch documents resolved for an earlier one.
        let asked = queries.lock().expect("test resolver recorder lock");
        assert_eq!(asked.len(), 3);
        let implementer = ConventionCapabilityId::from(&CapabilityName::try_new("implementer")?);
        let reviewer = ConventionCapabilityId::from(&CapabilityName::try_new("reviewer")?);
        assert_eq!(
            asked.iter().map(|query| &query.capability).collect::<Vec<_>>(),
            vec![&implementer, &reviewer, &implementer]
        );

        // Counting the calls does not by itself establish that each dispatch
        // was composed from its own answer: an interactor that asked again and
        // then dispatched the previous composition would keep the count
        // correct. The resolver answers differently per capability, so the
        // documents each dispatch was actually handed say which resolution
        // reached it.
        let recorded = dispatches.lock().expect("test dispatch recorder lock");
        assert_eq!(
            recorded
                .iter()
                .map(|dispatch| listed_documents(&dispatch.discipline))
                .collect::<Vec<_>>(),
            vec![vec![RESOLVED_DOCUMENT], vec![REVIEWER_DOCUMENT], vec![RESOLVED_DOCUMENT]],
            "each dispatch carries the documents resolved for the capability it named"
        );
        Ok(())
    }
}
