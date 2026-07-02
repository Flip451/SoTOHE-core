//! Syn-based implementation of
//! [`domain::tddd::primitive_occurrence_scanner::PrimitiveOccurrenceScanner`]
//! (ADR `2026-07-01-0004` D2/D3; spec AC-02/AC-03/AC-04, IN-02/IN-03/IN-05).
//!
//! Reuses the existing `type_ref_parser` syn-parsing machinery (CN-01) to
//! parse a catalogue [`TypeRef`] string into a bare `syn::Type`, then performs
//! a full recursive [`syn::visit::Visit`] walk with no per-position exclusion
//! (spec OS-09) to find every ident matching a caller-requested primitive
//! name, at every position: named struct fields, enum variant fields, method
//! or function params/returns, bounds, `type_alias` targets, and nested
//! inside transparent containers (`Option`, `Vec`, `Box`, `Arc`, `BTreeMap`,
//! custom generics, tuples, references, arrays, slices) and function-type
//! signatures (`Fn` / `FnMut` / `FnOnce`).
//!
//! [`PrimitiveOccurrencePosition::Bound`] is the one exception to the
//! bare-`syn::Type` parse: a catalogue bound slot's text may be a legal
//! `syn::TypeParamBound` only (`?Sized`, a lifetime such as `'static`, or a
//! `for<'a> Trait<'a>` HRTB form) that `syn::parse_str::<syn::Type>` rejects,
//! so a `Bound`-position scan parses `type_ref` as a `syn::TypeParamBound`
//! instead (see the internal `scan_bound` helper; PR #179 round 2 P1).
//!
//! ident matching is exact (no substring matches) -- `NonEmptyString` /
//! `OsString` never false-positive against a requested `String` primitive.

use std::collections::{BTreeMap, BTreeSet};

use domain::tddd::catalogue_v2::{NonEmptyVec, TypeRef};
use domain::tddd::primitive_occurrence_scanner::{
    PrimitiveName, PrimitiveOccurrencePosition, PrimitiveOccurrenceReport,
    PrimitiveOccurrenceScanError, PrimitiveOccurrenceScanner,
};
use syn::visit::Visit;

use super::type_ref_parser::{parse_syn_type, parse_syn_type_param_bound};

// ---------------------------------------------------------------------------
// SynPrimitiveOccurrenceScanner — secondary adapter
// ---------------------------------------------------------------------------

/// Syn-based [`PrimitiveOccurrenceScanner`] adapter.
///
/// Stateless: holds no fields, so [`Default`] is the only constructor needed.
#[derive(Debug, Default)]
pub struct SynPrimitiveOccurrenceScanner;

impl PrimitiveOccurrenceScanner for SynPrimitiveOccurrenceScanner {
    /// Parses `type_ref` into a `syn::Type` (reusing `type_ref_parser`, CN-01)
    /// and walks it fully recursively, labelling each occurrence of a
    /// requested primitive name with the [`PrimitiveOccurrencePosition`] it
    /// was found at.
    ///
    /// When `position` is [`PrimitiveOccurrencePosition::Bound`], `type_ref`
    /// is instead parsed as a `syn::TypeParamBound` (via the internal
    /// `scan_bound` helper) since a catalogue bound slot's text may be a legal bound (`?Sized`,
    /// `'static`, `for<'a> Fn(&'a str)`) that is not parseable as a bare
    /// `syn::Type`.
    ///
    /// # Errors
    ///
    /// Returns [`PrimitiveOccurrenceScanError::InvalidSitePosition`] if
    /// `position` is [`PrimitiveOccurrencePosition::ResultErr`] (scan-intrinsic
    /// only, never a valid caller-supplied call site).
    ///
    /// Returns [`PrimitiveOccurrenceScanError::ParseFailure`] if `type_ref`
    /// cannot be parsed as a `syn::Type` (or, at `Bound` position, as a
    /// `syn::TypeParamBound`).
    fn scan(
        &self,
        type_ref: TypeRef,
        primitives: NonEmptyVec<PrimitiveName>,
        position: PrimitiveOccurrencePosition,
    ) -> Result<PrimitiveOccurrenceReport, PrimitiveOccurrenceScanError> {
        if position == PrimitiveOccurrencePosition::ResultErr {
            return Err(PrimitiveOccurrenceScanError::InvalidSitePosition { position });
        }

        if position == PrimitiveOccurrencePosition::Bound {
            return scan_bound(type_ref, primitives, position);
        }

        let syn_type = match parse_syn_type(type_ref.as_str()) {
            Ok(ty) => ty,
            Err(_) => return Err(PrimitiveOccurrenceScanError::ParseFailure { type_ref }),
        };

        let mut visitor = OccurrenceVisitor::new(primitives.as_slice(), position);
        visitor.visit_type(&syn_type);

        Ok(visitor.into_report())
    }
}

