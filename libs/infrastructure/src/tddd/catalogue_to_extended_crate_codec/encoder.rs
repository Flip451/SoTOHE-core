//! `Encoder` and `EncoderState` struct definitions, plus `Encoder` impl
//! (pre-passes and the main encoding pipeline `run()`).

use std::collections::{BTreeMap, HashMap};

use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, ModulePath, StructShape, TypeKindV2,
};
use domain::tddd::extended_crate::ExtendedCrate;
use rustdoc_types::{
    Crate, ExternalCrate, FORMAT_VERSION, Id, Impl, Item, ItemEnum, ItemSummary, Module, Target,
};

use crate::tddd::catalogue_to_extended_crate_codec_error::CatalogueToExtendedCrateCodecError;

use super::helpers::{make_item, normalize_impl_for_type_path, resolved_path_type};

// ---------------------------------------------------------------------------
// Encoder — internal per-call state
// ---------------------------------------------------------------------------

/// Pre-pass state that holds the `CatalogueDocument` alongside encoding state.
///
/// After pre-passes complete, `Encoder` is consumed and destructured into
/// a `CatalogueDocument` + `EncoderState` so that encoding loops can borrow
/// the document immutably while mutating the state.
pub(super) struct Encoder {
    pub(super) doc: CatalogueDocument,
    pub(super) state: EncoderState,
}

/// Mutable encoding state used during the main encoding loop.
///
/// Separated from `Encoder` so that encoding methods can hold a mutable borrow
/// on `EncoderState` while the caller holds an immutable borrow on the
/// `CatalogueDocument`.
pub(super) struct EncoderState {
    /// Incremental Id counter (Id(0) = root module).
    pub(super) next_id: u32,
    /// Item index.
    pub(super) index: HashMap<Id, Item>,
    /// Paths map for `Crate::paths`.
    pub(super) paths: HashMap<Id, ItemSummary>,
    /// `crate_id` → `ExternalCrate` for `Crate::external_crates`.
    pub(super) external_crates: HashMap<u32, ExternalCrate>,
    /// `crate_name` → `crate_id` lookup for TypeRef resolution.
    pub(super) ext_name_to_id: HashMap<String, u32>,
    /// Next external crate_id to assign (0 is reserved for self).
    pub(super) next_ext_id: u32,
    /// Catalogue-declared type/trait short name → assigned `Id` for TypeRef resolution.
    ///
    /// Only types and traits are stored here. Function path strings are kept
    /// in `fn_path_to_id` to prevent function ids from polluting TypeRef
    /// resolution (which operates on single-segment names only).
    pub(super) local_name_to_id: HashMap<String, Id>,
    /// Full `FunctionPath` string → assigned `Id`.
    ///
    /// Kept separate from `local_name_to_id` so that function paths (which are
    /// multi-segment strings such as `"my_crate::fn_name"`) are never
    /// accidentally matched by the single-segment TypeRef resolver, and so that
    /// `AmbiguousIdentifier` checks are applied independently to the two
    /// namespaces (types/traits vs. functions).
    pub(super) fn_path_to_id: HashMap<String, Id>,
    /// Crate name used for `ItemSummary::path` construction.
    pub(super) crate_name: CrateName,
    /// Cache: full canonical path string → synthetic `Id` for external type references.
    ///
    /// Ensures that the same external type (e.g. `std::vec::Vec`) always gets the same
    /// synthetic item id within a single codec run, so `Crate::paths` entries are not
    /// duplicated and downstream consumers can reliably look up external types.
    pub(super) external_type_path_to_id: HashMap<String, Id>,
}

