//! Task-status attribution and unresolved-finding aggregation for the
//! test-obligation check and informational results surfaces.

use std::collections::HashMap;

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::tddd::semantic_verify::CatalogueEntryKey;
use domain::tddd::test_obligation::ids::TestObligationEdgeId;
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::{TaskId, TaskStatusKind, TrackId};

use crate::pre_review_gate::{ImplPlanReaderPort, TaskContractReaderPort};

use super::{LoadedCatalogueDocument, diag};

/// A catalogue entry qualified by the layer that owns its task-contract row.
///
/// Test-obligation ids deliberately use only an entry key, but task-contract
/// attribution is defined over `(layer, entry_key)`. Keeping the layer here
/// prevents same-named entries from different catalogues being conflated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StatusLaneTarget {
    layer: LayerId,
    entry_key: CatalogueEntryKey,
}

impl StatusLaneTarget {
    fn new(layer: LayerId, entry_key: CatalogueEntryKey) -> Self {
        Self { layer, entry_key }
    }

    fn layer(&self) -> &LayerId {
        &self.layer
    }

    fn entry_key(&self) -> &CatalogueEntryKey {
        &self.entry_key
    }
}

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
    target: StatusLaneTarget,
    kind: StatusLaneFindingKind,
}

impl StatusLaneFinding {
    /// Builds a status-attributable unresolved finding.
    #[must_use]
    pub(super) fn new(target: StatusLaneTarget, kind: StatusLaneFindingKind) -> Self {
        Self { target, kind }
    }

    /// Returns the source-qualified target catalogue entry.
    #[must_use]
    pub(super) fn target(&self) -> &StatusLaneTarget {
        &self.target
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
    statuses: Vec<(StatusLaneTarget, TaskStatusKind)>,
}

impl TaskStatusAttributor {
    /// Loads the two existing task artifacts and resolves `(layer, entry_key)
    /// → status` for every supplied target. Missing attribution is structural
    /// and is therefore returned to the caller as a fail-closed diagnostic.
    pub(super) fn load(
        task_contract_reader: &dyn TaskContractReaderPort,
        impl_plan_reader: &dyn ImplPlanReaderPort,
        track_id: &TrackId,
        targets: &[StatusLaneTarget],
    ) -> Result<Self, domain::tddd::test_obligation::ids::DiagnosticMessage> {
        let contract = task_contract_reader
            .read(track_id)
            .map_err(|error| diag(&format!("task-contract attribution read failed: {error}")))?;
        let task_statuses = impl_plan_reader
            .read_task_statuses(track_id)
            .map_err(|error| diag(&format!("impl-plan attribution read failed: {error}")))?;

        let mut statuses = Vec::new();
        for target in targets {
            if statuses.iter().any(|(known, _)| known == target) {
                continue;
            }
            let task_ids = task_ids_for_entry(&contract, target.layer(), target.entry_key());
            if task_ids.is_empty() {
                return Err(diag(&format!(
                    "entry '{}' in layer '{}' has no task attribution",
                    target.entry_key().as_str(),
                    target.layer().as_ref()
                )));
            }
            let status = strictest_status(&task_ids, &task_statuses).ok_or_else(|| {
                diag(&format!(
                    "entry '{}' references a task absent from impl-plan.json",
                    target.entry_key().as_str()
                ))
            })?;
            statuses.push((target.clone(), status));
        }
        Ok(Self { statuses })
    }