// ---------------------------------------------------------------------------
// scan_bound — Bound-position scan (syn::TypeParamBound, not syn::Type)
// ---------------------------------------------------------------------------

/// Scans a `Bound`-position `type_ref` for primitive occurrences.
///
/// A catalogue bound slot's text may be a legal `syn::TypeParamBound`
/// (`?Sized`, `'static`, `for<'a> Fn(&'a str)`) that
/// `syn::parse_str::<syn::Type>` rejects -- unlike every other position,
/// which is always scanned as a `syn::Type` (a struct field, param, return,
/// or type-alias target grammatically can never take bound-only syntax).
/// `type_ref` is therefore parsed as a `syn::TypeParamBound` first:
///
/// - [`syn::TypeParamBound::Lifetime`] (`'static`, `'a`) can never carry a
///   primitive occurrence -- returns an empty report without attempting a
///   `syn::Type` parse.
/// - [`syn::TypeParamBound::Trait`] walks the bound's `path` with the same
///   recursive [`Visit`] used for every other position (reusing
///   `OccurrenceVisitor::visit_path`), so nested primitive occurrences (e.g.
///   the `String` in `Into<Result<(), String>>`, or a `Param` occurrence
///   inside a `Fn(String) -> ()` callable-trait bound) are still found and
///   reclassified exactly as they would be from a `syn::Type` walk.
/// - Any other parsed form (e.g. a `Verbatim` token stream from unsupported
///   future bound syntax such as Rust 2024 precise-capture `use<..>` bounds)
///   is a [`PrimitiveOccurrenceScanError::ParseFailure`], matching
///   `parse_generic_bound`'s treatment of the same forms.
///
/// # Errors
///
/// Returns [`PrimitiveOccurrenceScanError::ParseFailure`] if `type_ref`
/// cannot be parsed as a `syn::TypeParamBound`, or parses as a form other
/// than [`syn::TypeParamBound::Lifetime`] / [`syn::TypeParamBound::Trait`].
fn scan_bound(
    type_ref: TypeRef,
    primitives: NonEmptyVec<PrimitiveName>,
    position: PrimitiveOccurrencePosition,
) -> Result<PrimitiveOccurrenceReport, PrimitiveOccurrenceScanError> {
    match parse_syn_type_param_bound(type_ref.as_str()) {
        Ok(syn::TypeParamBound::Lifetime(_)) => Ok(PrimitiveOccurrenceReport::new(BTreeMap::new())),
        Ok(syn::TypeParamBound::Trait(trait_bound)) => {
            let mut visitor = OccurrenceVisitor::new(primitives.as_slice(), position);
            visitor.visit_path(&trait_bound.path);
            Ok(visitor.into_report())
        }
        _ => Err(PrimitiveOccurrenceScanError::ParseFailure { type_ref }),
    }
}

// ---------------------------------------------------------------------------
// OccurrenceVisitor — full recursive syn::visit::Visit walker
// ---------------------------------------------------------------------------

