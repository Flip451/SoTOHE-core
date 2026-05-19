//! Tests for `SignalEvaluatorV2` (AC-08).

#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used, non_snake_case)]

use std::collections::{BTreeMap, HashMap};

use domain::tddd::catalogue_v2::ItemAction;
use domain::tddd::{ExtendedCrate, Phase1Error, SignalEvaluatorPort, SignalRegion};
use rustdoc_types::{
    Crate, FORMAT_VERSION, FunctionHeader, FunctionSignature, Generics, Id, Item, ItemEnum,
    ItemKind, ItemSummary, Module, Struct, StructKind, Target, Type, Visibility,
};

use crate::tddd::signal_evaluator_v2::SignalEvaluatorV2;

// -----------------------------------------------------------------------
// Test fixtures
// -----------------------------------------------------------------------

fn empty_crate() -> Crate {
    Crate {
        root: Id(0),
        crate_version: None,
        includes_private: false,
        index: HashMap::new(),
        paths: HashMap::new(),
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    }
}

fn empty_generics() -> Generics {
    Generics { params: vec![], where_predicates: vec![] }
}

fn make_item(id: Id, name: Option<&str>, inner: ItemEnum) -> Item {
    Item {
        id,
        crate_id: 0,
        name: name.map(|s| s.to_string()),
        span: None,
        visibility: Visibility::Public,
        docs: None,
        links: HashMap::new(),
        attrs: vec![],
        deprecation: None,
        inner,
    }
}

fn root_module_item(root_id: Id, crate_name: &str, items: Vec<Id>) -> Item {
    make_item(
        root_id,
        Some(crate_name),
        ItemEnum::Module(Module { is_crate: true, items, is_stripped: false }),
    )
}

fn struct_item(id: Id, name: &str) -> Item {
    make_item(
        id,
        Some(name),
        ItemEnum::Struct(Struct {
            kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
            generics: empty_generics(),
            impls: vec![],
        }),
    )
}

fn simple_crate_with_struct(crate_name: &str, struct_name: &str) -> Crate {
    let root_id = Id(0);
    let struct_id = Id(1);
    let mut index = HashMap::new();
    let mut paths = HashMap::new();

    index.insert(root_id, root_module_item(root_id, crate_name, vec![struct_id]));
    index.insert(struct_id, struct_item(struct_id, struct_name));
    paths.insert(
        struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), struct_name.to_string()],
            kind: ItemKind::Struct,
        },
    );

    Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    }
}

fn extended_crate_with_struct(
    crate_name: &str,
    struct_name: &str,
    action: ItemAction,
) -> ExtendedCrate {
    let krate = simple_crate_with_struct(crate_name, struct_name);
    let struct_id = Id(1);
    let mut actions = BTreeMap::new();
    actions.insert(struct_id, action);
    ExtendedCrate::new(krate, actions)
}

fn simple_fn_item(id: Id, fn_name: &str, is_async: bool) -> Item {
    make_item(
        id,
        Some(fn_name),
        ItemEnum::Function(rustdoc_types::Function {
            sig: FunctionSignature { inputs: vec![], output: None, is_c_variadic: false },
            generics: empty_generics(),
            header: FunctionHeader {
                is_unsafe: false,
                is_const: false,
                is_async,
                abi: rustdoc_types::Abi::Rust,
            },
            has_body: true,
        }),
    )
}

fn simple_crate_with_fn(crate_name: &str, fn_path: &[&str]) -> Crate {
    let fn_name = fn_path.last().unwrap();
    let root_id = Id(0);
    let fn_id = Id(1);
    let mut index = HashMap::new();
    let mut paths = HashMap::new();

    index.insert(root_id, root_module_item(root_id, crate_name, vec![fn_id]));
    index.insert(fn_id, simple_fn_item(fn_id, fn_name, false));
    paths.insert(
        fn_id,
        ItemSummary {
            crate_id: 0,
            path: fn_path.iter().map(|s| s.to_string()).collect(),
            kind: ItemKind::Function,
        },
    );

    Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    }
}

// -----------------------------------------------------------------------
// Region coverage tests — one per non-Skip SignalRegion variant (AC-08)
// -----------------------------------------------------------------------

#[test]
fn test_region_s_intersect_c_match_add_yields_blue() {
    // A has "User" with Add action; B has no "User"; C has "User" with same structure.
    let a = extended_crate_with_struct("my_crate", "User", ItemAction::Add);
    let b = empty_crate();
    let c = simple_crate_with_struct("my_crate", "User");

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let signals: Vec<_> = report.iter().collect();
    let user_signal = signals.iter().find(|s| s.item_name() == "User");
    assert!(user_signal.is_some(), "Expected 'User' signal in report");
    assert_eq!(user_signal.unwrap().region(), SignalRegion::SIntersectC_Match_Add);
    assert!(user_signal.unwrap().signal().is_blue());
}

#[test]
fn test_region_s_intersect_c_match_modify_yields_blue() {
    // A has "User" with Modify action; B has "User"; C has "User" matching A's version.
    let a = extended_crate_with_struct("my_crate", "User", ItemAction::Modify);
    let b = simple_crate_with_struct("my_crate", "User");
    let c = simple_crate_with_struct("my_crate", "User");

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let signals: Vec<_> = report.iter().collect();
    let user_signal = signals.iter().find(|s| s.item_name() == "User");
    assert!(user_signal.is_some(), "Expected 'User' signal in report");
    assert_eq!(user_signal.unwrap().region(), SignalRegion::SIntersectC_Match_Modify);
    assert!(user_signal.unwrap().signal().is_blue());
}

#[test]
fn test_region_s_intersect_c_mismatch_add_yields_yellow() {
    // A has "User" (Add) with one field; B has no "User"; C has "User" but different structure.
    // Build A with a field.
    let a_struct_id = Id(1);
    let a_field_id = Id(2);
    let root_id = Id(0);
    let crate_name = "my_crate";

    let mut a_index = HashMap::new();
    let mut a_paths = HashMap::new();
    a_index.insert(root_id, root_module_item(root_id, crate_name, vec![a_struct_id]));
    a_index.insert(
        a_struct_id,
        make_item(
            a_struct_id,
            Some("User"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![a_field_id], has_stripped_fields: false },
                generics: empty_generics(),
                impls: vec![],
            }),
        ),
    );
    a_index.insert(
        a_field_id,
        make_item(
            a_field_id,
            Some("id"),
            ItemEnum::StructField(Type::Primitive("u64".to_string())),
        ),
    );
    a_paths.insert(
        a_struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "User".to_string()],
            kind: ItemKind::Struct,
        },
    );
    let a_krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: a_index,
        paths: a_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };
    let mut a_actions = BTreeMap::new();
    a_actions.insert(a_struct_id, ItemAction::Add);
    let a = ExtendedCrate::new(a_krate, a_actions);

    // B has no "User".
    let b = empty_crate();

    // C has "User" but as plain struct with no fields (mismatch with A's version).
    let c = simple_crate_with_struct(crate_name, "User");

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let signals: Vec<_> = report.iter().collect();
    let user_signal = signals.iter().find(|s| s.item_name() == "User");
    assert!(user_signal.is_some(), "Expected 'User' signal in report");
    assert_eq!(user_signal.unwrap().region(), SignalRegion::SIntersectC_Mismatch_Add);
    assert!(user_signal.unwrap().signal().is_yellow());
}

#[test]
fn test_region_s_intersect_c_mismatch_modify_yields_yellow() {
    // A has "User" (Modify); B has "User"; C has "User" with different structure.
    // Use a C that has a different struct layout (no fields) vs A (which has a field).
    let a_struct_id = Id(1);
    let a_field_id = Id(2);
    let root_id = Id(0);
    let crate_name = "my_crate";

    let mut a_index = HashMap::new();
    let mut a_paths = HashMap::new();
    a_index.insert(root_id, root_module_item(root_id, crate_name, vec![a_struct_id]));
    a_index.insert(
        a_struct_id,
        make_item(
            a_struct_id,
            Some("User"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![a_field_id], has_stripped_fields: false },
                generics: empty_generics(),
                impls: vec![],
            }),
        ),
    );
    a_index.insert(
        a_field_id,
        make_item(
            a_field_id,
            Some("name"),
            ItemEnum::StructField(Type::Primitive("u32".to_string())),
        ),
    );
    a_paths.insert(
        a_struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "User".to_string()],
            kind: ItemKind::Struct,
        },
    );
    let a_krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: a_index,
        paths: a_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };
    let mut a_actions = BTreeMap::new();
    a_actions.insert(a_struct_id, ItemAction::Modify);
    let a = ExtendedCrate::new(a_krate, a_actions);

    // B has "User" as plain struct with no fields.
    let b = simple_crate_with_struct(crate_name, "User");
    // C also has "User" as plain struct with no fields (same as B, different from A).
    let c = simple_crate_with_struct(crate_name, "User");

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let signals: Vec<_> = report.iter().collect();
    let user_signal = signals.iter().find(|s| s.item_name() == "User");
    assert!(user_signal.is_some(), "Expected 'User' signal");
    assert_eq!(user_signal.unwrap().region(), SignalRegion::SIntersectC_Mismatch_Modify);
    assert!(user_signal.unwrap().signal().is_yellow());
}

#[test]
fn test_region_s_intersect_c_mismatch_reference_yields_red() {
    // A has "User" (Reference); B has "User"; C has "User" with different structure.
    // Build B with a field-bearing struct, C without.
    let root_id = Id(0);
    let crate_name = "my_crate";

    // B has User with one field.
    let b_struct_id = Id(1);
    let b_field_id = Id(2);
    let mut b_index = HashMap::new();
    let mut b_paths = HashMap::new();
    b_index.insert(root_id, root_module_item(root_id, crate_name, vec![b_struct_id]));
    b_index.insert(
        b_struct_id,
        make_item(
            b_struct_id,
            Some("User"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![b_field_id], has_stripped_fields: false },
                generics: empty_generics(),
                impls: vec![],
            }),
        ),
    );
    b_index.insert(
        b_field_id,
        make_item(
            b_field_id,
            Some("id"),
            ItemEnum::StructField(Type::Primitive("u64".to_string())),
        ),
    );
    b_paths.insert(
        b_struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "User".to_string()],
            kind: ItemKind::Struct,
        },
    );
    let b = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: b_index,
        paths: b_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    // A has User (Reference) — same as B (Reference means "it's in B, don't change").
    let a_struct_id = Id(1);
    let a_field_id = Id(2);
    let mut a_index = HashMap::new();
    let mut a_paths = HashMap::new();
    a_index.insert(root_id, root_module_item(root_id, crate_name, vec![a_struct_id]));
    a_index.insert(
        a_struct_id,
        make_item(
            a_struct_id,
            Some("User"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![a_field_id], has_stripped_fields: false },
                generics: empty_generics(),
                impls: vec![],
            }),
        ),
    );
    a_index.insert(
        a_field_id,
        make_item(
            a_field_id,
            Some("id"),
            ItemEnum::StructField(Type::Primitive("u64".to_string())),
        ),
    );
    a_paths.insert(
        a_struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "User".to_string()],
            kind: ItemKind::Struct,
        },
    );
    let a_krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: a_index,
        paths: a_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };
    let mut a_actions = BTreeMap::new();
    a_actions.insert(a_struct_id, ItemAction::Reference);
    let a = ExtendedCrate::new(a_krate, a_actions);

    // C has User without any fields (different structure → mismatch with S's Reference item).
    let c = simple_crate_with_struct(crate_name, "User");

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let signals: Vec<_> = report.iter().collect();
    let user_signal = signals.iter().find(|s| s.item_name() == "User");
    assert!(user_signal.is_some(), "Expected 'User' signal");
    assert_eq!(user_signal.unwrap().region(), SignalRegion::SIntersectC_Mismatch_Reference);
    assert!(user_signal.unwrap().signal().is_red());
}

#[test]
fn test_region_s_minus_c_add_yields_yellow() {
    // A has "NewType" (Add); B has no "NewType"; C has no "NewType" → add not yet done.
    let a = extended_crate_with_struct("my_crate", "NewType", ItemAction::Add);
    let b = empty_crate();
    let c = empty_crate();

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let signals: Vec<_> = report.iter().collect();
    let signal = signals.iter().find(|s| s.item_name() == "NewType");
    assert!(signal.is_some(), "Expected 'NewType' signal");
    assert_eq!(signal.unwrap().region(), SignalRegion::SMinusC_Add);
    assert!(signal.unwrap().signal().is_yellow());
}

#[test]
fn test_region_s_minus_c_reference_yields_red() {
    // A has "User" (Reference); B has "User"; C has no "User" → reference contract violated.
    let a = extended_crate_with_struct("my_crate", "User", ItemAction::Reference);
    let b = simple_crate_with_struct("my_crate", "User");
    let c = empty_crate();

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let signals: Vec<_> = report.iter().collect();
    let signal = signals.iter().find(|s| s.item_name() == "User");
    assert!(signal.is_some(), "Expected 'User' signal");
    assert_eq!(signal.unwrap().region(), SignalRegion::SMinusC_Reference);
    assert!(signal.unwrap().signal().is_red());
}

#[test]
fn test_region_s_minus_c_modify_yields_red() {
    // A has "User" (Modify); B has "User"; C has no "User" → modify declared but deleted.
    let a = extended_crate_with_struct("my_crate", "User", ItemAction::Modify);
    let b = simple_crate_with_struct("my_crate", "User");
    let c = empty_crate();

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let signals: Vec<_> = report.iter().collect();
    let signal = signals.iter().find(|s| s.item_name() == "User");
    assert!(signal.is_some(), "Expected 'User' signal");
    assert_eq!(signal.unwrap().region(), SignalRegion::SMinusC_Modify);
    assert!(signal.unwrap().signal().is_red());
}

#[test]
fn test_region_d_intersect_c_yields_yellow() {
    // A has "OldType" (Delete); B has "OldType"; C still has "OldType" → delete in progress.
    let a = extended_crate_with_struct("my_crate", "OldType", ItemAction::Delete);
    let b = simple_crate_with_struct("my_crate", "OldType");
    let c = simple_crate_with_struct("my_crate", "OldType");

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let signals: Vec<_> = report.iter().collect();
    let signal = signals.iter().find(|s| s.item_name() == "OldType");
    assert!(signal.is_some(), "Expected 'OldType' signal");
    assert_eq!(signal.unwrap().region(), SignalRegion::DIntersectC);
    assert!(signal.unwrap().signal().is_yellow());
}

#[test]
fn test_region_d_minus_c_yields_blue() {
    // A has "OldType" (Delete); B has "OldType"; C has no "OldType" → delete achieved.
    let a = extended_crate_with_struct("my_crate", "OldType", ItemAction::Delete);
    let b = simple_crate_with_struct("my_crate", "OldType");
    let c = empty_crate();

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let signals: Vec<_> = report.iter().collect();
    let signal = signals.iter().find(|s| s.item_name() == "OldType");
    assert!(signal.is_some(), "Expected 'OldType' signal");
    assert_eq!(signal.unwrap().region(), SignalRegion::DMinusC);
    assert!(signal.unwrap().signal().is_blue());
}

#[test]
fn test_region_c_minus_s_union_d_yields_red() {
    // A: empty (no declarations); B: empty; C has "GhostType" → undeclared.
    let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
    let b = empty_crate();
    let c = simple_crate_with_struct("my_crate", "GhostType");

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let signals: Vec<_> = report.iter().collect();
    let signal = signals.iter().find(|s| s.item_name() == "GhostType");
    assert!(signal.is_some(), "Expected 'GhostType' signal");
    assert_eq!(signal.unwrap().region(), SignalRegion::CMinusSUnionD);
    assert!(signal.unwrap().signal().is_red());
}

