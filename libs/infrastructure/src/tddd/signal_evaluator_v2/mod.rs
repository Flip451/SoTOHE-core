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

use format::{format_generic_args, format_type};
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

/// Trait short names whose auto-generated `impl` blocks are excluded from the
/// identity map.
///
/// Rustdoc emits these as ordinary (non-synthetic) `Impl` items in `Crate.index`
/// even though they were generated by `#[derive(...)]`.  Including them in the
/// `CMinusSUnionD` region would produce noise signals for every derived trait on
/// every type in the crate, because the catalogue never declares them explicitly.
///
/// **This list contains exactly these 6 entries** (see the constant below):
/// - `Clone`, `Copy`, `Debug` — standard derives that are **never** hand-written
///   in this codebase.
/// - `StructuralPartialEq`, `StructuralEq` — compiler-internal phantom marker
///   traits emitted automatically for `#[derive(PartialEq)]` / `#[derive(Eq)]`;
///   they are never written by users.
/// - `IntoStaticStr` (strum) — generated only by the `#[derive(strum::IntoStaticStr)]`
///   proc-macro attribute; never hand-written.
///
/// **Intentionally excluded (even though commonly derived):**
/// - `Default` — can be hand-written for types with non-trivial initialization
///   (e.g. configuration structs, types whose zero value would be invalid).
/// - `Eq`, `Hash`, `Ord`, `PartialEq`, `PartialOrd` — can be hand-written in
///   domain types (e.g. custom `PartialEq` to ignore a field).  Filtering them
///   would silently hide deliberate API-contract changes.
/// - `Serialize`, `Deserialize`, `DeserializeOwned` — custom serde impls are
///   common (newtype wrappers, sensitive-field redaction, etc.).
/// - `Display`, `FromStr`, `TryFrom`, `Error`, `AsRef`, `From` — all have
///   hand-written impls on value-object newtypes; filtering them would drop those
///   from the gate.
/// - `Send`, `Sync` — auto-generated forms are already filtered by
///   `impl_.is_synthetic`; only explicit `unsafe impl Send/Sync` remains visible.
///
/// **Selection rule:** add a trait to this list ONLY if it is *exclusively*
/// generated by a derive/proc macro and is NEVER hand-written in this codebase.
///
/// Trait names are matched against the **last path segment** of the normalized
/// trait path, so both a bare `"Debug"` (local) and a fully-qualified
/// `"std::fmt::Debug"` (external, preserved verbatim by `normalize_impl_trait_path`)
/// are correctly excluded.
const DERIVE_TRAIT_NAMES: &[&str] = &[
    "Clone",
    "Copy",
    "Debug",
    // `StructuralPartialEq` and `StructuralEq` are compiler-internal phantom
    // marker traits, never hand-written.
    "StructuralPartialEq",
    "StructuralEq",
    // NOTE: `Send` and `Sync` are intentionally NOT in this list.
    // Auto-generated `Send`/`Sync` impls (from `#[derive(Copy, Clone)]` etc.) are
    // already filtered by `impl_.is_synthetic = true` in rustdoc output.
    // Explicit `unsafe impl Send/Sync for LocalType {}` is a hand-written safety
    // contract and must remain visible so that adding or removing it produces a
    // TDDD signal.
    //
    // NOTE: `Default` is intentionally NOT in this list.  While `Default` is most
    // commonly derived, it can be hand-written to supply non-trivial defaults (e.g.
    // a type whose zero value would be invalid, configuration structs with sensible
    // defaults, or types that initialize internal state on construction).  Filtering
    // it would silently drop any hand-written `impl Default` from the identity map.
    //
    // NOTE: `Eq`, `Hash`, `Ord`, `PartialEq`, `PartialOrd` are intentionally NOT
    // in this list even though they are very commonly derived.  All five can be
    // hand-written for domain types (e.g. custom `PartialEq` to ignore a field,
    // custom `Ord` for business-defined ordering).  Including them would silently
    // drop hand-written impls from the identity map, causing real API-contract
    // changes to produce no TDDD signal.  Undeclared derive-generated impls appear
    // in `CMinusSUnionD`; the catalogue must declare them via `trait_impls` if they
    // are intentional API contracts.
    //
    // NOTE: `Serialize`, `Deserialize`, `DeserializeOwned` are intentionally NOT
    // in this list.  Custom serde implementations are common for domain types that
    // need non-standard serialization (e.g. newtype wrappers, enum dispatch,
    // sensitive-field redaction).  Filtering them globally would hide hand-written
    // serde impls from the gate.
    //
    // strum::IntoStaticStr — generates `impl From<&T> for &'static str` and
    // `impl IntoStaticStr for T` only via the proc-macro attribute; never
    // hand-written.
    "IntoStaticStr",
    // NOTE: `Display`, `FromStr`, `TryFrom`, `Error`, `AsRef`, and `From` are
    // intentionally NOT in this list even though strum/thiserror can derive them.
    // This codebase has hand-written implementations of these traits on non-enum
    // types (value-object newtypes, status types, identifier types).  Including
    // them here would silently drop those hand-written impls from the identity map,
    // causing real API-contract changes to produce no signal.
    //
    // Strum-generated `Display`/`FromStr`/`TryFrom` impls on pure-derive enums will
    // appear in `CMinusSUnionD`, which is the correct behavior: the catalogue must
    // declare them via `trait_impls` if they are intentional API contracts.
];
// NOTE: `From` is intentionally NOT in this list.
// `From<X> for Y` impls are catalogue-relevant (hand-written conversions).
// The `&str: From<T>` and `&str: From<&T>` impls generated by
// `strum::IntoStaticStr` are filtered separately in `build_impl_identity_map`
// via an explicit `for_name == "str"` check, not by filtering the trait name.

