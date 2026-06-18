//! Internal parse context and conversion logic.

use std::collections::HashMap;

use rustdoc_types::{
    AssocItemConstraint, AssocItemConstraintKind, DynTrait, GenericArg, GenericArgs, GenericBound,
    GenericParamDef, GenericParamDefKind, Id, Path, PolyTrait, TraitBoundModifier, Type,
};

use super::constants::{PRIMITIVE_TYPES, STD_PRELUDE_TYPES, UNRESOLVED_CRATE_ID};
use super::helpers::{
    array_len_to_string, expr_to_token_string, std_canonical_path, syn_expr_to_string,
    unresolved_type,
};

// ---------------------------------------------------------------------------
// ParseCtx definition
// ---------------------------------------------------------------------------

pub(super) struct ParseCtx<'a, F, G> {
    pub(super) resolve_local: &'a F,
    pub(super) external_crate_ids: &'a HashMap<String, u32>,
    pub(super) emit_external_crate: &'a mut G,
}

impl<'a, F, G> ParseCtx<'a, F, G>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    pub(super) fn convert_type(&mut self, ty: &syn::Type) -> Type {
        match ty {
            syn::Type::Path(type_path) => self.convert_type_path(type_path),
            syn::Type::Tuple(type_tuple) => self.convert_tuple(type_tuple),
            syn::Type::Slice(type_slice) => self.convert_slice(type_slice),
            syn::Type::Array(type_array) => self.convert_array(type_array),
            syn::Type::Reference(type_ref) => self.convert_reference(type_ref),
            syn::Type::Ptr(type_ptr) => self.convert_ptr(type_ptr),
            syn::Type::Never(_) => Type::Primitive("never".to_string()),
            syn::Type::Infer(_) => Type::Infer,
            // `dyn Trait + Trait2` — encode each bound as a `PolyTrait` entry.
            syn::Type::TraitObject(trait_obj) => self.convert_dyn_trait(trait_obj),
            // `impl Trait + Trait2` — encode each bound as a `GenericBound`.
            syn::Type::ImplTrait(impl_trait) => self.convert_impl_trait(impl_trait),
            // `(T)` — parenthesized type; unwrap to the inner type.
            syn::Type::Paren(paren) => self.convert_type(&paren.elem),
            // `fn(A, B) -> C` — function pointer; encode with minimal signature.
            syn::Type::BareFn(bare_fn) => self.convert_bare_fn(bare_fn),
            _ => unresolved_type("<unknown_type>"),
        }
    }

    fn convert_type_path(&mut self, type_path: &syn::TypePath) -> Type {
        // Collect path segments.
        let segments: Vec<_> = type_path.path.segments.iter().collect();
        if segments.is_empty() {
            return unresolved_type("<empty_path>");
        }

        // Qualified path (`<T as Trait>::Assoc`) — build a full `Type::QualifiedPath`.
        //
        // ADR 2026-06-18-0822 D1 needs this faithful `QualifiedPath` shape so
        // GAT projections compare against rustdoc instead of collapsing to an
        // unresolved marker. `syn::QSelf.position` is the index into
        // `type_path.path.segments` that marks the boundary between the trait
        // prefix and the associated-item name:
        //   - `segments[..position]` → trait path (may be empty when position == 0)
        //   - `segments[position]`   → associated item name + its generic args
        if let Some(qself) = type_path.qself.as_ref() {
            // 1. Recursively convert the self type.
            let self_type = Box::new(self.convert_type(&qself.ty));

            // 2. Build the trait path from the prefix segments (before `position`).
            //    When position == 0 there is no trait prefix, so trait_ is None.
            let trait_ = if qself.position == 0 {
                None
            } else {
                // Reconstruct a `syn::Path` from the prefix segments and resolve it.
                let prefix_segs: syn::punctuated::Punctuated<syn::PathSegment, syn::Token![::]> =
                    type_path.path.segments.iter().take(qself.position).cloned().collect();
                let trait_syn_path = syn::Path { leading_colon: None, segments: prefix_segs };
                Some(self.resolve_trait_bound_path(&trait_syn_path))
            };

            // 3. The associated-item segment (at `position`): name + generic args.
            let Some(assoc_seg) = segments.get(qself.position).copied() else {
                return unresolved_type("<qualified_path_missing_assoc>");
            };
            if segments.len() != qself.position.saturating_add(1) {
                return unresolved_type("<qualified_path_trailing_segments>");
            }
            let name = assoc_seg.ident.to_string();
            let args = self.convert_generic_args(&assoc_seg.arguments);

            return Type::QualifiedPath { name, self_type, trait_, args: args.map(Box::new) };
        }

        // Multi-segment: first segment is a crate name prefix.
        if segments.len() > 1 {
            return self.convert_crate_prefixed_path(&segments);
        }

        // Single-segment. We already checked `segments.is_empty()` above.
        let Some(first_seg) = segments.first() else {
            return unresolved_type("<empty_path>");
        };
        let name = first_seg.ident.to_string();

        // 1. Rust primitive?
        if PRIMITIVE_TYPES.contains(&name.as_str()) {
            return Type::Primitive(name);
        }

        // 2. Unit type `()`?  (expressed as empty-path in some syn versions)
        if name == "()" {
            return Type::Tuple(vec![]);
        }

        // 3. Local catalogue declaration?
        //
        // Checked BEFORE the `Self` keyword and the std prelude allowlist so that a
        // catalogue type with the same spelling as a prelude item or the `Self` keyword
        // (highly unusual but technically possible) is resolved to the local item id
        // rather than silently mapped to a sentinel or std path.
        if let Some(local_id) = (self.resolve_local)(&name) {
            let generic_args = self.convert_generic_args(&first_seg.arguments);
            return Type::ResolvedPath(Path {
                path: name,
                id: local_id,
                args: generic_args.map(Box::new),
            });
        }

        // 4. `Self` keyword — represents the implementing type. Encoded as a
        //    sentinel `ResolvedPath` with `Id(0)` (root module placeholder).
        //    Callers that need to remap `Self` to a concrete type do so after
        //    codec time.
        if name == "Self" {
            return Type::ResolvedPath(Path { path: "Self".to_string(), id: Id(0), args: None });
        }

        // 5. std prelude allowlist?
        //
        // `Path.id` is an item id, not a crate id. For external types we do not have
        // the actual item id, so we use the unresolved marker sentinel (`UNRESOLVED_CRATE_ID`).
        // Use the canonical `std::module::TypeName` path so downstream consumers can
        // distinguish real std types from truly-unresolved identifiers.
        if STD_PRELUDE_TYPES.contains(&name.as_str()) {
            let generic_args = self.convert_generic_args(&first_seg.arguments);
            let canonical = std_canonical_path(&name);
            return Type::ResolvedPath(Path {
                path: canonical,
                id: Id(UNRESOLVED_CRATE_ID),
                args: generic_args.map(Box::new),
            });
        }

        // 6. Unresolved marker.
        let generic_args = self.convert_generic_args(&first_seg.arguments);
        Type::ResolvedPath(Path {
            path: name,
            id: Id(UNRESOLVED_CRATE_ID),
            args: generic_args.map(Box::new),
        })
    }

    fn convert_crate_prefixed_path(&mut self, segments: &[&syn::PathSegment]) -> Type {
        // Caller guarantees segments.len() > 1 (multi-segment path).
        let Some(first_seg) = segments.first() else {
            return unresolved_type("<empty_crate_path>");
        };
        let first_name = first_seg.ident.to_string();
        let Some(last_seg) = segments.last() else {
            return unresolved_type("<empty_crate_path>");
        };
        let full_path = segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::");

        // `crate`, `self`, `super` are Rust path keywords, not external crate names.
        // For relative/self-referential paths, attempt to resolve the last segment
        // against the local catalogue; fall back to an unresolved marker without
        // registering a spurious `external_crates` entry.
        let is_path_keyword = matches!(first_name.as_str(), "crate" | "self" | "super");
        if is_path_keyword {
            let last_name = last_seg.ident.to_string();
            let generic_args = self.convert_generic_args(&last_seg.arguments);
            if let Some(local_id) = (self.resolve_local)(&last_name) {
                return Type::ResolvedPath(Path {
                    path: full_path,
                    id: local_id,
                    args: generic_args.map(Box::new),
                });
            }
            return Type::ResolvedPath(Path {
                path: full_path,
                id: Id(UNRESOLVED_CRATE_ID),
                args: generic_args.map(Box::new),
            });
        }

        // Regular multi-segment path: first segment is an external crate name.
        // Ensure the external crate is registered (so Crate::external_crates is populated),
        // but do not store the crate id in Path.id — Path.id is an item id.
        // Use UNRESOLVED_CRATE_ID as the item id sentinel for external types.
        if !self.external_crate_ids.contains_key(&first_name) {
            (self.emit_external_crate)(first_name);
        }

        let generic_args = self.convert_generic_args(&last_seg.arguments);

        Type::ResolvedPath(Path {
            path: full_path,
            id: Id(UNRESOLVED_CRATE_ID),
            args: generic_args.map(Box::new),
        })
    }

    fn convert_tuple(&mut self, type_tuple: &syn::TypeTuple) -> Type {
        let elems: Vec<Type> = type_tuple.elems.iter().map(|t| self.convert_type(t)).collect();
        Type::Tuple(elems)
    }

    fn convert_slice(&mut self, type_slice: &syn::TypeSlice) -> Type {
        let inner = self.convert_type(&type_slice.elem);
        Type::Slice(Box::new(inner))
    }

    fn convert_array(&mut self, type_array: &syn::TypeArray) -> Type {
        let inner = self.convert_type(&type_array.elem);
        let len_str = array_len_to_string(&type_array.len);
        Type::Array { type_: Box::new(inner), len: len_str }
    }

    fn convert_reference(&mut self, type_ref: &syn::TypeReference) -> Type {
        let inner = self.convert_type(&type_ref.elem);
        let is_mutable = type_ref.mutability.is_some();
        let lifetime = type_ref.lifetime.as_ref().map(|lt| format!("'{}", lt.ident));
        Type::BorrowedRef { lifetime, is_mutable, type_: Box::new(inner) }
    }

    fn convert_ptr(&mut self, type_ptr: &syn::TypePtr) -> Type {
        let inner = self.convert_type(&type_ptr.elem);
        let is_mutable = type_ptr.mutability.is_some();
        Type::RawPointer { is_mutable, type_: Box::new(inner) }
    }

    /// Resolves a `syn::Path` (from a trait bound) to a `rustdoc_types::Path`.
    ///
    /// Runs the same resolution logic as `convert_type_path` for the trait name:
    /// local catalogue check → std prelude allowlist → external crate registration.
    /// This ensures that `dyn Clone`, `impl Iterator<Item = T>`, and
    /// `dyn serde::Serialize` are resolved consistently with plain path types.
    pub(super) fn resolve_trait_bound_path(&mut self, syn_path: &syn::Path) -> Path {
        let segments: Vec<_> = syn_path.segments.iter().collect();
        let full_path = segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::");

        // Multi-segment: treat first as a crate/module prefix.
        if segments.len() > 1 {
            let Some(first_seg_ref) = segments.first() else {
                return Path { path: full_path, id: Id(UNRESOLVED_CRATE_ID), args: None };
            };
            let first_name = first_seg_ref.ident.to_string();
            let Some(last_seg) = segments.last() else {
                return Path { path: full_path, id: Id(UNRESOLVED_CRATE_ID), args: None };
            };
            let args = self.convert_generic_args(&last_seg.arguments);

            let is_path_keyword = matches!(first_name.as_str(), "crate" | "self" | "super");
            let id = if is_path_keyword {
                let last_name = last_seg.ident.to_string();
                if let Some(local_id) = (self.resolve_local)(&last_name) {
                    local_id
                } else {
                    Id(UNRESOLVED_CRATE_ID)
                }
            } else {
                // Register external crate if new.
                if !self.external_crate_ids.contains_key(&first_name) {
                    (self.emit_external_crate)(first_name);
                }
                Id(UNRESOLVED_CRATE_ID)
            };
            return Path { path: full_path, id, args: args.map(Box::new) };
        }

        // Single-segment: run the same resolution chain as `convert_type_path`.
        let Some(first_seg) = segments.first() else {
            return Path { path: full_path, id: Id(UNRESOLVED_CRATE_ID), args: None };
        };
        let name = first_seg.ident.to_string();
        let args = self.convert_generic_args(&first_seg.arguments);

        // Local catalogue?
        if let Some(local_id) = (self.resolve_local)(&name) {
            return Path { path: name, id: local_id, args: args.map(Box::new) };
        }

        // std prelude?
        if STD_PRELUDE_TYPES.contains(&name.as_str()) {
            let canonical = std_canonical_path(&name);
            return Path { path: canonical, id: Id(UNRESOLVED_CRATE_ID), args: args.map(Box::new) };
        }

        // Unresolved marker.
        Path { path: name, id: Id(UNRESOLVED_CRATE_ID), args: args.map(Box::new) }
    }

    /// Converts a `dyn Trait + Trait2` type object into `Type::DynTrait`.
    ///
    /// Each `TypeParamBound::Trait` in the bound list becomes a `PolyTrait` entry.
    /// Trait paths are resolved via `resolve_trait_bound_path` (same logic as
    /// `convert_type_path`), so local catalogue types and std prelude entries are
    /// handled consistently.
    /// Lifetime bounds are recorded in `DynTrait::lifetime` (first one wins; rare in
    /// catalogue `TypeRef` strings).
    fn convert_dyn_trait(&mut self, trait_obj: &syn::TypeTraitObject) -> Type {
        let mut traits = vec![];
        let mut lifetime: Option<String> = None;
        for bound in &trait_obj.bounds {
            match bound {
                syn::TypeParamBound::Trait(tb) => {
                    let trait_path = self.resolve_trait_bound_path(&tb.path);
                    traits.push(PolyTrait { trait_: trait_path, generic_params: vec![] });
                }
                syn::TypeParamBound::Lifetime(lt) => {
                    if lifetime.is_none() {
                        lifetime = Some(format!("'{}", lt.ident));
                    }
                }
                _ => {}
            }
        }
        Type::DynTrait(DynTrait { traits, lifetime })
    }

    /// Converts an `impl Trait + Trait2` type into `Type::ImplTrait`.
    ///
    /// Each `TypeParamBound::Trait` becomes a `GenericBound::TraitBound`, with the
    /// trait path resolved via `resolve_trait_bound_path`.
    fn convert_impl_trait(&mut self, impl_trait: &syn::TypeImplTrait) -> Type {
        let mut bounds = vec![];
        for bound in &impl_trait.bounds {
            match bound {
                syn::TypeParamBound::Trait(tb) => {
                    let trait_path = self.resolve_trait_bound_path(&tb.path);
                    bounds.push(GenericBound::TraitBound {
                        trait_: trait_path,
                        generic_params: vec![],
                        modifier: TraitBoundModifier::None,
                    });
                }
                syn::TypeParamBound::Lifetime(lt) => {
                    bounds.push(GenericBound::Outlives(format!("'{}", lt.ident)));
                }
                _ => {}
            }
        }
        Type::ImplTrait(bounds)
    }

    /// Converts a bare function pointer `fn(A, B) -> C` into `Type::FunctionPointer`.
    ///
    /// The parameter types are encoded via `convert_type`; the return type is
    /// treated as `None` for `-> ()` or encoded inline. The ABI is encoded from
    /// the `extern "..."` annotation, and HRTB lifetime binders (`for<'a>`) are
    /// encoded into `generic_params`.
    fn convert_bare_fn(&mut self, bare_fn: &syn::TypeBareFn) -> Type {
        let inputs: Vec<(String, Type)> = bare_fn
            .inputs
            .iter()
            .enumerate()
            .map(|(i, arg)| {
                let name = arg
                    .name
                    .as_ref()
                    .map(|(n, _)| n.to_string())
                    .unwrap_or_else(|| format!("_arg{i}"));
                let ty = self.convert_type(&arg.ty);
                (name, ty)
            })
            .collect();
        let output = match &bare_fn.output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_, ty) => {
                let converted = self.convert_type(ty);
                // Treat explicit `-> ()` as no output (consistent with encode_method_items).
                if matches!(&converted, Type::Tuple(v) if v.is_empty()) {
                    None
                } else {
                    Some(converted)
                }
            }
        };
        let is_unsafe = bare_fn.unsafety.is_some();
        let abi = syn_abi_to_rustdoc_abi(bare_fn.abi.as_ref());
        let generic_params = bound_lifetimes_to_generic_params(bare_fn.lifetimes.as_ref());
        Type::FunctionPointer(Box::new(rustdoc_types::FunctionPointer {
            sig: rustdoc_types::FunctionSignature {
                inputs,
                output,
                is_c_variadic: bare_fn.variadic.is_some(),
            },
            generic_params,
            header: rustdoc_types::FunctionHeader {
                is_async: false,
                is_const: false,
                is_unsafe,
                abi,
            },
        }))
    }

    pub(super) fn convert_generic_args(
        &mut self,
        args: &syn::PathArguments,
    ) -> Option<GenericArgs> {
        match args {
            syn::PathArguments::None => None,
            syn::PathArguments::AngleBracketed(ab) => {
                // Delegate to the shared angle-bracketed conversion helper that handles
                // AssocType, AssocConst, Constraint, and positional generic args.
                convert_angle_bracketed_args(self, ab)
            }
            syn::PathArguments::Parenthesized(p) => {
                let inputs: Vec<Type> = p.inputs.iter().map(|t| self.convert_type(t)).collect();
                let output = match &p.output {
                    syn::ReturnType::Default => None,
                    syn::ReturnType::Type(_, ty) => Some(self.convert_type(ty)),
                };
                Some(GenericArgs::Parenthesized { inputs, output })
            }
        }
    }

    pub(super) fn convert_generic_arg(&mut self, arg: &syn::GenericArgument) -> Option<GenericArg> {
        match arg {
            syn::GenericArgument::Type(ty) => Some(GenericArg::Type(self.convert_type(ty))),
            syn::GenericArgument::Lifetime(lt) => {
                Some(GenericArg::Lifetime(format!("'{}", lt.ident)))
            }
            // `Const` args (e.g. `ArrayVec<u8, 32>`): encode as `GenericArg::Const` using
            // the stringified expression. We cannot evaluate the expression, but we preserve
            // the textual form for downstream consumers.
            syn::GenericArgument::Const(expr) => {
                let expr_str = syn_expr_to_string(expr);
                Some(GenericArg::Const(rustdoc_types::Constant {
                    expr: expr_str,
                    value: None,
                    is_literal: matches!(expr, syn::Expr::Lit(_)),
                }))
            }
            // `AssocType`, `AssocConst`, and `Constraint` are handled upstream in
            // `convert_generic_args` as `AssocItemConstraint` entries.
            syn::GenericArgument::AssocType(_)
            | syn::GenericArgument::AssocConst(_)
            | syn::GenericArgument::Constraint(_) => None,
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// ParseCtx-local helpers (free functions that take a mutable ParseCtx reference)
// ---------------------------------------------------------------------------

/// Core angle-bracketed generic argument conversion.
///
/// Processes all `syn::GenericArgument` variants — `AssocType`, `AssocConst`,
/// `Constraint`, and generic args (type/lifetime/const) — into
/// `rustdoc_types::GenericArgs::AngleBracketed`.
///
/// Returns `None` when the result would be empty (no generic args and no
/// associated-item constraints).
fn convert_angle_bracketed_args<F, G>(
    ctx: &mut ParseCtx<'_, F, G>,
    ab: &syn::AngleBracketedGenericArguments,
) -> Option<GenericArgs>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    let mut generic_args: Vec<GenericArg> = vec![];
    let mut constraints: Vec<AssocItemConstraint> = vec![];
    for arg in &ab.args {
        match arg {
            syn::GenericArgument::AssocType(assoc) => {
                let ty = ctx.convert_type(&assoc.ty);
                let inner_args = angle_bracketed_to_generic_args(ctx, &assoc.generics);
                constraints.push(AssocItemConstraint {
                    name: assoc.ident.to_string(),
                    args: inner_args.map(Box::new),
                    binding: AssocItemConstraintKind::Equality(rustdoc_types::Term::Type(ty)),
                });
            }
            syn::GenericArgument::AssocConst(assoc_const) => {
                // `Trait<LENGTH = 32>` or `Trait<LEN<'a> = 32>` — encode the value as a
                // const expression. Use `expr_to_token_string` so binary/unary/paren/cast
                // forms preserve their actual value.
                let expr_str = expr_to_token_string(&assoc_const.value);
                let inner_args = angle_bracketed_to_generic_args(ctx, &assoc_const.generics);
                constraints.push(AssocItemConstraint {
                    name: assoc_const.ident.to_string(),
                    args: inner_args.map(Box::new),
                    binding: AssocItemConstraintKind::Equality(rustdoc_types::Term::Constant(
                        rustdoc_types::Constant {
                            expr: expr_str.clone(),
                            value: Some(expr_str),
                            is_literal: matches!(&assoc_const.value, syn::Expr::Lit(_)),
                        },
                    )),
                });
            }
            syn::GenericArgument::Constraint(c) => {
                let bounds: Vec<GenericBound> = c
                    .bounds
                    .iter()
                    .filter_map(|b| match b {
                        syn::TypeParamBound::Trait(tb) => {
                            let trait_path = ctx.resolve_trait_bound_path(&tb.path);
                            Some(GenericBound::TraitBound {
                                trait_: trait_path,
                                generic_params: vec![],
                                modifier: TraitBoundModifier::None,
                            })
                        }
                        syn::TypeParamBound::Lifetime(lt) => {
                            Some(GenericBound::Outlives(format!("'{}", lt.ident)))
                        }
                        _ => None,
                    })
                    .collect();
                constraints.push(AssocItemConstraint {
                    name: c.ident.to_string(),
                    args: None,
                    binding: AssocItemConstraintKind::Constraint(bounds),
                });
            }
            _ => {
                if let Some(ga) = ctx.convert_generic_arg(arg) {
                    generic_args.push(ga);
                }
            }
        }
    }
    if generic_args.is_empty() && constraints.is_empty() {
        None
    } else {
        Some(GenericArgs::AngleBracketed { args: generic_args, constraints })
    }
}

/// Converts `Option<syn::AngleBracketedGenericArguments>` to `Option<GenericArgs>`.
///
/// Returns `None` when the angle brackets are absent (via the `Option` wrapper
/// used in `syn::AssocType::generics` / `syn::AssocConst::generics`) or empty.
pub(super) fn angle_bracketed_to_generic_args<F, G>(
    ctx: &mut ParseCtx<'_, F, G>,
    generics: &Option<syn::AngleBracketedGenericArguments>,
) -> Option<GenericArgs>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    let ab = generics.as_ref()?;
    convert_angle_bracketed_args(ctx, ab)
}

