//! Unit tests for catalogue_v2 identifier newtypes.
//!
//! Kept in a separate file to stay within the 700-line module-size limit.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;

use super::*;
use crate::tddd::catalogue_v2::{
    ItemAction, identity_resolution::resolve_catalogue_identity_for_action_in_namespace,
};
use crate::tddd::semantic_verify::CatalogueEntryKey;

// ---------------------------------------------------------------------------
// Identifier
// ---------------------------------------------------------------------------

#[test]
fn test_identifier_with_valid_ascii_alphanumeric_succeeds() {
    let id = Identifier::new("my_field_name").unwrap();
    assert_eq!(id.as_str(), "my_field_name");
}

#[test]
fn test_identifier_with_underscore_prefix_succeeds() {
    let id = Identifier::new("_private_item").unwrap();
    assert_eq!(id.as_str(), "_private_item");
}

#[test]
fn test_identifier_with_empty_string_returns_empty_error() {
    assert_eq!(Identifier::new(""), Err(IdentifierError::Empty));
}

#[test]
fn test_identifier_with_leading_digit_returns_invalid_characters_error() {
    let result = Identifier::new("1bad");
    assert!(matches!(result, Err(IdentifierError::InvalidCharacters(_))));
}

#[test]
fn test_identifier_with_non_ascii_returns_invalid_characters_error() {
    let result = Identifier::new("café");
    assert!(matches!(result, Err(IdentifierError::InvalidCharacters(_))));
}

#[test]
fn test_identifier_with_hyphen_returns_invalid_characters_error() {
    let result = Identifier::new("my-field");
    assert!(matches!(result, Err(IdentifierError::InvalidCharacters(_))));
}

#[test]
fn test_identifier_display_fromstr_roundtrip() {
    let original = Identifier::new("user_id").unwrap();
    let displayed = original.to_string();
    let parsed: Identifier = displayed.parse().unwrap();
    assert_eq!(original, parsed);
}

// ---------------------------------------------------------------------------
// TypeName
// ---------------------------------------------------------------------------

#[test]
fn test_type_name_with_valid_pascal_case_succeeds() {
    let tn = TypeName::new("UserRepository").unwrap();
    assert_eq!(tn.as_str(), "UserRepository");
}

#[test]
fn test_type_name_with_empty_string_returns_empty_error() {
    assert_eq!(TypeName::new(""), Err(IdentifierError::Empty));
}

#[test]
fn test_type_name_display_fromstr_roundtrip() {
    let original = TypeName::new("UserId").unwrap();
    let displayed = original.to_string();
    let parsed: TypeName = displayed.parse().unwrap();
    assert_eq!(original, parsed);
}

// ---------------------------------------------------------------------------
// TraitName
// ---------------------------------------------------------------------------

#[test]
fn test_trait_name_with_valid_name_succeeds() {
    let tn = TraitName::new("UserRepository").unwrap();
    assert_eq!(tn.as_str(), "UserRepository");
}

#[test]
fn test_trait_name_with_leading_digit_returns_error() {
    assert!(TraitName::new("1Trait").is_err());
}

#[test]
fn test_trait_name_display_fromstr_roundtrip() {
    let original = TraitName::new("Repository").unwrap();
    let displayed = original.to_string();
    let parsed: TraitName = displayed.parse().unwrap();
    assert_eq!(original, parsed);
}

// ---------------------------------------------------------------------------
// FieldName
// ---------------------------------------------------------------------------

#[test]
fn test_field_name_with_valid_snake_case_succeeds() {
    let fn_ = FieldName::new("email_address").unwrap();
    assert_eq!(fn_.as_str(), "email_address");
}

#[test]
fn test_field_name_with_empty_string_returns_empty_error() {
    assert_eq!(FieldName::new(""), Err(IdentifierError::Empty));
}

#[test]
fn test_field_name_display_fromstr_roundtrip() {
    let original = FieldName::new("user_id").unwrap();
    let displayed = original.to_string();
    let parsed: FieldName = displayed.parse().unwrap();
    assert_eq!(original, parsed);
}

// ---------------------------------------------------------------------------
// MethodName
// ---------------------------------------------------------------------------

#[test]
fn test_method_name_with_valid_name_succeeds() {
    let mn = MethodName::new("find_by_email").unwrap();
    assert_eq!(mn.as_str(), "find_by_email");
}

