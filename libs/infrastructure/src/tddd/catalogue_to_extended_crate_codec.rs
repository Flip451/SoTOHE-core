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
    CatalogueDocument, CrateName, FunctionPath, MethodDeclaration, MethodGenericParam, ModulePath,
    SelfReceiver, TraitImplDeclV2, TraitName, TypeKindV2, TypeName, WherePredicateDecl,
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

    /// Validates that `bound` is a supported `GenericBound` variant for use inside
    /// `WherePredicate::BoundPredicate.bounds` (or inline `GenericParamDef.bounds`).
    ///
    /// Per ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D3, the scope
    /// of this track's `where_predicates` is `BoundPredicate` only:
    ///
    /// - `GenericBound::Outlives` (`T: 'a`) — fail-closed (lifetime predicates are
    ///   covered by `WherePredicate::LifetimePredicate`, deferred to a follow-up ADR).
    /// - `GenericBound::TraitBound` with non-empty `generic_params` (HRTB on TraitBound,
    ///   e.g. `for<'a> Fn(&'a T)`) — fail-closed.
    /// - `GenericBound::Use` — fail-closed (precise-capturing `use<...>`).
    ///
    /// Plain `GenericBound::TraitBound { generic_params: [], modifier, trait_ }` is the
    /// only supported variant; `modifier` (None / Maybe / MaybeConst) is preserved as-is.
    fn validate_supported_bound(
        bound: &GenericBound,
        bound_str: &str,
    ) -> Result<(), CatalogueToExtendedCrateCodecError> {
        match bound {
            GenericBound::TraitBound { generic_params, .. } if !generic_params.is_empty() => {
                Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                    type_ref: bound_str.to_string(),
                    reason: "higher-ranked trait bounds (HRTB) on TraitBound are not supported \
                             in catalogue where-predicates (ADR 2026-05-13-1153 D3)"
                        .to_string(),
                })
            }
            GenericBound::Outlives(_) => Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                type_ref: bound_str.to_string(),
                reason: "lifetime outlives bounds (`T: 'a`) are not supported in catalogue \
                         where-predicates (ADR 2026-05-13-1153 D3, deferred to follow-up)"
                    .to_string(),
            }),
            GenericBound::Use(_) => Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                type_ref: bound_str.to_string(),
                reason: "precise-capturing `use<...>` bounds are not supported in catalogue \
                         where-predicates (ADR 2026-05-13-1153 D3)"
                    .to_string(),
            }),
            GenericBound::TraitBound { .. } => Ok(()),
        }
    }

    /// Encodes a `MethodGenericParam.bounds[i]` or `WherePredicateDecl.bounds[i]` entry
    /// to a validated `GenericBound`, applying generic-name rewriting and the supported-
    /// bound validation (ADR `2026-05-13-1153` D1 + D3).
    ///
    /// A syntactic pre-check rejects precise-capture `use<...>` bounds before the
    /// internal parser is reached, because `encode_bound_str` (via the shared
    /// `parse_generic_bound` helper) currently encodes `use<...>` as a placeholder
    /// `GenericBound::TraitBound` rather than a `GenericBound::Use`, which would
    /// otherwise slip past `validate_supported_bound`. The check handles optional
    /// whitespace between the `use` keyword and `<` (e.g. `use <U>`).
    fn encode_and_validate_bound(
        &mut self,
        bound_str: &str,
        generic_names: &[&str],
    ) -> Result<GenericBound, CatalogueToExtendedCrateCodecError> {
        // Reject precise-capture `use<...>` bounds before the internal parser is reached.
        // `parse_generic_bound` silently encodes them as a placeholder `TraitBound`, which
        // would bypass `validate_supported_bound`. The syntactic keyword `use` may be
        // followed by optional whitespace before `<` (e.g. `use <U>`), so we check for
        // the `use` keyword (followed by `<` or ASCII whitespace) rather than just the
        // four-character literal `use<`.
        {
            let trimmed = bound_str.trim_start();
            let is_use_bound = trimmed.starts_with("use<")
                || (trimmed.starts_with("use")
                    && trimmed[3..].starts_with(|c: char| c == '<' || c.is_ascii_whitespace()));
            if is_use_bound {
                return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                    type_ref: bound_str.to_string(),
                    reason: "precise-capturing `use<...>` bounds are not supported in catalogue \
                             where-predicates (ADR 2026-05-13-1153 D3)"
                        .to_string(),
                });
            }
        }
        let raw = self.encode_bound_str(bound_str)?;
        let rewritten = if generic_names.is_empty() {
            raw
        } else {
            rewrite_generic_types_in_bound(raw, generic_names)
        };
        Self::validate_supported_bound(&rewritten, bound_str)?;
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
            let type_str = w.type_.as_str();
            // Guard: a `WherePredicateDecl` with no bounds would encode to
            // `WherePredicate::BoundPredicate { bounds: vec![] }` which is
            // syntactically invalid in Rust (`where T:` without any bound).
            // This mirrors the symmetrical check in `where_predicates_from_dtos`.
            if w.bounds.is_empty() {
                return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                    type_ref: type_str.to_owned(),
                    reason: "where predicate has no bounds (`where T:` is not valid Rust); \
                             at least one bound is required"
                        .to_owned(),
                });
            }
            // Reject qualified-path LHS forms (`<T as Trait>::Item`) before they
            // reach `parse_type_ref_str`. `parse_type_ref_str` cannot reconstruct
            // the `Type::QualifiedPath` shape that rustdoc emits for such predicates
            // and degrades them to an unresolved placeholder, producing a silent
            // structural mismatch in the signal evaluator (ADR D3 fail-closed).
            // Failing early here matches the reject-unknown strategy for other
            // unsupported syntax (e.g. `use<...>` bounds).
            if type_str.trim().starts_with('<') {
                return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                    type_ref: type_str.to_owned(),
                    reason: "qualified-path where-predicate LHS (`<T as Trait>::Assoc`) is not \
                             yet supported in catalogue where-predicates — use simple \
                             associated-type projection (`T::Assoc`) instead"
                        .to_owned(),
                });
            }
            let lhs_type =
                if !generic_names.is_empty() && is_bare_generic_name(type_str, generic_names) {
                    // Simple bare generic: `T` → `Type::Generic("T")`
                    Type::Generic(type_str.trim().to_string())
                } else if !generic_names.is_empty() {
                    if let Some(proj) = try_build_generic_projection(type_str, generic_names) {
                        // Single-level associated-type projection: `T::Item` →
                        // `Type::QualifiedPath { name: "Item", self_type: Generic("T"),
                        //  trait_: None, args: None }`.
                        //
                        // This matches the shape that rustdoc emits for `where T::Item: …`
                        // predicates so that A-catalogue and C-rustdoc representations
                        // compare equal in `build_where_form_view`.
                        proj
                    } else {
                        let raw = self.parse_type_ref_str(type_str)?;
                        rewrite_generic_types(raw, generic_names)
                    }
                } else {
                    self.parse_type_ref_str(type_str)?
                };
            let mut bounds: Vec<GenericBound> = Vec::with_capacity(w.bounds.len());
            for b in &w.bounds {
                bounds.push(self.encode_and_validate_bound(b.as_str(), generic_names)?);
            }
            where_predicates.push(WherePredicate::BoundPredicate {
                type_: lhs_type,
                bounds,
                generic_params: vec![],
            });
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