// -----------------------------------------------------------------------
// Phase 1 error tests
// -----------------------------------------------------------------------

#[test]
fn test_phase1_error_add_for_existing_in_b_returns_action_contradiction() {
    // A declares "User" as Add, but "User" already exists in B → contradiction.
    let a = extended_crate_with_struct("my_crate", "User", ItemAction::Add);
    let b = simple_crate_with_struct("my_crate", "User");
    let c = empty_crate();

    let evaluator = SignalEvaluatorV2::new();
    let result = evaluator.evaluate(a, b, c);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Phase1Error::ActionContradiction(_)));
}

#[test]
fn test_phase1_error_modify_for_nonexistent_in_b_returns_action_contradiction() {
    // A declares "NewType" as Modify, but "NewType" doesn't exist in B.
    let a = extended_crate_with_struct("my_crate", "NewType", ItemAction::Modify);
    let b = empty_crate();
    let c = empty_crate();

    let evaluator = SignalEvaluatorV2::new();
    let result = evaluator.evaluate(a, b, c);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Phase1Error::ActionContradiction(_)));
}

#[test]
fn test_phase1_error_reference_for_nonexistent_in_b_returns_action_contradiction() {
    // A declares "User" as Reference, but "User" doesn't exist in B.
    let a = extended_crate_with_struct("my_crate", "User", ItemAction::Reference);
    let b = empty_crate();
    let c = empty_crate();

    let evaluator = SignalEvaluatorV2::new();
    let result = evaluator.evaluate(a, b, c);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Phase1Error::ActionContradiction(_)));
}

#[test]
fn test_phase1_error_delete_for_nonexistent_in_b_returns_action_contradiction() {
    // A declares "User" as Delete, but "User" doesn't exist in B.
    let a = extended_crate_with_struct("my_crate", "User", ItemAction::Delete);
    let b = empty_crate();
    let c = empty_crate();

    let evaluator = SignalEvaluatorV2::new();
    let result = evaluator.evaluate(a, b, c);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Phase1Error::ActionContradiction(_)));
}

#[test]
fn test_phase1_error_unresolved_type_ref_yields_unresolved_type_ref_error() {
    // A has "User" (Add) with a field typed "UnknownRef" (unresolved marker).
    // B has no "UnknownRef" and it's not in S → Phase 1.5 should reject.
    let root_id = Id(0);
    let struct_id = Id(1);
    let field_id = Id(2);
    let crate_name = "my_crate";

    let mut a_index = HashMap::new();
    let mut a_paths = HashMap::new();
    a_index.insert(root_id, root_module_item(root_id, crate_name, vec![struct_id]));
    a_index.insert(
        struct_id,
        make_item(
            struct_id,
            Some("User"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![field_id], has_stripped_fields: false },
                generics: empty_generics(),
                impls: vec![],
            }),
        ),
    );
    // Field typed "UnknownRef" as unresolved marker (crate_id == UNRESOLVED_CRATE_ID).
    a_index.insert(
        field_id,
        make_item(
            field_id,
            Some("data"),
            ItemEnum::StructField(Type::ResolvedPath(rustdoc_types::Path {
                path: "UnknownRef".to_string(),
                id: Id(crate::tddd::type_ref_parser::UNRESOLVED_CRATE_ID),
                args: None,
            })),
        ),
    );
    a_paths.insert(
        struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "User".to_string()],
            kind: ItemKind::Struct,
        },
    );
    let a_krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: a_index,
        paths: a_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };
    let mut a_actions = BTreeMap::new();
    a_actions.insert(struct_id, ItemAction::Add);
    let a = ExtendedCrate::new(a_krate, a_actions);

    let evaluator = SignalEvaluatorV2::new();
    let result = evaluator.evaluate(a, empty_crate(), empty_crate());
    assert!(result.is_err(), "Expected UnresolvedTypeRef error");
    assert!(
        matches!(result.unwrap_err(), Phase1Error::UnresolvedTypeRef(_)),
        "Expected UnresolvedTypeRef variant"
    );
}

#[test]
fn test_phase1_error_dangling_id_after_delete_yields_dangling_id_error() {
    // B has "Order" (struct with a field that has type "UserId") AND "UserId".
    // A deletes "UserId".
    // When B's Order is seeded into S, its field references B's UserId Id.
    // After Phase 1 deletes UserId (moves it from S to D), Order's field still
    // references B's UserId Id — that Id is no longer in S → DanglingId.
    let root_id = Id(0);
    let b_order_id = Id(1);
    let b_user_id = Id(2);
    let b_field_id = Id(3);
    let crate_name = "my_crate";

    // Build B: Order (struct with UserId field) + UserId
    let mut b_index = HashMap::new();
    let mut b_paths = HashMap::new();
    b_index.insert(root_id, root_module_item(root_id, crate_name, vec![b_order_id, b_user_id]));
    // Order struct with field pointing to UserId (by b_user_id).
    b_index.insert(
        b_order_id,
        make_item(
            b_order_id,
            Some("Order"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![b_field_id], has_stripped_fields: false },
                generics: empty_generics(),
                impls: vec![],
            }),
        ),
    );
    // Field item pointing to b_user_id (B's UserId Id).
    b_index.insert(
        b_field_id,
        make_item(
            b_field_id,
            Some("user_id"),
            ItemEnum::StructField(Type::ResolvedPath(rustdoc_types::Path {
                path: "UserId".to_string(),
                id: b_user_id, // references B's UserId
                args: None,
            })),
        ),
    );
    // UserId struct in B.
    b_index.insert(b_user_id, struct_item(b_user_id, "UserId"));
    b_paths.insert(
        b_order_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Order".to_string()],
            kind: ItemKind::Struct,
        },
    );
    b_paths.insert(
        b_user_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "UserId".to_string()],
            kind: ItemKind::Struct,
        },
    );
    let b = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: b_index,
        paths: b_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    // A: declares UserId as Delete. Order has no A declaration → implicit Reference in S.
    let a_user_id = Id(1);
    let mut a_index = HashMap::new();
    let mut a_paths = HashMap::new();
    a_index.insert(root_id, root_module_item(root_id, crate_name, vec![a_user_id]));
    a_index.insert(a_user_id, struct_item(a_user_id, "UserId"));
    a_paths.insert(
        a_user_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "UserId".to_string()],
            kind: ItemKind::Struct,
        },
    );
    let a_krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: a_index,
        paths: a_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };
    let mut a_actions = BTreeMap::new();
    a_actions.insert(a_user_id, ItemAction::Delete);
    let a = ExtendedCrate::new(a_krate, a_actions);

    let evaluator = SignalEvaluatorV2::new();
    let result = evaluator.evaluate(a, b, empty_crate());
    // Phase 1.6: Order's field (seeded from B) references B's UserId Id (b_user_id = Id(2)).
    // After Delete, UserId is moved from S to D with a new Id.
    // B's b_user_id (Id(2)) is no longer in S → DanglingId.
    assert!(result.is_err(), "Expected DanglingId error; got: {:?}", result);
    assert!(
        matches!(result.unwrap_err(), Phase1Error::DanglingId(_)),
        "Expected DanglingId variant"
    );
}

// -----------------------------------------------------------------------
// Identity boundary tests
// -----------------------------------------------------------------------

#[test]
fn test_function_identity_uses_function_path() {
    // A has function at "my_crate::module::compute" (Add); B has no such function;
    // C has the same function → SIntersectC_Match_Add.
    let fn_path = &["my_crate", "module", "compute"];
    let a_krate = simple_crate_with_fn("my_crate", fn_path);
    let fn_id = Id(1);
    let mut a_actions = BTreeMap::new();
    a_actions.insert(fn_id, ItemAction::Add);
    let a = ExtendedCrate::new(a_krate, a_actions);

    let b = empty_crate();
    let c = simple_crate_with_fn("my_crate", fn_path);

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let fn_path_str = fn_path.join("::");
    let signals: Vec<_> = report.iter().collect();
    let signal = signals.iter().find(|s| s.item_name() == fn_path_str);
    assert!(signal.is_some(), "Expected function signal '{fn_path_str}' in report");
    assert_eq!(signal.unwrap().region(), SignalRegion::SIntersectC_Match_Add);
}

#[test]
fn test_empty_a_and_b_with_empty_c_produces_empty_report() {
    let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
    let b = empty_crate();
    let c = empty_crate();

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();
    assert!(report.is_empty(), "Expected empty report for empty inputs");
}

#[test]
fn test_b_only_item_not_in_c_yields_s_minus_c_reference_red() {
    // B has "Legacy" (no A declaration); C has no "Legacy" → S has "Legacy" as implicit
    // Reference → S \ C + Reference → Red.
    let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
    let b = simple_crate_with_struct("my_crate", "Legacy");
    let c = empty_crate();

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    let signals: Vec<_> = report.iter().collect();
    let signal = signals.iter().find(|s| s.item_name() == "Legacy");
    assert!(signal.is_some(), "Expected 'Legacy' signal");
    assert_eq!(signal.unwrap().region(), SignalRegion::SMinusC_Reference);
    assert!(signal.unwrap().signal().is_red());
}

#[test]
fn test_s_intersect_c_match_reference_is_skipped_in_report() {
    // B has "Maintained"; C has "Maintained" with same structure → Skip (not in report).
    let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
    let b = simple_crate_with_struct("my_crate", "Maintained");
    let c = simple_crate_with_struct("my_crate", "Maintained");

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    // The "Maintained" item is in S ∩ C with Reference action and matching structure → Skip.
    // Report should be empty (all skip signals are filtered).
    assert!(
        report.is_empty(),
        "Expected empty report (all maintained items are skip); got: {} signals",
        report.len()
    );
}

// -----------------------------------------------------------------------
// Compiler-internal trait filter tests (T039: replaces DERIVE_TRAIT_NAMES)
// -----------------------------------------------------------------------

/// Helper: build a crate with a struct that has a trait impl item (any trait).
///
/// The trait ID (`Id(9999)`) is NOT inserted into `krate.paths`, so
/// `build_impl_identity_map` uses the string-based fallback
/// (`normalize_impl_trait_path`) to compute the identity key.  This matches
/// the A-side (catalogue codec) code path where synthetic trait IDs have no
/// `paths` entry.
fn crate_with_trait_impl(crate_name: &str, struct_name: &str, trait_name: &str) -> Crate {
    use rustdoc_types::{Impl, Path as RdPath};

    let root_id = Id(0);
    let struct_id = Id(1);
    let impl_id = Id(2);

    let mut index = HashMap::new();
    let mut paths = HashMap::new();

    index.insert(root_id, root_module_item(root_id, crate_name, vec![struct_id, impl_id]));
    index.insert(struct_id, struct_item(struct_id, struct_name));
    paths.insert(
        struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), struct_name.to_string()],
            kind: ItemKind::Struct,
        },
    );

    let trait_impl = Impl {
        is_unsafe: false,
        generics: empty_generics(),
        provided_trait_methods: vec![],
        trait_: Some(RdPath { path: trait_name.to_string(), id: Id(9999), args: None }),
        for_: rustdoc_types::Type::ResolvedPath(RdPath {
            path: struct_name.to_string(),
            id: struct_id,
            args: None,
        }),
        items: vec![],
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    };
    index.insert(impl_id, make_item(impl_id, None, ItemEnum::Impl(trait_impl)));

    Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    }
}

/// Helper: build a crate with a struct that has an external trait impl.
///
/// The trait ID (`Id(9999)`) IS inserted into `krate.paths` with `crate_id = 1`
/// (non-zero → external crate) and the supplied `trait_path_segments`.  This
/// models the C-side (rustdoc) code path where external traits have a `paths`
/// entry.  `external_crate_name` is registered in `krate.external_crates` so
/// the single-segment expansion logic in `build_impl_identity_map` works.
fn crate_with_external_trait_impl(
    crate_name: &str,
    struct_name: &str,
    trait_path_segments: &[&str],
    external_crate_name: &str,
) -> Crate {
    use rustdoc_types::{ExternalCrate, Impl, Path as RdPath};

    let root_id = Id(0);
    let struct_id = Id(1);
    let impl_id = Id(2);
    let trait_id = Id(9999);
    let ext_crate_id = 1u32;

    let mut index = HashMap::new();
    let mut paths = HashMap::new();
    let mut external_crates = HashMap::new();

    index.insert(root_id, root_module_item(root_id, crate_name, vec![struct_id, impl_id]));
    index.insert(struct_id, struct_item(struct_id, struct_name));
    paths.insert(
        struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), struct_name.to_string()],
            kind: ItemKind::Struct,
        },
    );
    // Register the external trait in krate.paths with crate_id != 0.
    paths.insert(
        trait_id,
        ItemSummary {
            crate_id: ext_crate_id,
            path: trait_path_segments.iter().map(|s| s.to_string()).collect(),
            kind: ItemKind::Trait,
        },
    );
    external_crates.insert(
        ext_crate_id,
        ExternalCrate {
            name: external_crate_name.to_string(),
            html_root_url: None,
            path: std::path::PathBuf::new(),
        },
    );

    let trait_name = trait_path_segments.last().copied().unwrap_or("");
    let trait_impl = Impl {
        is_unsafe: false,
        generics: empty_generics(),
        provided_trait_methods: vec![],
        trait_: Some(RdPath { path: trait_path_segments.join("::"), id: trait_id, args: None }),
        for_: rustdoc_types::Type::ResolvedPath(RdPath {
            path: struct_name.to_string(),
            id: struct_id,
            args: None,
        }),
        items: vec![],
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    };
    // Suppress unused warning from trait_name binding.
    let _ = trait_name;
    index.insert(impl_id, make_item(impl_id, None, ItemEnum::Impl(trait_impl)));

    Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates,
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    }
}

/// T039 (AC-14, case b): `StructuralPartialEq` / `StructuralEq` Impls present
/// only in C must NOT trigger `CMinusSUnionD`.  These are compiler-internal
/// phantom traits that cannot be declared in any workspace catalogue.
///
/// The fixture uses `crate_with_external_trait_impl` (trait ID in `krate.paths`
/// with `crate_id != 0`) to model real rustdoc output, where the compiler-internal
/// traits always come from an external crate (`core`).
#[test]
fn test_t039_compiler_internal_traits_excluded_from_identity_map() {
    for (trait_segments, qualified_name) in [
        (["core", "marker", "StructuralPartialEq"].as_slice(), "core::marker::StructuralPartialEq"),
        (["core", "marker", "StructuralEq"].as_slice(), "core::marker::StructuralEq"),
    ] {
        let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
        let b = empty_crate();
        let c = crate_with_external_trait_impl("my_crate", "MyStruct", trait_segments, "core");

        let evaluator = SignalEvaluatorV2::new();
        let report = evaluator.evaluate(a, b, c).unwrap();

        let impl_signal =
            report.iter().find(|s| s.item_name().contains(&format!(": {qualified_name}")));
        assert!(
            impl_signal.is_none(),
            "T039: {qualified_name} (compiler-internal) must not produce a CMinusSUnionD signal; \
             got: {:?}",
            impl_signal.map(|s| s.item_name())
        );
    }
}

