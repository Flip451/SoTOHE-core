//! `SignalEvaluatorV2` — infrastructure-layer implementation of `SignalEvaluatorPort`.
//!
//! Implements the Phase 1 (S / D construction) + Phase 2 (S / D / C 3-way evaluation)
//! algorithm defined in ADR `2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md`.
//!
//! ## Phase 1 — S / D construction (ADR 3 D2)
//!
//! Inputs: `a: ExtendedCrate` (Catalogue-derived TypeGraph A), `b: rustdoc_types::Crate`
//! (Baseline TypeGraph B).
//!
//! 1. Build identity → Id maps for B (short names for Struct/Enum/TypeAlias/Trait,
//!    FunctionPath strings for Function items via `Crate::paths`).
//! 2. Start S by taking all B items as implicit Reference entries; assign fresh Ids.
//! 3. Apply each A item by its declared action (Add / Modify / Reference / Delete),
//!    returning `Phase1Error::ActionContradiction` on declare inconsistencies.
//! 4. Phase 1.5 — resolve unresolved-marker placeholders (`Id(UNRESOLVED_CRATE_ID)`)
//!    against the closed-world S universe; reject unresolvable names.
//! 5. Phase 1.6 — dangling Id check: verify no Id in S's items points to a deleted item.
//! 6. Rebuild `external_crates` per-scope for S and D.
//!
//! ## Phase 2 — 3-way evaluation (ADR 3 D3)
//!
//! Inputs: S (ExtendedCrate from Phase 1), D (`rustdoc_types::Crate` from Phase 1),
//! C (`rustdoc_types::Crate`, current code).
//!
//! Build identity sets for S, D, and C. For each identity key, determine the
//! `SignalRegion` and emit a `ThreeWaySignal`. Wrap results in `ThreeWayEvaluationReport`.
//!
//! ## Structural equality (ADR 3 D3)
//!
//! Types/traits/functions are compared by converting `rustdoc_types::Type` fields to
//! short-name strings via an internal `format_type` helper (L1 resolution, module paths
//! stripped). This matches the catalogue L1 representation so A-derived and rustdoc-derived
//! items compare symmetrically.
//!
//! ## Module structure
//!
//! - `format`          — `format_type`, `format_generic_args`, `format_generic_bounds`, etc.
//! - `structural_eq`   — `items_structurally_equal` (dispatch + struct/enum comparisons)
//! - `generics_eq`     — `generics_structurally_equal`, `build_trait_method_map`, `fn_sigs_*`
//! - `phase2`          — `phase2_evaluate` and S/D/C region helpers
//! - `resolve_type`    — `resolve_type` and friends (Phase 1.5 Id rewriting)
//! - `collect_refs`    — unresolved-marker scanning + dangling-Id collection
//! - `resolution`      — `resolve_unresolved_in_item` (Phase 1.5 item-level driver)
//! - `external_crates` — `build_external_crates_for_scope`, `patch_paths_crate_ids`
//! - `phase1`          — `Phase1State`, `phase1_build_s_and_d`, child-item helpers
//! - `tests`           — unit/integration tests (AC-08)

use std::collections::BTreeMap;

use domain::tddd::ExtendedCrate;
use domain::tddd::{Phase1Error, SignalEvaluatorPort, ThreeWayEvaluationReport};
use rustdoc_types::{Crate, Id, Item, ItemEnum, ItemKind, Type};

// ---------------------------------------------------------------------------
// Sub-modules
// ---------------------------------------------------------------------------

pub(super) mod collect_refs;
pub(super) mod external_crates;
pub(super) mod format;
pub(super) mod generics_eq;
pub(super) mod phase1;
pub(super) mod phase2;
pub(super) mod resolution;
pub(super) mod resolve_type;
pub(super) mod structural_eq;

#[cfg(test)]
pub(super) mod tests;

use format::{format_generic_args, format_type, format_type_strip_type_params};
use phase1::phase1_build_s_and_d;
use phase2::phase2_evaluate;

