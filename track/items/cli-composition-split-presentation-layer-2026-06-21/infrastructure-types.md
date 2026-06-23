<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodeFragmentExtractorAdapter | secondary_adapter | — | impl CodeFragmentExtractorPort, impl Default, impl Debug | 🔵 | 🔵 |
| FsArchAdapter | secondary_adapter | — | impl ArchPort, impl Default | 🟡 | 🔵 |
| FsArchivedTrackTelemetryAdapter | secondary_adapter | — | impl ArchivedTrackTelemetryPort | 🔵 | 🔵 |
| FsConventionsAdapter | secondary_adapter | — | impl ConventionsPort, impl Default | 🟡 | 🔵 |
| FsDemoAdapter | secondary_adapter | — | impl DemoPort, impl Default | 🟡 | 🔵 |
| FsDryCorpusMetaAdapter | secondary_adapter | — | impl DryCorpusMetaPort | 🔵 | 🔵 |
| FsFileWriteAdapter | secondary_adapter | — | impl FileWritePort, impl Default | 🟡 | 🔵 |
| FsGitWorkflowAdapter | secondary_adapter | — | impl GitWorkflowService, impl Default | 🟡 | 🔵 |
| FsRefVerifyAggregateAdapter | secondary_adapter | — | impl RefVerifyAggregateService, impl Default | 🟡 | 🔵 |
| FsRefVerifyCheckApprovedAdapter | secondary_adapter | — | impl RefVerifyCheckApprovedDriverService, impl Default | 🟡 | 🔵 |
| FsRefVerifyGateStateAdapter | secondary_adapter | — | impl RefVerifyGateStatePort | 🔵 | 🔵 |
| FsRefVerifyRunAdapter | secondary_adapter | — | impl RefVerifyRunService, impl Default | 🟡 | 🔵 |
| FsReviewGateStateAdapter | secondary_adapter | — | impl ReviewGateStatePort | 🔵 | 🔵 |
| FsSpecFileWriterAdapter | secondary_adapter | — | impl SpecFileWriterPort, impl Default | 🔵 | 🔵 |
| FsTelemetryEmitDynamicAdapter | secondary_adapter | — | impl TelemetryEmitDynamicPort, impl Default | 🟡 | 🔵 |
| FsTelemetryReportAdapter | secondary_adapter | — | impl TelemetryReportPort, impl Default | 🟡 | 🔵 |
| FsVerifyAdapter | secondary_adapter | — | impl VerifyPort, impl Default, impl Debug | 🟡 | 🔵 |
| GitDryCheckDiffGetter | secondary_adapter | modify | impl DryCheckDiffSource, impl Debug | 🔵 | 🔵 |
| NoOpDryApprovalService | secondary_adapter | — | impl DryCheckApprovalService | 🔵 | 🔵 |
| NoopSemanticIndexPort | secondary_adapter | — | impl SemanticIndexPort | 🔵 | 🔵 |
| NullInsertIndexProxy | secondary_adapter | — | impl SemanticIndexPort | 🔵 | 🔵 |
| RecordingDryAgent | secondary_adapter | — | impl DryCheckAgentPort | 🔵 | 🔵 |
| SystemSleepAdapter | secondary_adapter | — | impl SleepPort, impl Debug, impl Clone, impl Copy, impl Default | 🔵 | 🔵 |

