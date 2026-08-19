<!-- Generated from cli_driver-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackResolutionInput | enum | add | ReadFromItems, ReadFromRoot, WriteFromItems, WriteFromRoot, DetectActive | 🔵 | 🔵 |
| TrackResolutionOutcome | enum | add | Resolved, Inactive, Failed | 🔵 | 🔵 |
| TrackTdddInput | enum | add | TypeSignals, TypeGraph, BaselineGraph, ContractMap, CatalogueSpecSignals, SpecElementHash, BaselineCapture, Lint, CatalogueImplSignals, CatalogueLintActive | 🟡 | 🔵 |
| TrackTypeGraphEdgeInput | enum | add | Methods, Fields, Impls, All | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackIdInput | dto | reference | — | 🔵 | 🔵 |
| TrackItemsDirectoryInput | dto | add | — | 🔵 | 🔵 |
| TrackLayerInput | dto | add | — | 🔵 | 🔵 |
| TrackLayersInput | dto | add | — | 🔵 | 🔵 |
| TrackLintRulesFileInput | dto | add | — | 🔵 | 🔵 |
| TrackResolutionDiagnostic | dto | add | — | 🔵 | 🔵 |
| TrackSourceWorkspaceInput | dto | add | — | 🔵 | 🔵 |
| TrackSpecAnchorInput | dto | add | — | 🔵 | 🔵 |
| TrackTdddBaselineCaptureInput | dto | add | — | 🔵 | 🔵 |
| TrackTdddBaselineGraphInput | dto | add | — | 🔵 | 🔵 |
| TrackTdddCatalogueImplSignalsInput | dto | add | — | 🔵 | 🔵 |
| TrackTdddCatalogueLintActiveInput | dto | add | — | 🔵 | 🔵 |
| TrackTdddCatalogueSpecSignalsInput | dto | add | — | 🔵 | 🔵 |
| TrackTdddContractMapInput | dto | add | — | 🔵 | 🔵 |
| TrackTdddLintInput | dto | add | — | 🔵 | 🔵 |
| TrackTdddSpecElementHashInput | dto | add | — | 🔵 | 🔵 |
| TrackTdddTypeGraphInput | dto | add | — | 🟡 | 🔵 |
| TrackTdddTypeSignalsInput | dto | add | — | 🔵 | 🔵 |
| TrackTypeGraphClusterDepthInput | dto | add | — | 🟡 | 🔵 |
| TrackWorkspaceRootInput | dto | add | — | 🟡 | 🔵 |

## Primary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackDriver | primary_adapter | modify | — | 🟡 | 🔵 |
| TrackResolutionDriver | primary_adapter | add | — | 🔵 | 🔵 |
| TrackTdddDriver | primary_adapter | add | — | 🟡 | 🔵 |