/// T039 (AC-14): a LOCAL user-defined trait named `StructuralEq` (crate_id == 0 in
/// krate.paths) must NOT be silently excluded.  `build_impl_identity_map` applies
/// the compiler-internal filter only to external (crate_id != 0) or path-less
/// (fallback) traits, so a local trait with the same short name is always visible.
#[test]
fn test_t039_local_structural_eq_trait_not_filtered() {
    // Use crate_with_trait_impl with a trait ID NOT in krate.paths (synthetic fallback)
    // but then simulate what happens when krate.paths DOES have crate_id == 0.
    // The easiest approach: use crate_with_external_trait_impl with crate_id == 0 (local).
    use rustdoc_types::{Impl, Path as RdPath};

    let crate_name = "my_crate";
    let struct_name = "MyStruct";
    let root_id = Id(0);
    let struct_id = Id(1);
    let impl_id = Id(2);
    let trait_id = Id(9999);

    let mut index = HashMap::new();
    let mut paths = HashMap::new();

    index.insert(root_id, root_module_item(root_id, crate_name, vec![struct_id, impl_id]));
    index.insert(struct_id, struct_item(struct_id, struct_name));
    paths.insert(
        struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), struct_name.to_string()],
            kind: ItemKind::Struct,
        },
    );
    // Register the trait in krate.paths with crate_id == 0 (LOCAL user-defined trait).
    paths.insert(
        trait_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "StructuralEq".to_string()],
            kind: ItemKind::Trait,
        },
    );

    let trait_impl = Impl {
        is_unsafe: false,
        generics: empty_generics(),
        provided_trait_methods: vec![],
        trait_: Some(RdPath { path: "StructuralEq".to_string(), id: trait_id, args: None }),
        for_: Type::ResolvedPath(RdPath {
            path: struct_name.to_string(),
            id: struct_id,
            args: None,
        }),
        items: vec![],
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    };
    index.insert(impl_id, make_item(impl_id, None, ItemEnum::Impl(trait_impl)));

    let c = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
    let b = empty_crate();

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    // The local user-defined trait "StructuralEq" (crate_id == 0) must NOT be filtered;
    // its impl should produce a CMinusSUnionD signal.
    let impl_signal = report.iter().find(|s| s.item_name().contains(": StructuralEq"));
    assert!(
        impl_signal.is_some(),
        "T039: a LOCAL user-defined trait named StructuralEq (crate_id == 0) must NOT be \
         filtered by the compiler-internal guard; all signals: {:?}",
        report.iter().map(|s| s.item_name()).collect::<Vec<_>>()
    );
}

/// T039 (AC-14, case c): provenance-based filter is removed.  Without a catalogue
/// declaration, derive-generated trait impls (Clone, Debug, Copy, IntoStaticStr)
/// must now appear in `CMinusSUnionD` — the catalogue must declare them via
/// `trait_impls` per ADR `2026-05-08-0305` D9.
///
/// Note: `crate_with_trait_impl` uses a synthetic trait ID not present in
/// `krate.paths`, so the identity key is built via `normalize_impl_trait_path`.
/// That function expands well-known core trait short names to their canonical
/// fully-qualified form (e.g. `"Clone"` → `"core::clone::Clone"`), so the signal
/// name to search for uses the canonical path, not the raw short name.
#[test]
fn test_t039_derive_traits_no_longer_filtered() {
    // (input_trait_name_for_rustdoc, expected_canonical_key_fragment_in_signal_name)
    for (trait_name, canonical_key) in [
        ("Clone", "core::clone::Clone"),
        ("Copy", "core::marker::Copy"),
        ("Debug", "core::fmt::Debug"),
        // IntoStaticStr is not a known core trait; normalize_impl_trait_path keeps
        // the bare short name unchanged.
        ("IntoStaticStr", "IntoStaticStr"),
    ] {
        let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
        let b = empty_crate();
        let c = crate_with_trait_impl("my_crate", "MyStruct", trait_name);

        let evaluator = SignalEvaluatorV2::new();
        let report = evaluator.evaluate(a, b, c).unwrap();

        let impl_signal =
            report.iter().find(|s| s.item_name().contains(&format!(": {canonical_key}")));
        assert!(
            impl_signal.is_some(),
            "T039: {trait_name} impl must produce a signal (no longer provenance-filtered); \
             expected key fragment '`: {canonical_key}`'; \
             all signals: {:?}",
            report.iter().map(|s| s.item_name()).collect::<Vec<_>>()
        );
    }
}

/// T039 (AC-14, case a): the compiler-internal trait exclusion is an exact
/// allowlist covering all normalized forms — qualified paths from `krate.paths`,
/// the two-segment `core::` fallback, and bare short-names from the
/// `normalize_impl_trait_path` fallback.  Third-party paths sharing only the short
/// name and all derive-relevant traits must NOT pass.
#[test]
fn test_t039_compiler_internal_trait_classifier_scope() {
    use crate::tddd::signal_evaluator_v2::is_compiler_internal_trait;

    // Exact qualified paths from the allowlist match.
    assert!(is_compiler_internal_trait("core::marker::StructuralPartialEq"));
    assert!(is_compiler_internal_trait("core::marker::StructuralEq"));
    assert!(is_compiler_internal_trait("std::marker::StructuralPartialEq"));
    assert!(is_compiler_internal_trait("std::marker::StructuralEq"));
    // Fallback-form (two segments: core_canonical_path fallback for unknown names) also matches.
    assert!(is_compiler_internal_trait("core::StructuralPartialEq"));
    assert!(is_compiler_internal_trait("core::StructuralEq"));
    // Bare short-name fallback (normalize_impl_trait_path when ID absent from krate.paths).
    assert!(is_compiler_internal_trait("StructuralPartialEq"));
    assert!(is_compiler_internal_trait("StructuralEq"));

    // Third-party qualified paths with the same short name do NOT match (exact-path check).
    assert!(
        !is_compiler_internal_trait("foo::StructuralEq"),
        "third-party qualified path must not match — only core/std compiler-internal paths are excluded"
    );
    assert!(
        !is_compiler_internal_trait("my_crate::StructuralPartialEq"),
        "workspace-crate qualified path must not match"
    );

    // No derive-relevant trait qualified path passes — they must all be declared via catalogue.
    for trait_name in [
        "core::clone::Clone",
        "core::marker::Copy",
        "core::fmt::Debug",
        "IntoStaticStr",
        "core::cmp::PartialEq",
        "core::cmp::Eq",
        "core::hash::Hash",
        "core::default::Default",
        "serde::Serialize",
        "serde::Deserialize",
        "core::convert::From",
        "core::fmt::Display",
        "core::convert::TryFrom",
    ] {
        assert!(
            !is_compiler_internal_trait(trait_name),
            "T039: {trait_name} must NOT be filtered — catalogue must declare it"
        );
    }
}

// ---------------------------------------------------------------------------
// normalize_impl_trait_path unit tests
// ---------------------------------------------------------------------------

#[test]
fn test_normalize_impl_trait_path_bare_known_core_trait_expands_to_qualified() {
    use super::normalize_impl_trait_path;
    // Bare `From` → expanded to canonical three-segment path.
    assert_eq!(normalize_impl_trait_path("From", "my_crate"), "core::convert::From");
    // Bare `Display` → expanded to `core::fmt::Display`.
    assert_eq!(normalize_impl_trait_path("Display", "my_crate"), "core::fmt::Display");
}

#[test]
fn test_normalize_impl_trait_path_crate_prefix_does_not_expand_to_core() {
    use super::normalize_impl_trait_path;
    // `crate::Display` is a local-crate type, NOT `core::fmt::Display`.
    // Must strip to short name, not expand via core_canonical_path.
    assert_eq!(normalize_impl_trait_path("crate::Display", "my_crate"), "Display");
    assert_eq!(normalize_impl_trait_path("crate::MyTrait", "my_crate"), "MyTrait");
}

#[test]
fn test_normalize_impl_trait_path_self_and_super_prefix_strips_to_short_name() {
    use super::normalize_impl_trait_path;
    assert_eq!(normalize_impl_trait_path("self::Foo", "my_crate"), "Foo");
    assert_eq!(normalize_impl_trait_path("super::Bar", "my_crate"), "Bar");
}

#[test]
fn test_normalize_impl_trait_path_local_crate_rustdoc_path_strips_to_short_name() {
    use super::normalize_impl_trait_path;
    // rustdoc emits `my_crate::MyTrait` for local traits.
    assert_eq!(normalize_impl_trait_path("my_crate::MyTrait", "my_crate"), "MyTrait");
}

#[test]
fn test_normalize_impl_trait_path_external_path_preserved_verbatim() {
    use super::normalize_impl_trait_path;
    assert_eq!(normalize_impl_trait_path("serde::Serialize", "my_crate"), "serde::Serialize");
    assert_eq!(normalize_impl_trait_path("core::convert::From", "my_crate"), "core::convert::From");
}

#[test]
fn test_normalize_impl_trait_path_preserves_generic_args() {
    use super::normalize_impl_trait_path;
    // Bare known trait with generic args.
    assert_eq!(
        normalize_impl_trait_path("From<String>", "my_crate"),
        "core::convert::From<String>"
    );
    // crate:: prefix with generic args — strip prefix, keep args.
    assert_eq!(normalize_impl_trait_path("crate::MyTrait<u32>", "my_crate"), "MyTrait<u32>");
}

// -----------------------------------------------------------------------
// T036 regression: Phase 1.45 rewrite must not touch B-sourced Reference fns
// -----------------------------------------------------------------------

/// Regression test for T036 (updated for T037 B-side renumbering).
///
/// ## Original T036 problem (still covered)
///
/// Before T036, the Phase 1.45 discriminator was
/// `should_rewrite = a_sourced_top_ids.contains(&item_id) || item_id.0 >= first_fresh_id`.
/// Because `insert_s_fn` allocates a fresh Id for every function (including B-sourced
/// Reference functions), the `>= first_fresh_id` clause would incorrectly select them for
/// rewrite.  When A's `full_remap` collided numerically with a B-function's type-ref Id,
/// the signature was corrupted.  T036 fixed this by using `s_actions` as the discriminator.
///
/// ## T037 extension (B-side Id renumbering)
///
/// After T037, B-sourced items are no longer inserted at their original B-side Ids — all B
/// items are renumbered to fresh S Ids via `b_id_remap`.  Crucially, B-side type refs
/// (e.g. `process`'s param type `Id(1)` → B's `Bar`) are also rewritten via `b_id_remap`
/// during B-item insertion, so `process`'s param will point to Bar's fresh S Id after Phase 1.
///
/// The test still verifies structural consistency:
///   - `process_param_id` = the Id in `process`'s param type after Phase 1.
///   - `bar_id` = the Id of `Bar` in S after Phase 1 (fresh S Id, NOT Id(1) after T037).
///   - These must be equal: `process`'s param must still reference `Bar` (not `Foo`).
///
/// Setup:
///   - B: `Bar` at Id(1), `process(x: Bar)` at Id(2). `process`'s param refs Id(1).
///   - A: `Foo` at Id(1), action = Add.
///   - After Phase 1, both `Bar` and `process` are renumbered; param must still point to Bar.
#[test]
#[allow(clippy::panic, clippy::assertions_on_constants)]
fn test_t036_phase1_45_does_not_rewrite_b_sourced_reference_function_param_id() {
    use rustdoc_types::Path as RdPath;

    use super::phase1::phase1_build_s_and_d;

    // --- Construct B: struct `Bar` at Id(1), fn `process(x: Bar)` at Id(2) ---
    let crate_name = "my_crate";
    let b_root = Id(0);
    let b_bar_id = Id(1);
    let b_process_id = Id(2);

    let mut b_index = HashMap::new();
    let mut b_paths = HashMap::new();

    b_index.insert(b_root, root_module_item(b_root, crate_name, vec![b_bar_id, b_process_id]));
    b_index.insert(b_bar_id, struct_item(b_bar_id, "Bar"));
    // `process(x: Bar)` — parameter type is `Type::ResolvedPath` pointing to b_bar_id.
    b_index.insert(
        b_process_id,
        make_item(
            b_process_id,
            Some("process"),
            ItemEnum::Function(rustdoc_types::Function {
                sig: FunctionSignature {
                    inputs: vec![(
                        "x".to_string(),
                        Type::ResolvedPath(RdPath {
                            path: "Bar".to_string(),
                            id: b_bar_id, // references B's Bar (Id(1))
                            args: None,
                        }),
                    )],
                    output: None,
                    is_c_variadic: false,
                },
                generics: empty_generics(),
                header: FunctionHeader {
                    is_unsafe: false,
                    is_const: false,
                    is_async: false,
                    abi: rustdoc_types::Abi::Rust,
                },
                has_body: true,
            }),
        ),
    );
    b_paths.insert(
        b_bar_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Bar".to_string()],
            kind: ItemKind::Struct,
        },
    );
    b_paths.insert(
        b_process_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "process".to_string()],
            kind: ItemKind::Function,
        },
    );
    let b = Crate {
        root: b_root,
        crate_version: None,
        includes_private: false,
        index: b_index,
        paths: b_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    // --- Construct A: struct `Foo` at Id(1), action = Add (collides with B's Bar Id) ---
    let a = extended_crate_with_struct(crate_name, "Foo", ItemAction::Add);

    // --- Run Phase 1 ---
    let (s, _d) = phase1_build_s_and_d(a, &b).expect("phase 1 should succeed");

    // --- Locate `process` in S by name ---
    let process_item = s
        .krate()
        .index
        .values()
        .find(|item| item.name.as_deref() == Some("process"))
        .expect("`process` fn must be present in S");

    // Extract `process`'s parameter type info (Id + path string). These
    // structural assertions guard against an unexpected rustdoc shape changing
    // under us; they are not the regression check itself.
    let process_fn = match &process_item.inner {
        ItemEnum::Function(f) => f,
        other => {
            assert!(false, "expected Function inner, got {other:?}");
            return;
        }
    };
    let (_, param_ty) = process_fn.sig.inputs.first().expect("`process` has one param");
    let param_path = match param_ty {
        Type::ResolvedPath(p) => p,
        other => {
            assert!(false, "expected ResolvedPath for param, got {other:?}");
            return;
        }
    };
    let process_param_id = param_path.id;
    let process_param_path = param_path.path.clone();

    // --- Locate `Bar` in S by name (after T037 renumbering, Bar is no longer at Id(1)) ---
    let bar_item = s
        .krate()
        .index
        .values()
        .find(|item| item.name.as_deref() == Some("Bar"))
        .expect("`Bar` struct must be present in S");
    let bar_id = bar_item.id;

    // --- Assertion: process's param Id must still point to Bar, not to Foo ---
    // After T037, both Bar and process are renumbered to fresh S Ids via b_id_remap.
    // `process`'s param type-ref was also rewritten via b_id_remap, so it now points
    // to Bar's fresh S Id.  The assertion checks structural consistency: the param
    // should reference Bar (not Foo or any other spurious id).
    assert_eq!(
        process_param_id, bar_id,
        "T036/T037 regression: process(x: Bar) param Id must reference Bar's S-Id after Phase 1 \
         (got {process_param_id:?}, expected {bar_id:?}). A regression would map the param to \
         Foo's fresh S-Id or leave it at a stale original B Id."
    );

    // Defense-in-depth: param's path string is preserved (Phase 1.45 rewrites
    // Ids only, but a path drift here would indicate deeper corruption).
    assert_eq!(
        process_param_path, "Bar",
        "param path string must be preserved after Phase 1.45 rewrite"
    );
}

// -----------------------------------------------------------------------
// T037 regression: B-side Id renumbering — collision fixture
// -----------------------------------------------------------------------

