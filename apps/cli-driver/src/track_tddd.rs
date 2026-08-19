//! Primary adapter for Track TDDD command contexts.

use std::fmt::Write as FmtWrite;
use std::path::PathBuf;
use std::sync::Arc;

use usecase::track_lifecycle::tddd::baseline_capture::{
    TrackBaselineCaptureCommand, TrackBaselineCaptureError, TrackBaselineCaptureService,
};
use usecase::track_lifecycle::tddd::baseline_graph::{
    TrackBaselineGraphCommand, TrackBaselineGraphError, TrackBaselineGraphService,
};
use usecase::track_lifecycle::tddd::catalogue_impl_signals::{
    TrackCatalogueImplSignalsCommand, TrackCatalogueImplSignalsError,
    TrackCatalogueImplSignalsService,
};
use usecase::track_lifecycle::tddd::catalogue_lint_active::{
    TrackCatalogueLintActiveCommand, TrackCatalogueLintActiveService,
};
use usecase::track_lifecycle::tddd::catalogue_spec_signals::{
    TrackCatalogueSpecSignalsCommand, TrackCatalogueSpecSignalsError,
    TrackCatalogueSpecSignalsService,
};
use usecase::track_lifecycle::tddd::contract_map::TrackContractMapService;
use usecase::track_lifecycle::tddd::lint::{
    TrackLintCommand, TrackLintRulesFile, TrackLintService,
};
use usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashService;
use usecase::track_lifecycle::tddd::type_signals::TrackTypeSignalsCommand;
use usecase::track_lifecycle::{
    TrackItemsDirectory, TrackLayerFilter, TrackLayerSelection, TrackLifecycleIdInput,
    TrackSelection, TrackSourceWorkspace, TrackSpecAnchorSelection, TrackWorkspaceRoot,
};

use crate::adr_baseline::TrackIdInput;
use crate::render::CommandOutcome;
use crate::track_lint::{
    catalogue_lint_active_error_to_outcome, lint_error_to_outcome,
    render_catalogue_lint_active_result, render_lint_result,
};
use crate::track_resolution::TrackResolutionDiagnostic;
use crate::track_spec_element_hash::{
    render_track_spec_element_hash_result, spec_element_hash_input_to_command,
    track_spec_element_hash_error_to_outcome,
};
use crate::track_type_signals::type_signals_error_to_outcome;

/// Validated `track/items` input for a TDDD command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackItemsDirectoryInput {
    value: PathBuf,
}

impl TrackItemsDirectoryInput {
    /// Validates and wraps a `track/items` path.
    pub fn try_new(value: PathBuf) -> Result<Self, TrackResolutionDiagnostic> {
        TrackItemsDirectory::try_new(value.clone()).map_err(|error| {
            if error.to_string().contains("must end in 'track/items'") {
                TrackResolutionDiagnostic::new(format!(
                    "--items-dir must point to '<project-root>/track/items'; got {}",
                    value.display()
                ))
            } else {
                TrackResolutionDiagnostic::new(error.to_string())
            }
        })?;
        Ok(Self { value })
    }

    /// Derives the workspace-root input from this items directory.
    #[must_use]
    pub fn workspace_root(&self) -> TrackWorkspaceRootInput {
        let root = self
            .value
            .parent()
            .and_then(std::path::Path::parent)
            .map(std::path::Path::to_path_buf)
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| PathBuf::from("."));
        TrackWorkspaceRootInput { value: root }
    }

    pub(crate) fn into_usecase(self) -> Result<TrackItemsDirectory, TrackResolutionDiagnostic> {
        TrackItemsDirectory::try_new(self.value)
            .map_err(|error| TrackResolutionDiagnostic::new(error.to_string()))
    }
}

/// Validated comma-separated TDDD layer input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackLayersInput {
    value: Vec<usecase::LayerId>,
}

impl TrackLayersInput {
    /// Parses and validates a comma-separated layer list.
    pub fn try_new(value: String) -> Result<Self, TrackResolutionDiagnostic> {
        let mut layers = Vec::new();
        for token in value.split(',') {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                continue;
            }
            let layer = usecase::LayerId::try_new(trimmed.to_owned()).map_err(|error| {
                TrackResolutionDiagnostic::new(format!("invalid layer id '{trimmed}': {error}"))
            })?;
            layers.push(layer);
        }
        Ok(Self { value: layers })
    }

    pub(crate) fn into_usecase(self) -> TrackLayerFilter {
        TrackLayerFilter::Selected(self.value)
    }
}

/// Validated workspace-root input for a TDDD command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackWorkspaceRootInput {
    value: PathBuf,
}

impl TryFrom<PathBuf> for TrackWorkspaceRootInput {
    type Error = String;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        TrackWorkspaceRoot::try_new(value.clone())
            .map(|_| Self { value })
            .map_err(|error| error.to_string())
    }
}

impl TrackWorkspaceRootInput {
    pub(crate) fn into_usecase(self) -> Result<TrackWorkspaceRoot, String> {
        TrackWorkspaceRoot::try_new(self.value).map_err(|error| error.to_string())
    }
}

/// Validated rustdoc source-workspace input for a TDDD command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackSourceWorkspaceInput {
    value: PathBuf,
}

impl TryFrom<PathBuf> for TrackSourceWorkspaceInput {
    type Error = String;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        Self::try_new(value).map_err(|error| error.to_string())
    }
}

impl TrackSourceWorkspaceInput {
    /// Validates and wraps a source-workspace path.
    pub fn try_new(value: PathBuf) -> Result<Self, TrackResolutionDiagnostic> {
        TrackSourceWorkspace::try_new(value.clone())
            .map(|_| Self { value })
            .map_err(|error| TrackResolutionDiagnostic::new(error.to_string()))
    }

    fn into_usecase(self) -> Result<TrackSourceWorkspace, String> {
        TrackSourceWorkspace::try_new(self.value).map_err(|error| error.to_string())
    }
}

/// Validated TDDD layer input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackLayerInput {
    value: usecase::LayerId,
}

impl TryFrom<String> for TrackLayerInput {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value).map_err(|error| error.to_string())
    }
}

impl TrackLayerInput {
    /// Parses and validates a single TDDD layer id.
    pub fn try_new(value: String) -> Result<Self, TrackResolutionDiagnostic> {
        usecase::LayerId::try_new(value.clone()).map(|value| Self { value }).map_err(|error| {
            TrackResolutionDiagnostic::new(format!("invalid layer id '{value}': {error}"))
        })
    }

    fn into_usecase(self) -> usecase::LayerId {
        self.value
    }
}

/// Typed primary-adapter input for TDDD baseline capture.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackTdddBaselineCaptureInput {
    /// Optional explicit track id; omitted values select the active track.
    pub track_id: Option<TrackIdInput>,
    /// Workspace containing the track artifacts.
    pub workspace_root: TrackWorkspaceRootInput,
    /// Optional workspace from which rustdoc is run.
    pub source_workspace: Option<TrackSourceWorkspaceInput>,
    /// Optional layer filter.
    pub layer: Option<TrackLayerInput>,
}

/// Typed primary-adapter input for TDDD baseline-graph rendering.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackTdddBaselineGraphInput {
    /// Optional explicit track id; omitted values select the active track.
    pub track_id: Option<TrackIdInput>,
    /// Directory containing the track items.
    pub items_dir: TrackItemsDirectoryInput,
    /// Workspace containing the track artifacts and graph configuration.
    pub workspace_root: TrackWorkspaceRootInput,
    /// Optional comma-separated layer filter.
    pub layers: Option<TrackLayersInput>,
}