/// Returns `true` when the trait name represents a standard derive-macro-only
/// trait that should be excluded from the `CMinusSUnionD` identity set.
///
/// Accepts both bare short names (`"Debug"`) and fully-qualified external paths
/// (`"std::fmt::Debug"`), matching only against the **last path segment** after
/// stripping generic args.  This handles the fact that `normalize_impl_trait_path`
/// preserves external trait paths verbatim (e.g. `std::fmt::Debug`) while
/// local-crate paths are already reduced to their short name.
///
/// Used by [`build_impl_identity_map`] to exclude auto-derived impls from the
/// identity set so they do not generate noise Red signals.
pub(super) fn is_derive_trait(normalized_trait_name: &str) -> bool {
    // Strip generic args first: `PartialOrd<Self>` → `PartialOrd`.
    let without_generics = normalized_trait_name.split('<').next().unwrap_or(normalized_trait_name);
    // Use last segment so `std::fmt::Debug` matches the short name `"Debug"`.
    let last_segment = without_generics.rsplit("::").next().unwrap_or(without_generics);
    DERIVE_TRAIT_NAMES.contains(&last_segment)
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
/// Only **explicit, non-blanket, non-negative, non-synthetic, non-derive trait
/// impls** are included.  Only local-crate impls (crate_id == 0) are included.
/// Auto-derived trait impls (e.g. `Clone`, `Debug`, `PartialEq`) are excluded
/// via [`is_derive_trait`] so they do not produce noise `CMinusSUnionD` signals.
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
            let for_name = format_type(&impl_.for_);

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
                        // Single-segment path from krate.paths: likely a well-known
                        // core/std trait whose module prefix was omitted.
                        // `core_canonical_path` returns the canonical qualified form
                        // (e.g. `"core::convert::From"`) for known traits and falls
                        // back to `"core::{joined}"` for unknown ones — preserving
                        // consistency with the S-side codec's output.
                        crate::tddd::type_ref_parser::core_canonical_path(&joined)
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

            // Skip auto-derived trait impls (Clone, Debug, PartialEq, Display, …).
            // These are never declared in the catalogue so they would always
            // appear in `CMinusSUnionD`, producing noise Red signals.
            if is_derive_trait(&normalized_trait_path) {
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
        // Bare name or `crate::`/`self::`/`super::` prefix → short name.
        path.rsplit("::").next().unwrap_or(path).to_string()
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