    /// Returns the resolved status for one already-validated target entry.
    pub(super) fn status_for(
        &self,
        target: &StatusLaneTarget,
    ) -> Result<TaskStatusKind, domain::tddd::test_obligation::ids::DiagnosticMessage> {
        self.statuses
            .iter()
            .find(|(known, _)| known == target)
            .map(|(_, status)| *status)
            .ok_or_else(|| {
                diag(&format!(
                    "entry '{}' in layer '{}' was not included in task attribution",
                    target.entry_key().as_str(),
                    target.layer().as_ref()
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
        let status = attributor.status_for(finding.target())?;
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

/// Resolves the recorded source catalogue of a derived obligation to its
/// layer-qualified task-contract target.
pub(super) fn target_for_obligation(
    catalogues: &[LoadedCatalogueDocument],
    obligation: &TestObligation,
) -> Result<StatusLaneTarget, domain::tddd::test_obligation::ids::DiagnosticMessage> {
    let layers = catalogues
        .iter()
        .filter(|catalogue| catalogue.matches_file_path(&obligation.target_entry().file_path))
        .map(|catalogue| catalogue.document().layer().clone())
        .collect();
    let Some(layer) = exactly_one(layers) else {
        return Err(diag(&format!(
            "cannot uniquely resolve catalogue origin for entry '{}'",
            obligation.id().entry_key().as_str()
        )));
    };
    Ok(StatusLaneTarget::new(layer, obligation.id().entry_key().clone()))
}

/// Resolves a non-derived cited edge, whose edge id does not retain a source
/// catalogue path, only when its entry key has one active catalogue origin.
pub(super) fn target_for_direct_edge(
    catalogues: &[LoadedCatalogueDocument],
    edge: &TestObligationEdgeId,
) -> Result<StatusLaneTarget, domain::tddd::test_obligation::ids::DiagnosticMessage> {
    let layers = catalogues
        .iter()
        .filter(|catalogue| catalogue_contains_active(catalogue.document(), edge.entry_key()))
        .map(|catalogue| catalogue.document().layer().clone())
        .collect();
    let Some(layer) = exactly_one(layers) else {
        return Err(diag(&format!(
            "cannot uniquely resolve catalogue layer for direct entry '{}'",
            edge.entry_key().as_str()
        )));
    };
    Ok(StatusLaneTarget::new(layer, edge.entry_key().clone()))
}

/// Resolves every active entry in the current obligation scope to the target
/// shape that task-contract uses for attribution.
pub(super) fn targets_for_scope(
    obligations: &ObligationsDocument,
    cited_edges: &[TestObligationEdgeId],
    catalogues: &[LoadedCatalogueDocument],
) -> Result<Vec<StatusLaneTarget>, domain::tddd::test_obligation::ids::DiagnosticMessage> {
    let mut targets = obligations
        .obligations()
        .iter()
        .map(|obligation| target_for_obligation(catalogues, obligation))
        .collect::<Result<Vec<_>, _>>()?;
    for edge in cited_edges {
        let is_derived = obligations.obligations().iter().any(|obligation| {
            obligation.id().entry_key() == edge.entry_key()
                && obligation.spec_refs().iter().any(|anchor| anchor == edge.anchor_id())
        });
        if !is_derived {
            targets.push(target_for_direct_edge(catalogues, edge)?);
        }
    }
    Ok(targets)
}

/// Returns whether an active (`Add` / `Modify`) entry owns `entry_key`.
///
/// `Reference` and `Delete` entries never produce direct cited edges, so they
/// must not participate in the origin resolution for one.
fn catalogue_contains_active(document: &CatalogueDocument, entry_key: &CatalogueEntryKey) -> bool {
    document.types().iter().any(|(key, entry)| {
        key.as_str() == entry_key.as_str() && active_entry_action(entry.action())
    }) || document.traits().iter().any(|(key, entry)| {
        key.as_str() == entry_key.as_str() && active_entry_action(entry.action())
    }) || document.functions().iter().any(|(key, entry)| {
        key.to_string() == entry_key.as_str() && active_entry_action(entry.action())
    })
}

/// Returns whether an entry contributes a direct cited edge.
fn active_entry_action(action: ItemAction) -> bool {
    matches!(action, ItemAction::Add | ItemAction::Modify)
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::collections::{BTreeMap, HashMap};
    use std::path::Path;

    use domain::task_contract::{ContractedEntryRef, TaskContractDocument};
    use domain::tddd::LayerId;
    use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
    use domain::tddd::catalogue_v2::{
        CatalogueDocument, CrateName, ModulePath, StructKind, StructShape, TypeEntry, TypeKindV2,
    };
    use domain::tddd::semantic_verify::CatalogueEntryKey;
    use domain::tddd::test_obligation::ids::{TestObligationAnchorId, TestObligationEdgeId};
    use domain::{TaskId, TaskStatusKind, TrackId};

    use super::{
        LoadedCatalogueDocument, strictest_status, target_for_direct_edge, task_ids_for_entry,
    };

    fn catalogue(layer: &str, action: ItemAction) -> CatalogueDocument {
        let mut document = CatalogueDocument::new(
            5,
            CrateName::new(layer).unwrap(),
            LayerId::try_new(layer).unwrap(),
        );
        document.insert_type(
            CatalogueEntryKey::try_new("SharedEntry".to_owned()).unwrap(),
            TypeEntry::new(
                action,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Some(ModulePath::root()),
                None,
                Vec::new(),
                Vec::new(),
            ),
        );
        document
    }

    #[test]
    fn test_target_for_direct_edge_with_reference_peer_uses_active_origin() {
        let edge = TestObligationEdgeId::new(
            CatalogueEntryKey::try_new("SharedEntry".to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-01".to_owned()).unwrap(),
        );
        let catalogues = vec![
            LoadedCatalogueDocument::new(
                Path::new("domain-types.json"),
                catalogue("domain", ItemAction::Reference),
            ),
            LoadedCatalogueDocument::new(
                Path::new("usecase-types.json"),
                catalogue("usecase", ItemAction::Add),
            ),
        ];

        let target = target_for_direct_edge(&catalogues, &edge).unwrap();

        assert_eq!(target.layer(), &LayerId::try_new("usecase").unwrap());
        assert_eq!(
            target.entry_key(),
            &CatalogueEntryKey::try_new("SharedEntry".to_owned()).unwrap()
        );
    }

    #[test]
    fn test_strictest_status_with_shared_entry_competing_attributions_uses_total_order() {
        let layer = LayerId::try_new("usecase").unwrap();
        let entry_key = CatalogueEntryKey::try_new("SharedEntry".to_owned()).unwrap();
        let strictest_for_shared_entry = |statuses: &[(&str, TaskStatusKind)]| {
            let mut entries = BTreeMap::new();
            let mut plan_statuses = HashMap::new();
            for (task_label, status) in statuses {
                let task_id = TaskId::try_new((*task_label).to_owned()).unwrap();
                entries.insert(
                    task_id.clone(),
                    vec![ContractedEntryRef::new(layer.clone(), entry_key.clone())],
                );
                plan_statuses.insert(task_id, *status);
            }
            let contract = TaskContractDocument::new(
                TrackId::try_new("status-order-track".to_owned()).unwrap(),
                entries,
            )
            .unwrap();
            let attributed_tasks = task_ids_for_entry(&contract, &layer, &entry_key);

            strictest_status(&attributed_tasks, &plan_statuses)
        };

        assert_eq!(
            strictest_for_shared_entry(&[
                ("T001", TaskStatusKind::Done),
                ("T002", TaskStatusKind::Skipped),
            ]),
            Some(TaskStatusKind::Done)
        );
        assert_eq!(
            strictest_for_shared_entry(&[
                ("T001", TaskStatusKind::InProgress),
                ("T002", TaskStatusKind::Skipped),
            ]),
            Some(TaskStatusKind::InProgress)
        );
        assert_eq!(
            strictest_for_shared_entry(&[
                ("T001", TaskStatusKind::Skipped),
                ("T002", TaskStatusKind::Todo),
            ]),
            Some(TaskStatusKind::Skipped)
        );
    }
}
