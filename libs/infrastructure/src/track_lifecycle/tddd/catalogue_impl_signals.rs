//! System adapter for the Track TDDD catalogue-implementation signals port.

use std::sync::Arc;

use domain::TrackId;
use domain::tddd::signal_evaluator::{SignalRegion, ThreeWaySignal, ThreeWaySignalKind};
use usecase::catalogue_impl_signals::{
    CatalogueImplSignalsInteractor, CatalogueImplSignalsService,
};
use usecase::track_lifecycle::tddd::catalogue_impl_signals::{
    TrackCatalogueImplSignalsCommand, TrackCatalogueImplSignalsError,
    TrackCatalogueImplSignalsPort, TrackCatalogueImplSignalsResult,
};
use usecase::track_lifecycle::{TrackCatalogueImplLayerResult, TrackLayerSelection};

/// System-backed adapter for catalogue-to-implementation signal evaluation.
pub struct SystemTrackCatalogueImplSignalsAdapter;

impl TrackCatalogueImplSignalsPort for SystemTrackCatalogueImplSignalsAdapter {
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackCatalogueImplSignalsCommand,
    ) -> Result<TrackCatalogueImplSignalsResult, TrackCatalogueImplSignalsError> {
        let workspace_root = command.workspace_root.as_path().to_path_buf();
        let layer = match command.layer {
            TrackLayerSelection::All => None,
            TrackLayerSelection::One(layer) => Some(layer.to_string()),
        };

        let catalogue_loader =
            Arc::new(crate::tddd::tddd_catalogue_document_loader::FsCatalogueDocumentLoader::new());
        let ext_crate_codec = Arc::new(
            crate::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec::new(),
        );
        let evaluator =
            Arc::new(crate::tddd::signal_evaluator_v2::SignalEvaluatorV2::with_workspace_root(
                workspace_root.clone(),
            ));
        let rustdoc_crate_port = Arc::new(
            crate::tddd::rustdoc_crate_adapter::RustdocCrateAdapter::new(workspace_root.clone()),
        );
        let layer_bindings_port =
            Arc::new(crate::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter::new());
        let feature_declaration_port = Arc::new(
            crate::tddd::feature_declaration_adapter::FsTdddFeatureDeclarationAdapter::new(),
        );
        let symlink_guard = Arc::new(crate::FsSymlinkGuard::new());

        let interactor = CatalogueImplSignalsInteractor::new(
            catalogue_loader,
            ext_crate_codec,
            evaluator,
            rustdoc_crate_port,
            layer_bindings_port,
            feature_declaration_port,
            symlink_guard,
        );
        let report = interactor
            .run(track_id.to_string(), workspace_root, layer)
            .map_err(|error| execution_failed(error.to_string()))?;
        let layers = parse_report_layers(&report.text).map_err(execution_failed)?;

        Ok(TrackCatalogueImplSignalsResult { layers })
    }
}

fn parse_report_layers(report: &str) -> Result<Vec<TrackCatalogueImplLayerResult>, String> {
    let mut layers = Vec::new();
    let mut current_layer: Option<TrackCatalogueImplLayerResult> = None;

    for raw_line in report.lines() {
        let line = raw_line.trim();
        if let Some(layer_name) =
            line.strip_prefix("## Layer: `").and_then(|value| value.strip_suffix('`'))
        {
            if let Some(previous_layer) = current_layer.take() {
                layers.push(previous_layer);
            }
            let layer = domain::tddd::LayerId::try_new(layer_name.to_owned())
                .map_err(|error| format!("invalid layer in catalogue signal report: {error}"))?;
            current_layer = Some(TrackCatalogueImplLayerResult { layer, signals: Vec::new() });
            continue;
        }

        if line == "| Item | Region | Signal |" || line == "|------|--------|--------|" {
            continue;
        }
        if !line.starts_with('|') {
            continue;
        }

        let current = current_layer.as_mut().ok_or_else(|| {
            "catalogue signal report contains a table row before a layer section".to_owned()
        })?;
        let table_row = line
            .strip_prefix('|')
            .and_then(|value| value.strip_suffix('|'))
            .ok_or_else(|| "catalogue signal report contains a malformed table row".to_owned())?;
        // The item identity is emitted verbatim by the existing report formatter and may
        // contain `|` (for example, a char const-generic).  The region and signal are the
        // two fixed rightmost columns, so split from the right and leave the item intact.
        let mut columns = table_row.rsplitn(3, '|').map(str::trim);
        let reported_signal = columns.next().ok_or_else(|| {
            "catalogue signal report contains a table row without a signal".to_owned()
        })?;
        let region_name = columns.next().ok_or_else(|| {
            "catalogue signal report contains a table row without a region".to_owned()
        })?;
        let item_name = columns.next().ok_or_else(|| {
            "catalogue signal report contains a table row without an item".to_owned()
        })?;
        if item_name.is_empty() {
            return Err("catalogue signal report contains an empty item name".to_owned());
        }

        let region = parse_signal_region(region_name)?;
        let signal = ThreeWaySignal::new(item_name.to_owned(), region);
        let expected_signal = signal_label(signal.signal());
        if reported_signal != expected_signal {
            return Err(format!(
                "catalogue signal report signal `{reported_signal}` does not match region `{region_name}`"
            ));
        }
        if signal.signal().is_skip() {
            return Err("catalogue signal report must not contain skipped signals".to_owned());
        }
        current.signals.push(signal);
    }

    if let Some(last_layer) = current_layer {
        layers.push(last_layer);
    }
    if layers.is_empty() {
        return Err("catalogue signal report contains no layer sections".to_owned());
    }
    Ok(layers)
}

