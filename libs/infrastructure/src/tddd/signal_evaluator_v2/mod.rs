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
pub(crate) mod format;
pub(super) mod generics_eq;
pub(super) mod impl_identity;
pub(super) mod phase1;
pub(super) mod phase2;
pub(super) mod resolution;
pub(super) mod resolve_type;
pub(super) mod structural_eq;

#[cfg(test)]
pub(super) mod tests;

use phase1::phase1_build_s_and_d;
use phase2::phase2_evaluate;

pub(super) use impl_identity::build_impl_identity_map;
#[cfg(test)]
pub(crate) use impl_identity::{is_compiler_internal_trait, normalize_impl_trait_path};

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
/// Identity key: canonical `FunctionPath` = path segments joined by `"::"`, looked up
/// from `Crate::paths`.
///
/// For normal library/catalogue graphs the full path is preserved
/// (`"cli::module::fn_name"` stays `"cli::module::fn_name"`). For the workspace's
/// `cli` package, rustdoc for the `[[bin]]` target names the crate root after the binary
/// (`"sotp::module::fn_name"`). That single bin-root alias is rewritten to the package
/// crate root (`"cli::module::fn_name"`) so bin rustdoc entries still match catalogue
/// `FunctionPath` keys without making every function identity rootless.
///
/// Only **free** functions are included.  Associated methods (belonging to a
/// `Trait` or `Impl` `items` list) are explicitly excluded even when they
/// appear in `Crate::paths`: trait-method structural equality is captured at
/// the trait/impl level, and duplicating methods here would cause spurious or
/// double-counted Phase 2 signals.
///
/// Visibility is intentionally NOT filtered here: every function rustdoc surfaces
/// for the local crate is recorded so the catalogue can declare it (with the
/// action that reflects reality — `Add` / `Modify` / `Delete` / `Reference`).
/// `[[bin]]` targets surface even private `fn` items because `rustdoc --bin`
/// has no external-API consumer to hide them from; if the catalogue does not
/// want to track such an item it must still declare a row for it so the
/// trade-off is visible in source, not implicit in a framework filter.
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
        if !matches!(item.inner, ItemEnum::Function(_)) {
            continue;
        }
        let Some(summary) = krate.paths.get(id) else { continue };
        let identity_key = function_identity_key(&summary.path);
        if !identity_key.is_empty() {
            map.insert(identity_key, *id);
        }
    }
    map
}

fn function_identity_key(path: &[String]) -> String {
    let Some((root, rest)) = path.split_first() else {
        return String::new();
    };
    let mut segments = Vec::with_capacity(path.len());
    segments.push(canonical_function_root_segment(root).to_owned());
    segments.extend(rest.iter().cloned());
    segments.join("::")
}

fn canonical_function_root_segment(root: &str) -> &str {
    match root {
        // apps/cli is package `cli` with bin target `sotp`. rustdoc --bin uses
        // the bin name as the root segment, while catalogues use the package
        // crate name in FunctionPath keys.
        "sotp" => "cli",
        other => other,
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