// ---------------------------------------------------------------------------
// SignalEvaluatorV2 — stateless secondary adapter
// ---------------------------------------------------------------------------

/// Stateless secondary adapter that implements [`SignalEvaluatorPort`].
///
/// Drives the two-phase evaluation: Phase 1 builds S (`ExtendedCrate`) + D
/// (`rustdoc_types::Crate`) from the Catalogue-derived A and the Baseline B;
/// Phase 2 evaluates S / D / C to emit `ThreeWaySignal`s.
///
/// Construct with [`SignalEvaluatorV2::new`] and call
/// [`SignalEvaluatorPort::evaluate`].
#[derive(Debug, Clone, Default)]
pub struct SignalEvaluatorV2;

impl SignalEvaluatorV2 {
    /// Creates a new `SignalEvaluatorV2`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl SignalEvaluatorPort for SignalEvaluatorV2 {
    fn evaluate(
        &self,
        a: ExtendedCrate,
        b: Crate,
        c: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        let engine = EvaluationEngine::new(a, b, c);
        engine.run()
    }
}

// ---------------------------------------------------------------------------
// Identity helpers (shared across phase1 and phase2 submodules)
// ---------------------------------------------------------------------------

/// Build a `(short_name, Id)` map for types and traits in a `rustdoc_types::Crate`.
///
/// Identity key: short name (last path segment, e.g. `"User"`) for
/// `ItemEnum::Struct | Enum | TypeAlias | Trait`.  Items not matching these
/// kinds are skipped.
///
/// Used in Phase 1 where the catalogue operates at L1 (short-name) resolution
/// and for Phase 1 action matching between A and B.
///
/// When two items share the same short name (same-name types in different
/// modules), the item whose full path in `krate.paths` is lexicographically
/// smaller is preferred so that the result is deterministic regardless of
/// `HashMap` iteration order.
pub(super) fn build_type_trait_identity_map(krate: &Crate) -> BTreeMap<String, Id> {
    // Collect candidates: (short_name, full_path_string, id).
    let mut candidates: Vec<(String, String, Id)> = Vec::new();
    for (id, item) in &krate.index {
        // Only include local crate items (crate_id == 0 means "this crate").
        if item.crate_id != 0 {
            continue;
        }
        if is_type_or_trait_item(item) {
            if let Some(name) = &item.name {
                if !name.is_empty() {
                    let full_path =
                        krate.paths.get(id).map(|s| s.path.join("::")).unwrap_or_default();
                    candidates.push((name.clone(), full_path, *id));
                }
            }
        }
    }
    // Sort by (short_name, full_path, id) so that for each short name, the
    // lexicographically smallest full path wins — deterministic across crates.
    // The third key (id.0: u32) breaks ties when two items share the same
    // short name and full path (e.g. both have an empty path because neither
    // appears in krate.paths), preventing sort_unstable from producing
    // non-deterministic output that would cause a type/trait name collision to
    // flip between Yellow and Red across CI runs.
    candidates.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.0.cmp(&b.2.0)));
    let mut map: BTreeMap<String, Id> = BTreeMap::new();
    for (name, _, id) in candidates {
        // entry().or_insert keeps the first (= lexicographically smallest path).
        map.entry(name).or_insert(id);
    }
    map
}