/// Recursive `syn::Type` walker that tracks the current
/// [`PrimitiveOccurrencePosition`] attribution and accumulates primitive-name
/// hits per position.
///
/// The walk is fully recursive with no position exclusions (ADR
/// `2026-07-01-0004` D2, spec OS-09): every nested type-argument position is
/// visited. Three reclassification scopes reassign the *current* attribution
/// for their nested subtree only, restoring the prior attribution on exit:
///
/// - The second (`Err`) type argument of a two-type-argument `Result` path
///   segment -> [`PrimitiveOccurrencePosition::ResultErr`], regardless of
///   nesting depth or the ambient attribution.
/// - A parenthesized callable-type signature's params / return (`Fn(A, B) ->
///   R`, `FnMut(..)`, `FnOnce(..)`, including the same shape inside a
///   `dyn`/`impl` trait bound) -> [`PrimitiveOccurrencePosition::Param`] /
///   [`PrimitiveOccurrencePosition::Return`] respectively.
/// - Every bound of a `dyn Trait + Bound` / `impl Trait + Bound` type (the
///   trait head and each additional `+ Bound`) ->
///   [`PrimitiveOccurrencePosition::Bound`].
///
/// All other nested positions (`Option<_>`, `Vec<_>`, `Box<_>`,
/// `BTreeMap<_, _>`, `Rc<_>`, `Arc<_>`, custom generics, tuple elements,
/// references, pointers, arrays, slices, the `Ok` slot of `Result`) collapse
/// transparently to the current attribution.
struct OccurrenceVisitor<'p> {
    primitives: &'p [PrimitiveName],
    current: PrimitiveOccurrencePosition,
    hits: BTreeMap<PrimitiveOccurrencePosition, BTreeSet<PrimitiveName>>,
}

impl<'p> OccurrenceVisitor<'p> {
    fn new(primitives: &'p [PrimitiveName], position: PrimitiveOccurrencePosition) -> Self {
        Self { primitives, current: position, hits: BTreeMap::new() }
    }

    /// Consumes the visitor, returning only the positions that recorded at
    /// least one hit (a plain [`BTreeMap`] never holds empty-valued entries
    /// since [`Self::record`] only touches the map on an actual match).
    fn into_report(self) -> PrimitiveOccurrenceReport {
        PrimitiveOccurrenceReport::new(self.hits)
    }

    /// Records a hit at the current attribution if `name` exactly matches one
    /// of the requested primitive names.
    fn record(&mut self, name: &str) {
        if let Some(matched) = self.primitives.iter().find(|p| p.as_str() == name) {
            self.hits.entry(self.current).or_default().insert(matched.clone());
        }
    }

    /// Processes one path segment's generic arguments.
    ///
    /// Applies the `Result` Err-slot and parenthesized-callable-signature
    /// reclassification rules where the segment shape matches; otherwise
    /// recurses transparently (unchanged attribution) into any generic
    /// arguments the segment carries.
    fn visit_segment_arguments(&mut self, segment: &syn::PathSegment) {
        match &segment.arguments {
            syn::PathArguments::None => {}
            // `Fn(A, B) -> C` / `FnMut(..)` / `FnOnce(..)` shape -- this
            // parenthesized-argument syntax is exclusive to the callable-type
            // sugar, so detecting it structurally (rather than matching the
            // segment ident) uniformly covers plain `Fn(..)` paths and the
            // identical shape nested inside a `dyn`/`impl` trait bound.
            syn::PathArguments::Parenthesized(paren) => {
                let saved = self.current;
                self.current = PrimitiveOccurrencePosition::Param;
                for input in &paren.inputs {
                    self.visit_type(input);
                }
                if let syn::ReturnType::Type(_, ret_ty) = &paren.output {
                    self.current = PrimitiveOccurrencePosition::Return;
                    self.visit_type(ret_ty);
                }
                self.current = saved;
            }
            syn::PathArguments::AngleBracketed(angle) => {
                let type_args: Vec<&syn::Type> = angle
                    .args
                    .iter()
                    .filter_map(|arg| match arg {
                        syn::GenericArgument::Type(ty) => Some(ty),
                        _ => None,
                    })
                    .collect();

                // Exactly two *type* arguments on a segment literally named
                // `Result` -- the Err slot (2nd) is reclassified regardless of
                // nesting depth (spec IN-03/AC-03); the Ok slot (1st) is
                // transparent. A two-type-argument segment named anything else
                // (e.g. `BTreeMap<K, V>`) is not special-cased and falls
                // through to plain transparent recursion below.
                //
                // Slice-pattern destructuring (rather than `type_args[0]` /
                // `type_args[1]`) avoids `clippy::indexing_slicing` while
                // still statically requiring exactly two elements.
                match (segment.ident == "Result", type_args.as_slice()) {
                    (true, [ok_ty, err_ty]) => {
                        self.visit_type(ok_ty);
                        let saved = self.current;
                        self.current = PrimitiveOccurrencePosition::ResultErr;
                        self.visit_type(err_ty);
                        self.current = saved;
                    }
                    _ => {
                        for arg in &angle.args {
                            self.visit_generic_argument(arg);
                        }
                    }
                }
            }
        }
    }
}

