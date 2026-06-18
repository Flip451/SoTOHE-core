//! Core `EncoderState` methods: id allocation, external-crate registration,
//! generic-bound building, and path/generics helpers.
//!
//! TypeRef parsing and external-id resolution live in the sibling module
//! `encoder_state_type_ref_parsing` to keep each file within the 700-line limit.

use domain::tddd::catalogue_v2::{
    BoundOp, CrateName, MethodGenericParam, ModulePath, WherePredicateDecl,
};
use rustdoc_types::{
    ExternalCrate, GenericBound, GenericParamDef, GenericParamDefKind, Generics, Id, ItemKind,
    ItemSummary, Term, Type, WherePredicate,
};

use crate::tddd::catalogue_to_extended_crate_codec_error::CatalogueToExtendedCrateCodecError;

use super::encoder::EncoderState;
use super::helpers::{is_bare_generic_name, rewrite_generic_types, try_build_generic_projection};

impl EncoderState {
    pub(super) fn alloc_id(&mut self) -> Id {
        let id = Id(self.next_id);
        self.next_id += 1;
        id
    }

    /// Ensures an external crate is registered and returns its `crate_id`.
    pub(super) fn ensure_external_crate(&mut self, crate_name: String) -> u32 {
        if let Some(&id) = self.ext_name_to_id.get(&crate_name) {
            return id;
        }
        let id = self.next_ext_id;
        self.next_ext_id += 1;
        self.ext_name_to_id.insert(crate_name.clone(), id);
        self.external_crates.insert(
            id,
            ExternalCrate {
                name: crate_name,
                html_root_url: None,
                path: std::path::PathBuf::new(),
            },
        );
        id
    }

    /// Ensures a synthetic item id is allocated for a resolved external type and
    /// registers a `Crate::paths` entry for it, so downstream consumers can
    /// distinguish resolved externals from truly-unresolved markers by id lookup.
    ///
    /// The `canonical_path` must be the full `"::"` -separated path string (e.g.
    /// `"std::vec::Vec"` or `"domain_core::UserId"`).  The `crate_name` is the
    /// first path segment (used to look up the external crate's `crate_id`).
    ///
    /// Returns the synthetic `Id` for use in `Path.id`.  Repeated calls with the
    /// same `canonical_path` return the same `Id` (cached in
    /// `external_type_path_to_id`).
    pub(super) fn ensure_external_type_id(&mut self, canonical_path: &str, crate_name: &str) -> Id {
        if let Some(&cached) = self.external_type_path_to_id.get(canonical_path) {
            return cached;
        }
        let synthetic_id = self.alloc_id();
        self.external_type_path_to_id.insert(canonical_path.to_string(), synthetic_id);

        let crate_id = self.ensure_external_crate(crate_name.to_string());
        let path_segs: Vec<String> = canonical_path.split("::").map(str::to_string).collect();
        self.paths.insert(
            synthetic_id,
            ItemSummary { crate_id, path: path_segs, kind: ItemKind::Struct },
        );
        synthetic_id
    }