/// T037 regression: both A and B have an item at the same numeric Id(1),
/// but they represent completely different types.  After Phase 1:
///
/// - S must contain BOTH `Foo` (from A, Add action) and `Bar` (from B, Reference action).
/// - They must occupy DISTINCT fresh S Ids (no collision or clobber).
/// - No spurious Phase 2 signals should result from Id confusion.
///
/// Before T037, B items kept their original Ids.  If A also had `Id(1)` (for Foo),
/// after inserting B's Bar at Id(1) and then Foo at a fresh Id, both would end up in
/// s_index at different Ids — but B's Bar at Id(1) and A's Foo at, say, Id(3).  The
/// real ADR mandated issue was that a B-side type ref pointing to Id(1) could be
/// confused with an A-side type ref pointing to Id(1) if they had the same numeric value.
///
/// After T037, all B items are renumbered via b_id_remap, so Id(1) (B's Bar) becomes
/// a fresh S Id, and Id(1) in A (Foo) also becomes a different fresh S Id.  The two
/// spaces are fully separated in s_index.
#[test]
fn test_t037_b_and_a_overlapping_ids_produce_distinct_s_entries() {
    use super::phase1::phase1_build_s_and_d;

    let crate_name = "my_crate";

    // --- B: `Bar` at Id(1) ---
    let b_bar_id = Id(1);
    let b = simple_crate_with_struct(crate_name, "Bar");
    // b has root at Id(0), Bar at Id(1).

    // --- A: `Foo` at Id(1), action = Add (same numeric Id as B's Bar) ---
    let a = extended_crate_with_struct(crate_name, "Foo", ItemAction::Add);
    // a has root at Id(0), Foo at Id(1) — collides numerically with B's Bar.

    // --- Run Phase 1 ---
    let (s, _d) = phase1_build_s_and_d(a, &b).expect("phase 1 should succeed");

    // --- Find Foo and Bar in S by name ---
    let foo_item = s
        .krate()
        .index
        .values()
        .find(|item| item.name.as_deref() == Some("Foo"))
        .expect("`Foo` must be present in S (A-side Add)");

    let bar_item = s
        .krate()
        .index
        .values()
        .find(|item| item.name.as_deref() == Some("Bar"))
        .expect("`Bar` must be present in S (B-side Reference)");

    let foo_id = foo_item.id;
    let bar_id = bar_item.id;

    // Both must be present at DISTINCT fresh S Ids — neither should still be at Id(1)
    // (the original B-side id) after T037 renumbering.
    assert_ne!(
        foo_id, bar_id,
        "T037: Foo and Bar must occupy distinct S Ids after Phase 1; got same id {foo_id:?}"
    );

    // Neither should be at the original B-side Id (Id(1)) since b_id_remap
    // renumbers all B items to fresh Ids.
    assert_ne!(
        bar_id, b_bar_id,
        "T037: Bar must be renumbered to a fresh S Id; still at original B Id {b_bar_id:?}"
    );

    // Foo must also be at a fresh S Id distinct from the original B-side Id(1).
    // A regression that places A-side Foo at the colliding original B id (b_bar_id)
    // would not be caught by the `foo_id != bar_id` assertion alone (both could be wrong).
    assert_ne!(
        foo_id, b_bar_id,
        "T037: Foo must not be placed at the original B-side Id {b_bar_id:?}; A-side Ids must be \
         renumbered to fresh S Ids independently of B-side renumbering"
    );

    // Sanity: the s_actions for Foo is Add, Bar is Reference.
    assert_eq!(
        s.action_for(&foo_id),
        Some(ItemAction::Add),
        "T037: Foo must have Add action in s_actions"
    );
    assert_eq!(
        s.action_for(&bar_id),
        Some(ItemAction::Reference),
        "T037: Bar must have Reference action in s_actions"
    );
}

/// T037 regression: full evaluate()-level test to ensure no spurious Phase 2
/// signals arise when A-Add and B-Reference items share the same original numeric Id.
///
/// A: `Foo` added (Id(1) in A's index).
/// B: `Bar` reference (Id(1) in B's index).
/// C: `Foo` is present (matches A's Add) and `Bar` is present (unchanged from B).
///
/// Expected: Foo → SIntersectC_Match_Add (Blue), Bar → SIntersectC_Match_Reference (Red if drift
/// or Blue if no change).  The critical check is that there are NO errors or panics and
/// that `Foo` gets the `Add` region (not confused with `Bar`).
#[test]
fn test_t037_no_spurious_signals_with_overlapping_a_b_ids() {
    let crate_name = "my_crate";

    // A: Foo (Add). Uses Id(1) internally, same as B's Bar.
    let a = extended_crate_with_struct(crate_name, "Foo", ItemAction::Add);

    // B: Bar at Id(1).
    let b = simple_crate_with_struct(crate_name, "Bar");

    // C: both Foo and Bar present (like the expected final state).
    let c_root = Id(0);
    let c_foo_id = Id(1);
    let c_bar_id = Id(2);
    let mut c_index = HashMap::new();
    let mut c_paths = HashMap::new();
    c_index.insert(c_root, root_module_item(c_root, crate_name, vec![c_foo_id, c_bar_id]));
    c_index.insert(c_foo_id, struct_item(c_foo_id, "Foo"));
    c_index.insert(c_bar_id, struct_item(c_bar_id, "Bar"));
    c_paths.insert(
        c_foo_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Foo".to_string()],
            kind: ItemKind::Struct,
        },
    );
    c_paths.insert(
        c_bar_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Bar".to_string()],
            kind: ItemKind::Struct,
        },
    );
    let c = Crate {
        root: c_root,
        crate_version: None,
        includes_private: false,
        index: c_index,
        paths: c_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).expect("evaluate should not error");

    // Foo must be found in S as Add, and must match C (SIntersectC_Match_Add → Blue).
    let foo_signal = report.iter().find(|s| s.item_name() == "Foo");
    assert!(
        foo_signal.is_some(),
        "T037: Foo signal must be present in report; all signals: {:?}",
        report.iter().map(|s| s.item_name()).collect::<Vec<_>>()
    );
    assert_eq!(
        foo_signal.unwrap().region(),
        SignalRegion::SIntersectC_Match_Add,
        "T037: Foo must have Add match region (got {:?})",
        foo_signal.unwrap().region()
    );
    assert!(foo_signal.unwrap().signal().is_blue(), "T037: Foo SIntersectC_Match_Add must be Blue");

    // Bar (B-Reference, structurally unchanged in C) must NOT appear in the report.
    // SIntersectC_Match_Reference maps to ThreeWaySignalKind::Skip, which is filtered
    // out by ThreeWayEvaluationReport::new — "maintained" items are suppressed to
    // reduce noise (ADR 3 D3).  A regression that corrupts Bar's structure (producing
    // Mismatch_Reference → Red) or drops Bar from S (CMinusSUnionD → Red) would cause
    // Bar to appear in the report unexpectedly.
    let bar_signal = report.iter().find(|s| s.item_name() == "Bar");
    assert!(
        bar_signal.is_none(),
        "T037: Bar must be Skip (Reference + structural match → filtered from report); \
         unexpected signal: {:?}; all signals: {:?}",
        bar_signal.map(|s| (s.item_name(), s.region())),
        report.iter().map(|s| s.item_name()).collect::<Vec<_>>()
    );
}

// -----------------------------------------------------------------------
// T008 regression: A-side pre-step — build_a_id_remap (IN-10)
// -----------------------------------------------------------------------

/// T008 (a): When A-side Ids and B-side Ids share the same numeric values,
/// `a_id_remap` pre-allocates a fresh S Id for every A item before action
/// processing begins.  After Phase 1, every A-sourced item must reside at a
/// fresh S Id that does NOT collide with any B-sourced item's S Id.
///
/// Fixture:
///   A: `Foo` (Add) at Id(1), impl block at Id(2) whose `for_` points to Id(1).
///   B: `Bar` at Id(1) (same numeric Id as A's Foo), impl block at Id(2).
///
/// Expected: Foo and Bar are at *distinct* fresh S Ids.  The impl block for
/// Foo must have its `for_.id` pointing to the new S Id of Foo (not Bar's Id
/// and not the original A-side Id(1)).
#[test]
#[allow(clippy::panic)]
fn test_t008_a_id_remap_resolves_collisions() {
    use super::phase1::phase1_build_s_and_d;
    use rustdoc_types::{Impl, Path as RdPath};

    let crate_name = "my_crate";
    let root_id = Id(0);

    // --- B: `Bar` at Id(1), impl at Id(2) ---
    let b_bar_id = Id(1);
    let b_impl_id = Id(2);
    let mut b_index = HashMap::new();
    let mut b_paths = HashMap::new();
    b_index.insert(root_id, root_module_item(root_id, crate_name, vec![b_bar_id, b_impl_id]));
    b_index.insert(
        b_bar_id,
        make_item(
            b_bar_id,
            Some("Bar"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
                generics: empty_generics(),
                impls: vec![b_impl_id],
            }),
        ),
    );
    b_index.insert(
        b_impl_id,
        make_item(
            b_impl_id,
            None,
            ItemEnum::Impl(Impl {
                is_unsafe: false,
                generics: empty_generics(),
                provided_trait_methods: vec![],
                trait_: None,
                for_: Type::ResolvedPath(RdPath {
                    path: "Bar".to_string(),
                    id: b_bar_id,
                    args: None,
                }),
                items: vec![],
                is_synthetic: false,
                is_negative: false,
                blanket_impl: None,
            }),
        ),
    );
    b_paths.insert(
        b_bar_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Bar".to_string()],
            kind: ItemKind::Struct,
        },
    );
    let b = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: b_index,
        paths: b_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    // --- A: `Foo` (Add) at Id(1) — same numeric Id as B's Bar.
    //       impl at Id(2) whose for_ points to Id(1) (A's Foo). ---
    let a_foo_id = Id(1); // collides numerically with b_bar_id
    let a_impl_id = Id(2); // collides numerically with b_impl_id
    let mut a_index = HashMap::new();
    let mut a_paths = HashMap::new();
    a_index.insert(root_id, root_module_item(root_id, crate_name, vec![a_foo_id, a_impl_id]));
    a_index.insert(
        a_foo_id,
        make_item(
            a_foo_id,
            Some("Foo"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
                generics: empty_generics(),
                impls: vec![a_impl_id],
            }),
        ),
    );
    a_index.insert(
        a_impl_id,
        make_item(
            a_impl_id,
            None,
            ItemEnum::Impl(Impl {
                is_unsafe: false,
                generics: empty_generics(),
                provided_trait_methods: vec![],
                trait_: None,
                for_: Type::ResolvedPath(RdPath {
                    path: "Foo".to_string(),
                    id: a_foo_id, // references A's Foo at Id(1) — will be remapped
                    args: None,
                }),
                items: vec![],
                is_synthetic: false,
                is_negative: false,
                blanket_impl: None,
            }),
        ),
    );
    a_paths.insert(
        a_foo_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Foo".to_string()],
            kind: ItemKind::Struct,
        },
    );
    let a_krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: a_index,
        paths: a_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };
    let mut a_actions = BTreeMap::new();
    a_actions.insert(a_foo_id, ItemAction::Add);
    let a = ExtendedCrate::new(a_krate, a_actions);

    // --- Run Phase 1 ---
    let (s, _d) = phase1_build_s_and_d(a, &b).expect("phase 1 should succeed");

    // --- Verify: Foo and Bar are at distinct fresh S Ids ---
    let foo_item = s
        .krate()
        .index
        .values()
        .find(|item| item.name.as_deref() == Some("Foo"))
        .expect("Foo must be in S (Add action)");
    let bar_item = s
        .krate()
        .index
        .values()
        .find(|item| item.name.as_deref() == Some("Bar"))
        .expect("Bar must be in S (B-side Reference)");

    let foo_s_id = foo_item.id;
    let bar_s_id = bar_item.id;
    assert_ne!(
        foo_s_id, bar_s_id,
        "T008: Foo and Bar must occupy distinct S Ids; got the same Id {foo_s_id:?}"
    );
    // Both must be at fresh Ids (not the original A/B Id(1)).
    assert_ne!(
        foo_s_id,
        Id(1),
        "T008: Foo's S Id must be a fresh Id (not the original A-side Id(1))"
    );
    assert_ne!(
        bar_s_id,
        Id(1),
        "T008: Bar's S Id must be a fresh Id (not the original B-side Id(1))"
    );

    // --- Verify: the impl block for Foo has for_.id pointing to Foo's S Id ---
    // The impl block associated with Foo (A-sourced) must have its for_.id
    // updated to foo_s_id so that Phase 1.6 / Phase 2 finds a valid parent.
    let foo_impl = s.krate().index.values().find(|item| {
        if let ItemEnum::Impl(impl_) = &item.inner {
            if let Type::ResolvedPath(p) = &impl_.for_ { p.path == "Foo" } else { false }
        } else {
            false
        }
    });
    assert!(foo_impl.is_some(), "T008: an Impl with for_=Foo must be present in S");
    let foo_impl_inner = match &foo_impl.unwrap().inner {
        ItemEnum::Impl(i) => i,
        _ => panic!("expected Impl"),
    };
    let for_id = match &foo_impl_inner.for_ {
        Type::ResolvedPath(p) => p.id,
        _ => panic!("expected ResolvedPath for_"),
    };
    assert_eq!(
        for_id, foo_s_id,
        "T008: Foo's impl block for_.id must point to Foo's fresh S Id {foo_s_id:?}, got {for_id:?}"
    );
}

