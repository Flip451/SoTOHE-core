//! `Encoder` and `EncoderState` struct definitions, plus `Encoder` impl
//! (pre-passes and the main encoding pipeline `run()`).

use std::collections::{BTreeMap, HashMap};

use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::identifiers::{CatalogueItemNamespace, FullyQualifiedItemPath};
use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CatalogueEntryKey, CrateName, DeletionRecord, ModulePath, StructShape,
    TypeKindV2, TypeRef,
};
use domain::tddd::extended_crate::ExtendedCrate;
use rustdoc_types::{
    Crate, ExternalCrate, FORMAT_VERSION, Id, Impl, Item, ItemEnum, ItemKind, ItemSummary, Module,
    Target,
};

use super::encoder_deletions::encode_deletion_record;
use super::helpers::{make_item, normalize_impl_for_type_path, resolved_path_type};
use super::{entry_item_name, invalid_type_ref, map_identity_resolution_error, summary_identity};
use crate::tddd::canonical_type_identity::{
    CanonicalTypeIdentity, canonicalize_catalogue_type_ref,
};
use domain::tddd::catalogue_v2::identity_resolution::resolve_catalogue_identity_in_namespace;

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
    /// Authoritative baseline/current rustdoc paths used for reconciliation.
    pub(super) resolution_paths: HashMap<Id, ItemSummary>,
    /// Effective placement selected for catalogue declarations during the
    /// action-aware pre-pass.
    pub(super) resolved_entry_module_paths: HashMap<String, ModulePath>,
    /// `crate_id` → `ExternalCrate` for `Crate::external_crates`.
    pub(super) external_crates: HashMap<u32, ExternalCrate>,
    /// `crate_name` → `crate_id` lookup for TypeRef resolution.
    pub(super) ext_name_to_id: HashMap<String, u32>,
    /// Next external crate_id to assign (0 is reserved for self).
    pub(super) next_ext_id: u32,
    /// Canonical implementation identity → all assigned local item ids.
    ///
    /// Multiple catalogue declarations may temporarily share one rustdoc identity. The
    /// evaluator owns the eventual collision decision; codec resolution remains fail-closed
    /// by rejecting an identity with more than one local declaration when it is referenced.
    pub(super) local_identity_to_id:
        HashMap<CanonicalTypeIdentity, Vec<(FullyQualifiedItemPath, Id)>>,
    /// Full `FunctionPath` string → assigned `Id`.
    ///
    /// Kept separate from the canonical type/trait identity index because
    /// function paths are a distinct namespace from TypeRef identities.
    pub(super) fn_path_to_id: HashMap<String, Id>,
    /// Crate name used for `ItemSummary::path` construction.
    pub(super) crate_name: CrateName,
    /// Cache: full canonical path string → synthetic `Id` for external type references.
    ///
    /// Ensures that the same external type (e.g. `std::vec::Vec`) always gets the same
    /// synthetic item id within a single codec run, so `Crate::paths` entries are not
    /// duplicated and downstream consumers can reliably look up external types.
    pub(super) external_type_path_to_id: HashMap<String, Id>,
    /// First fail-closed local-resolution error observed while walking a parsed type.
    pub(super) resolution_error: Option<NewTypeGraphCodecError>,
    /// Namespace to use for the root path of the next parsed expression.
    ///
    /// Ordinary type expressions default to `Type`; the top-level trait-ref
    /// route sets this for one parse so its root path is resolved as a trait
    /// while nested generic arguments retain type resolution.
    pub(super) pending_root_namespace: Option<CatalogueItemNamespace>,
}

impl Encoder {
    pub(super) fn new(doc: CatalogueDocument, resolution_paths: HashMap<Id, ItemSummary>) -> Self {
        let crate_name = doc.crate_name().clone();
        Self {
            doc,
            state: EncoderState {
                next_id: 0,
                index: HashMap::new(),
                paths: HashMap::new(),
                resolution_paths,
                resolved_entry_module_paths: HashMap::new(),
                external_crates: HashMap::new(),
                ext_name_to_id: HashMap::new(),
                next_ext_id: 1,
                local_identity_to_id: HashMap::new(),
                fn_path_to_id: HashMap::new(),
                crate_name,
                external_type_path_to_id: HashMap::new(),
                resolution_error: None,
                pending_root_namespace: None,
            },
        }
    }