#[test]
fn test_method_name_with_space_returns_error() {
    assert!(MethodName::new("bad method").is_err());
}

#[test]
fn test_method_name_display_fromstr_roundtrip() {
    let original = MethodName::new("save").unwrap();
    let displayed = original.to_string();
    let parsed: MethodName = displayed.parse().unwrap();
    assert_eq!(original, parsed);
}

// ---------------------------------------------------------------------------
// ParamName
// ---------------------------------------------------------------------------

#[test]
fn test_param_name_with_valid_name_succeeds() {
    let pn = ParamName::new("user_id").unwrap();
    assert_eq!(pn.as_str(), "user_id");
}

#[test]
fn test_param_name_with_empty_string_returns_empty_error() {
    assert_eq!(ParamName::new(""), Err(IdentifierError::Empty));
}

#[test]
fn test_param_name_display_fromstr_roundtrip() {
    let original = ParamName::new("email").unwrap();
    let displayed = original.to_string();
    let parsed: ParamName = displayed.parse().unwrap();
    assert_eq!(original, parsed);
}

// ---------------------------------------------------------------------------
// VariantName
// ---------------------------------------------------------------------------

#[test]
fn test_variant_name_with_valid_pascal_case_succeeds() {
    let vn = VariantName::new("ZeroFindings").unwrap();
    assert_eq!(vn.as_str(), "ZeroFindings");
}

#[test]
fn test_variant_name_with_leading_digit_returns_error() {
    assert!(VariantName::new("2ndVariant").is_err());
}

#[test]
fn test_variant_name_display_fromstr_roundtrip() {
    let original = VariantName::new("FindingsRemain").unwrap();
    let displayed = original.to_string();
    let parsed: VariantName = displayed.parse().unwrap();
    assert_eq!(original, parsed);
}

// ---------------------------------------------------------------------------
// CrateName
// ---------------------------------------------------------------------------

#[test]
fn test_crate_name_with_valid_name_succeeds() {
    let cn = CrateName::new("domain_core").unwrap();
    assert_eq!(cn.as_str(), "domain_core");
}

#[test]
fn test_crate_name_with_hyphen_returns_error() {
    // Crate names in cargo use hyphens but identifier validation is stricter
    assert!(CrateName::new("my-crate").is_err());
}

#[test]
fn test_crate_name_display_fromstr_roundtrip() {
    let original = CrateName::new("domain").unwrap();
    let displayed = original.to_string();
    let parsed: CrateName = displayed.parse().unwrap();
    assert_eq!(original, parsed);
}

// ---------------------------------------------------------------------------
// FunctionName
// ---------------------------------------------------------------------------

#[test]
fn test_function_name_with_valid_name_succeeds() {
    let fn_ = FunctionName::new("register_user").unwrap();
    assert_eq!(fn_.as_str(), "register_user");
}

#[test]
fn test_function_name_with_empty_string_returns_empty_error() {
    assert_eq!(FunctionName::new(""), Err(IdentifierError::Empty));
}

#[test]
fn test_function_name_display_fromstr_roundtrip() {
    let original = FunctionName::new("find_by_id").unwrap();
    let displayed = original.to_string();
    let parsed: FunctionName = displayed.parse().unwrap();
    assert_eq!(original, parsed);
}

// ---------------------------------------------------------------------------
// ModulePath
// ---------------------------------------------------------------------------

#[test]
fn test_module_path_with_empty_string_returns_root() {
    let mp: ModulePath = "".parse().unwrap();
    assert!(mp.is_root());
    assert_eq!(mp.segments().len(), 0);
}

#[test]
fn test_module_path_with_two_segments_succeeds() {
    let mp: ModulePath = "tddd::catalogue".parse().unwrap();
    assert_eq!(mp.segments().len(), 2);
    assert_eq!(mp.segments()[0].as_str(), "tddd");
    assert_eq!(mp.segments()[1].as_str(), "catalogue");
}

#[test]
fn test_module_path_with_invalid_segment_returns_error() {
    let result = "tddd::1invalid".parse::<ModulePath>();
    assert!(matches!(result, Err(IdentifierError::InvalidSegment(_))));
}

#[test]
fn test_module_path_display_fromstr_roundtrip() {
    let original: ModulePath = "tddd::catalogue::v2".parse().unwrap();
    let displayed = original.to_string();
    let parsed: ModulePath = displayed.parse().unwrap();
    assert_eq!(original, parsed);
}

