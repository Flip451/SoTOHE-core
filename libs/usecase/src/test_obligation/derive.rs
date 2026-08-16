//! `bin/sotp test-obligation derive` — deterministic obligation projection.
//!
//! [`DeriveTestObligationsInteractor`] projects `(catalogue, spec, rules) →
//! obligations` (IN-07 / CN-06). Obligations are generated for `Add` / `Modify`
//! entries only (`Reference` is outside the edge universe, `Delete` yields zero —
//! CN-11 / AC-17). Each role resolves to a decision-table section whose per-axis
//! rules are interpreted by an exhaustive Rust `match` (CN-10 / CN-16), and
//! `trait_impls` rules resolve a `trait_ref` to its catalogue `TraitEntry`'s
//! `ContractRole` (external traits → explicit zero — IN-17 / AC-16). Obligation
//! identity is `(entry key, kind, item identifier)` only (CN-01 / AC-03).

use std::path::PathBuf;
use std::sync::Arc;

use domain::SpecRef;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::{
    CatalogueDocumentLoaderPort, TrackStatusReaderPort,
};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};
use domain::tddd::catalogue_v2::{CatalogueDocument, TraitEntry, TraitRefScope};
use domain::tddd::semantic_verify::{CatalogueEntryKey, CatalogueEntryRef, CatalogueSectionKey};
use domain::tddd::test_obligation::errors::{ObligationDeriveError, TrackStatusReadFailureKind};
use domain::tddd::test_obligation::hashes::DeclarationHash;
use domain::tddd::test_obligation::ids::{
    DiagnosticMessage, TestObligationAnchorId, TestObligationBrief, TestObligationId,
    TestObligationItemIdentifier,
};
use domain::tddd::test_obligation::obligations::{
    ObligationsDocument, TestObligation, validate_method_anchor_coverage,
};
use domain::tddd::test_obligation::ports::{
    ObligationsArtifactPort, TestObligationRulesLoaderPort,
};
use domain::tddd::test_obligation::projection::RoleObligationItemsProjector;
use domain::tddd::test_obligation::rules::{
    RoleObligationRules, TestObligationRule, TestObligationRulesDocument,
};
use domain::tddd::test_obligation::vocab::{
    TargetEntryRoleKind, TestObligationKind, TestObligationPatternKind, TestObligationPerAxis,
};

use domain::SpecDocumentLoaderPort;

use super::{
    TestObligationCatalogueCommandInput, catalogue_artifact_path, diag, is_active_branch,
    sha256_content_hash,
};

/// Command input for [`DeriveTestObligationsApplicationService`] (IN-07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeriveTestObligationsCommand {
    input: TestObligationCatalogueCommandInput,
}

impl DeriveTestObligationsCommand {
    /// Builds a [`DeriveTestObligationsCommand`].
    #[must_use]
    pub fn new(input: TestObligationCatalogueCommandInput) -> Self {
        Self { input }
    }
}

/// Primary port for `bin/sotp test-obligation derive` (IN-07 / AC-03).
pub trait DeriveTestObligationsApplicationService {
    /// Derives the obligations artifact and persists it.
    ///
    /// # Errors
    ///
    /// Returns [`ObligationDeriveError`] when the branch is not the active track
    /// branch, the rules / spec / catalogue cannot be loaded, a derivable
    /// `TraitEntry` fails method-anchor coverage, or the derived artifact
    /// cannot be written.
    fn execute(&self, cmd: &DeriveTestObligationsCommand) -> Result<(), ObligationDeriveError>;
}

/// Interactor implementing [`DeriveTestObligationsApplicationService`] (IN-07).
pub struct DeriveTestObligationsInteractor {
    rules_loader: Arc<dyn TestObligationRulesLoaderPort + Send + Sync>,
    obligations_port: Arc<dyn ObligationsArtifactPort + Send + Sync>,
    spec_reader: Arc<dyn SpecDocumentLoaderPort + Send + Sync>,
    catalogue_reader: Arc<dyn CatalogueDocumentLoaderPort + Send + Sync>,
    track_status_reader: Arc<dyn TrackStatusReaderPort + Send + Sync>,
    items_dir: PathBuf,
    projector: RoleObligationItemsProjector,
}

