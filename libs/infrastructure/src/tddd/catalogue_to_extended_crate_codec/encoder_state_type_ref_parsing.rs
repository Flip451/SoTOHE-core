//! `EncoderState` methods for TypeRef parsing, external-id resolution, and
//! generic-bound encoding.
//!
//! Extracted from `encoder_state_core` to keep each file within the 700-line
//! module-size limit while preserving identical public behaviour.

use std::collections::HashMap;

use rustdoc_types::{ExternalCrate, GenericBound, Id, Type};

use super::encoder::EncoderState;
use super::invalid_type_ref;
use crate::tddd::canonical_type_identity::canonicalize_catalogue_type_ref;
use crate::tddd::type_ref_parser::{
    parse_generic_bound_with_generics, parse_generic_bound_with_generics_preserving_spelling,
    parse_syn_type, parse_type_ref, parse_type_ref_with_generics,
    parse_type_ref_with_generics_preserving_spelling,
};
use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::identifiers::{ParamName, TypeRef};

impl EncoderState {
    /// Parses a `TypeRef` string into a `rustdoc_types::Type`.
    ///
    /// Resolves names via unique short-name aliases and the full-path identity index.
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
    ) -> Result<Type, NewTypeGraphCodecError> {
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
    ) -> Result<Type, NewTypeGraphCodecError> {
        self.parse_type_ref_str_inner(type_ref_str, generic_params, &[])
    }

    /// Parses a type reference while preserving the source spelling of std-prelude paths.
    pub(super) fn parse_type_ref_str_with_generics_preserving_spelling(
        &mut self,
        type_ref_str: &str,
        generic_params: &[&str],
    ) -> Result<Type, NewTypeGraphCodecError> {
        self.parse_type_ref_str_inner_with_prelude_spelling(type_ref_str, generic_params, &[], true)
    }

    pub(super) fn parse_type_ref_str_with_suppressed_external_prefixes(
        &mut self,
        type_ref_str: &str,
        generic_params: &[&str],
        suppressed_external_prefixes: &[&str],
    ) -> Result<Type, NewTypeGraphCodecError> {
        self.parse_type_ref_str_inner(type_ref_str, generic_params, suppressed_external_prefixes)
    }

    fn parse_type_ref_str_inner(
        &mut self,
        type_ref_str: &str,
        generic_params: &[&str],
        suppressed_external_prefixes: &[&str],
    ) -> Result<Type, NewTypeGraphCodecError> {
        self.parse_type_ref_str_inner_with_prelude_spelling(
            type_ref_str,
            generic_params,
            suppressed_external_prefixes,
            false,
        )
    }

    fn parse_type_ref_str_inner_with_prelude_spelling(
        &mut self,
        type_ref_str: &str,
        generic_params: &[&str],
        suppressed_external_prefixes: &[&str],
        preserve_prelude_spelling: bool,
    ) -> Result<Type, NewTypeGraphCodecError> {
        // Prelude spelling is a rendering concern, not a resolution escape hatch. A bare
        // prelude name that has multiple local candidates is ambiguous even when the caller
        // asks us to preserve the source spelling (for example, in a type-alias where clause).

        // The document crate is a local namespace even when a catalogue uses its explicit
        // crate-qualified spelling. Suppress it during parser discovery so a local
        // `domain::module::Input` reference never leaks `domain` into `external_crates`.
        let own_crate_name = self.crate_name.as_str().to_owned();
        let mut suppressed_prefixes = suppressed_external_prefixes.to_vec();
        if !suppressed_prefixes.contains(&own_crate_name.as_str()) {
            suppressed_prefixes.push(own_crate_name.as_str());
        }
        let suppressed_external_prefixes = suppressed_prefixes.as_slice();
        let std_crate_id = self
            .ext_name_to_id
            .get("std")
            .copied()
            .unwrap_or_else(|| self.ensure_external_crate("std".to_string()));

        // --- Pass 1: discover new external crate names ---
        let local_paths = self.resolution_paths.clone();
        let local_identity_to_id = self.local_identity_to_id.clone();
        let local_crate_name = self.crate_name.clone();
        let ext_snapshot = self.external_crate_ids_without_prefixes(suppressed_external_prefixes);
        let mut new_crate_names: Vec<String> = vec![];

        let _ = Self::parse_type_ref_with_context(
            type_ref_str,
            &|name: &str| {
                local_id_from_canonical_identity(
                    name,
                    generic_params,
                    &local_crate_name,
                    &local_paths,
                    &local_identity_to_id,
                )
            },
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
            preserve_prelude_spelling,
        )
        .map_err(|reason| invalid_type_ref(type_ref_str, reason))?;

        // Register any new external crate names before the encoding pass.
        for crate_name in new_crate_names {
            self.ensure_external_crate(crate_name);
        }

        // --- Pass 2: encode with complete crate-id map ---
        let ext_snapshot2 = self.external_crate_ids_without_prefixes(suppressed_external_prefixes);

        let raw_type = Self::parse_type_ref_with_context(
            type_ref_str,
            &|name: &str| {
                local_id_from_canonical_identity(
                    name,
                    generic_params,
                    &local_crate_name,
                    &local_paths,
                    &local_identity_to_id,
                )
            },
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
            preserve_prelude_spelling,
        )
        .map_err(|reason| invalid_type_ref(type_ref_str, reason))?;

        // --- Pass 3: post-process ---
        let suppressed_external_crates =
            self.remove_external_crate_prefixes(suppressed_external_prefixes);
        let resolved = self.resolve_external_type_ids(raw_type);
        self.restore_external_crate_prefixes(suppressed_external_crates);
        if let Some(error) = self.resolution_error.take() {
            return Err(error);
        }
        self.reconcile_type_ref_identity(type_ref_str, generic_params)?;
        Ok(resolved)
    }

    fn reconcile_type_ref_identity(
        &self,
        type_ref_str: &str,
        generic_names: &[&str],
    ) -> Result<(), NewTypeGraphCodecError> {
        let type_ref = TypeRef::new(type_ref_str.to_owned())
            .map_err(|_| invalid_type_ref(type_ref_str, "empty TypeRef"))?;
        let generic_params = generic_names
            .iter()
            .map(|name| {
                ParamName::new((*name).to_owned())
                    .map_err(|_| invalid_type_ref(type_ref_str, "invalid generic parameter"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        canonicalize_catalogue_type_ref(
            &type_ref,
            &self.crate_name,
            &self.resolution_paths,
            &generic_params,
        )
        .map(|_| ())
    }

    fn parse_type_ref_with_context<F, G>(
        type_ref_str: &str,
        resolve_local: &F,
        std_crate_id: u32,
        external_crate_ids: &HashMap<String, u32>,
        emit_external_crate: &mut G,
        generic_params: &[&str],
        preserve_prelude_spelling: bool,
    ) -> Result<Type, String>
    where
        F: Fn(&str) -> Option<Id>,
        G: FnMut(String) -> u32,
    {
        if preserve_prelude_spelling {
            parse_type_ref_with_generics_preserving_spelling(
                type_ref_str,
                resolve_local,
                std_crate_id,
                external_crate_ids,
                emit_external_crate,
                generic_params,
            )
        } else if generic_params.is_empty() {
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
    /// Uses `parse_generic_bound_with_generics` (which parses via `syn::TypeParamBound`) so that
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
    ///   the `parse_generic_bound_with_generics` fallback covers this case via `Err` propagation.)
    ///
    /// # Errors
    ///
    /// Returns `NewTypeGraphCodecError` if the bound string cannot be
    /// parsed as a `TypeParamBound` by `syn`.
    pub(super) fn encode_bound_str_with_generics(
        &mut self,
        bound_str: &str,
        generic_names: &[&str],
    ) -> Result<GenericBound, NewTypeGraphCodecError> {
        self.encode_bound_str_inner(bound_str, &[], generic_names, false)
    }

    pub(super) fn encode_bound_str_with_generics_preserving_spelling(
        &mut self,
        bound_str: &str,
        generic_names: &[&str],
    ) -> Result<GenericBound, NewTypeGraphCodecError> {
        self.encode_bound_str_inner(bound_str, &[], generic_names, true)
    }

    pub(super) fn encode_bound_str_with_suppressed_external_prefixes_and_generics(
        &mut self,
        bound_str: &str,
        suppressed_external_prefixes: &[&str],
        generic_names: &[&str],
    ) -> Result<GenericBound, NewTypeGraphCodecError> {
        self.encode_bound_str_inner(bound_str, suppressed_external_prefixes, generic_names, false)
    }

    fn encode_bound_str_inner(
        &mut self,
        bound_str: &str,
        suppressed_external_prefixes: &[&str],
        generic_names: &[&str],
        preserve_prelude_spelling: bool,
    ) -> Result<GenericBound, NewTypeGraphCodecError> {
        // Handle `~const` prefix manually because stable syn v2 does not have a
        // `TraitBoundModifier::MaybeConst` variant.  Strip the prefix and encode
        // the remainder as a plain trait bound with MaybeConst modifier.
        if let Some(inner) = bound_str.strip_prefix("~const ") {
            let inner = inner.trim_start();
            // Encode the inner trait path with the same spelling policy as an
            // ordinary bound (no modifier prefix).
            let ty = self.parse_type_ref_str_inner_with_prelude_spelling(
                inner,
                generic_names,
                suppressed_external_prefixes,
                preserve_prelude_spelling,
            )?;
            let trait_path = match ty {
                Type::ResolvedPath(p) => p,
                other => {
                    return Err(invalid_type_ref(
                        bound_str,
                        format!(
                            "~const bound must resolve to a trait path, got {:?}",
                            std::mem::discriminant(&other)
                        ),
                    ));
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
        let local_paths = self.resolution_paths.clone();
        let local_identity_to_id = self.local_identity_to_id.clone();
        let local_crate_name = self.crate_name.clone();

        // Pass 1: discover new external crate names (same two-pass strategy as parse_type_ref_str).
        {
            let ext_snapshot =
                self.external_crate_ids_without_prefixes(suppressed_external_prefixes);
            let mut new_crate_names: Vec<String> = vec![];
            let parse_bound = if preserve_prelude_spelling {
                parse_generic_bound_with_generics_preserving_spelling
            } else {
                parse_generic_bound_with_generics
            };
            let _ = parse_bound(
                bound_str,
                &|name: &str| {
                    local_id_from_canonical_identity(
                        name,
                        generic_names,
                        &local_crate_name,
                        &local_paths,
                        &local_identity_to_id,
                    )
                },
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
                generic_names,
            )
            .map_err(|reason| invalid_type_ref(bound_str, reason))?;
            for crate_name in new_crate_names {
                self.ensure_external_crate(crate_name);
            }
        }

        // Pass 2: encode with complete crate-id map.
        let ext_snapshot2 = self.external_crate_ids_without_prefixes(suppressed_external_prefixes);
        let parse_bound = if preserve_prelude_spelling {
            parse_generic_bound_with_generics_preserving_spelling
        } else {
            parse_generic_bound_with_generics
        };
        let bound = parse_bound(
            bound_str,
            &|name: &str| {
                local_id_from_canonical_identity(
                    name,
                    generic_names,
                    &local_crate_name,
                    &local_paths,
                    &local_identity_to_id,
                )
            },
            std_crate_id,
            &ext_snapshot2,
            &mut |crate_name: String| {
                if is_suppressed_external_prefix(&crate_name, suppressed_external_prefixes) {
                    u32::MAX - 1
                } else {
                    self.ensure_external_crate(crate_name)
                }
            },
            generic_names,
        )
        .map_err(|reason| invalid_type_ref(bound_str, reason))?;

        let resolved = self.resolve_external_type_ids_in_generic_bound(bound);
        if let Some(error) = self.resolution_error.take() {
            return Err(error);
        }
        let identity_source = bound_str.strip_prefix('?').unwrap_or(bound_str).trim();
        if parse_syn_type(identity_source).is_ok() {
            self.reconcile_type_ref_identity(identity_source, generic_names)?;
        }
        Ok(resolved)
    }

    /// Encodes a `MethodGenericParam.bounds[i]` or `WherePredicateDecl.bounds[i]` entry.
    ///
    /// All `syn`-parseable bound strings are accepted regardless of kind: lifetime
    /// bounds (`'static`, `'a`), HRTB (`for<'a> Fn(&'a T)`), precise-capture
    /// (`use<'a, T>`), and plain trait bounds (ADR `2026-05-18-1223` D1).
    /// Bounds that `syn` cannot parse are propagated as `Err`.
    pub(super) fn encode_and_validate_bound(
        &mut self,
        bound_str: &str,
        generic_names: &[&str],
    ) -> Result<GenericBound, NewTypeGraphCodecError> {
        self.encode_bound_str_with_generics(bound_str, generic_names)
    }

    pub(super) fn encode_and_validate_bound_preserving_spelling(
        &mut self,
        bound_str: &str,
        generic_names: &[&str],
    ) -> Result<GenericBound, NewTypeGraphCodecError> {
        self.encode_bound_str_with_generics_preserving_spelling(bound_str, generic_names)
    }
}

fn is_suppressed_external_prefix(crate_name: &str, suppressed_external_prefixes: &[&str]) -> bool {
    suppressed_external_prefixes.contains(&crate_name)
}

fn local_id_from_canonical_identity(
    name: &str,
    generic_params: &[&str],
    catalogue_crate: &domain::tddd::catalogue_v2::identifiers::CrateName,
    rustdoc_paths: &HashMap<Id, rustdoc_types::ItemSummary>,
    local_identity_to_id: &HashMap<
        crate::tddd::canonical_type_identity::CanonicalTypeIdentity,
        Vec<(domain::tddd::catalogue_v2::identifiers::FullyQualifiedItemPath, Id)>,
    >,
) -> Option<Id> {
    if generic_params.contains(&name) {
        return None;
    }
    let type_ref = TypeRef::new(name.to_owned()).ok()?;
    let generic_params = generic_params
        .iter()
        .map(|name| ParamName::new((*name).to_owned()).ok())
        .collect::<Option<Vec<_>>>()?;
    let identity =
        canonicalize_catalogue_type_ref(&type_ref, catalogue_crate, rustdoc_paths, &generic_params)
            .ok()?;
    local_identity_to_id.get(&identity).and_then(|entries| match entries.as_slice() {
        [(_, id)] => Some(*id),
        _ => None,
    })
}
