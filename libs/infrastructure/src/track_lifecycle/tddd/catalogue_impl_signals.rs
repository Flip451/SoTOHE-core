//! System adapter for the Track TDDD catalogue-implementation signals port.

use std::sync::{Arc, Mutex};

use domain::FreeText;
use domain::TrackId;
use domain::tddd::catalogue_v2::CatalogueItemNamespace;
use domain::tddd::signal_evaluator::{
    Phase1Error, SignalEvaluatorPort, SignalRegion, ThreeWayEvaluationReport, ThreeWaySignal,
    ThreeWaySignalIdentity, ThreeWaySignalKind,
};
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

/// Captures only the typed identity discriminant while preserving the production evaluator call.
///
/// `CatalogueImplSignalsInteractor` currently exposes formatted Markdown because that is the
/// usecase service's presentation contract. The track-lifecycle port, however, returns typed
/// signals. Keep compact namespace metadata alongside that presentation output so this adapter
/// does not reconstruct catalogue identities from a lossy item label or retain another full
/// evaluation report.
#[derive(Debug, Clone, Copy)]
enum CapturedSignalIdentity {
    CatalogueItem { namespace: CatalogueItemNamespace },
    Label,
}

impl CapturedSignalIdentity {
    fn into_signal(self, item_name: String, region: SignalRegion) -> ThreeWaySignal {
        match self {
            Self::CatalogueItem { namespace } => {
                ThreeWaySignal::catalogue_item(FreeText::new(item_name), namespace, region)
            }
            Self::Label => ThreeWaySignal::label(FreeText::new(item_name), region),
        }
    }
}

fn capture_signal_identities(report: &ThreeWayEvaluationReport) -> Vec<CapturedSignalIdentity> {
    report
        .iter()
        .map(|signal| match signal.identity() {
            ThreeWaySignalIdentity::CatalogueItem { namespace, .. } => {
                CapturedSignalIdentity::CatalogueItem { namespace: *namespace }
            }
            ThreeWaySignalIdentity::Label { .. } => CapturedSignalIdentity::Label,
        })
        .collect()
}

struct CapturingSignalEvaluator {
    delegate: Arc<dyn SignalEvaluatorPort>,
    identities: Arc<Mutex<Vec<Vec<CapturedSignalIdentity>>>>,
}

impl SignalEvaluatorPort for CapturingSignalEvaluator {
    fn evaluate(
        &self,
        a: domain::tddd::ExtendedCrate,
        b: rustdoc_types::Crate,
        c: rustdoc_types::Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        let report = self.delegate.evaluate(a, b, c)?;
        self.identities
            .lock()
            .map_err(|_| {
                Phase1Error::rustdoc_root_resolution("signal report capture lock poisoned")
            })?
            .push(capture_signal_identities(&report));
        Ok(report)
    }
}

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
        let evaluator: Arc<dyn SignalEvaluatorPort> =
            Arc::new(crate::tddd::signal_evaluator_v2::SignalEvaluatorV2::with_workspace_root(
                workspace_root.clone(),
            ));
        let captured_identities = Arc::new(Mutex::new(Vec::new()));
        let capturing_evaluator = Arc::new(CapturingSignalEvaluator {
            delegate: evaluator,
            identities: Arc::clone(&captured_identities),
        });
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
            capturing_evaluator,
            rustdoc_crate_port,
            layer_bindings_port,
            feature_declaration_port,
            symlink_guard,
        );
        let report = interactor
            .run(track_id.to_string(), workspace_root, layer)
            .map_err(|error| execution_failed(error.to_string()))?;
        let captured_identities = {
            let mut identities = captured_identities
                .lock()
                .map_err(|_| execution_failed("signal report capture lock poisoned"))?;
            std::mem::take(&mut *identities)
        };
        let layers =
            parse_report_layers(&report.text, captured_identities).map_err(execution_failed)?;

        Ok(TrackCatalogueImplSignalsResult { layers })
    }
}

struct ParsedReportRow {
    item_name: String,
    region: SignalRegion,
    reported_signal: String,
}

struct ParsedReportLayer {
    layer: domain::tddd::LayerId,
    rows: Vec<ParsedReportRow>,
}

