//! Core `EncoderState` methods: id allocation, external-crate registration,
//! TypeRef parsing, generic-bound encoding, and path/generics helpers.

use std::collections::HashMap;

use domain::tddd::catalogue_v2::{
    BoundOp, CrateName, MethodGenericParam, ModulePath, WherePredicateDecl,
};
use rustdoc_types::{
    ExternalCrate, GenericArg, GenericArgs, GenericBound, GenericParamDef, GenericParamDefKind,
    Generics, Id, ItemKind, ItemSummary, Path, Term, Type, WherePredicate,
};

use crate::tddd::catalogue_to_extended_crate_codec_error::CatalogueToExtendedCrateCodecError;
use crate::tddd::type_ref_parser::{UNRESOLVED_CRATE_ID, parse_generic_bound, parse_type_ref};

use super::encoder::EncoderState;
use super::helpers::{
    is_bare_generic_name, rewrite_generic_types, rewrite_generic_types_in_bound,
    try_build_generic_projection,
};

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

    /// Post-processes a `Type` tree returned by `parse_type_ref`, replacing the
    /// `UNRESOLVED_CRATE_ID` sentinel with fresh synthetic item ids for
    /// identifiers that are **known externals** (std prelude or crate-prefixed with a
    /// registered external crate).
    ///
    /// Truly-unresolved identifiers (single-segment names that have no `"::"` and
    /// are not registered) keep `Id(UNRESOLVED_CRATE_ID)` so Phase 1 can detect
    /// and reject them.
    ///
    /// ADR D11 / D10: std prelude and crate-prefixed refs must not be flagged as
    /// Phase 1 errors.  Allocating a `paths` entry for them lets the S-construction
    /// algorithm identify them as valid externals without string-pattern heuristics.
    pub(super) fn resolve_external_type_ids(&mut self, ty: Type) -> Type {
        match ty {
            // `ResolvedPath` — delegate to the shared path helper which fixes up the id
            // and recurses into generic args so nested externals are also corrected.
            Type::ResolvedPath(p) => Type::ResolvedPath(self.resolve_external_type_ids_in_path(p)),
            // Recurse into container types.
            Type::Tuple(elems) => {
                Type::Tuple(elems.into_iter().map(|t| self.resolve_external_type_ids(t)).collect())
            }
            Type::Slice(inner) => Type::Slice(Box::new(self.resolve_external_type_ids(*inner))),
            Type::Array { type_, len } => {
                Type::Array { type_: Box::new(self.resolve_external_type_ids(*type_)), len }
            }
            Type::BorrowedRef { lifetime, is_mutable, type_ } => Type::BorrowedRef {
                lifetime,
                is_mutable,
                type_: Box::new(self.resolve_external_type_ids(*type_)),
            },
            Type::RawPointer { is_mutable, type_ } => Type::RawPointer {
                is_mutable,
                type_: Box::new(self.resolve_external_type_ids(*type_)),
            },
            Type::ImplTrait(bounds) => Type::ImplTrait(
                bounds
                    .into_iter()
                    .map(|b| self.resolve_external_type_ids_in_generic_bound(b))
                    .collect(),
            ),
            // `dyn Trait + Trait2` — fix up each bound's trait path and generic args.
            Type::DynTrait(dyn_trait) => {
                let new_traits = dyn_trait
                    .traits
                    .into_iter()
                    .map(|pt| {
                        let new_trait_path = self.resolve_external_type_ids_in_path(pt.trait_);
                        rustdoc_types::PolyTrait {
                            trait_: new_trait_path,
                            generic_params: pt.generic_params,
                        }
                    })
                    .collect();
                Type::DynTrait(rustdoc_types::DynTrait {
                    traits: new_traits,
                    lifetime: dyn_trait.lifetime,
                })
            }
            // `fn(A, B) -> C` function pointers — fix up input and output types.
            Type::FunctionPointer(fp) => {
                let new_inputs = fp
                    .sig
                    .inputs
                    .into_iter()
                    .map(|(name, t)| (name, self.resolve_external_type_ids(t)))
                    .collect();
                let new_output = fp.sig.output.map(|t| self.resolve_external_type_ids(t));
                Type::FunctionPointer(Box::new(rustdoc_types::FunctionPointer {
                    sig: rustdoc_types::FunctionSignature {
                        inputs: new_inputs,
                        output: new_output,
                        is_c_variadic: fp.sig.is_c_variadic,
                    },
                    generic_params: fp.generic_params,
                    header: fp.header,
                }))
            }
            // `Primitive`, `Infer`, and any future variants need no id fix-up.
            other => other,
        }
    }

    /// Resolves external type ids inside a `Path` value (used for trait bound paths).
    pub(super) fn resolve_external_type_ids_in_path(&mut self, path: Path) -> Path {
        let new_id = if path.id == Id(UNRESOLVED_CRATE_ID) {
            if let Some(colon_pos) = path.path.find("::") {
                let first_seg = &path.path[..colon_pos];
                if self.ext_name_to_id.contains_key(first_seg) {
                    self.ensure_external_type_id(&path.path, first_seg)
                } else {
                    Id(UNRESOLVED_CRATE_ID)
                }
            } else {
                Id(UNRESOLVED_CRATE_ID)
            }
        } else {
            path.id
        };
        let new_args =
            path.args.map(|boxed| Box::new(self.resolve_external_type_ids_in_generic_args(*boxed)));
        Path { path: path.path, id: new_id, args: new_args }
    }

    /// Resolves external type ids inside a `GenericArgs` value.
    pub(super) fn resolve_external_type_ids_in_generic_args(
        &mut self,
        args: GenericArgs,
    ) -> GenericArgs {
        match args {
            GenericArgs::AngleBracketed { args: ga_args, constraints } => {
                let new_args = ga_args
                    .into_iter()
                    .map(|ga| match ga {
                        GenericArg::Type(t) => GenericArg::Type(self.resolve_external_type_ids(t)),
                        other => other,
                    })
                    .collect();
                let new_constraints = constraints
                    .into_iter()
                    .map(|c| {
                        use rustdoc_types::{AssocItemConstraint, AssocItemConstraintKind, Term};
                        let binding = match c.binding {
                            AssocItemConstraintKind::Equality(Term::Type(t)) => {
                                AssocItemConstraintKind::Equality(Term::Type(
                                    self.resolve_external_type_ids(t),
                                ))
                            }
                            AssocItemConstraintKind::Constraint(bounds) => {
                                AssocItemConstraintKind::Constraint(
                                    bounds
                                        .into_iter()
                                        .map(|b| self.resolve_external_type_ids_in_generic_bound(b))
                                        .collect(),
                                )
                            }
                            other => other,
                        };
                        AssocItemConstraint { binding, ..c }
                    })
                    .collect();
                GenericArgs::AngleBracketed { args: new_args, constraints: new_constraints }
            }
            GenericArgs::Parenthesized { inputs, output } => {
                let new_inputs =
                    inputs.into_iter().map(|t| self.resolve_external_type_ids(t)).collect();
                let new_output = output.map(|t| self.resolve_external_type_ids(t));
                GenericArgs::Parenthesized { inputs: new_inputs, output: new_output }
            }
            // `ReturnTypeNotation` (e.g. `Trait(..)`): no type args to fix up.
            other @ GenericArgs::ReturnTypeNotation => other,
        }
    }

    /// Resolves external type ids inside a `GenericBound` value.
    pub(super) fn resolve_external_type_ids_in_generic_bound(
        &mut self,
        bound: rustdoc_types::GenericBound,
    ) -> rustdoc_types::GenericBound {
        use rustdoc_types::GenericBound;
        match bound {
            GenericBound::TraitBound { trait_, generic_params, modifier } => {
                let new_trait = self.resolve_external_type_ids_in_path(trait_);
                GenericBound::TraitBound { trait_: new_trait, generic_params, modifier }
            }
            other => other,
        }
    }

    /// Parses a `TypeRef` string into a `rustdoc_types::Type`.
    ///
    /// Resolves names via `local_name_to_id`; unknown names become unresolved markers.
    /// New external crate names encountered during parse are registered automatically.
    ///
    /// Uses a two-pass strategy to satisfy the borrow checker:
    /// 1. Discovery pass — collect any new external crate names without keeping the
    ///    `Type` result (the returned type would have stale placeholder ids).
    /// 2. Encoding pass — register the new crates, rebuild a fresh snapshot, then
    ///    re-parse to produce the final `Type` with correct crate ids.
    /// 3. Post-processing — walk the `Type` tree and replace `UNRESOLVED_CRATE_ID` with
    ///    synthetic item ids for known externals (std prelude and crate-prefixed types),
    ///    so downstream Phase 1 validation can distinguish them from truly-unresolved
    ///    markers (ADR D10 / D11).
    pub(super) fn parse_type_ref_str(
        &mut self,
        type_ref_str: &str,
    ) -> Result<Type, CatalogueToExtendedCrateCodecError> {
        let std_crate_id = self
            .ext_name_to_id
            .get("std")
            .copied()
            .unwrap_or_else(|| self.ensure_external_crate("std".to_string()));

        // --- Pass 1: discover new external crate names ---
        let local_snapshot: HashMap<String, Id> = self.local_name_to_id.clone();
        let ext_snapshot: HashMap<String, u32> = self.ext_name_to_id.clone();
        let mut new_crate_names: Vec<String> = vec![];

        // We discard the returned `Type` here because it contains placeholder ids
        // for any newly discovered crates.
        let _ = parse_type_ref(
            type_ref_str,
            &|name: &str| local_snapshot.get(name).copied(),
            std_crate_id,
            &ext_snapshot,
            &mut |crate_name: String| {
                if !new_crate_names.contains(&crate_name) {
                    new_crate_names.push(crate_name);
                }
                u32::MAX - 1 // placeholder; discarded
            },
        )
        .map_err(|reason| CatalogueToExtendedCrateCodecError::InvalidTypeRef {
            type_ref: type_ref_str.to_string(),
            reason,
        })?;

        // Register any new external crate names before the encoding pass.
        for crate_name in new_crate_names {
            self.ensure_external_crate(crate_name);
        }

        // --- Pass 2: encode with complete crate-id map ---
        let local_snapshot2: HashMap<String, Id> = self.local_name_to_id.clone();
        let ext_snapshot2: HashMap<String, u32> = self.ext_name_to_id.clone();

        let raw_type = parse_type_ref(
            type_ref_str,
            &|name: &str| local_snapshot2.get(name).copied(),
            std_crate_id,
            &ext_snapshot2,
            // All crates are already registered; this closure should not be called.
            // If it is (e.g. due to a race in multi-segment paths), register on-demand.
            &mut |crate_name: String| self.ensure_external_crate(crate_name),
        )
        .map_err(|reason| CatalogueToExtendedCrateCodecError::InvalidTypeRef {
            type_ref: type_ref_str.to_string(),
            reason,
        })?;

        // --- Pass 3: post-process — replace UNRESOLVED_CRATE_ID with synthetic item
        // ids for known external types (std prelude and crate-prefixed paths whose
        // crate name is already registered in ext_name_to_id).  Truly-unresolved
        // markers (single-segment unknown names) keep Id(UNRESOLVED_CRATE_ID).
        Ok(self.resolve_external_type_ids(raw_type))
    }

    /// Encodes a bound string (e.g. `"Into<String>"`, `"Send"`, `"?Sized"`,
    /// `"'static"`, `"for<'a> Fn(&'a str)"`) into a `rustdoc_types::GenericBound`.
    ///
    /// Uses `parse_generic_bound` (which parses via `syn::TypeParamBound`) so that
    /// the set of accepted strings is identical between the decode path
    /// (`validate_bound_str` in `catalogue_document_codec`) and this encode path.
    /// Both use the same `syn::TypeParamBound` grammar, closing the round-trip hole
    /// that previously existed when `parse_type_ref_str` (which uses `syn::Type`)
    /// was used here — that stricter parser rejected `?Trait`, lifetime bounds
    /// (`'static`), and higher-ranked trait bounds (`for<'a> Fn(&'a str)`).
    ///
    /// Conversion:
    /// - `'lifetime` → `GenericBound::Outlives`.
    /// - `?Trait` → `GenericBound::TraitBound { modifier: Maybe, ... }`.
    /// - `for<'a> Trait<'a>` → `GenericBound::TraitBound { generic_params: [Lifetime('a)], ... }`.
    /// - Plain trait or `~const Trait` → `GenericBound::TraitBound { modifier: None/MaybeConst, ... }`.
    ///   (`~const` is nightly-only; the string `"~const "` prefix maps to `MaybeConst`
    ///   but `syn` v2 stable does not recognise it as a `TraitBoundModifier` variant —
    ///   the `parse_generic_bound` fallback covers this case via `Err` propagation.)
    ///
    /// # Errors
    ///
    /// Returns `CatalogueToExtendedCrateCodecError` if the bound string cannot be
    /// parsed as a `TypeParamBound` by `syn`.
    pub(super) fn encode_bound_str(
        &mut self,
        bound_str: &str,
    ) -> Result<GenericBound, CatalogueToExtendedCrateCodecError> {
        // Handle `~const` prefix manually because stable syn v2 does not have a
        // `TraitBoundModifier::MaybeConst` variant.  Strip the prefix and encode
        // the remainder as a plain trait bound with MaybeConst modifier.
        if let Some(inner) = bound_str.strip_prefix("~const ") {
            let inner = inner.trim_start();
            // Encode the inner trait path via parse_type_ref_str (no modifier prefix).
            let ty = self.parse_type_ref_str(inner)?;
            let trait_path = match ty {
                Type::ResolvedPath(p) => p,
                other => {
                    return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                        type_ref: bound_str.to_string(),
                        reason: format!(
                            "~const bound must resolve to a trait path, got {:?}",
                            std::mem::discriminant(&other)
                        ),
                    });
                }
            };
            return Ok(GenericBound::TraitBound {
                trait_: trait_path,
                generic_params: vec![],
                modifier: rustdoc_types::TraitBoundModifier::MaybeConst,
            });
        }

        let std_crate_id = self
            .ext_name_to_id
            .get("std")
            .copied()
            .unwrap_or_else(|| self.ensure_external_crate("std".to_string()));

        // Pass 1: discover new external crate names (same two-pass strategy as parse_type_ref_str).
        {
            let local_snapshot: HashMap<String, Id> = self.local_name_to_id.clone();
            let ext_snapshot: HashMap<String, u32> = self.ext_name_to_id.clone();
            let mut new_crate_names: Vec<String> = vec![];
            let _ = parse_generic_bound(
                bound_str,
                &|name: &str| local_snapshot.get(name).copied(),
                std_crate_id,
                &ext_snapshot,
                &mut |crate_name: String| {
                    if !new_crate_names.contains(&crate_name) {
                        new_crate_names.push(crate_name);
                    }
                    u32::MAX - 1
                },
            )
            .map_err(|reason| CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                type_ref: bound_str.to_string(),
                reason,
            })?;
            for crate_name in new_crate_names {
                self.ensure_external_crate(crate_name);
            }
        }

        // Pass 2: encode with complete crate-id map.
        let local_snapshot2: HashMap<String, Id> = self.local_name_to_id.clone();
        let ext_snapshot2: HashMap<String, u32> = self.ext_name_to_id.clone();
        parse_generic_bound(
            bound_str,
            &|name: &str| local_snapshot2.get(name).copied(),
            std_crate_id,
            &ext_snapshot2,
            &mut |crate_name: String| self.ensure_external_crate(crate_name),
        )
        .map_err(|reason| CatalogueToExtendedCrateCodecError::InvalidTypeRef {
            type_ref: bound_str.to_string(),
            reason,
        })
    }

    /// Encodes a `MethodGenericParam.bounds[i]` or `WherePredicateDecl.bounds[i]` entry
    /// to a `GenericBound`, applying generic-name rewriting.
    ///
    /// All `syn`-parseable bound strings are accepted regardless of kind: lifetime
    /// bounds (`'static`, `'a`), HRTB (`for<'a> Fn(&'a T)`), precise-capture
    /// (`use<'a, T>`), and plain trait bounds (ADR `2026-05-18-1223` D1).
    /// Bounds that `syn` cannot parse are propagated as `Err`.
    pub(super) fn encode_and_validate_bound(
        &mut self,
        bound_str: &str,
        generic_names: &[&str],
    ) -> Result<GenericBound, CatalogueToExtendedCrateCodecError> {
        let raw = self.encode_bound_str(bound_str)?;
        let rewritten = if generic_names.is_empty() {
            raw
        } else {
            rewrite_generic_types_in_bound(raw, generic_names)
        };
        Ok(rewritten)
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