impl DeriveTestObligationsInteractor {
    /// Builds a [`DeriveTestObligationsInteractor`] from its injected ports.
    #[must_use]
    pub fn new(
        rules_loader: Arc<dyn TestObligationRulesLoaderPort + Send + Sync>,
        obligations_port: Arc<dyn ObligationsArtifactPort + Send + Sync>,
        spec_reader: Arc<dyn SpecDocumentLoaderPort + Send + Sync>,
        catalogue_reader: Arc<dyn CatalogueDocumentLoaderPort + Send + Sync>,
        track_status_reader: Arc<dyn TrackStatusReaderPort + Send + Sync>,
        items_dir: PathBuf,
        projector: RoleObligationItemsProjector,
    ) -> Self {
        Self {
            rules_loader,
            obligations_port,
            spec_reader,
            catalogue_reader,
            track_status_reader,
            items_dir,
            projector,
        }
    }
}

impl DeriveTestObligationsApplicationService for DeriveTestObligationsInteractor {
    fn execute(&self, cmd: &DeriveTestObligationsCommand) -> Result<(), ObligationDeriveError> {
        let input = &cmd.input;
        if !is_active_branch(input.track_id(), input.current_branch()) {
            return Err(ObligationDeriveError::TrackNotActive {
                branch: diag(input.current_branch()),
            });
        }
        let track_status = self
            .track_status_reader
            .read_status(&self.items_dir, input.track_id().as_ref())
            .map_err(|_| {
                ObligationDeriveError::TrackStatusRead(TrackStatusReadFailureKind::Unavailable)
            })?;
        if let Some(status) = track_status.frozen_status() {
            return Err(ObligationDeriveError::TrackFrozen { status });
        }

        let rules = self.rules_loader.load().map_err(ObligationDeriveError::RulesLoad)?;

        // The spec is a declared derivation input; load it as a fail-closed
        // presence precondition (obligation anchors themselves are read from the
        // catalogue entries' `spec_refs`, per IN-05).
        let spec_path =
            PathBuf::from(format!("track/items/{}/spec.json", input.track_id().as_ref()));
        self.spec_reader.load(&spec_path).map_err(ObligationDeriveError::SpecLoad)?;

        let mut catalogues: Vec<(PathBuf, CatalogueDocument)> =
            Vec::with_capacity(input.catalogue_paths().len());
        for path in input.catalogue_paths() {
            let doc =
                self.catalogue_reader.load(path).map_err(ObligationDeriveError::CatalogueLoad)?;
            catalogues.push((path.clone(), doc));
        }

        // D1 coverage is a write-side gate on the catalogues named on this
        // command. The shared projection below is also used by read-only
        // `check`, which must not retroactively reject enrolled catalogues.
        for (_, catalogue) in &catalogues {
            validate_derivable_method_anchor_coverage(catalogue)
                .map_err(ObligationDeriveError::InvalidCatalogueState)?;
        }

        let document = derive_obligations_document(
            input.track_id().clone(),
            &rules,
            &catalogues,
            &self.projector,
        )
        .map_err(ObligationDeriveError::InvalidCatalogueState)?;
        self.obligations_port.save(&document).map_err(ObligationDeriveError::ArtifactWrite)?;
        Ok(())
    }
}

/// Derives an obligations document without reading from or writing to a port.
///
/// Both the write-side derive command and the read-only check gate use this
/// projection, so their definition of the expected artifact cannot diverge.
/// Method-anchor coverage is intentionally not applied here: `check` re-derives
/// to compare stored obligations, and D1 does not reject existing catalogues.
pub(crate) fn derive_obligations_document(
    track_id: domain::TrackId,
    rules: &TestObligationRulesDocument,
    catalogues: &[(PathBuf, CatalogueDocument)],
    projector: &RoleObligationItemsProjector,
) -> Result<ObligationsDocument, DiagnosticMessage> {
    let trait_roles = index_trait_roles(catalogues)?;
    let mut obligations: Vec<TestObligation> = Vec::new();
    for (path, catalogue) in catalogues {
        let file_path = catalogue_artifact_path(path);
        derive_type_obligations(rules, projector, catalogue, &file_path, &mut obligations)?;
        derive_function_obligations(rules, projector, catalogue, &file_path, &mut obligations)?;
        derive_trait_obligations(rules, projector, catalogue, &file_path, &mut obligations)?;
        derive_trait_impl_obligations(
            rules,
            projector,
            catalogue,
            &file_path,
            &trait_roles,
            &mut obligations,
        )?;
    }
    Ok(ObligationsDocument::new(track_id, obligations))
}