    /// Pre-pass: assign `Id`s to all declared types, traits, and functions.
    ///
    /// Assigns ids without collapsing declarations that share a short name.
    ///
    /// The short-name index is populated only after every declaration has been indexed. This
    /// lets two modules declare the same item name while preserving a fail-closed ambiguity
    /// result when a TypeRef uses that short name without enough context.
    fn assign_ids(&mut self) -> Result<(), NewTypeGraphCodecError> {
        // Id(0) = root module.
        let _ = self.state.alloc_id();

        // Collect declarations first to avoid simultaneous document/state borrows.
        let type_entries: Vec<(CatalogueEntryKey, Option<ModulePath>)> = self
            .doc
            .types()
            .iter()
            .map(|(key, entry)| (key.clone(), entry.module_path().cloned()))
            .collect();
        for (key, module_path) in type_entries {
            self.assign_local_entry(&key, module_path.as_ref(), CatalogueItemNamespace::Type)?;
        }
        let trait_entries: Vec<(CatalogueEntryKey, Option<ModulePath>)> = self
            .doc
            .traits()
            .iter()
            .map(|(key, entry)| (key.clone(), entry.module_path().cloned()))
            .collect();
        for (key, module_path) in trait_entries {
            self.assign_local_entry(&key, module_path.as_ref(), CatalogueItemNamespace::Trait)?;
        }
        // Functions are keyed by their full `FunctionPath` string (e.g. `"my_crate::fn"`).
        // Store them in the dedicated `fn_path_to_id` map; TypeRef identity resolution only
        // consults the canonical type/trait identity index.
        let fn_paths: Vec<String> = self.doc.functions().keys().map(|k| k.to_string()).collect();
        for path in fn_paths {
            self.assign_function_id(&path)?;
        }

        // Deletion records still participate in TypeGraph A as
        // Delete-marked top-level items. Phase 1 uses only their identity to
        // move the matching B-side item into the delete set.
        let deletion_local_names: Vec<(String, CatalogueItemNamespace)> = self
            .doc
            .deletions()
            .iter()
            .filter_map(|record| match record {
                DeletionRecord::Type { name, .. } => {
                    Some((name.as_str().to_owned(), CatalogueItemNamespace::Type))
                }
                DeletionRecord::Trait { name, .. } => {
                    Some((name.as_str().to_owned(), CatalogueItemNamespace::Trait))
                }
                DeletionRecord::Function { .. } => None,
            })
            .collect();
        for (name, namespace) in deletion_local_names {
            let key = CatalogueEntryKey::try_new(name).map_err(|_| {
                invalid_type_ref("<delete>", "delete tombstone contains an empty catalogue key")
            })?;
            self.assign_deletion_entry(&key, namespace)?;
        }

        let deletion_fn_paths: Vec<String> = self
            .doc
            .deletions()
            .iter()
            .filter_map(|record| match record {
                DeletionRecord::Function { path, .. } => Some(path.to_string()),
                DeletionRecord::Type { .. } | DeletionRecord::Trait { .. } => None,
            })
            .collect();
        for path in deletion_fn_paths {
            self.assign_function_id(&path)?;
        }
        Ok(())
    }

    fn assign_local_entry(
        &mut self,
        key: &CatalogueEntryKey,
        declared_module_path: Option<&ModulePath>,
        namespace: CatalogueItemNamespace,
    ) -> Result<(), NewTypeGraphCodecError> {
        let (fully_qualified, module_path) =
            self.identity_for_key(key, declared_module_path, namespace)?;
        let id = self.state.alloc_id();
        let item_kind = match namespace {
            CatalogueItemNamespace::Type => ItemKind::Struct,
            CatalogueItemNamespace::Trait => ItemKind::Trait,
        };
        self.state.register_identity_path(id, item_kind, &fully_qualified);
        self.state
            .resolved_entry_module_paths
            .insert(entry_scope_key(key, namespace), module_path.clone());
        let identity_ref = TypeRef::new(if fully_qualified.is_placed() {
            fully_qualified.to_string()
        } else {
            key.as_str().to_owned()
        })
        .map_err(|_| {
            invalid_type_ref(key.as_str(), "catalogue key produced an invalid identity")
        })?;
        let namespace_paths = self.state.resolution_paths_for_namespace(namespace);
        let identity = canonicalize_catalogue_type_ref(
            &identity_ref,
            &self.state.crate_name,
            &namespace_paths,
            &[],
        )?;
        self.state.local_identity_to_id.entry(identity).or_default().push((fully_qualified, id));
        Ok(())
    }

