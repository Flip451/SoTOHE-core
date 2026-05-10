//! Catalogue → ExtendedCrate (TypeGraph A) codec.
//!
//! `CatalogueToExtendedCrateCodec` converts a domain `CatalogueDocument` into an
//! `ExtendedCrate` (TypeGraph A). It implements the secondary-adapter role for the
//! `CatalogueToExtendedCratePort` port declared in the domain layer.
//!
//! ## Conversion pipeline (ADR 2 D8 / D9 / D10 / D11)
//!
//! 1. Pre-pass Id assignment: assign incremental `rustdoc_types::Id`s to all entries.
//!    Id(0) is reserved for the root module.
//! 2. External crate collection: gather `TraitImplDeclV2::origin_crate` names and
//!    `TypeRef` crate prefixes to build `Crate::external_crates`.
//! 3. TypeRef parse: convert each `TypeRef` string via `syn::parse_str` into
//!    `rustdoc_types::Type`. Unresolvable identifiers become open-world "unresolved
//!    markers" (ADR 2 D10).
//! 4. Inline → id-ref: `FieldDecl` / `VariantDecl` are promoted to individual
//!    `rustdoc_types::Item` entries and the parent references them via `Vec<Id>`.
//! 5. Inherent impl grouping: all `MethodDeclaration`s on a type are grouped into a
//!    single `Impl` item per type.
//! 6. Trait impl blocks: `TraitImplDeclV2` entries produce `Impl` items with trait
//!    identity only (no method items — ADR 2 D12).
//! 7. Crate.paths: each in-crate item gets an `ItemSummary` with
//!    `path = [crate_name, ...module_path, item_name]`.
//! 8. item_actions: each catalogue entry's `ItemAction` is recorded in
//!    `ExtendedCrate::item_actions`.
//!
//! (infrastructure-types.json: `CatalogueToExtendedCrateCodec`)

use std::collections::{BTreeMap, HashMap};

use domain::tddd::CatalogueToExtendedCratePort;
use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::composite::CompositePattern;
use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl, VariantPayload};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, FunctionPath, MethodDeclaration, ModulePath, SelfReceiver,
    TraitImplDeclV2, TraitName, TypeKindV2, TypeName,
};
use domain::tddd::extended_crate::ExtendedCrate;
use rustdoc_types::{
    Crate, ExternalCrate, FORMAT_VERSION, FunctionHeader, FunctionSignature, GenericArg,
    GenericArgs, Generics, Id, Impl, Item, ItemEnum, ItemKind, ItemSummary, Module, Path, Struct,
    StructKind, Target, Trait, Type, TypeAlias, Variant, VariantKind, Visibility,
};

use crate::tddd::catalogue_to_extended_crate_codec_error::CatalogueToExtendedCrateCodecError;
use crate::tddd::type_ref_parser::{UNRESOLVED_CRATE_ID, parse_type_ref};

// ---------------------------------------------------------------------------
// CatalogueToExtendedCrateCodec
// ---------------------------------------------------------------------------

/// Stateless codec that converts `CatalogueDocument` → `ExtendedCrate` (TypeGraph A).
///
/// Implements `CatalogueToExtendedCratePort`. Instantiate with `new()` and call
/// `encode()`.
#[derive(Debug, Clone, Default)]
pub struct CatalogueToExtendedCrateCodec;

impl CatalogueToExtendedCrateCodec {
    /// Creates a new codec instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl CatalogueToExtendedCratePort for CatalogueToExtendedCrateCodec {
    fn encode(&self, doc: CatalogueDocument) -> Result<ExtendedCrate, NewTypeGraphCodecError> {
        Encoder::new(doc).run().map_err(Into::into)
    }
}

// ---------------------------------------------------------------------------
// Encoder — internal per-call state
// ---------------------------------------------------------------------------

/// Pre-pass state that holds the `CatalogueDocument` alongside encoding state.
///
/// After pre-passes complete, `Encoder` is consumed and destructured into
/// a `CatalogueDocument` + `EncoderState` so that encoding loops can borrow
/// the document immutably while mutating the state.
struct Encoder {
    doc: CatalogueDocument,
    state: EncoderState,
}

/// Mutable encoding state used during the main encoding loop.
///
/// Separated from `Encoder` so that encoding methods can hold a mutable borrow
/// on `EncoderState` while the caller holds an immutable borrow on the
/// `CatalogueDocument`.
struct EncoderState {
    /// Incremental Id counter (Id(0) = root module).
    next_id: u32,
    /// Item index.
    index: HashMap<Id, Item>,
    /// Paths map for `Crate::paths`.
    paths: HashMap<Id, ItemSummary>,
    /// `crate_id` → `ExternalCrate` for `Crate::external_crates`.
    external_crates: HashMap<u32, ExternalCrate>,
    /// `crate_name` → `crate_id` lookup for TypeRef resolution.
    ext_name_to_id: HashMap<String, u32>,
    /// Next external crate_id to assign (0 is reserved for self).
    next_ext_id: u32,
    /// Catalogue-declared type/trait short name → assigned `Id` for TypeRef resolution.
    ///
    /// Only types and traits are stored here. Function path strings are kept
    /// in `fn_path_to_id` to prevent function ids from polluting TypeRef
    /// resolution (which operates on single-segment names only).
    local_name_to_id: HashMap<String, Id>,
    /// Full `FunctionPath` string → assigned `Id`.
    ///
    /// Kept separate from `local_name_to_id` so that function paths (which are
    /// multi-segment strings such as `"my_crate::fn_name"`) are never
    /// accidentally matched by the single-segment TypeRef resolver, and so that
    /// `AmbiguousIdentifier` checks are applied independently to the two
    /// namespaces (types/traits vs. functions).
    fn_path_to_id: HashMap<String, Id>,
    /// Crate name used for `ItemSummary::path` construction.
    crate_name: CrateName,
    /// Cache: full canonical path string → synthetic `Id` for external type references.
    ///
    /// Ensures that the same external type (e.g. `std::vec::Vec`) always gets the same
    /// synthetic item id within a single codec run, so `Crate::paths` entries are not
    /// duplicated and downstream consumers can reliably look up external types.
    external_type_path_to_id: HashMap<String, Id>,
}

impl Encoder {
    fn new(doc: CatalogueDocument) -> Self {
        let crate_name = doc.crate_name.clone();
        Self {
            doc,
            state: EncoderState {
                next_id: 0,
                index: HashMap::new(),
                paths: HashMap::new(),
                external_crates: HashMap::new(),
                ext_name_to_id: HashMap::new(),
                next_ext_id: 1,
                local_name_to_id: HashMap::new(),
                fn_path_to_id: HashMap::new(),
                crate_name,
                external_type_path_to_id: HashMap::new(),
            },
        }
    }