fn parse_signal_region(value: &str) -> Result<SignalRegion, String> {
    match value {
        "SIntersectC_Match_Add" => Ok(SignalRegion::SIntersectC_Match_Add),
        "SIntersectC_Match_Modify" => Ok(SignalRegion::SIntersectC_Match_Modify),
        "SIntersectC_Match_Reference" => Ok(SignalRegion::SIntersectC_Match_Reference),
        "SIntersectC_Mismatch_Reference" => Ok(SignalRegion::SIntersectC_Mismatch_Reference),
        "SIntersectC_Mismatch_Add" => Ok(SignalRegion::SIntersectC_Mismatch_Add),
        "SIntersectC_Mismatch_Modify" => Ok(SignalRegion::SIntersectC_Mismatch_Modify),
        "SMinusC_Reference" => Ok(SignalRegion::SMinusC_Reference),
        "SMinusC_Add" => Ok(SignalRegion::SMinusC_Add),
        "SMinusC_Modify" => Ok(SignalRegion::SMinusC_Modify),
        "DIntersectC" => Ok(SignalRegion::DIntersectC),
        "DMinusC" => Ok(SignalRegion::DMinusC),
        "CMinusSUnionD" => Ok(SignalRegion::CMinusSUnionD),
        _ => Err(format!("unknown signal region `{value}` in catalogue signal report")),
    }
}

fn signal_label(signal: ThreeWaySignalKind) -> &'static str {
    match signal {
        ThreeWaySignalKind::Skip => "Skip",
        ThreeWaySignalKind::Blue => "🔵 Blue",
        ThreeWaySignalKind::Yellow => "🟡 Yellow",
        ThreeWaySignalKind::Red => "🔴 Red",
    }
}

fn execution_failed(message: impl Into<String>) -> TrackCatalogueImplSignalsError {
    TrackCatalogueImplSignalsError::ExecutionFailed(usecase::git_workflow::DiagnosticText::new(
        message,
    ))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use usecase::track_lifecycle::{TrackLayerSelection, TrackSelection, TrackWorkspaceRoot};

    #[test]
    fn test_system_track_catalogue_impl_signals_adapter_missing_rules_returns_execution_error() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let track_id = TrackId::try_new("signals-track").expect("track id is valid");
        let command = TrackCatalogueImplSignalsCommand {
            track: TrackSelection::Explicit(track_id.clone()),
            workspace_root: TrackWorkspaceRoot::try_new(workspace.path().to_path_buf())
                .expect("workspace root is valid"),
            layer: TrackLayerSelection::All,
        };

        let error = match SystemTrackCatalogueImplSignalsAdapter.execute(track_id, command) {
            Ok(_) => panic!("missing architecture rules must fail closed"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("layer bindings load failed"));
    }

    #[test]
    fn test_parse_report_layers_preserves_structured_signals() {
        let report = "## Layer: `usecase`\n\n| Item | Region | Signal |\n|------|--------|--------|\n| TrackInitService | SIntersectC_Match_Add | 🔵 Blue |\nSummary: 🔵 1 Blue | 🟡 0 Yellow | 🔴 0 Red\n";

        let layers = parse_report_layers(report).expect("valid report parses");
        let layer = layers.first().expect("one layer is present");
        let signal = layer.signals.first().expect("one signal is present");

        assert_eq!(layers.len(), 1);
        assert_eq!(layer.layer.as_ref(), "usecase");
        assert_eq!(layer.signals.len(), 1);
        assert_eq!(signal.item_name(), "TrackInitService");
        assert_eq!(signal.region(), SignalRegion::SIntersectC_Match_Add);
    }

    #[test]
    fn test_parse_report_layers_item_with_pipe_preserves_structured_signal() {
        let report = "## Layer: `usecase`\n\n| Item | Region | Signal |\n|------|--------|--------|\n| Foo<'|'>: Trait | SIntersectC_Match_Add | 🔵 Blue |\n";

        let layers = parse_report_layers(report).expect("item delimiters must remain parseable");
        let signal =
            layers.first().and_then(|layer| layer.signals.first()).expect("one signal is present");

        assert_eq!(signal.item_name(), "Foo<'|'>: Trait");
        assert_eq!(signal.region(), SignalRegion::SIntersectC_Match_Add);
    }

    #[test]
    fn test_parse_report_layers_rejects_unknown_signal_region() {
        let report = "## Layer: `usecase`\n\n| Item | Region | Signal |\n|------|--------|--------|\n| TrackInitService | UnknownRegion | 🔵 Blue |\n";

        let error = match parse_report_layers(report) {
            Ok(_) => panic!("unknown region must fail closed"),
            Err(error) => error,
        };

        assert!(error.contains("unknown signal region"));
    }
}
