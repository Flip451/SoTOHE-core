//! Impl-block identity map construction helpers.
//!
//! Provides [`build_impl_identity_map`], [`normalize_impl_trait_path`],
//! [`is_compiler_internal_trait`], and supporting utilities used by Phase 1 and
//! Phase 2 to build the `(impl_identity_string, Id)` map for trait `Impl` items.

use std::collections::BTreeMap;

use rustdoc_types::{Crate, Id, ItemEnum};

use super::format::{format_generic_args, format_type, format_type_strip_type_params};
use super::is_local_unresolved_path;

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
    "std::marker::StructuralPartialEq",
    "std::marker::StructuralEq",
    // Two-segment fallback (core_canonical_path / std_canonical_path for unrecognised names)
    "core::StructuralPartialEq",
    "core::StructuralEq",
    "std::StructuralPartialEq",
    "std::StructuralEq",
    // Bare short-name fallback (normalize_impl_trait_path when ID absent from krate.paths)
    "StructuralPartialEq",
    "StructuralEq",
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
}

/// Builds a `(impl_identity_string, Id)` map for ordinary trait `Impl` items
/// in a crate.
///
/// Identity key format: `"ForTypeName: normalized_trait_path[<GenericArgs>]"`.
///
/// `for_` uses the short name from `format_type` (last path segment) to match
/// the `ThreeWaySignal` domain contract (short-name identity for types/traits)
/// and to ensure consistent matching between S-side impls (which carry B-origin
/// ids in `for_.id` that may not exist in S's paths map) and C-side impls.
///
/// `crate_name` is the name of the crate being indexed, used to distinguish
/// local-crate trait paths (e.g. `my_crate::MyTrait`) from external crate paths
/// (e.g. `serde::Serialize`).  Pass the empty string for A-side (codec) maps
/// where trait paths use `crate::` or bare names rather than the real crate name.
///
/// Trait path normalization (via `normalize_impl_trait_path`):
/// - Local-crate trait paths (`crate::MyTrait`, bare `MyTrait`,
///   `{crate_name}::MyTrait`) are reduced to their last segment so that S-side
///   codec paths and C-side rustdoc paths produce the same key.
/// - External crate paths (e.g. `serde::Serialize`) are preserved verbatim to
///   prevent collisions between distinct traits sharing the same short name.
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
pub(crate) fn build_impl_identity_map(krate: &Crate, crate_name: &str) -> BTreeMap<String, Id> {
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
            let for_name = if type_params.is_empty() {
                format_type(&impl_.for_)
            } else {
                format_type_strip_type_params(&impl_.for_, &type_params)
            };
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
            let for_path_raw: String = match &impl_.for_ {
                rustdoc_types::Type::ResolvedPath(p) => p.path.clone(),
                other => format_type(other),
            };

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
            // Fallback: when `trait_path.id` has no entry in `krate.paths` (A-side
            // codec-generated synthetic Ids), fall back to `normalize_impl_trait_path`
            // for string-based normalisation.  The catalogue codec always emits
            // fully-qualified paths for external traits (e.g. `"core::convert::From"`)
            // so the fallback path produces the same canonical form.
            //
            // Generic args on the trait (e.g. `From<MyError>`) are NOT part of this base
            // path resolution: they are carried structurally in `trait_path.args` — the
            // codec emits them via `resolve_trait_ref_for_top_level` and rustdoc emits them
            // natively — and are appended to the identity key below via `format_generic_args`.
            // Both the S-side and C-side maps therefore build the same key for the same
            // logical impl without any string-based re-embedding (ADR `2026-05-20-0048` D2).
            let normalized_trait_path = if let Some(ps) = krate.paths.get(&trait_path.id) {
                if ps.crate_id != 0 {
                    // External trait — use the canonical qualified path from krate.paths.
                    // This is the same form that the catalogue codec emits for S-side
                    // impls (e.g. `"core::convert::From"`, `"serde::Serialize"`), so
                    // S-side and C-side keys are consistent.
                    //
                    // Some rustdoc versions emit short-form path segments (e.g. `["From"]`)
                    // for well-known core/std traits even when `crate_id != 0`.  In that
                    // case `join("::")` produces `"From"` (bare) while the S-side codec
                    // always emits the fully-qualified `"core::convert::From"`.  To keep
                    // both sides consistent, expand bare single-segment paths for known
                    // core traits via `core_canonical_path`.
                    let joined = ps.path.join("::");
                    if !joined.contains("::") {
                        // Single-segment path from krate.paths: the module prefix was
                        // omitted by rustdoc.  Reconstruct the qualified path using the
                        // actual external crate name from `krate.external_crates`.
                        //
                        // Rustdoc sometimes emits a bare single-segment path (e.g. `["From"]`)
                        // for traits from core/std/alloc.  The S-side codec emits canonical
                        // qualified paths (`"core::convert::From"` / `"std::convert::From"`),
                        // so we must expand single-segment paths using the same canonical
                        // function the S-side codec uses for that crate:
                        //   - `std`  → `std_canonical_path` (e.g. `"std::convert::From"`)
                        //   - `core` → `core_canonical_path` (e.g. `"core::convert::From"`)
                        //   - `alloc`→ `core_canonical_path` (alloc shares core module paths)
                        //   - other  → `"{crate_name}::{short_name}"` (e.g. `"serde::Serialize"`)
                        let ext_crate_name = krate
                            .external_crates
                            .get(&ps.crate_id)
                            .map(|ec| ec.name.as_str())
                            .unwrap_or("core");
                        match ext_crate_name {
                            "std" => crate::tddd::type_ref_parser::std_canonical_path(&joined),
                            "core" | "alloc" => {
                                crate::tddd::type_ref_parser::core_canonical_path(&joined)
                            }
                            // Workspace crates (domain, usecase): the S-side codec emits
                            // the bare short trait name.  Return the short name here to
                            // match the multi-segment branch below.
                            "domain" | "usecase" => joined,
                            other => format!("{other}::{joined}"),
                        }
                    } else if let Some(first_seg) = ps.path.first() {
                        // For workspace crates (domain, usecase), the S-side catalogue codec
                        // emits the bare short trait name (to remain consistent when rustdoc
                        // omits the trait from krate.paths and falls back to the raw path
                        // string).  Normalise multi-segment domain/usecase paths to the bare
                        // short name so both sides produce the same identity key.
                        if first_seg == "domain" || first_seg == "usecase" {
                            // ps.path.last() is always Some when ps.path is non-empty.
                            ps.path.last().unwrap_or(first_seg).to_string()
                        } else {
                            joined
                        }
                    } else {
                        joined
                    }
                } else {
                    // Local trait (crate_id == 0) — short name, same as the codec's
                    // `crate::` or bare-name form.
                    ps.path
                        .last()
                        .map(|s| s.as_str())
                        .unwrap_or(trait_path.path.as_str())
                        .to_string()
                }
            } else {
                // ID not in paths: A-side codec-generated synthetic ID or anonymous item.
                // Fall back to string-based normalisation.
                normalize_impl_trait_path(&trait_path.path, crate_name)
            };

            // Skip compiler-internal phantom marker traits (StructuralPartialEq,
            // StructuralEq).  These cannot be declared in any workspace catalogue
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
                let rendered = format_generic_args(args);
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
    map
}

