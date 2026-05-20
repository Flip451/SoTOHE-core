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
// BoundOp — the operator in a where-clause predicate
// ---------------------------------------------------------------------------

/// The operator in a `where` clause predicate.
///
/// Corresponds to the two forms of Rust where-clause predicates:
/// - `Bound` — the `:` operator (e.g. `T: Clone + Send`)
/// - `Equal` — the `=` operator (e.g. `T::Assoc = U`)
///
/// Used in [`WherePredicateDecl::operator`].
///
/// (ADR `2026-05-18-1223-make-catalogue-schema-permissive` D1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoundOp {
    /// The `:` operator. `rhs` is a list of trait / lifetime bounds.
    #[default]
    Bound,
    /// The `=` operator. `rhs` must contain exactly one element.
    Equal,
}

// ---------------------------------------------------------------------------
// WherePredicateDecl — generic where-clause predicate (lhs / rhs / operator)
// ---------------------------------------------------------------------------

/// A single `where` clause predicate in `lhs operator rhs` form.
///
/// Represents the essential structure of a Rust where clause: a left-hand side
/// type expression, an operator (`:` or `=`), and one or more right-hand side
/// bounds.
///
/// - `lhs` — the left-hand side type expression.  May be a bare identifier (`T`),
///   a parameterised type (`Vec<T>`, `Hoge<Fuga>`), a projection (`T::Item`), or
///   an HRTB-prefixed expression (`for<'a> T::Item<'a>`).
/// - `rhs` — the right-hand side bounds.  For `Bound` predicates this is the
///   `+`-separated list of trait / lifetime bounds (`[Clone, Send]` for
///   `T: Clone + Send`).  For `Equal` predicates this is a single-element Vec
///   (`[U]` for `T::Assoc = U`).
/// - `operator` — the predicate operator (`BoundOp::Bound` or `BoundOp::Equal`).
///
/// Used in [`MethodDeclaration::where_predicates`] and
/// [`crate::tddd::catalogue_v2::entries::FunctionEntry::where_predicates`].
///
/// (ADR `2026-05-18-1223-make-catalogue-schema-permissive` D1 — supersedes
/// the earlier 2-field structure from `2026-05-13-1153-tddd-where-form-generics-normalization` D2)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WherePredicateDecl {
    /// The left-hand side type expression of the where predicate.
    ///
    /// May be a bare identifier (`T`), a parameterised type (`Vec<T>`,
    /// `Hoge<Fuga>`), a projection (`T::Item`), or an HRTB-prefixed
    /// expression (`for<'a> T::Item<'a>`).  The codec validates this as a
    /// non-empty string; full Rust type-expression syntax checking happens
    /// during A-codec encoding.
    pub lhs: TypeRef,
    /// The right-hand side bounds of the where predicate.
    ///
    /// For `BoundOp::Bound` predicates: the `+`-separated list of trait /
    /// lifetime bounds (e.g. `[Clone, Send]` for `T: Clone + Send`).
    /// For `BoundOp::Equal` predicates: a single-element Vec (e.g. `[U]`
    /// for `T::Assoc = U`).
    ///
    /// Each element is a non-empty string validated at the codec boundary.
    pub rhs: Vec<TypeRef>,
    /// The predicate operator.
    ///
    /// Defaults to `BoundOp::Bound` (the `:` operator).
    pub operator: BoundOp,
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
    /// Whether this trait method declaration carries a default implementation
    /// (rustdoc-flavor `provided_trait_methods`).
    ///
    /// `true` — the method has a default implementation provided by the trait
    /// itself (rustdoc emits `Function.has_body = true`).
    /// `false` — the method is required / abstract (rustdoc emits
    /// `Function.has_body = false`).
    ///
    /// For struct inherent method declarations this field is conceptually not
    /// meaningful and is always treated as `false` by the catalogue codec; the
    /// codec forces `Function.has_body = true` for inherent methods regardless.
    /// (ADR `2026-05-08-0248` D13)
    pub has_default_impl: bool,
    /// Generic type parameters on this method.
    ///
    /// Populated when the method is declared with APIT (`impl Into<String>`) or
    /// an explicit generic parameter. Default empty Vec for backward compatibility.
    /// The A-codec encodes these as `GenericParamDef::Type` entries in the function's
    /// `Generics`, mirroring the rustdoc C-side APIT desugaring.
    pub generics: Vec<MethodGenericParam>,
    /// `where`-clause bound predicates on this method's generics.
    ///
    /// Used to express bounds whose LHS is an arbitrary type expression — patterns
    /// `MethodGenericParam.bounds` (whose LHS is a single identifier) cannot represent,
    /// such as `where Vec<T>: Clone` or `where T::Item: Send`. Default empty Vec for
    /// backward compatibility. The A-codec emits every bound (from either
    /// `generics[].bounds` or `where_predicates`) into rustdoc
    /// `Function.generics.where_predicates`, leaving inline `GenericParamDef.bounds`
    /// always empty. The signal evaluator normalizes C-side inline bounds into the
    /// same where-form before fingerprinting so that `<T: Bound>` (APIT or inline-form
    /// source) compares structurally equal to `<T> where T: Bound` (explicit where-form
    /// source).
    ///
    /// (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D1, D2)
    pub where_predicates: Vec<WherePredicateDecl>,
    /// Optional documentation string.
    pub docs: Option<String>,
}