/// T008 (b): After the A-side pre-step (`a_id_remap`) is built and action processing
/// runs, the id_map for both Add and Modify actions must contain a mapping entry
/// for the parent type's A-side Id.  This confirms that `rewrite_type_ref_ids_in_item`
/// can remap `for_.id` and other local type refs without needing `patch_impl_for_ids`
/// as a fallback.
///
/// Fixture:
///   A: `Foo` (Add) at Id(1) + `Bar` (Modify, also in B) at Id(2).
///   B: `Bar` at Id(1).
///
/// Expected:
///   - Foo (Add): its impl's for_.id resolves to Foo's S Id (a_id_remap applied).
///   - Bar (Modify): its impl's for_.id resolves to Bar's S Id (same as B-seeded Id).
#[test]
#[allow(clippy::panic)]
fn test_t008_a_id_remap_built_after_pre_step_includes_parent_types() {
    use super::phase1::phase1_build_s_and_d;
    use rustdoc_types::{Impl, Path as RdPath};

    let crate_name = "my_crate";
    let root_id = Id(0);

    // --- B: `Bar` at Id(1) ---
    let b_bar_id = Id(1);
    let mut b_index = HashMap::new();
    let mut b_paths = HashMap::new();
    b_index.insert(root_id, root_module_item(root_id, crate_name, vec![b_bar_id]));
    b_index.insert(b_bar_id, struct_item(b_bar_id, "Bar"));
    b_paths.insert(
        b_bar_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Bar".to_string()],
            kind: ItemKind::Struct,
        },
    );
    let b = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: b_index,
        paths: b_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    // --- A: `Foo` (Add) at Id(1), `Bar` (Modify) at Id(2), each with an impl block ---
    let a_foo_id = Id(1);
    let a_foo_impl_id = Id(3);
    let a_bar_id = Id(2);
    let a_bar_impl_id = Id(4);
    let mut a_index = HashMap::new();
    let mut a_paths = HashMap::new();

    a_index.insert(
        root_id,
        root_module_item(
            root_id,
            crate_name,
            vec![a_foo_id, a_bar_id, a_foo_impl_id, a_bar_impl_id],
        ),
    );
    // Foo (Add) with an impl block pointing to a_foo_id.
    a_index.insert(
        a_foo_id,
        make_item(
            a_foo_id,
            Some("Foo"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
                generics: empty_generics(),
                impls: vec![a_foo_impl_id],
            }),
        ),
    );
    a_index.insert(
        a_foo_impl_id,
        make_item(
            a_foo_impl_id,
            None,
            ItemEnum::Impl(Impl {
                is_unsafe: false,
                generics: empty_generics(),
                provided_trait_methods: vec![],
                trait_: None,
                for_: Type::ResolvedPath(RdPath {
                    path: "Foo".to_string(),
                    id: a_foo_id, // references A's Foo — must be remapped via a_id_remap
                    args: None,
                }),
                items: vec![],
                is_synthetic: false,
                is_negative: false,
                blanket_impl: None,
            }),
        ),
    );
    // Bar (Modify) with an impl block pointing to a_bar_id.
    a_index.insert(
        a_bar_id,
        make_item(
            a_bar_id,
            Some("Bar"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
                generics: empty_generics(),
                impls: vec![a_bar_impl_id],
            }),
        ),
    );
    a_index.insert(
        a_bar_impl_id,
        make_item(
            a_bar_impl_id,
            None,
            ItemEnum::Impl(Impl {
                is_unsafe: false,
                generics: empty_generics(),
                provided_trait_methods: vec![],
                trait_: None,
                for_: Type::ResolvedPath(RdPath {
                    path: "Bar".to_string(),
                    id: a_bar_id, // references A's Bar — must be remapped via a_id_remap
                    args: None,
                }),
                items: vec![],
                is_synthetic: false,
                is_negative: false,
                blanket_impl: None,
            }),
        ),
    );
    a_paths.insert(
        a_foo_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Foo".to_string()],
            kind: ItemKind::Struct,
        },
    );
    a_paths.insert(
        a_bar_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Bar".to_string()],
            kind: ItemKind::Struct,
        },
    );
    let a_krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: a_index,
        paths: a_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };
    let mut a_actions = BTreeMap::new();
    a_actions.insert(a_foo_id, ItemAction::Add);
    a_actions.insert(a_bar_id, ItemAction::Modify);
    let a = ExtendedCrate::new(a_krate, a_actions);

    // --- Run Phase 1 ---
    let (s, _d) = phase1_build_s_and_d(a, &b).expect("phase 1 should succeed");

    // --- Verify Foo (Add) impl: for_.id must point to Foo's S Id ---
    let foo_item = s
        .krate()
        .index
        .values()
        .find(|item| item.name.as_deref() == Some("Foo"))
        .expect("Foo must be in S");
    let foo_s_id = foo_item.id;

    let foo_impl = s.krate().index.values().find(|item| {
        if let ItemEnum::Impl(impl_) = &item.inner {
            if let Type::ResolvedPath(p) = &impl_.for_ { p.path == "Foo" } else { false }
        } else {
            false
        }
    });
    assert!(foo_impl.is_some(), "T008(b): impl for Foo must be in S");
    let foo_impl_for_id = match &foo_impl.unwrap().inner {
        ItemEnum::Impl(i) => match &i.for_ {
            Type::ResolvedPath(p) => p.id,
            _ => panic!("expected ResolvedPath"),
        },
        _ => panic!("expected Impl"),
    };
    assert_eq!(
        foo_impl_for_id, foo_s_id,
        "T008(b) Add: impl for Foo must have for_.id = Foo's S Id {foo_s_id:?} (via a_id_remap); \
         got {foo_impl_for_id:?}"
    );

    // --- Verify Bar (Modify) impl: for_.id must point to Bar's S Id ---
    let bar_item = s
        .krate()
        .index
        .values()
        .find(|item| item.name.as_deref() == Some("Bar"))
        .expect("Bar must be in S");
    let bar_s_id = bar_item.id;

    let bar_impl = s.krate().index.values().find(|item| {
        if let ItemEnum::Impl(impl_) = &item.inner {
            if let Type::ResolvedPath(p) = &impl_.for_ { p.path == "Bar" } else { false }
        } else {
            false
        }
    });
    assert!(bar_impl.is_some(), "T008(b): impl for Bar must be in S");
    let bar_impl_for_id = match &bar_impl.unwrap().inner {
        ItemEnum::Impl(i) => match &i.for_ {
            Type::ResolvedPath(p) => p.id,
            _ => panic!("expected ResolvedPath"),
        },
        _ => panic!("expected Impl"),
    };
    assert_eq!(
        bar_impl_for_id, bar_s_id,
        "T008(b) Modify: impl for Bar must have for_.id = Bar's S Id {bar_s_id:?} (via a_id_remap); \
         got {bar_impl_for_id:?}"
    );
}

// -----------------------------------------------------------------------
// T038 regression: A-side TypeAlias orphan-impl pass (Symptom B / IN-31)
// -----------------------------------------------------------------------

/// T038 regression: A-side `TypeAlias` trait-impls that are standalone in
/// `a_krate.index` (not referenced by any `Struct.impls` / `Enum.impls`) must be
/// imported into S by the A-side orphan-impl pass.
///
/// Before T038, only the B-side orphan-impl pass existed; A-side standalone
/// `Impl` items (generated by `encode_type_alias` because `TypeAlias` has no
/// `impls` field) were not reached by the type-processing loop, so Phase 2's
/// `build_impl_identity_map` found them only in C and produced spurious
/// `CMinusSUnionD` (Red) signals.
#[test]
fn test_t038_a_side_orphan_impl_pass_imports_typealias_trait_impl_into_s() {
    use super::phase1::phase1_build_s_and_d;
    use rustdoc_types::{Impl, Path as RdPath, TypeAlias};

    let crate_name = "my_crate";
    let root_id = Id(0);
    let alias_id = Id(1);
    let orphan_impl_id = Id(2);

    // A: TypeAlias `MyAlias` (Add) + standalone Impl for MyAlias not referenced
    // by any Struct/Enum `impls` field (TypeAlias has no `impls` field at all).
    let mut a_index = HashMap::new();
    let mut a_paths = HashMap::new();

    a_index.insert(root_id, root_module_item(root_id, crate_name, vec![alias_id, orphan_impl_id]));
    a_index.insert(
        alias_id,
        make_item(
            alias_id,
            Some("MyAlias"),
            ItemEnum::TypeAlias(TypeAlias {
                // Use Primitive to avoid a DanglingId for the underlying type:
                // ResolvedPath with an unregistered Id would fail Phase 1.6.
                type_: Type::Primitive("u32".to_string()),
                generics: empty_generics(),
            }),
        ),
    );
    // Id used for the external trait `MyTrait` in the orphan impl.  Must be in
    // a_paths with crate_id != 0 so Phase 1.45 remaps it to a fresh S id and
    // Phase 1.6 validates it as "A-side external — valid".
    let my_trait_id = Id(98);
    a_paths.insert(
        alias_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "MyAlias".to_string()],
            kind: ItemKind::TypeAlias,
        },
    );
    a_paths.insert(
        my_trait_id,
        ItemSummary {
            crate_id: 1, // external crate (crate_id != 0 → A-side external)
            path: vec!["my_ext_crate".to_string(), "MyTrait".to_string()],
            kind: ItemKind::Trait,
        },
    );

    // The orphan Impl: `impl MyTrait for MyAlias`.
    let orphan_impl_inner = Impl {
        is_unsafe: false,
        generics: empty_generics(),
        provided_trait_methods: vec![],
        trait_: Some(RdPath { path: "MyTrait".to_string(), id: my_trait_id, args: None }),
        for_: Type::ResolvedPath(RdPath { path: "MyAlias".to_string(), id: alias_id, args: None }),
        items: vec![],
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    };
    a_index
        .insert(orphan_impl_id, make_item(orphan_impl_id, None, ItemEnum::Impl(orphan_impl_inner)));
    // NOTE: no `a_paths` entry for the orphan impl — impls typically have no path entry.

    let mut a_actions = BTreeMap::new();
    a_actions.insert(alias_id, ItemAction::Add);
    // No action entry for orphan_impl_id — the pass inherits action from the
    // parent type (alias_id → Add).

    let a_krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: a_index,
        paths: a_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };
    let a = ExtendedCrate::new(a_krate, a_actions);

    // B: empty baseline.
    let b = empty_crate();

    // Run Phase 1.
    let (s, _d) = phase1_build_s_and_d(a, &b).expect("phase 1 should succeed");

    // Assert: an Impl item with trait "MyTrait" must be in S (imported by the
    // A-side orphan-impl pass).  Before T038, this Impl would be absent from S
    // even though Phase 2 finds it in C, producing spurious CMinusSUnionD.
    let s_impl = s.krate().index.values().find(|item| {
        if let ItemEnum::Impl(impl_) = &item.inner {
            impl_.trait_.as_ref().map(|p| p.path.as_str()) == Some("MyTrait")
        } else {
            false
        }
    });
    assert!(
        s_impl.is_some(),
        "T038: orphan A-side Impl for MyAlias must be imported into S; \
         S Impls: {:?}",
        s.krate()
            .index
            .values()
            .filter_map(|item| {
                if let ItemEnum::Impl(impl_) = &item.inner {
                    impl_.trait_.as_ref().map(|p| p.path.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    );

    // Assert: the imported orphan impl carries the parent TypeAlias's action (Add).
    let s_impl_id = s_impl.expect("checked above").id;
    assert_eq!(
        s.action_for(&s_impl_id),
        Some(ItemAction::Add),
        "T038: orphan Impl must inherit the parent TypeAlias's action (Add)"
    );
}

// -----------------------------------------------------------------------
// T043 regression: generic impl identity normalization
// -----------------------------------------------------------------------

/// T043 (a): `impl<S> TaskOperationInteractor<S>: TaskOperationService` in C
/// must produce the same identity key `"TaskOperationInteractor: TaskOperationService"`
/// as the catalogue A-codec key (which uses the bare type name without impl-block
/// type parameters).
///
/// This test directly checks `build_impl_identity_map` key output.
/// Before T043, the key was `"TaskOperationInteractor<S>: TaskOperationService"`,
/// which did not match the A-side key `"TaskOperationInteractor: TaskOperationService"`,
/// causing a spurious `CMinusSUnionD` Red signal.
#[test]
fn test_t043_generic_impl_matches_catalogue_key_without_type_params() {
    use super::build_impl_identity_map;
    use rustdoc_types::{GenericArg, GenericArgs, GenericParamDef, Impl, Path as RdPath};

    let crate_name = "my_crate";
    let root_id = Id(0);
    let struct_id = Id(1);
    let impl_id = Id(2);
    // trait_id NOT in krate.paths → string-based fallback via normalize_impl_trait_path.
    // This models the C-side case where rustdoc emits a local-crate trait without
    // a fully-qualified path entry.
    let trait_id = Id(9999);

    let mut index = HashMap::new();
    let mut paths = HashMap::new();

    index.insert(root_id, root_module_item(root_id, crate_name, vec![struct_id, impl_id]));
    index.insert(struct_id, struct_item(struct_id, "TaskOperationInteractor"));
    paths.insert(
        struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "TaskOperationInteractor".to_string()],
            kind: ItemKind::Struct,
        },
    );
    // trait_id is a local trait (crate_id == 0) in krate.paths so the normaliser
    // strips to the last segment "TaskOperationService".
    paths.insert(
        trait_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "TaskOperationService".to_string()],
            kind: ItemKind::Trait,
        },
    );

    // C impl: `generics.params = [S: type param]`, `for_ = TaskOperationInteractor<S>`.
    let type_param_s = GenericParamDef {
        name: "S".to_string(),
        kind: rustdoc_types::GenericParamDefKind::Type {
            bounds: vec![],
            default: None,
            is_synthetic: false,
        },
    };
    let c_impl = Impl {
        is_unsafe: false,
        generics: rustdoc_types::Generics { params: vec![type_param_s], where_predicates: vec![] },
        provided_trait_methods: vec![],
        trait_: Some(RdPath { path: "TaskOperationService".to_string(), id: trait_id, args: None }),
        // `for_` is `TaskOperationInteractor<S>` — the generic arg is the impl-block type param.
        for_: Type::ResolvedPath(RdPath {
            path: "TaskOperationInteractor".to_string(),
            id: struct_id,
            args: Some(Box::new(GenericArgs::AngleBracketed {
                args: vec![GenericArg::Type(Type::Generic("S".to_string()))],
                constraints: vec![],
            })),
        }),
        items: vec![],
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    };
    index.insert(impl_id, make_item(impl_id, None, ItemEnum::Impl(c_impl)));

    let krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    let map = build_impl_identity_map(&krate, crate_name);

    // After T043, the impl-block type param `S` is stripped from `for_`, producing:
    //   "TaskOperationInteractor: TaskOperationService"
    // NOT the pre-T043 form:
    //   "TaskOperationInteractor<S>: TaskOperationService"
    let expected_key = "TaskOperationInteractor: TaskOperationService";
    assert!(
        map.contains_key(expected_key),
        "T043(a): impl<S> TaskOperationInteractor<S>: TaskOperationService must produce key \
         '{expected_key}' after type-param stripping; all keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
    // Verify the old (pre-T043) key is absent.
    let old_key = "TaskOperationInteractor<S>: TaskOperationService";
    assert!(
        !map.contains_key(old_key),
        "T043(a): old key with type param '{old_key}' must NOT be present after T043 fix; \
         all keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
}

/// T043 (b): Non-generic struct trait impls (`impl SymlinkGuardPort for Foo`)
/// must continue to produce the same identity key as before (no stripping occurs
/// when the impl block has no type params).
///
/// This is a regression guard: the T043 type-param stripping must not disturb
/// impls that have no impl-block generics (the `type_params.is_empty()` fast path).
#[test]
fn test_t043_non_generic_impl_still_matches_blue() {
    use super::build_impl_identity_map;
    use rustdoc_types::{Impl, Path as RdPath};

    let crate_name = "my_crate";
    let root_id = Id(0);
    let struct_id = Id(1);
    let impl_id = Id(2);
    let trait_id = Id(9998);

    let mut index = HashMap::new();
    let mut paths = HashMap::new();

    index.insert(root_id, root_module_item(root_id, crate_name, vec![struct_id, impl_id]));
    index.insert(struct_id, struct_item(struct_id, "Foo"));
    paths.insert(
        struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Foo".to_string()],
            kind: ItemKind::Struct,
        },
    );
    // Local trait (crate_id == 0) → key uses bare short name "SymlinkGuardPort".
    paths.insert(
        trait_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "SymlinkGuardPort".to_string()],
            kind: ItemKind::Trait,
        },
    );

    // Non-generic impl (no type params on the impl block).
    let c_impl = Impl {
        is_unsafe: false,
        generics: empty_generics(),
        provided_trait_methods: vec![],
        trait_: Some(RdPath { path: "SymlinkGuardPort".to_string(), id: trait_id, args: None }),
        for_: Type::ResolvedPath(RdPath { path: "Foo".to_string(), id: struct_id, args: None }),
        items: vec![],
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    };
    index.insert(impl_id, make_item(impl_id, None, ItemEnum::Impl(c_impl)));

    let krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    let map = build_impl_identity_map(&krate, crate_name);

    // Non-generic impl must produce the expected key unchanged.
    let expected_key = "Foo: SymlinkGuardPort";
    assert!(
        map.contains_key(expected_key),
        "T043(b): non-generic impl must produce key '{expected_key}' unchanged; \
         all keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
}