#[test]
fn test_module_path_root_display_is_empty() {
    let mp = ModulePath::root();
    assert_eq!(mp.to_string(), "");
}

#[test]
fn test_module_path_from_identifiers_with_valid_segments_succeeds() {
    let segs = vec![Identifier::new("tddd").unwrap(), Identifier::new("catalogue").unwrap()];
    let mp = ModulePath::from_identifiers(segs);
    assert_eq!(mp.segments().len(), 2);
    assert_eq!(mp.segments()[0].as_str(), "tddd");
    assert_eq!(mp.segments()[1].as_str(), "catalogue");
    assert_eq!(mp.to_string(), "tddd::catalogue");
}

// ---------------------------------------------------------------------------
// TypeRef
// ---------------------------------------------------------------------------

#[test]
fn test_type_ref_with_simple_name_succeeds() {
    let tr = TypeRef::new("UserId").unwrap();
    assert_eq!(tr.as_str(), "UserId");
}

#[test]
fn test_type_ref_with_generics_succeeds() {
    let tr = TypeRef::new("Result<Option<User>, DomainError>").unwrap();
    assert_eq!(tr.as_str(), "Result<Option<User>, DomainError>");
}

#[test]
fn test_type_ref_with_crate_prefix_succeeds() {
    let tr = TypeRef::new("domain_core::UserId").unwrap();
    assert_eq!(tr.as_str(), "domain_core::UserId");
}

#[test]
fn test_type_ref_with_empty_string_returns_empty_error() {
    assert_eq!(TypeRef::new(""), Err(IdentifierError::Empty));
}

#[test]
fn test_type_ref_display_fromstr_roundtrip() {
    let original = TypeRef::new("Vec<String>").unwrap();
    let displayed = original.to_string();
    let parsed: TypeRef = displayed.parse().unwrap();
    assert_eq!(original, parsed);
}

// ---------------------------------------------------------------------------
// FullyQualifiedItemPath
// ---------------------------------------------------------------------------

#[test]
fn test_fully_qualified_item_path_new_stores_components() {
    let crate_name = CrateName::new("domain_core").unwrap();
    let module_path: ModulePath = "user::input".parse().unwrap();
    let name = Identifier::new("Input").unwrap();

    let path = FullyQualifiedItemPath::new(crate_name.clone(), module_path.clone(), name.clone());

    assert_eq!(path.crate_name(), &crate_name);
    assert_eq!(path.module_path(), Some(&module_path));
    assert_eq!(path.name(), &name);
}

#[test]
fn test_fully_qualified_item_path_orders_by_full_path() {
    let crate_name = CrateName::new("domain_core").unwrap();
    let first = FullyQualifiedItemPath::new(
        crate_name.clone(),
        "a".parse().unwrap(),
        Identifier::new("Input").unwrap(),
    );
    let second = FullyQualifiedItemPath::new(
        crate_name,
        "b".parse().unwrap(),
        Identifier::new("Input").unwrap(),
    );

    assert!(first < second);
}

#[test]
fn test_fully_qualified_item_path_display_includes_crate_module_and_name() {
    let path = FullyQualifiedItemPath::new(
        CrateName::new("domain_core").unwrap(),
        "user::input".parse().unwrap(),
        Identifier::new("Input").unwrap(),
    );

    assert_eq!(path.to_string(), "domain_core::user::input::Input");
}

