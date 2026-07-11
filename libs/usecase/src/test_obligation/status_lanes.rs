//! Task-status attribution and unresolved-finding aggregation for the
//! test-obligation check and informational results surfaces.

use std::collections::HashMap;

use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::semantic_verify::CatalogueEntryKey;
use domain::{TaskId, TaskStatusKind, TrackId};

use crate::pre_review_gate::{ImplPlanReaderPort, TaskContractReaderPort};

use super::{LoadedCatalogueDocument, diag};

/// The category of an unresolved edge reported in a task-status lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StatusLaneFindingKind {
    Missing,
    Stale,
    VerdictAbsent,
}

/// An unresolved finding attributed by its catalogue entry key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StatusLaneFinding {
    entry_key: CatalogueEntryKey,
    kind: StatusLaneFindingKind,
}

impl StatusLaneFinding {
    /// Builds a status-attributable unresolved finding.
    #[must_use]
    pub(super) fn new(entry_key: CatalogueEntryKey, kind: StatusLaneFindingKind) -> Self {
        Self { entry_key, kind }
    }

    /// Returns the target catalogue entry key.
    #[must_use]
    pub(super) fn entry_key(&self) -> &CatalogueEntryKey {
        &self.entry_key
    }

    /// Returns the unresolved category.
    #[must_use]
    pub(super) fn kind(&self) -> StatusLaneFindingKind {
        self.kind
    }
}

/// Informational counts for one deterministic task-status lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StatusLaneTally {
    task_status: TaskStatusKind,
    missing_count: usize,
    stale_count: usize,
    verdict_absent_count: usize,
}

impl StatusLaneTally {
    fn new(task_status: TaskStatusKind) -> Self {
        Self { task_status, missing_count: 0, stale_count: 0, verdict_absent_count: 0 }
    }

    /// Returns the status represented by this tally.
    #[must_use]
    pub(super) fn task_status(&self) -> TaskStatusKind {
        self.task_status
    }

    /// Returns the count of missing bindings or test sources.
    #[must_use]
    pub(super) fn missing_count(&self) -> usize {
        self.missing_count
    }

    /// Returns the count of hash-stale edges.
    #[must_use]
    pub(super) fn stale_count(&self) -> usize {
        self.stale_count
    }

    /// Returns the count of unavailable current verdicts.
    #[must_use]
    pub(super) fn verdict_absent_count(&self) -> usize {
        self.verdict_absent_count
    }
}

/// Resolves every relevant catalogue entry to its strictest task status.
pub(super) struct TaskStatusAttributor {
    statuses: Vec<(CatalogueEntryKey, TaskStatusKind)>,
}

impl TaskStatusAttributor {
    /// Loads the two existing task artifacts and resolves `entry_key → status`
    /// for every supplied entry. Missing or ambiguous attribution is structural
    /// and is therefore returned to the caller as a fail-closed diagnostic.
    pub(super) fn load(
        task_contract_reader: &dyn TaskContractReaderPort,
        impl_plan_reader: &dyn ImplPlanReaderPort,
        track_id: &TrackId,
        catalogues: &[LoadedCatalogueDocument],
        entry_keys: &[CatalogueEntryKey],
    ) -> Result<Self, domain::tddd::test_obligation::ids::DiagnosticMessage> {
        let contract = task_contract_reader
            .read(track_id)
            .map_err(|error| diag(&format!("task-contract attribution read failed: {error}")))?;
        let task_statuses = impl_plan_reader
            .read_task_statuses(track_id)
            .map_err(|error| diag(&format!("impl-plan attribution read failed: {error}")))?;

        let mut statuses = Vec::new();
        for entry_key in entry_keys {
            if statuses.iter().any(|(known, _)| known == entry_key) {
                continue;
            }
            let layers = layers_for_entry(catalogues, entry_key);
            let Some(layer) = exactly_one(layers) else {
                return Err(diag(&format!(
                    "cannot uniquely resolve catalogue layer for entry '{}'",
                    entry_key.as_str()
                )));
            };
            let task_ids = task_ids_for_entry(&contract, &layer, entry_key);
            if task_ids.is_empty() {
                return Err(diag(&format!(
                    "entry '{}' in layer '{}' has no task attribution",
                    entry_key.as_str(),
                    layer.as_ref()
                )));
            }
            let status = strictest_status(&task_ids, &task_statuses).ok_or_else(|| {
                diag(&format!(
                    "entry '{}' references a task absent from impl-plan.json",
                    entry_key.as_str()
                ))
            })?;
            statuses.push((entry_key.clone(), status));
        }
        Ok(Self { statuses })
    }

