//! Method and parameter declaration types for the catalogue v2 schema.
//!
//! Implements:
//! - [`ParamDeclaration`]: a single method/function parameter using newtype fields
//!   (`ParamName`, `TypeRef`). Supersedes the old `catalogue::ParamDeclaration` String fields.
//! - [`MethodDeclaration`]: a method signature using newtype fields (`MethodName`, `SelfReceiver`,
//!   `TypeRef`). Supersedes the old `catalogue::MethodDeclaration` String fields.
//!
//! **Module separation**: these types live in `tddd::catalogue_v2::methods`. The old
//! `catalogue::ParamDeclaration` and `catalogue::MethodDeclaration` (plain String fields)
//! remain in `tddd::catalogue` until T008 removes them. The module path disambiguates
//! the two at the Rust type level; there is no Rust-level shadowing between separate modules.
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec (T003) handles JSON.

use crate::tddd::catalogue_v2::identifiers::{MethodName, ParamName, TypeRef};
use crate::tddd::catalogue_v2::roles::SelfReceiver;

// ---------------------------------------------------------------------------
// ParamDeclaration — single parameter in a method/function signature (V2)
// ---------------------------------------------------------------------------

/// A single parameter in a method or function signature, using newtype fields.
///
/// Supersedes `catalogue::ParamDeclaration` (which uses plain `String` fields).
/// Uses typed newtypes (`ParamName`, `TypeRef`) so that passing a `ParamName` where a
/// `TypeRef` is expected (or vice versa) is a compile-time error (ADR 1 D5 / D8).
///
/// Lives in `tddd::catalogue_v2::methods`; the old `catalogue::ParamDeclaration`
/// remains in `tddd::catalogue` until T008.
///
/// Used in [`MethodDeclaration::params`] and
/// [`crate::tddd::catalogue_v2::entries::FunctionEntry::params`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamDeclaration {
    /// The parameter binding name.
    pub name: ParamName,
    /// The parameter type (generics-inclusive type reference string).
    pub ty: TypeRef,
}

impl ParamDeclaration {
    /// Creates a new `ParamDeclaration`.
    #[must_use]
    pub fn new(name: ParamName, ty: TypeRef) -> Self {
        Self { name, ty }
    }
}

// ---------------------------------------------------------------------------
// MethodDeclaration — structured method signature (V2)
// ---------------------------------------------------------------------------

/// A structured method signature, using newtype fields.
///
/// Supersedes `catalogue::MethodDeclaration` (which uses plain `String` fields).
/// Uses typed newtypes (`MethodName`, `SelfReceiver`, `TypeRef`, `ParamName`) so that
/// type confusion between identifier kinds is caught at compile time (ADR 1 D5 / D8).
///
/// Lives in `tddd::catalogue_v2::methods`; the old `catalogue::MethodDeclaration`
/// remains in `tddd::catalogue` until T008.
///
/// `receiver: Option<SelfReceiver>`:
/// - `None` — associated function (no `self` parameter).
/// - `Some(SelfReceiver::Owned)` — `self` (value receiver).
/// - `Some(SelfReceiver::SharedRef)` — `&self`.
/// - `Some(SelfReceiver::ExclusiveRef)` — `&mut self`.
///
/// Used in [`crate::tddd::catalogue_v2::entries::TypeEntry::methods`] and
/// [`crate::tddd::catalogue_v2::entries::TraitEntry::methods`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodDeclaration {
    /// The method name.
    pub name: MethodName,
    /// The self-receiver form. `None` = associated function (no receiver).
    pub receiver: Option<SelfReceiver>,
    /// The method parameters (excludes the self receiver).
    pub params: Vec<ParamDeclaration>,
    /// The return type (generics-inclusive type reference string).
    pub returns: TypeRef,
    /// Whether this method is `async`.
    pub is_async: bool,
    /// Optional documentation string.
    pub docs: Option<String>,
}

impl MethodDeclaration {
    /// Creates a new `MethodDeclaration`.
    #[must_use]
    pub fn new(
        name: MethodName,
        receiver: Option<SelfReceiver>,
        params: Vec<ParamDeclaration>,
        returns: TypeRef,
        is_async: bool,
        docs: Option<String>,
    ) -> Self {
        Self { name, receiver, params, returns, is_async, docs }
    }

