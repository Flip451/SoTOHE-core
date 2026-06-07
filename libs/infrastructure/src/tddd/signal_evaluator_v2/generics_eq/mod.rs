//! Generics, function, and trait structural equality helpers for Phase 2.
//!
//! Provides `generics_structurally_equal`, `build_trait_method_map`,
//! and `fn_sigs_structurally_equal`.
//! These are used by `structural_eq::items_structurally_equal` indirectly via
//! `traits_structurally_equal` and directly for function and generics comparisons.

mod fn_eq;
mod trait_eq;
mod where_form;

// ---------------------------------------------------------------------------
// Re-exports of format helpers for sub-module use.
// These bridge the `pub(super)` visibility of `format.rs` items into this module
// so that `fn_eq`, `trait_eq`, and `where_form` can access them via `super::*`.
// ---------------------------------------------------------------------------

pub(crate) use super::format::{
    apply_canon_to_str, build_generic_canon_map, build_generic_canon_map_from_groups, format_abi,
    format_generic_bounds_with_canon, format_type_with_canon, format_type_with_canon_occ,
    format_where_predicate_with_canon,
};

// ---------------------------------------------------------------------------
// Public API re-exports (accessible as `super::generics_eq::*` from the parent).
// ---------------------------------------------------------------------------

pub(crate) use fn_eq::{fn_sigs_structurally_equal, generics_structurally_equal};
pub(crate) use trait_eq::{build_trait_method_map, traits_structurally_equal};

// ---------------------------------------------------------------------------
// Shared test fixture helpers (cfg(test) only, visible to sibling test modules).
// ---------------------------------------------------------------------------

/// Builds a `GenericBound::TraitBound` for a plain trait name (no args, no HRTB).
///
/// `pub(crate)` so sibling test modules (e.g. `structural_eq::tests`) can import
/// it without duplicating the same fixture construction.
#[cfg(test)]
pub(crate) fn make_simple_trait_bound(trait_name: &str) -> rustdoc_types::GenericBound {
    rustdoc_types::GenericBound::TraitBound {
        trait_: rustdoc_types::Path {
            path: trait_name.to_string(),
            id: rustdoc_types::Id(0),
            args: None,
        },
        generic_params: vec![],
        modifier: rustdoc_types::TraitBoundModifier::None,
    }
}