/// Whether an entry's change action participates in obligation derivation.
///
/// Only `Add` / `Modify` do (CN-11 / AC-17): `Reference` is outside the edge
/// universe and `Delete` yields zero obligations.
fn is_derivable(action: ItemAction) -> bool {
    matches!(action, ItemAction::Add | ItemAction::Modify)
}

/// Write-side D1 coverage: only derivable (`Add` / `Modify`) trait entries.
fn validate_derivable_method_anchor_coverage(
    catalogue: &CatalogueDocument,
) -> Result<(), DiagnosticMessage> {
    for entry in catalogue.traits().values() {
        if !is_derivable(entry.action()) {
            continue;
        }
        validate_method_anchor_coverage(entry)?;
    }
    Ok(())
}

/// Trait metadata needed to derive obligations for matching `trait_impl`s.
struct TraitRoleEntry {
    crate_name: String,
    name: String,
    role: ContractRole,
    anchors: Vec<TestObligationAnchorId>,
    declaration_text: String,
}

/// Indexes every catalogue `TraitEntry` name to its `ContractRole` and anchors
/// so a `trait_impl`'s `trait_ref` can be resolved across crates (IN-17).
fn index_trait_roles(
    catalogues: &[(PathBuf, CatalogueDocument)],
) -> Result<Vec<TraitRoleEntry>, DiagnosticMessage> {
    let mut index = Vec::new();
    for (_, catalogue) in catalogues {
        for (name, entry) in catalogue.traits() {
            index.push(TraitRoleEntry {
                crate_name: catalogue.crate_name().as_str().to_owned(),
                name: name.as_str().to_owned(),
                role: entry.role().clone(),
                anchors: anchors_from_spec_refs(entry.spec_refs())?,
                declaration_text: format!("{entry:?}"),
            });
        }
    }
    Ok(index)
}

/// Resolves a trait impl's classified `trait_ref` to the matching catalogue trait.
///
/// Bare trait refs are self-crate refs and must match the impl catalogue's crate;
/// workspace-qualified refs must match the qualified crate. External refs yield
/// no obligations because there is no catalogue `TraitEntry` role to project.
fn resolve_trait_role<'a>(
    trait_roles: &'a [TraitRoleEntry],
    scope: &TraitRefScope,
    impl_crate_name: &str,
) -> Option<&'a TraitRoleEntry> {
    match scope {
        TraitRefScope::SelfCrate { bare_name } => trait_roles
            .iter()
            .find(|entry| entry.crate_name == impl_crate_name && entry.name == bare_name.as_str()),
        TraitRefScope::Workspace { crate_name, bare_name } => trait_roles.iter().find(|entry| {
            entry.crate_name == crate_name.as_str() && entry.name == bare_name.as_str()
        }),
        TraitRefScope::External => None,
    }
}

/// Derives obligations for every derivable `TypeEntry` (role + typestate).
fn derive_type_obligations(
    rules: &TestObligationRulesDocument,
    projector: &RoleObligationItemsProjector,
    catalogue: &CatalogueDocument,
    file_path: &str,
    out: &mut Vec<TestObligation>,
) -> Result<(), DiagnosticMessage> {
    for (name, entry) in catalogue.types() {
        if !is_derivable(entry.action()) {
            continue;
        }
        let entry_key = catalogue_key(name.as_str())?;
        let target = CatalogueEntryRef::new(
            file_path.to_owned(),
            CatalogueSectionKey::Types,
            entry_key.clone(),
        );
        let role_kind = TargetEntryRoleKind::DataRole(entry.role().clone());
        let decl_hash = declaration_hash(entry);
        let anchors = anchors_from_spec_refs(entry.spec_refs())?;

        if let Some(role_rules) = find_data_role_rules(rules, entry.role()) {
            emit_rules(
                role_rules,
                &entry_key,
                &target,
                &role_kind,
                &decl_hash,
                &anchors,
                out,
                |axis| projector.data_role_items(entry, axis),
            )?;
        }
        if projector.type_has_typestate(entry) {
            if let Some(pattern_rules) = find_pattern_rules(rules) {
                emit_rules(
                    pattern_rules,
                    &entry_key,
                    &target,
                    &role_kind,
                    &decl_hash,
                    &anchors,
                    out,
                    |axis| projector.typestate_items(entry, axis),
                )?;
            }
        }
    }
    Ok(())
}

