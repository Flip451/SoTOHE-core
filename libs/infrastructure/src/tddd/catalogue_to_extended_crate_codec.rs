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

        // Encode `inherent_impls` (IN-05 / IN-08, ADR `2026-05-18-1223` D2).
        //
        // `CatalogueDocument::inherent_impls` is a Vec of `InherentImplDeclV2` entries,
        // each representing one inherent `impl` block for a named type with optional
        // impl-block-level generic parameters. Multiple entries for the same type_name
        // represent multiple impl blocks (the Rust source may split methods across impl
        // blocks, each potentially with its own generic constraints).
        //
        // Each `InherentImplDeclV2` entry is encoded as a separate `Impl` item (inherent,
        // `trait_: None`) with the type's pre-assigned `Id` as the `for_` type. The
        // impl-block-level generics are encoded in the maximally-desugared where-form via
        // `build_where_form_generics`, mirroring the treatment of method-level and
        // function-level generics.
        //
        // The new `Impl` items are appended to the existing type's `impls` list. If the
        // type is not found in `local_name_to_id`, encoding fails with `InvalidTypeRef`:
        // inherent impl blocks for external types are not valid Rust, so a missing entry
        // indicates a malformed catalogue (the domain catalogue must always declare the
        // type in the `types` map before referencing it from `inherent_impls`).
        for iid in &doc.inherent_impls {
            let type_name_str = iid.type_name.as_str();
            let type_id = match state.local_name_to_id.get(type_name_str).copied() {
                Some(id) => id,
                None => {
                    // Fail-closed: `InherentImplDeclV2` must always reference a type declared
                    // in this catalogue's `types` map. Inherent impl blocks for external types
                    // are not valid Rust, so a missing entry indicates a malformed catalogue.
                    return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                        type_ref: type_name_str.to_string(),
                        reason: format!(
                            "InherentImplDeclV2 references type '{type_name_str}' which is not \
                             declared in the catalogue's types map"
                        ),
                    });
                }
            };

            // Encode impl-block-level generics in the maximally-desugared where form.
            let impl_generic_names: Vec<&str> =
                iid.impl_generics.iter().map(|g| g.name.as_str()).collect();
            let impl_generics = state.build_where_form_generics(
                &iid.impl_generics,
                &iid.impl_where_predicates,
                &impl_generic_names,
            )?;

            // Encode methods in this impl block (has_body: true — inherent methods).
            let module_path = state
                .index
                .get(&type_id)
                .and_then(|_item| {
                    // Retrieve the module path from the existing paths entry so that method
                    // paths are registered with the correct module prefix.
                    state.paths.get(&type_id).map(|ps| {
                        // Build a ModulePath from the paths entry segments (excluding crate_name and type_name).
                        let segs: Vec<String> = ps
                            .path
                            .iter()
                            .skip(1) // skip crate_name
                            .rev()
                            .skip(1) // skip type_name (last segment)
                            .rev()
                            .cloned()
                            .collect();
                        segs
                    })
                })
                .unwrap_or_default();
            let module_path_domain = if module_path.is_empty() {
                ModulePath::root()
            } else {
                ModulePath::from_segments(module_path).unwrap_or_else(|_| ModulePath::root())
            };
            let method_ids = state.encode_method_items(
                &iid.methods,
                true,
                type_name_str,
                &module_path_domain,
                &impl_generic_names,
            )?;

            let impl_id = state.alloc_id();
            let for_type = resolved_path_type(type_id, type_name_str);
            let impl_inner = Impl {
                is_unsafe: false,
                generics: impl_generics,
                provided_trait_methods: vec![],
                trait_: None, // inherent impl
                for_: for_type,
                items: method_ids,
                is_synthetic: false,
                is_negative: false,
                blanket_impl: None,
            };
            state.index.insert(impl_id, make_item(impl_id, None, None, ItemEnum::Impl(impl_inner)));

            // Append the new impl_id to the type's impls list.
            // Fail-closed: only Struct and Enum have an `impls` field in rustdoc_types.
            // TypeAlias and other kinds cannot bear inherent impl blocks in valid Rust,
            // so a catalogue that declares one indicates a malformed entry.
            if let Some(type_item) = state.index.get_mut(&type_id) {
                match &mut type_item.inner {
                    ItemEnum::Struct(s) => s.impls.push(impl_id),
                    ItemEnum::Enum(e) => e.impls.push(impl_id),
                    _ => {
                        return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                            type_ref: type_name_str.to_string(),
                            reason: format!(
                                "InherentImplDeclV2 targets '{type_name_str}' which is not a \
                                 Struct or Enum — only Struct and Enum can bear inherent impl \
                                 blocks in Rust; TypeAlias and other kinds are not supported"
                            ),
                        });
                    }
                }
            }
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

    /// Checks whether a path string refers to the local crate and, if so, returns it with
    /// the root segment replaced by `"crate"` (e.g. `"domain::Foo<T>"` → `"crate::Foo<T>"`).
    ///
    /// Returns `Some(normalized)` when the root matches `self.crate_name` or is a Rust path
    /// keyword (`crate`, `self`, `super`).  Returns `None` for external-crate paths or bare
    /// names without `::`.
    fn try_normalize_to_local_crate_path(&self, path: &str) -> Option<String> {
        let root_crate = path.split("::").next()?;
        if root_crate.is_empty() || !path.contains("::") {
            return None;
        }
        let is_same_crate = root_crate == self.crate_name.as_str()
            || root_crate == "crate"
            || root_crate == "self"
            || root_crate == "super";
        if is_same_crate {
            let rest = &path[root_crate.len()..]; // includes the leading "::"
            // Normalize the root segment and also replace all occurrences of the explicit
            // crate name inside generic arguments (e.g. `crate::Foo<domain::Bar>` →
            // `crate::Foo<crate::Bar>`).
            let root_normalized = format!("crate{rest}");
            Some(self.replace_crate_prefix_globally(&root_normalized))
        } else {
            None
        }
    }

    /// Replaces occurrences of `{crate_name}::` in `s` with `crate::`, but only when
    /// `{crate_name}` appears as the **first segment** of a path component.
    ///
    /// In Rust path strings, `{crate_name}` is a module sub-segment (not a crate root) exactly
    /// when it is immediately preceded by `::` (e.g. `other::domain::Bar`).  In all other
    /// positions — start of string, after `<`, `(`, `[`, `,`, `=`, `&`, `*`, `+`, ` `, etc. —
    /// it is the root of a new path and should be normalised to `crate`.
    ///
    /// This preserves module paths (e.g. `crate::domain::Foo` is NOT rewritten to
    /// `crate::crate::Foo`) while normalising all crate-root occurrences inside generic
    /// arguments (e.g. `domain::Wrapper<domain::Inner>` → `crate::Wrapper<crate::Inner>`
    /// and `Vec<domain::A,domain::B>` → `Vec<crate::A,crate::B>`).
    fn replace_crate_prefix_globally(&self, s: &str) -> String {
        let cn = self.crate_name.as_str();
        if cn == "crate" || cn == "self" || cn == "super" {
            // Keywords are already valid — nothing to replace.
            return s.to_string();
        }
        let needle = format!("{cn}::");
        if !s.contains(needle.as_str()) {
            return s.to_string();
        }
        // Collect the byte offsets of all `{crate_name}::` occurrences that qualify as
        // crate-root matches (not sub-module segments or partial identifier matches).
        // Use the ORIGINAL string for all lookups to avoid the context-loss problem that
        // arises when iteratively consuming `remaining` after each replacement.
        let mut replace_at: Vec<usize> = Vec::new();
        let s_bytes = s.as_bytes();
        let needle_len = needle.len();
        let mut search_from = 0;
        while let Some(rel) = s[search_from..].find(needle.as_str()) {
            let abs = search_from + rel;
            // Determine whether `{crate_name}` is an independent path root at `abs`.
            // It is NOT a path root when immediately preceded by:
            //   - `:` → sub-module segment (e.g. `other::domain::Foo`)
            //   - alphanumeric or `_` → part of a longer identifier (e.g. `mydomain::Foo`)
            let is_path_root = abs == 0
                || s_bytes
                    .get(abs - 1)
                    .map(|&b| b != b':' && !b.is_ascii_alphanumeric() && b != b'_')
                    .unwrap_or(true);
            if is_path_root {
                replace_at.push(abs);
            }
            search_from = abs + needle_len;
        }
        if replace_at.is_empty() {
            return s.to_string();
        }
        // Build the result by copying segments from the original string, inserting `crate::`
        // at the recorded positions.
        let mut result = String::with_capacity(s.len());
        let mut cursor = 0;
        for pos in replace_at {
            result.push_str(&s[cursor..pos]);
            result.push_str("crate::");
            cursor = pos + needle_len;
        }
        result.push_str(&s[cursor..]);
        result
    }

    /// Encodes a `TraitImplDeclV2.for_` override path string into a `Type`.
    ///
    /// Classification:
    /// - **Same crate**: root segment matches `self.crate_name` or is a Rust path keyword
    ///   (`crate`, `self`, `super`).  The last path-segment ident (without generic args)
    ///   is looked up in `local_name_to_id`; unknown names are rejected with `InvalidTypeRef`.
    ///   Returns `Type::ResolvedPath` with `crate_id = 0` and `args: None`.
    /// - **External crate**: encoded via `parse_type_ref_str` so that generic arguments
    ///   (e.g. `Map<K, V>` in `serde::Map<K, V>`) are correctly placed in `Path.args`
    ///   rather than embedded in the name string.  After encoding, `rewrite_generic_types`
    ///   is applied so that impl-level generics become `Type::Generic` nodes.
    ///
    /// Validation:
    /// - `for_path` must contain at least one `::` (bare names are rejected).
    /// - The root crate segment must not be empty (`::Foo` absolute-path syntax is rejected).
    ///
    /// # Errors
    ///
    /// Returns `InvalidTypeRef` if:
    /// - `for_path` contains no `::` (ambiguous bare name).
    /// - The root segment is empty (absolute-path `::Foo` syntax).
    /// - A same-crate type is not declared in this catalogue document.
    /// - `parse_type_ref_str` fails for an external crate path.
    fn encode_for_path(
        &mut self,
        for_path: &str,
        generic_names: &[&str],
    ) -> Result<Type, CatalogueToExtendedCrateCodecError> {
        // Require at least one `::` so the root crate segment is unambiguous.
        if !for_path.contains("::") {
            return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                type_ref: for_path.to_owned(),
                reason: "TraitImplDeclV2.for_ must be a fully-qualified path \
                         (e.g. `crate_name::TypeName`) — bare single-segment paths are \
                         ambiguous in cross-crate impl context"
                    .to_owned(),
            });
        }
        // Reject absolute-path syntax (`::Foo`) that would produce an empty root crate name.
        let root_crate = for_path.split("::").next().unwrap_or("");
        if root_crate.is_empty() {
            return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                type_ref: for_path.to_owned(),
                reason: "TraitImplDeclV2.for_ may not start with `::` — use an explicit \
                         crate-name prefix (e.g. `crate::Foo` or `my_crate::Foo`) instead"
                    .to_owned(),
            });
        }
        // Rust path keywords (`crate`, `self`, `super`) and the document crate name all
        // denote the local crate.
        let is_same_crate = root_crate == self.crate_name.as_str()
            || root_crate == "crate"
            || root_crate == "self"
            || root_crate == "super";
        if is_same_crate {
            // Same-crate type: normalize the path root to `crate` so `parse_type_ref_str`
            // treats it as a local reference (which looks up `local_name_to_id`) rather than
            // registering a spurious external crate entry.  For example, `"domain::Foo<T>"`
            // becomes `"crate::Foo<T>"` before being passed to the parser.
            // Also replace all nested occurrences of the explicit crate name (e.g.
            // `domain::Wrapper<domain::Inner>` → `crate::Wrapper<crate::Inner>`) so that
            // generic arguments referencing same-crate types are also correctly classified.
            let rest = &for_path[root_crate.len()..]; // includes the leading "::"
            let root_normalized = format!("crate{rest}");
            let normalized = self.replace_crate_prefix_globally(&root_normalized);
            let raw = self.parse_type_ref_str(&normalized)?;
            // Apply `rewrite_generic_types` BEFORE checking for unresolved local types.
            // `parse_type_ref_str` represents generic parameters (e.g. `T` in `Foo<T>`) as
            // `Type::ResolvedPath { path: "T", id: UNRESOLVED_CRATE_ID }` until `rewrite_generic_types`
            // converts them to `Type::Generic("T")`.  Running the unresolved-type check before
            // the rewrite would incorrectly reject valid generic impls such as `impl<T> Trait for crate::Foo<T>`.
            let rewrote = if generic_names.is_empty() {
                raw
            } else {
                rewrite_generic_types(raw, generic_names)
            };
            // After generic rewriting, any remaining `UNRESOLVED_CRATE_ID` node (including
            // nodes nested inside generic arguments) indicates an undeclared same-crate type.
            // Reject to avoid dangling impl targets.
            if let Some(unresolved_path) = find_unresolved_local_path(&rewrote) {
                let short = unresolved_path
                    .rsplit("::")
                    .next()
                    .unwrap_or(unresolved_path)
                    .split('<')
                    .next()
                    .unwrap_or(unresolved_path);
                return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                    type_ref: for_path.to_owned(),
                    reason: format!(
                        "TraitImplDeclV2.for_ references same-crate type `{short}` that \
                         is not declared in this catalogue document — check spelling or \
                         add the type entry before using it as an impl target"
                    ),
                });
            }
            // Normalize the path string to the bare short name so that `for_path_raw`
            // (used as a tiebreaker in `build_impl_identity_map`) matches the spelling
            // that organic C-side rustdoc impls use for local types.  Rustdoc emits
            // `Type::ResolvedPath.path = "Foo"` (last segment only) for local types;
            // emitting `"crate::Foo"` here would make the tiebreaker non-deterministic
            // when an A-sourced impl and an organic local impl share the same key.
            let for_type = match rewrote {
                Type::ResolvedPath(mut p) => {
                    if p.path.contains("::") {
                        if let Some(short) = p.path.rsplit("::").next() {
                            let short = short.split('<').next().unwrap_or(short);
                            p.path = short.to_string();
                        }
                    }
                    Type::ResolvedPath(p)
                }
                other => other,
            };
            Ok(for_type)
        } else {
            // External crate type: use `parse_type_ref_str` so that generic arguments
            // (e.g. `serde::Map<K, V>`) are correctly encoded in `Path.args` rather than
            // embedded in the name string.  Apply `rewrite_generic_types` so that impl-level
            // generics become `Type::Generic` nodes.
            // Also normalize any occurrences of the local crate name inside generic arguments
            // (e.g. `other::Wrapper<domain::Inner>` → `other::Wrapper<crate::Inner>`) so that
            // `parse_type_ref_str` does not register the local crate as an external crate.
            let parse_path = self.replace_crate_prefix_globally(for_path);
            let raw = self.parse_type_ref_str(&parse_path)?;
            let for_type = if generic_names.is_empty() {
                raw
            } else {
                rewrite_generic_types(raw, generic_names)
            };
            Ok(for_type)
        }
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
            // Note: `use<...>` (precise-capture) bounds in the RHS position are
            // accepted by ADR `2026-05-18-1223` D1 — `parse_generic_bound` encodes
            // them as a best-effort `GenericBound::TraitBound` placeholder.
            //
            // For `BoundOp::Bound`, qualified-path LHS forms (`<T as Trait>::Item`) are
            // rejected because `parse_type_ref_str` cannot reconstruct the
            // `Type::QualifiedPath` shape that rustdoc emits for such predicates and
            // degrades them to an unresolved placeholder, producing a silent structural
            // mismatch in the signal evaluator (ADR D3 fail-closed).
            //
            // For `BoundOp::Equal`, qualified-path LHS forms are accepted because
            // `where <T as Trait>::Assoc = U` is valid Rust and the decoder already
            // accepts them.  The LHS is parsed by `encode_qualified_equal_lhs`.
            match w.operator {
                BoundOp::Bound => {
                    // Reject qualified-path LHS forms (`<T as Trait>::Item`) before they
                    // reach `parse_type_ref_str`. `parse_type_ref_str` cannot reconstruct
                    // the `Type::QualifiedPath` shape that rustdoc emits for such predicates
                    // and degrades them to an unresolved placeholder, producing a silent
                    // structural mismatch in the signal evaluator (ADR D3 fail-closed).
                    if lhs_str.trim().starts_with('<') {
                        return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                            type_ref: lhs_str.to_owned(),
                            reason: "qualified-path where-predicate LHS (`<T as Trait>::Assoc`) \
                                     is not supported in `Bound` where-predicates — use simple \
                                     associated-type projection (`T::Assoc`) instead"
                                .to_owned(),
                        });
                    }
                    let lhs_type = if !generic_names.is_empty()
                        && is_bare_generic_name(lhs_str, generic_names)
                    {
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
                    // `Equal` predicates (`where T::Assoc = U`, `where <T as Trait>::Assoc = U`)
                    // encode as `EqPredicate`.
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
                    // associated-type projection (e.g. `T::Assoc`, `<T as Trait>::Assoc`).
                    // A bare type parameter such as `"T"` would produce
                    // `WherePredicate::EqPredicate` for `where T = U`, which is not valid Rust.
                    // Catching this here prevents domain values constructed outside the JSON
                    // codec path from generating nonsensical `ExtendedCrate` representations.
                    if !lhs_str.contains("::") {
                        return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                            type_ref: lhs_str.to_owned(),
                            reason: format!(
                                "Equal where-predicate lhs '{}' is not a supported projection \
                                 form; expected a path with at least one '::' segment \
                                 (e.g. `T::Assoc`, `<T as Trait>::Assoc`) — bare type parameters \
                                 cannot appear as the LHS of a `where T = U` equality constraint",
                                lhs_str
                            ),
                        });
                    }
                    // Build the LHS type for this Equal predicate.
                    //
                    // Unlike `Bound` predicates, `Equal` predicates accept qualified-path LHS
                    // forms (`<T as Trait>::Assoc`) in addition to simple projections
                    // (`T::Assoc`).  The decoder already accepts both forms
                    // (`validate_equal_predicate_lhs` only requires `lhs_str.contains("::")`),
                    // so the encoder must accept them too (decode-success / encode-fail symmetry).
                    let lhs_type = if lhs_str.trim().starts_with('<') {
                        // Qualified-path LHS: `<SelfType as Trait>::Assoc`.
                        // Delegates to `encode_qualified_equal_lhs` which handles all self-type
                        // forms: generic params, `Self`, and concrete paths like `Vec<T>`.
                        self.encode_qualified_equal_lhs(lhs_str, generic_names)?
                    } else if !generic_names.is_empty()
                        && is_bare_generic_name(lhs_str, generic_names)
                    {
                        // Simple bare generic: `T` → `Type::Generic("T")`.
                        // (Already rejected above for Equal if it has no `::`, so this branch
                        // is unreachable for valid Equal LHS, but kept for completeness.)
                        Type::Generic(lhs_str.trim().to_string())
                    } else if let Some(proj) = try_build_generic_projection(lhs_str, generic_names)
                    {
                        // Associated-type projection starting with a generic param or `Self`:
                        // single-level `T::Item`, `Self::Assoc`, or multi-level `T::Nested::Assoc`.
                        // `try_build_generic_projection` returns `Some` for both generic params and
                        // the `Self` keyword, independent of whether `generic_names` is non-empty.
                        proj
                    } else if !generic_names.is_empty() {
                        let raw = self.parse_type_ref_str(lhs_str)?;
                        rewrite_generic_types(raw, generic_names)
                    } else {
                        self.parse_type_ref_str(lhs_str)?
                    };
                    // Safe: len == 1 asserted above.
                    let rhs_entry = w.rhs.first().ok_or_else(|| {
                        CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                            type_ref: lhs_str.to_owned(),
                            reason: "Equal predicate has no rhs (codec invariant violated)"
                                .to_owned(),
                        }
                    })?;
                    let rhs_str = rhs_entry.as_str();
                    // Encode the RHS of the `Equal` predicate.  The decoder accepts any valid
                    // `syn::Type` as the RHS (including qualified-path forms such as
                    // `<U as Trait>::Assoc`), so the encoder must handle the same set to
                    // maintain decode-encode symmetry.
                    let rhs_type = if rhs_str.trim().starts_with('<') {
                        // Qualified-path RHS: `<U as Trait>::Assoc`.
                        // Delegates to `encode_qualified_equal_lhs` which handles all self-type
                        // forms: generic params, `Self`, and concrete paths.
                        self.encode_qualified_equal_lhs(rhs_str, generic_names)?
                    } else if !generic_names.is_empty()
                        && is_bare_generic_name(rhs_str, generic_names)
                    {
                        Type::Generic(rhs_str.trim().to_string())
                    } else if let Some(proj) = try_build_generic_projection(rhs_str, generic_names)
                    {
                        // Projection starting with a generic param or `Self` (e.g. `Self::Assoc`).
                        proj
                    } else if !generic_names.is_empty() {
                        let raw = self.parse_type_ref_str(rhs_str)?;
                        rewrite_generic_types(raw, generic_names)
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
            self.encode_method_items(&methods, true, type_name.as_str(), &module_path, &[])?;

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
            self.encode_method_items(&methods, true, type_name.as_str(), &module_path, &[])?;

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
            self.encode_method_items(&methods, true, type_name.as_str(), &module_path, &[])?;

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
            self.encode_method_items(&methods, true, type_name.as_str(), &module_path, &[])?;

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
            self.encode_method_items(&methods, true, type_name.as_str(), &module_path, &[])?;
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

        // Encode trait-level generics (IN-07, ADR `2026-05-18-1223` D2).
        // `TraitEntry.generics` is a Vec<MethodGenericParam> (reused type); the generic
        // names are collected so that type references inside bounds can be resolved to
        // `Type::Generic` rather than unresolved-marker paths.
        // Uses the maximally-desugared where-form so both inline (`<T: Bound>`) and
        // explicit-where (`<T> where T: Bound`) catalogue declarations produce the same
        // rustdoc-style `Generics` representation that the signal evaluator fingerprints
        // correctly. (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D1)
        let trait_generic_names: Vec<&str> =
            entry.generics.iter().map(|g| g.name.as_str()).collect();
        let trait_generics = self.build_where_form_generics(
            &entry.generics,
            &entry.where_predicates,
            &trait_generic_names,
        )?;

        // Trait methods are declarations, not implementations (has_body: false).
        // Trait-level generic names are passed as outer context so that method signatures
        // that reference trait parameters (e.g. `fn get(&self) -> T` in `trait Foo<T>`)
        // encode `T` as `Type::Generic` rather than an unresolved-marker path.
        let methods: Vec<MethodDeclaration> = entry.methods.clone();
        let method_ids = self.encode_method_items(
            &methods,
            false,
            trait_name.as_str(),
            &module_path,
            &trait_generic_names,
        )?;

        // Encode supertrait bounds as GenericBound::TraitBound entries.
        // Each bound string (e.g. "Send", "Sync", "Into<T>") is parsed via
        // `encode_and_validate_bound` with `trait_generic_names` so that:
        //   - generic args land in `Path.args` (not embedded in `Path.path`)
        //   - trait-level generics in bound args (e.g. `T` in `Into<T>`) are
        //     rewritten to `Type::Generic` rather than an unresolved-marker path.
        let mut bounds: Vec<GenericBound> = Vec::with_capacity(entry.supertrait_bounds.len());
        for b in &entry.supertrait_bounds {
            bounds.push(self.encode_and_validate_bound(b.as_str(), &trait_generic_names)?);
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
                generics: trait_generics,
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
    ///
    /// `outer_generic_names` is a slice of generic parameter names that are in scope
    /// from the enclosing impl block (e.g. `T` in `impl<T> Foo`). These names are
    /// combined with each method's own generic parameter names so that type references
    /// to outer generics in method signatures are encoded as `Type::Generic` rather
    /// than as unresolved path markers.  Pass an empty slice when there are no enclosing
    /// impl-level generics (the common case for `TypeEntry.methods`).
    fn encode_method_items(
        &mut self,
        methods: &[MethodDeclaration],
        force_has_body: bool,
        parent_name: &str,
        parent_module_path: &ModulePath,
        outer_generic_names: &[&str],
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
            // Collect the combined set of generic parameter names in scope for this method:
            // method-level generic params AND any outer impl-block-level generic params.
            //
            // Rustdoc emits `Type::Generic("T")` for generic parameters in function
            // signatures (both bare `T` and composite `Option<T>`, `&T`, etc.).
            // The S-side must match exactly; otherwise Phase 1 reports
            // `UnresolvedTypeRef` and Phase 2 structural comparison mismatches.
            //
            // Strategy:
            // 1. For a bare single-word type string that matches a known generic name
            //    (method-level OR outer impl-level, e.g. `"T"`, `"From"`, `"Display"`),
            //    produce `Type::Generic` directly WITHOUT calling `parse_type_ref_str`.
            //    This avoids the STD_PRELUDE_TYPES expansion that `parse_type_ref_str`
            //    applies to well-known names: e.g. a generic named `"From"` would
            //    otherwise be expanded to the canonical path `"std::convert::From"`,
            //    which `rewrite_generic_types` would then fail to rewrite back (it
            //    checks for single-segment bare paths with no `::` in them).
            // 2. For all other type strings, parse via `parse_type_ref_str` (composite
            //    types, references, generics-in-generics like `Option<T>`) and then call
            //    `rewrite_generic_types` to replace any inner bare generic occurrences.
            let method_generic_names: Vec<&str> =
                method.generics.iter().map(|g| g.name.as_str()).collect();
            // Combine outer (impl-level) and method-level generic names. Outer names come
            // first so that method-level names can shadow them if needed (though shadowing
            // generic names is rare in practice).
            let generic_names: Vec<&str> = outer_generic_names
                .iter()
                .copied()
                .chain(method_generic_names.iter().copied())
                .collect();

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
            //
            // Note: `build_where_form_generics` receives the combined `generic_names`
            // (outer impl/trait generics + method-level generics) so that bound strings
            // referencing outer-scope names — e.g. `U: Into<T>` where `T` is an outer
            // impl/trait parameter — encode `T` as `Type::Generic` rather than an
            // unresolved-marker path. The `GenericParamDef` list produced by the function
            // only includes method-level params (`method.generics`) — outer params are
            // NOT duplicated into `Function.generics`; they appear on the enclosing
            // `Impl.generics` / `Trait.generics`.
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
            // `TraitImplDeclV2.for_` overrides the enclosing type when the impl targets a
            // different type (e.g. `impl MyTrait for ExternalCrate::ExternalType`).
            // When `None`, fall back to the enclosing `type_name` (the common case).
            // Collect impl-level generic parameter names early so they can be passed to
            // `encode_for_path` for correct `Type::Generic` rewriting in the `for_` type.
            // (e.g. `impl<T> MyTrait for crate::Foo<T>` — `T` must become `Type::Generic("T")`)
            let impl_generic_names: Vec<&str> =
                ti.impl_generics.iter().map(|g| g.name.as_str()).collect();

            let for_type = if let Some(ref for_path) = ti.for_ {
                self.encode_for_path(for_path, &impl_generic_names)?
            } else {
                resolved_path_type(type_id, type_name)
            };

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

            // Encode impl-block-level generics (IN-06, ADR `2026-05-18-1223` D2).
            // `impl_generic_names` was computed above (before `for_type`) so it could be passed
            // to `encode_for_path` for generic rewriting.  Reuse it here for `build_where_form_generics`.
            let impl_generics = self.build_where_form_generics(
                &ti.impl_generics,
                &ti.impl_where_predicates,
                &impl_generic_names,
            )?;

            let impl_inner = Impl {
                is_unsafe: false,
                generics: impl_generics,
                provided_trait_methods: vec![],
                trait_: Some(trait_path),
                for_: for_type,
                items: vec![],
                is_synthetic: false,
                is_negative: false,
                blanket_impl: None,
            };
            self.index.insert(impl_id, make_item(impl_id, None, None, ItemEnum::Impl(impl_inner)));
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

    /// Encodes a qualified-path `Equal` predicate LHS of the form `<SelfType as Trait>::Assoc`
    /// where `SelfType` may be any of:
    /// - A known generic parameter name → `Type::Generic(name)`
    /// - The keyword `"Self"` → `Type::ResolvedPath(Path { path: "Self", id: Id(0), .. })`
    /// - A concrete type expression → resolved via `parse_type_ref_str` with generic rewriting
    ///
    /// The trait segment is always resolved via `parse_type_ref_str` so that external traits
    /// (e.g. `std::iter::Iterator`, `serde::Serialize`) obtain a proper synthetic item id rather
    /// than the `UNRESOLVED_CRATE_ID` sentinel.  Storing `UNRESOLVED_CRATE_ID` for a trait that
    /// is not in the local catalogue causes Phase 1 `resolve_type` to raise `UnresolvedTypeRef`
    /// when it detects a bare single-segment path without `::` that is not in `s_known_names`.
    ///
    /// Returns `InvalidTypeRef` if the string does not match the `<...as...>::...` pattern,
    /// if the trait segment is empty, or if `parse_type_ref_str` fails for the self type or trait.
    fn encode_qualified_equal_lhs(
        &mut self,
        lhs_str: &str,
        generic_names: &[&str],
    ) -> Result<Type, CatalogueToExtendedCrateCodecError> {
        let t = lhs_str.trim();
        // Find the `>::` delimiter at the **outermost** nesting level.
        // Simple `find(">::")` would match the first inner delimiter in nested forms such as
        // `<<T as Trait>::Assoc as Another>::Item`, producing an incorrect split.
        // Walk the string tracking angle-bracket depth and stop at the `>` that closes the
        // outermost `<` (i.e., where depth returns to 0 and `::`follows).
        let delim = {
            let mut depth: usize = 0;
            let mut found: Option<usize> = None;
            let bytes = t.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                match bytes.get(i) {
                    Some(b'<') => {
                        depth += 1;
                        i += 1;
                    }
                    Some(b'>') if depth > 0 => {
                        depth -= 1;
                        if depth == 0 {
                            // Check whether `>::` follows.
                            if bytes.get(i + 1..i + 3) == Some(b"::") {
                                found = Some(i);
                                break;
                            }
                        }
                        i += 1;
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            found.ok_or_else(|| CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                type_ref: lhs_str.to_owned(),
                reason: "could not parse qualified-path Equal predicate LHS \
                         (`<SelfType as Trait>::AssocIdent` expected — missing `>::` delimiter \
                         at the outermost nesting level)"
                    .to_owned(),
            })?
        };
        let inner = &t[1..delim];
        // Trim the associated-type name to handle spaces after `>::` (e.g. `>:: Item`).
        let assoc = t[delim + 3..].trim();
        // `assoc` must be a non-empty plain identifier (no angle brackets or `::` separators).
        if assoc.is_empty() || assoc.contains('<') || assoc.contains('>') || assoc.contains("::") {
            return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                type_ref: lhs_str.to_owned(),
                reason: "qualified-path Equal predicate LHS has an invalid associated-type \
                         identifier after `>::`"
                    .to_owned(),
            });
        }
        // Find the `as` keyword at the outermost nesting level of `inner`, accepting any
        // surrounding whitespace (e.g. `T  as  Trait` or `T\tas\tTrait`).
        //
        // `inner.find(" as ")` would fail for multiple spaces or tab-separated forms that
        // `syn::Type` accepts, and would also match inside nested paths
        // (e.g. `<T as Trait>::Assoc as Another` — the first ` as ` is inside the nested `<T as
        // Trait>`).  Instead, walk `inner` with depth tracking and look for the pattern
        // `<whitespace> "as" <whitespace>` at depth 0 where "whitespace" is at least one
        // ASCII space or tab.
        //
        // Returns `(ws_start, kw_end)`:
        //   - `ws_start`: byte index of the first whitespace character before `as`.
        //   - `kw_end`:   byte index immediately after the trailing whitespace (start of the
        //                 trait name segment).
        //
        // `self_str = inner[..ws_start].trim()` and `trait_str = inner[kw_end..].trim()` so
        // that both sides are whitespace-independent.
        let (ws_start, kw_end) = {
            let mut depth: usize = 0;
            let ibytes = inner.as_bytes();
            let mut found: Option<(usize, usize)> = None;
            let mut i = 0;
            while i < ibytes.len() {
                match ibytes.get(i) {
                    Some(b'<') => {
                        depth += 1;
                        i += 1;
                    }
                    Some(b'>') if depth > 0 => {
                        depth -= 1;
                        i += 1;
                    }
                    Some(b' ' | b'\t') if depth == 0 => {
                        // Possible start of `<ws>as<ws>` pattern.
                        let ws_start = i;
                        // Skip all leading whitespace.
                        let mut j = i;
                        while matches!(ibytes.get(j), Some(b' ' | b'\t')) {
                            j += 1;
                        }
                        // Require the token `as` (exactly two bytes).
                        if ibytes.get(j..j + 2) == Some(b"as") {
                            let after_as = j + 2;
                            // Require at least one trailing whitespace char.
                            if matches!(ibytes.get(after_as), Some(b' ' | b'\t')) {
                                // Skip all trailing whitespace.
                                let mut k = after_as;
                                while matches!(ibytes.get(k), Some(b' ' | b'\t')) {
                                    k += 1;
                                }
                                // `k` is now at the first non-whitespace byte after `as`.
                                // Make sure the byte at `j - 1` (the char before the leading
                                // whitespace) is not part of an identifier (prevents matching
                                // `foo_as_bar` at depth 0 which should not be split).
                                // Since we only enter this branch when `ibytes[ws_start]` is
                                // whitespace (not alphanumeric or `_`), this guard is satisfied
                                // by the match condition above.
                                found = Some((ws_start, k));
                                break;
                            }
                        }
                        // Not a valid `<ws>as<ws>` — advance past the leading whitespace we
                        // already scanned so we do not re-examine those bytes.
                        i = j;
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            found.ok_or_else(|| CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                type_ref: lhs_str.to_owned(),
                reason: "qualified-path Equal predicate LHS is missing the `as` keyword \
                         (with surrounding whitespace) between the self type and trait name \
                         at the outermost nesting level"
                    .to_owned(),
            })?
        };
        let self_str = inner[..ws_start].trim();
        let trait_str = inner[kw_end..].trim();
        if trait_str.is_empty() {
            return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                type_ref: lhs_str.to_owned(),
                reason: "qualified-path Equal predicate LHS has an empty trait name".to_owned(),
            });
        }
        // Encode the self type:
        //   (a) known generic param → `Type::Generic(name)` (no parse needed)
        //   (b) `Self` keyword    → `Type::ResolvedPath(Path { path: "Self", id: Id(0), .. })`
        //   (c) nested qualified path (`<T as Trait>::Assoc`) → recursive call to
        //       `encode_qualified_equal_lhs` (since `parse_type_ref_str` cannot reconstruct
        //       the correct `Type::QualifiedPath` shape for `qself` syntax)
        //   (d) concrete path     → `parse_type_ref_str` with same-crate normalization + generic
        //       rewriting.  Same-crate paths (e.g. `domain::Foo`) are normalized to `crate::Foo`
        //       before parsing so that `parse_type_ref_str` looks up `local_name_to_id` rather
        //       than registering a spurious `external_crates` entry for the local crate name.
        let self_type = if generic_names.contains(&self_str) {
            Type::Generic(self_str.to_string())
        } else if self_str == "Self" {
            Type::ResolvedPath(Path { path: "Self".to_string(), id: Id(0), args: None })
        } else if self_str.starts_with('<') {
            // Nested qualified-path self type: `<T as Trait>::Assoc` appearing as the self type
            // of an outer `<<T as Trait>::Assoc as AnotherTrait>::Item` predicate.
            // `parse_type_ref_str` returns an unresolved placeholder for `qself` syntax, so we
            // delegate to `encode_qualified_equal_lhs` which correctly produces `QualifiedPath`.
            self.encode_qualified_equal_lhs(self_str, generic_names)?
        } else {
            let parse_str = self
                .try_normalize_to_local_crate_path(self_str)
                .unwrap_or_else(|| self.replace_crate_prefix_globally(self_str));
            let raw_self = self.parse_type_ref_str(&parse_str)?;
            // Apply generic rewriting BEFORE the unresolved-type check.  `parse_type_ref_str`
            // represents generic parameters as `UNRESOLVED_CRATE_ID` until `rewrite_generic_types`
            // converts them to `Type::Generic`.  Running the check before the rewrite would
            // incorrectly reject valid generic self types such as `<Foo<T> as Trait>::Assoc`.
            let rewritten_self = if generic_names.is_empty() {
                raw_self
            } else {
                rewrite_generic_types(raw_self, generic_names)
            };
            // Reject any self type with an unresolved local-type marker.  After generic
            // rewriting, `UNRESOLVED_CRATE_ID` indicates an undeclared type (bare name or
            // qualified same-crate path not in `local_name_to_id`), including unknown bare
            // names like `"Missing"` which have no `::` and therefore bypass the same-crate
            // detection heuristic.
            if let Some(unresolved_path) = find_unresolved_local_path(&rewritten_self) {
                let short = unresolved_path
                    .rsplit("::")
                    .next()
                    .unwrap_or(unresolved_path)
                    .split('<')
                    .next()
                    .unwrap_or(unresolved_path);
                return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                    type_ref: lhs_str.to_owned(),
                    reason: format!(
                        "qualified Equal predicate LHS self type `{short}` is not \
                         declared in this catalogue document — check spelling or add \
                         the type entry before using it in a qualified path predicate"
                    ),
                });
            }
            rewritten_self
        };
        // Encode the trait segment via `parse_type_ref_str` with same-crate normalization so
        // that predicates like `<domain::Foo as domain::Trait>::Assoc` correctly identify
        // `domain::Trait` as a local (same-crate) trait rather than treating `domain` as an
        // external crate.  Also applies `rewrite_generic_types` so that generic params in the
        // trait's args (e.g. `U` in `<T as Foo<U>>::Assoc`) become `Type::Generic` nodes.
        let trait_parse_str = self
            .try_normalize_to_local_crate_path(trait_str)
            .unwrap_or_else(|| self.replace_crate_prefix_globally(trait_str));
        let trait_type_raw = self.parse_type_ref_str(&trait_parse_str)?;
        let trait_type_rewritten = if generic_names.is_empty() {
            trait_type_raw
        } else {
            rewrite_generic_types(trait_type_raw, generic_names)
        };
        let trait_path = match trait_type_rewritten {
            Type::ResolvedPath(p) => p,
            _ => {
                return Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                    type_ref: lhs_str.to_owned(),
                    reason: format!(
                        "qualified-path Equal predicate LHS trait `{trait_str}` did not parse \
                         as a path type — only named trait paths are supported"
                    ),
                });
            }
        };
        Ok(Type::QualifiedPath {
            name: assoc.to_string(),
            args: None,
            self_type: Box::new(self_type),
            trait_: Some(trait_path),
        })
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
/// Attempts to build a `Type::QualifiedPath` chain from a multi-segment associated-type
/// projection string such as `"T::Item"` or `"T::Nested::Assoc"`.
///
/// The first path segment must be a known generic parameter name.  Each subsequent segment
/// adds a nesting level, producing a chain of `QualifiedPath` nodes (e.g. `T::A::B` becomes
/// `QualifiedPath { name: "B", self_type: QualifiedPath { name: "A", self_type: Generic("T"), .. }, .. }`).
///
/// Returns `None` if:
/// - `type_str` contains no `::` (not a projection).
/// - The first segment is not in `generic_names`.
/// - Any segment contains angle brackets (generic args in projections are not supported).
/// - Any segment is an invalid identifier.
fn try_build_generic_projection(type_str: &str, generic_names: &[&str]) -> Option<Type> {
    let t = type_str.trim();
    // Reject any angle brackets — generic args in projections are not supported.
    if t.contains('<') || t.contains('>') {
        return None;
    }
    // Must have at least one `::`.
    let sep_pos = t.find("::")?;
    let prefix = &t[..sep_pos];
    // Prefix must be either a known generic parameter name OR the `Self` keyword.
    // `Self::Assoc` is a valid projection form in where-predicates.
    let is_self_keyword = prefix == "Self";
    if !is_self_keyword && !generic_names.contains(&prefix) {
        return None;
    }
    // Collect all segments after the prefix.
    let rest_str = &t[sep_pos + 2..];
    let segments: Vec<&str> = rest_str.split("::").collect();
    // Each segment must be a valid identifier (non-empty, starts with letter or `_`).
    for seg in &segments {
        if seg.is_empty() {
            return None;
        }
        let first_char = seg.chars().next()?;
        if !first_char.is_ascii_alphabetic() && first_char != '_' {
            return None;
        }
    }
    // Build a nested `QualifiedPath` chain.
    // For generic params: innermost is `Generic(prefix)`.
    // For `Self`: innermost is `ResolvedPath { path: "Self", id: Id(0), .. }`.
    // For `T::A::B`: QualifiedPath { name: "B", self_type: QualifiedPath { name: "A", self_type: Generic("T") } }
    // For `Self::A::B`: QualifiedPath { name: "B", self_type: QualifiedPath { name: "A", self_type: ResolvedPath("Self") } }
    let mut current = if is_self_keyword {
        Type::ResolvedPath(Path { path: "Self".to_string(), id: Id(0), args: None })
    } else {
        Type::Generic(prefix.to_string())
    };
    for seg in &segments {
        current = Type::QualifiedPath {
            name: seg.to_string(),
            args: None,
            self_type: Box::new(current),
            trait_: None,
        };
    }
    Some(current)
}

