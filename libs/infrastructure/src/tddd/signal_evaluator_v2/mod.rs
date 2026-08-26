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
//! 1. Build identity → Id maps for B (fully-qualified paths from `Crate::paths`
//!    for Struct/Enum/TypeAlias/Trait, FunctionPath strings for Function items).
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
//! Types/traits/functions retain the established structural formatter for shape comparison,
//! while every available `Crate::paths` identity is checked before that formatter runs. This
//! keeps A-derived and rustdoc-derived items symmetric without allowing same-named declarations
//! in different modules to collapse to one short-name identity.
//!
//! ## Module structure
//!
//! - `format`          — `format_type`, `format_generic_args`, `format_generic_bounds`, etc.
//! - `structural_eq`   — `items_structurally_equal_with_paths` (dispatch + struct/enum comparisons)
//! - `generics_eq`     — `generics_structurally_equal`, `build_trait_method_map`, `fn_sigs_*`
//! - `phase2`          — `phase2_evaluate` and S/D/C region helpers
//! - `resolve_type`    — `resolve_type` and friends (Phase 1.5 Id rewriting)
//! - `collect_refs`    — unresolved-marker scanning + dangling-Id collection
//! - `resolution`      — `resolve_unresolved_in_item` (Phase 1.5 item-level driver)
//! - `external_crates` — `build_external_crates_for_scope`, `patch_paths_crate_ids`
//! - `phase1`          — `Phase1State`, `phase1_build_s_and_d`, child-item helpers
//! - `tests`           — unit/integration tests (AC-08)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use domain::tddd::ExtendedCrate;
use domain::tddd::catalogue_v2::CrateName;
use domain::tddd::{Phase1Error, SignalEvaluatorPort, ThreeWayEvaluationReport};
use rustdoc_types::{Crate, Id, Item, ItemEnum, ItemKind};

use crate::schema_export::{RustdocTargetResolution, resolve_rustdoc_root_name};

// ---------------------------------------------------------------------------
// Sub-modules
// ---------------------------------------------------------------------------

pub(super) mod alias_lexical;
pub(super) mod alias_structural_eq;
pub(super) mod collect_refs;
pub(super) mod external_crates;
pub(crate) mod format;
pub(super) mod generics_eq;
pub(super) mod impl_identity;
pub(super) mod impl_identity_helpers;
pub(super) mod phase1;
pub(super) mod phase2;
pub(super) mod resolution;
pub(super) mod resolve_type;
pub(super) mod structural_eq;
pub(super) mod target_lifetimes;

#[cfg(test)]
pub(super) mod tests;

use phase1::phase1_build_s_and_d_with_rustdoc_root;
use phase2::phase2_evaluate;

#[cfg(test)]
pub(super) fn build_impl_identity_map(
    krate: &rustdoc_types::Crate,
    crate_name: &str,
) -> Result<std::collections::BTreeMap<String, rustdoc_types::Id>, domain::tddd::Phase1Error> {
    let authority = crate::tddd::canonical_type_identity::DefinitionPathAuthority::from_path_maps(
        &krate.paths,
        &[],
    );
    impl_identity::build_impl_identity_map(krate, crate_name, &authority)
}
#[cfg(test)]
pub(crate) use impl_identity::is_compiler_internal_trait;

// ---------------------------------------------------------------------------
// SignalEvaluatorV2 — secondary adapter
// ---------------------------------------------------------------------------

/// Secondary adapter that implements [`SignalEvaluatorPort`].
///
/// Drives the two-phase evaluation: Phase 1 builds S (`ExtendedCrate`) + D
/// (`rustdoc_types::Crate`) from the Catalogue-derived A and the Baseline B;
/// Phase 2 evaluates S / D / C to emit `ThreeWaySignal`s.
///
/// Construct with [`SignalEvaluatorV2::new`] and call
/// [`SignalEvaluatorPort::evaluate`].
#[derive(Debug, Clone)]
pub struct SignalEvaluatorV2 {
    workspace_root: PathBuf,
}

impl SignalEvaluatorV2 {
    /// Creates a new `SignalEvaluatorV2` rooted at the current working directory.
    ///
    /// Use [`Self::with_workspace_root`] when the caller already knows the
    /// workspace root and may be invoked from another directory.
    #[must_use]
    pub fn new() -> Self {
        Self::with_workspace_root(PathBuf::from("."))
    }

