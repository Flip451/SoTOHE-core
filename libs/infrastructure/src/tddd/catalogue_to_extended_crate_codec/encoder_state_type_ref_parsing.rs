//! `EncoderState` methods for TypeRef parsing, external-id resolution, and
//! generic-bound encoding.
//!
//! Extracted from `encoder_state_core` to keep each file within the 700-line
//! module-size limit while preserving identical public behaviour.

use std::collections::HashMap;

use rustdoc_types::{ExternalCrate, GenericArg, GenericArgs, GenericBound, Id, Path, Type};

use crate::tddd::catalogue_to_extended_crate_codec_error::CatalogueToExtendedCrateCodecError;
use crate::tddd::type_ref_parser::{
    UNRESOLVED_CRATE_ID, parse_generic_bound, parse_type_ref, parse_type_ref_with_generics,
};

use super::encoder::EncoderState;
use super::helpers::rewrite_generic_types_in_bound;

impl EncoderState {
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
            Type::Pat { type_, __pat_unstable_do_not_use } => Type::Pat {
                type_: Box::new(self.resolve_external_type_ids(*type_)),
                __pat_unstable_do_not_use,
            },
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
            Type::QualifiedPath { name, args, self_type, trait_ } => Type::QualifiedPath {
                name,
                args: args
                    .map(|boxed| Box::new(self.resolve_external_type_ids_in_generic_args(*boxed))),
                self_type: Box::new(self.resolve_external_type_ids(*self_type)),
                trait_: trait_.map(|path| self.resolve_external_type_ids_in_path(path)),
            },
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
            // `Primitive`, `Generic`, `Infer`, and any future variants need no id fix-up.
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
        self.parse_type_ref_str_inner(type_ref_str, &[], &[])
    }

    /// Parses a `TypeRef` string into a `rustdoc_types::Type`, recognising impl-block
    /// generic type parameter names.
    ///
    /// Identical to [`parse_type_ref_str`] except that `generic_params` lists the names
    /// of type parameters declared on an `impl` block. Any single-segment identifier
    /// matching an entry in `generic_params` is encoded as `Type::Generic(name)`. Generic
    /// parameters shadow same-named local catalogue items, matching Rust name resolution.
    ///
    /// This is used when encoding `TraitImplDeclV2.for_type` so that `for_type: "T"` with
    /// `impl_generics: [{name: "T", ...}]` produces `Type::Generic("T")` — matching the
    /// shape that rustdoc emits for `impl<T> Trait for T` (ADR 2026-06-18-0822 D2).
    pub(super) fn parse_type_ref_str_with_generics(
        &mut self,
        type_ref_str: &str,
        generic_params: &[&str],
    ) -> Result<Type, CatalogueToExtendedCrateCodecError> {
        self.parse_type_ref_str_inner(type_ref_str, generic_params, &[])
    }

    pub(super) fn parse_type_ref_str_with_suppressed_external_prefixes(
        &mut self,
        type_ref_str: &str,
        generic_params: &[&str],
        suppressed_external_prefixes: &[&str],
    ) -> Result<Type, CatalogueToExtendedCrateCodecError> {
        self.parse_type_ref_str_inner(type_ref_str, generic_params, suppressed_external_prefixes)
    }

    fn parse_type_ref_str_inner(
        &mut self,
        type_ref_str: &str,
        generic_params: &[&str],
        suppressed_external_prefixes: &[&str],
    ) -> Result<Type, CatalogueToExtendedCrateCodecError> {
        let std_crate_id = self
            .ext_name_to_id
            .get("std")
            .copied()
            .unwrap_or_else(|| self.ensure_external_crate("std".to_string()));

        // --- Pass 1: discover new external crate names ---
        let local_snapshot: HashMap<String, Id> = self.local_name_to_id.clone();
        let ext_snapshot = self.external_crate_ids_without_prefixes(suppressed_external_prefixes);
        let mut new_crate_names: Vec<String> = vec![];

        let _ = Self::parse_type_ref_with_context(
            type_ref_str,
            &|name: &str| local_id_unless_generic(&local_snapshot, name, generic_params),
            std_crate_id,
            &ext_snapshot,
            &mut |crate_name: String| {
                if is_suppressed_external_prefix(&crate_name, suppressed_external_prefixes) {
                    return u32::MAX - 1;
                }
                if !new_crate_names.contains(&crate_name) {
                    new_crate_names.push(crate_name);
                }
                u32::MAX - 1 // placeholder; discarded
            },
            generic_params,
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
        let ext_snapshot2 = self.external_crate_ids_without_prefixes(suppressed_external_prefixes);

        let raw_type = Self::parse_type_ref_with_context(
            type_ref_str,
            &|name: &str| local_id_unless_generic(&local_snapshot2, name, generic_params),
            std_crate_id,
            &ext_snapshot2,
            &mut |crate_name: String| {
                if is_suppressed_external_prefix(&crate_name, suppressed_external_prefixes) {
                    u32::MAX - 1
                } else {
                    self.ensure_external_crate(crate_name)
                }
            },
            generic_params,
        )
        .map_err(|reason| CatalogueToExtendedCrateCodecError::InvalidTypeRef {
            type_ref: type_ref_str.to_string(),
            reason,
        })?;

        // --- Pass 3: post-process ---
        let suppressed_external_crates =
            self.remove_external_crate_prefixes(suppressed_external_prefixes);
        let resolved = self.resolve_external_type_ids(raw_type);
        self.restore_external_crate_prefixes(suppressed_external_crates);
        Ok(resolved)
    }

    fn parse_type_ref_with_context<F, G>(
        type_ref_str: &str,
        resolve_local: &F,
        std_crate_id: u32,
        external_crate_ids: &HashMap<String, u32>,
        emit_external_crate: &mut G,
        generic_params: &[&str],
    ) -> Result<Type, String>
    where
        F: Fn(&str) -> Option<Id>,
        G: FnMut(String) -> u32,
    {
        if generic_params.is_empty() {
            parse_type_ref(
                type_ref_str,
                resolve_local,
                std_crate_id,
                external_crate_ids,
                emit_external_crate,
            )
        } else {
            parse_type_ref_with_generics(
                type_ref_str,
                resolve_local,
                std_crate_id,
                external_crate_ids,
                emit_external_crate,
                generic_params,
            )
        }
    }

    fn external_crate_ids_without_prefixes(
        &self,
        suppressed_external_prefixes: &[&str],
    ) -> HashMap<String, u32> {
        let mut snapshot = self.ext_name_to_id.clone();
        for prefix in suppressed_external_prefixes {
            snapshot.remove(*prefix);
        }
        snapshot
    }

    fn remove_external_crate_prefixes(
        &mut self,
        suppressed_external_prefixes: &[&str],
    ) -> Vec<(String, Option<u32>, Option<ExternalCrate>)> {
        let mut removed: Vec<(String, Option<u32>, Option<ExternalCrate>)> =
            Vec::with_capacity(suppressed_external_prefixes.len());
        for prefix in suppressed_external_prefixes {
            if removed.iter().any(|(name, _, _)| name.as_str() == *prefix) {
                continue;
            }
            let crate_id = self.ext_name_to_id.remove(*prefix);
            let crate_entry = crate_id.and_then(|id| self.external_crates.remove(&id));
            removed.push(((*prefix).to_string(), crate_id, crate_entry));
        }
        removed
    }

    fn restore_external_crate_prefixes(
        &mut self,
        removed: Vec<(String, Option<u32>, Option<ExternalCrate>)>,
    ) {
        for (name, crate_id, crate_entry) in removed {
            if let Some(crate_id) = crate_id {
                self.ext_name_to_id.insert(name, crate_id);
                if let Some(crate_entry) = crate_entry {
                    self.external_crates.insert(crate_id, crate_entry);
                }
            }
        }
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
        self.encode_bound_str_inner(bound_str, &[])
    }

    pub(super) fn encode_bound_str_with_suppressed_external_prefixes(
        &mut self,
        bound_str: &str,
        suppressed_external_prefixes: &[&str],
    ) -> Result<GenericBound, CatalogueToExtendedCrateCodecError> {
        self.encode_bound_str_inner(bound_str, suppressed_external_prefixes)
    }

    fn encode_bound_str_inner(
        &mut self,
        bound_str: &str,
        suppressed_external_prefixes: &[&str],
    ) -> Result<GenericBound, CatalogueToExtendedCrateCodecError> {
        // Handle `~const` prefix manually because stable syn v2 does not have a
        // `TraitBoundModifier::MaybeConst` variant.  Strip the prefix and encode
        // the remainder as a plain trait bound with MaybeConst modifier.
        if let Some(inner) = bound_str.strip_prefix("~const ") {
            let inner = inner.trim_start();
            // Encode the inner trait path via parse_type_ref_str (no modifier prefix).
            let ty = self.parse_type_ref_str_inner(inner, &[], suppressed_external_prefixes)?;
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
            let ext_snapshot =
                self.external_crate_ids_without_prefixes(suppressed_external_prefixes);
            let mut new_crate_names: Vec<String> = vec![];
            let _ = parse_generic_bound(
                bound_str,
                &|name: &str| local_snapshot.get(name).copied(),
                std_crate_id,
                &ext_snapshot,
                &mut |crate_name: String| {
                    if is_suppressed_external_prefix(&crate_name, suppressed_external_prefixes) {
                        return u32::MAX - 1;
                    }
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
        let ext_snapshot2 = self.external_crate_ids_without_prefixes(suppressed_external_prefixes);
        parse_generic_bound(
            bound_str,
            &|name: &str| local_snapshot2.get(name).copied(),
            std_crate_id,
            &ext_snapshot2,
            &mut |crate_name: String| {
                if is_suppressed_external_prefix(&crate_name, suppressed_external_prefixes) {
                    u32::MAX - 1
                } else {
                    self.ensure_external_crate(crate_name)
                }
            },
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
}

fn is_suppressed_external_prefix(crate_name: &str, suppressed_external_prefixes: &[&str]) -> bool {
    suppressed_external_prefixes.contains(&crate_name)
}

fn local_id_unless_generic(
    local_snapshot: &HashMap<String, Id>,
    name: &str,
    generic_params: &[&str],
) -> Option<Id> {
    if generic_params.contains(&name) { None } else { local_snapshot.get(name).copied() }
}
