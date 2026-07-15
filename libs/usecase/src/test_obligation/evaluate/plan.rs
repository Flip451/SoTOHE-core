//! Plan / apply split for the evaluate lane.
//!
//! `bin/sotp test-obligation evaluate` originally interleaved cache lookup,
//! LLM subprocess dispatch, and tally recording in one serial async loop —
//! every pair blocked on its own verifier call before the next pair started.
//! To fan the LLM calls out under a bounded concurrency ceiling while keeping
//! cache documents byte-deterministic, the flow is split into three phases:
//!
//! 1. **Plan** — [`plan_binding_records`] walks the binding records once and
//!    resolves each edge into a [`PlannedAction`]: either an *immediate*
//!    outcome (pending / cache hit) whose tally impact is already known, or
//!    an *LLM task* carrying every input the verifier subprocess needs.
//! 2. **Fan out** — the caller collects each `LlmTask` future returned by
//!    [`fulfillment_llm_future`] / [`waiver_llm_future`] and drives them via
//!    [`super::concurrency::drive_bounded_in_order`], which returns the
//!    verdicts in the plan's original order.
//! 3. **Apply** — [`apply_planned`] folds the actions and verdicts back into
//!    the tally + cache-entry vectors in plan order, preserving the byte
//!    layout the serial loop produced.

use domain::SpecDocument;
use domain::tddd::semantic_verify::ModelTier;
use domain::tddd::test_obligation::binding::{
    TestBindingRecord, TestBindingsDocument, TestLocation,
};
use domain::tddd::test_obligation::errors::{ObligationEvaluateError, TestSourceScanError};
use domain::tddd::test_obligation::hashes::{
    AnchorTextHash, BoundTestsSetHash, DeclarationHash, WaivedReasonHash,
};
use domain::tddd::test_obligation::ids::{TestObligationEdgeId, TestObligationId, WaivedReason};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::pair::{
    AnchorText, EntryDeclaration, ObligationFulfillmentPair, TestsSource, WaiverPair,
};
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheEntry,
    ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict, WaiverCacheDocument,
    WaiverCacheEntry, WaiverCacheKey, WaiverVerdict,
};

use super::cache::{cached_fulfillment_verdict, cached_waiver_verdict};
use super::edges::{find_obligation, resolve_anchor_text, synthetic_obligation_id};
use super::verify::{
    map_verifier_error, record_fulfillment, record_pending_edge, record_pending_obligation_edges,
    record_pending_obligation_id, record_waiver,
};
use super::{EvaluateTestObligationsInteractor, Tally};
use crate::test_obligation::{
    LoadedCatalogueDocument, find_declaration_text_from_loaded,
    obligation_declaration_text_from_loaded,
};

/// A single plan step — either an outcome we already know, or an LLM task the
/// caller must dispatch through the escalation driver.
pub(super) enum PlannedAction {
    /// Applied synchronously in [`apply_planned`]; carries the tally delta.
    Immediate(ImmediateOutcome),
    /// The verifier subprocess must be run to produce the verdict.
    Fulfillment(FulfillmentLlmTask),
    Waiver(WaiverLlmTask),
}

/// An outcome resolved during planning — pending edges (no LLM needed) and
/// cache hits (verdict already frozen against the same three-hash key).
pub(super) enum ImmediateOutcome {
    PendingObligationId(TestObligationId),
    PendingObligationEdges(TestObligation),
    PendingEdge(TestObligationEdgeId),
    FulfillmentCached {
        edge_id: TestObligationEdgeId,
        obligation_id: TestObligationId,
        key: ObligationFulfillmentCacheKey,
        verdict: ObligationFulfillmentVerdict,
    },
    WaiverCached {
        edge_id: TestObligationEdgeId,
        key: WaiverCacheKey,
        verdict: WaiverVerdict,
    },
}