    /// Builds rustdoc `Generics` in the **maximally-desugared where form** from
    /// catalogue declarations (`MethodGenericParam`s and `WherePredicateDecl`s).
    ///
    /// All bounds — whether declared inline on a `MethodGenericParam` or explicitly
    /// in a `WherePredicateDecl` — are emitted into `Generics.where_predicates` as
    /// `WherePredicate::BoundPredicate` entries. `GenericParamDef.bounds` is always
    /// empty and `is_synthetic` is always false on the resulting params.
    ///
    /// This mirrors rustdoc's representation of `where` clauses and lets the signal
    /// evaluator compare both sides in a single canonical form
    /// (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D1).
    pub(super) fn build_where_form_generics(
        &mut self,
        generics_decl: &[MethodGenericParam],
        where_decls: &[WherePredicateDecl],
        generic_names: &[&str],
    ) -> Result<Generics, CatalogueToExtendedCrateCodecError> {
        let mut params: Vec<GenericParamDef> = Vec::with_capacity(generics_decl.len());
        let mut where_predicates: Vec<WherePredicate> = Vec::new();

        // (1) MethodGenericParam → empty-bound `GenericParamDef` + one
        //     `BoundPredicate { type_: Generic(name), bounds }` per param.
        for g in generics_decl {
            let mut bounds: Vec<GenericBound> = Vec::with_capacity(g.bounds.len());
            for b in &g.bounds {
                bounds.push(self.encode_and_validate_bound(b.as_str(), generic_names)?);
            }
            params.push(GenericParamDef {
                name: g.name.as_str().to_owned(),
                kind: GenericParamDefKind::Type {
                    bounds: vec![],
                    default: None,
                    is_synthetic: false,
                },
            });
            if !bounds.is_empty() {
                where_predicates.push(WherePredicate::BoundPredicate {
                    type_: Type::Generic(g.name.as_str().to_owned()),
                    bounds,
                    generic_params: vec![],
                });
            }
        }

        // (2) Explicit `WherePredicateDecl` entries — LHS is an arbitrary type
        //     expression. Parse via `parse_type_ref_str` (with bare-generic-name
        //     short-circuit identical to the param/return logic).
        for w in where_decls {
            let lhs_str = w.lhs.as_str();
            // Guard: a `WherePredicateDecl` with no rhs would encode to
            // `WherePredicate::BoundPredicate { bounds: vec![] }` which is
            // syntactically invalid in Rust (`where T:` without any bound).
            // This mirrors the symmetrical check in `where_predicates_from_dtos`.
            if w.rhs.is_empty() {
                return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                    type_ref: lhs_str.to_owned(),
                    reason: "where predicate has no rhs (`where T:` is not valid Rust); \
                             at least one rhs entry is required"
                        .to_owned(),
                });
            }
            // Permissive principle (ADR `2026-05-20-0048`): accept any syn-parseable LHS,
            // including qualified-path forms such as `<T as Trait>::Assoc`.  The type-ref
            // parser cannot reconstruct the exact `Type::QualifiedPath` shape that rustdoc
            // emits for such predicates (it falls back to an unresolved placeholder), but
            // this is acceptable under the permissive principle — syntactic validity is the
            // acceptance gate, not shape faithfulness.  The decoder accepts these forms via
            // `validate_type_ref_str` (syn::Type), so the encoder must also accept them to
            // preserve round-trip symmetry.
            let lhs_type =
                if !generic_names.is_empty() && is_bare_generic_name(lhs_str, generic_names) {
                    // Simple bare generic: `T` → `Type::Generic("T")`
                    Type::Generic(lhs_str.trim().to_string())
                } else if !generic_names.is_empty() {
                    if let Some(proj) = try_build_generic_projection(lhs_str, generic_names) {
                        // Single-level associated-type projection: `T::Item` →
                        // `Type::QualifiedPath { name: "Item", self_type: Generic("T"),
                        //  trait_: None, args: None }`.
                        //
                        // This matches the shape that rustdoc emits for `where T::Item: …`
                        // predicates so that A-catalogue and C-rustdoc representations
                        // compare equal in `build_where_form_view`.
                        proj
                    } else {
                        let raw = self.parse_type_ref_str(lhs_str)?;
                        rewrite_generic_types(raw, generic_names)
                    }
                } else {
                    self.parse_type_ref_str(lhs_str)?
                };
            match w.operator {
                BoundOp::Bound => {
                    let mut bounds: Vec<GenericBound> = Vec::with_capacity(w.rhs.len());
                    for b in &w.rhs {
                        bounds.push(self.encode_and_validate_bound(b.as_str(), generic_names)?);
                    }
                    where_predicates.push(WherePredicate::BoundPredicate {
                        type_: lhs_type,
                        bounds,
                        generic_params: vec![],
                    });
                }
                BoundOp::Equal => {
                    // `Equal` predicates (`where T::Assoc = U`) encode as `EqPredicate`.
                    // Enforce rhs.len() == 1 defensively: decode validates this, but
                    // in-memory domain values constructed outside the codec must also pass.
                    if w.rhs.len() != 1 {
                        return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                            type_ref: lhs_str.to_owned(),
                            reason: format!(
                                "Equal predicate must have exactly one rhs entry (got {}); \
                                 `where T::Assoc = U` accepts a single RHS only",
                                w.rhs.len()
                            ),
                        });
                    }
                    // Permissive principle (ADR `2026-05-18-1223` / `2026-05-20-0048`):
                    // accept any syn-parseable LHS for Equal predicates, including bare
                    // type parameters (`T = U`).  The `lhs_type` has already been computed
                    // via `parse_type_ref_str` / `is_bare_generic_name` above; no additional
                    // shape validation is applied here.
                    // Safe: len == 1 asserted above.
                    let rhs_entry = w.rhs.first().ok_or_else(|| {
                        CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                            type_ref: lhs_str.to_owned(),
                            reason: "Equal predicate has no rhs (codec invariant violated)"
                                .to_owned(),
                        }
                    })?;
                    let rhs_str = rhs_entry.as_str();
                    // Permissive principle (ADR `2026-05-20-0048`): accept any syn-parseable
                    // RHS, including qualified-path forms like `<T as Trait>::Assoc`.  The
                    // parser falls back to an unresolved placeholder for forms it cannot encode
                    // exactly, which is acceptable — syntactic validity is the gate.
                    let rhs_type = if !generic_names.is_empty()
                        && is_bare_generic_name(rhs_str, generic_names)
                    {
                        Type::Generic(rhs_str.trim().to_string())
                    } else if !generic_names.is_empty() {
                        if let Some(proj) = try_build_generic_projection(rhs_str, generic_names) {
                            proj
                        } else {
                            let raw = self.parse_type_ref_str(rhs_str)?;
                            rewrite_generic_types(raw, generic_names)
                        }
                    } else {
                        self.parse_type_ref_str(rhs_str)?
                    };
                    where_predicates.push(WherePredicate::EqPredicate {
                        lhs: lhs_type,
                        rhs: Term::Type(rhs_type),
                    });
                }
            }
        }

        Ok(Generics { params, where_predicates })
    }

    /// Builds `[crate_name, ...module_path, item_name]` path segments.
    pub(super) fn build_path_segments(
        crate_name: &CrateName,
        module_path: &ModulePath,
        item_name: &str,
    ) -> Vec<String> {
        let mut segments = vec![crate_name.as_str().to_string()];
        for seg in module_path.segments() {
            segments.push(seg.as_str().to_string());
        }
        segments.push(item_name.to_string());
        segments
    }

    /// Registers an `ItemSummary` in `Crate::paths` using the document crate name.
    ///
    /// Always uses `self.crate_name` as the crate component and `crate_id: 0` (local crate).
    /// Use `register_path_for_crate` when the effective crate name may differ.
    pub(super) fn register_path(
        &mut self,
        id: Id,
        kind: ItemKind,
        item_name: &str,
        module_path: &ModulePath,
    ) {
        let path = Self::build_path_segments(&self.crate_name.clone(), module_path, item_name);
        self.paths.insert(id, ItemSummary { crate_id: 0, path, kind });
    }

    /// Registers an `ItemSummary` in `Crate::paths` using an explicit crate name.
    ///
    /// If `fn_crate_name` matches the document crate, the item is recorded under
    /// `crate_id: 0` (local crate). If it differs, the external crate id is looked up
    /// or allocated via `ensure_external_crate`.
    pub(super) fn register_path_for_crate(
        &mut self,
        id: Id,
        kind: ItemKind,
        item_name: &str,
        module_path: &ModulePath,
        fn_crate_name: &CrateName,
    ) {
        let (effective_crate_name, crate_id) = if fn_crate_name.as_str() == self.crate_name.as_str()
        {
            (fn_crate_name.clone(), 0u32)
        } else {
            let ext_id = self.ensure_external_crate(fn_crate_name.as_str().to_string());
            (fn_crate_name.clone(), ext_id)
        };
        let path = Self::build_path_segments(&effective_crate_name, module_path, item_name);
        self.paths.insert(id, ItemSummary { crate_id, path, kind });
    }
}
