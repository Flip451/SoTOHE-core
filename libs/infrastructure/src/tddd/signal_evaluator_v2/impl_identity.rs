//! Impl-block identity map construction helpers.
//!
//! Provides [`build_impl_identity_map`], [`is_compiler_internal_trait`], and
//! supporting utilities used by Phase 1 and
//! Phase 2 to build the `(impl_identity_string, Id)` map for trait `Impl` items.

use std::collections::BTreeMap;

use domain::tddd::Phase1Error;
use domain::tddd::catalogue_v2::identifiers::CrateName;
use rustdoc_types::{Crate, GenericArgs, Id, ItemEnum, ItemSummary, Path, Type};

use crate::tddd::canonical_type_identity::{
    DefinitionPathAuthority, canonicalize_rustdoc_generic_args_with_authority,
    canonicalize_rustdoc_path, canonicalize_rustdoc_type_with_authority,
};
use crate::tddd::type_ref_parser::{core_canonical_path, render_type};

use super::impl_identity_helpers::{
    render_identity_generic_args, strip_impl_params_args, strip_impl_params_type,
};

/// Normalized path forms (both qualified and bare) for compiler-internal phantom
/// marker traits whose `Impl` blocks are excluded from the identity map.
///
/// These traits are emitted automatically by the Rust compiler as proxies for
/// `#[derive(PartialEq)]` / `#[derive(Eq)]` and have no stable definition that
/// a catalogue can declare — they cannot be hand-written or declared via
/// `trait_impls`.  Including them in the `CMinusSUnionD` region would produce
/// permanent noise signals that no catalogue declaration could resolve.
///
/// This is distinct from a provenance-based filter: per parent ADR
/// `2026-05-08-0305` D9, Phase 2's structural-equality judgement does NOT
/// distinguish derive-generated impls from hand-written ones.  Adopters of this
/// template are expected to declare every trait impl (derive or hand-written)
/// via `trait_impls` in their workspace catalogue.  The two compiler-internal
/// traits listed here are the only exception, on the grounds that they are
/// not even nameable from user code.
///
/// Multiple normalized forms are listed to cover all code paths in
/// `build_impl_identity_map`:
///
/// - `"core::marker::Structural*"` — standard multi-segment path from `krate.paths`
/// - `"std::marker::Structural*"` — std re-export (some rustdoc versions)
/// - `"core::Structural*"` — two-segment fallback from `core_canonical_path`
/// - `"std::Structural*"` — two-segment fallback from `std_canonical_path` (std external-crate path)
/// - `"StructuralPartialEq"` / `"StructuralEq"` — bare short name from the
///   `normalize_impl_trait_path` fallback when the trait ID is absent from
///   `krate.paths` (some rustdoc variants).  These are the only two names in the
///   entire Rust ecosystem with these exact identifiers; user-defined traits
///   sharing them are theoretically possible but would be indistinguishable from
///   the compiler-internal ones at this level of normalization.
///
/// A third-party crate trait with a different short name (e.g. `"foo::StructuralXxx"`)
/// is **not** listed here and is therefore never excluded.
const COMPILER_INTERNAL_TRAIT_PATHS: &[&str] = &[
    // Qualified paths (primary krate.paths code path)
    "core::marker::StructuralPartialEq",
    "core::marker::StructuralEq",
    "core::clone::TrivialClone",
    "std::marker::StructuralPartialEq",
    "std::marker::StructuralEq",
    "std::clone::TrivialClone",
    // Two-segment fallback (core_canonical_path / std_canonical_path for unrecognised names)
    "core::StructuralPartialEq",
    "core::StructuralEq",
    "core::TrivialClone",
    "std::StructuralPartialEq",
    "std::StructuralEq",
    "std::TrivialClone",
    // Bare short-name fallback (normalize_impl_trait_path when ID absent from krate.paths)
    "StructuralPartialEq",
    "StructuralEq",
    "TrivialClone",
];

