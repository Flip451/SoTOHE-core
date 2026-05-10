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
// Derive filter tests (Step C: is_derive_trait + build_impl_identity_map)
// -----------------------------------------------------------------------

/// Helper: build a crate with a struct that has an auto-derived impl (e.g. Clone).
fn crate_with_derive_impl(crate_name: &str, struct_name: &str, trait_name: &str) -> Crate {
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

    // Derive-generated impl item (is_synthetic = false in older rustdoc versions,
    // but trait name is a known derive trait).
    let derive_impl = Impl {
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
        is_synthetic: false, // Not marked as synthetic in older rustdoc versions.
        is_negative: false,
        blanket_impl: None,
    };
    index.insert(impl_id, make_item(impl_id, None, ItemEnum::Impl(derive_impl)));

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

#[test]
fn test_derive_impls_do_not_trigger_c_minus_s_union_d_signal() {
    // C has "MyStruct" with a `Clone` derive impl (is_synthetic = false).
    // A and B are empty — so normally Clone impl would appear in CMinusSUnionD (Red).
    // With the derive filter, it must be excluded from the impl identity map.
    let a = ExtendedCrate::new(empty_crate(), BTreeMap::new());
    let b = empty_crate();
    let c = crate_with_derive_impl("my_crate", "MyStruct", "Clone");

    let evaluator = SignalEvaluatorV2::new();
    let report = evaluator.evaluate(a, b, c).unwrap();

    // "MyStruct" itself appears in CMinusSUnionD (it's an undeclared type), but
    // the "MyStruct: Clone" impl key must NOT appear in the report.
    let impl_signal = report.iter().find(|s| s.item_name().contains(": Clone"));
    assert!(
        impl_signal.is_none(),
        "Clone derive impl must not produce a CMinusSUnionD signal, got: {:?}",
        impl_signal.map(|s| s.item_name())
    );
}

#[test]
fn test_is_derive_trait_returns_true_for_derive_only_traits() {
    use crate::tddd::signal_evaluator_v2::is_derive_trait;

    // Standard library / popular-crate derive-macro traits that are
    // EXCLUSIVELY generated by proc-macros in this codebase.
    // NOTE: `Send` and `Sync` are intentionally excluded from this list.
    // Auto-generated Send/Sync impls are already filtered by `is_synthetic = true`
    // in rustdoc; explicit `unsafe impl Send/Sync` carries a safety contract and
    // must remain catalogue-visible.
    //
    // NOTE: `Error` is also excluded — the filter applies to ALL identity maps (S, D,
    // and C), so filtering Error would drop declared `trait_impls: Error` from the
    // S-side identity set, making declared Error-contract violations undetectable.
    // Only traits that are provably never hand-written are in the filter.
    // Hand-writable traits (Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
    // Deserialize) were removed so that explicit catalogue declarations of these
    // traits are not silently dropped from the S-side identity set.
    let derive_traits = [
        "Clone",
        "Copy",
        "Debug",
        // Compiler-internal phantom marker traits: never hand-written.
        "StructuralPartialEq",
        "StructuralEq",
        // IntoStaticStr: exclusively generated by strum::IntoStaticStr derive.
        // The From<&T>-for-&str side-effects it generates are filtered separately.
        "IntoStaticStr",
    ];
    for name in &derive_traits {
        assert!(is_derive_trait(name), "Expected is_derive_trait({name}) == true");
    }
}

#[test]
fn test_is_derive_trait_returns_false_for_catalogue_relevant_traits() {
    use crate::tddd::signal_evaluator_v2::is_derive_trait;

    // These traits can be hand-written and are catalogue-relevant; they must NOT be filtered.
    // Note: From is kept catalogue-relevant even though strum::IntoStaticStr generates
    // `&str: From<T>` — those are suppressed in build_impl_identity_map via a `for_name`
    // check rather than by filtering the trait name here.
    //
    // Display, FromStr, TryFrom, and AsRef are also catalogue-relevant: this codebase
    // has numerous hand-written impls of these traits on non-enum types.  Even though
    // strum/thiserror can derive them, excluding them globally would hide real API-contract
    // changes.
    //
    // Send and Sync are catalogue-relevant: explicit `unsafe impl Send/Sync` blocks
    // carry safety contracts.  Auto-generated Send/Sync impls are already filtered by
    // `impl_.is_synthetic = true` in rustdoc, so there is no noise-filter need here.
    //
    // Error is catalogue-relevant: filtering Error globally would drop declared
    // `trait_impls: Error` from the S-side identity set, making declared Error-contract
    // violations undetectable.  Undeclared thiserror-generated impls will appear in
    // CMinusSUnionD — the correct behavior (a missing catalogue declaration).
    let catalogue_relevant = [
        "From",
        "Into",
        "TryInto",
        "Display",
        "FromStr",
        "TryFrom",
        "Error",
        "AsRef",
        "Send",
        "Sync",
        "TrackReader",
        "TrackWriter",
        "CatalogueLoader",
        "SchemaExporter",
        // Hand-writable comparison/hash traits: removed from DERIVE_TRAIT_NAMES
        // so that explicit catalogue declarations are not silently dropped.
        "PartialEq",
        "Eq",
        "Hash",
        "Ord",
        "PartialOrd",
        // Hand-writable serde traits: custom impls are common in domain types.
        "Serialize",
        "Deserialize",
        "DeserializeOwned",
        // Default is hand-writable (non-trivial defaults, invalid zero values, etc.)
        // and must not be silently filtered from the identity map.
        "Default",
    ];
    for name in &catalogue_relevant {
        assert!(!is_derive_trait(name), "Expected is_derive_trait({name}) == false");
    }
}

#[test]
fn test_is_derive_trait_matches_qualified_external_paths() {
    use crate::tddd::signal_evaluator_v2::is_derive_trait;

    // External traits preserved verbatim by normalize_impl_trait_path must still match.
    assert!(
        is_derive_trait("std::fmt::Debug"),
        "std::fmt::Debug must match via last-segment check"
    );
    // serde::Serialize is now catalogue-relevant (hand-writable for custom serde impls)
    // and must NOT be filtered.
    assert!(
        !is_derive_trait("serde::Serialize"),
        "serde::Serialize must not match — hand-written custom serde impls are catalogue-relevant"
    );
    // From is not a derive-only trait — even qualified it must not match.
    assert!(
        !is_derive_trait("std::convert::From"),
        "std::convert::From must not match (catalogue-relevant)"
    );
    // Display / TryFrom are also catalogue-relevant (hand-writable) and must NOT match.
    assert!(
        !is_derive_trait("std::fmt::Display"),
        "std::fmt::Display must not match (catalogue-relevant — hand-written impls exist)"
    );
    assert!(
        !is_derive_trait("std::convert::TryFrom"),
        "std::convert::TryFrom must not match (catalogue-relevant — hand-written impls exist)"
    );
}

#[test]
fn test_is_derive_trait_strips_generic_args() {
    use crate::tddd::signal_evaluator_v2::is_derive_trait;

    // PartialOrd<Self> must NOT be filtered — PartialOrd is hand-writable (removed from
    // DERIVE_TRAIT_NAMES because custom comparison logic is a valid API contract).
    assert!(
        !is_derive_trait("PartialOrd<Self>"),
        "PartialOrd<Self> must not match — PartialOrd is catalogue-relevant (hand-writable)"
    );
    // TryFrom is catalogue-relevant (hand-writable) — must NOT match even with generic args.
    assert!(
        !is_derive_trait("TryFrom<&str>"),
        "TryFrom<&str> must not match — TryFrom is catalogue-relevant (hand-writable)"
    );
    // From<u64> is NOT a derive-only trait (commonly hand-written).
    assert!(!is_derive_trait("From<u64>"), "From<u64> must not match — From is catalogue-relevant");
    // Qualified path with generics.
    assert!(
        is_derive_trait("std::fmt::Debug<Self>"),
        "std::fmt::Debug<Self> must match via last-segment + generic strip"
    );
}

#[test]
fn test_multiple_derive_impls_all_filtered_from_identity_map() {
    // C has "Order" with Clone, Debug, and PartialEq derive impls.
    // None of these should appear in the impl identity map or report.
    let root_id = Id(0);
    let struct_id = Id(1);
    let clone_impl_id = Id(2);
    let debug_impl_id = Id(3);
    let partial_eq_impl_id = Id(4);
    let crate_name = "my_crate";

    use rustdoc_types::{Impl, Path as RdPath};
    let make_derive_impl = |id: Id, trait_name: &str| {
        let impl_inner = Impl {
            is_unsafe: false,
            generics: empty_generics(),
            provided_trait_methods: vec![],
            trait_: Some(RdPath { path: trait_name.to_string(), id: Id(9999), args: None }),
            for_: rustdoc_types::Type::ResolvedPath(RdPath {
                path: "Order".to_string(),
                id: struct_id,
                args: None,
            }),
            items: vec![],
            is_synthetic: false,
            is_negative: false,
            blanket_impl: None,
        };
        make_item(id, None, ItemEnum::Impl(impl_inner))
    };

    let mut index = HashMap::new();
    let mut paths = HashMap::new();
    index.insert(
        root_id,
        root_module_item(
            root_id,
            crate_name,
            vec![struct_id, clone_impl_id, debug_impl_id, partial_eq_impl_id],
        ),
    );
    index.insert(struct_id, struct_item(struct_id, "Order"));
    paths.insert(
        struct_id,
        ItemSummary {
            crate_id: 0,
            path: vec![crate_name.to_string(), "Order".to_string()],
            kind: ItemKind::Struct,
        },
    );
    index.insert(clone_impl_id, make_derive_impl(clone_impl_id, "Clone"));
    index.insert(debug_impl_id, make_derive_impl(debug_impl_id, "Debug"));
    index.insert(partial_eq_impl_id, make_derive_impl(partial_eq_impl_id, "PartialEq"));

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

    // Clone and Debug are in DERIVE_TRAIT_NAMES and must not appear in the report.
    // PartialEq is now catalogue-relevant (removed from DERIVE_TRAIT_NAMES) and will
    // appear as a CMinusSUnionD signal because it is in C but not in S or D.
    for signal in report.iter() {
        let name = signal.item_name();
        assert!(
            !name.contains(": Clone") && !name.contains(": Debug"),
            "Clone/Debug derive impls must not appear in report, got: {name}"
        );
    }
    // PartialEq impl should now appear as CMinusSUnionD (not filtered).
    let has_partial_eq = report.iter().any(|s| s.item_name().contains(": PartialEq"));
    assert!(
        has_partial_eq,
        "PartialEq impl must appear in report (catalogue-relevant, not filtered)"
    );
}