    /// Returns the resolved status for one already-validated target entry.
    pub(super) fn status_for(
        &self,
        entry_key: &CatalogueEntryKey,
    ) -> Result<TaskStatusKind, domain::tddd::test_obligation::ids::DiagnosticMessage> {
        self.statuses
            .iter()
            .find(|(known, _)| known == entry_key)
            .map(|(_, status)| *status)
            .ok_or_else(|| {
                diag(&format!(
                    "entry '{}' was not included in task attribution",
                    entry_key.as_str()
                ))
            })
    }
}

/// Aggregates unresolved findings into all four task-status lanes.
pub(super) fn tally_findings(
    attributor: &TaskStatusAttributor,
    findings: &[StatusLaneFinding],
) -> Result<Vec<StatusLaneTally>, domain::tddd::test_obligation::ids::DiagnosticMessage> {
    let mut tallies = vec![
        StatusLaneTally::new(TaskStatusKind::Todo),
        StatusLaneTally::new(TaskStatusKind::InProgress),
        StatusLaneTally::new(TaskStatusKind::Done),
        StatusLaneTally::new(TaskStatusKind::Skipped),
    ];
    for finding in findings {
        let status = attributor.status_for(finding.entry_key())?;
        let Some(tally) = tallies.iter_mut().find(|tally| tally.task_status == status) else {
            return Err(diag("task status lane is not representable"));
        };
        match finding.kind() {
            StatusLaneFindingKind::Missing => tally.missing_count += 1,
            StatusLaneFindingKind::Stale => tally.stale_count += 1,
            StatusLaneFindingKind::VerdictAbsent => tally.verdict_absent_count += 1,
        }
    }
    Ok(tallies)
}

fn layers_for_entry(
    catalogues: &[LoadedCatalogueDocument],
    entry_key: &CatalogueEntryKey,
) -> Vec<domain::tddd::LayerId> {
    catalogues
        .iter()
        .filter(|catalogue| catalogue_contains(catalogue.document(), entry_key))
        .map(|catalogue| catalogue.document().layer().clone())
        .collect()
}

fn catalogue_contains(document: &CatalogueDocument, entry_key: &CatalogueEntryKey) -> bool {
    document.types().keys().any(|key| key.as_str() == entry_key.as_str())
        || document.traits().keys().any(|key| key.as_str() == entry_key.as_str())
        || document.functions().keys().any(|key| key.to_string() == entry_key.as_str())
}

fn task_ids_for_entry(
    contract: &domain::task_contract::TaskContractDocument,
    layer: &domain::tddd::LayerId,
    entry_key: &CatalogueEntryKey,
) -> Vec<TaskId> {
    contract
        .entries()
        .iter()
        .filter(|(_, entries)| {
            entries.iter().any(|entry| entry.layer() == layer && entry.entry_key() == entry_key)
        })
        .map(|(task_id, _)| task_id.clone())
        .collect()
}

fn exactly_one<T>(mut values: Vec<T>) -> Option<T> {
    match (values.pop(), values.pop()) {
        (Some(value), None) => Some(value),
        (None, _) | (Some(_), Some(_)) => None,
    }
}

fn strictest_status(
    task_ids: &[TaskId],
    statuses: &HashMap<TaskId, TaskStatusKind>,
) -> Option<TaskStatusKind> {
    task_ids
        .iter()
        .map(|task_id| statuses.get(task_id).copied())
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .max_by_key(|status| match status {
            TaskStatusKind::Todo => 0_u8,
            TaskStatusKind::Skipped => 1_u8,
            TaskStatusKind::InProgress => 2_u8,
            TaskStatusKind::Done => 3_u8,
        })
}