/// Converts a `syn::Abi` to the corresponding `rustdoc_types::Abi`.
///
/// `None` (no `extern` keyword) → `Abi::Rust`.
/// `extern` without a name string → `Abi::C { unwind: false }` (C is the
/// implicit ABI for bare `extern fn`).
/// Known ABI name strings are mapped to their specific variants; any other
/// string falls through to `Abi::Other(name)`.
pub(super) fn syn_abi_to_rustdoc_abi(abi: Option<&syn::Abi>) -> rustdoc_types::Abi {
    let Some(abi) = abi else {
        return rustdoc_types::Abi::Rust;
    };
    let name = match &abi.name {
        Some(lit) => lit.value(),
        // bare `extern` with no explicit string → defaults to C ABI
        None => return rustdoc_types::Abi::C { unwind: false },
    };
    match name.as_str() {
        "C" => rustdoc_types::Abi::C { unwind: false },
        "C-unwind" => rustdoc_types::Abi::C { unwind: true },
        "cdecl" => rustdoc_types::Abi::Cdecl { unwind: false },
        "cdecl-unwind" => rustdoc_types::Abi::Cdecl { unwind: true },
        "stdcall" => rustdoc_types::Abi::Stdcall { unwind: false },
        "stdcall-unwind" => rustdoc_types::Abi::Stdcall { unwind: true },
        "fastcall" => rustdoc_types::Abi::Fastcall { unwind: false },
        "fastcall-unwind" => rustdoc_types::Abi::Fastcall { unwind: true },
        "aapcs" => rustdoc_types::Abi::Aapcs { unwind: false },
        "aapcs-unwind" => rustdoc_types::Abi::Aapcs { unwind: true },
        "win64" => rustdoc_types::Abi::Win64 { unwind: false },
        "win64-unwind" => rustdoc_types::Abi::Win64 { unwind: true },
        "sysv64" => rustdoc_types::Abi::SysV64 { unwind: false },
        "sysv64-unwind" => rustdoc_types::Abi::SysV64 { unwind: true },
        "system" => rustdoc_types::Abi::System { unwind: false },
        "system-unwind" => rustdoc_types::Abi::System { unwind: true },
        other => rustdoc_types::Abi::Other(other.to_string()),
    }
}

/// Converts `syn::BoundLifetimes` (HRTB `for<'a, 'b>`) to a list of
/// `rustdoc_types::GenericParamDef` with `kind: Lifetime { outlives: [] }`.
///
/// Returns an empty `Vec` when `lifetimes` is `None`.
pub(super) fn bound_lifetimes_to_generic_params(
    lifetimes: Option<&syn::BoundLifetimes>,
) -> Vec<GenericParamDef> {
    let Some(bl) = lifetimes else {
        return vec![];
    };
    bl.lifetimes
        .iter()
        .filter_map(|param| {
            if let syn::GenericParam::Lifetime(lt_param) = param {
                // `syn::Lifetime.ident` omits the leading apostrophe; rustdoc stores
                // lifetime strings WITH the apostrophe (e.g. `"'a"`, `"'_"`).
                // Re-prepend `'` so A-codec GenericParamDef names match C-side names.
                let outlives: Vec<String> =
                    lt_param.bounds.iter().map(|lt| format!("'{}", lt.ident)).collect();
                Some(GenericParamDef {
                    name: format!("'{}", lt_param.lifetime.ident),
                    kind: GenericParamDefKind::Lifetime { outlives },
                })
            } else {
                None
            }
        })
        .collect()
}
