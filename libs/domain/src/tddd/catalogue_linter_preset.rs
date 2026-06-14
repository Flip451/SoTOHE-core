//! Built-in `ddd-strict` preset for the catalogue linter.
//!
//! This module is declared by `catalogue_linter.rs` via `#[path]` and is not
//! a public module. The `ddd_strict_preset` function is re-exported from the
//! parent module.

use super::{
    CatalogueLinterError, CatalogueLinterRule, CatalogueLinterRuleKind, RoleKind, RuleTarget,
};
use crate::tddd::catalogue_v2::roles::NonEmptyVec;
use crate::tddd::layer_id::LayerId;

/// Returns the `ddd-strict` preset: the minimum core rules from
/// ADR `2026-05-25-0000-tddd-pattern-semantics-extension` §D4-D11 / D16 / D18.
///
/// All 20 rules are constructed from fixed string constants and will never fail
/// at runtime; any construction error is an internal bug and is propagated as
/// `CatalogueLinterError::InvalidRuleConfig`.
///
/// # Errors
///
/// Returns [`CatalogueLinterError::InvalidRuleConfig`] if a preset rule cannot
/// be constructed (indicates a programming error in the preset implementation).
pub fn ddd_strict_preset() -> Result<Vec<CatalogueLinterRule>, CatalogueLinterError> {
    let mut rules: Vec<CatalogueLinterRule> = Vec::new();

    macro_rules! push_rule {
        ($target:expr, $kind:expr) => {{
            let rule = CatalogueLinterRule::new($target, $kind)
                .map_err(|e| CatalogueLinterError::InvalidRuleConfig(e.to_string()))?;
            rules.push(rule);
        }};
    }

    // D4: invariant SelfMethod existence + signature check
    push_rule!(
        RuleTarget::new(vec![RoleKind::Entity, RoleKind::AggregateRoot, RoleKind::ValueObject]),
        CatalogueLinterRuleKind::MethodReferenceSignature { target_field: "invariants".to_owned() }
    );

    // D5: Entity / AggregateRoot identity getter existence + signature
    push_rule!(
        RuleTarget::new(vec![RoleKind::Entity, RoleKind::AggregateRoot]),
        CatalogueLinterRuleKind::AccessorSignatureRequired { target_field: "identity".to_owned() }
    );

    // D5: equality impl declaration check for Entity / AggregateRoot
    push_rule!(
        RuleTarget::new(vec![RoleKind::Entity, RoleKind::AggregateRoot]),
        CatalogueLinterRuleKind::TraitImplRequired {
            required_traits: NonEmptyVec::new("PartialEq".to_owned(), vec!["Eq".to_owned()]),
        }
    );

    // D6: AggregateRoot exclusive_members → Entity role check
    push_rule!(
        RuleTarget::new(vec![RoleKind::AggregateRoot]),
        CatalogueLinterRuleKind::ReferencedRoleConstraint {
            target_field: "exclusive_members".to_owned(),
            expected_role: RoleKind::Entity,
        }
    );

    // D6: AggregateRoot shared_value_objects → ValueObject role check
    push_rule!(
        RuleTarget::new(vec![RoleKind::AggregateRoot]),
        CatalogueLinterRuleKind::ReferencedRoleConstraint {
            target_field: "shared_value_objects".to_owned(),
            expected_role: RoleKind::ValueObject,
        }
    );

    // D7: emits → DomainEvent role check (AggregateRoot + DomainService)
    push_rule!(
        RuleTarget::new(vec![RoleKind::AggregateRoot, RoleKind::DomainService]),
        CatalogueLinterRuleKind::ReferencedRoleConstraint {
            target_field: "emits".to_owned(),
            expected_role: RoleKind::DomainEvent,
        }
    );

    // D8: UseCase.handles → DomainEvent role check
    push_rule!(
        RuleTarget::new(vec![RoleKind::UseCase]),
        CatalogueLinterRuleKind::ReferencedRoleConstraint {
            target_field: "handles".to_owned(),
            expected_role: RoleKind::DomainEvent,
        }
    );

    // D9: DomainEvent &mut self prohibition
    push_rule!(
        RuleTarget::new(vec![RoleKind::DomainEvent]),
        CatalogueLinterRuleKind::ForbiddenMethodReceiver {
            forbidden_receiver: "&mut self".to_owned(),
        }
    );

    // D9: DomainEvent struct public field prohibition
    push_rule!(
        RuleTarget::new(vec![RoleKind::DomainEvent]),
        CatalogueLinterRuleKind::NoPublicField
    );

    // D10: Repository.aggregate → AggregateRoot role check
    push_rule!(
        RuleTarget::new(vec![RoleKind::Repository]),
        CatalogueLinterRuleKind::ReferencedRoleConstraint {
            target_field: "aggregate".to_owned(),
            expected_role: RoleKind::AggregateRoot,
        }
    );

    // D11: exclusive_members uniqueness across AggregateRoots
    push_rule!(
        RuleTarget::new(vec![RoleKind::AggregateRoot]),
        CatalogueLinterRuleKind::FieldElementUniqueAcrossEntries {
            target_field: "exclusive_members".to_owned(),
        }
    );

    // D11: exclusive_members no external reference in methods
    push_rule!(
        RuleTarget::new(vec![RoleKind::AggregateRoot]),
        CatalogueLinterRuleKind::NoExternalReferenceInMethods {
            target_field: "exclusive_members".to_owned(),
        }
    );

    // D11: ValueObject independence (no Entity/AggregateRoot in method sigs)
    push_rule!(
        RuleTarget::new(vec![RoleKind::ValueObject]),
        CatalogueLinterRuleKind::NoRoleInMethodSignature {
            forbidden_roles: NonEmptyVec::new(RoleKind::Entity, vec![RoleKind::AggregateRoot]),
        }
    );

    // D16: EventPolicy reacts_to → DomainEvent role check
    push_rule!(
        RuleTarget::new(vec![RoleKind::EventPolicy]),
        CatalogueLinterRuleKind::ReferencedRoleConstraint {
            target_field: "reacts_to".to_owned(),
            expected_role: RoleKind::DomainEvent,
        }
    );

    // D16: EventPolicy domain-layer-only placement
    push_rule!(
        RuleTarget::new(vec![RoleKind::EventPolicy]),
        CatalogueLinterRuleKind::KindLayerConstraint {
            permitted_layers: NonEmptyVec::new(
                LayerId::try_new("domain".to_owned())
                    .map_err(|e| CatalogueLinterError::InvalidRuleConfig(e.to_string()))?,
                vec![],
            ),
        }
    );

    // D16: EventPolicy &mut self prohibition
    push_rule!(
        RuleTarget::new(vec![RoleKind::EventPolicy]),
        CatalogueLinterRuleKind::ForbiddenMethodReceiver {
            forbidden_receiver: "&mut self".to_owned(),
        }
    );

    // D16: EventPolicy no side-effect roles in method signatures
    push_rule!(
        RuleTarget::new(vec![RoleKind::EventPolicy]),
        CatalogueLinterRuleKind::NoRoleInMethodSignature {
            forbidden_roles: NonEmptyVec::new(RoleKind::Repository, vec![RoleKind::UseCase]),
        }
    );

    // D18: equality impl declaration check for ValueObject
    push_rule!(
        RuleTarget::new(vec![RoleKind::ValueObject]),
        CatalogueLinterRuleKind::TraitImplRequired {
            required_traits: NonEmptyVec::new("PartialEq".to_owned(), vec!["Eq".to_owned()]),
        }
    );

    // D18: ValueObject &mut self prohibition
    push_rule!(
        RuleTarget::new(vec![RoleKind::ValueObject]),
        CatalogueLinterRuleKind::ForbiddenMethodReceiver {
            forbidden_receiver: "&mut self".to_owned(),
        }
    );

    // D18: ValueObject struct public field prohibition
    push_rule!(
        RuleTarget::new(vec![RoleKind::ValueObject]),
        CatalogueLinterRuleKind::NoPublicField
    );

    Ok(rules)
}