/// T043 (c): concrete type args on `for_` are preserved — `impl Foo<Vec<u32>>: Bar`
/// must NOT be normalized (the key retains `<Vec<u32>>`).
///
/// The stripping should only remove args whose type is `Type::Generic` with a name
/// that belongs to the impl-block's type-param set.  Concrete args like `Vec<u32>`
/// are structurally part of the identity and must be preserved.
#[test]
fn test_t043_concrete_type_args_preserved_in_identity_key() {
    use super::build_impl_identity_map;
    use rustdoc_types::{GenericArg, GenericArgs, Impl, Path as RdPath};

    let crate_name = "my_crate";
    let root_id = Id(0);
    let struct_id = Id(1);
    let impl_id = Id(2);
    let trait_id = Id(9999);

    let mut index = HashMap::new();
    let mut paths = HashMap::new();

    index.insert(root_id, root_module_item(root_id, crate_name, vec![struct_id, impl_id]));
    index.insert(struct_id, struct_item(struct_id, "Foo"));
    paths.insert(
        struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Foo".to_string()],
            kind: ItemKind::Struct,
        },
    );
    // Trait is external (crate_id != 0) so the key uses its qualified path.
    paths.insert(
        trait_id,
        ItemSummary {
            crate_id: 1,
            path: vec!["ext_crate".to_string(), "Bar".to_string()],
            kind: ItemKind::Trait,
        },
    );

    // `impl Foo<Vec<u32>>: Bar` — `Foo` has a CONCRETE arg `Vec<u32>`, not a type param.
    // The impl block has NO type params.
    let c_impl = Impl {
        is_unsafe: false,
        generics: empty_generics(), // no type params on the impl block
        provided_trait_methods: vec![],
        trait_: Some(RdPath { path: "Bar".to_string(), id: trait_id, args: None }),
        for_: Type::ResolvedPath(RdPath {
            path: "Foo".to_string(),
            id: struct_id,
            args: Some(Box::new(GenericArgs::AngleBracketed {
                args: vec![GenericArg::Type(Type::ResolvedPath(RdPath {
                    path: "Vec".to_string(),
                    id: Id(100),
                    args: Some(Box::new(GenericArgs::AngleBracketed {
                        args: vec![GenericArg::Type(Type::Primitive("u32".to_string()))],
                        constraints: vec![],
                    })),
                }))],
                constraints: vec![],
            })),
        }),
        items: vec![],
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    };
    index.insert(impl_id, make_item(impl_id, None, ItemEnum::Impl(c_impl)));

    // Use a dummy external_crates entry so the single-segment expansion logic works.
    use rustdoc_types::ExternalCrate;
    let mut external_crates = HashMap::new();
    external_crates.insert(
        1u32,
        ExternalCrate {
            name: "ext_crate".to_string(),
            html_root_url: None,
            path: std::path::PathBuf::new(),
        },
    );

    let krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates,
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    let map = build_impl_identity_map(&krate, crate_name);
    // The key must include the concrete arg `Vec<u32>` — stripping must not occur
    // because `Vec<u32>` is NOT a type parameter of the impl block.
    let key_with_vec = map.keys().find(|k| k.contains("Vec<u32>") || k.contains("Vec"));
    assert!(
        key_with_vec.is_some(),
        "T043(c): concrete arg Vec<u32> on for_ must be preserved in the identity key; \
         all keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
}

/// T043 (d): lifetime params on `impl<'a> Foo<'a>: Bar` must be stripped from
/// the `for_` key, producing `"Foo: Bar"` not `"Foo<'a>: Bar"`.
///
/// The catalogue A-codec never emits lifetime args in `for_` because lifetimes
/// are not part of the structural identity at the catalogue level.  Stripping
/// lifetime generic args ensures C-side and A-side keys are consistent.
#[test]
fn test_t043_lifetime_params_stripped_from_identity_key() {
    use super::build_impl_identity_map;
    use rustdoc_types::{
        GenericArg, GenericArgs, GenericParamDef, GenericParamDefKind, Impl, Path as RdPath,
    };

    let crate_name = "my_crate";
    let root_id = Id(0);
    let struct_id = Id(1);
    let impl_id = Id(2);
    let trait_id = Id(9999);

    let mut index = HashMap::new();
    let mut paths = HashMap::new();

    index.insert(root_id, root_module_item(root_id, crate_name, vec![struct_id, impl_id]));
    index.insert(struct_id, struct_item(struct_id, "Foo"));
    paths.insert(
        struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Foo".to_string()],
            kind: ItemKind::Struct,
        },
    );
    // Trait is local (crate_id == 0): key uses the bare short name.
    paths.insert(
        trait_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Bar".to_string()],
            kind: ItemKind::Trait,
        },
    );

    // `impl<'a> Foo<'a>: Bar` — lifetime param `'a` on the impl block.
    let lifetime_param = GenericParamDef {
        name: "a".to_string(),
        kind: GenericParamDefKind::Lifetime { outlives: vec![] },
    };
    let c_impl = Impl {
        is_unsafe: false,
        generics: rustdoc_types::Generics {
            params: vec![lifetime_param],
            where_predicates: vec![],
        },
        provided_trait_methods: vec![],
        trait_: Some(RdPath { path: "Bar".to_string(), id: trait_id, args: None }),
        // `for_` is `Foo<'a>` — the lifetime arg is the impl-block lifetime param.
        for_: Type::ResolvedPath(RdPath {
            path: "Foo".to_string(),
            id: struct_id,
            args: Some(Box::new(GenericArgs::AngleBracketed {
                args: vec![GenericArg::Lifetime("'a".to_string())],
                constraints: vec![],
            })),
        }),
        items: vec![],
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    };
    index.insert(impl_id, make_item(impl_id, None, ItemEnum::Impl(c_impl)));

    let krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    let map = build_impl_identity_map(&krate, crate_name);
    // After stripping the lifetime arg, the key must be `"Foo: Bar"` (no `<'a>`).
    let expected_key = "Foo: Bar";
    assert!(
        map.contains_key(expected_key),
        "T043(d): lifetime param 'a must be stripped from for_ key; \
         expected key '{expected_key}'; all keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
    // The key with `<'a>` must NOT exist.
    let key_with_lifetime = map.keys().find(|k| k.contains("'a"));
    assert!(
        key_with_lifetime.is_none(),
        "T043(d): key must not contain the lifetime param; found: {:?}",
        key_with_lifetime
    );
}

/// T043 (e): impl-block type params inside `Fn(S)` / `fn(S)` positions must be stripped.
///
/// `format_type_strip_type_params` must handle `Type::Generic` values that appear
/// directly as inputs of `GenericArgs::Parenthesized` (callable trait args, e.g.
/// `Fn(S) -> S`) and as parameters of `Type::FunctionPointer` (e.g. `fn(S) -> S`).
/// Before this fix the `Type::Generic` arm returned `name.clone()` unconditionally,
/// leaving impl-block params in the rendered string and causing spurious C-vs-A
/// mismatches in `build_impl_identity_map`.
#[test]
fn test_t043_generic_in_parenthesized_and_fn_pointer_positions_stripped() {
    use std::collections::BTreeSet;

    use rustdoc_types::{
        FunctionHeader, FunctionPointer, FunctionSignature, GenericArgs, Path as RdPath, Type,
    };

    use super::format::format_type_strip_type_params;

    let mut type_params = BTreeSet::new();
    type_params.insert("S".to_string());

    // --- Parenthesized: `Fn(S) -> S` when S is an impl-block type param ---
    // rustdoc represents `Fn(S) -> S` as
    //   Type::ResolvedPath { path: "Fn", args: GenericArgs::Parenthesized { inputs: [S], output: Some(S) } }
    let fn_trait_type = Type::ResolvedPath(RdPath {
        path: "Fn".to_string(),
        id: rustdoc_types::Id(9998),
        args: Some(Box::new(GenericArgs::Parenthesized {
            inputs: vec![Type::Generic("S".to_string())],
            output: Some(Type::Generic("S".to_string())),
        })),
    });
    let rendered_fn = format_type_strip_type_params(&fn_trait_type, &type_params);
    // After stripping: `S` must not appear; inputs should be empty → `Fn()`
    // and return type also stripped.
    assert!(
        !rendered_fn.contains('S'),
        "T043(e): impl-block param 'S' must be stripped from Fn(S)->S parenthesized args; \
         got: {rendered_fn:?}"
    );

    // --- FunctionPointer: `fn(S) -> S` when S is an impl-block type param ---
    let fn_ptr_type = Type::FunctionPointer(Box::new(FunctionPointer {
        sig: FunctionSignature {
            inputs: vec![("_".to_string(), Type::Generic("S".to_string()))],
            output: Some(Type::Generic("S".to_string())),
            is_c_variadic: false,
        },
        header: FunctionHeader {
            is_async: false,
            is_const: false,
            is_unsafe: false,
            abi: rustdoc_types::Abi::Rust,
        },
        generic_params: vec![],
    }));
    let rendered_fp = format_type_strip_type_params(&fn_ptr_type, &type_params);
    // After stripping: `S` must not appear; fn pointer becomes `fn()->()`.
    assert!(
        !rendered_fp.contains('S'),
        "T043(e): impl-block param 'S' must be stripped from fn(S)->S function pointer; \
         got: {rendered_fp:?}"
    );

    // --- Negative: concrete type `u32` must never be stripped ---
    let fn_ptr_concrete = Type::FunctionPointer(Box::new(FunctionPointer {
        sig: FunctionSignature {
            inputs: vec![("_".to_string(), Type::Primitive("u32".to_string()))],
            output: Some(Type::Primitive("u64".to_string())),
            is_c_variadic: false,
        },
        header: FunctionHeader {
            is_async: false,
            is_const: false,
            is_unsafe: false,
            abi: rustdoc_types::Abi::Rust,
        },
        generic_params: vec![],
    }));
    let rendered_concrete = format_type_strip_type_params(&fn_ptr_concrete, &type_params);
    assert!(
        rendered_concrete.contains("u32") && rendered_concrete.contains("u64"),
        "T043(e): concrete params u32/u64 must NOT be stripped; got: {rendered_concrete:?}"
    );
}

// -----------------------------------------------------------------------
// T012 regression: cross-crate impl (for_ is external type) included in identity map
// -----------------------------------------------------------------------

/// Helper: build a `rustdoc_types::Crate` that contains a local type `local_type_name` and an
/// `impl local_trait_name for ExternalType` block, where `ExternalType` lives in an external crate
/// (`crate_id != 0`).
///
/// `external_crate_name` is registered in `krate.external_crates` (external_crate_id = 2).
/// The external type's `crate_id` in `krate.index` is set to 2 so that
/// `build_impl_identity_map`'s loop skips the type item itself (crate_id != 0) but still
/// processes the Impl item (the impl item itself always has crate_id == 0 because the impl
/// was declared in THIS crate).
///
/// This fixture models `impl LocalTrait for ExternalType` — a "cross-crate target" impl where
/// the impl is defined in the local crate but the implementing type (`for_`) is external.
fn crate_with_cross_crate_target_impl(
    crate_name: &str,
    local_trait_name: &str,
    external_type_name: &str,
    external_crate_name: &str,
) -> Crate {
    use rustdoc_types::{ExternalCrate, Impl, Path as RdPath};

    let root_id = Id(0);
    let local_trait_id = Id(1);
    let impl_id = Id(2);
    // The external type's item is in the index but with crate_id != 0.
    let external_type_id = Id(3);
    let external_type_crate_id: u32 = 2;

    let mut index = HashMap::new();
    let mut paths = HashMap::new();
    let mut external_crates = HashMap::new();

    // Root module references the local trait and the impl.
    index.insert(root_id, root_module_item(root_id, crate_name, vec![local_trait_id, impl_id]));

    // Local trait item (crate_id == 0).
    index.insert(
        local_trait_id,
        make_item(
            local_trait_id,
            Some(local_trait_name),
            ItemEnum::Trait(rustdoc_types::Trait {
                is_auto: false,
                is_unsafe: false,
                is_dyn_compatible: true,
                items: vec![],
                generics: empty_generics(),
                bounds: vec![],
                implementations: vec![],
            }),
        ),
    );
    paths.insert(
        local_trait_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), local_trait_name.to_string()],
            kind: ItemKind::Trait,
        },
    );

    // External type entry in the index with crate_id != 0 — so the type item is skipped by the
    // `crate_id != 0` loop guard, but the Impl item (which has crate_id == 0) is still processed.
    let mut external_type_item = make_item(
        external_type_id,
        Some(external_type_name),
        ItemEnum::Struct(Struct {
            kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
            generics: empty_generics(),
            impls: vec![],
        }),
    );
    external_type_item.crate_id = external_type_crate_id; // mark as external
    index.insert(external_type_id, external_type_item);
    // The external type appears in krate.paths with crate_id != 0.
    paths.insert(
        external_type_id,
        ItemSummary {
            crate_id: external_type_crate_id,
            path: vec![external_crate_name.to_string(), external_type_name.to_string()],
            kind: ItemKind::Struct,
        },
    );

    // Register the external crate.
    external_crates.insert(
        external_type_crate_id,
        ExternalCrate {
            name: external_crate_name.to_string(),
            html_root_url: None,
            path: std::path::PathBuf::new(),
        },
    );

    // The impl `impl LocalTrait for ExternalType` — the Impl item itself has crate_id == 0
    // (it was declared in THIS crate).
    let cross_crate_impl = Impl {
        is_unsafe: false,
        generics: empty_generics(),
        provided_trait_methods: vec![],
        trait_: Some(RdPath {
            path: local_trait_name.to_string(),
            id: local_trait_id, // local trait — in krate.paths with crate_id == 0
            args: None,
        }),
        for_: Type::ResolvedPath(RdPath {
            path: external_type_name.to_string(),
            id: external_type_id, // external type
            args: None,
        }),
        items: vec![],
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    };
    index.insert(impl_id, make_item(impl_id, None, ItemEnum::Impl(cross_crate_impl)));

    Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates,
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    }
}

