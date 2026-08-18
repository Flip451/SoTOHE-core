//! Role-driven test-obligation item projection (IN-04 / IN-05 / IN-07 / IN-17 /
//! CN-10 / CN-16).
//!
//! [`RoleObligationItemsProjector`] owns the catalogue-entry → per-axis
//! item-identifier decision inside the domain, so the usecase interactor only
//! orchestrates derivation and never interprets a role itself. Keeping the
//! exhaustive per-axis `match` in the domain preserves hexagonal purity: the
//! role → item mapping is a domain rule, not application plumbing.

use crate::tddd::catalogue_v2::roles::{DataRole, ItemAction};
use crate::tddd::catalogue_v2::{TraitEntry, TypeEntry, TypeKindV2, TypeRef};
use crate::tddd::test_obligation::ids::TestObligationItemIdentifier;
use crate::tddd::test_obligation::vocab::TestObligationPerAxis;

/// Role-driven obligation item enumerator. See IN-04 / IN-05 / IN-07 / IN-17 / CN-10 / CN-16.
#[derive(Debug, Clone)]
pub struct RoleObligationItemsProjector {}

impl RoleObligationItemsProjector {
    /// See IN-04 / IN-07.
    // The catalogue declares `new` (not `Default`); the projector is a stateless dispatcher.
    #[allow(clippy::new_without_default)]
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }

    /// See IN-05 / IN-17 / CN-10 / CN-16.
    #[must_use]
    pub fn data_role_items(
        &self,
        entry: &TypeEntry,
        per_axis: &TestObligationPerAxis,
    ) -> Vec<TestObligationItemIdentifier> {
        let raw = match per_axis {
            TestObligationPerAxis::Invariant => invariant_items(entry.role()),
            TestObligationPerAxis::Method => entry
                .methods()
                .iter()
                .filter(|method| is_derivable_method(method.action()))
                .map(|m| format!("method:{}", m.name.as_str()))
                .collect(),
            TestObligationPerAxis::Handles => handles_items(entry.role()),
            TestObligationPerAxis::ReactsTo => reacts_to_items(entry.role()),
            TestObligationPerAxis::Emits => emits_items(entry.role()),
            TestObligationPerAxis::Transition => typestate_raw(entry, per_axis),
            TestObligationPerAxis::Entry => vec!["entry".to_owned()],
            TestObligationPerAxis::TraitMethod | TestObligationPerAxis::TraitImpl => Vec::new(),
        };
        wrap_items(raw)
    }

    /// See IN-05 / IN-17 / CN-10 / CN-16.
    #[must_use]
    pub fn contract_role_items(
        &self,
        entry: &TraitEntry,
        per_axis: &TestObligationPerAxis,
    ) -> Vec<TestObligationItemIdentifier> {
        let raw = match per_axis {
            TestObligationPerAxis::Method | TestObligationPerAxis::TraitMethod => entry
                .methods()
                .iter()
                .filter(|method| is_derivable_method(method.action()))
                .map(|m| format!("{}:{}", per_axis.as_kebab(), m.name.as_str()))
                .collect(),
            TestObligationPerAxis::Entry => vec!["entry".to_owned()],
            TestObligationPerAxis::Invariant
            | TestObligationPerAxis::Handles
            | TestObligationPerAxis::ReactsTo
            | TestObligationPerAxis::Transition
            | TestObligationPerAxis::Emits
            | TestObligationPerAxis::TraitImpl => Vec::new(),
        };
        wrap_items(raw)
    }

    /// See IN-05 / IN-17 / CN-10 / CN-16.
    #[must_use]
    pub fn function_role_items(
        &self,
        per_axis: &TestObligationPerAxis,
    ) -> Vec<TestObligationItemIdentifier> {
        let raw = match per_axis {
            TestObligationPerAxis::Entry => vec!["entry".to_owned()],
            _ => Vec::new(),
        };
        wrap_items(raw)
    }

    /// See IN-05 / IN-17 / CN-10 / CN-16.
    #[must_use]
    pub fn trait_impl_items(
        &self,
        trait_ref: &TypeRef,
        per_axis: &TestObligationPerAxis,
    ) -> Option<Vec<TestObligationItemIdentifier>> {
        match per_axis {
            TestObligationPerAxis::TraitImpl => {
                Some(wrap_items(vec![format!("trait_impl:{}", trait_ref.as_str())]))
            }
            _ => None,
        }
    }

    /// See IN-05 / IN-17 / CN-10 / CN-16.
    #[must_use]
    pub fn typestate_items(
        &self,
        entry: &TypeEntry,
        per_axis: &TestObligationPerAxis,
    ) -> Vec<TestObligationItemIdentifier> {
        wrap_items(typestate_raw(entry, per_axis))
    }

    /// Whether `entry` carries a typestate marker, gating typestate-pattern
    /// obligation derivation. See IN-17 / CN-10.
    #[must_use]
    pub fn type_has_typestate(&self, entry: &TypeEntry) -> bool {
        matches!(entry.kind(), TypeKindV2::Struct(sk) if sk.typestate.is_some())
    }
}

