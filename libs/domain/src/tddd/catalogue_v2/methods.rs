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
// MethodGenericParam — a single generic type parameter on a method (V2)
// ---------------------------------------------------------------------------

/// A single generic type parameter on a method or associated function.
///
/// Used when the method is declared with APIT (`impl Into<String>`) or an
/// explicit generic parameter (`fn new<T: Into<String>>(value: T) -> Self`).
/// The rustdoc C-side desugars APIT into synthetic `GenericParamDef` entries
/// (e.g. `T0: Into<String>`); this struct mirrors that representation so that
/// the A-codec can produce a matching `ExtendedCrate`.
///
/// Used in [`MethodDeclaration::generics`].
///
/// Both fields use validated newtypes to make illegal states unrepresentable:
/// - `name: ParamName` — a valid Rust identifier (validated via `Identifier`).
/// - `bounds: Vec<TypeRef>` — non-empty type/trait reference strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodGenericParam {
    /// The synthetic or explicit parameter name (e.g. `T0`, `T`).
    ///
    /// Must be a valid Rust identifier (`[a-zA-Z_][a-zA-Z0-9_]*`).
    pub name: ParamName,
    /// The trait bounds imposed on this parameter (e.g. `[Into<String>]`, `[?Sized]`).
    ///
    /// Each entry is a non-empty type/trait reference string validated at the
    /// codec boundary. `?Sized` is accepted (as `syn::TypeParamBound`, not
    /// `syn::Type`) and stored as a plain `TypeRef`.
    pub bounds: Vec<TypeRef>,
}

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
    /// Generic type parameters on this method.
    ///
    /// Populated when the method is declared with APIT (`impl Into<String>`) or
    /// an explicit generic parameter. Default empty Vec for backward compatibility.
    /// The A-codec encodes these as `GenericParamDef::Type` entries in the function's
    /// `Generics`, mirroring the rustdoc C-side APIT desugaring.
    pub generics: Vec<MethodGenericParam>,
    /// Optional documentation string.
    pub docs: Option<String>,
}

impl MethodDeclaration {
    /// Creates a new `MethodDeclaration` with no generic parameters.
    #[must_use]
    pub fn new(
        name: MethodName,
        receiver: Option<SelfReceiver>,
        params: Vec<ParamDeclaration>,
        returns: TypeRef,
        is_async: bool,
        docs: Option<String>,
    ) -> Self {
        Self { name, receiver, params, returns, is_async, generics: vec![], docs }
    }

    /// Creates a `MethodDeclaration` for an associated function (no `self` receiver).
    #[must_use]
    pub fn associated_function(
        name: MethodName,
        params: Vec<ParamDeclaration>,
        returns: TypeRef,
    ) -> Self {
        Self {
            name,
            receiver: None,
            params,
            returns,
            is_async: false,
            generics: vec![],
            docs: None,
        }
    }

    /// Reconstructs a human-readable signature string from the structured fields
    /// for rendering / debugging.
    ///
    /// Layout:
    ///
    /// ```text
    /// [async ]fn name[<T0: Bound0>](receiver[, param1: ty1, param2: ty2]) -> returns
    /// ```
    ///
    /// The unit return type is rendered as `"()"` when the returns field is `"()"`.
    /// Generic parameters are rendered as `<T0: Bound0, T1: Bound1>` when present.
    #[must_use]
    pub fn signature_string(&self) -> String {
        let async_prefix = if self.is_async { "async " } else { "" };
        let generics_str = if self.generics.is_empty() {
            String::new()
        } else {
            let params: Vec<String> = self
                .generics
                .iter()
                .map(|g| {
                    if g.bounds.is_empty() {
                        g.name.as_str().to_owned()
                    } else {
                        let bounds_str =
                            g.bounds.iter().map(|b| b.as_str()).collect::<Vec<_>>().join(" + ");
                        format!("{}: {}", g.name.as_str(), bounds_str)
                    }
                })
                .collect();
            format!("<{}>", params.join(", "))
        };
        let receiver_string = self.receiver.map(|r| r.to_string());
        let receiver_str = receiver_string.as_deref().unwrap_or("");
        let params_str = self
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name.as_str(), p.ty.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        let args = match (receiver_str.is_empty(), params_str.is_empty()) {
            (true, true) => String::new(),
            (true, false) => params_str,
            (false, true) => receiver_str.to_string(),
            (false, false) => format!("{receiver_str}, {params_str}"),
        };
        format!(
            "{async_prefix}fn {}{}({}) -> {}",
            self.name.as_str(),
            generics_str,
            args,
            self.returns.as_str()
        )
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
    // MethodGenericParam
    // -----------------------------------------------------------------------

    #[test]
    fn test_method_generic_param_with_bounds_stores_fields() {
        let p = MethodGenericParam {
            name: ParamName::new("T0").unwrap(),
            bounds: vec![TypeRef::new("Into<String>").unwrap()],
        };
        assert_eq!(p.name.as_str(), "T0");
        assert_eq!(p.bounds[0].as_str(), "Into<String>");
    }

    #[test]
    fn test_method_generic_param_without_bounds_is_valid() {
        let p = MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] };
        assert!(p.bounds.is_empty());
    }

    #[test]
    fn test_method_generic_param_equality() {
        let a = MethodGenericParam {
            name: ParamName::new("T0").unwrap(),
            bounds: vec![TypeRef::new("Send").unwrap()],
        };
        let b = MethodGenericParam {
            name: ParamName::new("T0").unwrap(),
            bounds: vec![TypeRef::new("Send").unwrap()],
        };
        let c = MethodGenericParam {
            name: ParamName::new("T1").unwrap(),
            bounds: vec![TypeRef::new("Send").unwrap()],
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

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
    fn test_method_declaration_with_generics_stores_and_renders_them() {
        let name = MethodName::new("new").unwrap();
        let param =
            ParamDeclaration::new(ParamName::new("value").unwrap(), TypeRef::new("T0").unwrap());
        let returns = TypeRef::new("Self").unwrap();
        let mut decl = MethodDeclaration::associated_function(name, vec![param], returns);
        decl.generics = vec![MethodGenericParam {
            name: ParamName::new("T0").unwrap(),
            bounds: vec![TypeRef::new("Into<String>").unwrap()],
        }];
        assert_eq!(decl.generics.len(), 1);
        assert_eq!(decl.generics[0].name.as_str(), "T0");
        let sig = decl.signature_string();
        assert!(sig.contains("<T0: Into<String>>"), "sig={sig}");
    }

    #[test]
    fn test_method_declaration_new_has_empty_generics_by_default() {
        let name = MethodName::new("save").unwrap();
        let returns = TypeRef::new("()").unwrap();
        let decl = MethodDeclaration::new(name, None, vec![], returns, false, None);
        assert!(decl.generics.is_empty());
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