/// Typed primary-adapter input for catalogue-to-implementation signals.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackTdddCatalogueImplSignalsInput {
    /// Optional explicit track id; omitted values select the active track.
    pub track_id: Option<TrackIdInput>,
    /// Workspace containing the track artifacts and catalogue configuration.
    pub workspace_root: TrackWorkspaceRootInput,
    /// Optional layer filter.
    pub layer: Option<TrackLayerInput>,
}

/// Typed primary-adapter input for catalogue-spec signals.
pub struct TrackTdddCatalogueSpecSignalsInput {
    /// Optional explicit track id; omitted values select the active track.
    pub track_id: Option<TrackIdInput>,
    /// Directory containing the track items.
    pub items_dir: TrackItemsDirectoryInput,
    /// Workspace containing the track artifacts and catalogue configuration.
    pub workspace_root: TrackWorkspaceRootInput,
    /// Optional layer filter.
    pub layer: Option<TrackLayerInput>,
}

/// Typed primary-adapter input for type-signal evaluation.
pub struct TrackTdddTypeSignalsInput {
    /// Optional explicit track id; omitted values select the active track.
    pub track_id: Option<TrackIdInput>,
    /// Workspace containing the track artifacts and TDDD configuration.
    pub workspace_root: TrackWorkspaceRootInput,
    /// Optional layer filter.
    pub layer: Option<TrackLayerInput>,
}

/// Validated spec-anchor input for a TDDD command.
pub struct TrackSpecAnchorInput {
    value: usecase::SpecElementId,
}

impl TrackSpecAnchorInput {
    /// Validates and wraps a spec-element anchor.
    pub fn try_new(value: String) -> Result<Self, TrackResolutionDiagnostic> {
        usecase::SpecElementId::try_new(value.clone()).map(|value| Self { value }).map_err(
            |error| {
                TrackResolutionDiagnostic::new(format!("invalid spec anchor '{value}': {error}"))
            },
        )
    }

    pub(crate) fn into_usecase(self) -> TrackSpecAnchorSelection {
        TrackSpecAnchorSelection::One(self.value)
    }
}

/// Typed primary-adapter input for spec-element-hash lookup.
pub struct TrackTdddSpecElementHashInput {
    /// Optional explicit track id; omitted values select the active track.
    pub track_id: Option<TrackIdInput>,
    /// Directory containing the track items.
    pub items_dir: TrackItemsDirectoryInput,
    /// Optional single spec anchor; omitted values select all anchors.
    pub anchor: Option<TrackSpecAnchorInput>,
}

/// Validated lint-rules-file input for a TDDD command.
pub struct TrackLintRulesFileInput {
    value: PathBuf,
}

impl TrackLintRulesFileInput {
    /// Validates and wraps a lint-rules-file path.
    pub fn try_new(value: PathBuf) -> Result<Self, TrackResolutionDiagnostic> {
        TrackLintRulesFile::try_new(value.clone())
            .map(|_| Self { value })
            .map_err(|error| TrackResolutionDiagnostic::new(error.to_string()))
    }

    fn into_usecase(self) -> Result<TrackLintRulesFile, String> {
        TrackLintRulesFile::try_new(self.value).map_err(|error| error.to_string())
    }
}

/// Typed primary-adapter input for single-layer catalogue linting.
pub struct TrackTdddLintInput {
    /// Optional explicit track id; omitted values select the active track.
    pub track_id: Option<TrackIdInput>,
    /// Workspace containing the track artifacts and TDDD configuration.
    pub workspace_root: TrackWorkspaceRootInput,
    /// The layer to lint.
    pub layer: TrackLayerInput,
    /// Optional override for the lint rules file.
    pub rules_file: Option<TrackLintRulesFileInput>,
}

/// Typed primary-adapter input for contract-map rendering.
pub struct TrackTdddContractMapInput {
    /// Optional explicit track id; omitted values select the active track.
    pub track_id: Option<TrackIdInput>,
    /// Directory containing the track artifacts.
    pub items_dir: TrackItemsDirectoryInput,
    /// Workspace containing the track artifacts and renderer configuration.
    pub workspace_root: TrackWorkspaceRootInput,
    /// Optional comma-separated layer filter.
    pub layers: Option<TrackLayersInput>,
}

/// Typed primary-adapter input for active-track catalogue linting.
pub struct TrackTdddCatalogueLintActiveInput {
    /// Optional explicit track id; omitted values select the active track.
    pub track_id: Option<TrackIdInput>,
    /// Workspace containing the track artifacts and TDDD configuration.
    pub workspace_root: TrackWorkspaceRootInput,
    /// Optional override for the lint rules file.
    pub rules_file: Option<TrackLintRulesFileInput>,
}

/// Primary adapter for the Track TDDD command family.
pub struct TrackTdddDriver {
    baseline_capture: Arc<dyn TrackBaselineCaptureService>,
    baseline_graph: Arc<dyn TrackBaselineGraphService>,
    catalogue_impl_signals: Arc<dyn TrackCatalogueImplSignalsService>,
    catalogue_spec_signals: Arc<dyn TrackCatalogueSpecSignalsService>,
    spec_element_hash: Arc<dyn TrackSpecElementHashService>,
    catalogue_lint_active: Arc<dyn TrackCatalogueLintActiveService>,
    lint: Arc<dyn TrackLintService>,
    contract_map: Arc<dyn TrackContractMapService>,
    type_signals: Arc<dyn usecase::track_lifecycle::tddd::type_signals::TrackTypeSignalsService>,
}