// ---------------------------------------------------------------------------
// Where-form normalization tests (ADR 2026-05-13-1153 D1 + D3)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::HashMap;

    use rustdoc_types::{
        GenericArgs, GenericBound, GenericParamDef, GenericParamDefKind, Generics, Id, Item, Path,
        Trait, TraitBoundModifier, Type, WherePredicate,
    };

    use super::{generics_structurally_equal, traits_structurally_equal};

    // Helper: a simple named type (no generic args).
    fn ty(name: &str) -> Type {
        Type::ResolvedPath(Path { path: name.to_string(), id: rustdoc_types::Id(999), args: None })
    }

    // Helper: a type-param `<T>` with the given inline bounds.
    fn type_param(name: &str, bounds: Vec<GenericBound>) -> GenericParamDef {
        GenericParamDef {
            name: name.to_string(),
            kind: GenericParamDefKind::Type { bounds, default: None, is_synthetic: false },
        }
    }

    // Helper: a `WherePredicate::BoundPredicate` entry for `<T: bounds...>`.
    fn where_bound(type_name: &str, bounds: Vec<GenericBound>) -> WherePredicate {
        WherePredicate::BoundPredicate {
            type_: Type::Generic(type_name.to_string()),
            bounds,
            generic_params: vec![],
        }
    }

    // Helper: a `GenericBound::TraitBound` for a simple named trait (no args, no HRTB).
    // Delegates to the module-level `make_simple_trait_bound` helper which is also
    // `pub(crate)` so that sibling test modules can share it without duplicating
    // the fixture construction.
    fn trait_bound(trait_name: &str) -> GenericBound {
        super::make_simple_trait_bound(trait_name)
    }

    // Helper: a `GenericBound::TraitBound` for `for<'a> Fn(&'a str)` (HRTB with
    // a binder lifetime) parameterised by the binder `lifetime` name (e.g. `"'a"`)
    // and an `outlives` constraint list on that binder lifetime.
    //
    // Used in D5 tests to avoid repeating the full struct-literal for the
    // `for<BINDER_LIFETIME> Fn(&BINDER_LIFETIME str)` shape.
    fn hrtb_fn_borrowed_str_bound(lifetime: &str, outlives: Vec<String>) -> GenericBound {
        GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::Parenthesized {
                    inputs: vec![Type::BorrowedRef {
                        lifetime: Some(lifetime.to_string()),
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    }],
                    output: None,
                })),
            },
            generic_params: vec![GenericParamDef {
                name: lifetime.to_string(),
                kind: GenericParamDefKind::Lifetime { outlives },
            }],
            modifier: TraitBoundModifier::None,
        }
    }

    // Helper: generics with no params and no where predicates.
    fn empty_generics() -> Generics {
        Generics { params: vec![], where_predicates: vec![] }
    }

    // --- Normalization: inline bounds ≡ explicit where predicates ---

    /// `<T: Clone>` and `<T> where T: Clone` must compare equal
    /// (inline bound lifted to where-form).
    #[test]
    fn test_inline_bound_equals_explicit_where_predicate() {
        let inline = Generics {
            params: vec![type_param("T", vec![trait_bound("Clone")])],
            where_predicates: vec![],
        };
        let where_form = Generics {
            params: vec![type_param("T", vec![])],
            where_predicates: vec![where_bound("T", vec![trait_bound("Clone")])],
        };
        assert!(
            generics_structurally_equal(&inline, &where_form),
            "<T: Clone> must equal <T> where T: Clone"
        );
    }

    /// `<T: A + B>` and `<T> where T: A + B` must compare equal.
    #[test]
    fn test_inline_multi_bound_equals_explicit_where_predicate() {
        let inline = Generics {
            params: vec![type_param("T", vec![trait_bound("A"), trait_bound("B")])],
            where_predicates: vec![],
        };
        let where_form = Generics {
            params: vec![type_param("T", vec![])],
            where_predicates: vec![where_bound("T", vec![trait_bound("A"), trait_bound("B")])],
        };
        assert!(
            generics_structurally_equal(&inline, &where_form),
            "<T: A + B> must equal <T> where T: A + B"
        );
    }

    /// `<T: A>` and `<T: B>` must compare unequal (different bound sets).
    #[test]
    fn test_different_bounds_compare_unequal() {
        let a = Generics {
            params: vec![type_param("T", vec![trait_bound("A")])],
            where_predicates: vec![],
        };
        let b = Generics {
            params: vec![type_param("T", vec![trait_bound("B")])],
            where_predicates: vec![],
        };
        assert!(!generics_structurally_equal(&a, &b), "<T: A> must NOT equal <T: B>");
    }

    /// Empty generics compare equal to empty generics.
    #[test]
    fn test_empty_generics_equal_empty() {
        assert!(
            generics_structurally_equal(&empty_generics(), &empty_generics()),
            "empty generics must compare equal"
        );
    }

    // --- Fail-closed: unsupported predicates / bounds → false even when identical ---

    /// Two identical `Outlives` bounds (e.g. `T: 'static`) must compare equal.
    /// `Outlives` is within D3 scope and compared verbatim by lifetime string so that
    /// `F: 'static + Fn(...)` produces matching fingerprints on both sides.
    #[test]
    fn test_outlives_bound_same_lifetime_compares_equal() {
        let make = || Generics {
            params: vec![type_param("T", vec![GenericBound::Outlives("'static".to_string())])],
            where_predicates: vec![],
        };
        assert!(
            generics_structurally_equal(&make(), &make()),
            "identical Outlives bounds with same lifetime must compare equal"
        );
    }

    /// Two `Outlives` bounds with DIFFERENT lifetime strings must compare unequal
    /// (e.g. `T: 'static` vs `T: 'a`).
    #[test]
    fn test_outlives_bound_different_lifetimes_compare_unequal() {
        let static_bound = Generics {
            params: vec![type_param("T", vec![GenericBound::Outlives("'static".to_string())])],
            where_predicates: vec![],
        };
        let a_bound = Generics {
            params: vec![type_param("T", vec![GenericBound::Outlives("'a".to_string())])],
            where_predicates: vec![],
        };
        assert!(
            !generics_structurally_equal(&static_bound, &a_bound),
            "`T: 'static` must NOT equal `T: 'a`"
        );
    }

    /// `<F: 'static + Send>` and `<F: 'static + Send>` must compare equal —
    /// regression test for `ReviewCheckApprovedInteractor::new<F>` Yellow signal fix.
    #[test]
    fn test_outlives_mixed_with_trait_bounds_compares_equal() {
        let make = || Generics {
            params: vec![type_param(
                "F",
                vec![
                    GenericBound::Outlives("'static".to_string()),
                    trait_bound("Send"),
                    trait_bound("Sync"),
                ],
            )],
            where_predicates: vec![],
        };
        assert!(
            generics_structurally_equal(&make(), &make()),
            "`F: 'static + Send + Sync` must compare equal to itself"
        );
    }

    /// Two identical `LifetimePredicate` where entries must compare unequal (D3 fail-closed).
    #[test]
    fn test_lifetime_predicate_is_fail_closed() {
        let make = || Generics {
            params: vec![],
            where_predicates: vec![WherePredicate::LifetimePredicate {
                lifetime: "'a".to_string(),
                outlives: vec!["'b".to_string()],
            }],
        };
        assert!(
            !generics_structurally_equal(&make(), &make()),
            "identical LifetimePredicates must still return false (D3 fail-closed)"
        );
    }

    /// D5 (ADR 2026-05-18-1223): Two identical HRTB-on-TraitBound entries with
    /// lifetime-only binders (`for<'a> Fn(&'a str)`) must now compare EQUAL.
    ///
    /// Rustdoc desugars elided-lifetime Fn bounds (`Fn(&str)`) into HRTB form
    /// (`for<'_> Fn(&'_ str)`), so both A-side and C-side should produce the same
    /// fingerprint when their logical Fn signatures are identical.
    ///
    /// This test replaces the former `test_hrtb_on_trait_bound_is_fail_closed` which
    /// protected the old D3 fail-closed behavior for HRTB-on-TraitBound.
    #[test]
    fn test_hrtb_on_trait_bound_lifetime_only_compares_equal() {
        let hrtb_bound = hrtb_fn_borrowed_str_bound("'a", vec![]);
        let make = || Generics {
            params: vec![type_param("F", vec![hrtb_bound.clone()])],
            where_predicates: vec![],
        };
        assert!(
            generics_structurally_equal(&make(), &make()),
            "identical HRTB-on-TraitBound with lifetime-only binder must compare equal \
             (D5: HRTB binder lifetime names normalized; for<'a> Fn(&'a str) == for<'a> Fn(&'a str))"
        );
    }

    /// D5 (ADR 2026-05-18-1223): HRTB-on-TraitBound with elision form (`'_`) must
    /// compare equal to HRTB-on-TraitBound with explicit form (`'a`).
    ///
    /// This models the A-side vs C-side asymmetry: A-side has `Fn(&str)` (no binder,
    /// `generic_params: []`), C-side has `for<'_> Fn(&'_ str)` (binder with `'_`).
    /// Both must produce the same fingerprint.
    #[test]
    fn test_hrtb_elision_vs_explicit_binder_compares_equal() {
        // A-side: no HRTB binder (as produced by A-codec from catalogue string `Fn(&str)`).
        let no_binder = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::Parenthesized {
                    inputs: vec![Type::BorrowedRef {
                        lifetime: None,
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    }],
                    output: None,
                })),
            },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        };
        // C-side: HRTB binder with `'_` (as rustdoc desugars `Fn(&str)`).
        let elided_binder = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::Parenthesized {
                    inputs: vec![Type::BorrowedRef {
                        lifetime: Some("'_".to_string()),
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    }],
                    output: None,
                })),
            },
            generic_params: vec![GenericParamDef {
                name: "'_".to_string(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }],
            modifier: TraitBoundModifier::None,
        };
        let a_generics =
            Generics { params: vec![type_param("F", vec![no_binder])], where_predicates: vec![] };
        let c_generics = Generics {
            params: vec![type_param("F", vec![elided_binder])],
            where_predicates: vec![],
        };
        assert!(
            generics_structurally_equal(&a_generics, &c_generics),
            "A-side Fn(&str) (no binder) must equal C-side for<'_> Fn(&'_ str) (elided binder) \
             (D5: HRTB lifetime binder normalized away; both produce same fingerprint)"
        );
    }

    /// D5 (ADR 2026-05-18-1223): HRTB-on-TraitBound with TYPE binders (`for<T: Foo>`)
    /// remains outside D5 scope and must still trigger fail-closed.
    #[test]
    fn test_hrtb_on_trait_bound_type_binder_is_fail_closed() {
        let hrtb_type_binder = GenericBound::TraitBound {
            trait_: Path { path: "Fn".to_string(), id: rustdoc_types::Id(0), args: None },
            generic_params: vec![GenericParamDef {
                name: "T".to_string(),
                kind: GenericParamDefKind::Type {
                    bounds: vec![],
                    default: None,
                    is_synthetic: false,
                },
            }],
            modifier: TraitBoundModifier::None,
        };
        let make = || Generics {
            params: vec![type_param("F", vec![hrtb_type_binder.clone()])],
            where_predicates: vec![],
        };
        assert!(
            !generics_structurally_equal(&make(), &make()),
            "HRTB-on-TraitBound with type binder must still be fail-closed (outside D5 scope)"
        );
    }

    /// Two identical HRTB binders on `BoundPredicate` (`for<'a> &'a T: Iterator`)
    /// must compare unequal (D3 fail-closed: BoundPredicate.generic_params non-empty).
    #[test]
    fn test_hrtb_binder_on_bound_predicate_is_fail_closed() {
        let make = || Generics {
            params: vec![type_param("T", vec![])],
            where_predicates: vec![WherePredicate::BoundPredicate {
                type_: Type::BorrowedRef {
                    lifetime: Some("'a".to_string()),
                    is_mutable: false,
                    type_: Box::new(Type::Generic("T".to_string())),
                },
                bounds: vec![trait_bound("Iterator")],
                generic_params: vec![GenericParamDef {
                    name: "'a".to_string(),
                    kind: GenericParamDefKind::Lifetime { outlives: vec![] },
                }],
            }],
        };
        assert!(
            !generics_structurally_equal(&make(), &make()),
            "identical HRTB binder on BoundPredicate must still return false (D3 fail-closed)"
        );
    }

    /// Two identical inline `Outlives` lifetime bounds (`<T: 'a>`) must compare equal.
    /// `Outlives` is within D3 scope and compared verbatim by lifetime string.
    /// Both the inline-param path and the where-predicate path go through
    /// different code branches in `build_where_form_view`, so both are tested.
    #[test]
    fn test_inline_outlives_bound_compares_equal() {
        let inline_only = || Generics {
            params: vec![type_param("T", vec![GenericBound::Outlives("'a".to_string())])],
            where_predicates: vec![],
        };
        assert!(
            generics_structurally_equal(&inline_only(), &inline_only()),
            "identical inline `<T: 'a>` must compare equal"
        );
    }

    /// Inline `<T: 'static>` and where-predicate `<T> where T: 'static` must compare equal
    /// (normalization: inline Outlives is lifted to where-form just like trait bounds).
    #[test]
    fn test_inline_outlives_equals_where_form_outlives() {
        let inline = Generics {
            params: vec![type_param("T", vec![GenericBound::Outlives("'static".to_string())])],
            where_predicates: vec![],
        };
        let where_form = Generics {
            params: vec![type_param("T", vec![])],
            where_predicates: vec![WherePredicate::BoundPredicate {
                type_: Type::Generic("T".to_string()),
                bounds: vec![GenericBound::Outlives("'static".to_string())],
                generic_params: vec![],
            }],
        };
        assert!(
            generics_structurally_equal(&inline, &where_form),
            "`<T: 'static>` must equal `<T> where T: 'static`"
        );
    }

    /// `GenericBound::Use(...)` inside a where predicate's bounds (e.g. `where T: use<U>`)
    /// must trigger fail-closed (D3: non-`TraitBound` variant).
    #[test]
    fn test_use_bound_in_where_predicate_is_fail_closed() {
        let use_bound =
            GenericBound::Use(vec![rustdoc_types::PreciseCapturingArg::Param("U".to_string())]);
        let make = || Generics {
            params: vec![type_param("T", vec![]), type_param("U", vec![])],
            where_predicates: vec![where_bound("T", vec![use_bound.clone()])],
        };
        assert!(
            !generics_structurally_equal(&make(), &make()),
            "identical `where T: use<U>` must still return false (D3 fail-closed)"
        );
    }

    /// Two identical `EqPredicate` where entries (`where T::Assoc = U`) must compare
    /// unequal (D3 fail-closed: EqPredicate is outside the supported scope).
    #[test]
    fn test_eq_predicate_is_fail_closed() {
        let make = || Generics {
            params: vec![type_param("T", vec![]), type_param("U", vec![])],
            where_predicates: vec![WherePredicate::EqPredicate {
                lhs: Type::QualifiedPath {
                    name: "Assoc".to_string(),
                    args: Some(Box::new(GenericArgs::AngleBracketed {
                        args: vec![],
                        constraints: vec![],
                    })),
                    self_type: Box::new(Type::Generic("T".to_string())),
                    trait_: None,
                },
                rhs: rustdoc_types::Term::Type(Type::Generic("U".to_string())),
            }],
        };
        assert!(
            !generics_structurally_equal(&make(), &make()),
            "identical EqPredicate must still return false (D3 fail-closed)"
        );
    }

    /// D5 (ADR 2026-05-18-1223): `where T: Iterator<Item: for<'a> Foo<&'a str>>` —
    /// an HRTB binder appears as a nested constraint bound inside an associated-type
    /// argument.  Since D5 relaxes `format_generic_bounds_with_canon` for
    /// lifetime-only HRTB binders, two identical such bounds compare equal.
    ///
    /// The binder lifetime is normalized away (D5: lifetime binders stripped from
    /// fingerprint), so `for<'a> Foo<&'a str>` and `Foo<&str>` produce the same
    /// fingerprint — which is the desired behavior for Fn trait desugaring symmetry.
    #[test]
    fn test_hrtb_inside_assoc_type_constraint_lifetime_only_compares_equal() {
        use rustdoc_types::{AssocItemConstraint, AssocItemConstraintKind, GenericArg};

        let hrtb_bound = GenericBound::TraitBound {
            trait_: Path {
                path: "Foo".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::AngleBracketed {
                    args: vec![GenericArg::Type(Type::BorrowedRef {
                        lifetime: Some("'a".to_string()),
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    })],
                    constraints: vec![],
                })),
            },
            // Lifetime-only HRTB binder — D5 normalizes this away.
            generic_params: vec![GenericParamDef {
                name: "'a".to_string(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }],
            modifier: TraitBoundModifier::None,
        };
        // `where T: Iterator<Item: for<'a> Foo<&'a str>>`
        let iterator_with_hrtb_item = GenericBound::TraitBound {
            trait_: Path {
                path: "Iterator".to_string(),
                id: rustdoc_types::Id(1),
                args: Some(Box::new(GenericArgs::AngleBracketed {
                    args: vec![],
                    constraints: vec![AssocItemConstraint {
                        name: "Item".to_string(),
                        args: Some(Box::new(GenericArgs::AngleBracketed {
                            args: vec![],
                            constraints: vec![],
                        })),
                        binding: AssocItemConstraintKind::Constraint(vec![hrtb_bound]),
                    }],
                })),
            },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        };
        let make = || Generics {
            params: vec![type_param("T", vec![])],
            where_predicates: vec![WherePredicate::BoundPredicate {
                type_: Type::Generic("T".to_string()),
                bounds: vec![iterator_with_hrtb_item.clone()],
                generic_params: vec![],
            }],
        };
        assert!(
            generics_structurally_equal(&make(), &make()),
            "D5: identical HRTB-on-TraitBound with lifetime-only binder inside assoc-type \
             constraint must compare equal (lifetime binder normalized away)"
        );
    }

    // --- Supertrait bounds: canon-aware comparison ---

    // Helper: a `GenericBound::TraitBound` whose path has one generic type argument.
    // Used to model `From<T>` / `From<u32>` / etc. in supertrait bounds.
    fn trait_bound_with_type_arg(trait_name: &str, arg: Type) -> GenericBound {
        GenericBound::TraitBound {
            trait_: Path {
                path: trait_name.to_string(),
                id: Id(0),
                args: Some(Box::new(GenericArgs::AngleBracketed {
                    args: vec![rustdoc_types::GenericArg::Type(arg)],
                    constraints: vec![],
                })),
            },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        }
    }

    // Helper: constructs a minimal `rustdoc_types::Trait` with the given generics and
    // supertrait bounds (no methods, no implementations).
    fn make_trait(generics: Generics, bounds: Vec<GenericBound>) -> Trait {
        Trait {
            is_auto: false,
            is_unsafe: false,
            is_dyn_compatible: true,
            items: vec![],
            generics,
            bounds,
            implementations: vec![],
        }
    }

    // Helper: empty item index (no methods to look up).
    fn empty_index() -> HashMap<Id, Item> {
        HashMap::new()
    }

    /// `trait A<T>: From<T>` and `trait A<U>: From<U>` must compare structurally
    /// equal — renaming the generic parameter in both the trait generics and the
    /// supertrait bound should be invisible to the canon-aware comparison.
    #[test]
    fn traits_with_renamed_supertrait_generic_param_compare_equal() {
        let trait_t = make_trait(
            Generics { params: vec![type_param("T", vec![])], where_predicates: vec![] },
            vec![trait_bound_with_type_arg("From", Type::Generic("T".to_string()))],
        );
        let trait_u = make_trait(
            Generics { params: vec![type_param("U", vec![])], where_predicates: vec![] },
            vec![trait_bound_with_type_arg("From", Type::Generic("U".to_string()))],
        );
        let idx = empty_index();
        assert!(
            traits_structurally_equal(&trait_t, &trait_u, &idx, &idx),
            "`trait A<T>: From<T>` must equal `trait A<U>: From<U>` (canon rename)"
        );
    }

    /// `trait A<T>: From<u32>` and `trait A<T>: From<u64>` must compare
    /// structurally unequal — concrete type arguments differ and are not
    /// affected by the generic parameter canon map.
    #[test]
    fn traits_with_different_supertrait_concrete_arg_compare_unequal() {
        let concrete_type = |name: &str| {
            Type::ResolvedPath(Path { path: name.to_string(), id: Id(999), args: None })
        };
        let trait_u32 = make_trait(
            Generics { params: vec![type_param("T", vec![])], where_predicates: vec![] },
            vec![trait_bound_with_type_arg("From", concrete_type("u32"))],
        );
        let trait_u64 = make_trait(
            Generics { params: vec![type_param("T", vec![])], where_predicates: vec![] },
            vec![trait_bound_with_type_arg("From", concrete_type("u64"))],
        );
        let idx = empty_index();
        assert!(
            !traits_structurally_equal(&trait_u32, &trait_u64, &idx, &idx),
            "`trait A<T>: From<u32>` must NOT equal `trait A<T>: From<u64>`"
        );
    }

    // ---------------------------------------------------------------------------
    // T003 (ADR 2026-05-18-1223 D1): strip_outlives_from_index removal + fingerprint
    // ---------------------------------------------------------------------------

    /// (a) T003: `T: 'static` Outlives bound retained on both A-side and C-side produces
    /// identical fingerprints via `build_generics_fingerprint_with_combined_canon`.
    ///
    /// Since `strip_outlives_from_index` has been removed (T003), both sides now retain
    /// `GenericBound::Outlives` bounds. When A-side (catalogue) and C-side (rustdoc)
    /// both declare `T: 'static`, they must produce the same fingerprint and compare Blue.
    #[test]
    fn test_t003_outlives_bound_retained_both_sides_produces_equal_fingerprint() {
        let static_bound = GenericBound::Outlives("'static".to_string());
        let generics_with_static = Generics {
            params: vec![type_param("T", vec![static_bound])],
            where_predicates: vec![],
        };
        // Build a simple (non-combined) canon map: T → #0.
        let mut canon = HashMap::new();
        canon.insert("T".to_string(), "#0".to_string());

        let fp_a = super::fn_eq::build_generics_fingerprint_with_combined_canon(
            &generics_with_static,
            &canon,
        );
        let fp_c = super::fn_eq::build_generics_fingerprint_with_combined_canon(
            &generics_with_static,
            &canon,
        );

        assert_eq!(
            fp_a, fp_c,
            "both-sides-retained `T: 'static` must produce identical fingerprints (T003)"
        );
        // The fingerprint must include the Outlives bound (not an empty string).
        assert!(
            !fp_a.is_empty(),
            "fingerprint for `T: 'static` must be non-empty (Outlives is retained)"
        );
    }

    /// (b) T003: `T: A + B` and `T: B + A` must produce the same fingerprint.
    ///
    /// `format_generic_bounds_with_canon` sorts rhs elements so that bound order is
    /// irrelevant. This test verifies the invariant holds when bounds are specified in
    /// opposite orders, ensuring `T: A + B` and `T: B + A` are treated identically.
    #[test]
    fn test_t003_bound_order_independent_fingerprint() {
        // `T: A + B` (A first, then B).
        let generics_ab = Generics {
            params: vec![type_param("T", vec![trait_bound("A"), trait_bound("B")])],
            where_predicates: vec![],
        };
        // `T: B + A` (B first, then A).
        let generics_ba = Generics {
            params: vec![type_param("T", vec![trait_bound("B"), trait_bound("A")])],
            where_predicates: vec![],
        };
        let mut canon = HashMap::new();
        canon.insert("T".to_string(), "#0".to_string());

        let fp_ab =
            super::fn_eq::build_generics_fingerprint_with_combined_canon(&generics_ab, &canon);
        let fp_ba =
            super::fn_eq::build_generics_fingerprint_with_combined_canon(&generics_ba, &canon);

        assert_eq!(
            fp_ab, fp_ba,
            "`T: A + B` and `T: B + A` must produce the same fingerprint (rhs is order-independent)"
        );
    }

    /// (c) T003 updated by D5 (ADR 2026-05-18-1223): HRTB-on-TraitBound with
    /// lifetime-only binders (`for<'a>`) is now SUPPORTED and two identical such
    /// bounds must compare equal.
    ///
    /// Previously (T003 pre-D5), this was fail-closed.  D5 changes `bounds_supported`
    /// to allow lifetime-only HRTB binders so that rustdoc's desugared Fn trait bounds
    /// (`for<'_> Fn(&'_ str)`) can compare symmetrically with the catalogue's
    /// `Fn(&str)` form (no binder).  Two identical HRTB-on-TraitBound forms with
    /// lifetime-only binders must now compare equal.
    #[test]
    fn test_t003_hrtb_lifetime_only_binder_is_now_supported_and_equal() {
        // Build a `GenericBound::TraitBound` with a lifetime-only HRTB binder — now
        // supported per D5 (`bounds_supported` relaxed).
        let hrtb_bound = GenericBound::TraitBound {
            trait_: Path { path: "Fn".to_string(), id: Id(0), args: None },
            generic_params: vec![GenericParamDef {
                name: "'a".to_string(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }],
            modifier: TraitBoundModifier::None,
        };
        let make_hrtb_generics = || Generics {
            params: vec![type_param("F", vec![hrtb_bound.clone()])],
            where_predicates: vec![],
        };
        // D5: two identical HRTB-on-TraitBound with lifetime-only binder must compare equal.
        assert!(
            generics_structurally_equal(&make_hrtb_generics(), &make_hrtb_generics()),
            "D5: HRTB-on-TraitBound with lifetime-only binder (`for<'a>`) is now supported; \
             two identical such bounds must compare equal (IN-19/AC-18)"
        );
    }

    /// D5 (ADR 2026-05-18-1223): HRTB-on-TraitBound with 2-lifetime binder
    /// (`for<'a,'b>`) must NOT compare equal to a 1-lifetime binder (`for<'a>`),
    /// because the binder arity differs and the fingerprint encodes a distinguishing
    /// `#L{n}:` prefix when n ≥ 2.  This prevents false Blue when both produce the
    /// same inner arg fingerprint (after lifetime-annotation stripping) but differ
    /// structurally in binder shape.
    #[test]
    fn test_hrtb_on_trait_bound_two_binder_lifetimes_not_equal_to_one() {
        let one_lifetime_binder = GenericBound::TraitBound {
            trait_: Path { path: "Fn".to_string(), id: Id(0), args: None },
            generic_params: vec![GenericParamDef {
                name: "'a".to_string(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }],
            modifier: TraitBoundModifier::None,
        };
        let two_lifetime_binder = GenericBound::TraitBound {
            trait_: Path { path: "Fn".to_string(), id: Id(0), args: None },
            generic_params: vec![
                GenericParamDef {
                    name: "'a".to_string(),
                    kind: GenericParamDefKind::Lifetime { outlives: vec![] },
                },
                GenericParamDef {
                    name: "'b".to_string(),
                    kind: GenericParamDefKind::Lifetime { outlives: vec![] },
                },
            ],
            modifier: TraitBoundModifier::None,
        };
        let one_binder_generics = Generics {
            params: vec![type_param("F", vec![one_lifetime_binder])],
            where_predicates: vec![],
        };
        let two_binder_generics = Generics {
            params: vec![type_param("F", vec![two_lifetime_binder])],
            where_predicates: vec![],
        };
        assert!(
            !generics_structurally_equal(&one_binder_generics, &two_binder_generics),
            "D5: HRTB-on-TraitBound with 1-lifetime binder must NOT equal 2-lifetime binder \
             (binder arity >= 2 adds a distinguishing #L{{n}} prefix to the fingerprint)"
        );
    }

    /// D5 correctness: `for<'a> Fn(&'static str)` must NOT compare equal to
    /// `for<'a> Fn(&'a str)`.
    ///
    /// A false Blue here would mean a real signature change (a function that
    /// accepts any-lifetime references vs one that only accepts `'static`
    /// references) is not detected by the evaluator.
    ///
    /// The fix: concrete non-binder lifetimes (`'static`) in BorrowedRef position
    /// are preserved verbatim even in the 1-binder context, while the binder
    /// lifetime (`'a`) is dropped (A-side ≡ C-side symmetry for binder references).
    #[test]
    fn test_hrtb_one_binder_concrete_lifetime_not_equal_to_binder_lifetime() {
        // `for<'a> Fn(&'static str)` — the reference carries `'static` (concrete,
        // non-binder), not the HRTB binder lifetime.
        let static_ref_bound = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::Parenthesized {
                    inputs: vec![Type::BorrowedRef {
                        lifetime: Some("'static".to_string()),
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    }],
                    output: None,
                })),
            },
            generic_params: vec![GenericParamDef {
                name: "'a".to_string(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }],
            modifier: TraitBoundModifier::None,
        };
        // `for<'a> Fn(&'a str)` — the reference carries the HRTB binder lifetime `'a`.
        let binder_ref_bound = hrtb_fn_borrowed_str_bound("'a", vec![]);
        let static_generics = Generics {
            params: vec![type_param("F", vec![static_ref_bound])],
            where_predicates: vec![],
        };
        let binder_generics = Generics {
            params: vec![type_param("F", vec![binder_ref_bound])],
            where_predicates: vec![],
        };
        assert!(
            !generics_structurally_equal(&static_generics, &binder_generics),
            "D5 correctness: `for<'a> Fn(&'static str)` must NOT equal `for<'a> Fn(&'a str)` \
             (concrete lifetime preserved verbatim; binder lifetime dropped for A/C symmetry)"
        );
    }

    /// No-binder case: `Fn(&'static str)` (A-side, concrete lifetime) must NOT compare
    /// equal to `Fn(&str)` (A-side, no lifetime annotation).
    ///
    /// Concrete named lifetimes in BorrowedRef position must be emitted verbatim even
    /// in the no-HRTB-binder context so that catalogue-declared `Fn(&'static str)` is
    /// distinguishable from `Fn(&str)`.
    ///
    /// This was a pre-existing correctness gap: the old Case 3 ("no HRTB context → drop
    /// lifetime") silently erased concrete lifetimes like `'static`, making these two
    /// distinct API signatures produce the same fingerprint.
    #[test]
    fn test_no_binder_static_lifetime_not_equal_to_no_lifetime() {
        // `Fn(&'static str)` — concrete `'static` lifetime in BorrowedRef, no HRTB binder.
        let static_ref_bound = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::Parenthesized {
                    inputs: vec![Type::BorrowedRef {
                        lifetime: Some("'static".to_string()),
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    }],
                    output: None,
                })),
            },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        };
        // `Fn(&str)` — no lifetime annotation in BorrowedRef, no HRTB binder.
        let no_lifetime_bound = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::Parenthesized {
                    inputs: vec![Type::BorrowedRef {
                        lifetime: None,
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    }],
                    output: None,
                })),
            },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        };
        let static_generics = Generics {
            params: vec![type_param("F", vec![static_ref_bound])],
            where_predicates: vec![],
        };
        let no_lt_generics = Generics {
            params: vec![type_param("F", vec![no_lifetime_bound])],
            where_predicates: vec![],
        };
        assert!(
            !generics_structurally_equal(&static_generics, &no_lt_generics),
            "no-binder `Fn(&'static str)` must NOT equal `Fn(&str)` \
             (concrete `'static` lifetime must be preserved verbatim)"
        );
    }

    /// D5 correctness: `for<'a> Foo<&'a str>` (1-binder, AngleBracketed) must NOT
    /// compare equal to `Foo<&str>` (no binder).
    ///
    /// The 1-binder drop (`@BR:binder_lt → ""`) applies ONLY for Fn-trait Parenthesized
    /// args (rustdoc desugaring of `Fn(&str)`).  For non-Fn AngleBracketed args the
    /// binder lifetime must be retained so that the presence of the HRTB binder is
    /// observable in the fingerprint.
    #[test]
    fn test_hrtb_one_binder_angle_bracketed_not_equal_to_no_binder() {
        // `for<'a> Foo<&'a str>` — AngleBracketed arg with binder lifetime, 1 binder.
        let hrtb_bound = GenericBound::TraitBound {
            trait_: Path {
                path: "Foo".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::AngleBracketed {
                    args: vec![rustdoc_types::GenericArg::Type(Type::BorrowedRef {
                        lifetime: Some("'a".to_string()),
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    })],
                    constraints: vec![],
                })),
            },
            generic_params: vec![GenericParamDef {
                name: "'a".to_string(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }],
            modifier: TraitBoundModifier::None,
        };
        // `Foo<&str>` — AngleBracketed arg with no lifetime, no binder.
        let no_binder_bound = GenericBound::TraitBound {
            trait_: Path {
                path: "Foo".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::AngleBracketed {
                    args: vec![rustdoc_types::GenericArg::Type(Type::BorrowedRef {
                        lifetime: None,
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    })],
                    constraints: vec![],
                })),
            },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        };
        let hrtb_generics =
            Generics { params: vec![type_param("F", vec![hrtb_bound])], where_predicates: vec![] };
        let no_binder_generics = Generics {
            params: vec![type_param("F", vec![no_binder_bound])],
            where_predicates: vec![],
        };
        assert!(
            !generics_structurally_equal(&hrtb_generics, &no_binder_generics),
            "D5 correctness: `for<'a> Foo<&'a str>` (AngleBracketed) must NOT equal \
             `Foo<&str>` (1-binder drop applies only to Fn-trait Parenthesized desugaring)"
        );
    }

    /// Non-HRTB lifetime alpha-rename: `fn f<'a>(&'a str)` must compare equal to
    /// `fn f<'b>(&'b str)`.
    ///
    /// Named lifetime params in function/method generics are alpha-equivalent —
    /// `generics_structurally_equal` ignores their names.  The BorrowedRef formatter
    /// must drop non-static named lifetimes in the non-HRTB context so that signatures
    /// differing only in lifetime param names produce the same fingerprint.
    #[test]
    fn test_non_hrtb_lifetime_alpha_rename_compares_equal() {
        // `F: Foo<&'a str>` with lifetime param `'a` on the outer generics (no HRTB binder
        // on the TraitBound).  The `'a` in `&'a str` is a named param, not an HRTB binder.
        let bound_a = GenericBound::TraitBound {
            trait_: Path {
                path: "Foo".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::AngleBracketed {
                    args: vec![rustdoc_types::GenericArg::Type(Type::BorrowedRef {
                        lifetime: Some("'a".to_string()),
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    })],
                    constraints: vec![],
                })),
            },
            generic_params: vec![], // no HRTB binder on the TraitBound
            modifier: TraitBoundModifier::None,
        };
        // Same but `'b` — alpha-rename of the lifetime param.
        let bound_b = GenericBound::TraitBound {
            trait_: Path {
                path: "Foo".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::AngleBracketed {
                    args: vec![rustdoc_types::GenericArg::Type(Type::BorrowedRef {
                        lifetime: Some("'b".to_string()),
                        is_mutable: false,
                        type_: Box::new(ty("str")),
                    })],
                    constraints: vec![],
                })),
            },
            generic_params: vec![], // no HRTB binder on the TraitBound
            modifier: TraitBoundModifier::None,
        };
        let gen_a = Generics {
            params: vec![
                GenericParamDef {
                    name: "'a".to_string(),
                    kind: GenericParamDefKind::Lifetime { outlives: vec![] },
                },
                type_param("F", vec![bound_a]),
            ],
            where_predicates: vec![],
        };
        let gen_b = Generics {
            params: vec![
                GenericParamDef {
                    name: "'b".to_string(),
                    kind: GenericParamDefKind::Lifetime { outlives: vec![] },
                },
                type_param("F", vec![bound_b]),
            ],
            where_predicates: vec![],
        };
        assert!(
            generics_structurally_equal(&gen_a, &gen_b),
            "non-HRTB lifetime alpha-rename: `F: Foo<&'a str>` must equal `F: Foo<&'b str>` \
             (named lifetime params are alpha-equivalent; only `'static` is preserved verbatim)"
        );
    }

    /// D5 correctness: `for<'a> Fn(&'a str, &'a str)` (1-binder, shared lifetime) must NOT
    /// compare equal to `Fn(&str, &str)` (no binder, independent lifetimes).
    ///
    /// The 1-binder Fn-desugaring drop is only correct when the binder lifetime appears at
    /// most once in the inputs.  When it appears more than once it expresses a shared-lifetime
    /// constraint that is semantically distinct from independent elision.
    #[test]
    fn test_hrtb_one_binder_shared_lifetime_not_equal_to_no_binder() {
        // `for<'a> Fn(&'a str, &'a str)` — shared binder lifetime, Parenthesized.
        let shared_lt_bound = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::Parenthesized {
                    inputs: vec![
                        Type::BorrowedRef {
                            lifetime: Some("'a".to_string()),
                            is_mutable: false,
                            type_: Box::new(ty("str")),
                        },
                        Type::BorrowedRef {
                            lifetime: Some("'a".to_string()),
                            is_mutable: false,
                            type_: Box::new(ty("str")),
                        },
                    ],
                    output: None,
                })),
            },
            generic_params: vec![GenericParamDef {
                name: "'a".to_string(),
                kind: GenericParamDefKind::Lifetime { outlives: vec![] },
            }],
            modifier: TraitBoundModifier::None,
        };
        // `Fn(&str, &str)` — no binder, independent lifetimes (both `lifetime: None`).
        let no_binder_bound = GenericBound::TraitBound {
            trait_: Path {
                path: "Fn".to_string(),
                id: rustdoc_types::Id(0),
                args: Some(Box::new(GenericArgs::Parenthesized {
                    inputs: vec![
                        Type::BorrowedRef {
                            lifetime: None,
                            is_mutable: false,
                            type_: Box::new(ty("str")),
                        },
                        Type::BorrowedRef {
                            lifetime: None,
                            is_mutable: false,
                            type_: Box::new(ty("str")),
                        },
                    ],
                    output: None,
                })),
            },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        };
        let shared_lt_generics = Generics {
            params: vec![type_param("F", vec![shared_lt_bound])],
            where_predicates: vec![],
        };
        let no_binder_generics = Generics {
            params: vec![type_param("F", vec![no_binder_bound])],
            where_predicates: vec![],
        };
        assert!(
            !generics_structurally_equal(&shared_lt_generics, &no_binder_generics),
            "D5 correctness: `for<'a> Fn(&'a str, &'a str)` must NOT equal `Fn(&str, &str)` \
             (shared lifetime is semantically distinct from independent elision)"
        );
    }

    /// D5 correctness: HRTB-on-TraitBound with a lifetime binder carrying `outlives`
    /// constraints (e.g. `for<'a: 'b>`) must be fail-closed.
    ///
    /// The formatter only records binder names/arity and does not encode `outlives`
    /// constraints.  Accepting such binders would silently discard the constraint and
    /// risk false Blue for `for<'a: 'b> Foo<&'a T>` vs `for<'a> Foo<&'a T>`.
    #[test]
    fn test_hrtb_lifetime_binder_with_outlives_is_fail_closed() {
        // `for<'a: 'static> Fn(&'a str)` — binder lifetime with `outlives: ['static]`.
        let outlives_bound = hrtb_fn_borrowed_str_bound("'a", vec!["'static".to_string()]);
        // `for<'a> Fn(&'a str)` — same but without `outlives` constraint.
        let plain_binder_bound = hrtb_fn_borrowed_str_bound("'a", vec![]);
        // Both sides with the outlives binder — should be fail-closed (not equal) because
        // the outlives constraint is outside the supported scope.
        let outlives_generics = Generics {
            params: vec![type_param("F", vec![outlives_bound.clone()])],
            where_predicates: vec![],
        };
        let plain_generics = Generics {
            params: vec![type_param("F", vec![plain_binder_bound])],
            where_predicates: vec![],
        };
        assert!(
            !generics_structurally_equal(&outlives_generics, &outlives_generics.clone()),
            "D5 fail-closed: HRTB binder with `outlives` constraints must not compare equal \
             (even to itself — the constraint is not represented in the fingerprint)"
        );
        assert!(
            !generics_structurally_equal(&outlives_generics, &plain_generics),
            "D5 fail-closed: `for<'a: 'static> Fn(&'a str)` must NOT equal `for<'a> Fn(&'a str)` \
             (outlives constraint is outside D5 scope)"
        );
    }

    // ---------------------------------------------------------------------------
    // impl Trait codec fix (ADR 2026-05-25-0423 D1): position-based identification
    // ---------------------------------------------------------------------------

    // Helper: a synthetic type param representing a rustdoc-desugared `impl Trait` arg.
    // `is_synthetic = true` and `name` is the bound string (e.g. `"impl Into<String>"`).
    fn synthetic_type_param(name: &str, bounds: Vec<GenericBound>) -> GenericParamDef {
        GenericParamDef {
            name: name.to_string(),
            kind: GenericParamDefKind::Type { bounds, default: None, is_synthetic: true },
        }
    }

    // Helper: construct a minimal default FunctionHeader (no qualifiers, Rust ABI).
    fn default_fn_header() -> rustdoc_types::FunctionHeader {
        rustdoc_types::FunctionHeader {
            is_async: false,
            is_const: false,
            is_unsafe: false,
            abi: rustdoc_types::Abi::Rust,
        }
    }

    // Helper: a `GenericBound::TraitBound` for a named trait with one angle-bracketed type arg.
    // Used to build bounds like `Into<String>`.
    fn trait_bound_with_generic_arg(trait_name: &str, arg_name: &str) -> GenericBound {
        GenericBound::TraitBound {
            trait_: Path {
                path: trait_name.to_string(),
                id: Id(0),
                args: Some(Box::new(GenericArgs::AngleBracketed {
                    args: vec![rustdoc_types::GenericArg::Type(Type::ResolvedPath(Path {
                        path: arg_name.to_string(),
                        id: Id(0),
                        args: None,
                    }))],
                    constraints: vec![],
                })),
            },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        }
    }

    /// (a) Single `impl Trait` argument: AC-02 / IN-04.
    ///
    /// A-side: `fn(a: impl Into<String>)` → `Type::ImplTrait([Into<String>])` in the sig,
    /// with empty generics (no synthetic params — the catalogue A-codec does not add them).
    /// C-side (rustdoc): `Type::Generic("impl Into<String>")` in the sig, with a synthetic
    /// generic param `"impl Into<String>"` in generics (is_synthetic = true).
    ///
    /// `fn_sigs_structurally_equal` must return `true` (Blue) for this pair.
    #[test]
    fn test_impl_trait_single_arg_a_side_impl_trait_vs_c_side_generic_equal() {
        let into_string_bound = trait_bound_with_generic_arg("Into", "String");
        // A-side: ImplTrait in the param type, empty generics.
        let a_sig = rustdoc_types::FunctionSignature {
            inputs: vec![("a".to_string(), Type::ImplTrait(vec![into_string_bound.clone()]))],
            output: None,
            is_c_variadic: false,
        };
        let a_generics = Generics { params: vec![], where_predicates: vec![] };
        // C-side: Generic("impl Into<String>") in the param type, synthetic param in generics.
        let c_sig = rustdoc_types::FunctionSignature {
            inputs: vec![(
                "impl_into_string".to_string(),
                Type::Generic("impl Into<String>".to_string()),
            )],
            output: None,
            is_c_variadic: false,
        };
        let c_generics = Generics {
            params: vec![synthetic_type_param(
                "impl Into<String>",
                vec![into_string_bound.clone()],
            )],
            where_predicates: vec![],
        };
        let hdr = default_fn_header();
        assert!(
            super::fn_sigs_structurally_equal(&a_sig, &c_sig, &hdr, &hdr, &a_generics, &c_generics),
            "single `impl Trait` arg: A-side ImplTrait and C-side Generic must be structurally \
             equal (AC-02: a-side impl Into<String> == c-side #0)"
        );
    }

    /// (b) Duplicate same-named `impl Trait` arguments: AC-01 / IN-04.
    ///
    /// `fn(a: impl Into<String>, b: impl Into<String>)`:
    /// - A-side: two `Type::ImplTrait` params with the same bounds, empty generics.
    /// - C-side: two `Type::Generic("impl Into<String>")` params, two synthetic params in
    ///   generics (both named `"impl Into<String>"`, is_synthetic = true).
    ///
    /// Before the fix, the HashMap collision caused the second synthetic param to overwrite
    /// the first, making C-side map both to the same placeholder and compare unequal to
    /// A-side.  After the fix, each occurrence maps to a distinct placeholder (#0, #1).
    #[test]
    fn test_impl_trait_duplicate_same_named_args_equal() {
        let into_string_bound = trait_bound_with_generic_arg("Into", "String");
        // A-side: two ImplTrait params, empty generics.
        let a_sig = rustdoc_types::FunctionSignature {
            inputs: vec![
                ("a".to_string(), Type::ImplTrait(vec![into_string_bound.clone()])),
                ("b".to_string(), Type::ImplTrait(vec![into_string_bound.clone()])),
            ],
            output: None,
            is_c_variadic: false,
        };
        let a_generics = Generics { params: vec![], where_predicates: vec![] };
        // C-side: two Generic("impl Into<String>") params, two synthetic params in generics.
        let c_sig = rustdoc_types::FunctionSignature {
            inputs: vec![
                ("param0".to_string(), Type::Generic("impl Into<String>".to_string())),
                ("param1".to_string(), Type::Generic("impl Into<String>".to_string())),
            ],
            output: None,
            is_c_variadic: false,
        };
        let c_generics = Generics {
            params: vec![
                synthetic_type_param("impl Into<String>", vec![into_string_bound.clone()]),
                synthetic_type_param("impl Into<String>", vec![into_string_bound.clone()]),
            ],
            where_predicates: vec![],
        };
        let hdr = default_fn_header();
        assert!(
            super::fn_sigs_structurally_equal(&a_sig, &c_sig, &hdr, &hdr, &a_generics, &c_generics),
            "duplicate same-named `impl Trait` args: A-side and C-side must be structurally equal \
             (AC-01: two impl Into<String> map to #0 and #1 respectively, not both to #0)"
        );
    }

    /// (c) Parameter-order distinction test: AC-03 / CN-02.
    ///
    /// `fn(a: impl Display, b: impl Into<String>)` vs
    /// `fn(a: impl Into<String>, b: impl Display)` must be structurally NOT equal.
    ///
    /// Position-based identification must preserve param order sensitivity.
    #[test]
    fn test_impl_trait_different_order_is_not_equal() {
        let display_bound = trait_bound("Display");
        let into_string_bound = trait_bound_with_generic_arg("Into", "String");
        // Signature 1: (Display, Into<String>)
        let sig_display_first = rustdoc_types::FunctionSignature {
            inputs: vec![
                ("a".to_string(), Type::ImplTrait(vec![display_bound.clone()])),
                ("b".to_string(), Type::ImplTrait(vec![into_string_bound.clone()])),
            ],
            output: None,
            is_c_variadic: false,
        };
        // Signature 2: (Into<String>, Display) — reversed order
        let sig_into_first = rustdoc_types::FunctionSignature {
            inputs: vec![
                ("a".to_string(), Type::ImplTrait(vec![into_string_bound.clone()])),
                ("b".to_string(), Type::ImplTrait(vec![display_bound.clone()])),
            ],
            output: None,
            is_c_variadic: false,
        };
        // Both sides use empty generics (A-side only, no synthetic params).
        let empty_generics = Generics { params: vec![], where_predicates: vec![] };
        let hdr = default_fn_header();
        assert!(
            !super::fn_sigs_structurally_equal(
                &sig_display_first,
                &sig_into_first,
                &hdr,
                &hdr,
                &empty_generics,
                &empty_generics
            ),
            "param-order distinction: fn(Display, Into<String>) must NOT equal \
             fn(Into<String>, Display) (AC-03: position-based identification respects order)"
        );
    }

    #[test]
    fn test_impl_trait_different_order_a_side_vs_c_side_is_not_equal() {
        let display_bound = trait_bound("Display");
        let into_string_bound = trait_bound_with_generic_arg("Into", "String");
        let a_sig = rustdoc_types::FunctionSignature {
            inputs: vec![
                ("a".to_string(), Type::ImplTrait(vec![display_bound.clone()])),
                ("b".to_string(), Type::ImplTrait(vec![into_string_bound.clone()])),
            ],
            output: None,
            is_c_variadic: false,
        };
        let a_generics = Generics { params: vec![], where_predicates: vec![] };
        let c_sig = rustdoc_types::FunctionSignature {
            inputs: vec![
                ("param0".to_string(), Type::Generic("impl Into<String>".to_string())),
                ("param1".to_string(), Type::Generic("impl Display".to_string())),
            ],
            output: None,
            is_c_variadic: false,
        };
        let c_generics = Generics {
            params: vec![
                synthetic_type_param("impl Into<String>", vec![into_string_bound.clone()]),
                synthetic_type_param("impl Display", vec![display_bound.clone()]),
            ],
            where_predicates: vec![],
        };
        let hdr = default_fn_header();
        assert!(
            !super::fn_sigs_structurally_equal(
                &a_sig,
                &c_sig,
                &hdr,
                &hdr,
                &a_generics,
                &c_generics
            ),
            "A/C order distinction: positional impl Trait keys must still include bound identity"
        );
    }

    /// (d) Regression: existing generic param tests still pass.
    ///
    /// `fn f<T>(x: T)` vs `fn f<U>(x: U)` must still compare equal (name rename invisible).
    /// This verifies that the position-based fix for synthetic impl Trait params does not
    /// regress ordinary named generic param comparisons (AC-04).
    #[test]
    fn test_normal_generic_param_rename_still_equal_regression() {
        let type_t = Type::Generic("T".to_string());
        let type_u = Type::Generic("U".to_string());
        // A-side: fn(x: T) with <T>
        let sig_t = rustdoc_types::FunctionSignature {
            inputs: vec![("x".to_string(), type_t)],
            output: None,
            is_c_variadic: false,
        };
        let gen_t = Generics { params: vec![type_param("T", vec![])], where_predicates: vec![] };
        // B-side: fn(x: U) with <U>
        let sig_u = rustdoc_types::FunctionSignature {
            inputs: vec![("x".to_string(), type_u)],
            output: None,
            is_c_variadic: false,
        };
        let gen_u = Generics { params: vec![type_param("U", vec![])], where_predicates: vec![] };
        let hdr = default_fn_header();
        assert!(
            super::fn_sigs_structurally_equal(&sig_t, &sig_u, &hdr, &hdr, &gen_t, &gen_u),
            "regression (AC-04): fn<T>(T) and fn<U>(U) must still compare equal after \
             impl Trait position-based fix (normal generic param rename is transparent)"
        );
    }

    /// (e) Trait/impl-method path test via `build_trait_method_map`: IN-03 / IN-04.
    ///
    /// Constructs a trait with a method that has an `impl Into<String>` parameter.
    /// A-side: `fn new(a: impl Into<String>)` with empty method generics.
    /// C-side: `fn new(param0: impl Into<String>)` with a synthetic param in method generics.
    /// Both sides should produce equal method map entries, confirming that the position-based
    /// fix propagates correctly through the `build_combined_canon_map` / `format_type_with_canon_occ`
    /// chain used by `build_trait_method_map`.
    #[test]
    fn test_build_trait_method_map_impl_trait_method_equal() {
        use rustdoc_types::ItemEnum;
        let into_string_bound = trait_bound_with_generic_arg("Into", "String");
        // A-side method: fn new(a: impl Into<String>) — ImplTrait in sig, empty generics.
        let a_fn = rustdoc_types::Function {
            sig: rustdoc_types::FunctionSignature {
                inputs: vec![("a".to_string(), Type::ImplTrait(vec![into_string_bound.clone()]))],
                output: None,
                is_c_variadic: false,
            },
            generics: Generics { params: vec![], where_predicates: vec![] },
            header: default_fn_header(),
            has_body: true,
        };
        // C-side method: fn new(param0: Generic("impl Into<String>")) — synthetic param in generics.
        let c_fn = rustdoc_types::Function {
            sig: rustdoc_types::FunctionSignature {
                inputs: vec![(
                    "param0".to_string(),
                    Type::Generic("impl Into<String>".to_string()),
                )],
                output: None,
                is_c_variadic: false,
            },
            generics: Generics {
                params: vec![synthetic_type_param(
                    "impl Into<String>",
                    vec![into_string_bound.clone()],
                )],
                where_predicates: vec![],
            },
            header: default_fn_header(),
            has_body: true,
        };
        let method_id_a = Id(100);
        let method_id_c = Id(200);
        let a_item = Item {
            id: method_id_a,
            crate_id: 0,
            name: Some("new".to_string()),
            span: None,
            visibility: rustdoc_types::Visibility::Public,
            docs: None,
            links: std::collections::HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner: ItemEnum::Function(a_fn),
        };
        let c_item = Item {
            id: method_id_c,
            crate_id: 0,
            name: Some("new".to_string()),
            span: None,
            visibility: rustdoc_types::Visibility::Public,
            docs: None,
            links: std::collections::HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner: ItemEnum::Function(c_fn),
        };
        let mut a_index: HashMap<Id, Item> = HashMap::new();
        a_index.insert(method_id_a, a_item);
        let mut c_index: HashMap<Id, Item> = HashMap::new();
        c_index.insert(method_id_c, c_item);
        let (a_map, a_unsupported) = super::build_trait_method_map(&[method_id_a], &a_index, None);
        let (c_map, c_unsupported) = super::build_trait_method_map(&[method_id_c], &c_index, None);
        assert!(!a_unsupported, "A-side method map must not have unsupported generics");
        assert!(!c_unsupported, "C-side method map must not have unsupported generics");
        assert_eq!(
            a_map, c_map,
            "build_trait_method_map: A-side fn new(impl Into<String>) and C-side must produce \
             equal method map entries (IN-03 / IN-04: position-based fix propagates through \
             build_combined_canon_map + format_type_with_canon_occ chain)"
        );
    }
}