/// T012 (new behavior, ADR D4): `impl LocalTrait for ExternalType` where `ExternalType` is an
/// external type (`for_` has `crate_id != 0` in `krate.paths`) must be included in
/// `build_impl_identity_map`.
///
/// Before T012, a `for_is_external` guard filtered these out.  After T012, the guard is removed
/// per ADR D4 (catalogue-schema-permissive): both C-side and S-side (B-side orphan-impl pass)
/// include cross-crate impls symmetrically so that fingerprints match and no spurious
/// `CMinusSUnionD` signal is generated.
///
/// This test validates the **new behavior** (inclusion).
#[test]
fn test_t012_cross_crate_target_impl_included_in_identity_map() {
    use super::build_impl_identity_map;

    let crate_name = "my_crate";
    let krate = crate_with_cross_crate_target_impl(
        crate_name,
        "MyLocalTrait",
        "ExternalType",
        "external_crate",
    );

    let map = build_impl_identity_map(&krate, crate_name);

    // The identity key is formed as "{for_name}: {trait_str}".
    // `for_name` = `format_type(&impl_.for_)` = last path segment of `ExternalType` = "ExternalType".
    // `trait_str` = normalized local trait = "MyLocalTrait" (local trait → stripped to short name).
    let expected_key = "ExternalType: MyLocalTrait";
    assert!(
        map.contains_key(expected_key),
        "T012: cross-crate target impl `impl MyLocalTrait for ExternalType` must be included \
         in build_impl_identity_map after D4 filter removal; \
         all keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
}

/// T012 (local-crate target regression): `impl LocalTrait for LocalType` (both `for_` and `trait_`
/// are local, `crate_id == 0`) must continue to be included in `build_impl_identity_map`.
///
/// This is a regression guard: the D4 filter removal must not break the existing behavior
/// for ordinary same-crate impls.
#[test]
fn test_t012_local_crate_target_impl_still_included_in_identity_map() {
    use super::build_impl_identity_map;
    use rustdoc_types::{Impl, Path as RdPath};

    let crate_name = "my_crate";
    let root_id = Id(0);
    let struct_id = Id(1);
    let trait_id = Id(2);
    let impl_id = Id(3);

    let mut index = HashMap::new();
    let mut paths = HashMap::new();

    index
        .insert(root_id, root_module_item(root_id, crate_name, vec![struct_id, trait_id, impl_id]));
    index.insert(struct_id, struct_item(struct_id, "LocalType"));
    index.insert(
        trait_id,
        make_item(
            trait_id,
            Some("LocalTrait"),
            ItemEnum::Trait(rustdoc_types::Trait {
                is_auto: false,
                is_unsafe: false,
                is_dyn_compatible: true,
                items: vec![],
                generics: empty_generics(),
                bounds: vec![],
                implementations: vec![],
            }),
        ),
    );
    paths.insert(
        struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "LocalType".to_string()],
            kind: ItemKind::Struct,
        },
    );
    paths.insert(
        trait_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "LocalTrait".to_string()],
            kind: ItemKind::Trait,
        },
    );

    // `impl LocalTrait for LocalType` — both sides are local.
    let local_impl = Impl {
        is_unsafe: false,
        generics: empty_generics(),
        provided_trait_methods: vec![],
        trait_: Some(RdPath { path: "LocalTrait".to_string(), id: trait_id, args: None }),
        for_: Type::ResolvedPath(RdPath {
            path: "LocalType".to_string(),
            id: struct_id,
            args: None,
        }),
        items: vec![],
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    };
    index.insert(impl_id, make_item(impl_id, None, ItemEnum::Impl(local_impl)));

    let krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    let map = build_impl_identity_map(&krate, crate_name);

    // Both `for_` (local → short name "LocalType") and `trait_` (local → short name "LocalTrait")
    // produce the key "LocalType: LocalTrait".
    let expected_key = "LocalType: LocalTrait";
    assert!(
        map.contains_key(expected_key),
        "T012 regression: local-crate impl `impl LocalTrait for LocalType` must still be included \
         in build_impl_identity_map; all keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
}

/// T012 (excluded by inherent/negative/synthetic/blanket filter): cross-crate target impls that
/// are inherent (no trait), negative, synthetic, or blanket must still be excluded —
/// the D4 change only removed the `for_is_external` guard, not the other guards.
///
/// Validates that the non-`for_`-based exclusion guards are intact after T012.
#[test]
fn test_t012_excluded_impl_variants_still_excluded() {
    use super::build_impl_identity_map;
    use rustdoc_types::{Impl, Path as RdPath};

    let crate_name = "my_crate";
    let root_id = Id(0);
    let struct_id = Id(1);
    let trait_id = Id(2);
    // external_type for `for_` — crate_id != 0.
    let external_type_id = Id(3);
    let external_type_crate_id: u32 = 2;

    // --- Inherent impl with external `for_` (no trait_ → excluded) ---
    {
        let mut index = HashMap::new();
        let mut paths = HashMap::new();
        let mut external_crates = HashMap::new();

        index.insert(root_id, root_module_item(root_id, crate_name, vec![struct_id]));
        index.insert(struct_id, struct_item(struct_id, "LocalHelper"));
        paths.insert(
            struct_id,
            ItemSummary {
                crate_id: 0,
                path: vec![crate_name.to_string(), "LocalHelper".to_string()],
                kind: ItemKind::Struct,
            },
        );

        let mut ext_item = make_item(
            external_type_id,
            Some("ExtType"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
                generics: empty_generics(),
                impls: vec![],
            }),
        );
        ext_item.crate_id = external_type_crate_id;
        index.insert(external_type_id, ext_item);
        paths.insert(
            external_type_id,
            ItemSummary {
                crate_id: external_type_crate_id,
                path: vec!["ext".to_string(), "ExtType".to_string()],
                kind: ItemKind::Struct,
            },
        );
        external_crates.insert(
            external_type_crate_id,
            rustdoc_types::ExternalCrate {
                name: "ext".to_string(),
                html_root_url: None,
                path: std::path::PathBuf::new(),
            },
        );

        // Inherent impl (no trait_) — must be excluded regardless of D4.
        let inherent_impl_id = Id(10);
        let inherent_impl = Impl {
            is_unsafe: false,
            generics: empty_generics(),
            provided_trait_methods: vec![],
            trait_: None, // inherent impl — excluded
            for_: Type::ResolvedPath(RdPath {
                path: "ExtType".to_string(),
                id: external_type_id,
                args: None,
            }),
            items: vec![],
            is_synthetic: false,
            is_negative: false,
            blanket_impl: None,
        };
        index.insert(
            inherent_impl_id,
            make_item(inherent_impl_id, None, ItemEnum::Impl(inherent_impl)),
        );

        let krate = Crate {
            root: root_id,
            crate_version: None,
            includes_private: false,
            index,
            paths,
            external_crates,
            format_version: FORMAT_VERSION,
            target: Target { triple: String::new(), target_features: vec![] },
        };
        let map = build_impl_identity_map(&krate, crate_name);
        assert!(
            map.is_empty(),
            "T012: inherent impl (no trait) must be excluded; keys: {:?}",
            map.keys().collect::<Vec<_>>()
        );
    }

    // --- Negative impl with external `for_` — must be excluded ---
    {
        let mut index = HashMap::new();
        let mut paths = HashMap::new();
        let mut external_crates = HashMap::new();

        index.insert(root_id, root_module_item(root_id, crate_name, vec![trait_id]));
        index.insert(
            trait_id,
            make_item(
                trait_id,
                Some("LocalTrait"),
                ItemEnum::Trait(rustdoc_types::Trait {
                    is_auto: false,
                    is_unsafe: false,
                    is_dyn_compatible: true,
                    items: vec![],
                    generics: empty_generics(),
                    bounds: vec![],
                    implementations: vec![],
                }),
            ),
        );
        paths.insert(
            trait_id,
            ItemSummary {
                crate_id: 0,
                path: vec![crate_name.to_string(), "LocalTrait".to_string()],
                kind: ItemKind::Trait,
            },
        );

        let mut ext_item = make_item(
            external_type_id,
            Some("ExtType"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
                generics: empty_generics(),
                impls: vec![],
            }),
        );
        ext_item.crate_id = external_type_crate_id;
        index.insert(external_type_id, ext_item);
        paths.insert(
            external_type_id,
            ItemSummary {
                crate_id: external_type_crate_id,
                path: vec!["ext".to_string(), "ExtType".to_string()],
                kind: ItemKind::Struct,
            },
        );
        external_crates.insert(
            external_type_crate_id,
            rustdoc_types::ExternalCrate {
                name: "ext".to_string(),
                html_root_url: None,
                path: std::path::PathBuf::new(),
            },
        );

        let neg_impl_id = Id(11);
        let neg_impl = Impl {
            is_unsafe: false,
            generics: empty_generics(),
            provided_trait_methods: vec![],
            trait_: Some(RdPath { path: "LocalTrait".to_string(), id: trait_id, args: None }),
            for_: Type::ResolvedPath(RdPath {
                path: "ExtType".to_string(),
                id: external_type_id,
                args: None,
            }),
            items: vec![],
            is_synthetic: false,
            is_negative: true, // negative impl — excluded
            blanket_impl: None,
        };
        index.insert(neg_impl_id, make_item(neg_impl_id, None, ItemEnum::Impl(neg_impl)));

        let krate = Crate {
            root: root_id,
            crate_version: None,
            includes_private: false,
            index,
            paths,
            external_crates,
            format_version: FORMAT_VERSION,
            target: Target { triple: String::new(), target_features: vec![] },
        };
        let map = build_impl_identity_map(&krate, crate_name);
        assert!(
            map.is_empty(),
            "T012: negative impl must be excluded; keys: {:?}",
            map.keys().collect::<Vec<_>>()
        );
    }
}

/// T012 (collision tiebreaker): when two impls produce the same short-name key because two
/// different `for_` path strings share the same last segment (e.g., `"MyError"` and
/// `"ext_crate::MyError"` both produce short name `"MyError"`), `build_impl_identity_map`
/// must resolve the collision *deterministically using the raw `for_` path string* — not by
/// raw `Id` value.
///
/// The raw `for_` path string (`Type::ResolvedPath.path`) is preserved identically in
/// B-origin orphan impls and C-side rustdoc output, making the tiebreaker consistent across
/// both sides and preventing a spurious structural mismatch in Phase 2.
///
/// The test is structured so that the `for_path_raw` tiebreaker is decisive: the impl whose
/// `for_.path` sorts lexicographically *earlier* (`"MyError"` < `"ext_crate::MyError"`) has a
/// **larger** Id than its competitor.  An id-only sort would pick the wrong impl.
#[test]
fn test_t012_collision_tiebreaker_uses_for_path_not_id() {
    use rustdoc_types::{ExternalCrate, Impl, Path as RdPath};

    use super::build_impl_identity_map;

    let crate_name = "my_crate";
    let root_id = Id(0);
    let trait_id = Id(1);
    // Local struct whose impl.for_.path will be the SHORT form "MyError".
    let local_error_id = Id(2);
    // External struct whose impl.for_.path will be the QUALIFIED form "ext_crate::MyError".
    // format_type() takes the last segment of both → both produce the short name "MyError"
    // → same identity key → collision.
    let ext_error_id = Id(3);
    let ext_crate_id: u32 = 2;

    // Id assignment: the impl whose for_path_raw sorts earlier ("MyError") gets the LARGER id
    // (10).  An id-only sort would incorrectly keep the impl with id 4 (ext impl,
    // for_path_raw = "ext_crate::MyError").  The for_path_raw tiebreaker corrects this.
    let impl_short_path_id = Id(10); // large id — for_path_raw = "MyError" (sorts first)
    let impl_qualified_path_id = Id(4); // small id — for_path_raw = "ext_crate::MyError" (sorts second)

    let mut index = HashMap::new();
    let mut paths = HashMap::new();
    let mut external_crates = HashMap::new();

    index.insert(root_id, root_module_item(root_id, crate_name, vec![trait_id, local_error_id]));

    // Trait (local).
    index.insert(
        trait_id,
        make_item(
            trait_id,
            Some("MyTrait"),
            ItemEnum::Trait(rustdoc_types::Trait {
                is_auto: false,
                is_unsafe: false,
                is_dyn_compatible: true,
                items: vec![],
                generics: empty_generics(),
                bounds: vec![],
                implementations: vec![],
            }),
        ),
    );
    paths.insert(
        trait_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "MyTrait".to_string()],
            kind: ItemKind::Trait,
        },
    );

    // Local struct. impl.for_.path will use the short form "MyError".
    index.insert(local_error_id, struct_item(local_error_id, "MyError"));
    paths.insert(
        local_error_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "MyError".to_string()],
            kind: ItemKind::Struct,
        },
    );

    // External struct. impl.for_.path will use the qualified form "ext_crate::MyError"
    // (rustdoc sometimes emits module-qualified paths for external types in impl.for_).
    // format_type() strips to "MyError" → same key → collision.
    let mut ext_item = make_item(
        ext_error_id,
        Some("MyError"),
        ItemEnum::Struct(Struct {
            kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
            generics: empty_generics(),
            impls: vec![],
        }),
    );
    ext_item.crate_id = ext_crate_id;
    index.insert(ext_error_id, ext_item);
    paths.insert(
        ext_error_id,
        ItemSummary {
            crate_id: ext_crate_id,
            path: vec!["ext_crate".to_string(), "MyError".to_string()],
            kind: ItemKind::Struct,
        },
    );
    external_crates.insert(
        ext_crate_id,
        ExternalCrate {
            name: "ext_crate".to_string(),
            html_root_url: None,
            path: std::path::PathBuf::new(),
        },
    );

    // impl MyTrait for local MyError — for_.path = "MyError" (short form).
    // Assigned impl_short_path_id = Id(10) — the LARGER id.
    let short_path_impl = Impl {
        is_unsafe: false,
        generics: empty_generics(),
        provided_trait_methods: vec![],
        trait_: Some(RdPath { path: "MyTrait".to_string(), id: trait_id, args: None }),
        for_: Type::ResolvedPath(RdPath {
            path: "MyError".to_string(), // short form — for_path_raw = "MyError"
            id: local_error_id,
            args: None,
        }),
        items: vec![],
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    };
    index.insert(
        impl_short_path_id,
        make_item(impl_short_path_id, None, ItemEnum::Impl(short_path_impl)),
    );

    // impl MyTrait for ext_crate::MyError — for_.path = "ext_crate::MyError" (qualified form,
    // as rustdoc might emit for external types).  Assigned impl_qualified_path_id = Id(4) — the
    // SMALLER id.  An id-only sort would keep THIS impl; the for_path_raw sort must override it.
    let qualified_path_impl = Impl {
        is_unsafe: false,
        generics: empty_generics(),
        provided_trait_methods: vec![],
        trait_: Some(RdPath { path: "MyTrait".to_string(), id: trait_id, args: None }),
        for_: Type::ResolvedPath(RdPath {
            path: "ext_crate::MyError".to_string(), // qualified form — for_path_raw = "ext_crate::MyError"
            id: ext_error_id,
            args: None,
        }),
        items: vec![],
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    };
    index.insert(
        impl_qualified_path_id,
        make_item(impl_qualified_path_id, None, ItemEnum::Impl(qualified_path_impl)),
    );

    let krate = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index,
        paths,
        external_crates,
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    let map = build_impl_identity_map(&krate, crate_name);

    // Both impls produce the same short-name key "MyError: MyTrait" (collision).
    // The map must contain exactly one entry.
    assert_eq!(
        map.len(),
        1,
        "T012 collision tiebreaker: exactly one impl must survive; all keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );
    assert!(
        map.contains_key("MyError: MyTrait"),
        "T012 collision tiebreaker: the surviving key must be \"MyError: MyTrait\"; \
         all keys: {:?}",
        map.keys().collect::<Vec<_>>()
    );

    // The for_path_raw tiebreaker must pick the impl with the lexicographically smaller
    // path string regardless of Id ordering:
    //   "MyError" < "ext_crate::MyError"  → impl_short_path_id (Id 10) wins
    // Without the tiebreaker, impl_qualified_path_id (Id 4, the SMALLER id) would win instead.
    let surviving_id = map["MyError: MyTrait"];
    assert_eq!(
        surviving_id, impl_short_path_id,
        "T012 collision tiebreaker: the impl with the lexicographically smaller for_path_raw \
         must win, regardless of Id ordering. Expected impl_short_path_id={:?} \
         (for_path_raw=\"MyError\"), got {:?}. \
         An id-only sort would have kept impl_qualified_path_id={:?} (Id 4, smaller).",
        impl_short_path_id, surviving_id, impl_qualified_path_id
    );
}