/// Derives obligations for every derivable `TraitEntry`.
fn derive_trait_obligations(
    rules: &TestObligationRulesDocument,
    projector: &RoleObligationItemsProjector,
    catalogue: &CatalogueDocument,
    file_path: &str,
    out: &mut Vec<TestObligation>,
) -> Result<(), DiagnosticMessage> {
    for (name, entry) in catalogue.traits() {
        if !is_derivable(entry.action()) {
            continue;
        }
        let entry_key = catalogue_key(name.as_str())?;
        let target = CatalogueEntryRef::new(
            file_path.to_owned(),
            CatalogueSectionKey::Traits,
            entry_key.clone(),
        );
        let role_kind = TargetEntryRoleKind::ContractRole(entry.role().clone());
        let decl_hash = declaration_hash(entry);
        let anchors = anchors_from_spec_refs(entry.spec_refs())?;

        if let Some(role_rules) = find_contract_role_rules(rules, entry.role()) {
            emit_trait_role_rules(
                role_rules, projector, entry, &entry_key, &target, &role_kind, &decl_hash,
                &anchors, out,
            )?;
        }
    }
    Ok(())
}

/// Emits trait obligations, assigning method-axis items only their method refs.
#[allow(clippy::too_many_arguments)]
fn emit_trait_role_rules(
    role_rules: &RoleObligationRules,
    projector: &RoleObligationItemsProjector,
    entry: &TraitEntry,
    entry_key: &CatalogueEntryKey,
    target: &CatalogueEntryRef,
    role_kind: &TargetEntryRoleKind,
    decl_hash: &DeclarationHash,
    entry_anchors: &[TestObligationAnchorId],
    out: &mut Vec<TestObligation>,
) -> Result<(), DiagnosticMessage> {
    for rule in role_rules.obligations() {
        if matches!(
            rule.per_axis(),
            TestObligationPerAxis::Method | TestObligationPerAxis::TraitMethod
        ) {
            let mut item_ids = projector.contract_role_items(entry, rule.per_axis());
            if let Some(minimum) = rule.minimum() {
                while item_ids.len() < minimum.as_usize() {
                    let filler =
                        TestObligationItemIdentifier::try_new(format!("min#{}", item_ids.len()))
                            .map_err(|_| diag("empty min filler"))?;
                    item_ids.push(filler);
                }
            }

            for (method_index, item) in item_ids.into_iter().enumerate() {
                let anchors = match entry.methods().get(method_index) {
                    Some(method) => anchors_from_spec_refs(method.spec_refs())?,
                    None => entry_anchors.to_vec(),
                };
                let obligation = build_obligation(
                    entry_key,
                    target,
                    role_kind,
                    rule.kind(),
                    &item,
                    decl_hash,
                    &anchors,
                )?;
                out.push(obligation);
            }
        } else {
            emit_rule_items(
                rule,
                entry_key,
                target,
                role_kind,
                decl_hash,
                entry_anchors,
                out,
                projector.contract_role_items(entry, rule.per_axis()),
            )?;
        }
    }
    Ok(())
}

/// Derives obligations for every derivable `FunctionEntry`.
fn derive_function_obligations(
    rules: &TestObligationRulesDocument,
    projector: &RoleObligationItemsProjector,
    catalogue: &CatalogueDocument,
    file_path: &str,
    out: &mut Vec<TestObligation>,
) -> Result<(), DiagnosticMessage> {
    for (path, entry) in catalogue.functions() {
        if !is_derivable(entry.action()) {
            continue;
        }
        let entry_key = catalogue_key(&path.to_string())?;
        let target = CatalogueEntryRef::new(
            file_path.to_owned(),
            CatalogueSectionKey::Functions,
            entry_key.clone(),
        );
        let role = entry.role();
        let role_kind = TargetEntryRoleKind::FunctionRole(role);
        let decl_hash = DeclarationHash::new(sha256_content_hash(format!("{entry:?}").as_bytes()));
        let anchors = anchors_from_spec_refs(entry.spec_refs())?;

        if let Some(role_rules) = find_function_role_rules(rules, role) {
            emit_rules(
                role_rules,
                &entry_key,
                &target,
                &role_kind,
                &decl_hash,
                &anchors,
                out,
                |axis| projector.function_role_items(axis),
            )?;
        }
    }
    Ok(())
}