impl TrackTdddDriver {
    /// Creates a baseline-capture driver from its application service.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        baseline_capture: Arc<dyn TrackBaselineCaptureService>,
        baseline_graph: Arc<dyn TrackBaselineGraphService>,
        catalogue_impl_signals: Arc<dyn TrackCatalogueImplSignalsService>,
        catalogue_spec_signals: Arc<dyn TrackCatalogueSpecSignalsService>,
        spec_element_hash: Arc<dyn TrackSpecElementHashService>,
        catalogue_lint_active: Arc<dyn TrackCatalogueLintActiveService>,
        lint: Arc<dyn TrackLintService>,
        contract_map: Arc<dyn TrackContractMapService>,
        type_signals: Arc<
            dyn usecase::track_lifecycle::tddd::type_signals::TrackTypeSignalsService,
        >,
    ) -> Self {
        Self {
            baseline_capture,
            baseline_graph,
            catalogue_impl_signals,
            catalogue_spec_signals,
            spec_element_hash,
            catalogue_lint_active,
            lint,
            contract_map,
            type_signals,
        }
    }

    /// Executes the baseline-capture input boundary.
    pub fn handle(&self, input: TrackTdddBaselineCaptureInput) -> CommandOutcome {
        let command = match input_to_command(input) {
            Ok(command) => command,
            Err(error) => return CommandOutcome::failure(Some(error)),
        };
        self.baseline_capture
            .execute(command)
            .map(|_| CommandOutcome::success(None))
            .unwrap_or_else(baseline_capture_error_to_outcome)
    }

    /// Executes the baseline-graph input boundary.
    pub fn handle_baseline_graph(&self, input: TrackTdddBaselineGraphInput) -> CommandOutcome {
        let command = match baseline_graph_input_to_command(input) {
            Ok(command) => command,
            Err(error) => return CommandOutcome::failure(Some(error)),
        };
        self.baseline_graph
            .execute(command)
            .map(render_baseline_graph_result)
            .unwrap_or_else(baseline_graph_error_to_outcome)
    }

    /// Executes the catalogue-to-implementation signal input boundary.
    pub fn handle_catalogue_impl_signals(
        &self,
        input: TrackTdddCatalogueImplSignalsInput,
    ) -> CommandOutcome {
        let command = match catalogue_impl_signals_input_to_command(input) {
            Ok(command) => command,
            Err(error) => return CommandOutcome::failure(Some(error)),
        };
        self.catalogue_impl_signals
            .execute(command)
            .map(render_catalogue_impl_signals_result)
            .unwrap_or_else(catalogue_impl_signals_error_to_outcome)
    }

    /// Executes the catalogue-spec signal input boundary.
    pub fn handle_catalogue_spec_signals(
        &self,
        input: TrackTdddCatalogueSpecSignalsInput,
    ) -> CommandOutcome {
        let command = match catalogue_spec_signals_input_to_command(input) {
            Ok(command) => command,
            Err(error) => return CommandOutcome::failure(Some(error)),
        };
        self.catalogue_spec_signals
            .execute(command)
            .map(|_| CommandOutcome::success(None))
            .unwrap_or_else(catalogue_spec_signals_error_to_outcome)
    }

    /// Executes the type-signals input boundary.
    pub fn handle_type_signals(&self, input: TrackTdddTypeSignalsInput) -> CommandOutcome {
        let command = match type_signals_input_to_command(input) {
            Ok(command) => command,
            Err(error) => return CommandOutcome::failure(Some(error)),
        };
        self.type_signals
            .execute(command)
            .map(|_| CommandOutcome::success(None))
            .unwrap_or_else(type_signals_error_to_outcome)
    }

    /// Executes the spec-element-hash input boundary.
    pub fn handle_spec_element_hash(&self, input: TrackTdddSpecElementHashInput) -> CommandOutcome {
        let command = match spec_element_hash_input_to_command(input) {
            Ok(command) => command,
            Err(error) => return CommandOutcome::failure(Some(error)),
        };
        self.spec_element_hash
            .execute(command)
            .map(render_track_spec_element_hash_result)
            .unwrap_or_else(track_spec_element_hash_error_to_outcome)
    }

    /// Executes the active-track catalogue-lint input boundary.
    pub fn handle_catalogue_lint_active(
        &self,
        input: TrackTdddCatalogueLintActiveInput,
    ) -> CommandOutcome {
        let command = match catalogue_lint_active_input_to_command(input) {
            Ok(command) => command,
            Err(error) => return CommandOutcome::failure(Some(error)),
        };
        self.catalogue_lint_active
            .execute(command)
            .map(render_catalogue_lint_active_result)
            .unwrap_or_else(catalogue_lint_active_error_to_outcome)
    }

    /// Executes the single-layer catalogue-lint input boundary.
    pub fn handle_lint(&self, input: TrackTdddLintInput) -> CommandOutcome {
        let command = match lint_input_to_command(input) {
            Ok(command) => command,
            Err(error) => return CommandOutcome::failure(Some(error)),
        };
        self.lint.execute(command).map(render_lint_result).unwrap_or_else(lint_error_to_outcome)
    }

    /// Executes the contract-map input boundary.
    pub fn handle_contract_map(&self, input: TrackTdddContractMapInput) -> CommandOutcome {
        crate::track_contract_map::render_track_contract_map_outcome(&*self.contract_map, input)
    }
}

fn input_to_command(
    input: TrackTdddBaselineCaptureInput,
) -> Result<TrackBaselineCaptureCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let workspace_root = input.workspace_root.into_usecase()?;
    let source_workspace =
        input.source_workspace.map(TrackSourceWorkspaceInput::into_usecase).transpose()?;
    let layer = input
        .layer
        .map(|layer| TrackLayerSelection::One(layer.into_usecase()))
        .unwrap_or(TrackLayerSelection::All);
    Ok(TrackBaselineCaptureCommand { track, workspace_root, source_workspace, layer })
}

fn baseline_capture_error_to_outcome(error: TrackBaselineCaptureError) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}

fn baseline_graph_input_to_command(
    input: TrackTdddBaselineGraphInput,
) -> Result<TrackBaselineGraphCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let items_dir = input.items_dir.into_usecase().map_err(|error| error.to_string())?;
    let workspace_root = input.workspace_root.into_usecase()?;
    let layers = input.layers.map(TrackLayersInput::into_usecase).unwrap_or(TrackLayerFilter::All);
    Ok(TrackBaselineGraphCommand { track, items_dir, workspace_root, layers })
}

fn render_baseline_graph_result(
    result: usecase::track_lifecycle::tddd::baseline_graph::TrackBaselineGraphResult,
) -> CommandOutcome {
    CommandOutcome::success(Some(format!(
        "[OK] baseline-graph: wrote depth-1 overview + depth-2 cluster files for track '{}' \
         (layers={}, files={})",
        result.track_id,
        result.rendered_layers.value(),
        result.written_files.value(),
    )))
}

fn baseline_graph_error_to_outcome(error: TrackBaselineGraphError) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}

fn catalogue_impl_signals_input_to_command(
    input: TrackTdddCatalogueImplSignalsInput,
) -> Result<TrackCatalogueImplSignalsCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let workspace_root = input.workspace_root.into_usecase()?;
    let layer = input
        .layer
        .map(|layer| TrackLayerSelection::One(layer.into_usecase()))
        .unwrap_or(TrackLayerSelection::All);
    Ok(TrackCatalogueImplSignalsCommand { track, workspace_root, layer })
}

fn catalogue_spec_signals_input_to_command(
    input: TrackTdddCatalogueSpecSignalsInput,
) -> Result<TrackCatalogueSpecSignalsCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let items_dir = input.items_dir.into_usecase().map_err(|error| error.to_string())?;
    let workspace_root = input.workspace_root.into_usecase()?;
    let layer = input
        .layer
        .map(|layer| TrackLayerSelection::One(layer.into_usecase()))
        .unwrap_or(TrackLayerSelection::All);
    Ok(TrackCatalogueSpecSignalsCommand { track, items_dir, workspace_root, layer })
}

fn type_signals_input_to_command(
    input: TrackTdddTypeSignalsInput,
) -> Result<TrackTypeSignalsCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let workspace_root = input.workspace_root.into_usecase()?;
    let layer = input
        .layer
        .map(|layer| TrackLayerSelection::One(layer.into_usecase()))
        .unwrap_or(TrackLayerSelection::All);
    Ok(TrackTypeSignalsCommand { track, workspace_root, layer })
}

fn catalogue_lint_active_input_to_command(
    input: TrackTdddCatalogueLintActiveInput,
) -> Result<TrackCatalogueLintActiveCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let workspace_root = input.workspace_root.into_usecase()?;
    let rules_file = input.rules_file.map(TrackLintRulesFileInput::into_usecase).transpose()?;
    Ok(TrackCatalogueLintActiveCommand { track, workspace_root, rules_file })
}

