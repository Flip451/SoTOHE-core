//! Waiver-edge cache freshness resolution for the `test-obligation check` gate.

use domain::tddd::test_obligation::drift::TestObligationDrift;
use domain::tddd::test_obligation::errors::ObligationCheckError;
use domain::tddd::test_obligation::hashes::{DeclarationHash, WaivedReasonHash};
use domain::tddd::test_obligation::ids::{
    TestObligationBrief, TestObligationEdgeId, TestObligationId, WaivedReason,
};
use domain::tddd::test_obligation::obligations::TestObligation;
use domain::tddd::test_obligation::verdict::{WaiverCacheDocument, WaiverCacheKey, WaiverVerdict};

use super::super::LoadedCatalogueDocument;
use super::super::check_support::synthetic_voluntary_obligation_id;
use super::super::check_support::{GateState, SpecElement};
use super::super::freshness::{responsibility_hash, spec_element_hash};
use super::super::status_lanes::{StatusLaneFindingKind, StatusLaneTarget};
use super::super::{
    declaration_with_obligation_item, diag, find_declaration_text_from_loaded,
    obligation_declaration_text_from_loaded, sha256_content_hash,
    synthetic_voluntary_obligation_brief,
};
use super::CheckTestObligationsInteractor;

impl CheckTestObligationsInteractor {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve_waiver_edge(
        &self,
        edge: &TestObligationEdgeId,
        obligation: &TestObligation,
        reason: &WaivedReason,
        target: &StatusLaneTarget,
        catalogues: &[LoadedCatalogueDocument],
        spec_elements: &[SpecElement],
        waiver: &WaiverCacheDocument,
        gate: &mut GateState,
    ) -> Result<(), ObligationCheckError> {
        let declaration = declaration_with_obligation_item(
            &obligation_declaration_text_from_loaded(catalogues, obligation).unwrap_or_default(),
            obligation.id().item_identifier().as_str(),
        );
        self.resolve_waiver_cache_entry(
            edge,
            obligation.id(),
            reason,
            &declaration,
            obligation.brief(),
            target,
            spec_elements,
            waiver,
            gate,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve_direct_waiver_edge(
        &self,
        edge: &TestObligationEdgeId,
        reason: &WaivedReason,
        target: &StatusLaneTarget,
        catalogues: &[LoadedCatalogueDocument],
        spec_elements: &[SpecElement],
        waiver: &WaiverCacheDocument,
        gate: &mut GateState,
    ) -> Result<(), ObligationCheckError> {
        let obligation_id = synthetic_voluntary_obligation_id(edge);
        let obligation_brief = match synthetic_voluntary_obligation_brief(edge) {
            Ok(brief) => brief,
            Err(_) => {
                gate.verdict_absent(edge.clone(), target.clone());
                return Ok(());
            }
        };
        let declaration = declaration_with_obligation_item(
            &find_declaration_text_from_loaded(catalogues, edge.entry_key().as_str())
                .unwrap_or_default(),
            obligation_id.item_identifier().as_str(),
        );
        self.resolve_waiver_cache_entry(
            edge,
            &obligation_id,
            reason,
            &declaration,
            &obligation_brief,
            target,
            spec_elements,
            waiver,
            gate,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_waiver_cache_entry(
        &self,
        edge: &TestObligationEdgeId,
        obligation_id: &TestObligationId,
        reason: &WaivedReason,
        declaration: &str,
        obligation_brief: &TestObligationBrief,
        target: &StatusLaneTarget,
        spec_elements: &[SpecElement],
        waiver: &WaiverCacheDocument,
        gate: &mut GateState,
    ) -> Result<(), ObligationCheckError> {
        let current_reason = sha256_content_hash(reason.as_str().as_bytes());
        let current_decl = sha256_content_hash(declaration.as_bytes());
        let current_spec_element = spec_element_hash(spec_elements, edge);
        let current_responsibility = responsibility_hash(obligation_id, obligation_brief);
        let current_key = WaiverCacheKey::new(
            WaivedReasonHash::new(current_reason),
            DeclarationHash::new(current_decl),
            current_spec_element,
            current_responsibility,
        );
        let Some(entry) = waiver
            .lookup_current(edge, obligation_id, &current_key, &self.waiver_verifier_fingerprint)
            .map_err(ObligationCheckError::WaiverCacheLookup)?
        else {
            let mut reason_changed = false;
            let mut declaration_changed = false;
            let mut spec_element_changed = false;
            let mut responsibility_changed = false;
            for candidate in waiver.entries().iter().filter(|candidate| {
                candidate.edge_id() == edge
                    && candidate.obligation_id() == Some(obligation_id)
                    && candidate.verifier_fingerprint() == Some(&self.waiver_verifier_fingerprint)
            }) {
                reason_changed |=
                    candidate.key().waived_reason_hash() != current_key.waived_reason_hash();
                declaration_changed |=
                    candidate.key().declaration_hash() != current_key.declaration_hash();
                spec_element_changed |=
                    candidate.key().spec_element_hash() != current_key.spec_element_hash();
                responsibility_changed |=
                    candidate.key().responsibility_hash() != current_key.responsibility_hash();
            }
            let mut record_stale = |changed, drift| {
                if changed {
                    gate.status_drift(drift, target.clone(), StatusLaneFindingKind::Stale);
                }
            };
            record_stale(
                reason_changed,
                TestObligationDrift::reason_changed_edge(
                    edge.clone(),
                    diag("waived reason changed since the verdict was frozen"),
                ),
            );
            record_stale(
                declaration_changed,
                TestObligationDrift::decl_changed_edge(
                    edge.clone(),
                    diag("entry declaration changed since the verdict was frozen"),
                ),
            );
            record_stale(
                spec_element_changed,
                TestObligationDrift::spec_changed_edge(
                    edge.clone(),
                    diag("anchor text changed since the verdict was frozen"),
                ),
            );
            record_stale(
                responsibility_changed,
                TestObligationDrift::decl_changed_edge(
                    edge.clone(),
                    diag("obligation responsibility changed since the verdict was frozen"),
                ),
            );
            if !reason_changed
                && !declaration_changed
                && !spec_element_changed
                && !responsibility_changed
            {
                gate.verdict_absent(edge.clone(), target.clone());
            }
            return Ok(());
        };
        if matches!(entry.verdict(), WaiverVerdict::Waived { .. }) {
            gate.resolved.push(edge.clone());
        } else {
            gate.verdict_absent(edge.clone(), target.clone());
        }
        Ok(())
    }
}