/// LLM task for the fulfillment lane — every input the verifier needs is
/// materialised so the future is `'static` in its captures.
pub(super) struct FulfillmentLlmTask {
    pub(super) edge_id: TestObligationEdgeId,
    pub(super) obligation_id: TestObligationId,
    pub(super) key: ObligationFulfillmentCacheKey,
    pub(super) tests_source: String,
    pub(super) declaration: String,
    pub(super) anchor_text: String,
}

/// LLM task for the waiver lane.
pub(super) struct WaiverLlmTask {
    pub(super) edge_id: TestObligationEdgeId,
    pub(super) key: WaiverCacheKey,
    pub(super) reason: WaivedReason,
    pub(super) declaration: String,
    pub(super) anchor_text: String,
}

impl EvaluateTestObligationsInteractor {
    /// Walks every binding record once and emits a plan of actions in binding
    /// order. Each record can produce zero (unresolvable) or many (a
    /// multi-anchor fulfillment record) planned actions; the plan order is
    /// what downstream `apply_planned` uses to fold results back in.
    pub(super) fn plan_binding_records(
        &self,
        bindings: &TestBindingsDocument,
        obligations: &ObligationsDocument,
        catalogues: &[LoadedCatalogueDocument],
        spec: &SpecDocument,
        existing_fulfillment_cache: Option<&ObligationFulfillmentCacheDocument>,
        existing_waiver_cache: Option<&WaiverCacheDocument>,
    ) -> Result<Vec<PlannedAction>, ObligationEvaluateError> {
        // Waiver-first precedence is the binding model's rule
        // (TestBindingsDocument::waived_edge_ids); the planner only orchestrates
        // the resolved outcome so evaluate and check cannot diverge on it.
        let records = bindings.records();
        let waived_edges: Vec<&TestObligationEdgeId> = bindings.waived_edge_ids();
        let mut plan = Vec::with_capacity(records.len());
        for record in records {
            match record {
                TestBindingRecord::Fulfillment { obligation_id, tests } => {
                    self.plan_fulfillment_record(
                        obligation_id,
                        tests.as_slice(),
                        obligations,
                        catalogues,
                        spec,
                        existing_fulfillment_cache,
                        &waived_edges,
                        &mut plan,
                    )?;
                }
                TestBindingRecord::VoluntaryBinding { edge_id, tests } => {
                    if waived_edges.contains(&edge_id) {
                        // Same precedence as fulfillment records: the edge's
                        // waiver record owns its adjudication at the check
                        // gate, so a voluntary binding on a waived edge must
                        // not plan a competing fulfillment adjudication.
                        continue;
                    }
                    self.plan_voluntary_record(
                        edge_id,
                        tests.as_slice(),
                        obligations,
                        catalogues,
                        spec,
                        existing_fulfillment_cache,
                        &mut plan,
                    )?;
                }
                TestBindingRecord::Waiver { edge_id, reason } => {
                    self.plan_waiver_record(
                        edge_id,
                        reason,
                        obligations,
                        catalogues,
                        spec,
                        existing_waiver_cache,
                        &mut plan,
                    )?;
                }
            }
        }
        Ok(plan)
    }