impl Encoder {
    pub(super) fn new(doc: CatalogueDocument) -> Self {
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

    /// Pre-pass: register external crates from top-level `trait_impls` (ADR `2026-05-20-0048` D1/D2).
    ///
    /// Both `trait_ref` and `for_type` may contain crate-prefixed type references.
    /// Extracts the first path segment (the crate name) from each string using
    /// AST-aware extraction: the `::` is only searched in the prefix before the
    /// first `<` so that generic arguments like `"Foo<serde::Serialize>"` do not
    /// produce a spurious `"Foo<serde"` crate registration.
    ///
    /// Rust path-keyword segments (`crate`, `self`, `super`) and the self-crate name
    /// are skipped — they are not real external crates.
    fn collect_external_from_trait_impls(&mut self) {
        let self_crate_name = self.doc.crate_name.as_str().to_string();
        // Reserved Rust path keywords that must not be registered as external crates.
        const PATH_KEYWORDS: &[&str] = &["crate", "self", "super"];

        // Returns the crate-name prefix of `type_str` if it looks like an
        // external-crate path (`first_seg::rest`), excluding Rust path keywords
        // and the self-crate name.
        let extract_crate = |type_str: &str| -> Option<String> {
            // Truncate at the first `<` to avoid matching `::` inside generic args.
            let angle_pos = type_str.find('<').unwrap_or(type_str.len());
            let base = &type_str[..angle_pos];
            let colon_pos = base.find("::")?;
            let first_seg = base[..colon_pos].trim();
            // Reject empty first segment (e.g. absolute paths starting with `::`)
            // and Rust path keywords / self-crate names.
            if first_seg.is_empty()
                || first_seg == self_crate_name.as_str()
                || PATH_KEYWORDS.contains(&first_seg)
            {
                return None;
            }
            Some(first_seg.to_string())
        };

        let mut crate_names: Vec<String> = Vec::new();
        for ti in &self.doc.trait_impls {
            // Extract crate prefix from trait_ref (e.g. "core" from "core::convert::From<X>").
            if let Some(cn) = extract_crate(ti.trait_ref.as_str()) {
                crate_names.push(cn);
            }
            // Extract crate prefix from for_type (e.g. "std" from "std::vec::Vec<i32>").
            if let Some(cn) = extract_crate(ti.for_type.as_str()) {
                crate_names.push(cn);
            }
        }
        for cn in crate_names {
            self.state.ensure_external_crate(cn);
        }
    }

    /// Runs the full encoding pipeline.
    pub(super) fn run(mut self) -> Result<ExtendedCrate, CatalogueToExtendedCrateCodecError> {
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
                TypeKindV2::Struct(struct_kind) => {
                    let domain::tddd::catalogue_v2::StructKind { shape, typestate } = struct_kind;
                    match shape {
                        StructShape::Unit => {
                            state.encode_unit_struct(type_id, type_name, entry)?;
                        }
                        StructShape::Tuple { fields, has_stripped_fields } => {
                            state.encode_tuple_struct(
                                type_id,
                                type_name,
                                entry,
                                fields,
                                has_stripped_fields,
                            )?;
                        }
                        StructShape::Plain { fields, has_stripped_fields } => {
                            state.encode_plain_struct(
                                type_id,
                                type_name,
                                entry,
                                fields,
                                has_stripped_fields,
                                typestate,
                            )?;
                        }
                    }
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

        // Encode top-level `trait_impls` (ADR `2026-05-20-0048` D1 / D2 / D4).
        //
        // `CatalogueDocument::trait_impls` is a Vec of `TraitImplDeclV2` entries, each
        // representing one `impl Trait for Type` block as a STANDALONE catalogue-level
        // independent entry — symmetric with `CatalogueDocument::inherent_impls` (D2).
        //
        // Per ADR D1, impl blocks are peers of TypeEntry/TraitEntry with NO "parent type"
        // relationship. Self-crate types and external-crate types are handled symmetrically:
        //
        // - `trait_ref`: parsed via syn; self-crate traits resolved via `local_name_to_id`,
        //   external traits via `ext_name_to_id` / `ensure_external_type_id`.
        //
        // - `for_type`: parsed via `parse_type_ref_str` (which includes Pass 3
        //   `resolve_external_type_ids` internally). Per ADR `2026-05-20-0048` D2, a
        //   self-crate type's `for_type` is its bare short name (e.g. `"SelfType"`),
        //   so `parse_type_ref` resolves it via `resolve_local` to the pre-assigned
        //   local id. An external type uses a fully-qualified crate-prefixed path
        //   (e.g. `"std::vec::Vec<i32>"`), resolved to a synthetic external item id.
        //
        // IMPORTANT (ADR D4): The resulting `Impl` items are NOT attached to any type's
        // `Struct.impls` / `Enum.impls` list. Each `Impl` item is a STANDALONE entry in
        // the index with its own `item_actions` entry. The unified A-side impl insertion
        // loop in Phase 1 (builder.rs) inserts these impls into S directly via their own
        // action, with no parent-type traversal needed.
        for ti in &doc.trait_impls {
            let impl_id = state.alloc_id();
            // Read action from the TraitImplDeclV2 entry (CN-04: ItemAction::Add must NOT be
            // hardcoded — the codec must use entry.action to mirror TypeEntry/TraitEntry handling).
            let action = ti.action;

            // Build the rustdoc `Type` for the impl's `for_` field.
            //
            // `parse_type_ref_str` handles both cases under ADR D2:
            // - Bare self-crate name (e.g. `"SelfType"`) → single segment → `resolve_local`
            //   → pre-assigned local id, path `"SelfType"`.
            // - Fully-qualified external path (e.g. `"std::vec::Vec<i32>"`) → multi-segment
            //   with non-keyword first segment → external, synthetic id via Pass 3
            //   `resolve_external_type_ids`.
            // The redundant manual workaround (normalize_local_short_name / is_possibly_local_path
            // / match override) is removed: parse_type_ref_str alone produces the correct Type.
            //
            // After parsing, normalize the `for_` path to the last segment (short name) so
            // that A-origin `for_path_raw` (the secondary tiebreaker in `build_impl_identity_map`)
            // matches the form rustdoc emits for `impl.for_` (e.g. `"Vec"` not
            // `"std::vec::Vec"`).  This normalization applies ONLY to the `for_` type —
            // NOT to the trait path (which needs its fully-qualified form for identity-key
            // disambiguation in `build_impl_identity_map`).
            let for_type_resolved =
                normalize_impl_for_type_path(state.parse_type_ref_str(ti.for_type.as_str())?);

            // Resolve trait_ref: parse and resolve via parse_type_ref_str so that
            // nested type references in generic args are fully resolved.
            let trait_path = state.resolve_trait_ref_for_top_level(ti.trait_ref.as_str())?;

            // Encode impl-block-level generics.
            let impl_generic_names: Vec<&str> =
                ti.impl_generics.iter().map(|g| g.name.as_str()).collect();
            let impl_generics = state.build_where_form_generics(
                &ti.impl_generics,
                &ti.impl_where_predicates,
                &impl_generic_names,
            )?;
            let impl_inner = Impl {
                is_unsafe: false,
                generics: impl_generics,
                provided_trait_methods: vec![],
                trait_: Some(trait_path),
                for_: for_type_resolved,
                items: vec![],
                is_synthetic: false,
                is_negative: false,
                blanket_impl: None,
            };
            // Insert as a STANDALONE index entry. Per ADR D1/D4, the impl is NOT attached
            // to any type's `Struct.impls` / `Enum.impls` — it is a top-level independent
            // entry discovered by the unified A-side impl insertion loop in Phase 1.
            state.index.insert(impl_id, make_item(impl_id, None, None, ItemEnum::Impl(impl_inner)));
            item_actions.insert(impl_id, action);
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