/// Walks a `Type` tree recursively and returns the `.path` string of the first
/// `Type::ResolvedPath` node whose `id` is `UNRESOLVED_CRATE_ID`, or `None` if every
/// node is resolved.
///
/// Used by `encode_for_path` to detect undeclared same-crate types nested inside generic
/// arguments (e.g. `crate::Outer<crate::Missing>`) after `parse_type_ref_str`.  Since the
/// same-crate branch normalises all local references to `crate::` before parsing, any
/// remaining `UNRESOLVED_CRATE_ID` node indicates an undeclared type that should be rejected
/// at the codec boundary rather than propagated to Phase 1.
fn find_unresolved_local_path(ty: &Type) -> Option<&str> {
    match ty {
        Type::ResolvedPath(p) => {
            if p.id == Id(UNRESOLVED_CRATE_ID) {
                return Some(p.path.as_str());
            }
            if let Some(args) = &p.args {
                return find_unresolved_in_generic_args(args);
            }
            None
        }
        Type::BorrowedRef { type_: inner, .. } | Type::RawPointer { type_: inner, .. } => {
            find_unresolved_local_path(inner)
        }
        Type::Slice(inner) | Type::Array { type_: inner, .. } => find_unresolved_local_path(inner),
        Type::Tuple(tys) => tys.iter().find_map(find_unresolved_local_path),
        Type::QualifiedPath { self_type, trait_, .. } => find_unresolved_local_path(self_type)
            .or_else(|| {
                trait_.as_ref().and_then(|tp| {
                    if tp.id == Id(UNRESOLVED_CRATE_ID) {
                        Some(tp.path.as_str())
                    } else {
                        tp.args.as_deref().and_then(find_unresolved_in_generic_args)
                    }
                })
            }),
        // Generic, Primitive, Infer, Never, Pat, DynTrait, ImplTrait, FunctionPointer:
        // none of these embed a ResolvedPath with an id.
        _ => None,
    }
}

fn find_unresolved_in_generic_args(args: &GenericArgs) -> Option<&str> {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => args
            .iter()
            .find_map(|arg| match arg {
                GenericArg::Type(ty) => find_unresolved_local_path(ty),
                _ => None,
            })
            .or_else(|| {
                constraints.iter().find_map(|c| {
                    c.args.as_deref().and_then(find_unresolved_in_generic_args).or_else(|| match &c
                        .binding
                    {
                        AssocItemConstraintKind::Equality(Term::Type(ty)) => {
                            find_unresolved_local_path(ty)
                        }
                        _ => None,
                    })
                })
            }),
        GenericArgs::Parenthesized { inputs, output } => inputs
            .iter()
            .find_map(find_unresolved_local_path)
            .or_else(|| output.as_ref().and_then(find_unresolved_local_path)),
        // `ReturnTypeNotation` (`(..)`) contains no nested types.
        GenericArgs::ReturnTypeNotation => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::expect_used)]
#[path = "catalogue_to_extended_crate_codec_tests.rs"]
mod tests;