/// Returns `true` when the normalized trait path matches one of the compiler-internal
/// phantom marker trait forms that have no stable catalogue declaration and must be
/// excluded from the identity set as a rustdoc artefact.
///
/// Matching covers both qualified forms (the primary `krate.paths` code path) and
/// the bare short-name form (the `normalize_impl_trait_path` fallback for trait IDs
/// absent from `krate.paths`).  Third-party traits sharing only the short name but
/// from a different module (e.g. `"foo::StructuralEq"`) are NOT in the list and
/// are therefore never excluded.
///
/// Used by [`build_impl_identity_map`] to exclude rustdoc-emitted compiler
/// internals so they do not generate permanent `CMinusSUnionD` noise.
pub(crate) fn is_compiler_internal_trait(normalized_trait_name: &str) -> bool {
    let without_generics = normalized_trait_name.split('<').next().unwrap_or(normalized_trait_name);
    COMPILER_INTERNAL_TRAIT_PATHS.contains(&without_generics)
        || COMPILER_INTERNAL_TRAIT_PATHS.contains(&core_canonical_path(without_generics).as_str())
}

/// Builds a `(impl_identity_string, Id)` map for ordinary trait `Impl` items
/// in a crate.
///
/// Identity key format: `"FullyQualifiedForType: fully_qualified_trait[<GenericArgs>]"`.
///
/// Both sides resolve `for_` and `trait_` through their `Crate::paths` universe.
/// The report layer may shorten an unambiguous key for display, but the map used
/// for action matching and three-way comparison never uses a short name as its
/// identity authority.
///
/// `crate_name` is the name of the crate being indexed, used to distinguish
/// local-crate trait paths (e.g. `my_crate::MyTrait`) from external crate paths
/// (e.g. `serde::Serialize`).  Pass the empty string for A-side (codec) maps
/// where trait paths use `crate::` or bare names rather than the real crate name.
///
/// Trait paths are taken verbatim from the authoritative path summaries. A
/// synthetic path without a summary is retained as a conservative compatibility
/// spelling; it is never used to collapse two path-backed identities.
///
/// Only **explicit, non-blanket, non-negative, non-synthetic trait impls** are
/// included.  Only local-crate impls (crate_id == 0) are included.  Compiler-
/// internal phantom marker traits (`StructuralPartialEq`, `StructuralEq`) are
/// excluded via [`is_compiler_internal_trait`] because they cannot be declared
/// in any workspace catalogue.  Derive-generated impls (e.g. `Clone`, `Debug`)
/// are NOT filtered: per ADR `2026-05-08-0305` D9 the catalogue must declare
/// every trait impl regardless of generation method.
///
/// When two impls produce the same identity key (e.g. two `impl Bar for Foo`
/// blocks in different modules with the same short-name key), the candidates are
/// sorted by `(key, for_path_raw, id)`:
/// - `key` (ascending): primary alphabetic key.
/// - `for_path_raw` (ascending): raw path string of the `for_` type.  Makes
///   collision resolution consistent across S-side and C-side maps for
///   **B-origin orphan impls** (cross-crate impls from the baseline crate,
///   inserted into S by `phase1/builder.rs`), because both sides preserve the
///   same rustdoc-emitted path string.  This matters when a local type and an
///   external type share the same short name (e.g. a local `Error` struct and
///   `std::error::Error`): without the tiebreaker, S and C could each keep a
///   *different* impl (depending on raw Id ordering), causing a spurious
///   structural mismatch in Phase 2.
/// - `id` (ascending): smallest `Id` as a final deterministic tiebreaker.
///
/// The former `priority_ids` parameter (a band-aid that forced A-side `Add` impls
/// to take precedence over B-side `Reference` impls with the same key) has been
/// removed (T015 / ADR `2026-05-20-0048` D4).  Action-driven insertion in Phase 1
/// (`phase1/builder.rs`) now inserts each `TraitImplDeclV2` according to its own
/// declared `action`, so stale B-side impls never shadow A-side impls in S for the
/// same identity key — the duplication problem is resolved structurally rather than
/// through Phase 2 priority tie-breaking.
///
/// **A-origin `for_` path normalization**: Per ADR `2026-05-20-0048` D2,
/// `TraitImplDeclV2.for_type` can express external-crate types via fully-qualified
/// paths (e.g. `"std::vec::Vec<i32>"`).  The catalogue codec stores only the
/// last-segment short name in `Type::ResolvedPath.path` (e.g. `"Vec"`) so that
/// A-origin impls in S produce the same `for_path_raw` as C-side rustdoc output.
/// This invariant ensures the tiebreaker is consistent across S and C for A-origin
/// external-self-type impls.
///
/// ## Why inherent impls are included via `InherentImplDeclV2`
///
/// Per ADR `2026-05-20-0048` D1, `InherentImplDeclV2` is a top-level entry in
/// `CatalogueDocument::inherent_impls`, symmetric with `TraitImplDeclV2`.  Each
/// `InherentImplDeclV2` is assigned an `ItemAction` (e.g. `Add`, `Reference`) and
/// can appear in S (A-sourced) or D.  This function covers **trait impls** only; the
/// corresponding inherent-impl identity map is built separately.
///
/// ## Cross-crate impls (ADR `2026-05-20-0048` D3)
///
/// Per ADR `2026-05-20-0048` D3, the former `for_is_external` filter has been removed.
/// Cross-crate impls (where `for_` is an external type, e.g.
/// `impl From<LocalErr> for external::Error`) are included in the identity map on
/// **both** sides:
/// - C-side: this function includes them (no `for_`-external filter).
/// - S-side: A-sourced impls declare them via `TraitImplDeclV2.for_type` (D2), and
///   the B-side orphan-impl pass in `phase1/builder.rs` also inserts them without any
///   `for_`-external check.
///
/// Symmetric inclusion ensures fingerprints match and no spurious `CMinusSUnionD`
/// Red signals are generated for cross-crate impls.
pub(crate) fn try_build_impl_identity_map_with_authority(
    krate: &Crate,
    crate_name: &str,
    authority: &DefinitionPathAuthority,
) -> Result<BTreeMap<String, Id>, Phase1Error> {
    let has_identity_candidates = krate.index.values().any(|item| {
        matches!(
            &item.inner,
            ItemEnum::Impl(implementation)
                if !implementation.is_negative
                    && !implementation.is_synthetic
                    && implementation.blanket_impl.is_none()
                    && implementation.trait_.is_some()
        )
    });
    if !has_identity_candidates {
        return Ok(BTreeMap::new());
    }
    let catalogue_crate = CrateName::new(crate_name.to_owned()).or_else(|_| {
        let root_name = krate
            .index
            .get(&krate.root)
            .and_then(|root| root.name.clone())
            .ok_or_else(|| {
                Phase1Error::rustdoc_root_resolution(format!(
                    "impl identity map cannot resolve paths without a valid crate root name: `{crate_name}`"
                ))
            })?;
        CrateName::new(root_name).map_err(|_| {
            Phase1Error::rustdoc_root_resolution(
                "impl identity map found an invalid local crate root name",
            )
        })
    })?;

    // Collect candidates: (key, for_path_raw, id) — then sort to make result
    // deterministic.
    //
    // `for_path_raw` is the verbatim path string from the `for_` type's
    // `Type::ResolvedPath.path` field (for other type variants, the formatted
    // short name).  Using it as a secondary sort key ensures that when two impls
    // share the same short-name key (e.g. a local `Error` and `std::error::Error`
    // both producing `"Error: Foo"`), the same impl wins on both the S-side and
    // the C-side — because B-origin orphan impls preserve the rustdoc-emitted path
    // string, keeping the tiebreaker consistent across S and C.
    let mut candidates: Vec<(String, String, Id)> = Vec::new();
    for (id, item) in &krate.index {
        if item.crate_id != 0 {
            continue;
        }
        if let ItemEnum::Impl(impl_) = &item.inner {
            // Skip inherent impls, negative impls, synthetic impls, and blanket impls.
            // See the doc comment above for the rationale on inherent impl exclusion.
            if impl_.is_negative || impl_.is_synthetic || impl_.blanket_impl.is_some() {
                continue;
            }
            let trait_path = match &impl_.trait_ {
                Some(tp) => tp,
                None => continue, // inherent impl — excluded per ADR 3 D3 identity scheme
            };
            // Short name for `for_`, consistent with the ThreeWaySignal contract
            // and with S-side impl construction (B-origin ids in for_.id don't exist
            // in S.paths, so full-path lookup would fall back to format_type anyway).
            //
            // Generic type parameters declared on the impl block itself (e.g. `impl<S>
            // TaskOperationInteractor<S>`) are stripped from the `for_` short name so
            // that the identity key matches the catalogue A-codec key, which uses the
            // bare type name without impl-block type parameters (per ADR D10 trait
            // identity normalization).  Concrete type arguments (e.g. `Vec<u32>`) are
            // preserved because they are part of the structural identity.
            // Collect all impl-block generic parameter names: type params (`T`),
            // lifetime params (`'a`, stored without the leading `'` in
            // `GenericParamDef::name`), and const params (`N`).  All three
            // contribute to `format_type_strip_type_params`'s strip set so that
            // `impl<S>`, `impl<'a>`, and `impl<const N: usize>` are all
            // normalized away from the `for_` key.
            let type_params: std::collections::BTreeSet<String> =
                impl_.generics.params.iter().map(|p| p.name.clone()).collect();
            let for_name = identity_type_text(
                &impl_.for_,
                &krate.paths,
                &type_params,
                &catalogue_crate,
                authority,
            )?;
            // Raw `for_` path used as a secondary sort key for deterministic collision
            // resolution when two impls share the same short-name key (e.g. a local
            // `Error` type and an external `std::error::Error` both producing `"Error:
            // Foo"`).  The verbatim `Type::ResolvedPath.path` string is preserved
            // identically in S-side (B-origin orphan impls) and C-side (rustdoc output),
            // making the tiebreaker consistent across both sides without requiring a
            // `krate.paths` lookup (which is unavailable for remapped S-side external ids).
            //
            // A-origin impls (from `TraitImplDeclV2`) use the short-name form in
            // `ResolvedPath.path` (enforced by the catalogue codec: only the last segment
            // of an external type path is stored, e.g. `"Vec"` not `"std::vec::Vec"`).
            // This invariant makes A-origin `for_path_raw` consistent with C-side output.
            let for_path_raw = identity_type_text(
                &impl_.for_,
                &krate.paths,
                &type_params,
                &catalogue_crate,
                authority,
            )?;

            // Per ADR D4 (catalogue-schema-permissive): the `for_` external-type filter
            // is intentionally absent.  Cross-crate impls such as
            // `impl From<LocalErr> for external::Error` are included in C's identity map
            // symmetrically with S (the B-side orphan-impl pass in `phase1/builder.rs`
            // inserts ALL orphan impls with no `for_`-external check).  Both sides track
            // the same set → fingerprints match → no spurious CMinusSUnionD signal.

            // Resolve the trait path to a canonical identity key.
            //
            // Priority: use `krate.paths` to obtain the fully qualified canonical
            // path for the trait item.  This correctly distinguishes an external
            // `core::fmt::Display` (crate_id != 0 → kept as `"core::fmt::Display"`)
            // from a local `Display` trait (crate_id == 0 → stripped to `"Display"`),
            // preventing false identity-key collisions between user-defined and
            // stdlib/core traits that share a short name.
            //
            // Generic args on the trait (e.g. `From<MyError>`) are NOT part of this base
            // path resolution: they are carried structurally in `trait_path.args` — the
            // codec emits them via `resolve_trait_ref_for_top_level` and rustdoc emits them
            // natively — and are appended to the identity key below via `format_generic_args`.
            // Both the S-side and C-side maps therefore build the same key for the same
            // logical impl without any string-based re-embedding (ADR `2026-05-20-0048` D2).
            let normalized_trait_path =
                canonical_trait_path(trait_path, &krate.paths, &catalogue_crate, authority)?;

            // Skip compiler-internal phantom marker traits (StructuralPartialEq,
            // StructuralEq, TrivialClone). These cannot be declared in any workspace catalogue
            // because they have no stable user-facing name, so they would always
            // appear in `CMinusSUnionD` regardless of catalogue completeness.
            // Per ADR `2026-05-08-0305` D9, derive-generated trait impls (Clone,
            // Debug, etc.) are NOT filtered here — adopters must declare them via
            // `trait_impls` in their workspace catalogue.
            //
            // Guard: only apply the compiler-internal check when the trait comes
            // from a non-workspace external crate (crate_id != 0 in krate.paths AND
            // not from "domain"/"usecase") or when using the string-based fallback
            // (ID not in krate.paths).
            //
            // Workspace crates (domain, usecase) are always catalogue-declarable,
            // so even if their paths are normalized to a bare short name, they must
            // never be silently filtered.  A user-defined LOCAL trait (crate_id == 0)
            // is also never filtered.  The real compiler-internal traits always have
            // crate_id != 0 and come from `core` or `std`.
            let trait_is_filterable = match krate.paths.get(&trait_path.id) {
                None => true,                          // synthetic ID (A-side codec path)
                Some(ps) if ps.crate_id == 0 => false, // local trait
                Some(ps) => {
                    // External crate — check it is not a workspace crate.
                    let ext_name = krate
                        .external_crates
                        .get(&ps.crate_id)
                        .map(|ec| ec.name.as_str())
                        .unwrap_or("");
                    !matches!(ext_name, "domain" | "usecase")
                }
            };
            if trait_is_filterable && is_compiler_internal_trait(&normalized_trait_path) {
                continue;
            }

            // Skip `&str: From<T>` and `&str: From<&T>` impls generated by
            // `strum::IntoStaticStr`.  These have `for_ = &str` (a primitive
            // reference, not a local type) so they are not meaningful catalogue
            // entries.  Rustdoc renders the `for_` side as a bare `&str` type
            // reference, which `format_type` formats as `"str"` (the inner type
            // of the reference).  The identity key is `"str: From<...>"`, so we
            // detect and skip it here rather than filtering all `From` impls
            // globally (which would hide legitimate hand-written `From` impls
            // like `impl From<CatalogueToExtendedCrateCodecError> for SomeError`).
            //
            // When `trait_path.id` is in `krate.paths` (C-side), the normalised path
            // is typically the canonical qualified form: `"core::convert::From"`.
            // When the ID is synthetic (S-side fallback), the catalogue codec emits
            // `"core::convert::From"` via `core_canonical_path`.
            // In rare cases where rustdoc omits the `paths` entry for a core trait,
            // the fallback path may produce just `"From"` or `"From<T>"` (bare).
            // All three forms (`core::convert::From`, `std::convert::From`, bare `From`)
            // are checked here so that strum `IntoStaticStr` side-effect impls are
            // correctly filtered regardless of which form the normaliser produces.
            let is_from_trait = normalized_trait_path == "core::convert::From"
                || normalized_trait_path.starts_with("core::convert::From<")
                || normalized_trait_path == "std::convert::From"
                || normalized_trait_path.starts_with("std::convert::From<")
                || normalized_trait_path == "From"
                || normalized_trait_path.starts_with("From<");
            if (for_name == "str" || for_name == "&str") && is_from_trait {
                continue;
            }

            // Include generic args on the trait, with angle brackets so that
            // `Iterator<Item = u8>` is distinct from a trait named `IteratorItem`.
            let trait_str = if let Some(args) = &trait_path.args {
                let rendered = identity_generic_args(
                    args,
                    &krate.paths,
                    &type_params,
                    &catalogue_crate,
                    authority,
                )?;
                if rendered.is_empty() {
                    normalized_trait_path
                } else {
                    format!("{}<{}>", normalized_trait_path, rendered)
                }
            } else {
                normalized_trait_path
            };
            let key = format!("{for_name}: {trait_str}");
            if !key.is_empty() {
                candidates.push((key, for_path_raw, *id));
            }
        }
    }
    // Sort by (key asc, for_path_raw asc, id asc):
    // - key ascending: primary alphabetic ordering.
    // - for_path_raw ascending: secondary tiebreaker consistent across S-side and C-side.
    // - id ascending: final deterministic tiebreaker.
    candidates.sort_unstable_by(|a, b| {
        a.0.cmp(&b.0) // key ascending
            .then(a.1.cmp(&b.1)) // for_path_raw ascending
            .then(a.2.0.cmp(&b.2.0)) // id ascending
    });
    let mut map: BTreeMap<String, Id> = BTreeMap::new();
    for (key, _for_path_raw, id) in candidates {
        map.entry(key).or_insert(id);
    }
    Ok(map)
}