fn is_derivable_method(action: ItemAction) -> bool {
    matches!(action, ItemAction::Add | ItemAction::Modify)
}

/// Wraps raw item strings into validated identifiers.
///
/// Each raw string is prefix-formatted from a non-empty catalogue component
/// (method name, invariant label, etc.), so `try_new` never rejects one in
/// practice; a `filter_map` on `ok()` keeps the projection panic-free while
/// preserving the "no empty item identifier" invariant fail-closed.
fn wrap_items(raw: Vec<String>) -> Vec<TestObligationItemIdentifier> {
    raw.into_iter().filter_map(|s| TestObligationItemIdentifier::try_new(s).ok()).collect()
}

/// Invariant item identifiers for the invariant-bearing `DataRole` variants.
fn invariant_items(role: &DataRole) -> Vec<String> {
    let invariants = match role {
        DataRole::ValueObject { invariants }
        | DataRole::Entity { invariants, .. }
        | DataRole::AggregateRoot { invariants, .. } => invariants,
        _ => return Vec::new(),
    };
    invariants.iter().map(|i| format!("invariant:{}", i.name.as_str())).collect()
}

/// `handles` item identifiers for a `UseCase` `DataRole`.
fn handles_items(role: &DataRole) -> Vec<String> {
    match role {
        DataRole::UseCase { handles } => {
            handles.iter().map(|t| format!("handles:{}", t.as_str())).collect()
        }
        _ => Vec::new(),
    }
}

/// `reacts_to` item identifiers for an `EventPolicy` `DataRole`.
fn reacts_to_items(role: &DataRole) -> Vec<String> {
    match role {
        DataRole::EventPolicy { reacts_to } => {
            reacts_to.as_slice().iter().map(|t| format!("reacts_to:{}", t.as_str())).collect()
        }
        _ => Vec::new(),
    }
}

/// `emits` item identifiers for the emitting `DataRole` variants.
fn emits_items(role: &DataRole) -> Vec<String> {
    let emits = match role {
        DataRole::AggregateRoot { emits, .. } | DataRole::DomainService { emits } => emits,
        _ => return Vec::new(),
    };
    emits.iter().map(|t| format!("emits:{}", t.as_str())).collect()
}