    /// Pre-pass: assign `Id`s to all declared types, traits, and functions.
    ///
    /// Returns `AmbiguousTypeName` if two entries share the same short name.
    fn assign_ids(&mut self) -> Result<(), CatalogueToExtendedCrateCodecError> {
        // Id(0) = root module.
        let _ = self.state.alloc_id();

        // Collect keys first to avoid simultaneous borrow + mutable borrow.
        let type_names: Vec<String> =
            self.doc.types.keys().map(|k| k.as_str().to_string()).collect();
        for name in type_names {
            if self.state.local_name_to_id.contains_key(&name) {
                return Err(CatalogueToExtendedCrateCodecError::AmbiguousIdentifier { name });
            }
            let id = self.state.alloc_id();
            self.state.local_name_to_id.insert(name, id);
        }
        let trait_names: Vec<String> =
            self.doc.traits.keys().map(|k| k.as_str().to_string()).collect();
        for name in trait_names {
            if self.state.local_name_to_id.contains_key(&name) {
                return Err(CatalogueToExtendedCrateCodecError::AmbiguousIdentifier { name });
            }
            let id = self.state.alloc_id();
            self.state.local_name_to_id.insert(name, id);
        }
        // Functions are keyed by their full `FunctionPath` string (e.g. `"my_crate::fn"`).
        // Store them in the dedicated `fn_path_to_id` map to keep the TypeRef-resolution
        // map (`local_name_to_id`) free of multi-segment function keys.
        let fn_paths: Vec<String> = self.doc.functions.keys().map(|k| k.to_string()).collect();
        for path in fn_paths {
            if self.state.fn_path_to_id.contains_key(&path) {
                return Err(CatalogueToExtendedCrateCodecError::AmbiguousIdentifier { name: path });
            }
            let id = self.state.alloc_id();
            self.state.fn_path_to_id.insert(path, id);
        }
        Ok(())
    }

    /// Pre-pass: register external crates from `TraitImplDeclV2::origin_crate`.
    fn collect_external_from_trait_impls(&mut self) {
        let self_crate_name = self.doc.crate_name.as_str().to_string();
        let origin_crates: Vec<String> = self
            .doc
            .types
            .values()
            .flat_map(|e| e.trait_impls.iter().map(|ti| ti.origin_crate.as_str().to_string()))
            .filter(|cn| *cn != self_crate_name)
            .collect();
        for cn in origin_crates {
            self.state.ensure_external_crate(cn);
        }
    }

    /// Runs the full encoding pipeline.
    fn run(mut self) -> Result<ExtendedCrate, CatalogueToExtendedCrateCodecError> {
        // Pre-passes.
        self.assign_ids()?;
        self.collect_external_from_trait_impls();
        self.state.ensure_external_crate("std".to_string());

        // Destructure: separate `doc` from mutable `state` so encoding loops can
        // borrow `doc` immutably while mutating `state`.
        let Encoder { doc, mut state } = self;

        // item_actions (domain layer BTreeMap).
        let mut item_actions: BTreeMap<Id, ItemAction> = BTreeMap::new();

        // Encode types.
        for (type_name, entry) in &doc.types {
            let type_id =
                state.local_name_to_id.get(type_name.as_str()).copied().ok_or_else(|| {
                    CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                        type_ref: type_name.as_str().to_string(),
                        reason: "pre-pass id not found (internal error)".to_string(),
                    }
                })?;
            let action = entry.action;
            match entry.kind.clone() {
                TypeKindV2::Struct { pattern, fields } => {
                    state.encode_struct(type_id, type_name, entry, pattern, fields)?;
                }
                TypeKindV2::Enum { variants } => {
                    state.encode_enum(type_id, type_name, entry, variants)?;
                }
                TypeKindV2::TypeAlias { target } => {
                    state.encode_type_alias(type_id, type_name, entry, target)?;
                }
            }
            item_actions.insert(type_id, action);
        }

        // Encode traits.
        for (trait_name, entry) in &doc.traits {
            let trait_id =
                state.local_name_to_id.get(trait_name.as_str()).copied().ok_or_else(|| {
                    CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                        type_ref: trait_name.as_str().to_string(),
                        reason: "pre-pass id not found (internal error)".to_string(),
                    }
                })?;
            let action = entry.action;
            state.encode_trait(trait_id, trait_name, entry)?;
            item_actions.insert(trait_id, action);
        }

        // Encode functions.
        for (fn_path, entry) in &doc.functions {
            let fn_id =
                state.fn_path_to_id.get(&fn_path.to_string()).copied().ok_or_else(|| {
                    CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                        type_ref: fn_path.to_string(),
                        reason: "pre-pass id not found (internal error)".to_string(),
                    }
                })?;
            let action = entry.action;
            state.encode_function(fn_id, fn_path, entry)?;
            item_actions.insert(fn_id, action);
        }

        // Root module: children = all top-level ids assigned in pre-pass
        // (types + traits from `local_name_to_id`, functions from `fn_path_to_id`).
        // Sort by numeric id to produce a stable ordering across runs (HashMap iteration
        // order is nondeterministic; sorting by id preserves pre-pass insertion order).
        let root_id = Id(0);
        let mut top_level_ids: Vec<Id> =
            state.local_name_to_id.values().chain(state.fn_path_to_id.values()).copied().collect();
        top_level_ids.sort_by_key(|id| id.0);
        let root_item = make_item(
            root_id,
            Some(doc.crate_name.as_str().to_string()),
            None,
            ItemEnum::Module(Module { is_crate: true, items: top_level_ids, is_stripped: false }),
        );
        state.index.insert(root_id, root_item);

        let krate = Crate {
            root: root_id,
            crate_version: None,
            includes_private: false,
            index: state.index,
            paths: state.paths,
            external_crates: state.external_crates,
            format_version: FORMAT_VERSION,
            target: Target { triple: String::new(), target_features: vec![] },
        };

        Ok(ExtendedCrate::new(krate, item_actions))
    }
}

impl EncoderState {
    fn alloc_id(&mut self) -> Id {
        let id = Id(self.next_id);
        self.next_id += 1;
        id
    }