impl MethodDeclaration {
    /// Creates a new `MethodDeclaration` with no generic parameters and
    /// `has_default_impl: false` (default — required / abstract method).
    #[must_use]
    pub fn new(
        name: MethodName,
        receiver: Option<SelfReceiver>,
        params: Vec<ParamDeclaration>,
        returns: TypeRef,
        is_async: bool,
        docs: Option<String>,
    ) -> Self {
        Self {
            name,
            receiver,
            params,
            returns,
            is_async,
            has_default_impl: false,
            generics: vec![],
            where_predicates: vec![],
            docs,
        }
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
            has_default_impl: false,
            generics: vec![],
            where_predicates: vec![],
            docs: None,
        }
    }

    /// Reconstructs a human-readable signature string from the structured fields
    /// for rendering / debugging.
    ///
    /// Layout:
    ///
    /// ```text
    /// [async ]fn name[<T0: Bound0>](receiver[, param1: ty1, param2: ty2]) -> returns[ where Pred1, Pred2]
    /// ```
    ///
    /// The unit return type is rendered as `"()"` when the returns field is `"()"`.
    /// Generic parameters are rendered as `<T0: Bound0, T1: Bound1>` when present.
    /// `where_predicates` (when non-empty) are appended as
    /// `where lhs1: bound1a + bound1b, lhs2: bound2a` so methods that differ only by
    /// their where-clauses stringify distinctly.
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
        let where_str = if self.where_predicates.is_empty() {
            String::new()
        } else {
            let preds: Vec<String> = self
                .where_predicates
                .iter()
                .map(|w| {
                    let rhs_str = w.rhs.iter().map(|b| b.as_str()).collect::<Vec<_>>().join(" + ");
                    match w.operator {
                        BoundOp::Bound => format!("{}: {}", w.lhs.as_str(), rhs_str),
                        BoundOp::Equal => format!("{} = {}", w.lhs.as_str(), rhs_str),
                    }
                })
                .collect();
            format!(" where {}", preds.join(", "))
        };
        format!(
            "{async_prefix}fn {}{}({}) -> {}{}",
            self.name.as_str(),
            generics_str,
            args,
            self.returns.as_str(),
            where_str
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
    fn test_method_declaration_new_defaults_has_default_impl_to_false() {
        let name = MethodName::new("op").unwrap();
        let returns = TypeRef::new("()").unwrap();
        let decl = MethodDeclaration::new(name, None, vec![], returns, false, None);
        assert!(
            !decl.has_default_impl,
            "MethodDeclaration::new must default has_default_impl to false"
        );
    }

    #[test]
    fn test_method_declaration_associated_function_defaults_has_default_impl_to_false() {
        let name = MethodName::new("new").unwrap();
        let returns = TypeRef::new("Self").unwrap();
        let decl = MethodDeclaration::associated_function(name, vec![], returns);
        assert!(
            !decl.has_default_impl,
            "MethodDeclaration::associated_function must default has_default_impl to false"
        );
    }

    #[test]
    fn test_method_declaration_can_set_has_default_impl_true_for_provided_trait_method() {
        // Per ADR 2026-05-08-0248 D13: traits with provided default impls must be
        // expressible at the catalogue level so the A-codec can emit has_body=true.
        let name = MethodName::new("describe").unwrap();
        let returns = TypeRef::new("String").unwrap();
        let mut decl = MethodDeclaration::new(
            name,
            Some(SelfReceiver::SharedRef),
            vec![],
            returns,
            false,
            None,
        );
        decl.has_default_impl = true;
        assert!(decl.has_default_impl);
    }

    #[test]
    fn test_method_declaration_has_default_impl_distinguishes_otherwise_equal_methods() {
        let name = MethodName::new("op").unwrap();
        let returns = TypeRef::new("()").unwrap();
        let abstract_method = MethodDeclaration::new(
            name.clone(),
            Some(SelfReceiver::SharedRef),
            vec![],
            returns.clone(),
            false,
            None,
        );
        let mut provided_method = abstract_method.clone();
        provided_method.has_default_impl = true;
        assert_ne!(
            abstract_method, provided_method,
            "has_default_impl participates in MethodDeclaration equality"
        );
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

    // -----------------------------------------------------------------------
    // BoundOp — operator enum (ADR 2026-05-18-1223 D1)
    // -----------------------------------------------------------------------

    #[test]
    fn test_bound_op_default_is_bound() {
        let op = BoundOp::default();
        assert_eq!(op, BoundOp::Bound, "BoundOp::default() must be BoundOp::Bound");
    }

    #[test]
    fn test_bound_op_bound_and_equal_are_distinct() {
        assert_ne!(BoundOp::Bound, BoundOp::Equal);
    }

    // -----------------------------------------------------------------------
    // WherePredicateDecl — 3-field structure (ADR 2026-05-18-1223 D1)
    // -----------------------------------------------------------------------

    #[test]
    fn test_where_predicate_decl_stores_lhs_rhs_operator_bound() {
        let w = WherePredicateDecl {
            lhs: TypeRef::new("Vec<T>").unwrap(),
            rhs: vec![TypeRef::new("Send").unwrap(), TypeRef::new("Sync").unwrap()],
            operator: BoundOp::Bound,
        };
        assert_eq!(w.lhs.as_str(), "Vec<T>");
        assert_eq!(w.rhs.len(), 2);
        assert_eq!(w.rhs[0].as_str(), "Send");
        assert_eq!(w.rhs[1].as_str(), "Sync");
        assert_eq!(w.operator, BoundOp::Bound);
    }

    #[test]
    fn test_where_predicate_decl_stores_lhs_rhs_operator_equal() {
        let w = WherePredicateDecl {
            lhs: TypeRef::new("T::Assoc").unwrap(),
            rhs: vec![TypeRef::new("u32").unwrap()],
            operator: BoundOp::Equal,
        };
        assert_eq!(w.lhs.as_str(), "T::Assoc");
        assert_eq!(w.rhs.len(), 1);
        assert_eq!(w.rhs[0].as_str(), "u32");
        assert_eq!(w.operator, BoundOp::Equal);
    }

    #[test]
    fn test_where_predicate_decl_equality_depends_on_all_three_fields() {
        let base = WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        };
        let different_operator = WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Equal,
        };
        let different_lhs = WherePredicateDecl {
            lhs: TypeRef::new("U").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        };
        let different_rhs = WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Send").unwrap()],
            operator: BoundOp::Bound,
        };
        let equal = WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        };
        assert_eq!(base, equal);
        assert_ne!(base, different_operator);
        assert_ne!(base, different_lhs);
        assert_ne!(base, different_rhs);
    }

    #[test]
    fn test_where_predicate_decl_hrtb_lhs_is_supported() {
        // HRTB バインダーは lhs 先頭に組み込む (ADR D1)
        let w = WherePredicateDecl {
            lhs: TypeRef::new("for<'a> T::Item<'a>").unwrap(),
            rhs: vec![TypeRef::new("Send").unwrap()],
            operator: BoundOp::Bound,
        };
        assert_eq!(w.lhs.as_str(), "for<'a> T::Item<'a>");
        assert_eq!(w.operator, BoundOp::Bound);
    }

    // -----------------------------------------------------------------------
    // WherePredicateDecl / MethodDeclaration.where_predicates
    // -----------------------------------------------------------------------

    #[test]
    fn test_method_declaration_new_defaults_where_predicates_to_empty() {
        let name = MethodName::new("save").unwrap();
        let returns = TypeRef::new("()").unwrap();
        let decl = MethodDeclaration::new(name, None, vec![], returns, false, None);
        assert!(decl.where_predicates.is_empty());
    }

    #[test]
    fn test_method_declaration_associated_function_defaults_where_predicates_to_empty() {
        let name = MethodName::new("new").unwrap();
        let returns = TypeRef::new("Self").unwrap();
        let decl = MethodDeclaration::associated_function(name, vec![], returns);
        assert!(decl.where_predicates.is_empty());
    }

    #[test]
    fn test_method_declaration_where_predicates_render_in_signature_string_bound() {
        let name = MethodName::new("collect").unwrap();
        let returns = TypeRef::new("Vec<T>").unwrap();
        let mut decl = MethodDeclaration::associated_function(name, vec![], returns);
        decl.generics =
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }];
        decl.where_predicates = vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap()],
            operator: BoundOp::Bound,
        }];
        let sig = decl.signature_string();
        assert!(sig.contains(" where T: Clone"), "sig={sig}");
    }

    #[test]
    fn test_method_declaration_where_predicates_render_in_signature_string_equal() {
        let name = MethodName::new("project").unwrap();
        let returns = TypeRef::new("()").unwrap();
        let mut decl = MethodDeclaration::associated_function(name, vec![], returns);
        decl.generics =
            vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }];
        decl.where_predicates = vec![WherePredicateDecl {
            lhs: TypeRef::new("T::Assoc").unwrap(),
            rhs: vec![TypeRef::new("u32").unwrap()],
            operator: BoundOp::Equal,
        }];
        let sig = decl.signature_string();
        assert!(sig.contains(" where T::Assoc = u32"), "sig={sig}");
    }

    #[test]
    fn test_method_declaration_where_predicates_distinguish_otherwise_equal_methods() {
        let name = MethodName::new("op").unwrap();
        let returns = TypeRef::new("()").unwrap();
        let unconstrained = MethodDeclaration::new(
            name.clone(),
            Some(SelfReceiver::SharedRef),
            vec![],
            returns.clone(),
            false,
            None,
        );
        let mut constrained = unconstrained.clone();
        constrained.where_predicates = vec![WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Send").unwrap()],
            operator: BoundOp::Bound,
        }];
        assert_ne!(
            unconstrained, constrained,
            "where_predicates participates in MethodDeclaration equality"
        );
    }

    #[test]
    fn test_where_predicate_decl_round_trip_equality() {
        // Domain round-trip test: construct → clone → compare (ADR 2026-05-18-1223 D1)
        let pred = WherePredicateDecl {
            lhs: TypeRef::new("T").unwrap(),
            rhs: vec![TypeRef::new("Clone").unwrap(), TypeRef::new("Send").unwrap()],
            operator: BoundOp::Bound,
        };
        let pred2 = pred.clone();
        assert_eq!(pred, pred2, "WherePredicateDecl must satisfy clone equality");
        // field accessors work correctly after clone
        assert_eq!(pred2.lhs.as_str(), "T");
        assert_eq!(pred2.rhs.len(), 2);
        assert_eq!(pred2.rhs[0].as_str(), "Clone");
        assert_eq!(pred2.rhs[1].as_str(), "Send");
        assert_eq!(pred2.operator, BoundOp::Bound);
    }
}