/// Normalizes an impl trait path string for identity-map key construction.
///
/// **Used as a fallback** when `krate.paths` does not contain the trait item's
/// `Id` (A-side codec-generated synthetic Ids, anonymous items).  The primary
/// resolution path in `build_impl_identity_map` uses `krate.paths` to obtain the
/// fully qualified canonical path, which correctly disambiguates local from
/// external traits sharing a short name.
///
/// `crate_name` is the real crate name as it appears in rustdoc paths (e.g.
/// `"my_crate"`).  Pass `""` for A-side (codec) maps where trait paths use
/// `crate::` or bare names rather than the real crate name.
///
/// Normalization rules (fallback only):
/// - Bare identifiers (no `::`) → returned as-is.
/// - `crate::`-, `self::`-, `super::`-prefixed paths → stripped to last segment.
/// - Paths that start with `{crate_name}::` (local crate in rustdoc) → stripped
///   to last segment, so `my_crate::MyTrait` produces `MyTrait` matching codec's
///   `crate::MyTrait` → `MyTrait`.
/// - All other paths → kept verbatim.  The catalogue codec emits qualified forms
///   for external traits (e.g. `"core::convert::From"`, `"serde::Serialize"`) so
///   the string is already in canonical form and requires no further transformation.
pub(crate) fn normalize_impl_trait_path(path: &str, crate_name: &str) -> String {
    if is_local_unresolved_path(path) {
        // Bare name (no `::`) or relative prefix (`crate::`/`self::`/`super::`).
        // Split off any generic args (e.g. `"From<T>"` → base=`"From"`, args=`"<T>"`).
        let (base, args) = split_generic_args(path);
        let short_name = base.rsplit("::").next().unwrap_or(base);
        // Paths with a relative prefix (`crate::X`, `self::X`, `super::X`) are
        // local-crate references; strip to the short name without attempting any
        // core/std expansion, which would produce incorrect results (e.g.
        // `crate::Display` must NOT expand to `core::fmt::Display`).
        if base.contains("::") {
            return format!("{short_name}{args}");
        }
        // Bare identifiers (no `::`) may be well-known core/std traits (e.g. `From`,
        // `Display`).  Expand to the canonical fully-qualified path so the fallback
        // normalisation matches the S-side codec, which always emits
        // `core_canonical_path("From")` = `"core::convert::From"`.
        // Unknown names (local/workspace traits) are kept as the bare short name.
        // `core_canonical_path` falls back to `"core::{name}"` (only two segments)
        // for unrecognised names, so we distinguish known expansions by checking
        // that the result contains at least two `::` separators (three segments).
        // The generic args are re-appended verbatim.
        let expanded_base = crate::tddd::type_ref_parser::core_canonical_path(short_name);
        // `core_canonical_path` falls back to `"core::{name}"` for any name it does
        // not recognise, so the result `"core::{short_name}"` means the name is NOT
        // a known core/std trait.  Only expand when the result has more than two
        // segments (i.e., points to a real sub-module like `core::convert::From`).
        let is_known_core_trait = expanded_base.matches("::").count() >= 2;
        if is_known_core_trait {
            // Recognized core/std trait: use the expanded qualified path.
            format!("{expanded_base}{args}")
        } else {
            // Unknown bare name: keep the short name as before.
            format!("{short_name}{args}")
        }
    } else if !crate_name.is_empty() && path.starts_with(&format!("{crate_name}::")) {
        // rustdoc local trait path (e.g. `my_crate::MyTrait`) → short name.
        path.rsplit("::").next().unwrap_or(path).to_string()
    } else {
        // External or unrecognised path → keep verbatim.
        // The catalogue codec emits fully-qualified paths for external traits
        // (e.g. `"core::convert::From"`, `"serde::Serialize"`), so no transformation
        // is needed for A-side fallback paths.
        path.to_string()
    }
}

/// Splits a trait path string into the base path and a trailing generic-arg suffix.
///
/// Returns `(base, args)` where `args` is the suffix starting at the first `<` (if any),
/// or `(path, "")` if no generic args are present.
///
/// Example: `"From<CatalogueLoaderError>"` → `("From", "<CatalogueLoaderError>")`.
pub(crate) fn split_generic_args(path: &str) -> (&str, &str) {
    if let Some(pos) = path.find('<') { (&path[..pos], &path[pos..]) } else { (path, "") }
}
