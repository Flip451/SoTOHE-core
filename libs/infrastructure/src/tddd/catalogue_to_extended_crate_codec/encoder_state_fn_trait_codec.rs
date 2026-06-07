//! `EncoderState` methods for encoding `TraitEntry`, `FunctionEntry`, methods,
//! and trait-impl resolution.

use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry};
use domain::tddd::catalogue_v2::{FunctionPath, MethodDeclaration, ModulePath, TraitName};
use rustdoc_types::{
    FunctionHeader, FunctionSignature, GenericBound, Id, ItemEnum, ItemKind, ItemSummary, Path,
    Trait, Type,
};

use crate::tddd::catalogue_to_extended_crate_codec_error::CatalogueToExtendedCrateCodecError;

use super::encoder::EncoderState;
use super::helpers::{
    is_bare_generic_name, make_item, make_item_with_crate_id, receiver_type, rewrite_generic_types,
};

impl EncoderState {
    /// Encodes a `TraitEntry` into a `Trait` item.
    pub(super) fn encode_trait(
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
    pub(super) fn encode_function(
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
    pub(super) fn encode_method_items(
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

    /// Resolves a `trait_ref` TypeRef string to a rustdoc `Path` for use in the
    /// top-level `trait_impls` encoding loop (ADR `2026-05-20-0048` D2/D4).
    ///
    /// The `trait_ref` is the full TypeRef string from `TraitImplDeclV2.trait_ref`,
    /// e.g. `"core::convert::From<MyError>"`, `"serde::Serialize"`, `"MyLocalTrait"`,
    /// or `"my_crate::MyLocalTrait"` (self-crate prefix form).
    ///
    /// Strategy:
    ///
    /// 1. If `trait_ref_str` starts with `"{crate_name}::"`, strip the self-crate prefix
    ///    so that the trait resolves as a local catalogue entry (crate_id = 0) rather than
    ///    being spuriously registered as an external crate.  The last segment of the base
    ///    path is used as the normalized short name (e.g. `"my_crate::module::MyTrait"` →
    ///    `"MyTrait"`); generic args (if any) are re-appended verbatim.
    /// 2. Pass the (possibly normalized) string to `parse_type_ref_str` to validate and
    ///    resolve it, registering any newly-discovered external crates in generic args.
    /// 3. Return the parsed `Path` verbatim: `Path.path` is the canonical base path and
    ///    `Path.args` carries the structured generic args (ADR `2026-05-20-0048` D2).
    ///
    /// # Errors
    ///
    /// Returns `InvalidTypeRef` if `trait_ref_str` cannot be parsed as a valid type-ref
    /// path (caught by `parse_type_ref_str` / syn), or if the result is not a
    /// `ResolvedPath` (e.g. a primitive or reference type).
    pub(super) fn resolve_trait_ref_for_top_level(
        &mut self,
        trait_ref_str: &str,
    ) -> Result<Path, CatalogueToExtendedCrateCodecError> {
        // Normalize self-crate-prefixed trait refs to their short name before parsing,
        // so that `my_crate::MyTrait` resolves as a local catalogue entry (crate_id = 0)
        // rather than being spuriously registered as an external crate reference.
        //
        // Without this step, `parse_type_ref_str("my_crate::MyTrait")` sees a
        // multi-segment path whose first segment is not a Rust path keyword (`crate` /
        // `self` / `super`), so it calls `emit_external_crate("my_crate")` and eventually
        // produces a `Path.id` that ends up in `krate.paths` with `crate_id != 0`.
        // `build_impl_identity_map` then treats it as an external trait and keeps the full
        // `"my_crate::MyTrait"` in the identity key, while the C-side rustdoc entry has
        // `crate_id == 0` (local trait) and emits the bare short name `"MyTrait"`.  The
        // resulting key mismatch causes Reference / Modify / Delete entries to miss their
        // B-side counterpart and Add entries to collide with existing B-side impls.
        //
        // Algorithm:
        //   1. If `trait_ref_str` starts with `"{crate_name}::"`, strip that prefix.
        //   2. Separate base path from generic args (first `<`).
        //   3. Take the last `::` segment of the remaining base (handles sub-module paths
        //      like `my_crate::module::MyTrait` → `MyTrait`).
        //   4. Rejoin with the generic-arg suffix.
        //
        // The resulting normalized string (e.g. `"MyTrait"` or `"MyTrait<Foo>"`) is then
        // resolved via `parse_type_ref_str` → `resolve_local("MyTrait")` → local id.
        //
        // For non-self-crate trait refs (external crates or bare names already without
        // a crate prefix), `trait_ref_str` is passed through unchanged.
        //
        // The parsed `Path` is returned verbatim: `path` is the canonical base path and
        // `args` carries the structured generic args (ADR `2026-05-20-0048` D2).
        let crate_prefix = format!("{}::", self.crate_name.as_str());
        let normalized_owned: Option<String> =
            if let Some(after_prefix) = trait_ref_str.strip_prefix(crate_prefix.as_str()) {
                // Separate base path from generic args to avoid splitting on `::` inside `<>`.
                let angle_pos = after_prefix.find('<').unwrap_or(after_prefix.len());
                let base = &after_prefix[..angle_pos];
                let args = &after_prefix[angle_pos..];
                // Last segment of the base (strips sub-module path if present).
                let last_seg = base.rsplit("::").next().unwrap_or(base);
                Some(format!("{last_seg}{args}"))
            } else {
                None
            };
        let normalized_str: &str = normalized_owned.as_deref().unwrap_or(trait_ref_str);

        let parsed_ty = self.parse_type_ref_str(normalized_str)?;
        match parsed_ty {
            Type::ResolvedPath(p) => Ok(p),
            other => Err(CatalogueToExtendedCrateCodecError::InvalidTypeRef {
                type_ref: trait_ref_str.to_string(),
                reason: format!(
                    "trait_ref must resolve to a path type, got {:?}",
                    std::mem::discriminant(&other)
                ),
            }),
        }
    }
}