/// Typestate transition item identifiers for a `TypeEntry`.
fn typestate_raw(entry: &TypeEntry, per_axis: &TestObligationPerAxis) -> Vec<String> {
    if !matches!(per_axis, TestObligationPerAxis::Transition) {
        return Vec::new();
    }
    match entry.kind() {
        TypeKindV2::Struct(sk) => match &sk.typestate {
            Some(marker) => marker
                .transitions()
                .transition_methods()
                .iter()
                .map(|m| format!("transition:{}", m.as_str()))
                .collect(),
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::RoleObligationItemsProjector;
    use crate::tddd::catalogue_v2::identifiers::InvariantName;
    use crate::tddd::catalogue_v2::roles::{
        ContractRole, DataRole, InvariantDecl, InvariantPredicate, ItemAction, NonEmptyVec,
        SelfReceiver,
    };
    use crate::tddd::catalogue_v2::{
        MethodDeclaration, MethodName, ModulePath, StructKind, StructShape, TraitEntry, TypeEntry,
        TypeKindV2, TypeName, TypeRef, TypestateMarker, TypestateTransitions,
    };
    use crate::tddd::test_obligation::ids::TestObligationItemIdentifier;
    use crate::tddd::test_obligation::vocab::TestObligationPerAxis;

    fn projector() -> RoleObligationItemsProjector {
        RoleObligationItemsProjector::new()
    }

    fn items_as_str(items: &[TestObligationItemIdentifier]) -> Vec<&str> {
        items.iter().map(TestObligationItemIdentifier::as_str).collect()
    }

    fn invariant(name: &str) -> InvariantDecl {
        InvariantDecl::new(
            InvariantName::new(name).unwrap(),
            InvariantPredicate::SelfMethod(MethodName::new(format!("is_{name}")).unwrap()),
        )
    }

    fn type_entry(role: DataRole, kind: TypeKindV2, methods: Vec<MethodDeclaration>) -> TypeEntry {
        TypeEntry::new(
            ItemAction::Add,
            role,
            kind,
            methods,
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        )
    }

    fn plain_struct() -> TypeKindV2 {
        TypeKindV2::Struct(StructKind::new(StructShape::Unit, None))
    }

    fn method(name: &str, action: ItemAction) -> MethodDeclaration {
        MethodDeclaration::new(
            MethodName::new(name).unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            false,
            vec![],
            vec![],
            vec![],
            action,
            None,
        )
    }

    #[test]
    fn data_role_items_enumerates_invariants() {
        // IN-05 / CN-10 / CN-16: one boundary item per declared invariant.
        let entry = type_entry(
            DataRole::ValueObject { invariants: vec![invariant("positive"), invariant("bounded")] },
            plain_struct(),
            vec![],
        );
        let items = projector().data_role_items(&entry, &TestObligationPerAxis::Invariant);
        assert_eq!(items_as_str(&items), vec!["invariant:positive", "invariant:bounded"]);
    }

    #[test]
    fn data_role_items_method_axis_prefixes_method_names() {
        let method = MethodDeclaration::new(
            MethodName::new("compute").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("Output").unwrap(),
            false,
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );
        let entry = type_entry(DataRole::domain_service(), plain_struct(), vec![method]);
        let items = projector().data_role_items(&entry, &TestObligationPerAxis::Method);
        assert_eq!(items_as_str(&items), vec!["method:compute"]);
    }

    #[test]
    fn data_role_items_entry_axis_is_singleton() {
        let entry = type_entry(DataRole::domain_service(), plain_struct(), vec![]);
        let items = projector().data_role_items(&entry, &TestObligationPerAxis::Entry);
        assert_eq!(items_as_str(&items), vec!["entry"]);
    }

    #[test]
    fn data_role_items_domain_service_without_emits_yields_no_emits_obligations() {
        // CN-16 / IN-07: the default DomainService declaration explicitly has
        // no emitted events, so the decision-table interpreter must not invent
        // an `emits:*` obligation for it.
        let entry = type_entry(DataRole::domain_service(), plain_struct(), vec![]);
        let items = projector().data_role_items(&entry, &TestObligationPerAxis::Emits);
        assert!(items.is_empty());
    }

    #[test]
    fn data_role_items_handles_axis_enumerates_use_case_handles() {
        // IN-17 / CN-10: UseCase handles -> handles:<type>.
        let entry = type_entry(
            DataRole::UseCase { handles: vec![TypeRef::new("RegisterUser").unwrap()] },
            plain_struct(),
            vec![],
        );
        let items = projector().data_role_items(&entry, &TestObligationPerAxis::Handles);
        assert_eq!(items_as_str(&items), vec!["handles:RegisterUser"]);
    }

    #[test]
    fn data_role_items_transition_axis_delegates_to_typestate() {
        let marker = TypestateMarker::new(
            TypeName::new("Machine").unwrap(),
            TypestateTransitions::new(vec![MethodName::new("advance").unwrap()]),
        );
        let entry = type_entry(
            DataRole::domain_service(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, Some(marker))),
            vec![],
        );
        let items = projector().data_role_items(&entry, &TestObligationPerAxis::Transition);
        assert_eq!(items_as_str(&items), vec!["transition:advance"]);
    }

    #[test]
    fn typestate_items_empty_when_no_typestate_marker() {
        let entry = type_entry(DataRole::domain_service(), plain_struct(), vec![]);
        let items = projector().typestate_items(&entry, &TestObligationPerAxis::Transition);
        assert!(items.is_empty());
    }

    #[test]
    fn type_has_typestate_detects_marker_presence() {
        // IN-17 / CN-10: the typestate gate is true only for marked structs.
        let marker = TypestateMarker::new(
            TypeName::new("Machine").unwrap(),
            TypestateTransitions::new(vec![MethodName::new("advance").unwrap()]),
        );
        let with = type_entry(
            DataRole::domain_service(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, Some(marker))),
            vec![],
        );
        let without = type_entry(DataRole::domain_service(), plain_struct(), vec![]);
        assert!(projector().type_has_typestate(&with));
        assert!(!projector().type_has_typestate(&without));
    }

    #[test]
    fn function_role_items_entry_axis_only() {
        let p = projector();
        assert_eq!(
            items_as_str(&p.function_role_items(&TestObligationPerAxis::Entry)),
            vec!["entry"]
        );
        assert!(p.function_role_items(&TestObligationPerAxis::Method).is_empty());
    }

    #[test]
    fn contract_role_items_entry_axis_is_singleton() {
        let entry = TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        );
        let items = projector().contract_role_items(&entry, &TestObligationPerAxis::Entry);
        assert_eq!(items_as_str(&items), vec!["entry"]);
    }

    #[test]
    fn trait_impl_items_trait_impl_axis_prefixes_trait_ref() {
        let items = projector()
            .trait_impl_items(
                &TypeRef::new("usecase::SharedPort").unwrap(),
                &TestObligationPerAxis::TraitImpl,
            )
            .unwrap();

        assert_eq!(items_as_str(&items), vec!["trait_impl:usecase::SharedPort"]);
    }

    #[test]
    fn trait_impl_items_unsupported_axis_is_none() {
        assert!(
            projector()
                .trait_impl_items(
                    &TypeRef::new("usecase::SharedPort").unwrap(),
                    &TestObligationPerAxis::Method,
                )
                .is_none()
        );
    }

    #[test]
    fn test_projector_interprets_remaining_per_axis_vocabulary_variants() {
        // CN-10: cover the remaining decision-table axes not exercised by
        // the focused invariant, method, handles, and transition tests above.
        let p = projector();
        let order_placed = TypeRef::new("OrderPlaced").unwrap();
        let event_policy = type_entry(
            DataRole::EventPolicy { reacts_to: NonEmptyVec::new(order_placed, vec![]) },
            plain_struct(),
            vec![],
        );
        assert_eq!(
            items_as_str(&p.data_role_items(&event_policy, &TestObligationPerAxis::ReactsTo)),
            vec!["reacts_to:OrderPlaced"]
        );

        let emitting_service = type_entry(
            DataRole::DomainService { emits: vec![TypeRef::new("OrderCreated").unwrap()] },
            plain_struct(),
            vec![],
        );
        assert_eq!(
            items_as_str(&p.data_role_items(&emitting_service, &TestObligationPerAxis::Emits)),
            vec!["emits:OrderCreated"]
        );

        let execute = MethodDeclaration::new(
            MethodName::new("execute").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("Output").unwrap(),
            false,
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );
        let port = TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![execute],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        );
        assert_eq!(
            items_as_str(&p.contract_role_items(&port, &TestObligationPerAxis::TraitMethod)),
            vec!["trait_method:execute"]
        );
        assert_eq!(
            items_as_str(&p.function_role_items(&TestObligationPerAxis::Entry)),
            vec!["entry"]
        );
        assert_eq!(
            items_as_str(
                &p.trait_impl_items(
                    &TypeRef::new("usecase::OrderPort").unwrap(),
                    &TestObligationPerAxis::TraitImpl,
                )
                .unwrap(),
            ),
            vec!["trait_impl:usecase::OrderPort"]
        );
    }

    #[test]
    fn contract_role_items_method_axis_omits_non_derivable_methods_by_name() {
        for (action, name) in [(ItemAction::Reference, "reference"), (ItemAction::Delete, "delete")]
        {
            let entry = type_entry(
                DataRole::domain_service(),
                plain_struct(),
                vec![method(name, action), method("save", ItemAction::Add)],
            );
            assert_eq!(
                items_as_str(&projector().data_role_items(&entry, &TestObligationPerAxis::Method)),
                vec!["method:save"]
            );

            let port = TraitEntry::new(
                ItemAction::Add,
                ContractRole::SecondaryPort,
                vec![method(name, action), method("save", ItemAction::Add)],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            );
            assert_eq!(
                items_as_str(
                    &projector().contract_role_items(&port, &TestObligationPerAxis::TraitMethod)
                ),
                vec!["trait_method:save"]
            );
        }
    }
}