fn parse_report_layers(
    report: &str,
    captured_identities: Vec<Vec<CapturedSignalIdentity>>,
) -> Result<Vec<TrackCatalogueImplLayerResult>, String> {
    let mut parsed_layers = Vec::new();
    let mut current_layer: Option<ParsedReportLayer> = None;

    for raw_line in report.lines() {
        let line = raw_line.trim();
        if let Some(layer_name) =
            line.strip_prefix("## Layer: `").and_then(|value| value.strip_suffix('`'))
        {
            if let Some(previous_layer) = current_layer.take() {
                parsed_layers.push(previous_layer);
            }
            let layer = domain::tddd::LayerId::try_new(layer_name.to_owned())
                .map_err(|error| format!("invalid layer in catalogue signal report: {error}"))?;
            current_layer = Some(ParsedReportLayer { layer, rows: Vec::new() });
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
        if reported_signal == signal_label(ThreeWaySignalKind::Skip) {
            return Err("catalogue signal report must not contain skipped signals".to_owned());
        }
        current.rows.push(ParsedReportRow {
            item_name: item_name.to_owned(),
            region,
            reported_signal: reported_signal.to_owned(),
        });
    }

    if let Some(last_layer) = current_layer {
        parsed_layers.push(last_layer);
    }
    if parsed_layers.is_empty() {
        return Err("catalogue signal report contains no layer sections".to_owned());
    }
    if parsed_layers.len() != captured_identities.len() {
        return Err(format!(
            "catalogue signal report contains {} layer sections but the evaluator returned {} identity sets",
            parsed_layers.len(),
            captured_identities.len()
        ));
    }

    let mut layers = Vec::with_capacity(parsed_layers.len());
    for (parsed_layer, identities) in parsed_layers.into_iter().zip(captured_identities) {
        if parsed_layer.rows.len() != identities.len() {
            return Err(format!(
                "catalogue signal report layer `{}` contains {} rows but the evaluator returned {} identities",
                parsed_layer.layer,
                parsed_layer.rows.len(),
                identities.len()
            ));
        }
        let mut signals = Vec::with_capacity(identities.len());
        for (index, (row, identity)) in parsed_layer.rows.into_iter().zip(identities).enumerate() {
            let ParsedReportRow { item_name, region, reported_signal } = row;
            let typed_signal = identity.into_signal(item_name, region);
            if reported_signal != signal_label(typed_signal.signal()) {
                return Err(format!(
                    "catalogue signal report row {index} in layer `{}` does not match the typed evaluator report",
                    parsed_layer.layer
                ));
            }
            signals.push(typed_signal);
        }
        layers.push(TrackCatalogueImplLayerResult { layer: parsed_layer.layer, signals });
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
    use domain::FreeText;
    use domain::tddd::catalogue_v2::CatalogueItemNamespace;
    use domain::tddd::signal_evaluator::ThreeWaySignal;
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
        let evaluation_report = ThreeWayEvaluationReport::new(vec![ThreeWaySignal::label(
            FreeText::new("TrackInitService"),
            SignalRegion::SIntersectC_Match_Add,
        )]);

        let layers =
            parse_report_layers(report, vec![capture_signal_identities(&evaluation_report)])
                .expect("valid report parses");
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
        let evaluation_report = ThreeWayEvaluationReport::new(vec![ThreeWaySignal::label(
            FreeText::new("Foo<'|'>: Trait"),
            SignalRegion::SIntersectC_Match_Add,
        )]);

        let layers =
            parse_report_layers(report, vec![capture_signal_identities(&evaluation_report)])
                .expect("item delimiters must remain parseable");
        let signal =
            layers.first().and_then(|layer| layer.signals.first()).expect("one signal is present");

        assert_eq!(signal.item_name(), "Foo<'|'>: Trait");
        assert_eq!(signal.region(), SignalRegion::SIntersectC_Match_Add);
    }

    #[test]
    fn test_parse_report_layers_rejects_unknown_signal_region() {
        let report = "## Layer: `usecase`\n\n| Item | Region | Signal |\n|------|--------|--------|\n| TrackInitService | UnknownRegion | 🔵 Blue |\n";

        let error = match parse_report_layers(report, Vec::new()) {
            Ok(_) => panic!("unknown region must fail closed"),
            Err(error) => error,
        };

        assert!(error.contains("unknown signal region"));
    }

    #[test]
    fn test_parse_report_layers_preserves_same_named_type_and_trait_identities() {
        let report = "## Layer: `domain`\n\n| Item | Region | Signal |\n|------|--------|--------|\n| Shared | SMinusC_Add | 🟡 Yellow |\n| Shared | SMinusC_Add | 🟡 Yellow |\n";
        let evaluation_report = ThreeWayEvaluationReport::new(vec![
            ThreeWaySignal::catalogue_item(
                FreeText::new("Shared"),
                CatalogueItemNamespace::Type,
                SignalRegion::SMinusC_Add,
            ),
            ThreeWaySignal::catalogue_item(
                FreeText::new("Shared"),
                CatalogueItemNamespace::Trait,
                SignalRegion::SMinusC_Add,
            ),
        ]);

        let layers =
            parse_report_layers(report, vec![capture_signal_identities(&evaluation_report)])
                .expect("typed evaluator reports must survive Markdown parsing");
        let signals = &layers.first().expect("one layer is present").signals;

        assert_eq!(signals.len(), 2);
        let first = signals.first().expect("type signal is present");
        let second = signals.get(1).expect("trait signal is present");
        assert_eq!(first.item_name(), "Shared");
        assert_eq!(second.item_name(), "Shared");
        assert_eq!(first.namespace(), Some(CatalogueItemNamespace::Type));
        assert_eq!(second.namespace(), Some(CatalogueItemNamespace::Trait));
    }
}