/// T043 (f): HRTB binders on `FunctionPointer` must be preserved (not collapsed to
/// `<UNSUPPORTED:FunctionPointer>`).
///
/// `format_type_strip_type_params` previously returned `<UNSUPPORTED:FunctionPointer>`
/// for any `FunctionPointer` with non-empty `generic_params`, which regressed HRTB
/// function-pointer types such as `for<'a> fn(&'a S)` into an unusable sentinel key.
/// After the fix the binder is rendered identically to `format_type` while the
/// sig's arguments are still stripped via `strip()`.
#[test]
fn test_t043_hrtb_function_pointer_binders_preserved() {
    use std::collections::BTreeSet;

    use rustdoc_types::{
        FunctionHeader, FunctionPointer, FunctionSignature, GenericParamDef, GenericParamDefKind,
        Type,
    };

    use super::format::format_type_strip_type_params;

    let mut type_params = BTreeSet::new();
    type_params.insert("S".to_string());

    // Build `for<'a> fn(&'a S)` — `'a` is a HRTB binder on the fn type,
    // `S` is an impl-block type param that must be stripped.
    let hrtb_fn_ptr = Type::FunctionPointer(Box::new(FunctionPointer {
        sig: FunctionSignature {
            inputs: vec![(
                "_".to_string(),
                Type::BorrowedRef {
                    lifetime: Some("'a".to_string()),
                    is_mutable: false,
                    type_: Box::new(Type::Generic("S".to_string())),
                },
            )],
            output: None,
            is_c_variadic: false,
        },
        header: FunctionHeader {
            is_async: false,
            is_const: false,
            is_unsafe: false,
            abi: rustdoc_types::Abi::Rust,
        },
        generic_params: vec![GenericParamDef {
            name: "a".to_string(),
            kind: GenericParamDefKind::Lifetime { outlives: vec![] },
        }],
    }));

    let rendered = format_type_strip_type_params(&hrtb_fn_ptr, &type_params);

    // Must NOT regress to the old sentinel.
    assert_ne!(
        rendered, "<UNSUPPORTED:FunctionPointer>",
        "T043(f): HRTB function pointer must not collapse to unsupported sentinel; got: {rendered:?}"
    );
    // The HRTB lifetime binder must be present in the output.
    assert!(
        rendered.contains("'a"),
        "T043(f): HRTB binder 'a must be preserved in the rendered key; got: {rendered:?}"
    );
    // The impl-block type param 'S' must be stripped.
    assert!(
        !rendered.contains('S'),
        "T043(f): impl-block param 'S' must be stripped inside the HRTB fn pointer; \
         got: {rendered:?}"
    );
}

// -----------------------------------------------------------------------
// T007: impl-block-level / trait-decl-level generics symmetric comparison
// (IN-09, AC-08)
// -----------------------------------------------------------------------

/// T007 (a): When both A-side (catalogue) and C-side (rustdoc) carry the same
/// impl-block-level generics on a trait impl, the signal must be Blue (Match).
///
/// Uses the CatalogueToExtendedCrateCodec to build the A side so the test
/// verifies the full A-codec → evaluator pipeline.
#[test]
fn test_impl_block_generics_symmetric_compare_blue() {
    use domain::tddd::catalogue_v2::composite::TypeKindV2;
    use domain::tddd::catalogue_v2::entries::TypeEntry;
    use domain::tddd::catalogue_v2::methods::MethodGenericParam;
    use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole};
    use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
    use domain::tddd::catalogue_v2::{
        CatalogueDocument, CrateName, ModulePath, ParamName, TraitName, TypeName, TypeRef,
    };
    use domain::tddd::{CatalogueToExtendedCratePort, LayerId};
    use rustdoc_types::{
        GenericBound, GenericParamDef, GenericParamDefKind, Impl, Path, TraitBoundModifier,
        WherePredicate,
    };

    use crate::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec;

    let crate_name = "my_crate";
    let mut doc = CatalogueDocument::new(
        2,
        CrateName::new(crate_name).unwrap(),
        LayerId::try_new("domain").expect("static"),
    );

    // A-side: `impl<T: Clone> MyTrait for Foo` — impl_generics: [T: Clone]
    let mut trait_impl = TraitImplDeclV2::new(
        TraitName::new("MyTrait").unwrap(),
        CrateName::new(crate_name).unwrap(),
    );
    trait_impl.impl_generics = vec![MethodGenericParam {
        name: ParamName::new("T").unwrap(),
        bounds: vec![TypeRef::new("Clone").unwrap()],
    }];

    doc.types.insert(
        TypeName::new("Foo").unwrap(),
        TypeEntry {
            action: domain::tddd::catalogue_v2::ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![trait_impl],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );
    // Also register the local trait "MyTrait" (needed for local trait id resolution).
    use domain::tddd::catalogue_v2::entries::TraitEntry;
    doc.traits.insert(
        TraitName::new("MyTrait").unwrap(),
        TraitEntry {
            action: domain::tddd::catalogue_v2::ItemAction::Add,
            role: ContractRole::SpecificationPort,
            methods: vec![],
            supertrait_bounds: vec![],
            generics: vec![],
            where_predicates: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let a = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();

    // Verify A-side actually encoded impl_generics: the codec must emit a trait Impl item
    // with non-empty `generics.params` — a Blue signal alone cannot prove this because the
    // evaluator's `items_structurally_equal` treats an empty A-side as "not encoded yet"
    // and skips the comparison rather than diffing. Without this check the test would pass
    // even if the codec silently emitted `empty_generics()`.
    {
        let a_krate = a.krate();
        // Use filter_map to both locate and downcast the trait Impl item in one pass,
        // avoiding `let-else { panic! }` which triggers clippy::panic in non-test scopes.
        let a_impl_inner = a_krate
            .index
            .values()
            .filter_map(|item| {
                if let ItemEnum::Impl(i) = &item.inner {
                    if i.trait_.is_some() { Some(i) } else { None }
                } else {
                    None
                }
            })
            .next()
            .expect("A-side must contain a trait Impl item for `impl<T: Clone> MyTrait for Foo`");
        assert!(
            !a_impl_inner.generics.params.is_empty(),
            "A-side Impl must have non-empty generics.params (impl_generics encoded from \
             TraitImplDeclV2.impl_generics:[T:Clone]), got: {:?}",
            a_impl_inner.generics.params
        );
        assert_eq!(
            a_impl_inner.generics.params[0].name, "T",
            "A-side Impl first generic param must be 'T'"
        );
    }

    // B = empty (Add action, B has nothing).
    let b = empty_crate();

    // C-side: crate with "Foo" (Struct) + "MyTrait" (Trait) + impl<T: Clone> MyTrait for Foo.
    let root_id = Id(0);
    let foo_id = Id(1);
    let my_trait_id = Id(2);
    let impl_id = Id(3);

    let mut c_index = HashMap::new();
    let mut c_paths = HashMap::new();

    c_index.insert(root_id, root_module_item(root_id, crate_name, vec![foo_id, my_trait_id]));

    // Struct "Foo"
    c_index.insert(
        foo_id,
        make_item(
            foo_id,
            Some("Foo"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
                generics: Generics { params: vec![], where_predicates: vec![] },
                impls: vec![impl_id],
            }),
        ),
    );
    c_paths.insert(
        foo_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Foo".to_string()],
            kind: ItemKind::Struct,
        },
    );

    // Trait "MyTrait"
    c_index.insert(
        my_trait_id,
        make_item(
            my_trait_id,
            Some("MyTrait"),
            ItemEnum::Trait(rustdoc_types::Trait {
                is_auto: false,
                is_unsafe: false,
                is_dyn_compatible: true,
                items: vec![],
                generics: Generics { params: vec![], where_predicates: vec![] },
                bounds: vec![],
                implementations: vec![],
            }),
        ),
    );
    c_paths.insert(
        my_trait_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "MyTrait".to_string()],
            kind: ItemKind::Trait,
        },
    );

    // `impl<T: Clone> MyTrait for Foo`
    // C-side rustdoc: generics.params = [T (type param)], where_predicates = [T: Clone]
    let t_param = GenericParamDef {
        name: "T".to_string(),
        kind: GenericParamDefKind::Type { bounds: vec![], default: None, is_synthetic: false },
    };
    let clone_bound = GenericBound::TraitBound {
        trait_: Path { path: "Clone".to_string(), id: Id(999), args: None },
        generic_params: vec![],
        modifier: TraitBoundModifier::None,
    };
    let t_where = WherePredicate::BoundPredicate {
        type_: Type::Generic("T".to_string()),
        bounds: vec![clone_bound],
        generic_params: vec![],
    };
    c_index.insert(
        impl_id,
        make_item(
            impl_id,
            None,
            ItemEnum::Impl(Impl {
                is_unsafe: false,
                generics: Generics { params: vec![t_param], where_predicates: vec![t_where] },
                provided_trait_methods: vec![],
                trait_: Some(Path { path: "MyTrait".to_string(), id: my_trait_id, args: None }),
                for_: Type::ResolvedPath(Path { path: "Foo".to_string(), id: foo_id, args: None }),
                items: vec![],
                is_negative: false,
                is_synthetic: false,
                blanket_impl: None,
            }),
        ),
    );

    let c = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: c_index,
        paths: c_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    // The "Foo: MyTrait" impl signal must be Blue: both A-side and C-side have the same
    // generics shape (impl<T: Clone>), so the evaluator must report Match (Blue), not Mismatch
    // (Yellow) and not CMinusSUnionD (Red).
    let impl_signal =
        report.iter().find(|s| s.item_name().contains("Foo") && s.item_name().contains("MyTrait"));
    assert!(
        impl_signal.is_some(),
        "must find a signal for Foo: MyTrait impl, got signals: {:?}",
        report.iter().map(|s| s.item_name()).collect::<Vec<_>>()
    );
    assert!(
        impl_signal.unwrap().signal().is_blue(),
        "Foo: MyTrait signal must be Blue when both sides declare the same impl_generics; \
         got region={:?}",
        impl_signal.unwrap().region()
    );
}

/// T007 (b): When A-side declares `impl_generics: []` (empty, old catalogue) and
/// C-side has `impl<T: Clone> MyTrait for Foo`, the existing behaviour is preserved:
/// the signal must not regress (backward compat).
///
/// With the current design, impl-block generics from empty A-side are not compared
/// so the existing Blue evaluation is retained.
#[test]
fn test_existing_catalogue_no_change_in_signal_for_trait_impl_no_generics() {
    use domain::tddd::catalogue_v2::composite::TypeKindV2;
    use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
    use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole};
    use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
    use domain::tddd::catalogue_v2::{
        CatalogueDocument, CrateName, ModulePath, TraitName, TypeName,
    };
    use domain::tddd::{CatalogueToExtendedCratePort, LayerId};
    use rustdoc_types::{
        GenericBound, GenericParamDef, GenericParamDefKind, Impl, Path, TraitBoundModifier,
        WherePredicate,
    };

    use crate::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec;

    let crate_name = "my_crate";
    let mut doc = CatalogueDocument::new(
        2,
        CrateName::new(crate_name).unwrap(),
        LayerId::try_new("domain").expect("static"),
    );

    // A-side: `impl MyTrait for Foo` — impl_generics: [] (old catalogue, no impl generics).
    let trait_impl = TraitImplDeclV2::new(
        TraitName::new("MyTrait").unwrap(),
        CrateName::new(crate_name).unwrap(),
    );

    doc.types.insert(
        TypeName::new("Foo").unwrap(),
        TypeEntry {
            action: domain::tddd::catalogue_v2::ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::PlainStruct {
                fields: vec![],
                has_stripped_fields: false,
                typestate: None,
            },
            methods: vec![],
            trait_impls: vec![trait_impl],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );
    doc.traits.insert(
        TraitName::new("MyTrait").unwrap(),
        TraitEntry {
            action: domain::tddd::catalogue_v2::ItemAction::Add,
            role: ContractRole::SpecificationPort,
            methods: vec![],
            supertrait_bounds: vec![],
            generics: vec![],
            where_predicates: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let a = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();

    // Verify A-side emits empty generics for old catalogue (impl_generics: []).
    // Without this check the test would pass even if the codec started emitting non-empty
    // generics for old catalogues (breaking backward compatibility).
    {
        let a_krate = a.krate();
        let a_impl_inner = a_krate
            .index
            .values()
            .filter_map(|item| {
                if let ItemEnum::Impl(i) = &item.inner {
                    if i.trait_.is_some() { Some(i) } else { None }
                } else {
                    None
                }
            })
            .next()
            .expect("A-side must contain a trait Impl item for `impl MyTrait for Foo`");
        assert!(
            a_impl_inner.generics.params.is_empty(),
            "A-side Impl with impl_generics:[] must have empty generics.params (backward \
             compat: old catalogues must not produce impl-level generics), got: {:?}",
            a_impl_inner.generics.params
        );
        assert!(
            a_impl_inner.generics.where_predicates.is_empty(),
            "A-side Impl with impl_generics:[] must have empty where_predicates, got: {:?}",
            a_impl_inner.generics.where_predicates
        );
    }

    let b = empty_crate();

    // C-side: `impl<T: Clone> MyTrait for Foo` (C has impl generics but A declares none).
    let root_id = Id(0);
    let foo_id = Id(1);
    let my_trait_id = Id(2);
    let impl_id = Id(3);

    let mut c_index = HashMap::new();
    let mut c_paths = HashMap::new();

    c_index.insert(root_id, root_module_item(root_id, crate_name, vec![foo_id, my_trait_id]));
    c_index.insert(
        foo_id,
        make_item(
            foo_id,
            Some("Foo"),
            ItemEnum::Struct(Struct {
                kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
                generics: Generics { params: vec![], where_predicates: vec![] },
                impls: vec![impl_id],
            }),
        ),
    );
    c_paths.insert(
        foo_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Foo".to_string()],
            kind: ItemKind::Struct,
        },
    );
    c_index.insert(
        my_trait_id,
        make_item(
            my_trait_id,
            Some("MyTrait"),
            ItemEnum::Trait(rustdoc_types::Trait {
                is_auto: false,
                is_unsafe: false,
                is_dyn_compatible: true,
                items: vec![],
                generics: Generics { params: vec![], where_predicates: vec![] },
                bounds: vec![],
                implementations: vec![],
            }),
        ),
    );
    c_paths.insert(
        my_trait_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "MyTrait".to_string()],
            kind: ItemKind::Trait,
        },
    );
    // C-side impl with generics
    let t_param = GenericParamDef {
        name: "T".to_string(),
        kind: GenericParamDefKind::Type { bounds: vec![], default: None, is_synthetic: false },
    };
    let clone_bound = GenericBound::TraitBound {
        trait_: Path { path: "Clone".to_string(), id: Id(999), args: None },
        generic_params: vec![],
        modifier: TraitBoundModifier::None,
    };
    let t_where = WherePredicate::BoundPredicate {
        type_: Type::Generic("T".to_string()),
        bounds: vec![clone_bound],
        generic_params: vec![],
    };
    c_index.insert(
        impl_id,
        make_item(
            impl_id,
            None,
            ItemEnum::Impl(Impl {
                is_unsafe: false,
                generics: Generics { params: vec![t_param], where_predicates: vec![t_where] },
                provided_trait_methods: vec![],
                trait_: Some(Path { path: "MyTrait".to_string(), id: my_trait_id, args: None }),
                for_: Type::ResolvedPath(Path { path: "Foo".to_string(), id: foo_id, args: None }),
                items: vec![],
                is_negative: false,
                is_synthetic: false,
                blanket_impl: None,
            }),
        ),
    );
    let c = Crate {
        root: root_id,
        crate_version: None,
        includes_private: false,
        index: c_index,
        paths: c_paths,
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    // When A-side declares no impl_generics (empty Vec = old catalogue) and C-side has
    // impl<T: Clone>, the identity keys still match ("Foo: MyTrait"), so the signal must
    // be Blue (Match_Add) — backward compat: old catalogues must not regress to Yellow or Red.
    let impl_signal =
        report.iter().find(|s| s.item_name().contains("Foo") && s.item_name().contains("MyTrait"));
    assert!(impl_signal.is_some(), "must find a signal for Foo: MyTrait impl");
    assert!(
        impl_signal.unwrap().signal().is_blue(),
        "old catalogue (impl_generics: []) must produce Blue when C has impl<T> generics; \
         identity keys must still match; got region={:?}",
        impl_signal.unwrap().region()
    );
}