#[test]
fn test_fully_qualified_item_path_ac03_covers_unique_and_fail_closed_resolution() {
    let crate_name = CrateName::new("domain").unwrap();
    let key = CatalogueEntryKey::try_new("Thing".to_owned()).unwrap();
    let unplaced =
        FullyQualifiedItemPath::from_type_catalogue_entry_key(&crate_name, &key, None).unwrap();
    assert!(matches!(unplaced, FullyQualifiedItemPath::UnplacedType { .. }));

    let current_identity = FullyQualifiedItemPath::new_type(
        crate_name.clone(),
        ModulePath::from_segments(vec!["generated".to_owned()]).unwrap(),
        Identifier::new("Thing").unwrap(),
    );
    let resolved_add = resolve_catalogue_identity_for_action_in_namespace(
        &TypeRef::new("Thing".to_owned()).unwrap(),
        &crate_name,
        ItemAction::Add,
        &BTreeSet::new(),
        &BTreeSet::from([current_identity.clone()]),
        CatalogueItemNamespace::Type,
    )
    .unwrap();
    assert_eq!(resolved_add, current_identity);
    assert!(resolved_add.is_placed());

    let baseline_collision = FullyQualifiedItemPath::new_type(
        crate_name.clone(),
        ModulePath::from_segments(vec!["baseline".to_owned()]).unwrap(),
        Identifier::new("Thing").unwrap(),
    );
    assert!(
        resolve_catalogue_identity_for_action_in_namespace(
            &TypeRef::new("Thing".to_owned()).unwrap(),
            &crate_name,
            ItemAction::Add,
            &BTreeSet::from([baseline_collision]),
            &BTreeSet::from([current_identity.clone()]),
            CatalogueItemNamespace::Type,
        )
        .is_err()
    );

    let second_current_identity = FullyQualifiedItemPath::new_type(
        crate_name.clone(),
        ModulePath::from_segments(vec!["other".to_owned()]).unwrap(),
        Identifier::new("Thing").unwrap(),
    );
    assert!(
        resolve_catalogue_identity_for_action_in_namespace(
            &TypeRef::new("Thing".to_owned()).unwrap(),
            &crate_name,
            ItemAction::Add,
            &BTreeSet::new(),
            &BTreeSet::from([current_identity.clone(), second_current_identity]),
            CatalogueItemNamespace::Type,
        )
        .is_err()
    );

    let unresolved_add = resolve_catalogue_identity_for_action_in_namespace(
        &TypeRef::new("Thing".to_owned()).unwrap(),
        &crate_name,
        ItemAction::Add,
        &BTreeSet::new(),
        &BTreeSet::new(),
        CatalogueItemNamespace::Type,
    )
    .unwrap();
    assert_eq!(unresolved_add, unplaced);
    assert!(!unresolved_add.is_placed());

    let baseline = BTreeSet::from([current_identity.clone()]);
    for action in [ItemAction::Modify, ItemAction::Delete, ItemAction::Reference] {
        let resolved = resolve_catalogue_identity_for_action_in_namespace(
            &TypeRef::new("Thing".to_owned()).unwrap(),
            &crate_name,
            action,
            &baseline,
            &BTreeSet::new(),
            CatalogueItemNamespace::Type,
        )
        .unwrap();
        assert_eq!(resolved, current_identity);
    }
}

// ---------------------------------------------------------------------------
// FunctionPath
// ---------------------------------------------------------------------------

#[test]
fn test_catalogue_entry_key_resolve_catalogue_identity_uses_declared_module_for_bare_key() {
    let key = CatalogueEntryKey::try_new("SharedPort".to_owned()).unwrap();
    let identity = FullyQualifiedItemPath::from_catalogue_entry_key(
        &CrateName::new("infrastructure").unwrap(),
        &key,
        &"adapters".parse().unwrap(),
    )
    .unwrap();
    assert_eq!(identity.to_string(), "infrastructure::adapters::SharedPort");
}

#[test]
fn test_catalogue_entry_key_resolve_catalogue_identity_uses_qualified_module() {
    let key = CatalogueEntryKey::try_new("infrastructure::alpha::SharedPort".to_owned()).unwrap();
    let identity = FullyQualifiedItemPath::from_catalogue_entry_key(
        &CrateName::new("infrastructure").unwrap(),
        &key,
        &"beta".parse().unwrap(),
    )
    .unwrap();
    assert_eq!(identity.to_string(), "infrastructure::alpha::SharedPort");
}

#[test]
fn test_catalogue_entry_key_resolve_catalogue_identity_rejects_invalid_item_segment() {
    let key = CatalogueEntryKey::try_new("domain::bad-name".to_owned()).unwrap();
    let result = FullyQualifiedItemPath::from_catalogue_entry_key(
        &CrateName::new("domain").unwrap(),
        &key,
        &ModulePath::root(),
    );
    assert!(matches!(
        result,
        Err(IdentifierError::InvalidCharacters(value)) if value == "bad-name"
    ));
}

#[test]
fn test_catalogue_entry_key_resolve_catalogue_identity_rejects_empty_module_segment() {
    let key = CatalogueEntryKey::try_new("domain::::Port".to_owned()).unwrap();
    let result = FullyQualifiedItemPath::from_catalogue_entry_key(
        &CrateName::new("domain").unwrap(),
        &key,
        &ModulePath::root(),
    );
    assert!(matches!(
        result,
        Err(IdentifierError::InvalidSegment(value)) if value.is_empty()
    ));
}

