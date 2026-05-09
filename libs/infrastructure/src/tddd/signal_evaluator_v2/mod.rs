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
use rustdoc_types::{Crate, Id, Item, ItemEnum, ItemKind};

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
    // Sort by (short_name, full_path) so that for each short name, the
    // lexicographically smallest full path wins — deterministic across crates.
    candidates.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
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
/// included.  Only local-crate impls (crate_id == 0) are included.
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
            // Normalize the trait path so S-side and C-side keys are consistent.
            let normalized_trait_path = normalize_impl_trait_path(&trait_path.path, crate_name);
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
/// `crate_name` is the real crate name as it appears in rustdoc paths (e.g.
/// `"my_crate"`).  Pass `""` for A-side (codec) maps where trait paths use
/// `crate::` or bare names rather than the real crate name.
///
/// Normalization rules:
/// - Bare identifiers (no `::`) → returned as-is (already short).
/// - `crate::`-, `self::`-, `super::`-prefixed paths → stripped to last segment.
/// - Paths that start with `{crate_name}::` (local crate in rustdoc) → stripped
///   to last segment, so `my_crate::MyTrait` produces `MyTrait` matching codec's
///   `crate::MyTrait` → `MyTrait`.
/// - All other paths (external: `serde::Serialize`, `std::fmt::Debug`) → kept
///   verbatim to prevent collisions between distinct external traits sharing a
///   short name.
pub(super) fn normalize_impl_trait_path(path: &str, crate_name: &str) -> String {
    if is_local_unresolved_path(path) {
        // Bare name or `crate::`/`self::`/`super::` prefix → short name.
        path.rsplit("::").next().unwrap_or(path).to_string()
    } else if !crate_name.is_empty() && path.starts_with(&format!("{crate_name}::")) {
        // rustdoc local trait path (e.g. `my_crate::MyTrait`) → short name.
        path.rsplit("::").next().unwrap_or(path).to_string()
    } else {
        // External crate path → keep verbatim for disambiguation.
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