    /// Creates a new `SignalEvaluatorV2` for an explicit workspace root.
    #[must_use]
    pub fn with_workspace_root(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

impl Default for SignalEvaluatorV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalEvaluatorPort for SignalEvaluatorV2 {
    fn evaluate(
        &self,
        a: ExtendedCrate,
        b: Crate,
        c: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        let engine = EvaluationEngine::new(a, b, c, &self.workspace_root);
        engine.run()
    }
}

// ---------------------------------------------------------------------------
// Identity helpers (shared across phase1 and phase2 submodules)
// ---------------------------------------------------------------------------

/// Build a `(fully_qualified_path, Id)` map for types and traits in a
/// `rustdoc_types::Crate`.
///
/// Identity keys are the complete `Crate::paths` entries (crate, module path,
/// and item name) for `ItemEnum::Struct | Enum | TypeAlias | Trait`.
///
/// Used in Phase 1 for action matching between A and B and in Phase 2 for
/// matching S/D/C items. Short names remain a separate input-resolution and
/// display concern; they must not decide which rustdoc item represents an
/// identity.
///
/// A valid rustdoc crate cannot expose two different local items at the same
/// fully-qualified path. The `(path, id)` ordering still makes malformed or
/// synthetic fixtures deterministic without collapsing distinct module paths.
///
/// # Errors
///
/// Returns `Phase1Error::RustdocRootResolution` when a local type or trait has
/// no non-empty authoritative entry in `Crate::paths`. Identity construction
/// is fail-closed: a missing path must not silently remove an item from Phase 1
/// or Phase 2.
pub(super) fn build_type_trait_identity_map(
    krate: &Crate,
) -> Result<BTreeMap<String, Id>, Phase1Error> {
    // Collect candidates from `Crate::paths`, which is the authoritative source
    // for local type/trait identity. `Item::name` is intentionally not used as
    // the key because it omits the module path and is therefore ambiguous.
    let mut candidates: Vec<(String, Id)> = Vec::new();
    for (id, item) in &krate.index {
        // Only include local crate items (crate_id == 0 means "this crate").
        if item.crate_id != 0 {
            continue;
        }
        if is_type_or_trait_item(item) {
            let item_name = item.name.as_deref().unwrap_or("<unnamed>");
            let path_summary = krate.paths.get(id).ok_or_else(|| {
                Phase1Error::rustdoc_root_resolution(format!(
                    "local type/trait `{item_name}` (id {}) has no authoritative Crate::paths entry",
                    id.0
                ))
            })?;
            let identity_key = path_summary.path.join("::");
            if identity_key.is_empty() {
                return Err(Phase1Error::rustdoc_root_resolution(format!(
                    "local type/trait `{item_name}` (id {}) has an empty authoritative Crate::paths path",
                    id.0
                )));
            }
            candidates.push((identity_key, *id));
        }
    }
    // Keep duplicate-path handling deterministic for synthetic inputs. Distinct
    // full paths are retained independently, including same-name items in
    // different modules.
    candidates.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.0.cmp(&b.1.0)));
    let mut map: BTreeMap<String, Id> = BTreeMap::new();
    for (identity_key, id) in candidates {
        map.entry(identity_key).or_insert(id);
    }
    Ok(map)
}

/// Build a `(function_path_string, Id)` map for free function items in a `rustdoc_types::Crate`.
///
/// Identity key: canonical `FunctionPath` = path segments joined by `"::"`, looked up
/// from `Crate::paths`.
///
/// For normal library/catalogue graphs the full path is preserved. For a bin-only
/// package, a supplied [`RustdocTargetResolution`] rewrites the rustdoc binary-root segment
/// to the package-root segment used by catalogue `FunctionPath` keys.
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
pub(super) fn build_function_identity_map(
    krate: &Crate,
    rustdoc_root: Option<&RustdocTargetResolution>,
) -> BTreeMap<String, Id> {
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
        let identity_key =
            crate::tddd::canonical_type_identity::canonicalize_function_identity_path(
                &summary.path,
                rustdoc_root.map(|translation| translation.package_name()),
                rustdoc_root.map(|translation| translation.rustdoc_root_name()),
            );
        if !identity_key.is_empty() {
            map.insert(identity_key, *id);
        }
    }
    map
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
    workspace_root: PathBuf,
}

impl EvaluationEngine {
    fn new(a: ExtendedCrate, b: Crate, c: Crate, workspace_root: &Path) -> Self {
        Self { a, b, c, workspace_root: workspace_root.to_path_buf() }
    }

    fn run(self) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        let rustdoc_root =
            resolve_function_rustdoc_root(&self.a, &self.b, &self.c, &self.workspace_root)?;
        let (s, d) =
            phase1_build_s_and_d_with_rustdoc_root(self.a, &self.b, rustdoc_root.as_ref())?;
        let report = phase2_evaluate(&s, &d, &self.c, rustdoc_root.as_ref())?;
        Ok(report)
    }
}

/// Resolves a root translation only when rustdoc data differs from the
/// catalogue package root. This preserves the zero-I/O path for ordinary
/// library evaluations and performs one metadata lookup for a bin-root alias.
fn resolve_function_rustdoc_root(
    a: &ExtendedCrate,
    b: &Crate,
    c: &Crate,
    workspace_root: &Path,
) -> Result<Option<RustdocTargetResolution>, Phase1Error> {
    let Some(package_root) = crate_root_name(a.krate()) else {
        return Ok(None);
    };
    let rustdoc_roots: Vec<&str> = [b, c].into_iter().filter_map(crate_root_name).collect();
    if rustdoc_roots.iter().all(|root| *root == package_root) {
        return Ok(None);
    }

    let package_name = CrateName::new(package_root.to_owned()).map_err(|error| {
        Phase1Error::rustdoc_root_resolution(format!(
            "invalid package root `{package_root}`: {error}"
        ))
    })?;
    let translation =
        resolve_rustdoc_root_name(workspace_root, &package_name).map_err(|error| {
            Phase1Error::rustdoc_root_resolution(format!(
                "cannot resolve package `{package_root}` from {}: {error}",
                workspace_root.display()
            ))
        })?;
    if rustdoc_roots.iter().any(|root| *root == translation.rustdoc_root_name().as_str()) {
        Ok(Some(translation))
    } else {
        Ok(None)
    }
}

fn crate_root_name(krate: &Crate) -> Option<&str> {
    krate.index.get(&krate.root).and_then(|item| item.name.as_deref())
}