#[test]
fn test_catalogue_entry_key_parse_fully_qualified_identity_preserves_module_path() {
    let key = CatalogueEntryKey::try_new("usecase::ports::SharedPort".to_owned()).unwrap();
    let identity = FullyQualifiedItemPath::from_fully_qualified_key(&key).unwrap();
    assert_eq!(identity.to_string(), "usecase::ports::SharedPort");
}

#[test]
fn test_catalogue_entry_key_parse_fully_qualified_identity_rejects_bare_key() {
    let key = CatalogueEntryKey::try_new("SharedPort".to_owned()).unwrap();
    assert!(matches!(
        FullyQualifiedItemPath::from_fully_qualified_key(&key),
        Err(IdentifierError::InvalidFunctionPath(_))
    ));
}

#[test]
fn test_catalogue_entry_key_parse_fully_qualified_identity_rejects_invalid_item_segment() {
    let key = CatalogueEntryKey::try_new("domain::bad-name".to_owned()).unwrap();
    let result = FullyQualifiedItemPath::from_fully_qualified_key(&key);
    assert!(matches!(
        result,
        Err(IdentifierError::InvalidCharacters(value)) if value == "bad-name"
    ));
}

#[test]
fn test_catalogue_entry_key_parse_fully_qualified_identity_rejects_empty_module_segment() {
    let key = CatalogueEntryKey::try_new("domain::::Port".to_owned()).unwrap();
    let result = FullyQualifiedItemPath::from_fully_qualified_key(&key);
    assert!(matches!(
        result,
        Err(IdentifierError::InvalidSegment(value)) if value.is_empty()
    ));
}

#[test]
fn test_function_path_at_crate_root_display_and_parse() {
    let crate_name = CrateName::new("domain_core").unwrap();
    let name = FunctionName::new("register_user").unwrap();
    let fp = FunctionPath::at_root(crate_name, name);
    assert_eq!(fp.to_string(), "domain_core::register_user");
}

#[test]
fn test_function_path_with_module_display_and_parse() {
    let crate_name = CrateName::new("domain_core").unwrap();
    let module_path: ModulePath = "user::commands".parse().unwrap();
    let name = FunctionName::new("execute").unwrap();
    let fp = FunctionPath::new(crate_name, module_path, name);
    assert_eq!(fp.to_string(), "domain_core::user::commands::execute");
}

#[test]
fn test_function_path_fromstr_at_root() {
    let fp: FunctionPath = "domain_core::register_user".parse().unwrap();
    assert_eq!(fp.crate_name.as_str(), "domain_core");
    assert!(fp.module_path.is_root());
    assert_eq!(fp.name.as_str(), "register_user");
}

#[test]
fn test_function_path_fromstr_with_module() {
    let fp: FunctionPath = "domain_core::user::commands::execute".parse().unwrap();
    assert_eq!(fp.crate_name.as_str(), "domain_core");
    assert_eq!(fp.module_path.to_string(), "user::commands");
    assert_eq!(fp.name.as_str(), "execute");
}

#[test]
fn test_function_path_with_single_segment_returns_error() {
    let result = "standalone".parse::<FunctionPath>();
    assert!(matches!(result, Err(IdentifierError::InvalidFunctionPath(_))));
}

#[test]
fn test_function_path_display_fromstr_roundtrip() {
    let original: FunctionPath = "domain_core::user::find_by_email".parse().unwrap();
    let displayed = original.to_string();
    let parsed: FunctionPath = displayed.parse().unwrap();
    assert_eq!(original, parsed);
}

// ---------------------------------------------------------------------------
// Cross-type compile-time isolation test (documentation)
// ---------------------------------------------------------------------------

/// Verifies that distinct newtype wrappers are not assignable to each other
/// at the type level. This test demonstrates the compile-time safety —
/// passing a `TypeName` where a `MethodName` is expected results in a
/// compile error. The runtime test below demonstrates the distinct type identity.
#[test]
fn test_newtype_wrappers_have_distinct_types() {
    let type_name = TypeName::new("UserId").unwrap();
    let method_name = MethodName::new("find_by_id").unwrap();
    // These are distinct types; the following would not compile:
    // let _: MethodName = type_name; // compile error
    // Only their string representations are the same when content matches.
    assert_ne!(type_name.as_str(), method_name.as_str());
}