    /// Creates a `MethodDeclaration` for an associated function (no `self` receiver).
    #[must_use]
    pub fn associated_function(
        name: MethodName,
        params: Vec<ParamDeclaration>,
        returns: TypeRef,
    ) -> Self {
        Self { name, receiver: None, params, returns, is_async: false, docs: None }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // ParamDeclaration
    // -----------------------------------------------------------------------

    #[test]
    fn test_param_declaration_new_stores_name_and_ty() {
        let name = ParamName::new("id").unwrap();
        let ty = TypeRef::new("UserId").unwrap();
        let decl = ParamDeclaration::new(name.clone(), ty.clone());
        assert_eq!(decl.name, name);
        assert_eq!(decl.ty, ty);
    }

    #[test]
    fn test_param_declaration_with_generic_type_ref_succeeds() {
        let name = ParamName::new("items").unwrap();
        let ty = TypeRef::new("Vec<OrderItem>").unwrap();
        let decl = ParamDeclaration { name, ty };
        assert_eq!(decl.ty.as_str(), "Vec<OrderItem>");
    }

    #[test]
    fn test_param_declaration_name_and_ty_are_distinct_types_at_compile_time() {
        // ParamName and TypeRef are distinct newtypes — this test documents the invariant.
        let name = ParamName::new("count").unwrap();
        let ty = TypeRef::new("u32").unwrap();
        assert_eq!(name.as_str(), "count");
        assert_eq!(ty.as_str(), "u32");
    }

    #[test]
    fn test_param_declaration_equality_by_name_and_ty() {
        let name = ParamName::new("user").unwrap();
        let ty = TypeRef::new("User").unwrap();
        let a = ParamDeclaration::new(name.clone(), ty.clone());
        let b = ParamDeclaration::new(name, ty);
        assert_eq!(a, b);
    }

    // -----------------------------------------------------------------------
    // MethodDeclaration
    // -----------------------------------------------------------------------

    #[test]
    fn test_method_declaration_with_shared_ref_receiver() {
        let name = MethodName::new("find_by_id").unwrap();
        let param =
            ParamDeclaration::new(ParamName::new("id").unwrap(), TypeRef::new("UserId").unwrap());
        let returns = TypeRef::new("Result<User, DomainError>").unwrap();
        let decl = MethodDeclaration::new(
            name.clone(),
            Some(SelfReceiver::SharedRef),
            vec![param.clone()],
            returns.clone(),
            false,
            None,
        );
        assert_eq!(decl.name, name);
        assert_eq!(decl.receiver, Some(SelfReceiver::SharedRef));
        assert_eq!(decl.params.len(), 1);
        assert_eq!(decl.params[0], param);
        assert_eq!(decl.returns, returns);
        assert!(!decl.is_async);
        assert_eq!(decl.docs, None);
    }

    #[test]
    fn test_method_declaration_associated_function_has_no_receiver() {
        let name = MethodName::new("new").unwrap();
        let returns = TypeRef::new("Self").unwrap();
        let decl = MethodDeclaration::associated_function(name.clone(), vec![], returns.clone());
        assert_eq!(decl.name, name);
        assert_eq!(decl.receiver, None);
        assert!(decl.params.is_empty());
        assert_eq!(decl.returns, returns);
        assert!(!decl.is_async);
    }

    #[test]
    fn test_method_declaration_with_owned_receiver() {
        let name = MethodName::new("consume").unwrap();
        let returns = TypeRef::new("()").unwrap();
        let decl =
            MethodDeclaration::new(name, Some(SelfReceiver::Owned), vec![], returns, false, None);
        assert_eq!(decl.receiver, Some(SelfReceiver::Owned));
    }

    #[test]
    fn test_method_declaration_with_exclusive_ref_receiver() {
        let name = MethodName::new("update").unwrap();
        let param = ParamDeclaration::new(
            ParamName::new("new_value").unwrap(),
            TypeRef::new("String").unwrap(),
        );
        let returns = TypeRef::new("()").unwrap();
        let decl = MethodDeclaration::new(
            name,
            Some(SelfReceiver::ExclusiveRef),
            vec![param],
            returns,
            false,
            None,
        );
        assert_eq!(decl.receiver, Some(SelfReceiver::ExclusiveRef));
        assert_eq!(decl.params.len(), 1);
    }

    #[test]
    fn test_method_declaration_async_with_docs() {
        let name = MethodName::new("execute").unwrap();
        let returns = TypeRef::new("Result<(), ApplicationError>").unwrap();
        let decl = MethodDeclaration::new(
            name,
            Some(SelfReceiver::SharedRef),
            vec![],
            returns,
            true,
            Some("Execute the use case.".to_string()),
        );
        assert!(decl.is_async);
        assert_eq!(decl.docs, Some("Execute the use case.".to_string()));
    }

    #[test]
    fn test_method_declaration_with_multiple_params_succeeds() {
        let name = MethodName::new("register").unwrap();
        let p1 =
            ParamDeclaration::new(ParamName::new("email").unwrap(), TypeRef::new("Email").unwrap());
        let p2 = ParamDeclaration::new(
            ParamName::new("password").unwrap(),
            TypeRef::new("Password").unwrap(),
        );
        let returns = TypeRef::new("Result<UserId, DomainError>").unwrap();
        let decl = MethodDeclaration::new(
            name,
            Some(SelfReceiver::SharedRef),
            vec![p1, p2],
            returns,
            false,
            None,
        );
        assert_eq!(decl.params.len(), 2);
    }

    #[test]
    fn test_method_declaration_equality_by_all_fields() {
        let name = MethodName::new("save").unwrap();
        let param =
            ParamDeclaration::new(ParamName::new("user").unwrap(), TypeRef::new("User").unwrap());
        let returns = TypeRef::new("Result<(), DomainError>").unwrap();
        let a = MethodDeclaration::new(
            name.clone(),
            Some(SelfReceiver::SharedRef),
            vec![param.clone()],
            returns.clone(),
            false,
            None,
        );
        let b = MethodDeclaration::new(
            name,
            Some(SelfReceiver::SharedRef),
            vec![param],
            returns,
            false,
            None,
        );
        assert_eq!(a, b);
    }

    #[test]
    fn test_method_declaration_none_receiver_is_not_equal_to_owned_receiver() {
        let name = MethodName::new("init").unwrap();
        let returns = TypeRef::new("Self").unwrap();
        let associated =
            MethodDeclaration::new(name.clone(), None, vec![], returns.clone(), false, None);
        let owned =
            MethodDeclaration::new(name, Some(SelfReceiver::Owned), vec![], returns, false, None);
        assert_ne!(associated, owned);
    }
}