    fn assign_deletion_entry(
        &mut self,
        key: &CatalogueEntryKey,
        namespace: CatalogueItemNamespace,
    ) -> Result<(), NewTypeGraphCodecError> {
        let identity = self.state.resolve_catalogue_key_identity(key, namespace)?;
        let identity_ref = TypeRef::new(identity.to_string()).map_err(|_| {
            invalid_type_ref(key.as_str(), "delete catalogue key is not valid TypeRef notation")
        })?;
        let namespace_paths = self.state.resolution_paths_for_namespace(namespace);
        let canonical = canonicalize_catalogue_type_ref(
            &identity_ref,
            &self.state.crate_name,
            &namespace_paths,
            &[],
        )?;
        if let Some(id) = self.state.local_id_for_identity_in_namespace(&canonical, namespace)? {
            return Err(invalid_type_ref(
                key.as_str(),
                format!(
                    "delete catalogue identity '{}' resolves to existing item id {}; refusing to overwrite the existing item",
                    key.as_str(),
                    id.0
                ),
            ));
        }
        let module_path = identity.module_path().cloned().ok_or_else(|| {
            invalid_type_ref(
                key.as_str(),
                "delete catalogue identity has no authoritative module placement",
            )
        })?;
        let effective_key = CatalogueEntryKey::try_new(identity.to_string()).map_err(|_| {
            invalid_type_ref(key.as_str(), "delete catalogue identity is not a valid catalogue key")
        })?;
        self.assign_local_entry(&effective_key, Some(&module_path), namespace)
    }

    fn identity_for_key(
        &self,
        key: &CatalogueEntryKey,
        declared_module_path: Option<&ModulePath>,
        namespace: CatalogueItemNamespace,
    ) -> Result<(FullyQualifiedItemPath, ModulePath), NewTypeGraphCodecError> {
        let identity = match namespace {
            CatalogueItemNamespace::Type => FullyQualifiedItemPath::from_type_catalogue_entry_key(
                &self.state.crate_name,
                key,
                declared_module_path,
            ),
            CatalogueItemNamespace::Trait => {
                FullyQualifiedItemPath::from_trait_catalogue_entry_key(
                    &self.state.crate_name,
                    key,
                    declared_module_path,
                )
            }
        }
        .map_err(|error| {
            invalid_type_ref(key.as_str(), format!("invalid catalogue identity: {error}"))
        })?;
        let identity = if identity.is_placed() {
            identity
        } else {
            let universe = self
                .state
                .resolution_paths
                .values()
                .filter_map(summary_identity)
                .collect::<std::collections::BTreeSet<_>>();
            let reference = TypeRef::new(key.as_str().to_owned())
                .map_err(|_| invalid_type_ref(key.as_str(), "catalogue key has no item name"))?;
            match resolve_catalogue_identity_in_namespace(
                &reference,
                &self.state.crate_name,
                &universe,
                Some(namespace),
            ) {
                Ok(resolved) => resolved,
                Err(error) => return Err(map_identity_resolution_error(error)),
            }
        };
        let module_path = identity.module_path().cloned().unwrap_or_else(ModulePath::root);
        Ok((identity, module_path))
    }