fn render_catalogue_impl_signals_result(
    result: usecase::track_lifecycle::tddd::catalogue_impl_signals::TrackCatalogueImplSignalsResult,
) -> CommandOutcome {
    let mut report = String::new();
    let mut total_red = 0usize;

    for layer in result.layers {
        let _ = writeln!(report);
        let _ = writeln!(report, "## Layer: `{}`", layer.layer);
        let _ = writeln!(report);

        if layer.signals.is_empty() {
            let _ = writeln!(report, "All items maintained (no non-skip signals).");
            continue;
        }

        let _ = writeln!(report, "| Item | Region | Signal |");
        let _ = writeln!(report, "|------|--------|--------|");
        for signal in &layer.signals {
            let kind = if signal.signal().is_blue() {
                "🔵 Blue"
            } else if signal.signal().is_yellow() {
                "🟡 Yellow"
            } else if signal.signal().is_red() {
                "🔴 Red"
            } else {
                "Skip"
            };
            let _ =
                writeln!(report, "| {} | {:?} | {} |", signal.item_name(), signal.region(), kind);
        }
        let _ = writeln!(report);
        let blue = layer.signals.iter().filter(|signal| signal.signal().is_blue()).count();
        let yellow = layer.signals.iter().filter(|signal| signal.signal().is_yellow()).count();
        let red = layer.signals.iter().filter(|signal| signal.signal().is_red()).count();
        total_red = total_red.saturating_add(red);
        let _ = writeln!(report, "Summary: 🔵 {blue} Blue | 🟡 {yellow} Yellow | 🔴 {red} Red");
    }

    CommandOutcome { stdout: Some(report), stderr: None, exit_code: u8::from(total_red > 0) }
}

fn catalogue_impl_signals_error_to_outcome(
    error: TrackCatalogueImplSignalsError,
) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}

fn catalogue_spec_signals_error_to_outcome(
    error: TrackCatalogueSpecSignalsError,
) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}

