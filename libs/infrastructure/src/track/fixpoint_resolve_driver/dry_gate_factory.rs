//! Feature-aware factory for the fixpoint DRY gate.

use std::sync::Arc;

use usecase::fixpoint_resolve::FixpointDryGateService;
use usecase::fixpoint_resolve_driver::FixpointDryGateFactoryPort;

use crate::dry_check::noop_approval::NoOpDryApprovalService;

#[cfg(feature = "semantic-dup")]
use crate::dry_check::FsDryCorpusMetaAdapter;
#[cfg(feature = "semantic-dup")]
use crate::dry_check::approval_factory::FsDryApprovalFactoryAdapter;
#[cfg(feature = "semantic-dup")]
use crate::dry_check::diff_base_resolver::FsDiffBaseResolverAdapter;
#[cfg(feature = "semantic-dup")]
use crate::semantic_dup::CodeFragmentExtractorAdapter;
#[cfg(not(feature = "semantic-dup"))]
use std::collections::BTreeSet;
#[cfg(feature = "semantic-dup")]
use usecase::dry_check::DryCheckApprovalService;
#[cfg(feature = "semantic-dup")]
use usecase::dry_check::DryFragmentPipelineInteractor;
#[cfg(feature = "semantic-dup")]
use usecase::fixpoint_resolve::FixpointDryGateInteractor;
#[cfg(not(feature = "semantic-dup"))]
use usecase::fixpoint_resolve::FixpointDryGateOutput;

/// Factory adapter implementing [`FixpointDryGateFactoryPort`].
///
/// Reproduces unchanged the wiring previously done by the removed
/// `TrackCompositionRoot::make_dry_gate_interactor` helper.
pub struct FsFixpointDryGateFactoryAdapter;

impl FixpointDryGateFactoryPort for FsFixpointDryGateFactoryAdapter {
    fn build(&self, base_branch: &str) -> Arc<dyn FixpointDryGateService> {
        #[cfg(feature = "semantic-dup")]
        {
            let diff_source = Arc::new(crate::dry_check::GitDryCheckDiffGetter);
            let extractor = Arc::new(CodeFragmentExtractorAdapter::new());
            let fragment_pipeline =
                Arc::new(DryFragmentPipelineInteractor::new(diff_source, extractor));
            Arc::new(FixpointDryGateInteractor::new(
                Arc::new(NoOpDryApprovalService) as Arc<dyn DryCheckApprovalService + Send + Sync>,
                Arc::new(FsDiffBaseResolverAdapter::new(base_branch.to_owned())),
                Arc::new(FsDryCorpusMetaAdapter),
                fragment_pipeline,
                Arc::new(FsDryApprovalFactoryAdapter),
            ))
        }

        #[cfg(not(feature = "semantic-dup"))]
        {
            let _ = base_branch;
            Arc::new(SemanticDupFeatureDisabledDryGate)
        }
    }
}

/// Fail closed when a configured DRY gate requires an omitted semantic-dup feature.
#[cfg(not(feature = "semantic-dup"))]
struct SemanticDupFeatureDisabledDryGate;

#[cfg(not(feature = "semantic-dup"))]
impl FixpointDryGateService for SemanticDupFeatureDisabledDryGate {
    fn resolve_dry_gate(
        &self,
        cmd: usecase::fixpoint_resolve::FixpointDryGateCommand,
    ) -> Result<FixpointDryGateOutput, usecase::d4_orchestration::D4OrchestrationError> {
        if cmd.dry_config.enabled {
            return Err(usecase::d4_orchestration::D4OrchestrationError::DryGate(
                "semantic-dup support is disabled; rebuild sotp with --features semantic-dup"
                    .to_owned(),
            ));
        }

        Ok(FixpointDryGateOutput {
            current_fragment_refs: BTreeSet::new(),
            dry_approval: Arc::new(NoOpDryApprovalService),
            approval_workspace_root: cmd.canonical_root,
        })
    }
}