impl<'p, 'ast> Visit<'ast> for OccurrenceVisitor<'p> {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        // Primitive-name match: exact equality against the LAST path
        // segment's ident only (e.g. `std::string::String` matches on
        // `String`, never on the `std` or `string` qualifying segments).
        if let Some(last) = path.segments.last() {
            self.record(&last.ident.to_string());
        }

        // Walk every segment's own generic arguments (Result / callable-type
        // reclassification, or transparent recursion for anything else).
        for segment in &path.segments {
            self.visit_segment_arguments(segment);
        }
    }

    fn visit_type_trait_object(&mut self, node: &'ast syn::TypeTraitObject) {
        let saved = self.current;
        self.current = PrimitiveOccurrencePosition::Bound;
        for bound in &node.bounds {
            self.visit_type_param_bound(bound);
        }
        self.current = saved;
    }

    fn visit_type_impl_trait(&mut self, node: &'ast syn::TypeImplTrait) {
        let saved = self.current;
        self.current = PrimitiveOccurrencePosition::Bound;
        for bound in &node.bounds {
            self.visit_type_param_bound(bound);
        }
        self.current = saved;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use domain::tddd::catalogue_v2::{NonEmptyVec, TypeRef};
    use domain::tddd::primitive_occurrence_scanner::{
        PrimitiveName, PrimitiveOccurrencePosition, PrimitiveOccurrenceScanError,
        PrimitiveOccurrenceScanner,
    };

    use super::SynPrimitiveOccurrenceScanner;

    fn primitive(name: &str) -> PrimitiveName {
        PrimitiveName::new(name).expect("valid primitive identifier")
    }

    fn one(name: &str) -> NonEmptyVec<PrimitiveName> {
        NonEmptyVec::new(primitive(name), vec![])
    }

    fn many(names: &[&str]) -> NonEmptyVec<PrimitiveName> {
        let mut iter = names.iter();
        let first = primitive(iter.next().expect("at least one name"));
        let rest = iter.map(|n| primitive(n)).collect();
        NonEmptyVec::new(first, rest)
    }

    fn type_ref(s: &str) -> TypeRef {
        TypeRef::new(s).expect("valid type_ref string")
    }

    fn scan(
        type_ref_str: &str,
        primitives: NonEmptyVec<PrimitiveName>,
        position: PrimitiveOccurrencePosition,
    ) -> Result<
        BTreeMap<PrimitiveOccurrencePosition, BTreeSet<PrimitiveName>>,
        PrimitiveOccurrenceScanError,
    > {
        let scanner = SynPrimitiveOccurrenceScanner;
        scanner
            .scan(type_ref(type_ref_str), primitives, position)
            .map(|report| report.by_position().clone())
    }

    fn expect(
        pairs: Vec<(PrimitiveOccurrencePosition, Vec<&str>)>,
    ) -> BTreeMap<PrimitiveOccurrencePosition, BTreeSet<PrimitiveName>> {
        pairs
            .into_iter()
            .map(|(pos, names)| (pos, names.into_iter().map(primitive).collect::<BTreeSet<_>>()))
            .collect()
    }

    // -- Required tests (verbatim from the T004 briefing) ------------------

    #[test]
    fn bare_primitive_at_named_field() {
        let result =
            scan("String", one("String"), PrimitiveOccurrencePosition::NamedField).unwrap();
        assert_eq!(result, expect(vec![(PrimitiveOccurrencePosition::NamedField, vec!["String"])]));
    }

    #[test]
    fn option_wrapped_primitive_collapses_transparently() {
        let result =
            scan("Option<String>", one("String"), PrimitiveOccurrencePosition::NamedField).unwrap();
        assert_eq!(result, expect(vec![(PrimitiveOccurrencePosition::NamedField, vec!["String"])]));
    }

    #[test]
    fn result_err_slot_reclassified_regardless_of_caller_position() {
        let result =
            scan("Result<u32, String>", one("String"), PrimitiveOccurrencePosition::Return)
                .unwrap();
        assert_eq!(result, expect(vec![(PrimitiveOccurrencePosition::ResultErr, vec!["String"])]));
    }

    #[test]
    fn boxed_dyn_fn_param_and_result_err_both_reclassified() {
        let result = scan(
            "Box<dyn Fn(String) -> Result<u32, String>>",
            one("String"),
            PrimitiveOccurrencePosition::NamedField,
        )
        .unwrap();
        assert_eq!(
            result,
            expect(vec![
                (PrimitiveOccurrencePosition::Param, vec!["String"]),
                (PrimitiveOccurrencePosition::ResultErr, vec!["String"]),
            ])
        );
    }

    #[test]
    fn bare_primitive_at_type_alias_target() {
        let result =
            scan("String", one("String"), PrimitiveOccurrencePosition::TypeAliasTarget).unwrap();
        assert_eq!(
            result,
            expect(vec![(PrimitiveOccurrencePosition::TypeAliasTarget, vec!["String"])])
        );
    }

    #[test]
    fn result_err_as_caller_position_is_rejected() {
        let scanner = SynPrimitiveOccurrenceScanner;
        let err = scanner
            .scan(type_ref("String"), one("String"), PrimitiveOccurrencePosition::ResultErr)
            .unwrap_err();
        assert_eq!(
            err,
            PrimitiveOccurrenceScanError::InvalidSitePosition {
                position: PrimitiveOccurrencePosition::ResultErr
            }
        );
    }

    #[test]
    fn non_primitive_type_yields_empty_report() {
        let result =
            scan("MyType", one("String"), PrimitiveOccurrencePosition::NamedField).unwrap();
        assert_eq!(result, BTreeMap::new());
    }

    #[test]
    fn unparseable_type_ref_is_parse_failure() {
        let scanner = SynPrimitiveOccurrenceScanner;
        let err = scanner
            .scan(type_ref("Vec<String"), one("String"), PrimitiveOccurrencePosition::NamedField)
            .unwrap_err();
        assert_eq!(
            err,
            PrimitiveOccurrenceScanError::ParseFailure { type_ref: type_ref("Vec<String") }
        );
    }

    // -- Additional coverage --------------------------------------------

    #[test]
    fn exact_ident_match_excludes_substring_types() {
        // `OsString` must never false-positive against a requested `String`
        // primitive (spec IN-04/AC-03: exact ident match only).
        let result =
            scan("OsString", one("String"), PrimitiveOccurrencePosition::NamedField).unwrap();
        assert_eq!(result, BTreeMap::new());
    }

    #[test]
    fn dyn_trait_head_and_extra_bound_reclassified_to_bound() {
        let result =
            scan("Box<dyn String + Send>", one("String"), PrimitiveOccurrencePosition::NamedField)
                .unwrap();
        assert_eq!(result, expect(vec![(PrimitiveOccurrencePosition::Bound, vec!["String"])]));
    }

    #[test]
    fn two_type_arg_container_other_than_result_is_not_reclassified() {
        let result =
            scan("BTreeMap<String, u32>", one("String"), PrimitiveOccurrencePosition::NamedField)
                .unwrap();
        assert_eq!(result, expect(vec![(PrimitiveOccurrencePosition::NamedField, vec!["String"])]));
    }

    #[test]
    fn nested_result_inside_transparent_container_reclassified_at_any_depth() {
        let result =
            scan("Vec<Result<u32, String>>", one("String"), PrimitiveOccurrencePosition::Param)
                .unwrap();
        assert_eq!(result, expect(vec![(PrimitiveOccurrencePosition::ResultErr, vec!["String"])]));
    }

    #[test]
    fn caller_supplied_param_position_passthrough() {
        let result = scan("String", one("String"), PrimitiveOccurrencePosition::Param).unwrap();
        assert_eq!(result, expect(vec![(PrimitiveOccurrencePosition::Param, vec!["String"])]));
    }

    #[test]
    fn caller_supplied_variant_field_position_passthrough() {
        let result =
            scan("String", one("String"), PrimitiveOccurrencePosition::VariantField).unwrap();
        assert_eq!(
            result,
            expect(vec![(PrimitiveOccurrencePosition::VariantField, vec!["String"])])
        );
    }

    #[test]
    fn caller_supplied_bound_position_passthrough() {
        let result = scan("String", one("String"), PrimitiveOccurrencePosition::Bound).unwrap();
        assert_eq!(result, expect(vec![(PrimitiveOccurrencePosition::Bound, vec!["String"])]));
    }

    // -- PR #179 round 2 P1: Bound-position scan of non-`syn::Type` bounds --

    #[test]
    fn bound_position_maybe_sized_is_ok_with_empty_report() {
        // `?Sized` is a legal catalogue bound (a `syn::TypeParamBound`) but
        // not a legal `syn::Type`; a Bound-position scan must not fail.
        let result = scan("?Sized", one("String"), PrimitiveOccurrencePosition::Bound).unwrap();
        assert_eq!(result, BTreeMap::new());
    }

    #[test]
    fn bound_position_lifetime_is_ok_with_empty_report() {
        // `'static` is a legal catalogue bound (a `syn::TypeParamBound`) but
        // not a legal `syn::Type`; a Bound-position scan must not fail, and a
        // bare lifetime can never carry a primitive-name occurrence.
        let result = scan("'static", one("String"), PrimitiveOccurrencePosition::Bound).unwrap();
        assert_eq!(result, BTreeMap::new());
    }

    #[test]
    fn bound_position_trait_bound_with_no_primitive_match_yields_empty_report() {
        let result = scan("MyTrait", one("String"), PrimitiveOccurrencePosition::Bound).unwrap();
        assert_eq!(result, BTreeMap::new());
    }

    #[test]
    fn bound_position_callable_trait_bound_reclassifies_param_to_param() {
        // `Fn(String) -> ()` is a legal trait bound (callable-type sugar);
        // the nested `String` param must still be reclassified to `Param`,
        // exactly as it would be from a `syn::Type` walk.
        let result =
            scan("Fn(String) -> ()", one("String"), PrimitiveOccurrencePosition::Bound).unwrap();
        assert_eq!(result, expect(vec![(PrimitiveOccurrencePosition::Param, vec!["String"])]));
    }

    #[test]
    fn bound_position_unparseable_bound_is_parse_failure() {
        let scanner = SynPrimitiveOccurrenceScanner;
        let err = scanner
            .scan(type_ref("Vec<String"), one("String"), PrimitiveOccurrencePosition::Bound)
            .unwrap_err();
        assert_eq!(
            err,
            PrimitiveOccurrenceScanError::ParseFailure { type_ref: type_ref("Vec<String") }
        );
    }

    #[test]
    fn multiple_requested_primitives_tracked_independently_across_result_slots() {
        let result = scan(
            "Result<String, u32>",
            many(&["String", "u32"]),
            PrimitiveOccurrencePosition::NamedField,
        )
        .unwrap();
        assert_eq!(
            result,
            expect(vec![
                (PrimitiveOccurrencePosition::NamedField, vec!["String"]),
                (PrimitiveOccurrencePosition::ResultErr, vec!["u32"]),
            ])
        );
    }
}