/// Builds an impl identity map and propagates failures from the canonical
/// rustdoc path resolver.
pub(crate) fn build_impl_identity_map(
    krate: &Crate,
    crate_name: &str,
    authority: &DefinitionPathAuthority,
) -> Result<BTreeMap<String, Id>, Phase1Error> {
    try_build_impl_identity_map_with_authority(krate, crate_name, authority)
}

/// Renders a trait path through the shared definition-path resolver.
fn canonical_trait_path(
    path: &Path,
    paths: &std::collections::HashMap<Id, ItemSummary>,
    crate_name: &CrateName,
    authority: &DefinitionPathAuthority,
) -> Result<String, Phase1Error> {
    canonicalize_rustdoc_path(path, crate_name, paths, authority).map_err(|error| {
        Phase1Error::rustdoc_root_resolution(format!(
            "trait impl identity path `{}` could not be resolved through Crate::paths: {error}",
            path.path
        ))
    })
}

/// Renders an impl owner while retaining the fully-qualified identity of every
/// resolved path and stripping only generic parameters declared on that impl.
fn identity_type_text(
    ty: &Type,
    paths: &std::collections::HashMap<Id, ItemSummary>,
    impl_params: &std::collections::BTreeSet<String>,
    crate_name: &CrateName,
    authority: &DefinitionPathAuthority,
) -> Result<String, Phase1Error> {
    let canonical = canonicalize_rustdoc_type_with_authority(ty, crate_name, paths, authority)
        .map_err(|error| {
            Phase1Error::rustdoc_root_resolution(format!(
                "impl owner identity could not be resolved through Crate::paths: {error}"
            ))
        })?;
    let stripped = strip_impl_params_type(canonical, impl_params);
    render_type(&stripped).ok_or_else(|| {
        Phase1Error::rustdoc_root_resolution(
            "impl owner identity contains a rustdoc type without an authoritative rendering",
        )
    })
}

fn identity_generic_args(
    args: &GenericArgs,
    paths: &std::collections::HashMap<Id, ItemSummary>,
    impl_params: &std::collections::BTreeSet<String>,
    crate_name: &CrateName,
    authority: &DefinitionPathAuthority,
) -> Result<String, Phase1Error> {
    let canonical = canonicalize_rustdoc_generic_args_with_authority(
        args, crate_name, paths, authority,
    )
    .map_err(|error| {
        Phase1Error::rustdoc_root_resolution(format!(
            "trait impl generic identity could not be resolved through Crate::paths: {error}"
        ))
    })?;
    render_identity_generic_args(&strip_impl_params_args(canonical, impl_params))
}