// ---------------------------------------------------------------------------
// Tests — AC-05 / AC-06
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::expect_used)]
mod tests {
    use domain::tddd::CatalogueToExtendedCratePort;
    use domain::tddd::LayerId;
    use domain::tddd::catalogue_v2::composite::TypeKindV2;
    use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
    use domain::tddd::catalogue_v2::methods::{
        MethodDeclaration, MethodGenericParam, ParamDeclaration,
    };
    use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction, SelfReceiver};
    use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
    use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl};
    use domain::tddd::catalogue_v2::{
        CatalogueDocument, CrateName, FieldName, MethodName, ModulePath, ParamName, TraitName,
        TypeName, TypeRef, VariantName,
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
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        doc.traits.insert(
            TraitName::new("Foo").unwrap(),
            TraitEntry {
                action: ItemAction::Add,
                role: ContractRole::SpecificationPort,
                methods: vec![],
                supertrait_bounds: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
                kind: TypeKindV2::PlainStruct {
                    fields: vec![FieldDecl::new(
                        FieldName::new("value").unwrap(),
                        // "42invalid" is not a valid Rust type expression.
                        TypeRef::new("String").unwrap(),
                    )],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![MethodDeclaration {
                    name: MethodName::new("get_value").unwrap(),
                    receiver: Some(SelfReceiver::SharedRef),
                    params: vec![],
                    // TypeRef::new accepts any non-empty string; the codec rejects it at syn parse time.
                    returns: TypeRef::new("42invalid").unwrap(),
                    is_async: false,
                    has_default_impl: false,
                    generics: vec![],
                    where_predicates: vec![],
                    docs: None,
                }],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
                kind: TypeKindV2::PlainStruct {
                    fields: vec![
                        FieldDecl::new(
                            FieldName::new("email").unwrap(),
                            TypeRef::new("String").unwrap(),
                        ),
                        FieldDecl::new(FieldName::new("id").unwrap(), TypeRef::new("u32").unwrap()),
                    ],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
                spec_refs: vec![],
                informal_grounds: vec![],
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
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
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
                spec_refs: vec![],
                informal_grounds: vec![],
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
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::from_segments(vec!["review".to_string()]).unwrap(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
                kind: TypeKindV2::PlainStruct {
                    fields: vec![FieldDecl::new(
                        FieldName::new("items").unwrap(),
                        TypeRef::new("Vec<String>").unwrap(),
                    )],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
                kind: TypeKindV2::PlainStruct {
                    fields: vec![FieldDecl::new(
                        FieldName::new("name").unwrap(),
                        TypeRef::new("String").unwrap(),
                    )],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
                kind: TypeKindV2::PlainStruct {
                    fields: vec![FieldDecl::new(
                        FieldName::new("error").unwrap(),
                        TypeRef::new("DomainError").unwrap(),
                    )],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                trait_impls: vec![TraitImplDeclV2::new(
                    TraitName::new("Serialize").unwrap(),
                    CrateName::new("serde").unwrap(),
                )],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
                supertrait_bounds: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
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
                spec_refs: vec![],
                informal_grounds: vec![],
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
                spec_refs: vec![],
                informal_grounds: vec![],
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
                spec_refs: vec![],
                informal_grounds: vec![],
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
                spec_refs: vec![],
                informal_grounds: vec![],
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

    // -----------------------------------------------------------------------
    // AC-method-generics: method generic params are encoded as Type::Generic
    // -----------------------------------------------------------------------

    /// A method with `generics: [{ name: "T", bounds: ["Into<String>"] }]` and
    /// a parameter of type `"T"` must encode that parameter as `Type::Generic("T")`,
    /// not as a `ResolvedPath`.  Rustdoc emits `Type::Generic` for method-level
    /// generic type parameters, so the S-side must match.
    #[test]
    fn test_encode_method_generic_param_type_emits_type_generic() {
        let mut doc = make_doc("domain");
        let mut method = MethodDeclaration::new(
            MethodName::new("set_value").unwrap(),
            Some(SelfReceiver::ExclusiveRef),
            vec![ParamDeclaration::new(
                ParamName::new("value").unwrap(),
                TypeRef::new("T").unwrap(),
            )],
            TypeRef::new("()").unwrap(),
            false,
            None,
        );
        method.generics = vec![MethodGenericParam {
            name: ParamName::new("T").unwrap(),
            bounds: vec![TypeRef::new("Into<String>").unwrap()],
        }];
        doc.types.insert(
            TypeName::new("ValueHolder").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![method],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let krate = ec.krate();
        // Find the method Function item (set_value).
        let fn_item = krate.index.values().find(|item| {
            item.name.as_deref() == Some("set_value") && matches!(item.inner, ItemEnum::Function(_))
        });
        assert!(fn_item.is_some(), "expected Function item for set_value");
        let ItemEnum::Function(ref f) = fn_item.unwrap().inner else { panic!("expected Function") };
        // The first input is "self" (ExclusiveRef); the second is the "value: T" param.
        let value_param = f.sig.inputs.iter().find(|(name, _)| name == "value");
        assert!(value_param.is_some(), "expected input named 'value'");
        let (_, ty) = value_param.unwrap();
        assert!(
            matches!(ty, Type::Generic(g) if g == "T"),
            "expected Type::Generic(\"T\") for generic param type, got: {ty:?}"
        );
    }

    // -----------------------------------------------------------------------
    // ADR 0248 D13: per-method `has_body` from `has_default_impl` (Gap 1)
    // -----------------------------------------------------------------------

    /// A trait method declared with `has_default_impl: true` (provided default impl)
    /// must encode to `rustdoc_types::Function.has_body = true` so that A-side and
    /// C-side fingerprints both emit `;body` and `structurally_equal` returns true.
    #[test]
    fn test_encode_trait_method_with_has_default_impl_true_produces_has_body_true() {
        let mut doc = make_doc("usecase");
        let mut method = MethodDeclaration::new(
            MethodName::new("describe").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("String").unwrap(),
            false,
            None,
        );
        method.has_default_impl = true;
        doc.traits.insert(
            TraitName::new("Describable").unwrap(),
            TraitEntry {
                action: ItemAction::Add,
                role: ContractRole::SpecificationPort,
                methods: vec![method],
                supertrait_bounds: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let fn_item = ec
            .krate()
            .index
            .values()
            .find(|item| {
                item.name.as_deref() == Some("describe")
                    && matches!(item.inner, ItemEnum::Function(_))
            })
            .expect("expected Function item for describe");
        let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
        assert!(
            f.has_body,
            "trait method with has_default_impl=true must encode has_body=true (ADR 0248 D13)"
        );
    }

    /// A trait method declared with `has_default_impl: false` (required / abstract)
    /// must encode to `rustdoc_types::Function.has_body = false` so that A-side and
    /// C-side fingerprints both emit `;abstract`.
    #[test]
    fn test_encode_trait_method_with_has_default_impl_false_produces_has_body_false() {
        let mut doc = make_doc("usecase");
        let method = MethodDeclaration::new(
            MethodName::new("required_op").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("()").unwrap(),
            false,
            None,
        );
        // has_default_impl defaults to false via MethodDeclaration::new.
        assert!(!method.has_default_impl);
        doc.traits.insert(
            TraitName::new("RequiredOps").unwrap(),
            TraitEntry {
                action: ItemAction::Add,
                role: ContractRole::SpecificationPort,
                methods: vec![method],
                supertrait_bounds: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let fn_item = ec
            .krate()
            .index
            .values()
            .find(|item| {
                item.name.as_deref() == Some("required_op")
                    && matches!(item.inner, ItemEnum::Function(_))
            })
            .expect("expected Function item for required_op");
        let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
        assert!(
            !f.has_body,
            "trait method with has_default_impl=false must encode has_body=false (ADR 0248 D13)"
        );
    }

    /// Inherent method `has_body` is forced to `true` regardless of the
    /// `has_default_impl` field (which is not semantically meaningful for inherent
    /// methods). This preserves the pre-D13 invariant for struct inherent impls.
    #[test]
    fn test_encode_inherent_method_always_has_body_true_regardless_of_has_default_impl() {
        let mut doc = make_doc("domain");
        // Even if the catalogue accidentally sets has_default_impl=false on an
        // inherent method, the encoder must still emit has_body=true.
        let method = MethodDeclaration::new(
            MethodName::new("compute").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("u32").unwrap(),
            false,
            None,
        );
        assert!(!method.has_default_impl);
        doc.types.insert(
            TypeName::new("Calculator").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![method],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let fn_item = ec
            .krate()
            .index
            .values()
            .find(|item| {
                item.name.as_deref() == Some("compute")
                    && matches!(item.inner, ItemEnum::Function(_))
            })
            .expect("expected Function item for compute");
        let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
        assert!(
            f.has_body,
            "inherent method must always encode has_body=true (force_has_body invariant)"
        );
    }

    // -----------------------------------------------------------------------
    // ADR 0248 D14: FunctionEntry.generics → Function.generics (Gap 2)
    // -----------------------------------------------------------------------

    /// A free function with generic parameters must encode `entry.generics` as
    /// `Function.generics`, and any param/return type that names one of those
    /// generics must be emitted as `Type::Generic(_)` rather than as an
    /// unresolved path. Mirrors `MethodDeclaration.generics` handling.
    #[test]
    fn test_encode_function_with_generics_emits_type_generic_in_signature() {
        use domain::tddd::catalogue_v2::FunctionName;
        use domain::tddd::catalogue_v2::entries::FunctionEntry;
        use domain::tddd::catalogue_v2::identifiers::FunctionPath;
        use domain::tddd::catalogue_v2::roles::FunctionRole;

        let mut doc = make_doc("domain");
        let crate_n = CrateName::new("domain").unwrap();
        let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("generic_fn").unwrap());
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![ParamDeclaration::new(
                ParamName::new("value").unwrap(),
                TypeRef::new("T").unwrap(),
            )],
            returns: TypeRef::new("T").unwrap(),
            is_async: false,
            generics: vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![TypeRef::new("Clone").unwrap()],
            }],
            where_predicates: vec![],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.functions.insert(fn_path, entry);

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let fn_item = ec
            .krate()
            .index
            .values()
            .find(|item| {
                item.name.as_deref() == Some("generic_fn")
                    && matches!(item.inner, ItemEnum::Function(_))
            })
            .expect("expected Function item for generic_fn");
        let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

        // generics participates: 1 type-param `T` with bound `Clone`.
        assert_eq!(
            f.generics.params.len(),
            1,
            "expected 1 generic param, got {:?}",
            f.generics.params
        );
        assert_eq!(f.generics.params[0].name, "T");

        // The first input is `value: T` — must be Type::Generic("T").
        let (pname, pty) = &f.sig.inputs[0];
        assert_eq!(pname, "value");
        assert!(
            matches!(pty, Type::Generic(g) if g == "T"),
            "expected Type::Generic(\"T\") for `value` param, got {pty:?}"
        );

        // Return type is `T` — must be Type::Generic("T").
        let output = f.sig.output.as_ref().expect("expected Some output");
        assert!(
            matches!(output, Type::Generic(g) if g == "T"),
            "expected Type::Generic(\"T\") for return, got {output:?}"
        );
    }

    /// A free function with no generics emits `empty_generics()` (no params,
    /// no where_predicates). This preserves the pre-D14 baseline for the vast
    /// majority of free functions in the workspace.
    #[test]
    fn test_encode_function_without_generics_emits_empty_generics() {
        use domain::tddd::catalogue_v2::FunctionName;
        use domain::tddd::catalogue_v2::entries::FunctionEntry;
        use domain::tddd::catalogue_v2::identifiers::FunctionPath;
        use domain::tddd::catalogue_v2::roles::FunctionRole;

        let mut doc = make_doc("domain");
        let crate_n = CrateName::new("domain").unwrap();
        let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("simple").unwrap());
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.functions.insert(fn_path, entry);

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let fn_item = ec
            .krate()
            .index
            .values()
            .find(|item| {
                item.name.as_deref() == Some("simple")
                    && matches!(item.inner, ItemEnum::Function(_))
            })
            .expect("expected Function item for simple");
        let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };
        assert!(
            f.generics.params.is_empty() && f.generics.where_predicates.is_empty(),
            "function without generics must emit empty Generics"
        );
    }

    /// A catalogue `WherePredicateDecl.bounds[i]` whose string form starts with `use<`
    /// must be rejected at encode time (ADR 2026-05-13-1153 D3). The syntactic
    /// pre-check fires before the internal parser, which currently converts `use<...>`
    /// to a placeholder `GenericBound::TraitBound` that would otherwise bypass the
    /// later `validate_supported_bound` check.
    #[test]
    fn test_encode_function_with_use_capture_bound_in_where_predicate_returns_error() {
        use domain::tddd::catalogue_v2::FunctionName;
        use domain::tddd::catalogue_v2::entries::FunctionEntry;
        use domain::tddd::catalogue_v2::identifiers::FunctionPath;
        use domain::tddd::catalogue_v2::methods::{MethodGenericParam, WherePredicateDecl};
        use domain::tddd::catalogue_v2::roles::FunctionRole;
        use domain::tddd::catalogue_v2::{ParamName, TypeRef};

        let mut doc = make_doc("domain");
        let crate_n = CrateName::new("domain").unwrap();
        let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("bad_use").unwrap());
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![
                MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] },
                MethodGenericParam { name: ParamName::new("U").unwrap(), bounds: vec![] },
            ],
            where_predicates: vec![WherePredicateDecl {
                type_: TypeRef::new("T").unwrap(),
                bounds: vec![TypeRef::new("use<U>").unwrap()],
            }],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.functions.insert(fn_path, entry);

        let result = CatalogueToExtendedCrateCodec::new().encode(doc);
        assert!(
            matches!(result, Err(NewTypeGraphCodecError::InvalidTypeRef(_))),
            "expected InvalidTypeRef for use<...> bound, got: {result:?}"
        );
    }

    /// Same as the previous test but the precise-capture bound has a space between the
    /// `use` keyword and the `<` token (i.e. `use <U>`). This variant must also be
    /// rejected — the pre-check must handle optional whitespace after `use`.
    #[test]
    fn test_encode_function_with_use_capture_bound_with_space_returns_error() {
        use domain::tddd::catalogue_v2::FunctionName;
        use domain::tddd::catalogue_v2::entries::FunctionEntry;
        use domain::tddd::catalogue_v2::identifiers::FunctionPath;
        use domain::tddd::catalogue_v2::methods::{MethodGenericParam, WherePredicateDecl};
        use domain::tddd::catalogue_v2::roles::FunctionRole;
        use domain::tddd::catalogue_v2::{ParamName, TypeRef};

        let mut doc = make_doc("domain");
        let crate_n = CrateName::new("domain").unwrap();
        let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("bad_use_space").unwrap());
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![
                MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] },
                MethodGenericParam { name: ParamName::new("U").unwrap(), bounds: vec![] },
            ],
            where_predicates: vec![WherePredicateDecl {
                type_: TypeRef::new("T").unwrap(),
                // Precise-capture with whitespace between `use` and `<`.
                bounds: vec![TypeRef::new("use <U>").unwrap()],
            }],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.functions.insert(fn_path, entry);

        let result = CatalogueToExtendedCrateCodec::new().encode(doc);
        assert!(
            matches!(result, Err(NewTypeGraphCodecError::InvalidTypeRef(_))),
            "expected InvalidTypeRef for `use <U>` (spaced) bound, got: {result:?}"
        );
    }

    /// A `WherePredicateDecl` whose `type_` is a qualified-path form
    /// (`<T as Trait>::Assoc`) must be rejected at encode time. The A-codec
    /// cannot reconstruct the `Type::QualifiedPath` shape that rustdoc emits for
    /// such predicates — `parse_type_ref_str` degrades it to an unresolved
    /// placeholder which silently breaks structural equality.
    #[test]
    fn test_encode_function_with_qualified_path_lhs_in_where_predicate_returns_error() {
        use domain::tddd::catalogue_v2::FunctionName;
        use domain::tddd::catalogue_v2::entries::FunctionEntry;
        use domain::tddd::catalogue_v2::identifiers::FunctionPath;
        use domain::tddd::catalogue_v2::methods::{MethodGenericParam, WherePredicateDecl};
        use domain::tddd::catalogue_v2::roles::FunctionRole;
        use domain::tddd::catalogue_v2::{ParamName, TypeRef};

        let mut doc = make_doc("domain");
        let crate_n = CrateName::new("domain").unwrap();
        let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("bad_qpath_lhs").unwrap());
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![],
            }],
            where_predicates: vec![WherePredicateDecl {
                // Qualified-path LHS: `<T as Iterator>::Item`.
                type_: TypeRef::new("<T as Iterator>::Item").unwrap(),
                bounds: vec![TypeRef::new("Clone").unwrap()],
            }],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.functions.insert(fn_path, entry);

        let result = CatalogueToExtendedCrateCodec::new().encode(doc);
        assert!(
            matches!(result, Err(NewTypeGraphCodecError::InvalidTypeRef(_))),
            "expected InvalidTypeRef for `<T as Trait>::Assoc` LHS, got: {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // ADR 2026-05-13-1153 D1: explicit WherePredicateDecl → where_predicates
    // -----------------------------------------------------------------------

    /// A `FunctionEntry` with an explicit `WherePredicateDecl` (`where T: Clone`)
    /// must emit a `WherePredicate::BoundPredicate` in `Function.generics.where_predicates`
    /// with `type_ = Type::Generic("T")`, and the `GenericParamDef.bounds` for that
    /// parameter must be empty (ADR D1 — all bounds lifted to where form).
    #[test]
    fn test_encode_function_with_explicit_where_predicate_emits_bound_predicate() {
        use domain::tddd::catalogue_v2::FunctionName;
        use domain::tddd::catalogue_v2::entries::FunctionEntry;
        use domain::tddd::catalogue_v2::identifiers::FunctionPath;
        use domain::tddd::catalogue_v2::methods::{MethodGenericParam, WherePredicateDecl};
        use domain::tddd::catalogue_v2::roles::FunctionRole;
        use domain::tddd::catalogue_v2::{ParamName, TypeRef};

        let mut doc = make_doc("domain");
        let crate_n = CrateName::new("domain").unwrap();
        let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("where_fn").unwrap());
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            // generic param `T` with no inline bounds
            generics: vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![],
            }],
            // explicit where predicate: `where T: Clone`
            where_predicates: vec![WherePredicateDecl {
                type_: TypeRef::new("T").unwrap(),
                bounds: vec![TypeRef::new("Clone").unwrap()],
            }],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.functions.insert(fn_path, entry);

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let fn_item = ec
            .krate()
            .index
            .values()
            .find(|item| {
                item.name.as_deref() == Some("where_fn")
                    && matches!(item.inner, ItemEnum::Function(_))
            })
            .expect("expected Function item for where_fn");
        let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

        // One type param `T` with empty inline bounds (all bounds lifted to where form).
        assert_eq!(f.generics.params.len(), 1, "expected 1 generic param");
        let param = &f.generics.params[0];
        assert_eq!(param.name, "T");
        let GenericParamDefKind::Type { bounds, .. } = &param.kind else {
            panic!("expected Type kind for param T");
        };
        assert!(
            bounds.is_empty(),
            "GenericParamDef.bounds must be empty (D1: bounds lifted to where form)"
        );

        // One BoundPredicate for `T: Clone` in where_predicates.
        assert_eq!(
            f.generics.where_predicates.len(),
            1,
            "expected 1 where predicate, got {:?}",
            f.generics.where_predicates
        );
        let WherePredicate::BoundPredicate { type_, bounds, .. } = &f.generics.where_predicates[0]
        else {
            panic!("expected BoundPredicate, got {:?}", f.generics.where_predicates[0]);
        };
        assert!(
            matches!(type_, Type::Generic(g) if g == "T"),
            "BoundPredicate LHS must be Type::Generic(\"T\"), got {type_:?}"
        );
        assert!(!bounds.is_empty(), "BoundPredicate bounds must be non-empty");
    }

    /// A `FunctionEntry` with a non-trivial LHS in a `WherePredicateDecl`
    /// (`where Vec<T>: Clone`) must emit a `WherePredicate::BoundPredicate` whose
    /// `type_` is NOT `Type::Generic` (it is a resolved-path or generic array).
    /// Verifies the non-bare-generic-name branch of `build_where_form_generics`.
    #[test]
    fn test_encode_function_with_non_trivial_lhs_where_predicate_emits_bound_predicate() {
        use domain::tddd::catalogue_v2::FunctionName;
        use domain::tddd::catalogue_v2::entries::FunctionEntry;
        use domain::tddd::catalogue_v2::identifiers::FunctionPath;
        use domain::tddd::catalogue_v2::methods::{MethodGenericParam, WherePredicateDecl};
        use domain::tddd::catalogue_v2::roles::FunctionRole;
        use domain::tddd::catalogue_v2::{ParamName, TypeRef};

        let mut doc = make_doc("domain");
        let crate_n = CrateName::new("domain").unwrap();
        let fn_path = FunctionPath::at_root(crate_n, FunctionName::new("vec_where_fn").unwrap());
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![MethodGenericParam {
                name: ParamName::new("T").unwrap(),
                bounds: vec![],
            }],
            // explicit where predicate: `where Vec<T>: Clone` — non-trivial LHS
            where_predicates: vec![WherePredicateDecl {
                type_: TypeRef::new("Vec<T>").unwrap(),
                bounds: vec![TypeRef::new("Clone").unwrap()],
            }],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        doc.functions.insert(fn_path, entry);

        let ec = CatalogueToExtendedCrateCodec::new().encode(doc).unwrap();
        let fn_item = ec
            .krate()
            .index
            .values()
            .find(|item| {
                item.name.as_deref() == Some("vec_where_fn")
                    && matches!(item.inner, ItemEnum::Function(_))
            })
            .expect("expected Function item for vec_where_fn");
        let ItemEnum::Function(ref f) = fn_item.inner else { panic!("expected Function") };

        // Must have exactly one where predicate (the Vec<T>: Clone entry).
        assert_eq!(
            f.generics.where_predicates.len(),
            1,
            "expected 1 where predicate for `where Vec<T>: Clone`"
        );
        let WherePredicate::BoundPredicate { type_, bounds, .. } = &f.generics.where_predicates[0]
        else {
            panic!("expected BoundPredicate, got {:?}", f.generics.where_predicates[0]);
        };
        // LHS must not be a bare generic; it should be some compound type.
        assert!(
            !matches!(type_, Type::Generic(g) if g == "T"),
            "LHS for `Vec<T>: Clone` must not be Type::Generic(\"T\")"
        );
        assert!(!bounds.is_empty(), "BoundPredicate bounds must be non-empty for Clone");
    }
}