    fn assign_function_id(&mut self, path: &str) -> Result<(), NewTypeGraphCodecError> {
        if self.state.fn_path_to_id.contains_key(path) {
            return Err(invalid_type_ref(path, "duplicate function path in catalogue"));
        }
        let id = self.state.alloc_id();
        self.state.fn_path_to_id.insert(path.to_owned(), id);
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
        let self_crate_name = self.doc.crate_name().as_str().to_string();
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
        for ti in self.doc.trait_impls() {
            // Extract crate prefix from trait_ref (e.g. "core" from "core::convert::From<X>").
            if let Some(cn) = extract_crate(ti.trait_ref().as_str()) {
                crate_names.push(cn);
            }
            // Extract crate prefix from for_type (e.g. "std" from "std::vec::Vec<i32>").
            if let Some(cn) = extract_crate(ti.for_type().as_str()) {
                crate_names.push(cn);
            }
        }
        for cn in crate_names {
            self.state.ensure_external_crate(cn);
        }
    }

    /// Runs the full encoding pipeline.
    pub(super) fn run(mut self) -> Result<ExtendedCrate, NewTypeGraphCodecError> {
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
        for (type_name, entry) in doc.types() {
            let type_id = state.local_id_for_catalogue_key(
                type_name,
                entry.module_path(),
                CatalogueItemNamespace::Type,
            )?;
            let action = entry.action();
            match entry.kind().clone() {
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
                TypeKindV2::TypeAlias { target, generics } => {
                    state.encode_type_alias(type_id, type_name, entry, target, &generics)?;
                }
            }
            item_actions.insert(type_id, action);
        }

        // Encode traits.
        for (trait_name, entry) in doc.traits() {
            let trait_id = state.local_id_for_catalogue_key(
                trait_name,
                entry.module_path(),
                CatalogueItemNamespace::Trait,
            )?;
            let action = entry.action();
            state.encode_trait(trait_id, trait_name, entry)?;
            item_actions.insert(trait_id, action);
        }

        // Encode functions.
        for (fn_path, entry) in doc.functions() {
            let fn_id =
                state.fn_path_to_id.get(&fn_path.to_string()).copied().ok_or_else(|| {
                    invalid_type_ref(fn_path.to_string(), "pre-pass id not found (internal error)")
                })?;
            let action = entry.action();
            state.encode_function(fn_id, fn_path, entry)?;
            item_actions.insert(fn_id, action);
        }

        for record in doc.deletions() {
            encode_deletion_record(&mut state, &mut item_actions, record)?;
        }

        // Encode top-level trait implementations as standalone items.
        for ti in doc.trait_impls() {
            let impl_id = state.alloc_id();
            // Read action from the TraitImplDeclV2 entry (CN-04: ItemAction::Add must NOT be
            // hardcoded — the codec must use entry.action to mirror TypeEntry/TraitEntry handling).
            let action = ti.action();

            // Collect impl-block generic parameter names (e.g. `["T", "U"]`) before
            // parsing `for_type`, so that bare generic names in `for_type` (e.g. `"T"` in
            // `impl<T> Trait for T`) are encoded as `Type::Generic("T")` rather than as
            // unresolved-marker `ResolvedPath` nodes (ADR 2026-06-18-0822 D2).
            let impl_generic_names: Vec<&str> =
                ti.impl_generics().iter().map(|g| g.name.as_str()).collect();

            // Build the rustdoc `Type` for the impl's `for_` field.
            //
            // `parse_type_ref_str_with_generics` handles all cases:
            // - Bare generic name (e.g. `"T"` when impl_generics contains `"T"`) →
            //   `Type::Generic("T")` (ADR 2026-06-18-0822 D2).
            // - Bare self-crate name (e.g. `"SelfType"`) → single segment → `resolve_local`
            //   → pre-assigned local id, path `"SelfType"`.
            // - Fully-qualified external path (e.g. `"std::vec::Vec<i32>"`) → multi-segment
            //   with non-keyword first segment → external, synthetic id via Pass 3
            //   `resolve_external_type_ids`.
            //
            // After parsing, normalize the `for_` path to the last segment (short name) so
            // that A-origin `for_path_raw` (the secondary tiebreaker in `build_impl_identity_map`)
            // matches the form rustdoc emits for `impl.for_` (e.g. `"Vec"` not
            // `"std::vec::Vec"`).  This normalization applies ONLY to the `for_` type —
            // NOT to the trait path (which needs its fully-qualified form for identity-key
            // disambiguation in `build_impl_identity_map`).
            let for_type_resolved =
                normalize_impl_for_type_path(state.parse_type_ref_str_with_generics(
                    ti.for_type().as_str(),
                    &impl_generic_names,
                )?);

            // Resolve trait_ref: parse and resolve via parse_type_ref_str so that
            // nested type references in generic args are fully resolved.
            let trait_path = state.resolve_trait_ref_for_top_level_in_trait_namespace(
                ti.trait_ref().as_str(),
                &impl_generic_names,
            )?;

            // Encode impl-block-level generics.
            let impl_generics = state.build_where_form_generics(
                ti.impl_generics(),
                ti.impl_where_predicates(),
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

        // Encode each inherent implementation as a separate item and attach it to its owner.
        for iid in doc.inherent_impls() {
            let type_name_key = iid.type_name().as_str();
            let type_name_str = entry_item_name(iid.type_name());
            let type_id =
                match state.local_id_for_path(type_name_key, CatalogueItemNamespace::Type)? {
                    Some(id) => id,
                    None => {
                        // Fail-closed: `InherentImplDeclV2` must always reference a type declared
                        // in this catalogue's `types` map. Inherent impl blocks for external types
                        // are not valid Rust, so a missing entry indicates a malformed catalogue.
                        return Err(invalid_type_ref(
                            type_name_key,
                            format!(
                                "InherentImplDeclV2 references type '{type_name_key}' which is not \
                             declared in the catalogue's types map"
                            ),
                        ));
                    }
                };

            // Encode impl-block-level generics in the maximally-desugared where form.
            let impl_generic_names: Vec<&str> =
                iid.impl_generics().iter().map(|g| g.name.as_str()).collect();
            let impl_generics = state.build_where_form_generics(
                iid.impl_generics(),
                iid.impl_where_predicates(),
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
                iid.methods(),
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
                        return Err(invalid_type_ref(
                            type_name_key,
                            format!(
                                "InherentImplDeclV2 targets '{type_name_key}' which is not a \
                                 Struct or Enum — only Struct and Enum can bear inherent impl \
                                 blocks in Rust; TypeAlias and other kinds are not supported"
                            ),
                        ));
                    }
                }
            }
        }

        // Root module children are all pre-assigned type, trait, and function ids.
        // Sort by numeric id to produce a stable ordering across runs (HashMap iteration
        // order is nondeterministic; sorting by id preserves pre-pass insertion order).
        let root_id = Id(0);
        let mut top_level_ids: Vec<Id> = state
            .local_identity_to_id
            .values()
            .flat_map(|entries| entries.iter().map(|(_, id)| *id))
            .chain(state.fn_path_to_id.values().copied())
            .collect();
        top_level_ids.sort_unstable_by_key(|id| id.0);
        top_level_ids.dedup();
        let root_item = make_item(
            root_id,
            Some(doc.crate_name().as_str().to_string()),
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
    /// Resolves a type or trait catalogue key through the shared rustdoc and
    /// catalogue resolution set, retaining the caller's namespace.
    pub(super) fn resolve_catalogue_key_identity(
        &self,
        key: &CatalogueEntryKey,
        namespace: CatalogueItemNamespace,
    ) -> Result<FullyQualifiedItemPath, NewTypeGraphCodecError> {
        let reference = TypeRef::new(key.as_str().to_owned())
            .map_err(|_| invalid_type_ref(key.as_str(), "catalogue key is empty"))?;
        let universe = self
            .resolution_paths
            .values()
            .filter_map(summary_identity)
            .collect::<std::collections::BTreeSet<_>>();
        resolve_catalogue_identity_in_namespace(
            &reference,
            &self.crate_name,
            &universe,
            Some(namespace),
        )
        .map_err(map_identity_resolution_error)
    }
}

fn entry_scope_key(key: &CatalogueEntryKey, namespace: CatalogueItemNamespace) -> String {
    let prefix = match namespace {
        CatalogueItemNamespace::Type => "type",
        CatalogueItemNamespace::Trait => "trait",
    };
    format!("{prefix}:{}", key.as_str())
}
