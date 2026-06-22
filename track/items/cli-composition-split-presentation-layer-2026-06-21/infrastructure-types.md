<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodeFragmentExtractorAdapter | secondary_adapter | — | impl CodeFragmentExtractorPort, impl Default, impl Debug | 🔵 | 🔵 |
| FsArchivedTrackTelemetryAdapter | secondary_adapter | — | impl ArchivedTrackTelemetryPort | 🔵 | 🔵 |
| FsRefVerifyGateStateAdapter | secondary_adapter | — | impl RefVerifyGateStatePort | 🔵 | 🔵 |
| FsReviewGateStateAdapter | secondary_adapter | — | impl ReviewGateStatePort | 🔵 | 🔵 |
| FsSpecFileWriterAdapter | secondary_adapter | — | impl SpecFileWriterPort, impl Default | 🔵 | 🔵 |
| GitDryCheckDiffGetter | secondary_adapter | modify | impl DryCheckDiffSource, impl Debug | 🔵 | 🔵 |
| NoOpDryApprovalService | secondary_adapter | — | impl DryCheckApprovalService | 🔵 | 🔵 |
| NoopSemanticIndexPort | secondary_adapter | — | impl SemanticIndexPort | 🔵 | 🔵 |
| NullInsertIndexProxy | secondary_adapter | — | impl SemanticIndexPort | 🔵 | 🔵 |
| RecordingDryAgent | secondary_adapter | — | impl DryCheckAgentPort | 🔵 | 🔵 |