/// Build a `(function_path_string, Id)` map for free function items in a `rustdoc_types::Crate`.
///
/// Identity key: `FunctionPath` = path segments joined by `"::"` (e.g.
/// `"my_crate::module::fn_name"`), looked up from `Crate::paths`.
///
/// Only **free** functions are included.  Associated methods (belonging to a
/// `Trait` or `Impl` `items` list) are explicitly excluded even when they
/// appear in `Crate::paths`: trait-method structural equality is captured at
/// the trait/impl level, and duplicating methods here would cause spurious or
/// double-counted Phase 2 signals.
pub(super) fn build_function_identity_map(krate: &Crate) -> BTreeMap<String, Id> {
    use std::collections::HashSet;
    // Build the set of all method Ids that belong to a trait or impl's items list.
    // Functions in this set are associated methods, not free functions.
    let method_ids: HashSet<Id> = krate
        .index
        .values()
        .flat_map(|item| match &item.inner {
            ItemEnum::Trait(t) => t.items.as_slice(),
            ItemEnum::Impl(i) => i.items.as_slice(),
            _ => &[],
        })
        .copied()
        .collect();

    let mut map: BTreeMap<String, Id> = BTreeMap::new();
    for (id, item) in &krate.index {
        // Only include local crate items (crate_id == 0 means "this crate").
        if item.crate_id != 0 {
            continue;
        }
        // Skip methods: they are part of their containing trait/impl structure.
        if method_ids.contains(id) {
            continue;
        }
        if matches!(item.inner, ItemEnum::Function(_)) {
            if let Some(summary) = krate.paths.get(id) {
                let path_str = summary.path.join("::");
                if !path_str.is_empty() {
                    map.insert(path_str, *id);
                }
            }
        }
    }
    map
}

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
pub(super) fn is_compiler_internal_trait(normalized_trait_name: &str) -> bool {
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
/// blocks in different modules with the same short-name key), the one whose
/// `Id` value is smallest is kept so the result is deterministic regardless of
/// `HashMap` iteration order.
///
/// ## Why inherent impls are excluded
///
/// Per ADR 3 D3, Phase 2 identity is defined as:
/// - types / traits → short name
/// - functions     → FunctionPath
///
/// Inherent impls (`impl Foo { ... }`) are not independently declared in the
/// catalogue; their existence is expressed through the containing type's shape.
/// The catalogue does not have an `ItemAction` for individual inherent impl blocks,
/// so they cannot appear in S or D and have no valid identity key.  Including
/// them here would generate spurious `CMinusSUnionD` signals for every inherent
/// impl in C, which is not the intent of the spec.
pub(super) fn build_impl_identity_map(krate: &Crate, crate_name: &str) -> BTreeMap<String, Id> {
    // Collect candidates: (key, id) — then sort to make result deterministic.
    let mut candidates: Vec<(String, Id)> = Vec::new();
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

            // Skip impls where `for_` is an external type (belongs to another crate).
            //
            // Cross-crate impls such as `impl From<LocalErr> for external::Error` appear
            // in C's rustdoc (crate_id == 0 for the Impl item itself) but have no
            // corresponding catalogue entry because the `for_` type is not owned by this
            // crate.  Including them would generate spurious `CMinusSUnionD` Red signals.
            //
            // Detection: if `impl_.for_` is a `ResolvedPath` whose Id appears in
            // `krate.paths` with `crate_id != 0`, the target type is external.
            // When the Id is absent from `krate.paths` (e.g. A-side synthetic Ids for
            // which no paths entry was created) the type is treated as local (conservative,
            // avoids false positives on S-side impls encoded by the catalogue codec).
            if let Type::ResolvedPath(p) = &impl_.for_ {
                let for_is_external = krate.paths.get(&p.id).is_some_and(|ps| ps.crate_id != 0);
                if for_is_external {
                    continue;
                }
            }

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
            // When `trait_path.path` contains `<` (i.e. the catalogue codec embedded
            // generic args in the path string for a `TraitImplDeclV2` with `generic_args`),
            // AND `trait_path.args` is `None`, the generic args are only present in
            // `trait_path.path` — not in `trait_path.args`.  In that case we must use
            // the fallback path (`normalize_impl_trait_path`) even when the trait ID is
            // found in `krate.paths`, because `krate.paths` only stores the bare module
            // path without generic arguments.  Using the bare path would produce an
            // identity key without generics (e.g. `"Foo: core::convert::From"`) that
            // the phase-2 stripped-key fallback would then match against any
            // `From<T>` impl — defeating the per-type specificity of the declared key.
            let normalized_trait_path = if krate.paths.contains_key(&trait_path.id)
                && trait_path.args.is_none()
                && trait_path.path.contains('<')
            {
                // S-side path with inline generic args (e.g. "core::convert::From<CatalogueLoaderError>").
                // The krate.paths entry only has the bare path; use the string-based fallback
                // to preserve the generic suffix verbatim.
                normalize_impl_trait_path(&trait_path.path, crate_name)
            } else if let Some(ps) = krate.paths.get(&trait_path.id) {
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
                candidates.push((key, *id));
            }
        }
    }
    // Sort by (key, id) so that for each key the smallest Id wins — deterministic
    // across crates regardless of HashMap iteration order.
    candidates.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.0.cmp(&b.1.0)));
    let mut map: BTreeMap<String, Id> = BTreeMap::new();
    for (key, id) in candidates {
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
pub(super) fn normalize_impl_trait_path(path: &str, crate_name: &str) -> String {
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
fn split_generic_args(path: &str) -> (&str, &str) {
    if let Some(pos) = path.find('<') { (&path[..pos], &path[pos..]) } else { (path, "") }
}

/// Returns `true` if the item is a type (Struct/Enum/TypeAlias) or a Trait.
pub(super) fn is_type_or_trait_item(item: &Item) -> bool {
    matches!(
        item.inner,
        ItemEnum::Struct(_) | ItemEnum::Enum(_) | ItemEnum::TypeAlias(_) | ItemEnum::Trait(_)
    )
}

/// Derives the `ItemKind` corresponding to an item's `inner` variant.
///
/// Used to record the correct kind in `ItemSummary` entries rather than
/// hardcoding `ItemKind::Struct` for every item.
pub(super) fn item_kind_from_inner(inner: &ItemEnum) -> ItemKind {
    match inner {
        ItemEnum::Struct(_) => ItemKind::Struct,
        ItemEnum::Enum(_) => ItemKind::Enum,
        ItemEnum::TypeAlias(_) => ItemKind::TypeAlias,
        ItemEnum::Trait(_) => ItemKind::Trait,
        ItemEnum::Function(_) => ItemKind::Function,
        ItemEnum::Module(_) => ItemKind::Module,
        ItemEnum::Variant(_) => ItemKind::Variant,
        ItemEnum::StructField(_) => ItemKind::StructField,
        ItemEnum::Impl(_) => ItemKind::Impl,
        _ => ItemKind::Primitive, // safe fallback for uncommon kinds
    }
}

/// Returns `true` for paths that carry the unresolved-crate-id sentinel and are
/// local (not from an already-resolved external crate).
///
/// A path is considered local-unresolved when:
/// - It has no `::` (bare identifier, e.g. `MyType`), OR
/// - It starts with `crate::`, `self::`, or `super::` (relative path segments).
///
/// Paths that contain `::` but do NOT start with these keywords (e.g. `std::vec::Vec`)
/// were resolved to an external crate by the codec and must not be re-flagged here.
pub(super) fn is_local_unresolved_path(path: &str) -> bool {
    !path.contains("::")
        || path.starts_with("crate::")
        || path.starts_with("self::")
        || path.starts_with("super::")
}

// ---------------------------------------------------------------------------
// Per-evaluation engine
// ---------------------------------------------------------------------------

/// Per-call evaluation state.
///
/// Owns all intermediate data: S, D, and C.
struct EvaluationEngine {
    a: ExtendedCrate,
    b: Crate,
    c: Crate,
}

impl EvaluationEngine {
    fn new(a: ExtendedCrate, b: Crate, c: Crate) -> Self {
        Self { a, b, c }
    }

    fn run(self) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        let (s, d) = phase1_build_s_and_d(self.a, &self.b)?;
        let report = phase2_evaluate(&s, &d, &self.c);
        Ok(report)
    }
}