/// Derives `contract_conformance` obligations for every derivable `trait_impl`.
///
/// The `trait_ref` is resolved to its catalogue `TraitEntry`'s `ContractRole`;
/// an unresolved (external) trait yields zero obligations (IN-17 / AC-16).
fn derive_trait_impl_obligations(
    rules: &TestObligationRulesDocument,
    projector: &RoleObligationItemsProjector,
    catalogue: &CatalogueDocument,
    file_path: &str,
    trait_roles: &[TraitRoleEntry],
    out: &mut Vec<TestObligation>,
) -> Result<(), DiagnosticMessage> {
    for impl_decl in catalogue.trait_impls() {
        if !is_derivable(impl_decl.action()) {
            continue;
        }
        let scope = impl_decl.trait_ref_scope();
        let Some(trait_entry) =
            resolve_trait_role(trait_roles, &scope, catalogue.crate_name().as_str())
        else {
            continue;
        };
        let role = trait_entry.role.clone();
        let Some(role_rules) = find_trait_impl_rules(rules, &role) else {
            continue;
        };
        let entry_key = catalogue_key(impl_decl.for_type().as_str())?;
        let target = CatalogueEntryRef::new(
            file_path.to_owned(),
            CatalogueSectionKey::Traits,
            entry_key.clone(),
        );
        let role_kind = TargetEntryRoleKind::TraitImpl(role);
        let declaration =
            super::trait_impl_pair_declaration_text(impl_decl, &trait_entry.declaration_text);
        let decl_hash = DeclarationHash::new(sha256_content_hash(declaration.as_bytes()));
        for rule in role_rules.obligations() {
            let Some(item_ids) = projector.trait_impl_items(impl_decl.trait_ref(), rule.per_axis())
            else {
                continue;
            };
            emit_rule_items(
                rule,
                &entry_key,
                &target,
                &role_kind,
                &decl_hash,
                &trait_entry.anchors,
                out,
                item_ids,
            )?;
        }
    }
    Ok(())
}

/// Emits obligations for each rule in `role_rules`, enumerating per-axis items
/// via `items` and applying each rule's minimum floor.
#[allow(clippy::too_many_arguments)]
fn emit_rules<F>(
    role_rules: &RoleObligationRules,
    entry_key: &CatalogueEntryKey,
    target: &CatalogueEntryRef,
    role_kind: &TargetEntryRoleKind,
    decl_hash: &DeclarationHash,
    anchors: &[TestObligationAnchorId],
    out: &mut Vec<TestObligation>,
    items: F,
) -> Result<(), DiagnosticMessage>
where
    F: Fn(&TestObligationPerAxis) -> Vec<TestObligationItemIdentifier>,
{
    for rule in role_rules.obligations() {
        emit_rule_items(
            rule,
            entry_key,
            target,
            role_kind,
            decl_hash,
            anchors,
            out,
            items(rule.per_axis()),
        )?;
    }
    Ok(())
}

/// Emits one rule's obligations after applying the rule's minimum floor.
#[allow(clippy::too_many_arguments)]
fn emit_rule_items(
    rule: &TestObligationRule,
    entry_key: &CatalogueEntryKey,
    target: &CatalogueEntryRef,
    role_kind: &TargetEntryRoleKind,
    decl_hash: &DeclarationHash,
    anchors: &[TestObligationAnchorId],
    out: &mut Vec<TestObligation>,
    mut item_ids: Vec<TestObligationItemIdentifier>,
) -> Result<(), DiagnosticMessage> {
    if let Some(minimum) = rule.minimum() {
        while item_ids.len() < minimum.as_usize() {
            let filler = TestObligationItemIdentifier::try_new(format!("min#{}", item_ids.len()))
                .map_err(|_| diag("empty min filler"))?;
            item_ids.push(filler);
        }
    }
    for item in item_ids {
        let obligation =
            build_obligation(entry_key, target, role_kind, rule.kind(), &item, decl_hash, anchors)?;
        out.push(obligation);
    }
    Ok(())
}