fn lint_input_to_command(input: TrackTdddLintInput) -> Result<TrackLintCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let workspace_root = input.workspace_root.into_usecase()?;
    let layer = input.layer.into_usecase();
    let rules_file = input.rules_file.map(TrackLintRulesFileInput::into_usecase).transpose()?;
    Ok(TrackLintCommand { track, workspace_root, layer, rules_file })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unreachable, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use usecase::track_lifecycle::tddd::baseline_capture::TrackBaselineCaptureResult;
    use usecase::track_lifecycle::tddd::baseline_graph::TrackBaselineGraphResult;
    use usecase::track_lifecycle::tddd::catalogue_impl_signals::TrackCatalogueImplSignalsResult;
    use usecase::track_lifecycle::tddd::catalogue_lint_active::{
        TrackCatalogueLintActiveError, TrackCatalogueLintActiveResult,
        TrackCatalogueLintLayerResult,
    };
    use usecase::track_lifecycle::tddd::contract_map::{
        TrackContractMapCommand, TrackContractMapError,
    };
    use usecase::track_lifecycle::tddd::lint::{TrackLintError, TrackLintResult};
    use usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashCommand;
    use usecase::track_lifecycle::tddd::type_signals::{
        TrackTypeSignalsCommand, TrackTypeSignalsError, TrackTypeSignalsResult,
        TrackTypeSignalsService,
    };
    use usecase::track_lifecycle::{TrackRenderedLayerCount, TrackWrittenFileCount};

    #[test]
    fn test_track_items_directory_input_try_new_rejects_noncanonical_path() {
        let error = TrackItemsDirectoryInput::try_new(PathBuf::from("items"))
            .expect_err("noncanonical items dir must fail");
        assert!(error.to_string().contains("--items-dir must point to"));
    }

    #[test]
    fn test_track_path_inputs_reject_parent_relative_paths_without_filesystem_access() {
        let items =
            TrackItemsDirectoryInput::try_new(PathBuf::from("/workspace/anchor/../track/items"));
        assert!(items.is_err());

        let workspace = TrackWorkspaceRootInput::try_from(PathBuf::from("/workspace/anchor/.."));
        assert!(workspace.is_err());
    }

    struct UnusedBaselineGraphService;

    impl TrackBaselineGraphService for UnusedBaselineGraphService {
        fn execute(
            &self,
            _: TrackBaselineGraphCommand,
        ) -> Result<TrackBaselineGraphResult, TrackBaselineGraphError> {
            Err(TrackBaselineGraphError::ExecutionFailed(
                usecase::git_workflow::DiagnosticText::new("baseline graph service unused"),
            ))
        }
    }

    struct UnusedCatalogueImplSignalsService;

    impl TrackCatalogueImplSignalsService for UnusedCatalogueImplSignalsService {
        fn execute(
            &self,
            _: TrackCatalogueImplSignalsCommand,
        ) -> Result<TrackCatalogueImplSignalsResult, TrackCatalogueImplSignalsError> {
            unreachable!()
        }
    }

    struct UnusedCatalogueSpecSignalsService;

    impl TrackCatalogueSpecSignalsService for UnusedCatalogueSpecSignalsService {
        fn execute(
            &self,
            _: TrackCatalogueSpecSignalsCommand,
        ) -> Result<
            usecase::track_lifecycle::tddd::catalogue_spec_signals::TrackCatalogueSpecSignalsResult,
            TrackCatalogueSpecSignalsError,
        > {
            unreachable!()
        }
    }

    struct UnusedSpecElementHashService;

    impl TrackSpecElementHashService for UnusedSpecElementHashService {
        fn execute(
            &self,
            _: TrackSpecElementHashCommand,
        ) -> Result<
            usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashResult,
            usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashError,
        > {
            unreachable!()
        }
    }

    struct UnusedCatalogueLintActiveService;

    impl TrackCatalogueLintActiveService for UnusedCatalogueLintActiveService {
        fn execute(
            &self,
            _: TrackCatalogueLintActiveCommand,
        ) -> Result<TrackCatalogueLintActiveResult, TrackCatalogueLintActiveError> {
            unreachable!()
        }
    }

    struct UnusedLintService;

    impl TrackLintService for UnusedLintService {
        fn execute(&self, _: TrackLintCommand) -> Result<TrackLintResult, TrackLintError> {
            unreachable!()
        }
    }

    struct UnusedTrackContractMapService;

    struct UnusedTypeSignalsService;

    impl TrackContractMapService for UnusedTrackContractMapService {
        fn execute(
            &self,
            _: TrackContractMapCommand,
        ) -> Result<
            usecase::track_lifecycle::tddd::contract_map::TrackContractMapResult,
            TrackContractMapError,
        > {
            unreachable!()
        }
    }

    impl TrackTypeSignalsService for UnusedTypeSignalsService {
        fn execute(
            &self,
            _: TrackTypeSignalsCommand,
        ) -> Result<TrackTypeSignalsResult, TrackTypeSignalsError> {
            unreachable!()
        }
    }

    struct RecordingLintService {
        commands: Mutex<Vec<TrackLintCommand>>,
        result: Result<TrackLintResult, String>,
    }

    impl TrackLintService for RecordingLintService {
        fn execute(&self, command: TrackLintCommand) -> Result<TrackLintResult, TrackLintError> {
            self.commands.lock().expect("command lock is available").push(command);
            match &self.result {
                Ok(result) => Ok(TrackLintResult { violations: result.violations.clone() }),
                Err(error) => Err(TrackLintError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
            }
        }
    }

    struct RecordingCatalogueImplSignalsService {
        commands: Mutex<Vec<TrackCatalogueImplSignalsCommand>>,
        error: Option<String>,
    }

    impl TrackCatalogueImplSignalsService for RecordingCatalogueImplSignalsService {
        fn execute(
            &self,
            command: TrackCatalogueImplSignalsCommand,
        ) -> Result<TrackCatalogueImplSignalsResult, TrackCatalogueImplSignalsError> {
            self.commands.lock().expect("command lock is available").push(command);
            match &self.error {
                Some(error) => Err(TrackCatalogueImplSignalsError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
                None => Ok(TrackCatalogueImplSignalsResult {
                    layers: vec![usecase::track_lifecycle::TrackCatalogueImplLayerResult {
                        layer: domain::tddd::LayerId::try_new("usecase")
                            .expect("layer id is valid"),
                        signals: vec![domain::tddd::ThreeWaySignal::new(
                            "TrackInitService".to_owned(),
                            domain::tddd::SignalRegion::SIntersectC_Match_Add,
                        )],
                    }],
                }),
            }
        }
    }

    struct RecordingCatalogueSpecSignalsService {
        commands: Mutex<Vec<TrackCatalogueSpecSignalsCommand>>,
        error: Option<String>,
    }

    impl TrackCatalogueSpecSignalsService for RecordingCatalogueSpecSignalsService {
        fn execute(
            &self,
            command: TrackCatalogueSpecSignalsCommand,
        ) -> Result<
            usecase::track_lifecycle::tddd::catalogue_spec_signals::TrackCatalogueSpecSignalsResult,
            TrackCatalogueSpecSignalsError,
        > {
            self.commands.lock().expect("command lock is available").push(command);
            match &self.error {
                Some(error) => Err(TrackCatalogueSpecSignalsError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
                None => Ok(
                    usecase::track_lifecycle::tddd::catalogue_spec_signals::TrackCatalogueSpecSignalsResult {
                        layers: vec![],
                    },
                ),
            }
        }
    }

    struct RecordingSpecElementHashService {
        commands: Mutex<Vec<TrackSpecElementHashCommand>>,
        result: Result<
            usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashResult,
            String,
        >,
    }

    impl TrackSpecElementHashService for RecordingSpecElementHashService {
        fn execute(
            &self,
            command: TrackSpecElementHashCommand,
        ) -> Result<
            usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashResult,
            usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashError,
        > {
            self.commands.lock().expect("command lock is available").push(command);
            match &self.result {
                Ok(usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashResult::Single(hash)) => {
                    Ok(usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashResult::Single(hash.clone()))
                }
                Ok(usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashResult::All(hashes)) => {
                    Ok(usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashResult::All(hashes.clone()))
                }
                Err(error) => Err(usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
            }
        }
    }

    struct RecordingCatalogueLintActiveService {
        commands: Mutex<Vec<TrackCatalogueLintActiveCommand>>,
        error: Option<String>,
    }

    impl TrackCatalogueLintActiveService for RecordingCatalogueLintActiveService {
        fn execute(
            &self,
            command: TrackCatalogueLintActiveCommand,
        ) -> Result<TrackCatalogueLintActiveResult, TrackCatalogueLintActiveError> {
            self.commands.lock().expect("command lock is available").push(command);
            match &self.error {
                Some(error) => Err(TrackCatalogueLintActiveError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
                None => Ok(TrackCatalogueLintActiveResult::Checked { layers: vec![] }),
            }
        }
    }

    struct RecordingService {
        commands: Mutex<Vec<TrackBaselineCaptureCommand>>,
        result: Result<TrackBaselineCaptureResult, TrackBaselineCaptureError>,
    }

    impl TrackBaselineCaptureService for RecordingService {
        fn execute(
            &self,
            command: TrackBaselineCaptureCommand,
        ) -> Result<TrackBaselineCaptureResult, TrackBaselineCaptureError> {
            self.commands.lock().expect("command lock is available").push(command);
            match &self.result {
                Ok(result) => Ok(result.clone()),
                Err(error) => Err(TrackBaselineCaptureError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error.to_string()),
                )),
            }
        }
    }

    struct RecordingBaselineGraphService {
        commands: Mutex<Vec<TrackBaselineGraphCommand>>,
        result: Result<(usize, usize), String>,
    }

    impl TrackBaselineGraphService for RecordingBaselineGraphService {
        fn execute(
            &self,
            command: TrackBaselineGraphCommand,
        ) -> Result<TrackBaselineGraphResult, TrackBaselineGraphError> {
            let track_id = match &command.track {
                TrackSelection::Explicit(track_id) => track_id.clone(),
                TrackSelection::Active => {
                    domain::TrackId::try_new("graph-track").expect("track id is valid")
                }
            };
            self.commands.lock().expect("command lock is available").push(command);
            match &self.result {
                Ok((rendered_layers, written_files)) => Ok(TrackBaselineGraphResult {
                    track_id,
                    rendered_layers: TrackRenderedLayerCount::new(*rendered_layers),
                    written_files: TrackWrittenFileCount::new(*written_files),
                }),
                Err(error) => Err(TrackBaselineGraphError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
            }
        }
    }

    fn workspace_root() -> TrackWorkspaceRootInput {
        TrackWorkspaceRootInput::try_from(PathBuf::from("workspace"))
            .expect("workspace root is valid")
    }

    fn track_id() -> TrackIdInput {
        "capture-track".parse().expect("track id is valid")
    }

    #[test]
    fn test_track_tddd_driver_valid_baseline_capture_input_returns_success() {
        let service = Arc::new(RecordingService {
            commands: Mutex::new(Vec::new()),
            result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
        });
        let driver = TrackTdddDriver::new(
            service.clone(),
            Arc::new(UnusedBaselineGraphService),
            Arc::new(UnusedCatalogueImplSignalsService),
            Arc::new(UnusedCatalogueSpecSignalsService),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );

        let outcome = driver.handle(TrackTdddBaselineCaptureInput {
            track_id: Some(track_id()),
            workspace_root: workspace_root(),
            source_workspace: None,
            layer: Some(TrackLayerInput::try_from("usecase".to_owned()).expect("layer is valid")),
        });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, None);
        let commands = service.commands.lock().expect("command lock is available");
        assert_eq!(commands.len(), 1);
        let command = commands.first().expect("driver forwards one command");
        assert!(matches!(
            &command.track,
            TrackSelection::Explicit(track_id) if track_id.as_ref() == "capture-track"
        ));
        assert_eq!(command.workspace_root.as_path(), std::path::Path::new("workspace"));
        assert_eq!(command.source_workspace.as_ref().map(|workspace| workspace.as_path()), None);
        assert!(matches!(
            &command.layer,
            TrackLayerSelection::One(layer) if layer.as_ref() == "usecase"
        ));
    }

    #[test]
    fn test_track_tddd_driver_invalid_workspace_input_returns_failure_without_service_call() {
        let service = Arc::new(RecordingService {
            commands: Mutex::new(Vec::new()),
            result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
        });
        let driver = TrackTdddDriver::new(
            service.clone(),
            Arc::new(UnusedBaselineGraphService),
            Arc::new(UnusedCatalogueImplSignalsService),
            Arc::new(UnusedCatalogueSpecSignalsService),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );

        let input = TrackTdddBaselineCaptureInput {
            track_id: None,
            workspace_root: TrackWorkspaceRootInput { value: PathBuf::from("../escape") },
            source_workspace: None,
            layer: None,
        };
        let outcome = driver.handle(input);

        assert_eq!(outcome.exit_code, 1);
        assert!(service.commands.lock().expect("command lock is available").is_empty());
    }

    #[test]
    fn test_track_tddd_driver_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingService {
            commands: Mutex::new(Vec::new()),
            result: Err(TrackBaselineCaptureError::ExecutionFailed(
                usecase::git_workflow::DiagnosticText::new("capture failed"),
            )),
        });
        let driver = TrackTdddDriver::new(
            service,
            Arc::new(UnusedBaselineGraphService),
            Arc::new(UnusedCatalogueImplSignalsService),
            Arc::new(UnusedCatalogueSpecSignalsService),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );

        let outcome = driver.handle(TrackTdddBaselineCaptureInput {
            track_id: Some(track_id()),
            workspace_root: workspace_root(),
            source_workspace: None,
            layer: None,
        });

        assert_eq!(outcome.stderr.as_deref(), Some("capture failed"));
        assert_eq!(outcome.exit_code, 1);
    }

    fn graph_input() -> TrackTdddBaselineGraphInput {
        TrackTdddBaselineGraphInput {
            track_id: Some("graph-track".parse().expect("track id is valid")),
            items_dir: TrackItemsDirectoryInput::try_new(PathBuf::from("workspace/track/items"))
                .expect("items directory is valid"),
            workspace_root: workspace_root(),
            layers: Some(
                TrackLayersInput::try_new("domain, usecase".to_owned())
                    .expect("layer list is valid"),
            ),
        }
    }

    #[test]
    fn test_track_tddd_items_directory_input_rejects_noncanonical_path() {
        let result = TrackItemsDirectoryInput::try_new(PathBuf::from("fixture/items"));

        assert_eq!(
            result.unwrap_err().message(),
            "--items-dir must point to '<project-root>/track/items'; got fixture/items"
        );
    }

    #[test]
    fn test_track_layers_input_invalid_layer_preserves_legacy_diagnostic_prefix() {
        let inner_error = usecase::LayerId::try_new("not a layer".to_owned())
            .expect_err("invalid layer fixture must fail");
        let error = TrackLayersInput::try_new("not a layer".to_owned())
            .expect_err("invalid layer input must fail");

        assert_eq!(error.to_string(), format!("invalid layer id 'not a layer': {inner_error}"));
    }

    #[test]
    fn test_track_layer_input_try_new_invalid_layer_preserves_legacy_diagnostic_prefix() {
        let inner_error = usecase::LayerId::try_new("not a layer".to_owned())
            .expect_err("invalid layer fixture must fail");
        let error = TrackLayerInput::try_new("not a layer".to_owned())
            .expect_err("invalid layer input must fail");

        assert_eq!(error.to_string(), format!("invalid layer id 'not a layer': {inner_error}"));
    }

    #[test]
    fn test_track_tddd_driver_valid_baseline_graph_input_returns_render_summary() {
        let service = Arc::new(RecordingBaselineGraphService {
            commands: Mutex::new(Vec::new()),
            result: Ok((2, 5)),
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            service.clone(),
            Arc::new(UnusedCatalogueImplSignalsService),
            Arc::new(UnusedCatalogueSpecSignalsService),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );

        let outcome = driver.handle_baseline_graph(graph_input());

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "[OK] baseline-graph: wrote depth-1 overview + depth-2 cluster files for track 'graph-track' (layers=2, files=5)"
            )
        );
        let commands = service.commands.lock().expect("command lock is available");
        assert_eq!(commands.len(), 1);
        let command = commands.first().expect("driver forwards one command");
        assert_eq!(command.items_dir.as_path(), std::path::Path::new("workspace/track/items"));
        assert!(matches!(
            &command.track,
            TrackSelection::Explicit(track_id) if track_id.as_ref() == "graph-track"
        ));
        assert!(matches!(
            &command.layers,
            TrackLayerFilter::Selected(layers)
                if layers.iter().map(|layer| layer.as_ref()).collect::<Vec<_>>()
                    == ["domain", "usecase"]
        ));
    }

    #[test]
    fn test_track_tddd_driver_invalid_baseline_graph_items_dir_returns_failure_without_service_call()
     {
        let service = Arc::new(RecordingBaselineGraphService {
            commands: Mutex::new(Vec::new()),
            result: Ok((1, 1)),
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            service.clone(),
            Arc::new(UnusedCatalogueImplSignalsService),
            Arc::new(UnusedCatalogueSpecSignalsService),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );
        let input = TrackTdddBaselineGraphInput {
            track_id: None,
            items_dir: TrackItemsDirectoryInput { value: PathBuf::from("../escape") },
            workspace_root: workspace_root(),
            layers: None,
        };

        let outcome = driver.handle_baseline_graph(input);

        assert_eq!(outcome.exit_code, 1);
        assert!(service.commands.lock().expect("command lock is available").is_empty());
    }

    #[test]
    fn test_track_tddd_driver_baseline_graph_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingBaselineGraphService {
            commands: Mutex::new(Vec::new()),
            result: Err("graph failed".to_owned()),
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            service,
            Arc::new(UnusedCatalogueImplSignalsService),
            Arc::new(UnusedCatalogueSpecSignalsService),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );

        let outcome = driver.handle_baseline_graph(graph_input());

        assert_eq!(outcome.stderr.as_deref(), Some("graph failed"));
        assert_eq!(outcome.exit_code, 1);
    }

    fn catalogue_impl_signals_input() -> TrackTdddCatalogueImplSignalsInput {
        TrackTdddCatalogueImplSignalsInput {
            track_id: Some("signals-track".parse().expect("track id is valid")),
            workspace_root: workspace_root(),
            layer: Some(TrackLayerInput::try_from("usecase".to_owned()).expect("layer is valid")),
        }
    }

    #[test]
    fn test_track_tddd_driver_valid_catalogue_impl_signals_input_returns_markdown_report() {
        let service = Arc::new(RecordingCatalogueImplSignalsService {
            commands: Mutex::new(Vec::new()),
            error: None,
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            Arc::new(UnusedBaselineGraphService),
            service.clone(),
            Arc::new(UnusedCatalogueSpecSignalsService),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );

        let outcome = driver.handle_catalogue_impl_signals(catalogue_impl_signals_input());

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.expect("catalogue report is printed");
        assert!(stdout.contains("## Layer: `usecase`"));
        let commands = service.commands.lock().expect("command lock is available");
        assert_eq!(commands.len(), 1);
        let command = commands.first().expect("one command is recorded");
        assert!(matches!(
            &command.track,
            TrackSelection::Explicit(track_id) if track_id.as_ref() == "signals-track"
        ));
        assert!(matches!(&command.layer, TrackLayerSelection::One(_)));
    }

    #[test]
    fn test_render_catalogue_impl_signals_result_red_signal_returns_failure_outcome() {
        let outcome = render_catalogue_impl_signals_result(TrackCatalogueImplSignalsResult {
            layers: vec![usecase::track_lifecycle::TrackCatalogueImplLayerResult {
                layer: domain::tddd::LayerId::try_new("usecase").expect("layer id is valid"),
                signals: vec![domain::tddd::ThreeWaySignal::new(
                    "TrackInitService".to_owned(),
                    domain::tddd::SignalRegion::CMinusSUnionD,
                )],
            }],
        });

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stdout.unwrap().contains("🔴 Red"));
    }

    #[test]
    fn test_track_tddd_driver_invalid_catalogue_impl_signals_workspace_returns_failure_without_service_call()
     {
        let service = Arc::new(RecordingCatalogueImplSignalsService {
            commands: Mutex::new(Vec::new()),
            error: None,
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            Arc::new(UnusedBaselineGraphService),
            service.clone(),
            Arc::new(UnusedCatalogueSpecSignalsService),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );
        let input = TrackTdddCatalogueImplSignalsInput {
            track_id: None,
            workspace_root: TrackWorkspaceRootInput { value: PathBuf::from("../escape") },
            layer: None,
        };

        let outcome = driver.handle_catalogue_impl_signals(input);

        assert_eq!(outcome.exit_code, 1);
        assert!(service.commands.lock().expect("command lock is available").is_empty());
    }

    #[test]
    fn test_track_tddd_driver_catalogue_impl_signals_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingCatalogueImplSignalsService {
            commands: Mutex::new(Vec::new()),
            error: Some("signals failed".to_owned()),
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            Arc::new(UnusedBaselineGraphService),
            service,
            Arc::new(UnusedCatalogueSpecSignalsService),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );

        let outcome = driver.handle_catalogue_impl_signals(catalogue_impl_signals_input());

        assert_eq!(outcome.stderr.as_deref(), Some("signals failed"));
        assert_eq!(outcome.exit_code, 1);
    }

    fn catalogue_spec_signals_input() -> TrackTdddCatalogueSpecSignalsInput {
        TrackTdddCatalogueSpecSignalsInput {
            track_id: Some("signals-track".parse().expect("track id is valid")),
            items_dir: TrackItemsDirectoryInput::try_new(PathBuf::from("workspace/track/items"))
                .expect("items directory is valid"),
            workspace_root: workspace_root(),
            layer: Some(TrackLayerInput::try_from("usecase".to_owned()).expect("layer is valid")),
        }
    }

    #[test]
    fn test_track_tddd_driver_valid_catalogue_spec_signals_input_returns_empty_success() {
        let service = Arc::new(RecordingCatalogueSpecSignalsService {
            commands: Mutex::new(Vec::new()),
            error: None,
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            Arc::new(UnusedBaselineGraphService),
            Arc::new(UnusedCatalogueImplSignalsService),
            service.clone(),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );

        let outcome = driver.handle_catalogue_spec_signals(catalogue_spec_signals_input());

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, None);
        let commands = service.commands.lock().expect("command lock is available");
        assert_eq!(commands.len(), 1);
        let command = commands.first().expect("one command is recorded");
        assert!(matches!(
            &command.track,
            TrackSelection::Explicit(track_id) if track_id.as_ref() == "signals-track"
        ));
        assert_eq!(command.items_dir.as_path(), std::path::Path::new("workspace/track/items"));
        assert_eq!(command.workspace_root.as_path(), std::path::Path::new("workspace"));
        assert!(matches!(&command.layer, TrackLayerSelection::One(_)));
    }

    #[test]
    fn test_track_tddd_driver_invalid_catalogue_spec_signals_items_returns_failure_without_service_call()
     {
        let service = Arc::new(RecordingCatalogueSpecSignalsService {
            commands: Mutex::new(Vec::new()),
            error: None,
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            Arc::new(UnusedBaselineGraphService),
            Arc::new(UnusedCatalogueImplSignalsService),
            service.clone(),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );
        let input = TrackTdddCatalogueSpecSignalsInput {
            track_id: None,
            items_dir: TrackItemsDirectoryInput { value: PathBuf::from("../escape") },
            workspace_root: workspace_root(),
            layer: None,
        };

        let outcome = driver.handle_catalogue_spec_signals(input);

        assert_eq!(outcome.exit_code, 1);
        assert!(service.commands.lock().expect("command lock is available").is_empty());
    }

    #[test]
    fn test_track_tddd_driver_catalogue_spec_signals_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingCatalogueSpecSignalsService {
            commands: Mutex::new(Vec::new()),
            error: Some("catalogue spec signals failed".to_owned()),
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            Arc::new(UnusedBaselineGraphService),
            Arc::new(UnusedCatalogueImplSignalsService),
            service,
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );

        let outcome = driver.handle_catalogue_spec_signals(catalogue_spec_signals_input());

        assert_eq!(outcome.stderr.as_deref(), Some("catalogue spec signals failed"));
        assert_eq!(outcome.exit_code, 1);
    }

    fn spec_element_hash_input() -> TrackTdddSpecElementHashInput {
        TrackTdddSpecElementHashInput {
            track_id: Some("hash-track".parse().expect("track id is valid")),
            items_dir: TrackItemsDirectoryInput::try_new(PathBuf::from("workspace/track/items"))
                .expect("items directory is valid"),
            anchor: Some(
                TrackSpecAnchorInput::try_new("GL-01".to_owned()).expect("spec anchor is valid"),
            ),
        }
    }

    fn spec_element_hash_driver(service: Arc<RecordingSpecElementHashService>) -> TrackTdddDriver {
        TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            Arc::new(UnusedBaselineGraphService),
            Arc::new(UnusedCatalogueImplSignalsService),
            Arc::new(UnusedCatalogueSpecSignalsService),
            service,
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        )
    }

    #[test]
    fn test_track_spec_anchor_input_try_new_rejects_malformed_anchor() {
        let error = match TrackSpecAnchorInput::try_new("not-an-anchor".to_owned()) {
            Ok(_) => panic!("malformed spec anchor must be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("invalid spec anchor"));
    }

    #[test]
    fn test_track_tddd_driver_spec_element_hash_preserves_single_hash_output() {
        let service = Arc::new(RecordingSpecElementHashService {
            commands: Mutex::new(Vec::new()),
            result: Ok(
                usecase::track_lifecycle::tddd::spec_element_hash::TrackSpecElementHashResult::Single(
                    domain::ContentHash::from_bytes([7; 32]),
                ),
            ),
        });
        let driver = spec_element_hash_driver(service.clone());

        let outcome = driver.handle_spec_element_hash(spec_element_hash_input());

        assert_eq!(outcome.exit_code, 0);
        let expected = "07".repeat(32);
        assert_eq!(outcome.stdout.as_deref(), Some(expected.as_str()));
        let commands = service.commands.lock().expect("command lock is available");
        assert_eq!(commands.len(), 1);
        let command = commands.first().expect("one command is recorded");
        assert!(matches!(
            &command.track,
            TrackSelection::Explicit(track_id) if track_id.as_ref() == "hash-track"
        ));
        assert_eq!(command.items_dir.as_path(), std::path::Path::new("workspace/track/items"));
        assert!(matches!(
            &command.anchor,
            TrackSpecAnchorSelection::One(anchor) if anchor.as_ref() == "GL-01"
        ));
    }

    fn catalogue_lint_active_input() -> TrackTdddCatalogueLintActiveInput {
        TrackTdddCatalogueLintActiveInput {
            track_id: Some("lint-track".parse().expect("track id is valid")),
            workspace_root: workspace_root(),
            rules_file: Some(
                TrackLintRulesFileInput::try_new(PathBuf::from("custom/rules.json"))
                    .expect("rules file is valid"),
            ),
        }
    }

    #[test]
    fn test_track_tddd_driver_valid_catalogue_lint_active_input_returns_success() {
        let service = Arc::new(RecordingCatalogueLintActiveService {
            commands: Mutex::new(Vec::new()),
            error: None,
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            Arc::new(UnusedBaselineGraphService),
            Arc::new(UnusedCatalogueImplSignalsService),
            Arc::new(UnusedCatalogueSpecSignalsService),
            Arc::new(UnusedSpecElementHashService),
            service.clone(),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        );

        let outcome = driver.handle_catalogue_lint_active(catalogue_lint_active_input());

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.is_none());
        assert_eq!(outcome.stderr.as_deref(), Some("Found 0 violation(s) across 0 layer(s)"));
        let commands = service.commands.lock().expect("command lock is available");
        assert_eq!(commands.len(), 1);
        let command = commands.first().expect("one command is recorded");
        assert!(matches!(
            &command.track,
            TrackSelection::Explicit(track_id) if track_id.as_ref() == "lint-track"
        ));
        assert_eq!(command.workspace_root.as_path(), std::path::Path::new("workspace"));
        assert_eq!(
            command.rules_file.as_ref().map(|rules| rules.as_path()),
            Some(std::path::Path::new("custom/rules.json"))
        );
    }

    #[test]
    fn test_render_catalogue_lint_active_result_red_report_preserves_report_and_summary() {
        let result = TrackCatalogueLintActiveResult::Checked {
            layers: vec![TrackCatalogueLintLayerResult {
                layer: domain::tddd::LayerId::try_new("usecase").expect("layer id is valid"),
                violations: vec![domain::CatalogueLintViolation::new(
                    "NoPublicField",
                    "TrackAddTaskCommand",
                    "public field is forbidden",
                )],
            }],
        };

        let outcome = render_catalogue_lint_active_result(result);

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stderr.as_deref(), Some("Found 1 violation(s) across 1 layer(s)"));
        assert!(outcome.stdout.expect("red report is printed").contains("NoPublicField"));
    }

    fn lint_input() -> TrackTdddLintInput {
        TrackTdddLintInput {
            track_id: Some("lint-track".parse().expect("track id is valid")),
            workspace_root: workspace_root(),
            layer: TrackLayerInput::try_new("domain".to_owned()).expect("layer is valid"),
            rules_file: Some(
                TrackLintRulesFileInput::try_new(PathBuf::from("custom/rules.json"))
                    .expect("rules file is valid"),
            ),
        }
    }

    fn lint_driver(lint: Arc<RecordingLintService>) -> TrackTdddDriver {
        TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            Arc::new(UnusedBaselineGraphService),
            Arc::new(UnusedCatalogueImplSignalsService),
            Arc::new(UnusedCatalogueSpecSignalsService),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            lint,
            Arc::new(UnusedTrackContractMapService),
            Arc::new(UnusedTypeSignalsService),
        )
    }

    #[test]
    fn test_track_tddd_driver_valid_lint_input_returns_success() {
        let service = Arc::new(RecordingLintService {
            commands: Mutex::new(Vec::new()),
            result: Ok(TrackLintResult { violations: Vec::new() }),
        });
        let outcome = lint_driver(service.clone()).handle_lint(lint_input());

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.is_none());
        assert_eq!(outcome.stderr.as_deref(), Some("Found 0 violation(s)"));
        let commands = service.commands.lock().expect("command lock is available");
        assert_eq!(commands.len(), 1);
        let command = commands.first().expect("one command is recorded");
        assert!(matches!(
            &command.track,
            TrackSelection::Explicit(track_id) if track_id.as_ref() == "lint-track"
        ));
        assert_eq!(command.workspace_root.as_path(), std::path::Path::new("workspace"));
        assert_eq!(command.layer.as_ref(), "domain");
        assert_eq!(
            command.rules_file.as_ref().map(|rules| rules.as_path()),
            Some(std::path::Path::new("custom/rules.json"))
        );
    }

    #[test]
    fn test_track_tddd_driver_lint_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingLintService {
            commands: Mutex::new(Vec::new()),
            result: Err("lint failed".to_owned()),
        });
        let outcome = lint_driver(service).handle_lint(lint_input());
        assert_eq!(outcome.stderr.as_deref(), Some("lint failed"));
        assert_eq!(outcome.exit_code, 1);
    }

    #[test]
    fn test_render_lint_result_red_report_preserves_report_and_summary() {
        let outcome = render_lint_result(TrackLintResult {
            violations: vec![domain::CatalogueLintViolation::new(
                "NoPublicField",
                "TrackLintCommand",
                "public field is forbidden",
            )],
        });
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stderr.as_deref(), Some("Found 1 violation(s)"));
        assert!(outcome.stdout.expect("red report is printed").contains("NoPublicField"));
    }

    #[test]
    fn test_track_lint_rules_file_input_try_new_rejects_parent_relative_path() {
        let error = match TrackLintRulesFileInput::try_new(PathBuf::from("../rules.json")) {
            Ok(_) => panic!("parent-relative rules path must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("track lint rules file"));
    }

    struct RecordingTypeSignalsService {
        commands: Mutex<Vec<TrackTypeSignalsCommand>>,
        error: Option<String>,
    }

    impl TrackTypeSignalsService for RecordingTypeSignalsService {
        fn execute(
            &self,
            command: TrackTypeSignalsCommand,
        ) -> Result<TrackTypeSignalsResult, TrackTypeSignalsError> {
            self.commands.lock().expect("command lock is available").push(command);
            match &self.error {
                Some(error) => Err(TrackTypeSignalsError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
                None => Ok(TrackTypeSignalsResult { layers: Vec::new() }),
            }
        }
    }

    fn type_signals_input() -> TrackTdddTypeSignalsInput {
        TrackTdddTypeSignalsInput {
            track_id: Some("signals-track".parse().expect("track id is valid")),
            workspace_root: workspace_root(),
            layer: Some(TrackLayerInput::try_new("usecase".to_owned()).expect("layer is valid")),
        }
    }

    fn type_signals_driver(service: Arc<RecordingTypeSignalsService>) -> TrackTdddDriver {
        TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            Arc::new(UnusedBaselineGraphService),
            Arc::new(UnusedCatalogueImplSignalsService),
            Arc::new(UnusedCatalogueSpecSignalsService),
            Arc::new(UnusedSpecElementHashService),
            Arc::new(UnusedCatalogueLintActiveService),
            Arc::new(UnusedLintService),
            Arc::new(UnusedTrackContractMapService),
            service,
        )
    }

    #[test]
    fn test_track_tddd_driver_type_signals_valid_input_preserves_empty_success_contract() {
        let service =
            Arc::new(RecordingTypeSignalsService { commands: Mutex::new(Vec::new()), error: None });
        let outcome =
            type_signals_driver(service.clone()).handle_type_signals(type_signals_input());

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.is_none());
        assert!(outcome.stderr.is_none());
        let commands = service.commands.lock().expect("command lock is available");
        assert_eq!(commands.len(), 1);
        let command = commands.first().expect("one command is recorded");
        assert!(matches!(
            &command.track,
            TrackSelection::Explicit(track_id) if track_id.as_ref() == "signals-track"
        ));
        assert_eq!(command.workspace_root.as_path(), std::path::Path::new("workspace"));
        assert!(
            matches!(&command.layer, TrackLayerSelection::One(layer) if layer.as_ref() == "usecase")
        );
    }

    #[test]
    fn test_track_tddd_driver_type_signals_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingTypeSignalsService {
            commands: Mutex::new(Vec::new()),
            error: Some("type-signals failed".to_owned()),
        });
        let outcome = type_signals_driver(service).handle_type_signals(type_signals_input());

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stderr.as_deref(), Some("type-signals failed"));
    }
}