    #[allow(clippy::too_many_arguments)]
    fn plan_fulfillment_record(
        &self,
        obligation_id: &TestObligationId,
        tests: &[TestLocation],
        obligations: &ObligationsDocument,
        catalogues: &[LoadedCatalogueDocument],
        spec: &SpecDocument,
        existing_fulfillment_cache: Option<&ObligationFulfillmentCacheDocument>,
        waived_edges: &[&TestObligationEdgeId],
        plan: &mut Vec<PlannedAction>,
    ) -> Result<(), ObligationEvaluateError> {
        let Some(obligation) = find_obligation(obligations, obligation_id) else {
            plan.push(PlannedAction::Immediate(ImmediateOutcome::PendingObligationId(
                obligation_id.clone(),
            )));
            return Ok(());
        };
        let Some(declaration) = obligation_declaration_text_from_loaded(catalogues, obligation)
        else {
            plan.push(PlannedAction::Immediate(ImmediateOutcome::PendingObligationEdges(
                obligation.clone(),
            )));
            return Ok(());
        };
        let declaration_hash = DeclarationHash::new(self.hasher.sha256(declaration.as_bytes()));
        // Multi-anchor fulfillment: one planned action per anchor, in the
        // order the obligation lists them (matches the pre-refactor loop).
        for anchor in obligation.spec_refs() {
            let edge_id =
                TestObligationEdgeId::new(obligation.id().entry_key().clone(), anchor.clone());
            if waived_edges.contains(&&edge_id) {
                // The edge's waiver record owns its adjudication (check-gate
                // precedence); planning a fulfillment pair here would judge
                // the bound tests against a claim the waiver already covers.
                continue;
            }
            let Some(anchor_text) = resolve_anchor_text(spec, anchor.element_id()) else {
                plan.push(PlannedAction::Immediate(ImmediateOutcome::PendingEdge(edge_id)));
                continue;
            };
            self.emit_fulfillment_action(
                edge_id,
                obligation.id().clone(),
                &declaration,
                &anchor_text,
                declaration_hash.clone(),
                tests,
                existing_fulfillment_cache,
                plan,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn plan_voluntary_record(
        &self,
        edge_id: &TestObligationEdgeId,
        tests: &[TestLocation],
        obligations: &ObligationsDocument,
        catalogues: &[LoadedCatalogueDocument],
        spec: &SpecDocument,
        existing_fulfillment_cache: Option<&ObligationFulfillmentCacheDocument>,
        plan: &mut Vec<PlannedAction>,
    ) -> Result<(), ObligationEvaluateError> {
        // Prefer the derived obligations when the edge still belongs to any;
        // fall back to catalogue-only edge resolution (AC-12). Edge ownership
        // is the domain's predicate (ObligationsDocument::owners_of_edge); each
        // (edge × obligation) pair is adjudicated independently.
        let owners = obligations.owners_of_edge(edge_id);
        if !owners.is_empty() {
            let Some(anchor_text) = resolve_anchor_text(spec, edge_id.anchor_id().element_id())
            else {
                plan.push(PlannedAction::Immediate(ImmediateOutcome::PendingEdge(edge_id.clone())));
                return Ok(());
            };
            for obligation in owners {
                let Some(declaration) =
                    obligation_declaration_text_from_loaded(catalogues, obligation)
                else {
                    plan.push(PlannedAction::Immediate(ImmediateOutcome::PendingEdge(
                        edge_id.clone(),
                    )));
                    continue;
                };
                let declaration_hash =
                    DeclarationHash::new(self.hasher.sha256(declaration.as_bytes()));
                self.emit_fulfillment_action(
                    edge_id.clone(),
                    obligation.id().clone(),
                    &declaration,
                    &anchor_text,
                    declaration_hash,
                    tests,
                    existing_fulfillment_cache,
                    plan,
                )?;
            }
            return Ok(());
        }

        let Some((declaration, anchor_text, declaration_hash)) =
            self.resolve_edge(edge_id, catalogues, spec)
        else {
            plan.push(PlannedAction::Immediate(ImmediateOutcome::PendingEdge(edge_id.clone())));
            return Ok(());
        };
        self.emit_fulfillment_action(
            edge_id.clone(),
            synthetic_obligation_id(edge_id),
            &declaration,
            &anchor_text,
            declaration_hash,
            tests,
            existing_fulfillment_cache,
            plan,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn plan_waiver_record(
        &self,
        edge_id: &TestObligationEdgeId,
        reason: &WaivedReason,
        obligations: &ObligationsDocument,
        catalogues: &[LoadedCatalogueDocument],
        spec: &SpecDocument,
        existing_waiver_cache: Option<&WaiverCacheDocument>,
        plan: &mut Vec<PlannedAction>,
    ) -> Result<(), ObligationEvaluateError> {
        let Some((declaration, anchor_text, _declaration_hash)) =
            self.resolve_waiver_edge(edge_id, obligations, catalogues, spec)
        else {
            plan.push(PlannedAction::Immediate(ImmediateOutcome::PendingEdge(edge_id.clone())));
            return Ok(());
        };
        let reason_hash = WaivedReasonHash::new(self.hasher.sha256(reason.as_str().as_bytes()));
        let declaration_hash_actual =
            DeclarationHash::new(self.hasher.sha256(declaration.as_bytes()));
        let anchor_hash = AnchorTextHash::new(self.hasher.sha256(anchor_text.as_bytes()));
        let key = WaiverCacheKey::new(reason_hash, declaration_hash_actual, anchor_hash);
        if let Some(verdict) = cached_waiver_verdict(
            existing_waiver_cache,
            edge_id,
            &key,
            &self.waiver_verifier_fingerprint,
        ) {
            plan.push(PlannedAction::Immediate(ImmediateOutcome::WaiverCached {
                edge_id: edge_id.clone(),
                key,
                verdict,
            }));
            return Ok(());
        }
        plan.push(PlannedAction::Waiver(WaiverLlmTask {
            edge_id: edge_id.clone(),
            key,
            reason: reason.clone(),
            declaration,
            anchor_text,
        }));
        Ok(())
    }

    /// Emits either a cache-hit `Immediate` or a `Fulfillment` LLM task.
    #[allow(clippy::too_many_arguments)]
    fn emit_fulfillment_action(
        &self,
        edge_id: TestObligationEdgeId,
        obligation_id: TestObligationId,
        declaration: &str,
        anchor_text: &str,
        declaration_hash: DeclarationHash,
        tests: &[TestLocation],
        existing_fulfillment_cache: Option<&ObligationFulfillmentCacheDocument>,
        plan: &mut Vec<PlannedAction>,
    ) -> Result<(), ObligationEvaluateError> {
        let (tests_source, bound_hash) = self.bound_tests(tests)?;
        let anchor_hash = AnchorTextHash::new(self.hasher.sha256(anchor_text.as_bytes()));
        let key = ObligationFulfillmentCacheKey::new(bound_hash, declaration_hash, anchor_hash);
        if let Some(verdict) = cached_fulfillment_verdict(
            existing_fulfillment_cache,
            &edge_id,
            &obligation_id,
            &key,
            &self.fulfillment_verifier_fingerprint,
        ) {
            plan.push(PlannedAction::Immediate(ImmediateOutcome::FulfillmentCached {
                edge_id,
                obligation_id,
                key,
                verdict,
            }));
            return Ok(());
        }
        plan.push(PlannedAction::Fulfillment(FulfillmentLlmTask {
            edge_id,
            obligation_id,
            key,
            tests_source,
            declaration: declaration.to_owned(),
            anchor_text: anchor_text.to_owned(),
        }));
        Ok(())
    }

    /// Applies every planned action + its LLM verdict to the tally / cache
    /// entry vectors in plan order.
    ///
    /// `fulfillment_verdicts` and `waiver_verdicts` are consumed in the order
    /// their respective `Fulfillment` / `Waiver` planned actions appear.
    pub(super) fn apply_planned(
        &self,
        plan: Vec<PlannedAction>,
        fulfillment_verdicts: Vec<ObligationFulfillmentVerdict>,
        waiver_verdicts: Vec<WaiverVerdict>,
        tally: &mut Tally,
        fulfillment_entries: &mut Vec<ObligationFulfillmentCacheEntry>,
        waiver_entries: &mut Vec<WaiverCacheEntry>,
    ) {
        let mut f_iter = fulfillment_verdicts.into_iter();
        let mut w_iter = waiver_verdicts.into_iter();
        for action in plan {
            match action {
                PlannedAction::Immediate(outcome) => match outcome {
                    ImmediateOutcome::PendingObligationId(id) => {
                        record_pending_obligation_id(&id, tally);
                    }
                    ImmediateOutcome::PendingObligationEdges(obligation) => {
                        record_pending_obligation_edges(&obligation, tally);
                    }
                    ImmediateOutcome::PendingEdge(edge_id) => {
                        record_pending_edge(edge_id, tally);
                    }
                    ImmediateOutcome::FulfillmentCached {
                        edge_id,
                        obligation_id,
                        key,
                        verdict,
                    } => {
                        record_fulfillment(&edge_id, &verdict, tally);
                        fulfillment_entries.push(ObligationFulfillmentCacheEntry::new(
                            edge_id,
                            obligation_id,
                            key,
                            verdict,
                            Some(self.fulfillment_verifier_fingerprint.clone()),
                        ));
                    }
                    ImmediateOutcome::WaiverCached { edge_id, key, verdict } => {
                        record_waiver(&edge_id, &verdict, tally);
                        waiver_entries.push(WaiverCacheEntry::new(
                            edge_id,
                            key,
                            verdict,
                            Some(self.waiver_verifier_fingerprint.clone()),
                        ));
                    }
                },
                PlannedAction::Fulfillment(task) => {
                    // The caller passes exactly one verdict per Fulfillment
                    // planned action (from the multiplexer's ordered output);
                    // any shortfall is a caller bug, but we still fail closed
                    // by recording the edge as pending instead of panicking.
                    let Some(verdict) = f_iter.next() else {
                        record_pending_edge_task(task.edge_id, tally);
                        continue;
                    };
                    record_fulfillment(&task.edge_id, &verdict, tally);
                    fulfillment_entries.push(ObligationFulfillmentCacheEntry::new(
                        task.edge_id,
                        task.obligation_id,
                        task.key,
                        verdict,
                        Some(self.fulfillment_verifier_fingerprint.clone()),
                    ));
                }
                PlannedAction::Waiver(task) => {
                    let Some(verdict) = w_iter.next() else {
                        record_pending_edge_task(task.edge_id, tally);
                        continue;
                    };
                    record_waiver(&task.edge_id, &verdict, tally);
                    waiver_entries.push(WaiverCacheEntry::new(
                        task.edge_id,
                        task.key,
                        verdict,
                        Some(self.waiver_verifier_fingerprint.clone()),
                    ));
                }
            }
        }
    }

    /// Builds a `SemanticEscalationFuture` wrapper that returns
    /// `Result<ObligationFulfillmentVerdict, ObligationEvaluateError>` — the
    /// shape the bounded multiplexer collects.
    pub(super) fn fulfillment_llm_future<'a>(
        &'a self,
        task: &FulfillmentLlmTask,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<ObligationFulfillmentVerdict, ObligationEvaluateError>,
                > + Send
                + 'a,
        >,
    > {
        // Materialise the pair once — the resulting `Pair` value is cloned into
        // the async block below and lives for the lifetime of the future.
        let pair_input = build_fulfillment_pair_input(task);
        let key = task.key.clone();
        Box::pin(async move {
            let pair = pair_input?;
            self.fulfillment_driver
                .evaluate_with_escalation(&pair, &key, ModelTier::Fast)
                .await
                .map_err(map_verifier_error)
        })
    }

    /// Waiver-lane companion to [`fulfillment_llm_future`].
    pub(super) fn waiver_llm_future<'a>(
        &'a self,
        task: &WaiverLlmTask,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<WaiverVerdict, ObligationEvaluateError>>
                + Send
                + 'a,
        >,
    > {
        let pair_input = build_waiver_pair_input(task);
        let key = task.key.clone();
        Box::pin(async move {
            let pair = pair_input?;
            self.waiver_driver
                .evaluate_with_escalation(&pair, &key, ModelTier::Fast)
                .await
                .map_err(map_verifier_error)
        })
    }

    /// Concatenates bound test bodies (shared with the calibration probe).
    ///
    /// Extracted from `mod.rs` so `plan.rs` can share the single
    /// `TestSourceScannerPort` boundary the interactor exposes.
    pub(super) fn bound_tests(
        &self,
        tests: &[TestLocation],
    ) -> Result<(String, BoundTestsSetHash), ObligationEvaluateError> {
        let mut source = String::new();
        for location in tests {
            let body = self
                .source_scanner
                .scan_test_body(location)
                .map_err(ObligationEvaluateError::TestSourceScan)?
                .ok_or_else(|| {
                    ObligationEvaluateError::TestSourceScan(TestSourceScanError::Io(super::diag(
                        "bound test source not found",
                    )))
                })?;
            source.push_str(&body);
            source.push('\n');
        }
        let hash = BoundTestsSetHash::new(self.hasher.sha256(source.as_bytes()));
        Ok((source, hash))
    }

    /// Resolves the verification triple for a waiver edge; kept here so
    /// waiver planning can reuse the fulfillment-first precedence.
    fn resolve_waiver_edge(
        &self,
        edge_id: &TestObligationEdgeId,
        obligations: &ObligationsDocument,
        catalogues: &[LoadedCatalogueDocument],
        spec: &SpecDocument,
    ) -> Option<(String, String, DeclarationHash)> {
        if let Some(obligation) = obligations.owners_of_edge(edge_id).into_iter().next() {
            let declaration = obligation_declaration_text_from_loaded(catalogues, obligation)?;
            let anchor_text = resolve_anchor_text(spec, edge_id.anchor_id().element_id())?;
            let declaration_hash = DeclarationHash::new(self.hasher.sha256(declaration.as_bytes()));
            return Some((declaration, anchor_text, declaration_hash));
        }
        self.resolve_edge(edge_id, catalogues, spec)
    }

    /// Catalogue-only edge resolution — used when no derived obligation owns
    /// the edge.
    fn resolve_edge(
        &self,
        edge_id: &TestObligationEdgeId,
        catalogues: &[LoadedCatalogueDocument],
        spec: &SpecDocument,
    ) -> Option<(String, String, DeclarationHash)> {
        let declaration =
            find_declaration_text_from_loaded(catalogues, edge_id.entry_key().as_str())?;
        let anchor_text = resolve_anchor_text(spec, edge_id.anchor_id().element_id())?;
        let declaration_hash = DeclarationHash::new(self.hasher.sha256(declaration.as_bytes()));
        Some((declaration, anchor_text, declaration_hash))
    }
}

/// Materialises the fulfillment pair value once so the LLM future's async
/// block only borrows it — reduces per-poll allocations in the multiplexer.
fn build_fulfillment_pair_input(
    task: &FulfillmentLlmTask,
) -> Result<ObligationFulfillmentPair, ObligationEvaluateError> {
    let tests_source = TestsSource::try_new(task.tests_source.clone())
        .map_err(|_| invalid_input_error("tests_source"))?;
    let entry_declaration = EntryDeclaration::try_new(task.declaration.clone())
        .map_err(|_| invalid_input_error("entry_declaration"))?;
    let anchor_text = AnchorText::try_new(task.anchor_text.clone())
        .map_err(|_| invalid_input_error("anchor_text"))?;
    Ok(ObligationFulfillmentPair::new(tests_source, entry_declaration, anchor_text))
}

fn build_waiver_pair_input(task: &WaiverLlmTask) -> Result<WaiverPair, ObligationEvaluateError> {
    let entry_declaration = EntryDeclaration::try_new(task.declaration.clone())
        .map_err(|_| invalid_input_error("entry_declaration"))?;
    let anchor_text = AnchorText::try_new(task.anchor_text.clone())
        .map_err(|_| invalid_input_error("anchor_text"))?;
    Ok(WaiverPair::new(task.reason.clone(), entry_declaration, anchor_text))
}

/// Fail-closed fallback for the caller-bug case where verdict counts do not
/// line up with the plan's LLM tasks: report the edge as pending so the gate
/// still blocks instead of panicking.
fn record_pending_edge_task(edge_id: TestObligationEdgeId, tally: &mut Tally) {
    record_pending_edge(edge_id, tally);
}

fn invalid_input_error(field: &str) -> ObligationEvaluateError {
    ObligationEvaluateError::VerifierPort(
        domain::tddd::test_obligation::errors::SemanticVerifierError::VerifierPort(super::diag(
            &format!("invalid evaluate input: {field}"),
        )),
    )
}