/// Builds one [`TestObligation`] from its identity + evidence components.
fn build_obligation(
    entry_key: &CatalogueEntryKey,
    target: &CatalogueEntryRef,
    role_kind: &TargetEntryRoleKind,
    kind: &TestObligationKind,
    item: &TestObligationItemIdentifier,
    decl_hash: &DeclarationHash,
    anchors: &[TestObligationAnchorId],
) -> Result<TestObligation, DiagnosticMessage> {
    let id = TestObligationId::new(entry_key.clone(), kind.clone(), item.clone());
    let brief_text = format!(
        "Author a {} test covering '{}' of {}",
        kind.as_kebab(),
        item.as_str(),
        entry_key.as_str()
    );
    let brief = TestObligationBrief::try_new(brief_text).map_err(|_| diag("empty brief"))?;
    Ok(TestObligation::new(
        id,
        target.clone(),
        role_kind.canonical_form()?,
        brief,
        decl_hash.clone(),
        anchors.to_vec(),
    ))
}

/// Computes the declaration hash from a `Debug`-canonicalised entry.
fn declaration_hash<T: std::fmt::Debug>(entry: &T) -> DeclarationHash {
    DeclarationHash::new(sha256_content_hash(format!("{entry:?}").as_bytes()))
}

/// Validates a catalogue entry key.
fn catalogue_key(key: &str) -> Result<CatalogueEntryKey, DiagnosticMessage> {
    CatalogueEntryKey::try_new(key.to_owned()).map_err(|_| diag("empty catalogue entry key"))
}

/// Converts catalogue `spec_refs` to obligation anchor ids.
fn anchors_from_spec_refs(
    refs: &[SpecRef],
) -> Result<Vec<TestObligationAnchorId>, DiagnosticMessage> {
    let mut anchors = Vec::with_capacity(refs.len());
    for spec_ref in refs {
        let anchor = TestObligationAnchorId::try_new(
            spec_ref.file.display().to_string(),
            spec_ref.anchor.as_ref().to_owned(),
        )
        .map_err(|_| diag("empty spec ref"))?;
        anchors.push(anchor);
    }
    Ok(anchors)
}

/// Finds the rules declared for `role` in the `DataRole` section.
fn find_data_role_rules<'a>(
    rules: &'a TestObligationRulesDocument,
    role: &DataRole,
) -> Option<&'a RoleObligationRules> {
    rules
        .data_roles()
        .iter()
        .find(|(r, _)| r.variant_name() == role.variant_name())
        .map(|(_, rules)| rules)
}

/// Finds the rules declared for `role` in the `ContractRole` section.
fn find_contract_role_rules<'a>(
    rules: &'a TestObligationRulesDocument,
    role: &ContractRole,
) -> Option<&'a RoleObligationRules> {
    rules
        .contract_roles()
        .iter()
        .find(|(r, _)| r.variant_name() == role.variant_name())
        .map(|(_, rules)| rules)
}

/// Finds the rules declared for `role` in the `FunctionRole` section.
fn find_function_role_rules(
    rules: &TestObligationRulesDocument,
    role: FunctionRole,
) -> Option<&RoleObligationRules> {
    rules.function_roles().iter().find(|(r, _)| *r == role).map(|(_, rules)| rules)
}

/// Finds the rules declared for the typestate pattern section.
fn find_pattern_rules(rules: &TestObligationRulesDocument) -> Option<&RoleObligationRules> {
    rules
        .patterns()
        .iter()
        .find(|(p, _)| matches!(p, TestObligationPatternKind::Typestate))
        .map(|(_, rules)| rules)
}

/// Finds the rules declared for `role` in the `trait_impls` section.
fn find_trait_impl_rules<'a>(
    rules: &'a TestObligationRulesDocument,
    role: &ContractRole,
) -> Option<&'a RoleObligationRules> {
    rules
        .trait_impls()
        .iter()
        .find(|(r, _)| r.variant_name() == role.variant_name())
        .map(|(_, rules)| rules)
}

#[cfg(test)]
#[path = "derive_tests.rs"]
mod tests;
