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
use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl, VariantPayload};
use domain::tddd::catalogue_v2::{
    BoundOp, CatalogueDocument, CrateName, FunctionPath, MethodDeclaration, MethodGenericParam,
    ModulePath, SelfReceiver, TraitImplDeclV2, TraitName, TypeKindV2, TypeName, WherePredicateDecl,
};
use domain::tddd::extended_crate::ExtendedCrate;
use rustdoc_types::{
    AssocItemConstraint, AssocItemConstraintKind, Crate, DynTrait, ExternalCrate, FORMAT_VERSION,
    FunctionHeader, FunctionSignature, GenericArg, GenericArgs, GenericBound, GenericParamDef,
    GenericParamDefKind, Generics, Id, Impl, Item, ItemEnum, ItemKind, ItemSummary, Module, Path,
    PolyTrait, Struct, StructKind, Target, Term, Trait, Type, TypeAlias, Variant, VariantKind,
    Visibility, WherePredicate,
};

use crate::tddd::catalogue_to_extended_crate_codec_error::CatalogueToExtendedCrateCodecError;
use crate::tddd::type_ref_parser::{UNRESOLVED_CRATE_ID, parse_generic_bound, parse_type_ref};

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
                TypeKindV2::UnitStruct => {
                    state.encode_unit_struct(type_id, type_name, entry)?;
                }
                TypeKindV2::TupleStruct { fields, has_stripped_fields } => {
                    state.encode_tuple_struct(
                        type_id,
                        type_name,
                        entry,
                        fields,
                        has_stripped_fields,
                    )?;
                }
                TypeKindV2::PlainStruct { fields, has_stripped_fields, typestate } => {
                    state.encode_plain_struct(
                        type_id,
                        type_name,
                        entry,
                        fields,
                        has_stripped_fields,
                        typestate,
                    )?;
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
    fn encode_bound_str(
        &mut self,
        bound_str: &str,
    ) -> Result<GenericBound, CatalogueToExtendedCrateCodecError> {
        // Handle `~const` prefix manually because stable syn v2 does not have a
        // `TraitBoundModifier::MaybeConst` variant.  Strip the prefix and encode
        // the remainder as a plain trait bound with MaybeConst modifier.
        if let Some(inner) = bound_str.strip_prefix("~const ") {
            let inner = inner.trim_start();
            // Encode the inner trait path via parse_type_ref_str (no modifier prefix).
            let ty = self.parse_type_ref_str(inner)?;
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
            let ext_snapshot: HashMap<String, u32> = self.ext_name_to_id.clone();
            let mut new_crate_names: Vec<String> = vec![];
            let _ = parse_generic_bound(
                bound_str,
                &|name: &str| local_snapshot.get(name).copied(),
                std_crate_id,
                &ext_snapshot,
                &mut |crate_name: String| {
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
        let ext_snapshot2: HashMap<String, u32> = self.ext_name_to_id.clone();
        parse_generic_bound(
            bound_str,
            &|name: &str| local_snapshot2.get(name).copied(),
            std_crate_id,
            &ext_snapshot2,
            &mut |crate_name: String| self.ensure_external_crate(crate_name),
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
    fn encode_and_validate_bound(
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

    /// Builds rustdoc `Generics` in the **maximally-desugared where form** from
    /// catalogue declarations (`MethodGenericParam`s and `WherePredicateDecl`s).
    ///
    /// All bounds — whether declared inline on a `MethodGenericParam` or explicitly
    /// in a `WherePredicateDecl` — are emitted into `Generics.where_predicates` as
    /// `WherePredicate::BoundPredicate` entries. `GenericParamDef.bounds` is always
    /// empty and `is_synthetic` is always false on the resulting params.
    ///
    /// This mirrors rustdoc's representation of `where` clauses and lets the signal
    /// evaluator compare both sides in a single canonical form
    /// (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D1).
    fn build_where_form_generics(
        &mut self,
        generics_decl: &[MethodGenericParam],
        where_decls: &[WherePredicateDecl],
        generic_names: &[&str],
    ) -> Result<Generics, CatalogueToExtendedCrateCodecError> {
        let mut params: Vec<GenericParamDef> = Vec::with_capacity(generics_decl.len());
        let mut where_predicates: Vec<WherePredicate> = Vec::new();

        // (1) MethodGenericParam → empty-bound `GenericParamDef` + one
        //     `BoundPredicate { type_: Generic(name), bounds }` per param.
        for g in generics_decl {
            let mut bounds: Vec<GenericBound> = Vec::with_capacity(g.bounds.len());
            for b in &g.bounds {
                bounds.push(self.encode_and_validate_bound(b.as_str(), generic_names)?);
            }
            params.push(GenericParamDef {
                name: g.name.as_str().to_owned(),
                kind: GenericParamDefKind::Type {
                    bounds: vec![],
                    default: None,
                    is_synthetic: false,
                },
            });
            if !bounds.is_empty() {
                where_predicates.push(WherePredicate::BoundPredicate {
                    type_: Type::Generic(g.name.as_str().to_owned()),
                    bounds,
                    generic_params: vec![],
                });
            }
        }

        // (2) Explicit `WherePredicateDecl` entries — LHS is an arbitrary type
        //     expression. Parse via `parse_type_ref_str` (with bare-generic-name
        //     short-circuit identical to the param/return logic).
        for w in where_decls {
            let lhs_str = w.lhs.as_str();
            // Guard: a `WherePredicateDecl` with no rhs would encode to
            // `WherePredicate::BoundPredicate { bounds: vec![] }` which is
            // syntactically invalid in Rust (`where T:` without any bound).
            // This mirrors the symmetrical check in `where_predicates_from_dtos`.
            if w.rhs.is_empty() {
                return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                    type_ref: lhs_str.to_owned(),
                    reason: "where predicate has no rhs (`where T:` is not valid Rust); \
                             at least one rhs entry is required"
                        .to_owned(),
                });
            }
            // Reject qualified-path LHS forms (`<T as Trait>::Item`) before they
            // reach `parse_type_ref_str`. `parse_type_ref_str` cannot reconstruct
            // the `Type::QualifiedPath` shape that rustdoc emits for such predicates
            // and degrades them to an unresolved placeholder, producing a silent
            // structural mismatch in the signal evaluator (ADR D3 fail-closed).
            // Note: `use<...>` (precise-capture) bounds in the RHS position are
            // accepted by ADR `2026-05-18-1223` D1 — `parse_generic_bound` encodes
            // them as a best-effort `GenericBound::TraitBound` placeholder.
            if lhs_str.trim().starts_with('<') {
                return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                    type_ref: lhs_str.to_owned(),
                    reason: "qualified-path where-predicate LHS (`<T as Trait>::Assoc`) is not \
                             yet supported in catalogue where-predicates — use simple \
                             associated-type projection (`T::Assoc`) instead"
                        .to_owned(),
                });
            }
            let lhs_type =
                if !generic_names.is_empty() && is_bare_generic_name(lhs_str, generic_names) {
                    // Simple bare generic: `T` → `Type::Generic("T")`
                    Type::Generic(lhs_str.trim().to_string())
                } else if !generic_names.is_empty() {
                    if let Some(proj) = try_build_generic_projection(lhs_str, generic_names) {
                        // Single-level associated-type projection: `T::Item` →
                        // `Type::QualifiedPath { name: "Item", self_type: Generic("T"),
                        //  trait_: None, args: None }`.
                        //
                        // This matches the shape that rustdoc emits for `where T::Item: …`
                        // predicates so that A-catalogue and C-rustdoc representations
                        // compare equal in `build_where_form_view`.
                        proj
                    } else {
                        let raw = self.parse_type_ref_str(lhs_str)?;
                        rewrite_generic_types(raw, generic_names)
                    }
                } else {
                    self.parse_type_ref_str(lhs_str)?
                };
            match w.operator {
                BoundOp::Bound => {
                    let mut bounds: Vec<GenericBound> = Vec::with_capacity(w.rhs.len());
                    for b in &w.rhs {
                        bounds.push(self.encode_and_validate_bound(b.as_str(), generic_names)?);
                    }
                    where_predicates.push(WherePredicate::BoundPredicate {
                        type_: lhs_type,
                        bounds,
                        generic_params: vec![],
                    });
                }
                BoundOp::Equal => {
                    // `Equal` predicates (`where T::Assoc = U`) encode as `EqPredicate`.
                    // Enforce rhs.len() == 1 defensively: decode validates this, but
                    // in-memory domain values constructed outside the codec must also pass.
                    if w.rhs.len() != 1 {
                        return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                            type_ref: lhs_str.to_owned(),
                            reason: format!(
                                "Equal predicate must have exactly one rhs entry (got {}); \
                                 `where T::Assoc = U` accepts a single RHS only",
                                w.rhs.len()
                            ),
                        });
                    }
                    // Mirror the JSON decoder's Equal-LHS invariant: the LHS must be an
                    // associated-type projection (e.g. `T::Assoc`).  A bare type parameter
                    // such as `"T"` would produce `WherePredicate::EqPredicate` for
                    // `where T = U`, which is not valid Rust.  Catching this here prevents
                    // domain values constructed outside the JSON codec path from generating
                    // nonsensical `ExtendedCrate` representations.
                    if !lhs_str.contains("::") {
                        return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                            type_ref: lhs_str.to_owned(),
                            reason: format!(
                                "Equal where-predicate lhs '{}' is not a supported projection \
                                 form; expected a path with at least one '::' segment \
                                 (e.g. `T::Assoc`) — bare type parameters cannot appear as \
                                 the LHS of a `where T = U` equality constraint",
                                lhs_str
                            ),
                        });
                    }
                    // Safe: len == 1 asserted above.
                    let rhs_entry = w.rhs.first().ok_or_else(|| {
                        CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                            type_ref: lhs_str.to_owned(),
                            reason: "Equal predicate has no rhs (codec invariant violated)"
                                .to_owned(),
                        }
                    })?;
                    let rhs_str = rhs_entry.as_str();
                    // Reject qualified-path RHS forms (`<T as Trait>::Assoc`).
                    // `parse_type_ref_str` cannot reconstruct the correct `Type::QualifiedPath`
                    // shape and silently produces an unresolved placeholder, breaking round-trip.
                    if rhs_str.trim().starts_with('<') {
                        return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                            type_ref: rhs_str.to_owned(),
                            reason: "qualified-path Equal predicate RHS \
                                     (`<T as Trait>::Assoc`) is not supported — \
                                     use a simple type expression instead"
                                .to_owned(),
                        });
                    }
                    let rhs_type = if !generic_names.is_empty()
                        && is_bare_generic_name(rhs_str, generic_names)
                    {
                        Type::Generic(rhs_str.trim().to_string())
                    } else if !generic_names.is_empty() {
                        if let Some(proj) = try_build_generic_projection(rhs_str, generic_names) {
                            proj
                        } else {
                            let raw = self.parse_type_ref_str(rhs_str)?;
                            rewrite_generic_types(raw, generic_names)
                        }
                    } else {
                        self.parse_type_ref_str(rhs_str)?
                    };
                    where_predicates.push(WherePredicate::EqPredicate {
                        lhs: lhs_type,
                        rhs: Term::Type(rhs_type),
                    });
                }
            }
        }

        Ok(Generics { params, where_predicates })
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

    /// Encodes a `UnitStruct` kind `TypeEntry`.
    fn encode_unit_struct(
        &mut self,
        type_id: Id,
        type_name: &TypeName,
        entry: &TypeEntry,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        let module_path = entry.module_path.clone();
        let docs = entry.docs.clone();

        // Encode inherent method items.
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

        let mut all_impl_ids = vec![impl_id];
        all_impl_ids.extend(trait_impl_ids);

        let struct_item = make_item(
            type_id,
            Some(type_name.as_str().to_string()),
            docs,
            ItemEnum::Struct(Struct {
                kind: StructKind::Unit,
                generics: empty_generics(),
                impls: all_impl_ids,
            }),
        );
        self.index.insert(type_id, struct_item);
        self.register_path(type_id, ItemKind::Struct, type_name.as_str(), &module_path);
        Ok(())
    }

    /// Encodes a `TupleStruct` kind `TypeEntry`.
    fn encode_tuple_struct(
        &mut self,
        type_id: Id,
        type_name: &TypeName,
        entry: &TypeEntry,
        fields: Vec<domain::tddd::catalogue_v2::identifiers::TypeRef>,
        has_stripped_fields: bool,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        let module_path = entry.module_path.clone();
        let docs = entry.docs.clone();

        // Encode positional fields as StructField items with None names (tuple style).
        // Positional field names (.0, .1, ...) are synthesized by the rustdoc format;
        // the catalogue stores only the types.
        let mut field_ids: Vec<Option<Id>> = vec![];
        for field_ty_ref in &fields {
            let field_id = self.alloc_id();
            let field_ty = self.parse_type_ref_str(field_ty_ref.as_str())?;
            self.index
                .insert(field_id, make_item(field_id, None, None, ItemEnum::StructField(field_ty)));
            field_ids.push(Some(field_id));
        }
        // Represent the presence of private fields as a single trailing None.
        //
        // Note on position fidelity: the catalogue (TupleStruct.has_stripped_fields) records
        // only *whether* private fields exist, not their exact positions.  Rustdoc's
        // StructKind::Tuple preserves exact None-slot positions.  The structural equality
        // check therefore may not distinguish "private field moved before a public field" from
        // an unchanged layout — both produce length-matching vectors if public-field types
        // match.  This is an accepted limitation of the current catalogue schema: we prevent
        // false Blue when private fields are *added or removed* (length change) but cannot
        // surface field-position changes within tuple structs as a Mismatch.
        if has_stripped_fields {
            field_ids.push(None);
        }

        let struct_kind = StructKind::Tuple(field_ids);

        // Encode inherent method items.
        let methods: Vec<MethodDeclaration> = entry.methods.clone();
        let method_ids =
            self.encode_method_items(&methods, true, type_name.as_str(), &module_path)?;

        let impl_id = self.alloc_id();
        let for_type = resolved_path_type(type_id, type_name.as_str());
        self.index.insert(
            impl_id,
            make_item(impl_id, None, None, ItemEnum::Impl(make_impl(for_type, None, method_ids))),
        );

        let trait_impls: Vec<TraitImplDeclV2> = entry.trait_impls.clone();
        let trait_impl_ids =
            self.encode_trait_impl_blocks(type_id, type_name.as_str(), &trait_impls)?;

        let mut all_impl_ids = vec![impl_id];
        all_impl_ids.extend(trait_impl_ids);

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

    /// Encodes a `PlainStruct` kind `TypeEntry`.
    ///
    /// The `typestate` marker (if present) does not affect the rustdoc structure — the
    /// plain struct is encoded identically with or without it. The marker is carried
    /// only at the catalogue domain level for signal evaluation and rendering.
    fn encode_plain_struct(
        &mut self,
        type_id: Id,
        type_name: &TypeName,
        entry: &TypeEntry,
        fields: Vec<FieldDecl>,
        has_stripped_fields: bool,
        _typestate: Option<domain::tddd::catalogue_v2::composite::TypestateMarker>,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        let module_path = entry.module_path.clone();
        let docs = entry.docs.clone();

        // Encode named fields → StructField items.
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
        let struct_kind = StructKind::Plain { fields: field_ids, has_stripped_fields };

        // Encode inherent method items.
        let methods: Vec<MethodDeclaration> = entry.methods.clone();
        let method_ids =
            self.encode_method_items(&methods, true, type_name.as_str(), &module_path)?;

        let impl_id = self.alloc_id();
        let for_type = resolved_path_type(type_id, type_name.as_str());
        self.index.insert(
            impl_id,
            make_item(impl_id, None, None, ItemEnum::Impl(make_impl(for_type, None, method_ids))),
        );

        let trait_impls: Vec<TraitImplDeclV2> = entry.trait_impls.clone();
        let trait_impl_ids =
            self.encode_trait_impl_blocks(type_id, type_name.as_str(), &trait_impls)?;

        let mut all_impl_ids = vec![impl_id];
        all_impl_ids.extend(trait_impl_ids);

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

        // Encode supertrait bounds as GenericBound::TraitBound entries.
        // Each bound string (e.g. "Send", "Sync", "Into<String>") is parsed via
        // `encode_bound_str` so that generic args land in `Path.args` (not embedded
        // in `Path.path`) and external types get proper crate ids for Phase 1 visibility.
        let mut bounds: Vec<GenericBound> = Vec::with_capacity(entry.supertrait_bounds.len());
        for b in &entry.supertrait_bounds {
            bounds.push(self.encode_bound_str(b.as_str())?);
        }

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
                bounds,
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

        // Collect function-level generic parameter names so that occurrences of
        // those names in param/return type strings are encoded as `Type::Generic`
        // rather than as unresolved path markers. Mirrors `encode_method_items`.
        // (ADR `2026-05-08-0248` D14)
        let generic_names: Vec<&str> = entry.generics.iter().map(|g| g.name.as_str()).collect();

        let params: Vec<_> = entry
            .params
            .iter()
            .map(|p| (p.name.as_str().to_string(), p.ty.as_str().to_string()))
            .collect();
        let returns_str = entry.returns.as_str().to_string();

        let mut inputs: Vec<(String, Type)> = vec![];
        for (pname, pty_str) in params {
            let pty = if !generic_names.is_empty() && is_bare_generic_name(&pty_str, &generic_names)
            {
                Type::Generic(pty_str.trim().to_string())
            } else {
                let raw = self.parse_type_ref_str(&pty_str)?;
                if generic_names.is_empty() {
                    raw
                } else {
                    rewrite_generic_types(raw, &generic_names)
                }
            };
            inputs.push((pname, pty));
        }
        let output = if returns_str == "()" {
            None
        } else if !generic_names.is_empty() && is_bare_generic_name(&returns_str, &generic_names) {
            Some(Type::Generic(returns_str.trim().to_string()))
        } else {
            let raw = self.parse_type_ref_str(&returns_str)?;
            Some(if generic_names.is_empty() {
                raw
            } else {
                rewrite_generic_types(raw, &generic_names)
            })
        };

        // Encode function-level generics in the maximally-desugared where form.
        // All bounds (both inline and explicit-where) land in `where_predicates`;
        // `GenericParamDef.bounds` is always empty. The signal evaluator normalizes
        // C-side inline bounds the same way so both sides fingerprint to the same
        // canonical form. (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D1)
        let fn_generics = self.build_where_form_generics(
            &entry.generics,
            &entry.where_predicates,
            &generic_names,
        )?;

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
                generics: fn_generics,
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
    /// `force_has_body` selects how each method's `rustdoc_types::Function.has_body`
    /// is determined:
    /// - `true` — every method gets `has_body: true` regardless of its
    ///   `MethodDeclaration.has_default_impl` value. Used for inherent method
    ///   declarations (concrete implementations always have a body).
    /// - `false` — each method's `has_body` is taken from its
    ///   `MethodDeclaration.has_default_impl` field. Used for trait method
    ///   declarations where required (`has_default_impl: false` → `has_body: false`)
    ///   and provided (`has_default_impl: true` → `has_body: true`) differ per
    ///   method. (ADR `2026-05-08-0248` D13)
    ///
    /// `parent_name` is the short name of the containing type or trait (used to build
    /// `Crate::paths` entries with path `[crate, ...module, ParentName, method_name]`).
    /// `parent_module_path` is the module path of the containing item.
    fn encode_method_items(
        &mut self,
        methods: &[MethodDeclaration],
        force_has_body: bool,
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
            // Collect method-level generic parameter names so that occurrences of
            // those names in param/return type strings are encoded as `Type::Generic`
            // rather than as unresolved path markers.
            //
            // Rustdoc emits `Type::Generic("T")` for generic parameters in function
            // signatures (both bare `T` and composite `Option<T>`, `&T`, etc.).
            // The S-side must match exactly; otherwise Phase 1 reports
            // `UnresolvedTypeRef` and Phase 2 structural comparison mismatches.
            //
            // Strategy:
            // 1. For a bare single-word type string that matches a method generic name
            //    (e.g. `"T"`, `"From"`, `"Display"`), produce `Type::Generic` directly
            //    WITHOUT calling `parse_type_ref_str`.  This avoids the STD_PRELUDE_TYPES
            //    expansion that `parse_type_ref_str` applies to well-known names: e.g.
            //    a generic named `"From"` would otherwise be expanded to the canonical
            //    path `"std::convert::From"`, which `rewrite_generic_types` would then
            //    fail to rewrite back (it checks for single-segment bare paths with no
            //    `::` in them).
            // 2. For all other type strings, parse via `parse_type_ref_str` (composite
            //    types, references, generics-in-generics like `Option<T>`) and then call
            //    `rewrite_generic_types` to replace any inner bare generic occurrences.
            let generic_names: Vec<&str> =
                method.generics.iter().map(|g| g.name.as_str()).collect();

            let param_pairs: Vec<(String, String)> = method
                .params
                .iter()
                .map(|p| (p.name.as_str().to_string(), p.ty.as_str().to_string()))
                .collect();
            for (pname, pty_str) in param_pairs {
                let pty = if !generic_names.is_empty()
                    && is_bare_generic_name(&pty_str, &generic_names)
                {
                    // Bare single-word name that is a method generic: emit directly.
                    // Trim so that whitespace-padded strings (e.g. " T ") produce the
                    // same `Type::Generic("T")` that the normal parser would emit.
                    Type::Generic(pty_str.trim().to_string())
                } else {
                    let raw = self.parse_type_ref_str(&pty_str)?;
                    if generic_names.is_empty() {
                        raw
                    } else {
                        rewrite_generic_types(raw, &generic_names)
                    }
                };
                inputs.push((pname, pty));
            }
            let output = if returns_str == "()" {
                None
            } else if !generic_names.is_empty()
                && is_bare_generic_name(&returns_str, &generic_names)
            {
                // Trim so that whitespace-padded strings produce the same
                // `Type::Generic("T")` that the normal parser would emit.
                Some(Type::Generic(returns_str.trim().to_string()))
            } else {
                let raw = self.parse_type_ref_str(&returns_str)?;
                Some(if generic_names.is_empty() {
                    raw
                } else {
                    rewrite_generic_types(raw, &generic_names)
                })
            };

            // Encode method-level generics in the maximally-desugared where form.
            // Both inline `MethodGenericParam.bounds` and explicit
            // `method.where_predicates` are emitted into `Generics.where_predicates`;
            // `GenericParamDef.bounds` is always empty. This mirrors rustdoc's
            // representation for `where`-form sources and is normalized to match
            // inline-form sources by the signal evaluator.
            // (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D1)
            let method_generics = self.build_where_form_generics(
                &method.generics,
                &method.where_predicates,
                &generic_names,
            )?;

            // Per-method has_body: inherent methods always get true via
            // `force_has_body`; trait methods read from `has_default_impl`.
            let method_has_body = force_has_body || method.has_default_impl;
            let fn_item = make_item(
                method_id,
                Some(name.clone()),
                docs,
                ItemEnum::Function(rustdoc_types::Function {
                    sig: FunctionSignature { inputs, output, is_c_variadic: false },
                    generics: method_generics,
                    has_body: method_has_body,
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
                        // `domain`/`usecase` workspace traits: use the bare short name.
                        //
                        // Rustdoc may or may not include these cross-workspace traits in
                        // `krate.paths`.  When the ID is absent from `krate.paths`, the C-side
                        // `build_impl_identity_map` falls back to the bare `trait_path.path`
                        // string, which rustdoc may emit as just `"CatalogueLoader"` (no crate
                        // prefix).  Using the bare name on the S-side as well ensures both
                        // sides produce the same identity key regardless of whether the C-side
                        // rustdoc includes a `krate.paths` entry for the trait.
                        //
                        // `build_impl_identity_map` strips `domain::` / `usecase::` prefixes
                        // to the short name when they appear in multi-segment `krate.paths`
                        // entries, so the C-side path through `krate.paths` also produces the
                        // bare short name.
                        ti.trait_name.as_str().to_string()
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

/// Recursively rewrites a `Type` tree, replacing any `ResolvedPath` node whose
/// `path` exactly matches a method-level generic parameter name (and whose `id`
/// is `Id(UNRESOLVED_CRATE_ID)` with no generic args) with `Type::Generic(name)`.
///
/// Rustdoc emits `Type::Generic("T")` for generic parameters in function
/// signatures (e.g. `fn foo<T>(x: T)`, `fn foo<T>(x: Option<T>)`).  The
/// catalogue codec must emit the same representation so that Phase 1 / Phase 2
/// structural comparison succeeds.
///
/// Only plain single-segment unresolved paths are replaced — composite args such
/// as `Option<T>` keep their outer `ResolvedPath(Option)` form but have the inner
/// `T` arg rewritten to `GenericArg::Type(Type::Generic("T"))`.
fn rewrite_generic_types(ty: Type, generic_names: &[&str]) -> Type {
    match ty {
        // Single-segment path (no `::` in path, no generic args) whose name is a method-level
        // generic parameter → `Type::Generic`.
        //
        // Method-scope generics take precedence over catalogue-local type resolution.
        // `parse_type_ref_str` may resolve a method generic name (e.g. `"T"`) to a local
        // `ResolvedPath` if the catalogue also declares a type named `"T"`.  Rustdoc always
        // emits `Type::Generic("T")` for method generics, so we must rewrite ANY
        // single-segment no-args path whose name is in `generic_names`, regardless of its Id.
        //
        // Only bare single-segment paths (no `::` in `p.path`) without generic args are
        // eligible: composite outer paths like `Option` in `Option<T>` must NOT be replaced
        // even if a generic happens to share that name.
        Type::ResolvedPath(ref p)
            if p.args.is_none()
                && !p.path.contains("::")
                && generic_names.contains(&p.path.as_str()) =>
        {
            Type::Generic(p.path.clone())
        }
        // Composite ResolvedPath: keep the path but recurse into generic args.
        Type::ResolvedPath(p) => {
            let new_args = p.args.map(|args| Box::new(rewrite_generic_args(*args, generic_names)));
            Type::ResolvedPath(Path { args: new_args, ..p })
        }
        Type::BorrowedRef { lifetime, is_mutable, type_ } => Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_: Box::new(rewrite_generic_types(*type_, generic_names)),
        },
        Type::RawPointer { is_mutable, type_ } => Type::RawPointer {
            is_mutable,
            type_: Box::new(rewrite_generic_types(*type_, generic_names)),
        },
        Type::Tuple(elems) => Type::Tuple(
            elems.into_iter().map(|t| rewrite_generic_types(t, generic_names)).collect(),
        ),
        Type::Slice(inner) => Type::Slice(Box::new(rewrite_generic_types(*inner, generic_names))),
        Type::Array { type_, len } => {
            Type::Array { type_: Box::new(rewrite_generic_types(*type_, generic_names)), len }
        }
        // ImplTrait: recurse into each bound (e.g. `impl Iterator<Item = T>`).
        Type::ImplTrait(bounds) => Type::ImplTrait(
            bounds.into_iter().map(|b| rewrite_generic_types_in_bound(b, generic_names)).collect(),
        ),
        // DynTrait: recurse into each PolyTrait's path args (e.g. `dyn Iterator<Item = T>`).
        Type::DynTrait(dyn_trait) => {
            let new_traits = dyn_trait
                .traits
                .into_iter()
                .map(|pt| {
                    let new_args = pt
                        .trait_
                        .args
                        .map(|args| Box::new(rewrite_generic_args(*args, generic_names)));
                    PolyTrait {
                        trait_: Path { args: new_args, ..pt.trait_ },
                        generic_params: pt.generic_params,
                    }
                })
                .collect();
            Type::DynTrait(DynTrait { traits: new_traits, lifetime: dyn_trait.lifetime })
        }
        // FunctionPointer: recurse into input and output types.
        // A method with a generic `fn(T) -> T` parameter type must have `T` rewritten
        // to `Type::Generic("T")` inside the function pointer signature.
        Type::FunctionPointer(fp) => {
            let new_inputs = fp
                .sig
                .inputs
                .into_iter()
                .map(|(name, ty)| (name, rewrite_generic_types(ty, generic_names)))
                .collect();
            let new_output = fp.sig.output.map(|t| rewrite_generic_types(t, generic_names));
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
        // Primitive, Generic, Infer, QualifiedPath: leave unchanged.
        other => other,
    }
}

/// Rewrites method-generic names that appear as type arguments inside a `GenericBound`.
///
/// `encode_bound_str` produces `GenericBound::TraitBound { trait_: Path, ... }`.
/// If the bound has generic args (e.g. `Into<U>`) and `U` is a method-level generic,
/// the `U` arg will be `ResolvedPath(UNRESOLVED_CRATE_ID)` after parsing.  This
/// function rewrites those occurrences to `Type::Generic("U")` so that Phase 1
/// does not misreport them as unresolved catalogue types.
fn rewrite_generic_types_in_bound(bound: GenericBound, generic_names: &[&str]) -> GenericBound {
    match bound {
        GenericBound::TraitBound { trait_: path, generic_params, modifier } => {
            let new_args =
                path.args.map(|args| Box::new(rewrite_generic_args(*args, generic_names)));
            GenericBound::TraitBound {
                trait_: Path { args: new_args, ..path },
                generic_params,
                modifier,
            }
        }
        // Outlives bounds have no nested types.
        GenericBound::Outlives(_) => bound,
        // Use bound (e.g. `T: use<'a>`) has no type args to rewrite.
        GenericBound::Use(_) => bound,
    }
}

/// Recursively rewrites generic args inside a `GenericArgs` node.
///
/// For `AngleBracketed` args, rewrites both type arguments and associated-type
/// constraint values (e.g. `Iterator<Item = T>` where `T` is a method generic).
fn rewrite_generic_args(args: GenericArgs, generic_names: &[&str]) -> GenericArgs {
    match args {
        GenericArgs::AngleBracketed { args: arg_list, constraints } => {
            let new_args = arg_list
                .into_iter()
                .map(|a| match a {
                    GenericArg::Type(t) => {
                        GenericArg::Type(rewrite_generic_types(t, generic_names))
                    }
                    other => other,
                })
                .collect();
            // Also rewrite types inside associated-type constraints
            // (e.g. `Iterator<Item = T>` where `T` is a method generic).
            let new_constraints = constraints
                .into_iter()
                .map(|c| rewrite_assoc_constraint(c, generic_names))
                .collect();
            GenericArgs::AngleBracketed { args: new_args, constraints: new_constraints }
        }
        GenericArgs::Parenthesized { inputs, output } => {
            let new_inputs =
                inputs.into_iter().map(|t| rewrite_generic_types(t, generic_names)).collect();
            let new_output = output.map(|t| rewrite_generic_types(t, generic_names));
            GenericArgs::Parenthesized { inputs: new_inputs, output: new_output }
        }
        // ReturnTypeNotation (`(..)`) has no nested types to rewrite.
        GenericArgs::ReturnTypeNotation => GenericArgs::ReturnTypeNotation,
    }
}

/// Rewrites method-generic names inside an `AssocItemConstraint`.
///
/// Handles all three constraint variants:
/// - `Equality(Term::Type(T))` (e.g. `Item = T`) — rewrites `T` if it matches a generic name.
/// - `Constraint(Vec<GenericBound>)` (e.g. `Item: Into<T>`) — rewrites each bound via
///   `rewrite_generic_types_in_bound` so trait-path type args (e.g. `T` in `Into<T>`) are
///   also rewritten to `Type::Generic("T")` when `T` is a method generic name.
/// - `Equality(Term::Const(_))` — left unchanged (no type parameter to rewrite).
fn rewrite_assoc_constraint(
    constraint: AssocItemConstraint,
    generic_names: &[&str],
) -> AssocItemConstraint {
    let new_args = constraint.args.map(|args| Box::new(rewrite_generic_args(*args, generic_names)));
    let new_binding = match constraint.binding {
        AssocItemConstraintKind::Equality(Term::Type(ty)) => {
            AssocItemConstraintKind::Equality(Term::Type(rewrite_generic_types(ty, generic_names)))
        }
        AssocItemConstraintKind::Constraint(bounds) => {
            // `Item: T` bound constraints — T may be a method generic name.
            AssocItemConstraintKind::Constraint(
                bounds
                    .into_iter()
                    .map(|b| rewrite_generic_types_in_bound(b, generic_names))
                    .collect(),
            )
        }
        // Const equality: no type to rewrite.
        other => other,
    };
    AssocItemConstraint { name: constraint.name, args: new_args, binding: new_binding }
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

/// Returns `true` when `type_str` is a bare single-word identifier that matches
/// one of the method-level generic parameter names in `generic_names`.
///
/// "Bare single-word" means:
/// - No `::` (not a qualified path like `std::fmt::Display`)
/// - No `<` or `>` (not a generic application like `Option<T>`)
/// - No `'` prefix (not a lifetime)
/// - No `&`, `*`, `[`, `(` (not a reference, pointer, slice, or tuple)
///
/// This pre-check prevents `parse_type_ref_str` from expanding well-known names
/// via `STD_PRELUDE_TYPES` (e.g. `"From"` → `"std::convert::From"`) before
/// `rewrite_generic_types` gets a chance to recognise and replace them.
fn is_bare_generic_name(type_str: &str, generic_names: &[&str]) -> bool {
    // Quick character-level checks before the slice lookup.
    let t = type_str.trim();
    if t.is_empty()
        || t.contains("::")
        || t.contains('<')
        || t.contains('>')
        || t.contains('\'')
        || t.contains('&')
        || t.contains('*')
        || t.contains('[')
        || t.contains('(')
    {
        return false;
    }
    generic_names.contains(&t)
}

/// If `type_str` is a single-level associated-type projection path whose LHS is a
/// known generic parameter (`T::Item`), build the corresponding
/// `Type::QualifiedPath` that matches what rustdoc emits for such predicates.
///
/// This covers the form `GENERIC_PARAM::ASSOC_IDENT` (no extra `::` nesting, no
/// angle-bracket args on the associated type).  More complex forms (`T::Item<U>`,
/// `<T as Trait>::Assoc`, multi-level `T::A::B`) return `None` so the caller can
/// fall back to `parse_type_ref_str`.
///
/// Background: rustdoc represents `where T::Item: Send` as
/// `WherePredicate::BoundPredicate { type_: Type::QualifiedPath { name: "Item",
/// self_type: Generic("T"), trait_: None, args: None }, ... }`.  `parse_type_ref_str`
/// cannot produce this shape (it treats the first segment as a crate name), so we
/// must handle the pattern here before falling through to the parser.
fn try_build_generic_projection(type_str: &str, generic_names: &[&str]) -> Option<Type> {
    let t = type_str.trim();
    // Must contain exactly one `::` separator (two-segment form only).
    let sep_pos = t.find("::")?;
    let prefix = &t[..sep_pos];
    let rest = &t[sep_pos + 2..];
    // No further `::` in the rest (single-level projection only).
    if rest.contains("::") {
        return None;
    }
    // No angle brackets (associated type with no generic args).
    if rest.contains('<') || rest.contains('>') {
        return None;
    }
    // Prefix must be a known generic parameter name.
    if !generic_names.contains(&prefix) {
        return None;
    }
    // `rest` must be a valid identifier (non-empty, starts with letter or `_`).
    let first_char = rest.chars().next()?;
    if !first_char.is_ascii_alphabetic() && first_char != '_' {
        return None;
    }
    Some(Type::QualifiedPath {
        name: rest.to_string(),
        args: None,
        self_type: Box::new(Type::Generic(prefix.to_string())),
        trait_: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::expect_used)]
#[path = "catalogue_to_extended_crate_codec_tests.rs"]
mod tests;