    /// Ensures an external crate is registered and returns its `crate_id`.
    fn ensure_external_crate(&mut self, crate_name: String) -> u32 {
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
    fn ensure_external_type_id(&mut self, canonical_path: &str, crate_name: &str) -> Id {
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
    fn resolve_external_type_ids(&mut self, ty: Type) -> Type {
        match ty {
            // `ResolvedPath` — fix up the id if unresolved, and always recurse into args
            // so that nested generics (e.g. `Wrapper<serde::Value>`) are also corrected.
            Type::ResolvedPath(p) => {
                let new_id = if p.id == Id(UNRESOLVED_CRATE_ID) {
                    let path_str = &p.path;
                    // Determine whether this is a known external or a truly-unresolved marker.
                    // A known external has a "::" in the path (multi-segment) and its first
                    // segment is a registered external crate name.
                    if let Some(colon_pos) = path_str.find("::") {
                        let first_seg = &path_str[..colon_pos];
                        if self.ext_name_to_id.contains_key(first_seg) {
                            self.ensure_external_type_id(path_str, first_seg)
                        } else {
                            Id(UNRESOLVED_CRATE_ID)
                        }
                    } else {
                        // Single-segment: not a known external — truly unresolved.
                        Id(UNRESOLVED_CRATE_ID)
                    }
                } else {
                    // Already resolved (e.g. a local catalogue type). Keep the id as-is.
                    p.id
                };
                // Always recurse into generic args so nested external paths are also fixed.
                let new_args = p.args.map(|boxed_args| {
                    Box::new(self.resolve_external_type_ids_in_generic_args(*boxed_args))
                });
                Type::ResolvedPath(Path { path: p.path, id: new_id, args: new_args })
            }
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
    fn resolve_external_type_ids_in_path(&mut self, path: Path) -> Path {
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
    fn resolve_external_type_ids_in_generic_args(&mut self, args: GenericArgs) -> GenericArgs {
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
    fn resolve_external_type_ids_in_generic_bound(
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
    fn parse_type_ref_str(
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

    /// Builds `[crate_name, ...module_path, item_name]` path segments.
    fn build_path_segments(
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
    fn register_path(&mut self, id: Id, kind: ItemKind, item_name: &str, module_path: &ModulePath) {
        let path = Self::build_path_segments(&self.crate_name.clone(), module_path, item_name);
        self.paths.insert(id, ItemSummary { crate_id: 0, path, kind });
    }

    /// Registers an `ItemSummary` in `Crate::paths` using an explicit crate name.
    ///
    /// If `fn_crate_name` matches the document crate, the item is recorded under
    /// `crate_id: 0` (local crate). If it differs, the external crate id is looked up
    /// or allocated via `ensure_external_crate`.
    fn register_path_for_crate(
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

    /// Encodes a struct-kind `TypeEntry`.
    fn encode_struct(
        &mut self,
        type_id: Id,
        type_name: &TypeName,
        entry: &TypeEntry,
        pattern: CompositePattern,
        fields: Vec<FieldDecl>,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        let module_path = entry.module_path.clone();
        let docs = entry.docs.clone();

        // Choose the `StructKind` based on the pattern.
        // - `Newtype { inner }` → `StructKind::Tuple` with a single anonymous field.
        //   The `inner` type is the newtype's wrapped type.
        // - `Plain` and `TypestateState { .. }` → `StructKind::Plain` with named fields.
        let struct_kind = match &pattern {
            CompositePattern::Newtype { inner } => {
                let field_id = self.alloc_id();
                let field_ty = self.parse_type_ref_str(inner.as_str())?;
                self.index.insert(
                    field_id,
                    make_item(field_id, None, None, ItemEnum::StructField(field_ty)),
                );
                // Encode any extra declared fields (rare, but respect the catalogue).
                let mut extra_ids: Vec<Option<Id>> = vec![Some(field_id)];
                for field in &fields {
                    let fid = self.alloc_id();
                    let fty = self.parse_type_ref_str(field.ty.as_str())?;
                    self.index.insert(fid, make_item(fid, None, None, ItemEnum::StructField(fty)));
                    extra_ids.push(Some(fid));
                }
                StructKind::Tuple(extra_ids)
            }
            CompositePattern::Plain | CompositePattern::TypestateState { .. } => {
                // Encode struct fields → StructField items.
                let mut field_ids: Vec<Id> = vec![];
                for field in &fields {
                    let field_id = self.alloc_id();
                    let field_ty = self.parse_type_ref_str(field.ty.as_str())?;
                    self.index.insert(
                        field_id,
                        make_item(
                            field_id,
                            Some(field.name.as_str().to_string()),
                            None,
                            ItemEnum::StructField(field_ty),
                        ),
                    );
                    field_ids.push(field_id);
                }
                StructKind::Plain { fields: field_ids, has_stripped_fields: false }
            }
        };

        // Encode inherent method items (concrete implementations → has_body: true).
        let methods: Vec<MethodDeclaration> = entry.methods.clone();
        let method_ids =
            self.encode_method_items(&methods, true, type_name.as_str(), &module_path)?;

        // Inherent Impl block.
        let impl_id = self.alloc_id();
        let for_type = resolved_path_type(type_id, type_name.as_str());
        self.index.insert(
            impl_id,
            make_item(impl_id, None, None, ItemEnum::Impl(make_impl(for_type, None, method_ids))),
        );

        // Trait impl blocks.
        let trait_impls: Vec<TraitImplDeclV2> = entry.trait_impls.clone();
        let trait_impl_ids =
            self.encode_trait_impl_blocks(type_id, type_name.as_str(), &trait_impls)?;

        // Build the full impls list: inherent impl first, then trait impls.
        let mut all_impl_ids = vec![impl_id];
        all_impl_ids.extend(trait_impl_ids);

        // Struct item.
        let struct_item = make_item(
            type_id,
            Some(type_name.as_str().to_string()),
            docs,
            ItemEnum::Struct(Struct {
                kind: struct_kind,
                generics: empty_generics(),
                impls: all_impl_ids,
            }),
        );
        self.index.insert(type_id, struct_item);
        self.register_path(type_id, ItemKind::Struct, type_name.as_str(), &module_path);
        Ok(())
    }

    /// Encodes an enum-kind `TypeEntry`.
    fn encode_enum(
        &mut self,
        type_id: Id,
        type_name: &TypeName,
        entry: &TypeEntry,
        variants: Vec<VariantDecl>,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        let module_path = entry.module_path.clone();
        let docs = entry.docs.clone();

        // Encode variant items.
        let mut variant_ids: Vec<Id> = vec![];
        for variant in &variants {
            let variant_id = self.alloc_id();
            let variant_name = variant.name.as_str().to_string();
            let payload = variant.payload.clone();
            let kind = self.encode_variant_kind(payload)?;
            self.index.insert(
                variant_id,
                make_item(
                    variant_id,
                    Some(variant_name),
                    None,
                    ItemEnum::Variant(Variant { kind, discriminant: None }),
                ),
            );
            variant_ids.push(variant_id);
        }

        // Inherent methods (concrete implementations → has_body: true).
        let methods: Vec<MethodDeclaration> = entry.methods.clone();
        let method_ids =
            self.encode_method_items(&methods, true, type_name.as_str(), &module_path)?;

        // Inherent Impl block.
        let impl_id = self.alloc_id();
        let for_type = resolved_path_type(type_id, type_name.as_str());
        self.index.insert(
            impl_id,
            make_item(impl_id, None, None, ItemEnum::Impl(make_impl(for_type, None, method_ids))),
        );

        // Trait impl blocks.
        let trait_impls: Vec<TraitImplDeclV2> = entry.trait_impls.clone();
        let trait_impl_ids =
            self.encode_trait_impl_blocks(type_id, type_name.as_str(), &trait_impls)?;

        // Build the full impls list: inherent impl first, then trait impls.
        let mut all_impl_ids = vec![impl_id];
        all_impl_ids.extend(trait_impl_ids);

        // Enum item.
        let enum_item = make_item(
            type_id,
            Some(type_name.as_str().to_string()),
            docs,
            ItemEnum::Enum(rustdoc_types::Enum {
                generics: empty_generics(),
                variants: variant_ids,
                impls: all_impl_ids,
                has_stripped_variants: false,
            }),
        );
        self.index.insert(type_id, enum_item);
        self.register_path(type_id, ItemKind::Enum, type_name.as_str(), &module_path);
        Ok(())
    }

    /// Encodes a type-alias-kind `TypeEntry`.
    fn encode_type_alias(
        &mut self,
        type_id: Id,
        type_name: &TypeName,
        entry: &TypeEntry,
        target: domain::tddd::catalogue_v2::TypeRef,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        let module_path = entry.module_path.clone();
        let docs = entry.docs.clone();

        let target_ty = self.parse_type_ref_str(target.as_str())?;

        // Encode inherent methods (rare for type aliases; concrete implementations → has_body: true).
        let methods: Vec<MethodDeclaration> = entry.methods.clone();
        let method_ids =
            self.encode_method_items(&methods, true, type_name.as_str(), &module_path)?;
        let impl_id = self.alloc_id();
        let for_type = resolved_path_type(type_id, type_name.as_str());
        self.index.insert(
            impl_id,
            make_item(impl_id, None, None, ItemEnum::Impl(make_impl(for_type, None, method_ids))),
        );

        // Trait impl blocks.
        let trait_impls: Vec<TraitImplDeclV2> = entry.trait_impls.clone();
        // `TypeAlias` has no `impls` field in rustdoc_types; trait impl items are
        // recorded in the index but cannot be referenced from the alias item directly.
        let _trait_impl_ids =
            self.encode_trait_impl_blocks(type_id, type_name.as_str(), &trait_impls)?;

        let alias_item = make_item(
            type_id,
            Some(type_name.as_str().to_string()),
            docs,
            ItemEnum::TypeAlias(TypeAlias { type_: target_ty, generics: empty_generics() }),
        );
        self.index.insert(type_id, alias_item);
        self.register_path(type_id, ItemKind::TypeAlias, type_name.as_str(), &module_path);
        Ok(())
    }

    /// Encodes a `TraitEntry` into a `Trait` item.
    fn encode_trait(
        &mut self,
        trait_id: Id,
        trait_name: &TraitName,
        entry: &TraitEntry,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        let module_path = entry.module_path.clone();
        let docs = entry.docs.clone();

        // Trait methods are declarations, not implementations (has_body: false).
        let methods: Vec<MethodDeclaration> = entry.methods.clone();
        let method_ids =
            self.encode_method_items(&methods, false, trait_name.as_str(), &module_path)?;

        let trait_item = make_item(
            trait_id,
            Some(trait_name.as_str().to_string()),
            docs,
            ItemEnum::Trait(Trait {
                is_auto: false,
                is_unsafe: false,
                is_dyn_compatible: true,
                items: method_ids,
                generics: empty_generics(),
                bounds: vec![],
                implementations: vec![],
            }),
        );
        self.index.insert(trait_id, trait_item);
        self.register_path(trait_id, ItemKind::Trait, trait_name.as_str(), &module_path);
        Ok(())
    }

    /// Encodes a `FunctionEntry` into a `Function` item.
    fn encode_function(
        &mut self,
        fn_id: Id,
        fn_path: &FunctionPath,
        entry: &FunctionEntry,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        let module_path = fn_path.module_path.clone();
        let docs = entry.docs.clone();
        let is_async = entry.is_async;

        let params: Vec<_> = entry
            .params
            .iter()
            .map(|p| (p.name.as_str().to_string(), p.ty.as_str().to_string()))
            .collect();
        let returns_str = entry.returns.as_str().to_string();

        let mut inputs: Vec<(String, Type)> = vec![];
        for (name, ty_str) in params {
            let ty = self.parse_type_ref_str(&ty_str)?;
            inputs.push((name, ty));
        }
        let output =
            if returns_str == "()" { None } else { Some(self.parse_type_ref_str(&returns_str)?) };

        // Determine the effective crate_id for this function. Local functions use
        // crate_id 0; cross-workspace functions are assigned an external crate id.
        let fn_crate_id = if fn_path.crate_name.as_str() == self.crate_name.as_str() {
            0u32
        } else {
            self.ensure_external_crate(fn_path.crate_name.as_str().to_string())
        };
        let fn_item = make_item_with_crate_id(
            fn_crate_id,
            fn_id,
            Some(fn_path.name.as_str().to_string()),
            docs,
            ItemEnum::Function(rustdoc_types::Function {
                sig: FunctionSignature { inputs, output, is_c_variadic: false },
                generics: empty_generics(),
                has_body: true,
                header: FunctionHeader {
                    is_async,
                    is_const: false,
                    is_unsafe: false,
                    abi: rustdoc_types::Abi::Rust,
                },
            }),
        );
        self.index.insert(fn_id, fn_item);
        // Preserve the embedded crate name from `fn_path` so that cross-workspace functions
        // are not silently flattened under the document's own crate name (ADR 2 D5).
        self.register_path_for_crate(
            fn_id,
            ItemKind::Function,
            fn_path.name.as_str(),
            &module_path,
            &fn_path.crate_name,
        );
        Ok(())
    }

    /// Encodes `MethodDeclaration`s as `Function` items; returns their Ids.
    ///
    /// `has_body` should be `true` for inherent methods (concrete implementations)
    /// and `false` for trait method declarations (signatures only).
    ///
    /// `parent_name` is the short name of the containing type or trait (used to build
    /// `Crate::paths` entries with path `[crate, ...module, ParentName, method_name]`).
    /// `parent_module_path` is the module path of the containing item.
    fn encode_method_items(
        &mut self,
        methods: &[MethodDeclaration],
        has_body: bool,
        parent_name: &str,
        parent_module_path: &ModulePath,
    ) -> Result<Vec<Id>, CatalogueToExtendedCrateCodecError> {
        let mut ids = vec![];
        for method in methods {
            let method_id = self.alloc_id();
            let is_async = method.is_async;
            let docs = method.docs.clone();
            let name = method.name.as_str().to_string();
            let returns_str = method.returns.as_str().to_string();
            let receiver_opt = method.receiver;

            // Build inputs: receiver then params.
            let mut inputs: Vec<(String, Type)> = vec![];
            if let Some(recv) = receiver_opt {
                let recv_ty = receiver_type(recv);
                inputs.push(("self".to_string(), recv_ty));
            }
            let param_pairs: Vec<(String, String)> = method
                .params
                .iter()
                .map(|p| (p.name.as_str().to_string(), p.ty.as_str().to_string()))
                .collect();
            for (pname, pty_str) in param_pairs {
                let pty = self.parse_type_ref_str(&pty_str)?;
                inputs.push((pname, pty));
            }
            let output = if returns_str == "()" {
                None
            } else {
                Some(self.parse_type_ref_str(&returns_str)?)
            };

            let fn_item = make_item(
                method_id,
                Some(name.clone()),
                docs,
                ItemEnum::Function(rustdoc_types::Function {
                    sig: FunctionSignature { inputs, output, is_c_variadic: false },
                    generics: empty_generics(),
                    has_body,
                    header: FunctionHeader {
                        is_async,
                        is_const: false,
                        is_unsafe: false,
                        abi: rustdoc_types::Abi::Rust,
                    },
                }),
            );
            self.index.insert(method_id, fn_item);

            // Register this method in `Crate::paths` with path
            // `[crate_name, ...module_segs, ParentName, method_name]`.
            let crate_name = self.crate_name.clone();
            let mut path_segs = vec![crate_name.as_str().to_string()];
            for seg in parent_module_path.segments() {
                path_segs.push(seg.as_str().to_string());
            }
            path_segs.push(parent_name.to_string());
            path_segs.push(name);
            self.paths.insert(
                method_id,
                ItemSummary { crate_id: 0, path: path_segs, kind: ItemKind::Function },
            );

            ids.push(method_id);
        }
        Ok(ids)
    }

    /// Encodes `TraitImplDeclV2` entries as identity-only `Impl` items.
    ///
    /// Returns the list of allocated `Impl` item ids so callers can attach them
    /// to the owning type's `impls` field (ADR 2 D12).
    fn encode_trait_impl_blocks(
        &mut self,
        type_id: Id,
        type_name: &str,
        trait_impls: &[TraitImplDeclV2],
    ) -> Result<Vec<Id>, CatalogueToExtendedCrateCodecError> {
        let mut impl_ids = vec![];
        for ti in trait_impls {
            let impl_id = self.alloc_id();
            let for_type = resolved_path_type(type_id, type_name);

            // `Path.id` is an item id, not a crate id.
            // * Local traits: use the pre-assigned item id from the pre-pass.
            //   Only use the local id when `origin_crate` matches this crate; a
            //   coincidental name match (e.g. a local `Display` vs `std::fmt::Display`)
            //   must not hijack the external trait's path entry.
            // * External traits: allocate a fresh item id and register a `paths` entry
            //   so downstream consumers (e.g. `schema_export`) can recover the trait's
            //   origin crate via `krate.paths[trait_id].crate_id`.
            let is_local_trait = ti.origin_crate.as_str() == self.crate_name.as_str();
            let trait_id = if is_local_trait {
                // Local trait: must have been indexed in the pre-pass. Missing entry is an
                // internal error (the catalogue declares an impl before the trait's own entry).
                self.local_name_to_id.get(ti.trait_name.as_str()).copied().ok_or_else(|| {
                    CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                        type_ref: ti.trait_name.as_str().to_string(),
                        reason: "trait declared as local but not found in pre-pass index"
                            .to_string(),
                    }
                })?
            } else {
                let cn = ti.origin_crate.as_str().to_string();
                let ext_crate_id = self.ensure_external_crate(cn.clone());

                // Allocate a synthetic item id for this external trait so it has a
                // unique `paths` entry (multiple external traits must not share one id).
                let synthetic_trait_id = self.alloc_id();

                // Build the canonical path segments for this external trait.
                // These segments are used by downstream consumers (e.g. `schema_export`)
                // to recover the trait's origin crate via `krate.paths[trait_id].crate_id`.
                // They are NOT used for impl-identity key matching (which goes through
                // `trait_path_str` + `normalize_impl_trait_path`).
                // For std/core traits, use the known full module path (e.g.
                // `std::fmt::Display`, `core::convert::From`) for accurate path metadata.
                // For other crates, fall back to `[crate_name, trait_name]`.
                let trait_path_segments: Vec<String> = if cn == "std" {
                    let canonical =
                        crate::tddd::type_ref_parser::std_canonical_path(ti.trait_name.as_str());
                    canonical.split("::").map(str::to_string).collect()
                } else if cn == "core" {
                    let canonical =
                        crate::tddd::type_ref_parser::core_canonical_path(ti.trait_name.as_str());
                    canonical.split("::").map(str::to_string).collect()
                } else {
                    vec![cn, ti.trait_name.as_str().to_string()]
                };
                self.paths.insert(
                    synthetic_trait_id,
                    ItemSummary {
                        crate_id: ext_crate_id,
                        path: trait_path_segments.clone(),
                        kind: ItemKind::Trait,
                    },
                );
                synthetic_trait_id
            };

            // Build the trait path using the canonical full path string for external traits,
            // or the short name for local traits (which are looked up by id in the index).
            //
            // `normalize_impl_trait_path` (used by `build_impl_identity_map` on S, D, and C)
            // keeps external-crate paths verbatim and strips local-crate prefixes to the
            // short name.  S-side paths must therefore match the C-side rustdoc format:
            //
            // * std traits: `std_canonical_path` returns the full module path
            //   (e.g. `std::fmt::Display`), which rustdoc also uses verbatim.
            // * Non-std external traits (e.g. `serde`, `core`, `futures`): rustdoc emits
            //   the full path `"{crate_name}::{trait_name}"` (e.g. `serde::Serialize`).
            //   Using only the bare short name here would produce an unresolved local-path
            //   key (e.g. `"Serialize"`) that `normalize_impl_trait_path` cannot distinguish
            //   from a local trait, causing an identity-key mismatch and missed evaluation.
            // * Local traits: use the bare short name (no `::` prefix) so that
            //   `normalize_impl_trait_path` correctly identifies it as a local-unresolved
            //   path and strips it to the short name on both S and C sides.
            //
            // When `generic_args` is present (e.g. `"CatalogueLoaderError"` for a specific
            // `#[from]` variant), append them in angle-bracket notation so the generated
            // identity key matches the C-side rustdoc key exactly (e.g. `From<CatalogueLoaderError>`).
            let trait_path_str = {
                let is_external = ti.origin_crate.as_str() != self.crate_name.as_str();
                let base = if is_external {
                    let cn = ti.origin_crate.as_str();
                    if cn == "std" {
                        crate::tddd::type_ref_parser::std_canonical_path(ti.trait_name.as_str())
                    } else if cn == "core" {
                        // `core` traits: emit the fully-qualified canonical path
                        // (e.g. `"core::convert::From"`, `"core::fmt::Display"`).
                        //
                        // `build_impl_identity_map` uses `krate.paths` to resolve C-side
                        // impl trait paths to their canonical qualified form, so S-side and
                        // C-side both produce `"core::convert::From"` as the identity key.
                        // This correctly disambiguates `core::fmt::Display` from a
                        // user-defined local `Display` trait (which would have `crate_id == 0`
                        // in `krate.paths` and would be stripped to its short name).
                        crate::tddd::type_ref_parser::core_canonical_path(ti.trait_name.as_str())
                    } else if cn == "domain" || cn == "usecase" {
                        // `domain`/`usecase` traits: use a two-segment qualified path
                        // (e.g. `"domain::CatalogueLoader"`, `"usecase::SignalEvaluatorPort"`).
                        //
                        // C-side rustdoc emits just the short name for these cross-workspace
                        // crate traits, but `krate.paths` has the canonical `crate_id != 0`
                        // entry with the full path `["{cn}", "{trait_name}"]`.
                        // `build_impl_identity_map` resolves via `krate.paths.join("::")`,
                        // producing `"domain::CatalogueLoader"` — matching this S-side form.
                        format!("{cn}::{}", ti.trait_name.as_str())
                    } else {
                        // For all other external crates, build a two-segment path
                        // `{crate_name}::{trait_name}` (e.g. `serde::Serialize`).
                        // `build_impl_identity_map` uses `krate.paths.join("::")` which
                        // produces `"serde::Serialize"` for a `serde` origin trait — matching
                        // this two-segment form.
                        format!("{cn}::{}", ti.trait_name.as_str())
                    }
                } else {
                    ti.trait_name.as_str().to_string()
                };
                match ti.generic_args() {
                    Some(args) => format!("{base}<{args}>"),
                    None => base,
                }
            };
            let trait_path = Path { path: trait_path_str, id: trait_id, args: None };
            self.index.insert(
                impl_id,
                make_item(
                    impl_id,
                    None,
                    None,
                    ItemEnum::Impl(make_impl(for_type, Some(trait_path), vec![])),
                ),
            );
            impl_ids.push(impl_id);
        }
        Ok(impl_ids)
    }

    /// Encodes a `VariantPayload` into `rustdoc_types::VariantKind`.
    fn encode_variant_kind(
        &mut self,
        payload: VariantPayload,
    ) -> Result<VariantKind, CatalogueToExtendedCrateCodecError> {
        match payload {
            VariantPayload::Unit => Ok(VariantKind::Plain),
            VariantPayload::Tuple(type_refs) => {
                let mut field_ids = vec![];
                for ty_ref in type_refs {
                    let field_id = self.alloc_id();
                    let field_ty = self.parse_type_ref_str(ty_ref.as_str())?;
                    self.index.insert(
                        field_id,
                        make_item(field_id, None, None, ItemEnum::StructField(field_ty)),
                    );
                    field_ids.push(Some(field_id));
                }
                Ok(VariantKind::Tuple(field_ids))
            }
            VariantPayload::Struct(fields) => {
                let mut field_ids = vec![];
                for f in fields {
                    let field_id = self.alloc_id();
                    let field_ty = self.parse_type_ref_str(f.ty.as_str())?;
                    self.index.insert(
                        field_id,
                        make_item(
                            field_id,
                            Some(f.name.as_str().to_string()),
                            None,
                            ItemEnum::StructField(field_ty),
                        ),
                    );
                    field_ids.push(field_id);
                }
                Ok(VariantKind::Struct { fields: field_ids, has_stripped_fields: false })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

/// Creates a `rustdoc_types::Item` with common fixed-value fields.
///
/// Sets `crate_id: 0` (local crate). Use `make_item_with_crate_id` when the item
/// belongs to an external crate.
fn make_item(id: Id, name: Option<String>, docs: Option<String>, inner: ItemEnum) -> Item {
    make_item_with_crate_id(0, id, name, docs, inner)
}

/// Creates a `rustdoc_types::Item` with an explicit `crate_id`.
///
/// Use `0` for items belonging to the document crate; pass the external crate's
/// numeric id for items belonging to a foreign crate.
fn make_item_with_crate_id(
    crate_id: u32,
    id: Id,
    name: Option<String>,
    docs: Option<String>,
    inner: ItemEnum,
) -> Item {
    Item {
        id,
        crate_id,
        name,
        span: None,
        visibility: Visibility::Public,
        docs,
        links: HashMap::new(),
        attrs: vec![],
        deprecation: None,
        inner,
    }
}

/// Returns a `Type::ResolvedPath` for a self-referential / placeholder path.
///
/// `path` is the short type name (without module prefix) used in `Impl.for_` so
/// downstream consumers can identify the owning type by name.
fn resolved_path_type(id: Id, path: &str) -> Type {
    Type::ResolvedPath(Path { path: path.to_string(), id, args: None })
}

/// Builds an `Impl` with the given `for_` type and optional trait.
fn make_impl(for_: Type, trait_: Option<Path>, items: Vec<Id>) -> Impl {
    Impl {
        is_unsafe: false,
        generics: empty_generics(),
        provided_trait_methods: vec![],
        trait_,
        for_,
        items,
        is_synthetic: false,
        is_negative: false,
        blanket_impl: None,
    }
}

/// Returns an empty `rustdoc_types::Generics`.
fn empty_generics() -> Generics {
    Generics { params: vec![], where_predicates: vec![] }
}

/// Converts a `SelfReceiver` into the corresponding `rustdoc_types::Type`.
///
/// Used as the receiver parameter type in `FunctionSignature::inputs`.
fn receiver_type(receiver: SelfReceiver) -> Type {
    match receiver {
        SelfReceiver::Owned => {
            Type::ResolvedPath(Path { path: "Self".to_string(), id: Id(0), args: None })
        }
        SelfReceiver::SharedRef => {
            let inner =
                Type::ResolvedPath(Path { path: "Self".to_string(), id: Id(0), args: None });
            Type::BorrowedRef { lifetime: None, is_mutable: false, type_: Box::new(inner) }
        }
        SelfReceiver::ExclusiveRef => {
            let inner =
                Type::ResolvedPath(Path { path: "Self".to_string(), id: Id(0), args: None });
            Type::BorrowedRef { lifetime: None, is_mutable: true, type_: Box::new(inner) }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — AC-05 / AC-06
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::expect_used)]
mod tests {
    use domain::tddd::CatalogueToExtendedCratePort;
    use domain::tddd::LayerId;
    use domain::tddd::catalogue_v2::composite::{CompositePattern, TypeKindV2};
    use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
    use domain::tddd::catalogue_v2::methods::MethodDeclaration;
    use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction, SelfReceiver};
    use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
    use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl};
    use domain::tddd::catalogue_v2::{
        CatalogueDocument, CrateName, FieldName, MethodName, ModulePath, TraitName, TypeName,
        TypeRef, VariantName,
    };
    use rustdoc_types::{Id, ItemEnum, Type};

    use super::*;
    use crate::tddd::type_ref_parser::UNRESOLVED_CRATE_ID;

    fn make_doc(crate_name: &str) -> CatalogueDocument {
        CatalogueDocument::new(
            2,
            CrateName::new(crate_name).unwrap(),
            LayerId::try_new("domain").expect("static valid"),
        )
    }

    // -----------------------------------------------------------------------
    // Error path: AmbiguousIdentifier
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_returns_ambiguous_identifier_when_type_and_trait_share_name() {
        // A type named "Foo" and a trait named "Foo" in the same catalogue collide
        // in the short-name index, triggering AmbiguousIdentifier.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Foo").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Enum { variants: vec![] },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );
        doc.traits.insert(
            TraitName::new("Foo").unwrap(),
            TraitEntry {
                action: ItemAction::Add,
                role: ContractRole::SpecificationPort,
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let result = CatalogueToExtendedCrateCodec::new().encode(doc);
        assert!(
            result.is_err(),
            "expected error due to name collision between type Foo and trait Foo"
        );
        // The domain error should be AmbiguousTypeName (converted from AmbiguousIdentifier).
        let err = result.unwrap_err();
        assert!(
            matches!(err, domain::tddd::NewTypeGraphCodecError::AmbiguousTypeName(_)),
            "expected AmbiguousTypeName error, got: {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Error path: InvalidTypeRef
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_returns_invalid_type_ref_for_unparseable_field_type() {
        // A struct field with a TypeRef that syn cannot parse triggers InvalidTypeRef.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("BadType").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Struct {
                    pattern: CompositePattern::Plain,
                    fields: vec![FieldDecl::new(
                        FieldName::new("value").unwrap(),
                        // "42invalid" is not a valid Rust type expression.
                        TypeRef::new("String").unwrap(),
                    )],
                },
                methods: vec![MethodDeclaration {
                    name: MethodName::new("get_value").unwrap(),
                    receiver: Some(SelfReceiver::SharedRef),
                    params: vec![],
                    // TypeRef::new accepts any non-empty string; the codec rejects it at syn parse time.
                    returns: TypeRef::new("42invalid").unwrap(),
                    is_async: false,
                    docs: None,
                }],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let result = CatalogueToExtendedCrateCodec::new().encode(doc);
        assert!(result.is_err(), "expected InvalidTypeRef error for unparseable return type");
        let err = result.unwrap_err();
        assert!(
            matches!(err, domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(_)),
            "expected InvalidTypeRef error, got: {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // AC-05: inline → id-ref conversion — struct fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_struct_fields_are_promoted_to_struct_field_items() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("User").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Struct {
                    pattern: CompositePattern::Plain,
                    fields: vec![
                        FieldDecl::new(
                            FieldName::new("email").unwrap(),
                            TypeRef::new("String").unwrap(),
                        ),
                        FieldDecl::new(FieldName::new("id").unwrap(), TypeRef::new("u32").unwrap()),
                    ],
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let struct_field_count = ec
            .krate()
            .index
            .values()
            .filter(|item| matches!(item.inner, ItemEnum::StructField(_)))
            .count();
        assert_eq!(struct_field_count, 2);
    }

    // -----------------------------------------------------------------------
    // AC-05: inline → id-ref conversion — enum variants
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_enum_variants_are_promoted_to_variant_items() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("ItemAction").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Enum {
                    variants: vec![
                        VariantDecl::unit(VariantName::new("Add").unwrap()),
                        VariantDecl::tuple(
                            VariantName::new("Error").unwrap(),
                            vec![TypeRef::new("String").unwrap()],
                        ),
                    ],
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let variant_count = ec
            .krate()
            .index
            .values()
            .filter(|item| matches!(item.inner, ItemEnum::Variant(_)))
            .count();
        assert_eq!(variant_count, 2);
    }

    // -----------------------------------------------------------------------
    // AC-05: 1 type = 1 Inherent Impl block
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_type_with_methods_produces_single_inherent_impl_block() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Email").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Struct { pattern: CompositePattern::Plain, fields: vec![] },
                methods: vec![
                    MethodDeclaration::new(
                        MethodName::new("new").unwrap(),
                        None,
                        vec![],
                        TypeRef::new("Self").unwrap(),
                        false,
                        None,
                    ),
                    MethodDeclaration::new(
                        MethodName::new("as_str").unwrap(),
                        Some(SelfReceiver::SharedRef),
                        vec![],
                        TypeRef::new("str").unwrap(),
                        false,
                        None,
                    ),
                ],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let krate = ec.krate();

        // Exactly 1 inherent Impl block.
        let inherent_impl_count = krate
            .index
            .values()
            .filter(|item| matches!(&item.inner, ItemEnum::Impl(i) if i.trait_.is_none()))
            .count();
        assert_eq!(inherent_impl_count, 1, "expected 1 inherent Impl block");

        // 2 Function items for the methods.
        let fn_count =
            krate.index.values().filter(|item| matches!(item.inner, ItemEnum::Function(_))).count();
        assert_eq!(fn_count, 2);
    }

    // -----------------------------------------------------------------------
    // AC-05: Crate.paths — module_path included
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_paths_includes_module_path_segments() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Draft").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Struct { pattern: CompositePattern::Plain, fields: vec![] },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::from_segments(vec!["review".to_string()]).unwrap(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let summary = ec
            .krate()
            .paths
            .values()
            .find(|s| s.path.last().map(|n| n == "Draft").unwrap_or(false))
            .expect("Draft not found in paths");
        assert_eq!(summary.path, vec!["domain", "review", "Draft"]);
    }

    #[test]
    fn test_encode_paths_crate_root_type_has_two_segment_path() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("UserId").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Struct { pattern: CompositePattern::Plain, fields: vec![] },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let summary = ec
            .krate()
            .paths
            .values()
            .find(|s| s.path.last().map(|n| n == "UserId").unwrap_or(false))
            .expect("UserId not found in paths");
        assert_eq!(summary.path, vec!["domain", "UserId"]);
    }

    // -----------------------------------------------------------------------
    // AC-06: TypeRef generics parse
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_field_with_generic_type_ref_creates_resolved_path_with_args() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Cart").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Struct {
                    pattern: CompositePattern::Plain,
                    fields: vec![FieldDecl::new(
                        FieldName::new("items").unwrap(),
                        TypeRef::new("Vec<String>").unwrap(),
                    )],
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let field_with_args = ec.krate().index.values().find(|item| {
            matches!(&item.inner, ItemEnum::StructField(Type::ResolvedPath(p)) if p.path.contains("Vec") && p.args.is_some())
        });
        assert!(field_with_args.is_some(), "expected Vec<String> field with generic args");
    }

    // -----------------------------------------------------------------------
    // AC-06: std prelude auto-resolution
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_std_prelude_type_creates_std_external_crate_entry() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Foo").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Struct {
                    pattern: CompositePattern::Plain,
                    fields: vec![FieldDecl::new(
                        FieldName::new("name").unwrap(),
                        TypeRef::new("String").unwrap(),
                    )],
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let has_std = ec.krate().external_crates.values().any(|e| e.name == "std");
        assert!(has_std, "expected 'std' in external_crates");
    }

    // -----------------------------------------------------------------------
    // AC-06: unresolved marker for undeclared types
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_undeclared_type_ref_field_gets_unresolved_marker_id() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Foo").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Struct {
                    pattern: CompositePattern::Plain,
                    fields: vec![FieldDecl::new(
                        FieldName::new("error").unwrap(),
                        TypeRef::new("DomainError").unwrap(),
                    )],
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let unresolved = ec.krate().index.values().find(|item| {
            matches!(&item.inner, ItemEnum::StructField(Type::ResolvedPath(p)) if p.id == Id(UNRESOLVED_CRATE_ID))
        });
        assert!(unresolved.is_some(), "expected unresolved marker field item");
    }

    // -----------------------------------------------------------------------
    // AC-05: item_actions populated
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_item_actions_contains_declared_action() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Email").unwrap(),
            TypeEntry {
                action: ItemAction::Modify,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Struct { pattern: CompositePattern::Plain, fields: vec![] },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let has_modify = ec.item_actions().values().any(|a| *a == ItemAction::Modify);
        assert!(has_modify);
    }

    // -----------------------------------------------------------------------
    // AC-05: external_crates from TraitImplDeclV2::origin_crate
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_trait_impl_origin_crate_registered_in_external_crates() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("Foo").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Struct { pattern: CompositePattern::Plain, fields: vec![] },
                methods: vec![],
                trait_impls: vec![TraitImplDeclV2::new(
                    TraitName::new("Serialize").unwrap(),
                    CrateName::new("serde").unwrap(),
                )],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let has_serde = ec.krate().external_crates.values().any(|e| e.name == "serde");
        assert!(has_serde, "expected 'serde' in external_crates");
    }

    // -----------------------------------------------------------------------
    // AC-05: trait entry encoding
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_trait_entry_produces_trait_item() {
        let mut doc = make_doc("domain");
        doc.traits.insert(
            TraitName::new("UserRepository").unwrap(),
            TraitEntry {
                action: ItemAction::Add,
                role: ContractRole::SecondaryPort,
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let trait_item = ec.krate().index.values().find(|item| {
            matches!(&item.inner, ItemEnum::Trait(_))
                && item.name.as_deref() == Some("UserRepository")
        });
        assert!(trait_item.is_some(), "expected Trait item for UserRepository");
    }

    // -----------------------------------------------------------------------
    // Type alias
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_type_alias_produces_type_alias_item() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("UserResult").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::TypeAlias {
                    target: TypeRef::new("Result<User, DomainError>").unwrap(),
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let alias_item = ec.krate().index.values().find(|item| {
            matches!(&item.inner, ItemEnum::TypeAlias(_))
                && item.name.as_deref() == Some("UserResult")
        });
        assert!(alias_item.is_some(), "expected TypeAlias item for UserResult");
    }

    // -----------------------------------------------------------------------
    // Empty catalogue
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_empty_catalogue_produces_root_module() {
        let doc = make_doc("domain");
        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        assert!(ec.krate().index.contains_key(&Id(0)), "expected root Id(0)");
    }

    // -----------------------------------------------------------------------
    // generic_args in TraitImplDeclV2 → trait_path_str includes <X>
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_trait_impl_with_generic_args_produces_impl_with_parameterised_trait_path() {
        // When `generic_args` is Some, the Impl item's trait path must be
        // `"From<CatalogueLoaderError>"` so that `build_impl_identity_map` produces
        // the key `"RenderContractMapError: From<CatalogueLoaderError>"`, matching
        // the C-side rustdoc key exactly.
        let mut doc = make_doc("usecase");
        doc.types.insert(
            TypeName::new("RenderContractMapError").unwrap(),
            TypeEntry {
                action: ItemAction::Modify,
                role: DataRole::ErrorType,
                kind: TypeKindV2::Enum { variants: vec![] },
                methods: vec![],
                trait_impls: vec![
                    TraitImplDeclV2::new_with_generic_args(
                        TraitName::new("From").unwrap(),
                        CrateName::new("core").unwrap(),
                        "CatalogueLoaderError".to_string(),
                    )
                    .unwrap(),
                    TraitImplDeclV2::new_with_generic_args(
                        TraitName::new("From").unwrap(),
                        CrateName::new("core").unwrap(),
                        "ContractMapWriterError".to_string(),
                    )
                    .unwrap(),
                ],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let krate = ec.krate();

        // Collect trait impl paths from all Impl items that have a trait.
        let trait_paths: Vec<String> = krate
            .index
            .values()
            .filter_map(|item| {
                if let ItemEnum::Impl(impl_) = &item.inner {
                    impl_.trait_.as_ref().map(|tp| tp.path.clone())
                } else {
                    None
                }
            })
            .collect();

        // `core::From` with generic_args: emit the fully-qualified `core::convert::From`
        // path with generic args appended.  `build_impl_identity_map` resolves C-side via
        // `krate.paths`, obtaining `"core::convert::From"` as the canonical qualified form.
        // S-side uses `core_canonical_path("From")` = `"core::convert::From"` so both
        // sides produce the same identity key.
        assert!(
            trait_paths.iter().any(|p| p == "core::convert::From<CatalogueLoaderError>"),
            "expected impl trait path 'core::convert::From<CatalogueLoaderError>', got: {trait_paths:?}"
        );
        assert!(
            trait_paths.iter().any(|p| p == "core::convert::From<ContractMapWriterError>"),
            "expected impl trait path 'core::convert::From<ContractMapWriterError>', got: {trait_paths:?}"
        );
    }

    #[test]
    fn test_encode_trait_impl_without_generic_args_produces_impl_with_qualified_core_trait_path() {
        // When `generic_args` is None and `origin_crate` is `"core"`, the impl trait path
        // must be the fully-qualified canonical path (`"core::convert::From"` not bare `"From"`).
        // `build_impl_identity_map` uses `krate.paths` to resolve C-side trait paths to
        // their canonical qualified form (e.g. `"core::convert::From"`) so S-side must
        // emit the same form to avoid identity-key mismatches.
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("SomeError").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ErrorType,
                kind: TypeKindV2::Enum { variants: vec![] },
                methods: vec![],
                trait_impls: vec![TraitImplDeclV2::new(
                    TraitName::new("From").unwrap(),
                    CrateName::new("core").unwrap(),
                )],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let krate = ec.krate();

        let trait_paths: Vec<String> = krate
            .index
            .values()
            .filter_map(|item| {
                if let ItemEnum::Impl(impl_) = &item.inner {
                    impl_.trait_.as_ref().map(|tp| tp.path.clone())
                } else {
                    None
                }
            })
            .collect();

        assert!(
            trait_paths.iter().any(|p| p == "core::convert::From"),
            "expected qualified 'core::convert::From' trait path when generic_args is None, got: {trait_paths:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Struct variant with named fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_enum_struct_variant_produces_named_struct_field_items() {
        let mut doc = make_doc("domain");
        doc.types.insert(
            TypeName::new("ParseError").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ErrorType,
                kind: TypeKindV2::Enum {
                    variants: vec![VariantDecl::struct_variant(
                        VariantName::new("InvalidToken").unwrap(),
                        vec![FieldDecl::new(
                            FieldName::new("message").unwrap(),
                            TypeRef::new("String").unwrap(),
                        )],
                    )],
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let struct_variant = ec.krate().index.values().find(|item| {
            if let ItemEnum::Variant(v) = &item.inner {
                matches!(&v.kind, VariantKind::Struct { fields, .. } if !fields.is_empty())
            } else {
                false
            }
        });
        assert!(struct_variant.is_some(), "expected struct Variant with fields");
    }
}
